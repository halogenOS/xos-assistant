//! The framework's real `OpenRouter` module — its shared chat-completions
//! wire — against a scripted loopback server: the
//! assembly registers the in-memory-configured wrapper, a message ingests,
//! the framework's own wire speaks to the server this test controls, and the
//! answer comes back over the outbound edge — with the key provably in the
//! request and provably absent from the store file.
//!
//! The loopback property is the test's own base-URL configuration: the
//! wrapper is pointed at the listener below, and the hit assertion proves
//! the traffic went there.

use std::sync::{Arc, Mutex};

use agent_ledger::{EventBus, ProviderRegistry, Store};
use assistant_core::provider::MemoryConfiguredProvider;
use assistant_core::schema::store_config;
use assistant_core::tools::{ToolSet, runtime};
use assistant_core::{Assistant, ChannelKind, ModelBinding, ReplyKind};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{self, TempDb, channel, field, inbound, recv_reply, settle_shape};

/// The fake key. Nothing real: the only server this test speaks to is its
/// own loopback listener, and the absence assertions below scan for this
/// exact string.
const FAKE_KEY: &str = "sk-or-FAKE-TEST-KEY-FOR-THE-LOOPBACK-SERVER";

/// The one answer the scripted server streams for every completion request.
const SERVER_ANSWER: &str = "The scripted provider answer.";

/// One recorded completion request: the path asked, the authorization
/// header presented, and the decoded JSON body — what the wire-shape pins
/// read the projected messages from.
#[derive(Debug, Clone)]
struct Hit {
    path: String,
    authorization: Option<String>,
    body: serde_json::Value,
}

/// The server's script: the stream one request draws, chosen from the
/// request itself. Reading the round off the wire body — never off a call
/// counter — is what keeps a script exact when a turn is redispatched,
/// exactly as the event-native fixtures script themselves by ledger
/// content.
type Script = fn(&Value) -> String;

/// A chat-completions-shaped loopback server answering every POST with the
/// single-round send script.
async fn start_completions_server() -> (String, Arc<Mutex<Vec<Hit>>>) {
    start_scripted_server(one_send_round).await
}

/// The single-round script: the answer goes out through the sending tool —
/// the one way words reach a chat since unit 55 — and the round behind it,
/// which carries the send's own result, writes the words down as the turn's
/// notes and ends.
fn one_send_round(request: &Value) -> String {
    if send_results(request) > 0 {
        return stream_of(&[text_delta(SERVER_ANSWER), finish("stop")]);
    }
    stream_of(&[send_call(0, SERVER_ANSWER), finish("tool_calls")])
}

/// The provider id every scripted SEND is made under — what tells a send's
/// own result apart from any other tool's on this wire.
const SEND_CALL_ID: &str = "send-";

/// One chunk carrying a call of the plain sending tool, under an id this
/// module's scripts can recognize the answer to.
fn send_call(round: usize, text: &str) -> Value {
    json!({ "choices": [{ "delta": { "tool_calls": [{
        "index": 0,
        "id": format!("{SEND_CALL_ID}{round}"),
        "type": "function",
        "function": {
            "name": assistant_core::tools::send::NAME,
            "arguments": json!({ "text": text }).to_string()
        }
    }] } }] })
}

/// How many of a recorded request's tool messages answer a SEND, SINCE the
/// newest thing a person said.
///
/// Scoped to this turn because a conversation keeps its history: a
/// whole-request count would carry an earlier turn's sends into the reading
/// and tell a fresh turn it had already spoken.
fn send_results(request: &Value) -> usize {
    let messages = request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let since = messages
        .iter()
        .rposition(|message| message["role"] == "user")
        .map_or(0, |at| at + 1);
    messages[since..]
        .iter()
        .filter(|message| message["role"] == "tool")
        .filter_map(|message| message["tool_call_id"].as_str())
        .filter(|id| id.starts_with(SEND_CALL_ID))
        .count()
}

/// The call ids a recorded request's tool-voiced messages answer.
fn tool_call_ids(request: &Value) -> impl Iterator<Item = &str> {
    request["messages"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|message| message["role"] == "tool")
        .filter_map(|message| message["tool_call_id"].as_str())
}

/// One server-sent stream: the given chunks, each on its own `data:` line,
/// closed by the end marker every chat-completions stream ends on.
fn stream_of(chunks: &[Value]) -> String {
    chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .chain(std::iter::once("data: [DONE]\n\n".to_owned()))
        .collect()
}

/// A chunk carrying one piece of assistant text.
fn text_delta(text: &str) -> Value {
    json!({ "choices": [{ "delta": { "content": text } }] })
}

