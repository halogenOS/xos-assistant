//! AC2 of the tools unit: the whole tool round trip over the adapter — a
//! scripted update, the tool-scripted provider, the commit lookup against a
//! scripted loopback forge, and the answer out through the scripted
//! server's `sendMessage` — asserted on the ledger block by block, tool
//! call and result included. The announce variant proves a turn that
//! announces beside its call puts two messages in the chat, and that the
//! notes it wrote reach nobody.

use std::sync::{Arc, Mutex};

use agent_ledger::Block;
use assistant_core::tools::ToolSet;
use assistant_core::tools::commit::{self, CommitLookup};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::server::BotApiServer;
use crate::support::{
    self, TOOL_CLOSING_ANSWER, TempStateFile, ToolScript, await_conversations, await_shape,
    private_update, recording_sleep, spawn_adapter, start_assistant_with_tools,
};

/// The scripted call's input — non-empty by the script's contract.
const COMMIT_INPUT: &str = r#"{"repository":"android_manifest","reference":"deadbeef"}"#;

/// The scripted forge: one loopback listener answering every GET with the
/// fixed Forgejo-shaped commit body and recording each request path.
struct ScriptedForge {
    base: String,
    paths: Arc<Mutex<Vec<String>>>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl Drop for ScriptedForge {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

impl ScriptedForge {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a loopback listener binds");
        let addr = listener.local_addr().expect("the bound address reads");
        let paths = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&paths);
        let accept_task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let recorded = Arc::clone(&recorded);
                tokio::spawn(async move {
                    let mut buffered: Vec<u8> = Vec::new();
                    loop {
                        if buffered.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                        let mut chunk = [0_u8; 4096];
                        match stream.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(read) => buffered.extend_from_slice(&chunk[..read]),
                        }
                    }
                    let head = String::from_utf8_lossy(&buffered).into_owned();
                    let path = head
                        .lines()
                        .next()
                        .and_then(|line| line.split(' ').nth(1))
                        .unwrap_or_default()
                        .to_owned();
                    recorded.lock().expect("the path log locks").push(path);
                    let body = json!({
                        "sha": "deadbeef00112233445566778899aabbccddeeff",
                        "html_url": "https://example.invalid/commit/deadbeef",
                        "commit": {
                            "message": "Track the manifest\n",
                            "author": {
                                "name": "A. Committer",
                                "date": "2026-08-17T01:23:26+02:00"
                            }
                        }
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self {
            base: format!("http://{addr}"),
            paths,
            accept_task,
        }
    }

    fn paths(&self) -> Vec<String> {
        self.paths.lock().expect("the path log locks").clone()
    }
}

/// The compact result the scripted forge's answer decodes to.
fn compact_result() -> String {
    "Commit deadbeef0011 in halogenOS/android_manifest\n\
     Subject: Track the manifest\n\
     Author: A. Committer\n\
     Date: 2026-08-17T01:23:26+02:00\n\
     Link: https://example.invalid/commit/deadbeef"
        .to_owned()
}

/// A tool set holding the commit lookup against the scripted forge.
fn commit_tools(forge: &ScriptedForge) -> ToolSet {
    let mut tools = ToolSet::new();
    tools.admit(CommitLookup::new(
        forge.base.clone(),
        commit::DEFAULT_TIMEOUT,
    ));
    tools
}

/// The stored field of one block, as text.
fn field(block: &Block, name: &str) -> String {
    block.fields[name].as_str().unwrap_or_default().to_owned()
}

/// The whole tool round trip: an addressed question makes the scripted
/// model call the commit lookup, the tool executes against the loopback
/// forge, the result reaches the model's second request — the closing prose
/// only streams for a request carrying the answered call — and the answer
/// reaches the chat.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_addressed_question_runs_the_commit_lookup_end_to_end() {
    let forge = ScriptedForge::start().await;
    let fixture = start_assistant_with_tools(
        Some(ToolScript {
            tool: commit::NAME.into(),
            input: COMMIT_INPUT.into(),
            narration: None,
            announce: None,
        }),
        commit_tools(&forge),
    )
    .await;
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 42, "what changed in the manifest?"));

