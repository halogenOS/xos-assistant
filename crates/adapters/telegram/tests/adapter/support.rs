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

use agent_ledger::agency::{DateMarker, LeafKind};
use agent_ledger::providers::{
    BoxFuture, ContentPart as WirePart, Message, MessageContent, ModelInfo, ProviderRx, ProviderTx,
};
use agent_ledger::{
    Block, EventBus, LlmError, ProviderModule, ProviderRegistry, ProviderRequest, ProviderResponse,
    StopReason, Store, StoreError, StreamEvent,
};
use assistant_adapter_telegram::{ADAPTER_NAME, Config, Sleep, TelegramAdapter};
use assistant_core::schema::store_config;
use assistant_core::{
    Assistant, ChannelKey, ChannelKind, ModelBinding, Observation, ObserveOutcome, ObservedFact,
    OperatorConfig, SenderIdentity,
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::server::BotApiServer;

/// The provider module's type id — the binding's vendor.
pub const VENDOR: &str = "scripted";

/// The marker that tells a title derivation from a turn. Title derivation
/// is off in the assembly (decision 0077), so a request carrying this mark
/// is a regression: the scripted provider counts it instead of answering
/// it. The text mirrors the framework's title instruction wording.
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

/// The operator every test assembly is configured with — the platform user
/// id whose adds admit groups.
pub const OPERATOR_ID: i64 = 777_000;

/// The answer the script streams for a turn whose newest projected message
/// carries this text — derived from the request, so two chats' answers
/// differ and a reply sent to the wrong chat cannot pass.
#[must_use]
pub fn answer_to(text: &str) -> String {
    format!("The scripted answer to: {text}")
}

/// The assistant name every adapter-suite assembly resolves — matching the
/// display name the scripted `getMe` answers, so the wire identity and the
/// core's name agree.
pub const NAME: &str = "Fixture";

/// An answer as its first delivery to someone carries it: the fixture's
/// composed disclosure line, a blank line, then the answer.
#[must_use]
pub fn disclosed(answer: &str) -> String {
    assistant_core::Disclosure::resolve(None, NAME).disclosed(answer)
}

/// The first answer a person is sent, as the core stores and delivers it:
/// the disclosure line, a blank line, then the scripted answer.
#[must_use]
pub fn first_answer_to(text: &str) -> String {
    disclosed(&answer_to(text))
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

/// The opening line a held FAILING turn streams before it awaits its
/// release: real text, so the core's typing cue — keyed on the first
/// non-empty text delta since unit 22 — is provably up while the stream
/// is held, and the scripted error then ends the turn before anything
/// finalizes.
pub const HELD_TURN_OPENING: &str = "Let me look into that.";

/// Stream one opened text block's prose — the start event and its delta,
/// the shape every scripted turn opens its text with.
fn stream_text(response_tx: &mpsc::UnboundedSender<ProviderResponse>, text: String) {
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta { text }));
}