/// The chunk that finishes a round, carrying the wire's own finish reason:
/// `stop` ends the turn, `tool_calls` hands it to the drained calls.
fn finish(reason: &str) -> Value {
    json!({ "choices": [{ "delta": {}, "finish_reason": reason }] })
}

/// A chat-completions-shaped loopback server running the given script:
/// every POST is recorded as a hit, then answered with the stream that
/// script draws for it.
async fn start_scripted_server(script: Script) -> (String, Arc<Mutex<Vec<Hit>>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback listener binds");
    let base = format!(
        "http://{}",
        listener.local_addr().expect("the address reads")
    );
    let hits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&hits);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let recorded = Arc::clone(&recorded);
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut chunk = [0_u8; 4096];
                let (head, body_start) = loop {
                    let Ok(read) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&request[..end]).into_owned();
                        let length = head
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())?
                            })
                            .unwrap_or(0);
                        // Read the whole body in, so the peer finishes
                        // writing before the response goes out and the hit
                        // records what was asked.
                        while request.len() < end + 4 + length {
                            let Ok(read) = stream.read(&mut chunk).await else {
                                return;
                            };
                            if read == 0 {
                                break;
                            }
                            request.extend_from_slice(&chunk[..read]);
                        }
                        break (head, end + 4);
                    }
                };
                let path = head
                    .lines()
                    .next()
                    .and_then(|line| line.split(' ').nth(1))
                    .unwrap_or_default()
                    .to_owned();
                let authorization = head.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("authorization")
                        .then(|| value.trim().to_owned())
                });
                let asked: Value =
                    serde_json::from_slice(&request[body_start..]).unwrap_or(Value::Null);
                let body = script(&asked);
                recorded.lock().expect("the hit log locks").push(Hit {
                    path,
                    authorization,
                    body: asked,
                });
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (base, hits)
}

/// The assembly over the framework's real module, pointed at the loopback
/// server: the module registers, the assistant starts on it, and every wire
/// pin in this suite gets the same wiring from here — one place for the
/// binding and the configuration, so a new pin adds a test instead of
/// another copy of them.
///
/// The tool set passed in is empty, which does not mean toolless: the
/// assembly admits the runtime-facts tool on nothing, so the choice these
/// turns record carries it and a call naming it resolves.
async fn start_over_the_module(store: &Store, base: &str) -> Arc<Assistant> {
    let mut providers = ProviderRegistry::new();
    let provider =
        MemoryConfiguredProvider::new(store, FAKE_KEY.into(), Some(base.to_owned())).await;
    let vendor = agent_ledger::ProviderModule::type_id(&provider).to_owned();
    providers.register(Box::new(provider));
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        Arc::new(providers),
        ToolSet::new(),
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: ModelBinding {
                // The framework's own module identifiers: its type id and
                // display name, which the binary derives the same way. Its
                // name, not this project's endpoint naming.
                provider_instance: "openrouter-1".into(),
                provider_display_name: "OpenRouter".into(),
                vendor,
                model: "test-vendor/test-model".into(),
                model_display_name: "Test Model".into(),
                context_window: None,
            },
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
    .expect("the assembly starts over the real module");
    // A send is a PENDING call since unit 55, and the delivery report is
    // what settles it: without a stand-in adapter reporting back, every
    // answered turn here would sit open on its own send and no closing
    // round would ever be asked for.
    let assistant = Arc::new(assistant);
    support::spawn_delivery_reporter(&assistant).await;
    assistant
}

/// The whole loop over the real module: ingest, answer, and the two key
/// residence facts — presented on the wire, absent from the store file.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_chat_completions_module_answers_over_the_loopback_wire_and_stores_no_key() {
    let (base, hits) = start_completions_server().await;
    let db = TempDb::new("chat-completions");
    let key = channel("dm-chat-completions");

    {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        let assistant = start_over_the_module(&store, &base).await;
        let mut replies = support::outbound_of(&assistant, &store).await;
        support::ingest_recorded(
            &assistant,
            inbound(&key, ChannelKind::Direct, "42", "ask the model"),
        )
        .await;
        let reply = recv_reply(&mut replies).await;
        assert_eq!(reply.kind, ReplyKind::Answer);
        assert_eq!(reply.text, support::disclosed(SERVER_ANSWER));
        assert_eq!(reply.channel, key);

        // The server this test controls was actually hit, on the completions
        // path, with the in-memory key on the wire.
        let recorded = hits.lock().expect("the hit log locks").clone();
        assert!(
            !recorded.is_empty(),
            "the loopback server answered the turn"
        );
        for hit in &recorded {
            assert!(
                hit.path.ends_with("/chat/completions"),
                "the module speaks the completions path; hit {hit:?}"
            );
            assert_eq!(
                hit.authorization.as_deref(),
                Some(format!("Bearer {FAKE_KEY}").as_str()),
                "the in-memory key authenticates the wire"
            );
        }
        // The assembly and the store close before the file is scanned, so
        // the scan reads settled bytes.
    }

    // The store file itself must exist for the scan to bind — a path that
    // changed shape would otherwise pass the scan vacuously; only the
    // sidecars are legitimately merged away when the store closes cleanly.
    for suffix in ["", "-wal", "-shm"] {
        let mut path = db.path().to_path_buf().into_os_string();
        path.push(suffix);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                assert!(
                    !suffix.is_empty(),
                    "the store file {path:?} must exist for the scan to mean anything: {error}"
                );
                continue;
            }
        };
        assert!(
            !bytes
                .windows(FAKE_KEY.len())
                .any(|window| window == FAKE_KEY.as_bytes()),
            "the key must not appear anywhere in the store file {path:?}"
        );
    }
}

