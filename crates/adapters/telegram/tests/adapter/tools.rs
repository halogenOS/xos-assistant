//! AC2 of the tools unit: the whole tool round trip over the adapter — a
//! scripted update, the tool-scripted provider, the commit lookup against a
//! scripted loopback forge, and the answer out through the scripted
//! server's `sendMessage` — asserted on the ledger block by block, tool
//! call and result included. The narration variant proves both of the
//! turn's texts reach the chat.

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
    message_id_of, private_update, recording_sleep, spawn_adapter, start_assistant_with_tools,
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
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[2], "text"), "what changed in the manifest?");
    assert_eq!(field(&blocks[3], "name"), commit::NAME);
    assert_eq!(field(&blocks[3], "input"), COMMIT_INPUT);
    assert_eq!(field(&blocks[4], "content"), compact_result());
    assert_eq!(
        field(&blocks[5], "content"),
        support::disclosed(TOOL_CLOSING_ANSWER)
    );

    // The tool executed against the loopback forge, once, at the dialect's
    // path.
    assert_eq!(
        forge.paths(),
        vec!["/api/v1/repos/halogenOS/android_manifest/git/commits/deadbeef".to_owned()]
    );
}

/// The narration variant: a turn that narrates before calling the tool
/// delivers both texts to the chat — the narration and the closing answer,
/// in that order — with both standing in the ledger, and each threaded
/// onto the message it answers.
///
/// Both threading is the rule doing its job, not a slip (unit 26): each
/// delivered answer names the one message that addressed the assistant
/// among what the turn had absorbed when that text was written, and here
/// that is the same ask for both. Threading only the turn's first text
/// would put the quote on "let me look that up" and leave the answer
/// itself loose, and a turn's last text is not knowable at delivery: a
/// text is delivered when its stream ends, rounds before the turn does.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn narration_before_the_call_delivers_both_texts_to_the_chat() {
    const NARRATION: &str = "Let me look that commit up.";
    let forge = ScriptedForge::start().await;
    let fixture = start_assistant_with_tools(
        Some(ToolScript {
            tool: commit::NAME.into(),
            input: COMMIT_INPUT.into(),
            narration: Some(NARRATION.into()),
        }),
        commit_tools(&forge),
    )
    .await;
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 42, "what changed?"));

    let state = TempStateFile::new("tool-narration");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Both texts reach the chat, narration first.
    let sends = server.await_recorded("sendMessage", 2).await;
    let texts: Vec<&str> = sends
        .iter()
        .filter_map(|send| send.body["text"].as_str())
        .collect();
    // The narration is the person's first answer block and carries the
    // line; the closing answer behind it arrives bare.
    let introduced = support::disclosed(NARRATION);
    assert_eq!(texts, vec![introduced.as_str(), TOOL_CLOSING_ANSWER]);
    for send in &sends {
        assert_eq!(
            send.body["reply_parameters"]["message_id"],
            json!(message_id_of(1)),
            "every text this turn delivers is a reply to the one message \
             that addressed the assistant"
        );
    }

    // Both stand in the ledger, in the production order: the message end
    // finalizes the narration text before the drained tool lifecycle
    // inserts the call block, so the narration precedes the call.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = await_shape(
        &fixture.store,
        conversation,
        &[
            "system_prompt",
            "tool_choice",
            "chat_message",
            "text",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[3], "content"), introduced);
    assert_eq!(field(&blocks[6], "content"), TOOL_CLOSING_ANSWER);
}