/// The scripted provider: answers every turn with [`answer_to`] the newest
/// projected message's text, deterministically. A scripted failure count
/// makes the next turns fail with a stream error instead; a tool script
/// replaces the prose turns with the call-then-close shape. A title
/// derivation request is counted, never answered — titles are off
/// (decision 0077).
struct ScriptedChat {
    failures: Arc<AtomicUsize>,
    /// Every turn request's projected messages, for the projection pins.
    seen: Arc<Mutex<Vec<Vec<Message>>>>,
    tool_script: Option<ToolScript>,
    /// When set, every chat turn streams its opening text and then awaits
    /// one release before it ends — the composing pins' fixture: the
    /// core's typing cue begins at the first real text delta, so a held
    /// turn shows the cue while its answer provably has not reached the
    /// wire.
    turn_hold: Arc<Mutex<Option<Arc<tokio::sync::Notify>>>>,
    /// The error text a scripted failure streams; see
    /// [`Fixture::word_failures_as`].
    failure_text: Arc<Mutex<String>>,
    /// Every title derivation request that reached the provider — asserted
    /// zero, since the assembly switched titles off.
    title_requests: Arc<AtomicUsize>,
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
        let seen = Arc::clone(&self.seen);
        let tool_script = self.tool_script.clone();
        let turn_hold = Arc::clone(&self.turn_hold);
        let failure_text = Arc::clone(&self.failure_text);
        let title_requests = Arc::clone(&self.title_requests);
        tokio::spawn(async move {
            let mut calls = 0_usize;
            while let Some(request) = requests.recv().await {
                let ProviderRequest::Stream { messages, .. } = request else {
                    continue;
                };
                // Titles are off (decision 0077): count the regression,
                // answer nothing.
                if messages.iter().any(|m| carries(m, TITLE_INSTRUCTION_MARK)) {
                    title_requests.fetch_add(1, Ordering::SeqCst);
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                seen.lock()
                    .expect("the request log locks")
                    .push(messages.clone());
                let hold = turn_hold.lock().expect("the turn hold locks").clone();
                if failures
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_ok()
                {
                    // A held failing turn streams an opening line first:
                    // real text raises the core's typing cue, so the
                    // refresh loop under test provably runs while the
                    // stream is held open — and the error then kills the
                    // turn before anything finalizes, so no send follows.
                    if let Some(hold) = &hold {
                        stream_text(&response_tx, HELD_TURN_OPENING.into());
                        hold.notified().await;
                    }
                    let error = failure_text.lock().expect("the failure text locks").clone();
                    let _ = response_tx.send(ProviderResponse::Error(error));
                    continue;
                }
                if let Some(hold) = &hold
                    && tool_script.is_some()
                {
                    hold.notified().await;
                }
                if let Some(script) = &tool_script {
                    // Scripted by ledger content: a request already carrying
                    // an answered call closes with the fixed prose, the
                    // opening turn narrates (when scripted) and calls.
                    if messages.iter().any(carries_tool_result) {
                        stream_text(&response_tx, TOOL_CLOSING_ANSWER.into());
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
                        stream_text(&response_tx, narration.clone());
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
                let answer = answer_to(&without_origin_marks(
                    &messages.last().map(message_text).unwrap_or_default(),
                ));
                stream_text(&response_tx, answer);
                // The hold sits between the streamed text and the message
                // end: the answer's text is on the stream — so the core's
                // typing cue is up — while the turn provably has not
                // completed and nothing can be delivered.
                if let Some(hold) = &hold {
                    hold.notified().await;
                }
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

/// The projected text with every message's bracketed id mark removed —
/// what the scripted answer derives from. The core's projection opens each
/// recorded message with its origin in brackets (unit 15), the way a model
/// reads past an id to the words; stripping here keeps the suite's answer
/// pins about the words.
fn without_origin_marks(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("] ") {
            Some(end) if line.starts_with('[') => &line[end + 2..],
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    /// Every turn request's projected messages, as the scripted provider
    /// received them.
    pub seen: Arc<Mutex<Vec<Vec<Message>>>>,
    /// The scripted provider's turn hold, unset by default; see
    /// [`Fixture::hold_turns`].
    turn_hold: Arc<Mutex<Option<Arc<tokio::sync::Notify>>>>,
    /// The error text a scripted failure streams; see
    /// [`Fixture::word_failures_as`].
    failure_text: Arc<Mutex<String>>,
    /// Every title derivation request that reached the provider — asserted
    /// zero by the end-to-end pin, since the assembly switched titles off
    /// (decision 0077).
    pub title_requests: Arc<AtomicUsize>,
}

impl Fixture {
    /// Hold every upcoming chat turn until the returned handle's
    /// `notify_one` releases it, one release per held turn. What the
    /// composing pins need: while a turn is held, its answer provably has
    /// not reached the wire, so anything recorded meanwhile came before
    /// the answer.
    pub fn hold_turns(&self) -> Arc<tokio::sync::Notify> {
        let hold = Arc::new(tokio::sync::Notify::new());
        *self.turn_hold.lock().expect("the turn hold locks") = Some(Arc::clone(&hold));
        hold
    }

    /// Reword the error every scripted failure streams. The default is an
    /// ordinary transient rendering, which the core answers with the
    /// failure notice; the quiet-failure pin words it as the payment
    /// rendering instead, which the core keeps out of the chat entirely.
    pub fn word_failures_as(&self, text: &str) {
        text.clone_into(&mut self.failure_text.lock().expect("the failure text locks"));
    }
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
        assistant_core::tools::ToolSet::production_lookups(
            assistant_core::tools::LookupEndpoints {
                forge: UNROUTABLE.into(),
                mirror: UNROUTABLE.into(),
                mirror_token: None,
                wiki: UNROUTABLE.into(),
                wiki_index: UNROUTABLE.into(),
            },
        ),
    )
    .await
}

/// Assemble a running assistant with the given tool script and tool set —
/// the seam the tool round-trip tests use.
pub async fn start_assistant_with_tools(
    tool_script: Option<ToolScript>,
    tools: assistant_core::tools::ToolSet,
) -> Fixture {
    assemble(
        tool_script,
        tools,
        None,
        assistant_core::DirectChats::default(),
        assistant_core::AnsweringMode::Addressed,
    )
    .await
}

/// The moderation handle the report round-trip configures — the report
/// line's `/report@` suffix under test.
pub const MODERATION_HANDLE: &str = "moderation_fixture_bot";

/// Assemble a running assistant that serves no direct chats, under the
/// suites' shared default tool set — the fixture behind the direct-chat
/// switch pins.
pub async fn start_assistant_direct_off() -> Fixture {
    assemble(
        None,
        assistant_core::tools::ToolSet::production_lookups(
            assistant_core::tools::LookupEndpoints {
                forge: UNROUTABLE.into(),
                mirror: UNROUTABLE.into(),
                mirror_token: None,
                wiki: UNROUTABLE.into(),
                wiki_index: UNROUTABLE.into(),
            },
        ),
        None,
        assistant_core::DirectChats::Off,
        assistant_core::AnsweringMode::Addressed,
    )
    .await
}

/// The moderating seam: the tool script, the tool set, and an optional
/// moderation handle — under HELPFUL answering, because the report tool's
/// registration takes a handle plus helpful mode since unit 15, and the
/// autonomous assessment only exists where every message reaches the
/// model.
pub async fn start_assistant_moderating(
    tool_script: Option<ToolScript>,
    tools: assistant_core::tools::ToolSet,
    moderation_handle: Option<String>,
) -> Fixture {
    assemble(
        tool_script,
        tools,
        moderation_handle,
        assistant_core::DirectChats::default(),
        assistant_core::AnsweringMode::Helpful,
    )
    .await
}

/// The one assembly every seam above funnels into.
async fn assemble(
    tool_script: Option<ToolScript>,
    tools: assistant_core::tools::ToolSet,
    moderation_handle: Option<String>,
    direct_chats: assistant_core::DirectChats,
    answering: assistant_core::AnsweringMode,
) -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let failures = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let turn_hold = Arc::new(Mutex::new(None));
    let failure_text = Arc::new(Mutex::new("scripted stream failure".to_owned()));
    let title_requests = Arc::new(AtomicUsize::new(0));
    let mut providers = ProviderRegistry::new();
    providers.register(Box::new(ScriptedChat {
        failures: Arc::clone(&failures),
        seen: Arc::clone(&seen),
        tool_script,
        turn_hold: Arc::clone(&turn_hold),
        failure_text: Arc::clone(&failure_text),
        title_requests: Arc::clone(&title_requests),
    }));
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        Arc::new(providers),
        tools,
        assistant_core::AssemblyConfig {
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: ModelBinding {
                provider_instance: "scripted-1".into(),
                provider_display_name: "Scripted".into(),
                vendor: VENDOR.into(),
                model: "script-model".into(),
                model_display_name: "Script Model".into(),
            },
            system_prompt: "You are the adapter suite's scripted assistant fixture.".into(),
            answering,
            name: NAME.into(),
            disclosure: None,
            protection: assistant_core::ProtectionConfig::default(),
            operators: operator_config(),
            direct_chats,
            privacy_policy_address: None,
            moderation_handle,
            web_search: None,
        },
    )
    .await
    .expect("the assembly starts");
    Fixture {
        assistant: Arc::new(assistant),
        store,
        failures,
        seen,
        turn_hold,
        failure_text,
        title_requests,
    }
}

/// The suite's operator wiring: [`OPERATOR_ID`] on this adapter.
pub fn operator_config() -> OperatorConfig {
    OperatorConfig {
        by_adapter: std::collections::HashMap::from([(
            ADAPTER_NAME.to_owned(),
            OPERATOR_ID.to_string(),
        )]),
    }
}

/// The core-side key of one chat on this adapter — what the direct
/// admission below and the ledger assertions name a chat by.
#[must_use]
pub fn chat_key(chat_id: i64) -> ChannelKey {
    ChannelKey {
        adapter: ADAPTER_NAME.into(),
        channel: chat_id.to_string(),
    }
}

/// Admit one group chat as the configured operator, through the core's own
/// observation edge — the standing premise of every test that speaks in a
/// group; the wire-driven admission pins push the membership update through
/// the adapter instead.
pub async fn authorize_group(assistant: &Assistant, chat_id: i64) {
    let outcome = assistant
        .observe(Observation {
            channel: chat_key(chat_id),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::Added {
                by: Some(SenderIdentity {
                    external_id: OPERATOR_ID.to_string(),
                    username: None,
                }),
            },
        })
        .await
        .expect("the membership observation is judged");
    assert_eq!(
        outcome,
        ObserveOutcome::Observed { deliver: None },
        "the operator's add admits the group"
    );
}

/// One membership update: the assistant moved between the given member
/// statuses in the given chat, by the acting user.
#[must_use]
pub fn membership_update(
    update_id: i64,
    chat_kind: &str,
    chat_id: i64,
    actor_id: i64,
    old_status: &str,
    new_status: &str,
) -> Value {
    json!({
        "update_id": update_id,
        "my_chat_member": {
            "chat": { "id": chat_id, "type": chat_kind },
            "from": { "id": actor_id, "first_name": format!("Person {actor_id}") },
            "date": date_of(update_id),
            "old_chat_member": { "user": { "id": BOT_ID }, "status": old_status },
            "new_chat_member": { "user": { "id": BOT_ID }, "status": new_status },
        },
    })
}

/// One pin service message: the chat's pinned announcement carrying the
/// given text, pinned by the given user.
#[must_use]
pub fn pin_update(update_id: i64, chat_id: i64, pinner_id: i64, pinned_text: &str) -> Value {
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": "group" },
            "from": { "id": pinner_id, "first_name": format!("Person {pinner_id}") },
            "pinned_message": {
                "message_id": message_id_of(update_id) - 1,
                "date": date_of(update_id) - 1,
                "chat": { "id": chat_id, "type": "group" },
                "text": pinned_text,
            },
        },
    })
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
/// the given sleep and the suite's [`NAME`] as the wake name — matching the
/// core fixture's resolved name, as the embedder wires it.
pub fn spawn_adapter(
    server: &BotApiServer,
    state_file: &Path,
    assistant: Arc<Assistant>,
    sleep: Sleep,
) -> AdapterGuard {
    spawn_adapter_named(server, state_file, assistant, sleep, Some(NAME))
}

/// The name-varying seam behind [`spawn_adapter`]: the wake-trigger pins
/// hand a punctuated or absent name through here.
pub fn spawn_adapter_named(
    server: &BotApiServer,
    state_file: &Path,
    assistant: Arc<Assistant>,
    sleep: Sleep,
    name: Option<&str>,
) -> AdapterGuard {
    let mut config = Config::new(TOKEN, state_file);
    config.api_root = server.root();
    config.name = name.map(ToOwned::to_owned);
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

/// The ledger as the consumer's own content: every block the assistant or a
/// member put there, and none of the framework's date records.
///
/// The framework writes a `date_marker` on the first user-voiced append of
/// a day — its own calendar entry, ordered before the block that tripped
/// it, carrying no consumer content. A suite that spells out what the
/// adapter's traffic recorded is asserting about consumer content, so it
/// judges this view in one place instead of each test carrying its own
/// arithmetic about the framework's records. The kind is named through the
/// framework leaf's own `KINDS`, never a literal here.
#[must_use]
pub fn consumer_view(blocks: &[Block]) -> Vec<Block> {
    blocks
        .iter()
        .filter(|block| !DateMarker::KINDS.contains(&block.block_type.as_str()))
        .cloned()
        .collect()
}

/// Await one conversation's settled turn by its exact block-type shape —
/// every stored type in the consumer view ([`consumer_view`]) matches
/// `shape` in order — and return that view.
pub async fn await_shape(store: &Store, conversation: i64, shape: &[&str]) -> Vec<Block> {
    let expected: Vec<String> = shape.iter().map(|s| (*s).to_owned()).collect();
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let blocks = consumer_view(
            &store
                .list_blocks(conversation)
                .await
                .expect("the ledger reads"),
        );
        let types: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
        if types == expected.iter().map(String::as_str).collect::<Vec<_>>() {
            return blocks;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the ledger shape {expected:?}; have {types:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
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
