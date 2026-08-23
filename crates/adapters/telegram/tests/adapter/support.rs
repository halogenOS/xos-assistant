//! Shared fixtures: the scripted provider, the assembled core, the update
//! builders, and the polling helpers.
//!
//! The scripted provider is this unit's own, written against the framework's
//! public provider traits per decision 0009 — the core's test support is a
//! pattern, not an importable artifact. Like the core's, it answers the
//! metadata worker's title-derivation request deterministically.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_ledger::providers::{
    BoxFuture, ContentPart as WirePart, Message, MessageContent, ModelInfo, ProviderRx, ProviderTx,
};
use agent_ledger::{
    Block, EventBus, LlmError, ProviderModule, ProviderRegistry, ProviderRequest, ProviderResponse,
    StopReason, Store, StoreError, StreamEvent,
};
use assistant_adapter_telegram::{Config, Sleep, TelegramAdapter};
use assistant_core::schema::store_config;
use assistant_core::{Assistant, ModelBinding};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::server::BotApiServer;

/// The scripted title every derivation streams.
pub const TITLE: &str = "A derived title";

/// The provider module's type id — the binding's vendor.
pub const VENDOR: &str = "scripted";

/// The marker that tells a title derivation from a turn. The text mirrors
/// the framework's title instruction; if that wording changes, this suite
/// fails loudly on an unanswered title instead of silently miscounting
/// turns.
pub const TITLE_INSTRUCTION_MARK: &str = "Generate a concise title";

/// How long a poll or an awaited condition may take before the test names a
/// stall.
pub const DEADLINE: Duration = Duration::from_secs(10);

/// The scripted server's bot identity: what `getMe` answers, and what the
/// addressed builders mention and reply to.
pub const BOT_ID: i64 = 999_000;
pub const BOT_USERNAME: &str = "assistant_fixture_bot";

/// A fake token for the scripted server. Nothing real: the tests only ever
/// talk to the loopback listener, and the token check (AC6) asserts this
/// exact string reaches no log line.
pub const TOKEN: &str = "0000000000:FAKE-TEST-TOKEN-FOR-THE-SCRIPTED-SERVER";

/// The answer the script streams for a turn whose newest projected message
/// carries this text — derived from the request, so two chats' answers
/// differ and a reply sent to the wrong chat cannot pass.
#[must_use]
pub fn answer_to(text: &str) -> String {
    format!("The scripted answer to: {text}")
}

/// A unique path in the temp directory, removed on drop with the offset
/// write's sidecar, so parallel tests never share a state file and no run
/// leaves litter.
pub struct TempStateFile(PathBuf);

