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
    Block, CoreEvent, EventBus, LlmError, ProviderModule, ProviderRegistry, ProviderRequest,
    ProviderResponse, StopReason, Store, StoreError, StreamEvent,
};
use assistant_adapter_telegram::{ADAPTER_NAME, AdapterError, Config, Sleep, TelegramAdapter};
use assistant_core::delivery::Delivered;
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
        for suffix in ["", ".next", ".secret", ".secret.next"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            // A directory is one of the shapes a test puts in the way of a
            // write, so both removals are tried and neither is required.
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_dir(&path);
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
    /// Prose streamed before the call: the model's own notes, which reach
    /// nobody from unit 55 on.
    pub narration: Option<String>,
    /// A line the opening round SENDS beside its call, where the script has
    /// one — the heads-up before slow work, which from unit 55 is a message
    /// like any other and not prose a relay carried.
    pub announce: Option<String>,
}

/// The closing prose a tool-scripted turn streams once its request carries
/// the answered call — the cue itself is the proof the result reached the
/// model's second request.
pub const TOOL_CLOSING_ANSWER: &str = "The scripted closing answer.";

/// The words a held FAILING turn opens its send with before it awaits its
/// release. Only the call's START reaches the wire: the core's typing cue
/// — keyed on a sending tool's recorded call start since unit 55 — is
/// provably up while the stream is held, and the scripted error then kills
/// the turn before the call ever completes, so nothing is ever sent.
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
    /// When set, every chat turn records its sending tool's call and then
    /// awaits one release before it ends — the composing pins' fixture:
    /// the core's typing cue begins at that call's start, so a held turn
    /// shows the cue while its message provably has not reached the wire.
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

    #[allow(clippy::too_many_lines)]
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
                // The rules acknowledgment is a one-shot generation and not
                // a turn: its whole product is the text the item carries, so
                // it is answered with text and never with a send (unit 55).
                if messages.iter().any(|m| carries(m, ACKNOWLEDGMENT_MARK)) {
                    let rules =
                        without_envelope(&messages.last().map(message_text).unwrap_or_default());
                    stream_text(&response_tx, answer_to(&rules));
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                        usage: agent_ledger::providers::Usage::default(),
                        stop_reason: StopReason::EndTurn,
                    }));
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
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
                    // A held failing turn OPENS a send first: a sending
                    // tool's recorded call start raises the core's typing
                    // cue, so the refresh loop under test provably runs
                    // while the stream is held open. Only the start
                    // reaches the wire — the error then kills the turn
                    // before the call can complete, so the tool never runs
                    // and nothing is ever sent.
                    if let Some(hold) = &hold {
                        calls += 1;
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::ToolUse,
                            }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                                id: format!("{SEND_CALL_ID}{calls}"),
                                name: assistant_core::tools::send::NAME.to_owned(),
                            }));
                        let _ = response_tx.send(ProviderResponse::Event(
                            StreamEvent::ToolUseInputDelta {
                                json: serde_json::json!({ "text": HELD_TURN_OPENING }).to_string(),
                            },
                        ));
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
                    // How many sends this script owes before its closing
                    // one: the announce, where the script has a line to
                    // announce with. Counting them is what tells the
                    // closing round from the round after it, on a wire
                    // where both carry a send's result.
                    let announces = usize::from(script.announce.is_some());
                    // Scripted by ledger content: the round after the
                    // closing send writes the notes down and ends, a
                    // request carrying the answered call sends the closing
                    // prose, and the opening turn narrates (when scripted),
                    // announces (when scripted) and calls.
                    if send_results_this_turn(&messages) > announces {
                        stream_text(&response_tx, TOOL_CLOSING_ANSWER.into());
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::EndTurn,
                            }));
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    if a_call_was_answered(&messages) {
                        calls += 1;
                        send_scripted_message(&response_tx, calls, TOOL_CLOSING_ANSWER, None);
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
                    // The announce, where the script has one: a SEND beside
                    // the call, in the same round (unit 55). It used to be
                    // prose ahead of the call, which the relay carried; the
                    // relay is gone, so a line the chat must read before the
                    // slow work is a message like any other.
                    if let Some(announce) = &script.announce {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                                id: format!("{SEND_CALL_ID}{calls}"),
                                name: assistant_core::tools::send::NAME.to_owned(),
                            }));
                        let _ = response_tx.send(ProviderResponse::Event(
                            StreamEvent::ToolUseInputDelta {
                                json: serde_json::json!({ "text": announce }).to_string(),
                            },
                        ));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                    }
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
                // The closing round: this request carries the send's own
                // result, so the message is already in the chat and the
                // turn writes down what it said and ends.
                let ask = newest_ask(&messages);
                let answer = answer_to(&without_envelope(&ask));
                if send_results_this_turn(&messages) > 0 {
                    stream_text(&response_tx, answer);
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                        usage: agent_ledger::providers::Usage::default(),
                        stop_reason: StopReason::EndTurn,
                    }));
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                calls += 1;
                // The threaded script: an ask carrying the cue is answered
                // with the reply tool aimed at that ask's own msgid — the
                // way the model chooses a target from unit 55 on, instead
                // of the edge deriving one.
                let aimed = ask
                    .contains(REPLY_CUE)
                    .then(|| newest_msgid(&ask))
                    .flatten();
                send_scripted_message(&response_tx, calls, &answer, aimed.as_deref());
                // The hold sits between the send's own call events and the
                // stream's close: the call is recorded — so the core's
                // typing cue is up — while the turn provably has not
                // completed and nothing can be delivered.
                if let Some(hold) = &hold {
                    hold.notified().await;
                }
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