    let state = TempStateFile::new("tool-e2e");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The answer reaches the chat.
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(42));
    assert_eq!(
        sends[0].body["text"],
        json!(support::disclosed(TOOL_CLOSING_ANSWER))
    );

    // The ledger, block by block, tool call and result included.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = await_shape(
        &fixture.store,
        conversation,
        &[
            "system_prompt",
            "tool_choice",
            "chat_message",
            "tool_call",
            "tool_result",
            "tool_call",
            assistant_core::outgoing::OUTGOING_MESSAGE_KIND,
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[2], "text"), "what changed in the manifest?");
    assert_eq!(field(&blocks[3], "name"), commit::NAME);
    assert_eq!(field(&blocks[3], "input"), COMMIT_INPUT);
    assert_eq!(field(&blocks[4], "content"), compact_result());
    // The closing round SENDS its answer (unit 55): the call names the
    // sending tool, the outgoing block carries the words the chat received
    // — the disclosure line composed in — and the turn's own text behind
    // the send's receipt is the notes, which reach nobody.
    assert_eq!(field(&blocks[5], "name"), assistant_core::tools::send::NAME);
    assert_eq!(
        field(&blocks[6], assistant_core::outgoing::COLUMN_TEXT),
        support::disclosed(TOOL_CLOSING_ANSWER)
    );
    assert_eq!(field(&blocks[8], "content"), TOOL_CLOSING_ANSWER);

    // The tool executed against the loopback forge, once, at the dialect's
    // path.
    assert_eq!(
        forge.paths(),
        vec!["/api/v1/repos/halogenOS/android_manifest/git/commits/deadbeef".to_owned()]
    );
}

/// The announce variant: a turn that ANNOUNCES beside its call puts two
/// messages in the chat — the heads-up and the closing answer, in that
/// order — while the narration it wrote in the same round reaches nobody.
///
/// The heads-up before slow work is a message like any other from unit 55
/// on, so it goes out through a sending tool; the turn's own text is
/// private notes, and the chat never receives a word of it. Both facts are
/// read on the wire together: exactly two sends, neither of them the
/// narration.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_announce_beside_the_call_delivers_two_messages_and_the_notes_reach_nobody() {
    const NARRATION: &str = "The commit lookup is the right tool here.";
    const ANNOUNCE: &str = "Let me look that commit up.";
    let forge = ScriptedForge::start().await;
    let fixture = start_assistant_with_tools(
        Some(ToolScript {
            tool: commit::NAME.into(),
            input: COMMIT_INPUT.into(),
            narration: Some(NARRATION.into()),
            announce: Some(ANNOUNCE.into()),
        }),
        commit_tools(&forge),
    )
    .await;
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 42, "what changed?"));

    let state = TempStateFile::new("tool-announce");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Both messages reach the chat, the announce first. It is this person's
    // first delivery and so carries the disclosure line; the closing answer
    // behind it arrives bare.
    let sends = server.await_recorded("sendMessage", 2).await;
    let texts: Vec<&str> = sends
        .iter()
        .filter_map(|send| send.body["text"].as_str())
        .collect();
    let introduced = support::disclosed(ANNOUNCE);
    assert_eq!(texts, vec![introduced.as_str(), TOOL_CLOSING_ANSWER]);
    assert!(
        !texts.iter().any(|text| text.contains(NARRATION)),
        "the turn's own text is private notes: no send carries a word of it"
    );
    for send in &sends {
        assert_eq!(
            send.body.get("reply_parameters"),
            None,
            "a plain send threads onto nothing: the model aims a reply, and \
             this turn aimed at nothing"
        );
    }

    // The ledger holds the narration as her own text and the two sent
    // messages as outgoing blocks, in the order the chat received them.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = await_conversation_settles(&fixture.store, conversation).await;
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == "text")
            .map(|block| field(block, "content"))
            .collect::<Vec<_>>(),
        vec![NARRATION.to_owned(), TOOL_CLOSING_ANSWER.to_owned()],
        "her own texts are the narration and the notes the closing round wrote"
    );
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND)
            .map(|block| field(block, assistant_core::outgoing::COLUMN_TEXT))
            .collect::<Vec<_>>(),
        vec![introduced, TOOL_CLOSING_ANSWER.to_owned()],
        "the ledger's sent messages are the announce and then the answer"
    );
}

/// Await the ledger settling on the announced turn's whole shape: the two
/// sent messages, every call resolved, and both of the turn's own texts.
///
/// A shape pin would have to fix the order of the announce's own resolution
/// against the lookup's, and those two resolve on different clocks — the
/// platform's round trip and the forge's. The settled counts are the honest
/// reading.
async fn await_conversation_settles(store: &agent_ledger::Store, conversation: i64) -> Vec<Block> {
    let deadline = std::time::Instant::now() + support::DEADLINE;
    loop {
        let blocks = store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let sent = blocks
            .iter()
            .filter(|block| block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND)
            .count();
        let results = blocks
            .iter()
            .filter(|block| block.block_type == "tool_result")
            .count();
        let texts = blocks
            .iter()
            .filter(|block| block.block_type == "text")
            .count();
        if sent == 2 && results == 3 && texts == 2 {
            return blocks;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the announced turn to settle; have {:?}",
            blocks
                .iter()
                .map(|block| block.block_type.clone())
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