impl TempStateFile {
    pub fn new(name: &str) -> Self {
        let unique = format!(
            "adapter-state-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the clock is past the epoch")
                .as_nanos()
        );
        Self(std::env::temp_dir().join(unique))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempStateFile {
    fn drop(&mut self) {
        for suffix in ["", ".next"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

/// One tool-call script for the tool round-trip tests: the opening turn
/// calls the tool with this input — after the optional narration — and the
/// turn whose request carries the answered call closes with
/// [`TOOL_CLOSING_ANSWER`]. Patterned on the framework's tool-call event
/// shapes and their production order — the message end first, then the
/// drained tool lifecycle — written here per decision 0009: this unit's own
/// scripted provider, not an import.
#[derive(Clone)]
pub struct ToolScript {
    /// The registered tool name the opening turn calls.
    pub tool: String,
    /// The call's input JSON, non-empty by the script's contract.
    pub input: String,
    /// Prose streamed before the call, for the narration variant.
    pub narration: Option<String>,
}

/// The closing prose a tool-scripted turn streams once its request carries
/// the answered call — the cue itself is the proof the result reached the
/// model's second request.
pub const TOOL_CLOSING_ANSWER: &str = "The scripted closing answer.";

/// The scripted provider: answers every turn with [`answer_to`] the newest
/// projected message's text and every title derivation with [`TITLE`],
/// deterministically. A scripted failure count makes the next turns fail
/// with a stream error instead; a tool script replaces the prose turns with
/// the call-then-close shape.
struct ScriptedChat {
    failures: Arc<AtomicUsize>,
    tool_script: Option<ToolScript>,
}

impl ProviderModule for ScriptedChat {
    fn type_id(&self) -> &'static str {
        VENDOR
    }
    fn display_name(&self) -> &'static str {
        "Scripted"
    }
    fn description(&self) -> &'static str {
        "answers from a script"
    }
    fn get_config(&self, _provider_id: String) -> BoxFuture<'_, Result<Option<Value>, StoreError>> {
        Box::pin(async { Ok(Some(json!({}))) })
    }
    fn save_config(
        &self,
        _provider_id: String,
        _config: Value,
    ) -> BoxFuture<'_, Result<(), StoreError>> {
        Box::pin(async { Ok(()) })
    }
    fn delete_config(&self, _provider_id: String) -> BoxFuture<'_, Result<(), StoreError>> {
        Box::pin(async { Ok(()) })
    }
    fn summary(&self, _provider_id: String) -> BoxFuture<'_, Result<Option<String>, StoreError>> {
        Box::pin(async { Ok(None) })
    }
    fn list_models(&self, _config: Value) -> BoxFuture<'_, Result<Vec<ModelInfo>, LlmError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn bind(
        &self,
        _conversation_id: i64,
        _provider_id: String,
        _config: Value,
    ) -> (ProviderTx, ProviderRx) {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        let failures = Arc::clone(&self.failures);
        let tool_script = self.tool_script.clone();
        tokio::spawn(async move {
            let mut calls = 0_usize;
            while let Some(request) = requests.recv().await {
                let ProviderRequest::Stream { messages, .. } = request else {
                    continue;
                };
                if messages.iter().any(|m| carries(m, TITLE_INSTRUCTION_MARK)) {
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                        text: TITLE.into(),
                    }));
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                if failures
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_ok()
                {
                    let _ =
                        response_tx.send(ProviderResponse::Error("scripted stream failure".into()));
                    continue;
                }
                if let Some(script) = &tool_script {
                    // Scripted by ledger content: a request already carrying
                    // an answered call closes with the fixed prose, the
                    // opening turn narrates (when scripted) and calls.
                    if messages.iter().any(carries_tool_result) {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: TOOL_CLOSING_ANSWER.into(),
                        }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::EndTurn,
                            }));
                        // The trailing done real wires send after every
                        // completed turn: the framework settles the dispatch
                        // state on the closed signal, so a scripted turn
                        // ending without it would leave the state open and
                        // stall the suite.
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    if let Some(narration) = &script.narration {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: narration.clone(),
                        }));
                    }
                    calls += 1;
                    // The production order: every provider emits its tool
                    // lifecycle AFTER `MessageEnd`, which finalizes any
                    // narration first — so the ledger holds the narration
                    // text before the tool-call block.
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                        usage: agent_ledger::providers::Usage::default(),
                        stop_reason: StopReason::ToolUse,
                    }));
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                        id: format!("call-{calls}"),
                        name: script.tool.clone(),
                    }));
                    let _ =
                        response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseInputDelta {
                            json: script.input.clone(),
                        }));
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                let answer = answer_to(&messages.last().map(message_text).unwrap_or_default());
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                    text: answer,
                }));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                    usage: agent_ledger::providers::Usage::default(),
                    stop_reason: StopReason::EndTurn,
                }));
                let _ = response_tx.send(ProviderResponse::Done);
            }
        });
        (request_tx, responses)
    }
}

/// Whether one projected message carries a tool-result part — the scripted
/// ledger-content cue for the closing prose.
fn carries_tool_result(message: &Message) -> bool {
    matches!(&message.content, MessageContent::Parts(parts)
        if parts.iter().any(|part| matches!(part, WirePart::ToolResult { .. })))
}

/// Whether one projected message carries this text, in either content mode.
fn carries(message: &Message, needle: &str) -> bool {
    match &message.content {
        MessageContent::Text(text) => text.contains(needle),
        MessageContent::Parts(parts) => parts
            .iter()
            .any(|part| matches!(part, WirePart::Text { text } if text.contains(needle))),
    }
}

/// One projected message's whole text, in either content mode.
fn message_text(message: &Message) -> String {
    match &message.content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                WirePart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// One test's assembled core: the running assistant, shared for the adapter,
/// the store handle the test reads the ledger through, and the provider's
/// failure script.
pub struct Fixture {
    pub assistant: Arc<Assistant>,
    pub store: Store,
    /// How many upcoming turns fail with a scripted stream error.
    pub failures: Arc<AtomicUsize>,
}

/// A loopback address nothing listens on: a tool constructed over it can be
/// registered — and its palette entry recorded — without any test traffic
/// ever succeeding against it.
pub const UNROUTABLE: &str = "http://127.0.0.1:1";

/// Assemble a running assistant over a fresh in-memory store with the
/// scripted provider registered, under the suites' one shared default tool
/// set: the core's production lookups, pointed at the unroutable loopback
/// address — the same answer the core suite's default fixture gives.
pub async fn start_assistant() -> Fixture {
    start_assistant_with_tools(
        None,
        assistant_core::tools::ToolSet::production_lookups(UNROUTABLE, UNROUTABLE, None),
    )
    .await
}

/// Assemble a running assistant with the given tool script and tool set —
/// the seam the tool round-trip tests use.
pub async fn start_assistant_with_tools(
    tool_script: Option<ToolScript>,
    tools: assistant_core::tools::ToolSet,
) -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let failures = Arc::new(AtomicUsize::new(0));
    let mut providers = ProviderRegistry::new();
    providers.register(Box::new(ScriptedChat {
        failures: Arc::clone(&failures),
        tool_script,
    }));
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        Arc::new(providers),
        tools,
        ModelBinding {
            provider_instance: "scripted-1".into(),
            provider_display_name: "Scripted".into(),
            vendor: VENDOR.into(),
            model: "script-model".into(),
            model_display_name: "Script Model".into(),
        },
        "You are the adapter suite's scripted assistant fixture.".into(),
        assistant_core::ProtectionConfig::default(),
    )
    .await
    .expect("the assembly starts");
    Fixture {
        assistant: Arc::new(assistant),
        store,
        failures,
    }
}