/// A distinctive clause of the core's rules-acknowledgment instruction —
/// the one-shot generation the observation surface runs when a real rules
/// delta lands.
const ACKNOWLEDGMENT_MARK: &str = "the group's newly pinned rules";

/// The provider id every scripted SEND is made under — what tells a send's
/// own result apart from any other tool's, inside the fixture.
const SEND_CALL_ID: &str = "send-";

/// The call ids one projected message answers.
fn tool_result_ids(message: &Message) -> impl Iterator<Item = &str> {
    let parts = match &message.content {
        MessageContent::Parts(parts) => parts.as_slice(),
        MessageContent::Text(_) => &[],
    };
    parts.iter().filter_map(|part| match part {
        WirePart::ToolResult { tool_use_id, .. } => Some(tool_use_id.as_str()),
        _ => None,
    })
}

/// How many SENDS this turn has already had answered — the cue that a
/// scripted turn is done, since a send is a PENDING call and the round
/// after it is the turn's last (unit 55, 2026-09-02).
fn send_results_this_turn(messages: &[Message]) -> usize {
    let since = messages
        .iter()
        .rposition(|message| {
            message.role == agent_ledger::providers::MessageRole::User
                && !carries_tool_result(message)
        })
        .map_or(0, |at| at + 1);
    messages[since..]
        .iter()
        .map(|message| {
            tool_result_ids(message)
                .filter(|id| id.starts_with(SEND_CALL_ID))
                .count()
        })
        .sum()
}

/// Emit one scripted SEND: the production event order — the message end
/// carrying the tool-use stop reason, then the call's lifecycle — for a
/// call of the plain sending tool carrying the given text, or of the reply
/// tool where a target is named.
///
/// Words reach a chat the one way anything does from unit 55 on: through a
/// sending tool. A provider that streamed its answer as text would be a
/// turn of private notes, and nothing would ever be sent.
fn send_scripted_message(
    response_tx: &tokio::sync::mpsc::UnboundedSender<ProviderResponse>,
    turn: usize,
    text: &str,
    reply_to: Option<&str>,
) {
    let (tool, input) = match reply_to {
        Some(target) => (
            assistant_core::tools::reply::NAME,
            serde_json::json!({ "text": text, "reply_to": target }),
        ),
        None => (
            assistant_core::tools::send::NAME,
            serde_json::json!({ "text": text }),
        ),
    };
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
        usage: agent_ledger::providers::Usage::default(),
        stop_reason: StopReason::ToolUse,
    }));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
        id: format!("{SEND_CALL_ID}{turn}"),
        name: tool.to_owned(),
    }));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseInputDelta {
        json: input.to_string(),
    }));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
}

/// The cue that scripts a THREADED send: a turn whose newest ask carries
/// this text is answered with the reply tool, aimed at that ask's own
/// msgid.
pub const REPLY_CUE: &str = "(the threaded cue)";

