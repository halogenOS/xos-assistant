//! The real `OpenRouter` module against a scripted loopback server: the
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
use assistant_core::{Assistant, ChannelKind, ModelBinding, ReplyKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::support::{self, TempDb, channel, inbound, recv_reply};

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

/// A chat-completions-shaped loopback server: answers every POST with one
/// server-sent text delta, a finish chunk and the end marker, recording
/// each hit.
async fn start_completions_server() -> (String, Arc<Mutex<Vec<Hit>>>) {
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
                let body = serde_json::from_slice(&request[body_start..])
                    .unwrap_or(serde_json::Value::Null);
                recorded.lock().expect("the hit log locks").push(Hit {
                    path,
                    authorization,
                    body,
                });
                let body = format!(
                    "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{SERVER_ANSWER}\"}}}}]}}\n\n\
                     data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\n\
                     data: [DONE]\n\n"
                );
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

/// The whole loop over the real module: ingest, answer, and the two key
/// residence facts — presented on the wire, absent from the store file.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_openrouter_module_answers_over_the_loopback_wire_and_stores_no_key() {
    let (base, hits) = start_completions_server().await;
    let db = TempDb::new("openrouter");
    let key = channel("dm-openrouter");

    {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        let mut providers = ProviderRegistry::new();
        let provider =
            MemoryConfiguredProvider::new(&store, FAKE_KEY.into(), Some(base.clone())).await;
        let vendor = agent_ledger::ProviderModule::type_id(&provider).to_owned();
        providers.register(Box::new(provider));
        let assistant = Assistant::start(
            store.clone(),
            Arc::new(EventBus::new()),
            Arc::new(providers),
            assistant_core::tools::ToolSet::new(),
            assistant_core::AssemblyConfig {
                reasoning: assistant_core::ReasoningLevel::Low,
                binding: ModelBinding {
                    provider_instance: "openrouter-1".into(),
                    provider_display_name: "OpenRouter".into(),
                    vendor: vendor.clone(),
                    model: "test-vendor/test-model".into(),
                    model_display_name: "Test Model".into(),
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
            },
        )
        .await
        .expect("the assembly starts over the real module");
        let mut replies = assistant
            .replies(support::ADAPTER)
            .await
            .expect("the outbound edge opens");
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

    let mut providers = ProviderRegistry::new();
    let provider = MemoryConfiguredProvider::new(&store, FAKE_KEY.into(), Some(base.clone())).await;
    let vendor = agent_ledger::ProviderModule::type_id(&provider).to_owned();
    providers.register(Box::new(provider));
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        Arc::new(providers),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: ModelBinding {
                provider_instance: "openrouter-1".into(),
                provider_display_name: "OpenRouter".into(),
                vendor,
                model: "test-vendor/test-model".into(),
                model_display_name: "Test Model".into(),
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
        },
    )
    .await
    .expect("the assembly starts over the real module");
    let mut replies = assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
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