/// AC4's wire half: a context note landing between two chat messages
/// renders a wire shape the live provider module builds and completes a
/// turn over — the note travels as its own system-voiced message after the
/// first exchange, and the system prompt stays untouched ahead of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_note_between_two_chat_messages_renders_a_wire_shape_the_module_accepts() {
    let (base, hits) = start_completions_server().await;
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let key = channel("group-noted-wire");

    let assistant = start_over_the_module(&store, &base).await;
    let mut replies = support::outbound_of(&assistant, &store).await;
    support::authorize(&assistant, &key).await;

    // First exchange, then the note, then the second ask.
    support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Group, "42", "the first ask"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(SERVER_ANSWER)
    );
    assistant
        .observe(assistant_core::Observation {
            channel: key.clone(),
            channel_kind: ChannelKind::Group,
            fact: assistant_core::ObservedFact::PinnedAnnouncement("Rules:\nBe kind.".into()),
        })
        .await
        .expect("the rules pin is judged");
    support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Group, "42", "the second ask"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.kind, ReplyKind::Answer);
    assert_eq!(
        reply.text, SERVER_ANSWER,
        "the module completed the turn over the noted ledger — bare, the \
         person's introduction rode the first answer"
    );

    // The second turn's wire body: the note is its own system message,
    // after the first exchange and before the second ask, with the system
    // prompt still leading.
    let turn = recorded_turn(&hits, "the second ask");
    assert_eq!(turn[0].0, "system", "the system prompt leads: {turn:?}");
    assert!(turn[0].1.contains(support::SYSTEM_PROMPT));
    let position = |role: &str, needle: &str| {
        turn.iter()
            .position(|(r, content)| r == role && content.contains(needle))
            .unwrap_or_else(|| {
                panic!("no {role} message carrying {needle:?} on the wire: {turn:?}")
            })
    };
    let note_index = position("system", "The group's rules are now:\nBe kind.");
    assert!(
        position("user", "the first ask") < note_index
            && note_index < position("user", "the second ask"),
        "the note sits between the two chat messages: {turn:?}"
    );
}

/// The heads-up line the wire script narrates ahead of its call. A fixture
/// string, not product copy: the live model words its own line from the
/// teaching this unit added.
const WIRE_ANNOUNCE: &str = "Let me check what I am running on.";

/// The two-round script (unit 40): a round that narrates and then calls,
/// and the round that answers once the call is resolved. The rounds are
/// told apart by the request itself — a request already carrying the
/// wire's tool-role message is the closing one — so a redispatched turn
/// draws the round its ledger is at, never the next one in a counter.
///
/// The calling round is written as the wire really carries it: text
/// deltas, then tool-call fragments whose arguments arrive split across
/// chunks and are folded by index, then the `tool_calls` finish. The
/// decoder releases the end of the turn BEFORE the drained call
/// lifecycle, which is what finalizes the narration as its own committed
/// answer ahead of the call block.
fn announced_call_then_answer(request: &Value) -> String {
    // The opening round owes one send — the announce — so a second send's
    // result is what says the closing round has already spoken.
    if send_results(request) > 1 {
        return stream_of(&[text_delta(SERVER_ANSWER), finish("stop")]);
    }
    if carries_tool_result(request) {
        return stream_of(&[send_call(2, SERVER_ANSWER), finish("tool_calls")]);
    }
    stream_of(&[
        send_call(0, WIRE_ANNOUNCE),
        json!({ "choices": [{ "delta": { "tool_calls": [{
            "index": 1,
            "id": "wire-call-1",
            "type": "function",
            "function": { "name": runtime::NAME, "arguments": "{" }
        }] } }] }),
        json!({ "choices": [{ "delta": { "tool_calls": [{
            "index": 1,
            "function": { "arguments": "}" }
        }] } }] }),
        finish("tool_calls"),
    ])
}