/// The msgid one projected ask declares, read off its own envelope.
///
/// The LAST one in the text, because a projected user turn joins several
/// contributions under one message and the newest is the one at the end —
/// which is the one a model aiming at "what was just said" would name.
fn newest_msgid(ask: &str) -> Option<String> {
    ask.lines()
        .filter_map(|line| line.strip_prefix(assistant_core::kind::ENVELOPE_MSGID))
        .next_back()
        .map(ToOwned::to_owned)
}

/// Whether one projected request carries the answer to a call of a tool
/// that is NOT a sending tool, anywhere in its history — the cue a
/// tool-scripted turn closes on.
///
/// It reads the whole history and not the newest message, because that is
/// what the scripts mean by "my call has been answered": the script plays
/// one call per conversation, and every later turn is the closing kind. The
/// send's own results are excluded, or the closing round would read its own
/// transport as its answer.
fn a_call_was_answered(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|message| tool_result_ids(message).any(|id| !id.starts_with(SEND_CALL_ID)))
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

/// The newest thing a PERSON said in one projected request — what every
/// scripted answer is derived from. A tool result is user-voiced on this
/// wire and carries no words of anybody's, so it is skipped.
fn newest_ask(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| {
            message.role == agent_ledger::providers::MessageRole::User
                && !carries_tool_result(message)
        })
        .map(message_text)
        .unwrap_or_default()
}

/// The projected text with the envelope stripped — what the scripted answer
/// derives from. Every recorded message projects under a fenced envelope
/// naming its author, its send time and its msgid (unit 55), the way a
/// model reads past the header to the words; stripping here keeps the
/// suite's answer pins about the words.
fn without_envelope(text: &str) -> String {
    let mut body = String::new();
    let mut rest = text;
    loop {
        let opens_at = if rest.starts_with("---\n") {
            Some(0)
        } else {
            rest.find("\n---\n").map(|at| at + 1)
        };
        let Some(opens_at) = opens_at else {
            body.push_str(rest);
            return body;
        };
        body.push_str(&rest[..opens_at]);
        let after_open = &rest[opens_at + "---\n".len()..];
        if let Some((_, under)) = after_open.split_once("\n---\n") {
            rest = under;
        } else {
            // A fence with no closing fence is a member's own prose.
            body.push_str(&rest[opens_at..]);
            return body;
        }
    }
}

/// The old bracketed-mark strip, kept for the cases that still name it.
#[allow(dead_code)]
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
    /// The bus the assembly runs on, kept so a test can wait on a turn's
    /// own settling instead of on the wire. A failed turn writes nothing to
    /// the store and sends nothing to the platform, so its events are the
    /// only place its ending is stated; see [`await_failure_latch`].
    pub bus: Arc<EventBus<CoreEvent>>,
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

    /// Reword the error every scripted failure streams. Nothing reads the
    /// text: a failed turn is silent in the chat whatever it says, and a
    /// test that words it differently asserts exactly that — the error
    /// reaches the log and no further.
    pub fn word_failures_as(&self, text: &str) {
        text.clone_into(&mut self.failure_text.lock().expect("the failure text locks"));
    }
}

/// A loopback address nothing listens on: a tool constructed over it can be
/// registered — and its name recorded in the tool choice — without any test traffic
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
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        Arc::new(providers),
        tools,
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: ModelBinding {
                provider_instance: "scripted-1".into(),
                provider_display_name: "Scripted".into(),
                vendor: VENDOR.into(),
                model: "script-model".into(),
                model_display_name: "Script Model".into(),
                context_window: None,
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
        bus,
        failures,
        seen,
        turn_hold,
        failure_text,
        title_requests,
    }
}