/// The running adapter, aborted when the guard drops so no test leaks a
/// polling loop into the next.
pub struct AdapterGuard(tokio::task::JoinHandle<()>);

impl Drop for AdapterGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Start the adapter against the scripted server, with every wait handed to
/// the given sleep.
pub fn spawn_adapter(
    server: &BotApiServer,
    state_file: &Path,
    assistant: Arc<Assistant>,
    sleep: Sleep,
) -> AdapterGuard {
    let mut config = Config::new(TOKEN, state_file);
    config.api_root = server.root();
    let adapter = TelegramAdapter::with_sleep(config, sleep);
    AdapterGuard(tokio::spawn(async move {
        adapter
            .run(assistant)
            .await
            .expect("the adapter takes its edge and runs");
    }))
}

/// A sleep that never waits: it records every requested duration and yields
/// once, so waits are pinned by assertion instead of by the clock.
pub fn recording_sleep() -> (Sleep, Arc<Mutex<Vec<Duration>>>) {
    let waits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&waits);
    let sleep: Sleep = Arc::new(move |wait| {
        recorded.lock().expect("the wait log locks").push(wait);
        Box::pin(tokio::task::yield_now())
    });
    (sleep, waits)
}

/// The message id the builders derive from an update id, so origin
/// assertions can name it.
#[must_use]
pub fn message_id_of(update_id: i64) -> i64 {
    update_id + 1000
}

/// The send date the builders stamp on an update, unix seconds — derived
/// from the update id so a timestamp assertion can name the exact instant.
#[must_use]
pub fn date_of(update_id: i64) -> i64 {
    1_700_000_000 + update_id
}

/// One update carrying a chat message of the given chat type.
#[must_use]
pub fn message_update(
    update_id: i64,
    chat_kind: &str,
    chat_id: i64,
    user_id: i64,
    text: &str,
) -> Value {
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": chat_kind },
            "from": { "id": user_id, "first_name": format!("Person {user_id}") },
            "text": text,
        },
    })
}

/// A private-chat message; the chat id is the person's own id, as on the
/// platform.
#[must_use]
pub fn private_update(update_id: i64, user_id: i64, text: &str) -> Value {
    message_update(update_id, "private", user_id, user_id, text)
}

/// A group message. Unaddressed unless the text itself mentions the bot:
/// recorded, resting, not answered.
#[must_use]
pub fn group_update(update_id: i64, chat_id: i64, user_id: i64, text: &str) -> Value {
    message_update(update_id, "group", chat_id, user_id, text)
}

/// A group message mentioning the bot — addressed through the mention rule.
#[must_use]
pub fn mention_update(update_id: i64, chat_id: i64, user_id: i64, text: &str) -> Value {
    message_update(
        update_id,
        "group",
        chat_id,
        user_id,
        &format!("@{BOT_USERNAME} {text}"),
    )
}

/// A group message replying to one of the bot's own messages — addressed
/// through the reply rule.
#[must_use]
pub fn reply_to_bot_update(update_id: i64, chat_id: i64, user_id: i64, text: &str) -> Value {
    let mut update = message_update(update_id, "group", chat_id, user_id, text);
    update["message"]["reply_to_message"] = json!({
        "message_id": message_id_of(update_id) - 1,
        "date": date_of(update_id) - 1,
        "chat": { "id": chat_id, "type": "group" },
        "from": { "id": BOT_ID, "is_bot": true, "first_name": "Fixture", "username": BOT_USERNAME },
        "text": "an earlier answer",
    });
    update
}

/// Await the recorded chat-message blocks of one conversation reaching the
/// given count, and return exactly those blocks in ledger order.
pub async fn await_chat_messages(store: &Store, conversation_id: i64, count: usize) -> Vec<Block> {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let messages: Vec<Block> = store
            .list_blocks(conversation_id)
            .await
            .expect("the ledger reads")
            .into_iter()
            .filter(|block| block.block_type == "chat_message")
            .collect();
        if messages.len() >= count {
            assert_eq!(
                messages.len(),
                count,
                "more recorded messages than the test expects"
            );
            return messages;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {count} recorded messages; have {}",
            messages.len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Await the store's conversation list reaching the given count, and return
/// the conversation ids in creation order.
pub async fn await_conversations(store: &Store, count: usize) -> Vec<i64> {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let mut ids: Vec<i64> = store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .iter()
            .map(|conversation| conversation.id)
            .collect();
        ids.sort_unstable();
        if ids.len() >= count {
            return ids;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {count} conversations; have {}",
            ids.len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Await the state file holding exactly this next offset.
pub async fn await_state_file(path: &Path, next_offset: i64) {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        if content.trim() == next_offset.to_string() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the state file to hold {next_offset}; holds {content:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