/// Whether a recorded request already carries an answered call of a tool
/// that is NOT a sending tool: the cue the scripted lookup has come back.
/// The send's own results are excluded, or the closing round would read its
/// own transport as its answer.
fn carries_tool_result(request: &Value) -> bool {
    tool_call_ids(request).any(|id| !id.starts_with(SEND_CALL_ID))
}

/// AC4 (unit 40): the announce composes over the PRODUCTION wire, not only
/// over the event-native fixtures. The framework's real chat-completions
/// module reads a two-round script off the loopback server — narration
/// deltas, tool-call fragments, a `tool_calls` finish, then the closing
/// text once the call is answered — and the consumer-visible ledger shows
/// the composition the operator's example describes: the heads-up line, the
/// call, its result, the answer.
///
/// The call names the runtime-facts tool on purpose: it is the tool that
/// reaches no network, so this wire test keeps exactly one server in it.
/// The tool is registered by the assembly on nothing, so no configuration
/// beyond the assembly's own admits it.
///
/// The module is behind the `chat_completions` feature; the workspace-wide
/// suite compiles it, and a per-crate iteration needs
/// `--features chat_completions` or this pin is silently absent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_announced_tool_round_composes_over_the_production_wire() {
    let (base, hits) = start_scripted_server(announced_call_then_answer).await;
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let key = channel("dm-wire-announce");

    let assistant = start_over_the_module(&store, &base).await;
    let mut replies = support::outbound_of(&assistant, &store).await;
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Direct, "42", "which model are you?"),
    )
    .await;

    // The chat receives the announce first, introduced as the person's
    // first delivery, then the answer bare.
    let introduced = support::disclosed(WIRE_ANNOUNCE);
    let announced = recv_reply(&mut replies).await;
    assert_eq!(announced.kind, ReplyKind::Answer);
    assert_eq!(
        announced.text, introduced,
        "the narration of the calling round is delivered on its own"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        SERVER_ANSWER,
        "the closing round's answer follows it"
    );

    // The ledger's order, in the consumer's view — which shows the lookup
    // and the turn's own notes, and never a send's own call, block and
    // resolution.
    let blocks = settle_shape(
        &store,
        receipt.conversation_id,
        "the announced wire turn",
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
    assert_eq!(field(&blocks[3], "name"), runtime::NAME);
    assert!(
        field(&blocks[4], "content").starts_with("model: "),
        "the tool really ran: {:?}",
        field(&blocks[4], "content")
    );
    assert_eq!(field(&blocks[5], "content"), SERVER_ANSWER);
    assert_eq!(
        support::sent_texts(&store, receipt.conversation_id).await,
        vec![introduced, SERVER_ANSWER.to_owned()],
        "the chat received the announce and then the answer, in that order"
    );

    // Three rounds, three requests: the announcing call, the round that
    // sends the answer once the lookup came back, and the round that writes
    // the notes down and ends. The closing one carries the answered call
    // back to the model under the wire's tool role.
    let recorded = hits.lock().expect("the hit log locks").clone();
    assert_eq!(recorded.len(), 3, "one request per round: {recorded:?}");
    assert!(
        !carries_tool_result(&recorded[0].body),
        "the calling round was asked with no result yet: {:?}",
        recorded[0].body
    );
    assert!(
        carries_tool_result(&recorded[1].body),
        "the sending round was asked with the answered call: {:?}",
        recorded[1].body
    );
}

/// The recorded wire request whose messages carry the needle, as
/// `(role, content)` pairs in wire order — the newest matching hit, so the
/// pin reads the turn it names, not an earlier projection.
fn recorded_turn(hits: &Arc<Mutex<Vec<Hit>>>, needle: &str) -> Vec<(String, String)> {
    let recorded = hits.lock().expect("the hit log locks").clone();
    recorded
        .iter()
        .rev()
        .find_map(|hit| {
            let messages = hit.body.get("messages")?.as_array()?;
            let pairs: Vec<(String, String)> = messages
                .iter()
                .map(|message| {
                    (
                        message["role"].as_str().unwrap_or_default().to_owned(),
                        message["content"].as_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect();
            pairs
                .iter()
                .any(|(_, content)| content.contains(needle))
                .then_some(pairs)
        })
        .expect("the named turn's request was recorded")
}