/// Wait until one conversation's failed turn has settled: the stream error
/// followed by the latch that closes the conversation on it. A failed turn
/// leaves no block and sends no message, so this is the only statement of
/// its ending — and reading the wire before it would race the core rather
/// than observe it.
pub async fn await_failure_latch(
    events: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
    conversation_id: i64,
) {
    let mut failed = false;
    loop {
        let event = tokio::time::timeout(DEADLINE, events.recv())
            .await
            .expect("the failed turn settles before the deadline")
            .expect("the bus outlives the test");
        match event {
            CoreEvent::StreamError {
                conversation_id: conv,
                ..
            } if conv == conversation_id => failed = true,
            CoreEvent::ConversationState {
                conversation_id: conv,
                latched: true,
                ..
            } if failed && conv == conversation_id => return,
            _ => {}
        }
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
                    bot: false,
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

/// Start the adapter and hand back its RUN, so a test can read what the run
/// answered. [`spawn_adapter`] asserts the run never fails, which is right
/// for every test but the one about a core that cannot serve.
pub fn spawn_adapter_for_outcome(
    server: &BotApiServer,
    state_file: &Path,
    assistant: Arc<Assistant>,
    sleep: Sleep,
) -> tokio::task::JoinHandle<Result<(), AdapterError>> {
    let mut config = Config::new(TOKEN, state_file);
    config.api_root = server.root();
    config.name = Some(NAME.to_owned());
    let adapter = TelegramAdapter::with_sleep(config, sleep);
    tokio::spawn(async move { adapter.run(assistant).await })
}

/// The public address the webhook pins configure — nothing resolves it and
/// nothing calls it: the scripted platform only records what it was told,
/// and the door under test is reached over loopback.
pub const WEBHOOK_PUBLIC_URL: &str = "https://assistant.example.org/telegram/webhook";

/// The path that address carries, which is the only path the door answers.
pub const WEBHOOK_PATH: &str = "/telegram/webhook";

/// The webhook wiring the pins hand the adapter: the address above and an
/// ephemeral loopback port. Two deployment decisions and nothing else — the
/// bound address the suite needs is announced through the adapter's own
/// seam, not through this configuration.
pub fn webhook_config(listen_port: u16) -> assistant_adapter_telegram::WebhookConfig {
    assistant_adapter_telegram::WebhookConfig {
        address: assistant_adapter_telegram::WebhookAddress::parse(WEBHOOK_PUBLIC_URL)
            .expect("the suite's public address parses"),
        listen_port,
    }
}

/// The adapter's configuration against the scripted server, in webhook mode.
pub fn webhook_adapter_config(
    server: &BotApiServer,
    state_file: &Path,
    listen_port: u16,
) -> Config {
    let mut config = Config::new(TOKEN, state_file);
    config.api_root = server.root();
    config.name = Some(NAME.to_owned());
    config.webhook = Some(webhook_config(listen_port));
    config
}

/// Start the adapter in webhook mode on an ephemeral loopback port, and
/// answer the address its listener bound. An extra observer of the bind runs
/// first, which is how the startup-order pin reads the platform's record at
/// the exact moment the port is bound and before anything is registered.
pub async fn spawn_webhook_adapter(
    server: &BotApiServer,
    state_file: &Path,
    assistant: Arc<Assistant>,
    sleep: Sleep,
    observer: Option<assistant_adapter_telegram::BoundListener>,
) -> (AdapterGuard, std::net::SocketAddr) {
    let (run, address) =
        spawn_webhook_adapter_for_outcome(server, state_file, assistant, sleep, observer).await;
    // The [`AdapterGuard`] task only reads the run's outcome, so it carries
    // the run's abort handle: dropping it ends the task that watches AND the
    // run it watches. Every webhook test relies on that to leave no listener
    // behind for the next one.
    let ended = AbortOnDrop(run.abort_handle());
    let guard = AdapterGuard(tokio::spawn(async move {
        let _ended = ended;
        if let Ok(outcome) = run.await {
            outcome.expect("the adapter takes its edge and serves");
        }
    }));
    (guard, address)
}

/// An abort handle that fires when it is dropped. Aborting a task that
/// already finished does nothing, so a run that ended by itself is
/// unaffected.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The same webhook start, handing back its RUN instead of asserting the run
/// never fails: what the one test about a core that cannot serve reads.
pub async fn spawn_webhook_adapter_for_outcome(
    server: &BotApiServer,
    state_file: &Path,
    assistant: Arc<Assistant>,
    sleep: Sleep,
    observer: Option<assistant_adapter_telegram::BoundListener>,
) -> (
    tokio::task::JoinHandle<Result<(), AdapterError>>,
    std::net::SocketAddr,
) {
    let (reported, mut bound) = mpsc::unbounded_channel();
    let announce: assistant_adapter_telegram::BoundListener = Arc::new(move |address| {
        if let Some(observer) = &observer {
            observer(address);
        }
        let _ = reported.send(address);
    });
    let config = webhook_adapter_config(server, state_file, 0);
    let adapter = TelegramAdapter::with_sleep(config, sleep).announcing_bound(announce);
    let run = tokio::spawn(async move { adapter.run(assistant).await });
    let address = tokio::time::timeout(DEADLINE, bound.recv())
        .await
        .expect("the listener binds within the deadline")
        .expect("the bound address is announced");
    (run, address)
}

/// The secret the adapter generated and kept beside its state file, read
/// from the adapter's own derivation of that path.
pub fn kept_secret(state_file: &Path) -> String {
    std::fs::read_to_string(assistant_adapter_telegram::webhook_secret_path(state_file))
        .expect("the webhook secret file reads")
        .trim()
        .to_owned()
}

/// Post one update to the door with the right secret; answers the status.
pub async fn deliver(address: std::net::SocketAddr, secret: &str, update: &Value) -> u16 {
    knock(
        address,
        reqwest::Method::POST,
        WEBHOOK_PATH,
        Some(secret),
        update.to_string().into_bytes(),
    )
    .await
}

/// One raw request at the door, over real local HTTP: whichever method, path,
/// secret and body the pin wants to see refused or served.
pub async fn knock(
    address: std::net::SocketAddr,
    method: reqwest::Method,
    path: &str,
    secret: Option<&str>,
    body: Vec<u8>,
) -> u16 {
    try_knock(address, method, path, secret, body)
        .await
        .expect("the door answers the request")
}

/// The same knock for the pins that expect some requests never to be
/// answered — the queue-full one, whose held requests outlive the adapter
/// they were sent to — where a transport failure is an outcome, not a
/// panic.
pub async fn try_knock(
    address: std::net::SocketAddr,
    method: reqwest::Method,
    path: &str,
    secret: Option<&str>,
    body: Vec<u8>,
) -> Option<u16> {
    let mut request = reqwest::Client::new()
        .request(method, format!("http://{address}{path}"))
        .body(body);
    if let Some(secret) = secret {
        request = request.header("X-Telegram-Bot-Api-Secret-Token", secret);
    }
    request
        .send()
        .await
        .ok()
        .map(|response| response.status().as_u16())
}

/// A sleep that records every requested duration and never finishes — what
/// the webhook pins hand the door, so its answer deadline provably never
/// fires and every status they read is the consumer's own outcome.
pub fn pending_sleep() -> (Sleep, Arc<Mutex<Vec<Duration>>>) {
    let waits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&waits);
    let sleep: Sleep = Arc::new(move |wait| {
        recorded.lock().expect("the wait log locks").push(wait);
        Box::pin(std::future::pending())
    });
    (sleep, waits)
}

/// A sleep whose first `answered` waits finish at once and whose later ones
/// never finish, recording every requested duration either way. The pin that
/// needs the door to give up on the first deliveries and then genuinely wait
/// on a later one uses it: the count is spent when a wait is REQUESTED, so
/// which delivery gets which behavior follows arrival order and nothing
/// else.
pub fn sleep_answering_first(answered: usize) -> (Sleep, Arc<Mutex<Vec<Duration>>>) {
    let waits = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&waits);
    let remaining = Arc::new(AtomicUsize::new(answered));
    let sleep: Sleep = Arc::new(move |wait| {
        recorded.lock().expect("the wait log locks").push(wait);
        if remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok()
        {
            Box::pin(tokio::task::yield_now())
                as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        } else {
            Box::pin(std::future::pending())
        }
    });
    (sleep, waits)
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

/// A message replying to one of the bot's own messages — addressed through
/// the reply rule — naming the platform id of the message of hers it points
/// at, in the given chat kind.
///
/// The id is a parameter because it is the whole subject of the delivery
/// pins (unit 38, 2026-08-30): a reply resolves against the id her send was
/// answered with, so a test that cannot name one proves nothing.
#[must_use]
pub fn reply_to_bot_message(
    update_id: i64,
    chat_kind: &str,
    chat_id: i64,
    user_id: i64,
    her_message_id: i64,
    text: &str,
) -> Value {
    let mut update = message_update(update_id, chat_kind, chat_id, user_id, text);
    update["message"]["reply_to_message"] = json!({
        "message_id": her_message_id,
        "date": date_of(update_id) - 1,
        "chat": { "id": chat_id, "type": chat_kind },
        "from": { "id": BOT_ID, "is_bot": true, "first_name": "Fixture", "username": BOT_USERNAME },
        "text": "an earlier answer",
    });
    update
}

/// A group message replying to one of the bot's own messages — addressed
/// through the reply rule, pointing at a message of hers this suite does
/// not otherwise name.
#[must_use]
pub fn reply_to_bot_update(update_id: i64, chat_id: i64, user_id: i64, text: &str) -> Value {
    reply_to_bot_message(
        update_id,
        "group",
        chat_id,
        user_id,
        message_id_of(update_id) - 1,
        text,
    )
}

/// The ledger as the consumer's own content: every block the assistant or a
/// member put there, and none of the framework's date records nor the
/// adapter's own delivery receipts.
///
/// The framework writes a `date_marker` on the first user-voiced append of
/// a day — its own calendar entry, ordered before the block that tripped
/// it, carrying no consumer content. A suite that spells out what the
/// adapter's traffic recorded is asserting about consumer content, so it
/// judges this view in one place instead of each test carrying its own
/// arithmetic about the framework's records. The kind is named through the
/// framework leaf's own `KINDS`, never a literal here.
///
/// A delivery receipt (unit 38, 2026-08-30) is filtered for a second
/// reason on top of that one: it is appended AFTER the send it records, so
/// its arrival is not ordered against anything a test drives, and a shape
/// assertion holding it would be racing the wire rather than pinning the
/// ledger. What the receipts themselves record is pinned by the suite's
/// `delivery` module, which reads them on purpose.
#[must_use]
pub fn consumer_view(blocks: &[Block]) -> Vec<Block> {
    blocks
        .iter()
        .filter(|block| {
            !DateMarker::KINDS.contains(&block.block_type.as_str())
                && block.block_type != assistant_core::delivery::DELIVERED_KIND
        })
        .cloned()
        .collect()
}

/// Every delivery receipt one conversation holds, oldest first (unit 38,
/// 2026-08-30) — the rows [`consumer_view`] filters out, read on purpose.
pub async fn receipts(store: &Store, conversation_id: i64) -> Vec<Delivered> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| block.block_type == assistant_core::delivery::DELIVERED_KIND)
        .map(Delivered::parse)
        .collect()
}

/// Await one conversation holding at least `count` delivery receipts, or
/// name the stall.
///
/// A receipt is appended AFTER the send it records, so a test that has seen
/// the send on the wire has not yet seen the record — and a test that wants
/// a later write ordered behind the receipt waits here for it, instead of
/// racing the two.
pub async fn await_receipts(store: &Store, conversation_id: i64, count: usize) -> Vec<Delivered> {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let rows = receipts(store, conversation_id).await;
        if rows.len() >= count {
            return rows;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {count} delivery receipts; have {}",
            rows.len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
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

/// The stillness [`await_quiet`] reads as a settled turn: long enough that
/// a turn between two of its own rounds is still moving, short enough to
/// cost a suite nothing.
const QUIET: Duration = Duration::from_millis(600);

/// Await every conversation falling quiet — no ledger of the store grew
/// across a whole [`QUIET`] window.
///
/// A turn takes several rounds since unit 55: the answer goes out through
/// a sending tool mid-turn, and the turn writes its own notes in the round
/// behind it. So a message pushed the instant the chat receives an answer
/// lands INSIDE the turn that answered, where it is absorbed rather than
/// summoning a turn of its own. A case that means "and then, later, she is
/// asked again" waits here first, or it is timing the wire instead of
/// testing the assistant.
pub async fn await_quiet(store: &Store) {
    let deadline = std::time::Instant::now() + DEADLINE;
    let mut seen = 0_usize;
    let mut since = std::time::Instant::now();
    loop {
        let mut blocks = 0_usize;
        for conversation in store
            .list_conversations()
            .await
            .expect("the conversation list reads")
        {
            blocks += store
                .list_blocks(conversation.id)
                .await
                .expect("the ledger reads")
                .len();
        }
        if blocks == seen {
            if since.elapsed() >= QUIET {
                return;
            }
        } else {
            seen = blocks;
            since = std::time::Instant::now();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the conversations to fall quiet"
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
