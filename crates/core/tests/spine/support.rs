//! Shared fixtures: the scripted provider, the assembly helpers, and the
//! ledger-polling helpers.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_ledger::agency::{DateMarker, LeafKind};
use agent_ledger::providers::{
    BoxFuture, ContentPart as WirePart, Message, MessageContent, MessageRole, ModelInfo,
    ProviderRx, ProviderTx, ReasoningLevel,
};
use agent_ledger::{
    Block, BlockKind, CoreEvent, EventBus, FromBlock, LlmError, ProviderModule, ProviderRegistry,
    ProviderRequest, ProviderResponse, StopReason, Store, StoreError, StreamEvent,
};
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::{
    Assistant, Authority, Budget, ChannelKey, ChannelKind, DeliveryHandle, DeliveryItem,
    InboundMessage, IngestReceipt, InvokedCommand, ModelBinding, Observation, ObserveOutcome,
    ObservedFact, OperatorConfig, Outbound, OutboundMark, OutboundReply, ProtectionConfig,
    SenderIdentity,
};
use serde_json::{Value, json};
use tokio::sync::{Semaphore, mpsc};

/// The provider module's type id — the binding's vendor.
pub const VENDOR: &str = "scripted";

/// The marker that tells a title derivation from a turn — the framework's
/// title request appends its title instruction to the conversation's prose,
/// a turn does not. The assembly switches title derivation off (decision
/// 0077), so a request carrying this mark is a regression: every scripted
/// provider counts it on [`ScriptHandle::title_requests`] instead of
/// answering it, and the zero pin fails loudly on the count. The text
/// mirrors the framework's instruction wording.
pub const TITLE_INSTRUCTION_MARK: &str = "Generate a concise title";

/// How long a poll or an awaited event may take before the test names a
/// stall.
pub const DEADLINE: Duration = Duration::from_secs(10);

/// The system prompt every test assembly is started with — a fixed fixture
/// string. The assembly records it with the composed identity and
/// answering sections behind it, so the prompt-recording assertions match
/// against [`composed_prompt`].
pub const SYSTEM_PROMPT: &str = "You are the suite's scripted assistant fixture.";

/// The assistant name every test assembly resolves — what the composed
/// prompt identity and the default disclosure line carry.
pub const NAME: &str = "Scripted";

/// The answering mode the shared fixtures run under: addressed, the quiet
/// shape, so the suite's standing pins — unaddressed messages rest, budgets
/// spend on addressed ones — keep their meaning. Helpful-mode tests choose
/// the mode through [`start_assistant_answering`].
pub const FIXTURE_ANSWERING: assistant_core::AnsweringMode =
    assistant_core::AnsweringMode::Addressed;

/// The whole system prompt a fixture assembly records, as the core
/// composes it — the default fixtures configure no moderation handle.
#[must_use]
pub fn composed_prompt() -> String {
    assistant_core::composed_system_prompt(
        SYSTEM_PROMPT,
        NAME,
        FIXTURE_ANSWERING,
        assistant_core::Capabilities::default(),
    )
}

/// The system prompt a reporting fixture records: helpful answering with
/// the moderation handle configured, so the moderation teaching rides it.
#[must_use]
pub fn composed_moderating_prompt() -> String {
    assistant_core::composed_system_prompt(
        SYSTEM_PROMPT,
        NAME,
        assistant_core::AnsweringMode::Helpful,
        assistant_core::Capabilities {
            moderation_handle: true,
            web_search: false,
        },
    )
}

/// The fixture's resolved disclosure: the line composed from [`NAME`],
/// exactly as the assembly resolves it with no `disclosure` override.
#[must_use]
pub fn fixture_disclosure() -> assistant_core::Disclosure {
    assistant_core::Disclosure::resolve(None, NAME)
}

/// An answer as its first delivery to someone carries it: the fixture's
/// disclosure line, a blank line, then the answer.
#[must_use]
pub fn disclosed(answer: &str) -> String {
    fixture_disclosure().disclosed(answer)
}

/// The answer the script streams for a turn whose newest projected message
/// carries this text. Deriving the answer from the request is what lets a
/// test pin a reply's text and channel key together: two channels' answers
/// differ, so a reply bound to the wrong key cannot pass by carrying an
/// identical constant.
#[must_use]
pub fn answer_to(text: &str) -> String {
    format!("The scripted answer to: {text}")
}

/// The projected text with the envelope stripped — what the scripted
/// provider derives its answer from. Every recorded message projects under
/// a fenced envelope naming its author, its send time and its msgid (unit
/// 55), the way a model reads past the header to the words; stripping here
/// keeps the suite's answer pins about the words while the envelope itself
/// is pinned where the projection rule is.
///
/// The strip is the envelope's own shape read back: a leading fence, the
/// header lines, the FIRST closing fence, and the body under it. A text
/// that does not open with a fence carries no envelope and is its own
/// body.
#[must_use]
pub fn without_envelope(text: &str) -> String {
    let mut body = String::new();
    let mut rest = text;
    loop {
        // An envelope opens at the start of the text or at a line start.
        // A projected turn joins several contributions, each with its own,
        // so the strip repeats instead of taking the first.
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

/// The cue that scripts a silent turn: a turn whose newest projected
/// message carries this text ends without streaming any text, the way the
/// prompt teaches the model to stay silent — and the framework commits
/// the turn as a real empty assistant text block.
pub const SILENT_CUE: &str = "(the quiet cue)";

/// The cue that scripts a turn of NOTES: a turn whose newest projected
/// message carries this text writes real text and SENDS NOTHING, so its
/// every word stays where only the model reads it.
///
/// The ordinary shape from unit 55 on, and the one a silent turn cannot
/// stand in for: a silent turn writes nothing at all, while this one proves
/// that written text — however much of it — reaches nobody.
pub const NOTES_CUE: &str = "(the notes cue)";

/// The cue that scripts a spoken don't-know: a turn whose projected
/// question carries this text answers with [`DONT_KNOW_LINE`] — the
/// model's own words for an unresolved lookup, ordinary prose to every
/// mechanism past the model.
pub const DONT_KNOW_CUE: &str = "(the unanswerable cue)";

/// The don't-know prose the script answers with on [`DONT_KNOW_CUE`]: the
/// plain admission the teaching draws when the model was addressed and a
/// lookup could not back an answer. A fixture string, not product copy —
/// the live model words its own.
pub const DONT_KNOW_LINE: &str = "I don't know — I looked this up and could not confirm an answer.";

/// The cue that scripts a THREADED send: a turn whose newest ask carries
/// this text answers it with the reply tool, aimed at that ask's own msgid
/// — the way the model chooses a target from unit 55 on, instead of the
/// edge deriving one.
pub const REPLY_CUE: &str = "(the threaded cue)";

/// The msgid the newest ask declares, read off its own envelope.
///
/// The LAST one in the message, because a projected user turn joins several
/// contributions under one message and the newest is the one at the end —
/// which is the one a model aiming at "what was just said" would name.
#[must_use]
pub fn newest_msgid(messages: &[Message]) -> Option<String> {
    let ask = messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User && !carries_tool_result(message))
        .map(message_text)?;
    ask.lines()
        .filter_map(|line| line.strip_prefix("msgid: "))
        .next_back()
        .map(ToOwned::to_owned)
}

/// The cue that scripts a clarifying question: a turn whose newest
/// projected message carries this text answers with
/// [`CLARIFYING_QUESTION`] as its whole answer, the way the teaching has
/// the model ask before assuming on a use-versus-build ambiguity
/// (unit 21).
pub const CLARIFY_CUE: &str = "(the ambiguous cue)";

/// The clarifying question the script answers with on [`CLARIFY_CUE`]: the
/// one brief question the teaching draws on a question genuinely ambiguous
/// between using and building — ordinary prose to every mechanism past the
/// model, which is the mechanical fact the audience pins prove.
pub const CLARIFYING_QUESTION: &str =
    "Are you asking how to use it on your device, or how to build it into a ROM?";

/// The first answer a person is delivered, as the edge stores and sends it:
/// the disclosure line, a blank line, then the scripted answer to the given
/// text.
#[must_use]
pub fn first_answer_to(text: &str) -> String {
    disclosed(&answer_to(text))
}

/// The acknowledgment a real rules delta delivers under the scripted
/// provider (unit 20): the one-shot request carries the new rules text
/// verbatim as its newest message, so the script's derived answer proves
/// both halves at once — the rules reached the request, and the model's
/// output is what the chat receives.
#[must_use]
pub fn scripted_acknowledgment(rules_text: &str) -> String {
    answer_to(rules_text)
}

/// A file path for one test's store, deleted with its sidecar files when this
/// value drops, so parallel tests never share a database and no run leaves
/// litter.
pub struct TempDb(std::path::PathBuf);

impl TempDb {
    pub fn new(name: &str) -> Self {
        let unique = format!(
            "assistant-core-{name}-{}-{}.sqlite3",
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

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A hold on the scripted stream: each turn announces itself when its prose
/// has streamed, then waits for a permit before ending the message. This is
/// what keeps a stream provably open while a test appends a second message.
pub struct TurnHold {
    started: tokio::sync::Mutex<mpsc::UnboundedReceiver<usize>>,
    started_tx: mpsc::UnboundedSender<usize>,
    permits: Semaphore,
}

impl TurnHold {
    pub fn new() -> Arc<Self> {
        let (started_tx, started) = mpsc::unbounded_channel();
        Arc::new(Self {
            started: tokio::sync::Mutex::new(started),
            started_tx,
            permits: Semaphore::new(0),
        })
    }

    /// Await the next turn's announcement: its stream is open and held.
    pub async fn started(&self) -> usize {
        tokio::time::timeout(DEADLINE, self.started.lock().await.recv())
            .await
            .expect("a turn announces itself before the deadline")
            .expect("the provider outlives the test")
    }

    /// Let the held turn end its message.
    pub fn release(&self) {
        self.permits.add_permits(1);
    }

    /// Announce this turn and park it until the test releases it OR the turn
    /// is torn down. Answers whether the provider carries on; `false` ends
    /// its task, as a torn-down wire does.
    ///
    /// The teardown arm is the load-bearing one and both scripted providers
    /// take it from here rather than each spelling it out: an interrupt drops
    /// the request sender, and a held provider that kept its response channel
    /// open past that would hold the settle protocol under test hostage — no
    /// reader ever drains, so no stream-end signal is emitted and every
    /// settle burns its whole bound against a stream the framework has
    /// already torn down.
    async fn await_release(
        &self,
        turn: usize,
        requests: &mut mpsc::UnboundedReceiver<ProviderRequest>,
    ) -> bool {
        let _ = self.started_tx.send(turn);
        tokio::select! {
            permit = self.permits.acquire() => match permit {
                Ok(permit) => {
                    permit.forget();
                    true
                }
                // The hold closed with the test.
                Err(_) => false,
            },
            _ = requests.recv() => false,
        }
    }
}

/// The error text an unqualified scripted failure carries: prose in no
/// particular shape, standing for every failure the consumer classifies as
/// ordinary.
pub const SCRIPTED_FAILURE: &str = "scripted stream failure";

/// What a test observes of the scripted provider: turn count, every turn
/// request's projected messages, and the failure script.
#[derive(Clone)]
pub struct ScriptHandle {
    pub turns: Arc<AtomicUsize>,
    pub seen: Arc<std::sync::Mutex<Vec<Vec<Message>>>>,
    /// The reasoning level each answered turn's request carried, in turn
    /// order — what the framework resolved from the conversation's stored
    /// key, observed at the provider's edge so a test pins the level end to
    /// end instead of at the row alone.
    pub reasonings: Arc<std::sync::Mutex<Vec<Option<ReasoningLevel>>>>,
    /// The error texts the upcoming turns fail with, one per turn, in order;
    /// a turn that finds the queue empty answers normally. Texts and not a
    /// count, because the consumer reads the error text to classify the
    /// failure, so a test that pins a classification has to say which text
    /// arrives.
    pub failures: Arc<std::sync::Mutex<std::collections::VecDeque<String>>>,
    /// Every title derivation request that reached the provider. The
    /// assembly switches titles off (decision 0077), so the pins assert
    /// zero here; any count above it is a regression.
    pub title_requests: Arc<AtomicUsize>,
}

impl ScriptHandle {
    /// A fresh handle, all observations at zero.
    pub fn fresh() -> Self {
        Self {
            turns: Arc::new(AtomicUsize::new(0)),
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            reasonings: Arc::new(std::sync::Mutex::new(Vec::new())),
            failures: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            title_requests: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Script the next `count` turns to fail with the unqualified
    /// [`SCRIPTED_FAILURE`] text.
    pub fn fail_next_turns(&self, count: usize) {
        self.fail_next_turns_with(count, SCRIPTED_FAILURE);
    }

    /// Script the next `count` turns to fail with the given error text — the
    /// wire's rendering of a particular provider refusal, for a test that
    /// pins how the consumer classifies it.
    pub fn fail_next_turns_with(&self, count: usize, error: &str) {
        let mut scripted = self.failures.lock().unwrap();
        for _ in 0..count {
            scripted.push_back(error.to_owned());
        }
    }
}

/// A provider module whose whole behavior is its bind. The trait's
/// configuration and catalog surface is inert scaffolding every scripted
/// stub shares; a test module supplies only the stream shape it actually
/// varies, so a provider-trait change is absorbed here once.
struct StubProvider<F> {
    display_name: &'static str,
    description: &'static str,
    bind: F,
}

/// Build a boxed provider stub under the suite's [`VENDOR`] type id, with
/// the given bind called once per conversation binding.
pub fn provider_stub<F>(
    display_name: &'static str,
    description: &'static str,
    bind: F,
) -> Box<dyn ProviderModule>
where
    F: Fn() -> (ProviderTx, ProviderRx) + Send + Sync + 'static,
{
    Box::new(StubProvider {
        display_name,
        description,
        bind,
    })
}

impl<F> ProviderModule for StubProvider<F>
where
    F: Fn() -> (ProviderTx, ProviderRx) + Send + Sync,
{
    fn type_id(&self) -> &'static str {
        VENDOR
    }
    fn display_name(&self) -> &'static str {
        self.display_name
    }
    fn description(&self) -> &'static str {
        self.description
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
        (self.bind)()
    }
}

/// Build the scripted provider and the handle a test observes it through:
/// every turn SENDS [`answer_to`] the newest projected message's text,
/// deterministically, so turn-count and block-by-block assertions stay
/// exact. A title derivation request is counted, never answered — titles
/// are off (decision 0077), so one arriving is the regression the zero pin
/// exists for.
///
/// It sends through the plain sending tool because that is the only way
/// anything reaches a chat from unit 55 on: a turn that streamed its answer
/// as text would be a turn of private notes, and every reply pin in this
/// suite would wait forever. So an answered turn is TWO requests — the
/// round that makes the call, and the round that reads its result and ends
/// the turn with nothing more to say — which is the production shape.
#[allow(clippy::too_many_lines)]
pub fn scripted_provider(hold: Option<Arc<TurnHold>>) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = ScriptHandle::fresh();
    let script = handle.clone();
    let provider = provider_stub("Scripted", "answers from a script", move || {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        let turns = Arc::clone(&script.turns);
        let seen = Arc::clone(&script.seen);
        let reasonings = Arc::clone(&script.reasonings);
        let failures = Arc::clone(&script.failures);
        let title_requests = Arc::clone(&script.title_requests);
        let hold = hold.clone();
        tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let ProviderRequest::Stream {
                    messages,
                    reasoning,
                    ..
                } = request
                else {
                    continue;
                };
                if messages.iter().any(|m| carries(m, TITLE_INSTRUCTION_MARK)) {
                    title_requests.fetch_add(1, Ordering::SeqCst);
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                // Connected mirrors the real wire's first stream event; the
                // runtime surfaces it as stream status, which is what the
                // assembly's stream observer keys on.
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                let scripted_failure = failures.lock().unwrap().pop_front();
                if let Some(failure) = scripted_failure {
                    let _ = response_tx.send(ProviderResponse::Error(failure));
                    continue;
                }
                // A compaction's own turn, recognized by the harness
                // message it carries: it is answered with the summary,
                // as TEXT — the digest is the one-shot's product, not a
                // message anybody is sent.
                if is_compaction_turn(&messages) {
                    turns.fetch_add(1, Ordering::SeqCst);
                    seen.lock().unwrap().push(messages);
                    reasonings.lock().unwrap().push(reasoning);
                    answer_compaction(&response_tx);
                    continue;
                }
                // The rules acknowledgment is a one-shot generation and
                // not a turn: its product is the text the item carries, so
                // it is answered with text and never with a send.
                if is_acknowledgment_request(&messages) {
                    let rules =
                        without_envelope(&messages.last().map(message_text).unwrap_or_default());
                    // The silent cue streams nothing here too, which is what
                    // the deterministic fallback's own pin drives.
                    let text = (!rules.contains(SILENT_CUE)).then(|| answer_to(&rules));
                    turns.fetch_add(1, Ordering::SeqCst);
                    seen.lock().unwrap().push(messages);
                    reasonings.lock().unwrap().push(reasoning);
                    if let Some(text) = text {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx
                            .send(ProviderResponse::Event(StreamEvent::TextDelta { text }));
                    }
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                        usage: agent_ledger::providers::Usage::default(),
                        stop_reason: StopReason::EndTurn,
                    }));
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                // The ASK is the newest thing a person said, wherever the
                // round stands: on the closing round the newest message of
                // all is the send's own result, and the answer is still
                // derived from the question, so the suite's pins read one
                // deterministic text either way.
                let newest = without_envelope(&newest_ask(&messages));
                let answer = if newest.contains(SILENT_CUE) {
                    None
                } else if newest.contains(DONT_KNOW_CUE) {
                    Some(DONT_KNOW_LINE.to_owned())
                } else if newest.contains(CLARIFY_CUE) {
                    Some(CLARIFYING_QUESTION.to_owned())
                } else {
                    Some(answer_to(&newest))
                };
                // The closing round: this request carries the send's own
                // result, so the message is already in the chat. The turn
                // writes down what it said — private notes, which reach
                // nobody — and ends.
                let closing = answers_a_send(&messages);
                let turn = turns.fetch_add(1, Ordering::SeqCst) + 1;
                seen.lock().unwrap().push(messages);
                reasonings.lock().unwrap().push(reasoning);
                // A HELD turn writes its notes before it is held, so the
                // ledger carries a streaming tail while the test holds it —
                // the stored evidence that a stream is open. An unheld turn
                // writes them in its closing round instead, which keeps the
                // ordinary shape one text block per turn either way.
                if let Some(hold) = &hold {
                    if closing {
                        continue_after_notes(&response_tx);
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    if let Some(text) = &answer {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: text.clone(),
                        }));
                    }
                    if !hold.await_release(turn, &mut requests).await {
                        break;
                    }
                }
                let was_held = hold.is_some();
                // The notes turn: real text, no send. It takes the writing
                // arm below, which is the same arm a closing round takes —
                // the words are committed and the turn ends there.
                let notes_only = newest.contains(NOTES_CUE);
                match answer {
                    Some(text) if !closing && !notes_only => {
                        let aimed = newest
                            .contains(REPLY_CUE)
                            .then(|| {
                                newest_msgid(
                                    seen.lock()
                                        .unwrap()
                                        .last()
                                        .expect("the request was just recorded"),
                                )
                            })
                            .flatten();
                        send_scripted_message(&response_tx, turn, &text, aimed.as_deref());
                    }
                    answer => {
                        // The notes: the words the turn sent, written down
                        // where only the model reads them. A silent turn
                        // writes nothing, exactly as before — and a held
                        // turn wrote them before it was held.
                        if let Some(text) = answer.filter(|_| (closing || notes_only) && !was_held)
                        {
                            let _ = response_tx
                                .send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                            let _ = response_tx
                                .send(ProviderResponse::Event(StreamEvent::TextDelta { text }));
                        }
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::EndTurn,
                            }));
                    }
                }
                // The trailing done real wires send after every completed
                // turn: the framework settles the dispatch state on the
                // closed signal, so a scripted turn ending without it would
                // leave the state open and stall the suite.
                let _ = response_tx.send(ProviderResponse::Done);
            }
        });
        (request_tx, responses)
    });
    (provider, handle)
}

/// The newest thing a PERSON said in one projected request — what every
/// scripted answer is derived from.
///
/// Not simply the newest message: from unit 55 a turn takes several rounds,
/// and the newest message of a later round is the send's own result. A tool
/// result is user-voiced on this wire, so the role alone does not tell the
/// two apart and the parts do: a message carrying one is machinery, never
/// an ask. A request with no ask at all answers the empty string, which no
/// cue matches and which the ordinary answer names plainly.
pub fn newest_ask(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User && !carries_tool_result(message))
        .map(message_text)
        .unwrap_or_default()
}

/// The provider id every scripted SEND is made under — what tells a send's
/// own result apart from any other tool's, inside the fixture and without
/// reading a sentence the core writes for the model.
const SEND_CALL_ID: &str = "send-";

/// Whether one projected request's NEWEST message is a SEND's own result —
/// the cue that this round closes a turn whose message is already in the
/// chat.
///
/// The newest and not "any", which is the whole of the distinction: every
/// round after the first send carries that send's result somewhere in the
/// history, and a round that read history as its cue would never send
/// again. And a send's result and not any result, so a script that calls a
/// lookup first still gets its closing round.
pub fn answers_a_send(messages: &[Message]) -> bool {
    messages
        .last()
        .is_some_and(|message| tool_result_ids(message).any(|id| id.starts_with(SEND_CALL_ID)))
}

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

/// Whether one projected request's NEWEST message answers a call of ANY
/// tool — the cue that this round continues a turn whose call has just
/// been resolved, rather than opening one.
///
/// Scoped to the newest message on purpose: a conversation that has
/// answered a call before holds that result forever, and a script reading
/// history as its cue would replay its closing round on every later turn.
pub fn answers_any_call(messages: &[Message]) -> bool {
    messages.last().is_some_and(carries_tool_result)
}

/// Whether one projected request carries the answer to a call of a tool
/// that is NOT a sending tool, anywhere in its history — the cue a
/// tool-scripted turn closes on.
///
/// It reads the whole history and not the newest message, because that is
/// what the scripts mean by "my call has been answered": the script plays
/// one call per conversation, and every later turn is the closing kind.
/// The send's own results are excluded, or the closing round would read
/// its own transport as its answer.
pub fn a_call_was_answered(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|message| tool_result_ids(message).any(|id| !id.starts_with(SEND_CALL_ID)))
}

/// How many of one request's tool results answer a SEND — what a
/// round-indexed script subtracts, so its own rounds keep their numbers
/// while the sends between them accumulate.
pub fn send_results(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|message| {
            tool_result_ids(message)
                .filter(|id| id.starts_with(SEND_CALL_ID))
                .count()
        })
        .sum()
}

/// The same count for THIS TURN alone: the sends answered since the newest
/// thing a person said.
///
/// A conversation keeps its history, so a whole-request count would carry
/// every earlier turn's sends into the reading and tell a fresh turn it had
/// already spoken. The newest ask is the turn's own boundary.
pub fn send_results_this_turn(messages: &[Message]) -> usize {
    let since = messages
        .iter()
        .rposition(|message| message.role == MessageRole::User && !carries_tool_result(message))
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

/// End a turn that has nothing more to write.
fn continue_after_notes(response_tx: &mpsc::UnboundedSender<ProviderResponse>) {
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
        usage: agent_ledger::providers::Usage::default(),
        stop_reason: StopReason::EndTurn,
    }));
}

/// Emit one scripted send: the production event order — the message end
/// carrying the tool-use stop reason, then the call's lifecycle — for a
/// call of the plain sending tool carrying the given text, or of the reply
/// tool where a target is named.
///
/// One emitter, because every scripted provider in this suite that puts
/// words in a chat has to put them there the same way the real one does,
/// and a second spelling of the call shape would be a second place for it
/// to drift.
pub fn send_scripted_message(
    response_tx: &mpsc::UnboundedSender<ProviderResponse>,
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

/// The one message [`sending_stub`] puts in a chat, every turn.
pub const SENT_LINE: &str = "The stub's sent message.";

/// A provider whose every turn SENDS one fixed message and then ends.
///
/// The shape a budget pin needs from unit 55 on: a debt counts against a
/// budget only once its turn delivered a message (AC13), so a provider that
/// answers nothing spends nothing and every budget stays open forever. This
/// one spends exactly one slot per turn, deterministically, without
/// deriving anything from the request — which is what keeps the stamp pins
/// reading stamps and not prose.
pub fn sending_stub() -> Box<dyn ProviderModule> {
    provider_stub("Sending stub", "sends one fixed message per turn", || {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut turn = 0_usize;
            while let Some(request) = requests.recv().await {
                let ProviderRequest::Stream { messages, .. } = request else {
                    continue;
                };
                if messages.iter().any(|m| carries(m, TITLE_INSTRUCTION_MARK)) {
                    let _ = response_tx.send(ProviderResponse::Done);
                    continue;
                }
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                if answers_a_send(&messages) {
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                        usage: agent_ledger::providers::Usage::default(),
                        stop_reason: StopReason::EndTurn,
                    }));
                } else {
                    turn += 1;
                    send_scripted_message(&response_tx, turn, SENT_LINE, None);
                }
                let _ = response_tx.send(ProviderResponse::Done);
            }
        });
        (request_tx, responses)
    })
}

/// Wait until one conversation holds `count` SENT messages whose calls have
/// been settled — what a budget pin waits for between asks, because a debt
/// counts only once its turn delivered.
pub async fn await_sends(store: &Store, conversation_id: i64, count: usize) {
    await_ledger(
        store,
        conversation_id,
        "the delivered sends",
        move |blocks| {
            blocks
                .iter()
                .filter(|block| block.block_type == "tool_result")
                .count()
                >= count
        },
    )
    .await;
}

/// A provider that accepts every stream request and never answers: the tail
/// under it stays the newest recorded message, so every stamp is observable
/// without racing an answer, and a ledger built through the production
/// ingest path holds exactly the recorded messages.
pub fn silent_provider() -> Box<dyn ProviderModule> {
    provider_stub("Silent", "accepts and never answers", || {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while requests.recv().await.is_some() {}
            drop(response_tx);
        });
        (request_tx, responses)
    })
}

/// One process of a multi-phase test: its own runtime, dropped at the end
/// of the phase so every task the assembly spawned dies with it — the
/// store is shared state on disk, the runtime is not.
pub fn process_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_time()
        .build()
        .expect("a runtime for one simulated process")
}

/// One tool-call script: what the opening turn calls, with what input, and
/// whether prose narrates before the call. Patterned on the framework's own
/// tool-call event shapes and their production order: the message end with
/// the tool-use stop reason comes first, then the drained tool lifecycle —
/// the input as streamed deltas closed by the terminal end.
#[derive(Clone)]
pub struct ToolScript {
    /// The registered tool name the opening turn calls.
    pub tool: String,
    /// The call's input JSON, non-empty by the script's contract.
    pub input: String,
    /// Prose streamed before the call, for the narration variant: the
    /// model's own notes, which reach nobody (unit 55).
    pub narration: Option<String>,
    /// A line the round SENDS beside its call, where the script has one —
    /// the heads-up before slow work, which from unit 55 is a message like
    /// any other and not prose the relay carried.
    pub announce: Option<String>,
}

/// The closing prose the tool script streams once a request carries an
/// answered call.
pub const CLOSING_ANSWER: &str = "The scripted closing answer.";

/// A distinctive clause of the core's rules-acknowledgment instruction —
/// the one-shot generation the observation surface runs when a real rules
/// delta lands. It is NOT a turn: its whole product is the text the item
/// carries, so the scripted providers answer it with text and never with a
/// send (unit 55).
pub const ACKNOWLEDGMENT_MARK: &str = "the group's newly pinned rules";

/// Whether one request is that one-shot generation.
pub fn is_acknowledgment_request(messages: &[Message]) -> bool {
    messages.iter().any(|m| carries(m, ACKNOWLEDGMENT_MARK))
}

/// A distinctive opening clause of the core's compaction instructions — the
/// harness message a compaction appends to its temporary conversation, and
/// the one thing in this suite that carries these words. The core pins the
/// instructions' full text; the scripted providers only need to recognize
/// the turn.
pub const COMPACTION_MARK: &str = "You are compacting the conversation above";

/// What the scripted providers answer a compaction's turn with — the
/// summary a compacted thread then carries as its compaction message.
pub const SCRIPTED_SUMMARY: &str = "The scripted summary of the first half.";

/// Whether this request is a compaction's own turn: its harness message
/// carries the instructions' opening clause.
pub fn is_compaction_turn(messages: &[Message]) -> bool {
    messages.iter().any(|m| carries(m, COMPACTION_MARK))
}

/// Stream the scripted summary and end the turn — what every scripted
/// provider answers a compaction's turn with, written once so the two of
/// them cannot answer it differently.
fn answer_compaction(response_tx: &mpsc::UnboundedSender<ProviderResponse>) {
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
        text: SCRIPTED_SUMMARY.into(),
    }));
    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
        usage: agent_ledger::providers::Usage::default(),
        stop_reason: StopReason::EndTurn,
    }));
    let _ = response_tx.send(ProviderResponse::Done);
}

/// Build the tool-scripted provider: the opening turn answers with one tool
/// call carrying the script's input, and every request already carrying an
/// answered call — a result or a recorded decline alike, both projecting as
/// tool-result parts — answers with [`CLOSING_ANSWER`]. Scripted by ledger
/// content, not arrival order, so reruns and absorbed turns stay exact.
/// With a hold, the opening turn announces itself after its narration and
/// before the call events, which is what lets a test absorb a message
/// mid-turn, provably before the call block exists.
///
/// Every completed turn ends with the trailing done real wires send. A
/// request repeating the already-played call round replays it — the
/// framework's duplicate-turn window is closed, so a replay is a regression
/// the suite's turn counts and ledger shapes fail loudly on instead of a
/// tolerated echo.
#[allow(clippy::too_many_lines)]
pub fn tool_scripted_provider(
    script: ToolScript,
    hold: Option<Arc<TurnHold>>,
) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = ScriptHandle::fresh();
    let observed = handle.clone();
    let provider = provider_stub(
        "Scripted tools",
        "calls one tool from a script",
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let turns = Arc::clone(&observed.turns);
            let seen = Arc::clone(&observed.seen);
            let failures = Arc::clone(&observed.failures);
            let title_requests = Arc::clone(&observed.title_requests);
            let script = script.clone();
            let hold = hold.clone();
            tokio::spawn(async move {
                while let Some(request) = requests.recv().await {
                    let ProviderRequest::Stream { messages, .. } = request else {
                        continue;
                    };
                    if messages.iter().any(|m| carries(m, TITLE_INSTRUCTION_MARK)) {
                        title_requests.fetch_add(1, Ordering::SeqCst);
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    // A compaction's own turn, recognized by the harness
                    // message it carries: it is answered with the summary
                    // and never with the tool script, whose call the empty
                    // tool choice would refuse anyway. A scripted failure
                    // takes it like any other turn, which is how a test
                    // drives a capture that produces nothing.
                    if is_compaction_turn(&messages) {
                        seen.lock().unwrap().push(messages);
                        let scripted_failure = failures.lock().unwrap().pop_front();
                        match scripted_failure {
                            Some(failure) => {
                                let _ = response_tx
                                    .send(ProviderResponse::Event(StreamEvent::Connected));
                                let _ = response_tx.send(ProviderResponse::Error(failure));
                            }
                            None => answer_compaction(&response_tx),
                        }
                        continue;
                    }
                    // How many sends this script owes before its closing
                    // one: the announce, where the script has a line to
                    // announce with. Counting them is what tells the
                    // closing round from the round after it, on a wire
                    // where both carry a send's result.
                    let announces = usize::from(script.announce.is_some());
                    let sent = send_results_this_turn(&messages) > announces;
                    let answered = a_call_was_answered(&messages);
                    // The sufficiency script: a question carrying the
                    // don't-know cue closes with the plain don't-know prose
                    // after its lookup — the taught reaction to a result
                    // that did not contain the claim — while every other
                    // turn closes with the ordinary answer.
                    let missed = messages.iter().any(|m| carries(m, DONT_KNOW_CUE));
                    let closing_text = if missed {
                        DONT_KNOW_LINE
                    } else {
                        CLOSING_ANSWER
                    };
                    let turn = turns.fetch_add(1, Ordering::SeqCst) + 1;
                    seen.lock().unwrap().push(messages);
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    // The round after the send: the message is in the chat,
                    // so the turn writes down what it said and ends.
                    if sent {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: closing_text.into(),
                        }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::EndTurn,
                            }));
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    // The scripted call is answered: the closing words go
                    // to the chat the one way anything does.
                    if answered {
                        send_scripted_message(&response_tx, turn, closing_text, None);
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
                    if let Some(hold) = &hold
                        && !hold.await_release(turn, &mut requests).await
                    {
                        break;
                    }
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
                    // relay is gone, so a line the chat must read before
                    // the slow work is a message like any other.
                    if let Some(announce) = &script.announce {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                                id: format!("{SEND_CALL_ID}{turn}"),
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
                        id: format!("call-{turn}"),
                        name: script.tool.clone(),
                    }));
                    let _ =
                        response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseInputDelta {
                            json: script.input.clone(),
                        }));
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        },
    );
    (provider, handle)
}

/// One call of a same-round batch: the registered tool and the input JSON
/// it carries.
#[derive(Clone)]
pub struct RoundCall {
    /// The registered tool name this call names.
    pub tool: String,
    /// The call's input JSON, non-empty by the batch's contract.
    pub input: String,
}

/// Build a provider whose opening turn emits SEVERAL tool calls in ONE
/// round — the shape the runner executes in parallel tasks, which is what
/// every serialization claim about a scan-then-append pair has to survive.
/// Every later request, which carries the answered calls, closes with
/// [`CLOSING_ANSWER`].
///
/// The event order is the production one [`tool_scripted_provider`] states:
/// the message end with the tool-use stop reason, then each call's
/// lifecycle, then the trailing done.
pub fn same_round_calls_provider(calls: Vec<RoundCall>) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = ScriptHandle::fresh();
    let observed = handle.clone();
    let provider = provider_stub(
        "Same-round caller",
        "calls several tools in one round",
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let turns = Arc::clone(&observed.turns);
            let calls = calls.clone();
            tokio::spawn(async move {
                while let Some(request) = requests.recv().await {
                    let ProviderRequest::Stream { messages, .. } = request else {
                        continue;
                    };
                    let turn = turns.fetch_add(1, Ordering::SeqCst) + 1;
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    if answers_a_send(&messages) {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: CLOSING_ANSWER.into(),
                        }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::EndTurn,
                            }));
                    } else if answers_any_call(&messages) {
                        send_scripted_message(&response_tx, turn, CLOSING_ANSWER, None);
                    } else {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::ToolUse,
                            }));
                        for (index, call) in calls.iter().enumerate() {
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::ToolUseStart {
                                    id: format!("call-{index}"),
                                    name: call.tool.clone(),
                                },
                            ));
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::ToolUseInputDelta {
                                    json: call.input.clone(),
                                },
                            ));
                            let _ =
                                response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                        }
                    }
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        },
    );
    (provider, handle)
}

/// Whether one projected message carries a tool-result part — the scripted
/// ledger-content cue for the closing prose. A recorded decline projects
/// the same part shape, so a declined call closes a turn the same way.
pub fn carries_tool_result(message: &Message) -> bool {
    matches!(&message.content, MessageContent::Parts(parts)
        if parts.iter().any(|part| matches!(part, WirePart::ToolResult { .. })))
}

/// One scripted round of the multi-round tool provider: what the round
/// narrates, whether it holds for the test, and which tool it calls.
#[derive(Clone, Copy)]
pub struct Round {
    /// Prose streamed before the round's action, finalized by the message
    /// end ahead of any tool events, per the production order.
    pub narration: Option<&'static str>,
    /// Whether the round announces on the hold and waits for one release
    /// in the window between the message end that finalized its narration
    /// and its tool events — where the ledger's tail is the finalized
    /// narration text, and where the redispatch canary absorbs a message.
    pub hold_after_finalize: bool,
    /// Whether the round announces on the hold and waits for one release
    /// between its tool events and its trailing done — the still-open
    /// stream a real wire keeps while the runner records the call and its
    /// result, so a test can absorb a message that sits AFTER the round's
    /// result and BEFORE the continuation the close re-drives. Only a
    /// calling round holds here; a closing round's done follows its
    /// message end directly.
    pub hold_before_done: bool,
    /// The registered tool the round calls with a fixed probe input.
    /// `None` closes the turn with [`CLOSING_ANSWER`]; a closing round
    /// names no narration.
    pub call: Option<&'static str>,
}

/// Build a provider playing one scripted round per model request, indexed
/// by how many resolved calls the projected request already carries —
/// scripted by ledger content, like [`tool_scripted_provider`], so reruns
/// stay exact. Every hold announces its round on the shared [`TurnHold`]
/// and waits for one release, in round order, so a test absorbs messages
/// at exactly the scripted windows.
///
/// Every completed round ends with the trailing done real wires send. A
/// repeated request for an already-played round replays it — the
/// framework's duplicate-turn window is closed, so a replay is a
/// regression the redispatch canary's turn count fails loudly on instead
/// of a tolerated echo.
pub fn round_scripted_provider(
    rounds: Vec<Round>,
    hold: Arc<TurnHold>,
) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = ScriptHandle::fresh();
    let observed = handle.clone();
    let rounds = Arc::new(rounds);
    let provider = provider_stub(
        "Scripted rounds",
        "plays one scripted round per request",
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let turns = Arc::clone(&observed.turns);
            let seen = Arc::clone(&observed.seen);
            let title_requests = Arc::clone(&observed.title_requests);
            let rounds = Arc::clone(&rounds);
            let hold = Arc::clone(&hold);
            tokio::spawn(async move {
                while let Some(request) = requests.recv().await {
                    let ProviderRequest::Stream { messages, .. } = request else {
                        continue;
                    };
                    if messages.iter().any(|m| carries(m, TITLE_INSTRUCTION_MARK)) {
                        title_requests.fetch_add(1, Ordering::SeqCst);
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    // The rounds are numbered by the script's OWN resolved
                    // calls: a send's result is transport, not a round, so
                    // it is subtracted and the script keeps its numbering.
                    let resolved = tool_result_parts(&messages) - send_results(&messages);
                    let round = rounds[resolved.min(rounds.len() - 1)];
                    let turn = turns.fetch_add(1, Ordering::SeqCst) + 1;
                    let sent = answers_a_send(&messages);
                    seen.lock().unwrap().push(messages);
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    // The round after a send: the words are in the chat, so
                    // the turn writes them down and ends.
                    if sent {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: CLOSING_ANSWER.into(),
                        }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::EndTurn,
                            }));
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    if let Some(narration) = round.narration {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: narration.into(),
                        }));
                    }
                    if let Some(tool) = round.call {
                        // The production order: the message end finalizes
                        // any narration before the tool lifecycle streams.
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: StopReason::ToolUse,
                            }));
                        if round.hold_after_finalize && !pause(&hold, resolved).await {
                            break;
                        }
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                                id: format!("call-{resolved}"),
                                name: tool.into(),
                            }));
                        let _ = response_tx.send(ProviderResponse::Event(
                            StreamEvent::ToolUseInputDelta {
                                json: r#"{"ask":"run"}"#.into(),
                            },
                        ));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                        if round.hold_before_done && !pause(&hold, resolved).await {
                            break;
                        }
                    } else {
                        send_scripted_message(&response_tx, turn, CLOSING_ANSWER, None);
                    }
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        },
    );
    (provider, handle)
}

/// Announce the round on the hold and wait for the test's release; false
/// when the hold closed with the test, which ends the provider task.
async fn pause(hold: &TurnHold, round: usize) -> bool {
    let _ = hold.started_tx.send(round);
    match hold.permits.acquire().await {
        Ok(permit) => {
            permit.forget();
            true
        }
        Err(_) => false,
    }
}

/// How many resolved calls a projected request carries: its tool-result
/// parts, counted across every message — a recorded decline projects the
/// same part shape, so a declined round counts like an answered one.
fn tool_result_parts(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|message| match &message.content {
            MessageContent::Parts(parts) => parts
                .iter()
                .filter(|part| matches!(part, WirePart::ToolResult { .. }))
                .count(),
            MessageContent::Text(_) => 0,
        })
        .sum()
}

/// Whether one projected message carries this text, in either content mode —
/// which mode the fold picks is the projection's decision, not a test's.
pub fn carries(message: &Message, needle: &str) -> bool {
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

/// The binding every test assembles under.
pub fn binding() -> ModelBinding {
    ModelBinding {
        provider_instance: "scripted-1".into(),
        provider_display_name: "Scripted".into(),
        vendor: VENDOR.into(),
        model: "script-model".into(),
        model_display_name: "Script Model".into(),
        // Unstated on purpose: the suite drives the compaction through its
        // two explicit doors, and the threshold arms are pinned at their own
        // policy function over injected numbers. A fixture that stated a
        // window would also start the threshold sweep, whose timer perturbs
        // every paused-clock test in this suite for no coverage gained.
        context_window: None,
    }
}

/// One test's assembled core, together with the wiring the test passed in.
/// The store and bus handles are the constructor's caller's own — the
/// assembly exposes no accessor for them, so a test reads the ledger and the
/// event order through the clones it kept.
pub struct Fixture {
    pub assistant: Arc<Assistant>,
    pub script: ScriptHandle,
    pub store: Store,
    pub bus: Arc<EventBus<CoreEvent>>,
}

impl Fixture {
    /// Stop this process's assembly and wait until the runtime has stopped
    /// it — what a restart test owes between its two processes, and what
    /// dropping the fixture does not do.
    ///
    /// A drop drops the handle, not the running assembly: the reactor and
    /// the per-conversation schedulers it spawned live on the test's
    /// runtime, and a scheduler wakes on the STORE's row changes, not on
    /// this bus. A dropped process therefore still sees the next process's
    /// message land in the store they share, still finds the turn owed,
    /// and can answer it from its own provider before the restarted
    /// process does. A restart test that gives each process a runtime of
    /// its own ([`process_runtime`]) and drops that runtime has no such
    /// race; a test restarting inside one runtime stops its first process
    /// here.
    ///
    /// The stop is the framework's own teardown edge, the interrupt the
    /// core's stream settle already stops a turn with: it latches the
    /// conversation — a latched delivery stands down, and only an append or
    /// an unlatch on THIS bus releases one, neither of which a stopped
    /// process can produce — closes the dispatch and drops the provider
    /// binding. The awaited signal is the teardown's own last act, the
    /// status block it appends once the rest of it has run: the latched
    /// state publishes earlier, so waiting on that would leave the append
    /// to land behind the next process's message.
    ///
    /// The assembly's own workers are stopped after the interrupts, through
    /// the same door the binary takes at the end of its run: the stopped
    /// process's compaction driver and retention sweep are ended and awaited,
    /// so neither can compact or retire against the store the next process
    /// opens.
    pub async fn shutdown(self) {
        let conversations = self
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads");
        for conversation in conversations {
            let before = status_blocks(
                &self
                    .store
                    .list_blocks(conversation.id)
                    .await
                    .expect("the ledger reads"),
            );
            self.bus.emit(CoreEvent::InterruptRequested {
                conversation_id: conversation.id,
            });
            await_ledger(
                &self.store,
                conversation.id,
                "the stopped process's teardown",
                |blocks| status_blocks(blocks) > before,
            )
            .await;
        }
        self.assistant.shutdown().await;
    }
}

/// The endings an assembly declares for its sends, read off the raw carrier
/// (unit 55): the observation a delivery case makes around a report — no
/// send was declared done before it, exactly one was after it.
///
/// Off the carrier and not off a composing edge, because an edge yields its
/// begin and its stop whether or not any send ever reported: the round's own
/// ending stops the signal too. Only this reading tells a missing stop apart
/// from the one the turn's end raised.
pub struct SendEndings(tokio::sync::broadcast::Receiver<i64>);

impl SendEndings {
    /// Start watching before the send goes out, so the whole window from
    /// the call to the report is covered.
    #[must_use]
    pub fn watching(assistant: &Assistant) -> Self {
        Self(assistant.finished_sends())
    }

    /// Nothing has declared a send done yet: the send is on its way.
    pub fn none_yet(&mut self) {
        assert!(
            self.0.try_recv().is_err(),
            "the send is on its way and nothing has declared it done yet"
        );
    }

    /// The next ending, which must be this conversation's — the stop the
    /// report just raised, named by what the case is proving.
    pub async fn one_for(&mut self, conversation_id: i64, what: &str) {
        assert_eq!(
            tokio::time::timeout(DEADLINE, self.0.recv())
                .await
                .expect("the send is declared done before the deadline")
                .expect("the carrier outlives the test"),
            conversation_id,
            "{what}"
        );
    }
}

/// How many status blocks one conversation's ledger holds — the count the
/// stop above compares against, the way the core's own stream settle
/// confirms an interrupt completed.
fn status_blocks(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .filter(|block| block.block_type == "status")
        .count()
}

/// Assemble a running assistant over a fresh in-memory store, under the
/// default budgets — which is also the standing proof that the defaults do
/// not limit the suite's ordinary traffic.
pub async fn start_assistant(hold: Option<Arc<TurnHold>>) -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    start_assistant_on(store, hold).await
}

/// Assemble a running assistant over the given store — a file-backed one
/// when a test proves the durable path — under the default budgets.
pub async fn start_assistant_on(store: Store, hold: Option<Arc<TurnHold>>) -> Fixture {
    start_assistant_configured(store, hold, ProtectionConfig::default()).await
}

/// Assemble a running assistant over the given store with the given
/// budgets, for the protection tests that pin a small window.
pub async fn start_assistant_configured(
    store: Store,
    hold: Option<Arc<TurnHold>>,
    protection: ProtectionConfig,
) -> Fixture {
    let (provider, script) = scripted_provider(hold);
    start_assistant_full(store, provider, script, production_toolset(), protection).await
}

/// Assemble a running assistant over the given provider and tool set — the
/// full seam the tool tests use, under the suite's default operator wiring.
pub async fn start_assistant_full(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    protection: ProtectionConfig,
) -> Fixture {
    start_assistant_operators(
        store,
        provider,
        script,
        tools,
        protection,
        operator_config(),
        None,
    )
    .await
}

/// The widest seam: everything spelled out, operator wiring and privacy
/// address included — for the group-context tests that pin a configured
/// privacy address or an absent operator.
pub async fn start_assistant_operators(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    protection: ProtectionConfig,
    operators: OperatorConfig,
    privacy_policy_address: Option<String>,
) -> Fixture {
    start_assistant_config(
        store,
        provider,
        script,
        tools,
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: binding(),
            system_prompt: SYSTEM_PROMPT.into(),
            answering: FIXTURE_ANSWERING,
            name: NAME.into(),
            disclosure: None,
            protection,
            operators,
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
}

/// Assemble a running assistant under the given answering mode, everything
/// else the suite's defaults — the seam of the helpful-mode pins.
pub async fn start_assistant_answering(
    store: Store,
    hold: Option<Arc<TurnHold>>,
    protection: ProtectionConfig,
    answering: assistant_core::AnsweringMode,
) -> Fixture {
    let (provider, script) = scripted_provider(hold);
    start_assistant_config(
        store,
        provider,
        script,
        production_toolset(),
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: binding(),
            system_prompt: SYSTEM_PROMPT.into(),
            answering,
            name: NAME.into(),
            disclosure: None,
            protection,
            operators: operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
}

/// The moderation handle every report fixture configures — the report
/// line's `/report@` suffix under test.
pub const MODERATION_HANDLE: &str = "moderation_fixture_bot";

/// Assemble a running assistant with the report tool registered: the
/// suite's [`MODERATION_HANDLE`] beside the given tool set, under helpful
/// answering — the two conditions the registration takes since unit 15 —
/// and the default operator wiring. The seam the report tests use.
pub async fn start_assistant_reporting(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    protection: ProtectionConfig,
) -> Fixture {
    start_assistant_reporting_as(
        store,
        provider,
        script,
        tools,
        protection,
        assistant_core::AnsweringMode::Helpful,
    )
    .await
}

/// The reporting seam with the answering mode spelled out — for the
/// gating pins that prove an addressed deployment registers no report
/// tool even with the handle configured.
pub async fn start_assistant_reporting_as(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    protection: ProtectionConfig,
    answering: assistant_core::AnsweringMode,
) -> Fixture {
    start_assistant_config(
        store,
        provider,
        script,
        tools,
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: binding(),
            system_prompt: SYSTEM_PROMPT.into(),
            answering,
            name: NAME.into(),
            disclosure: None,
            protection,
            operators: operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: Some(MODERATION_HANDLE.into()),
            web_search: None,
        },
    )
    .await
}

/// The search key every searching fixture configures. A fake, and the
/// string AC6's pins look for everywhere the assembly renders anything.
pub const SEARCH_KEY: &str = "FAKE-SEARCH-KEY-0123456789";

/// Assemble a running assistant with the web search configured against the
/// given vendor address — the tool the ASSEMBLY registers from that
/// configuration, so the fixture proves the one predicate rather than
/// admitting the tool by hand.
pub async fn start_assistant_searching(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    search_base_url: String,
) -> Fixture {
    start_assistant_config(
        store,
        provider,
        script,
        tools,
        assistant_core::AssemblyConfig {
            web_search: Some(assistant_core::tools::search::SearchConfig {
                base_url: search_base_url,
                api_key: SEARCH_KEY.into(),
                country: None,
                language: "en".into(),
            }),
            ..assembly_config()
        },
    )
    .await
}

/// The system prompt a searching fixture records: the suite's default
/// composition with the search teaching riding it.
#[must_use]
pub fn composed_searching_prompt() -> String {
    assistant_core::composed_system_prompt(
        SYSTEM_PROMPT,
        NAME,
        FIXTURE_ANSWERING,
        assistant_core::Capabilities {
            moderation_handle: false,
            web_search: true,
        },
    )
}

/// The suite's default assembly configuration whole — the fixture binding,
/// prompt, answering mode, name, budgets and operator wiring — for a test
/// that assembles by hand and varies nothing.
pub fn assembly_config() -> assistant_core::AssemblyConfig {
    assistant_core::AssemblyConfig {
        // Unstated on purpose, exactly as the binding leaves the context
        // window unstated: a fixture that configured a retention span would
        // start the sweep's hourly timer, which perturbs every paused-clock
        // test in this suite for no coverage gained. The retention module's
        // own suite drives the sweep, and the retention spine tests state
        // their span themselves.
        retention: assistant_core::RetentionConfig::disabled(),
        started_at: std::time::Instant::now(),
        reasoning: assistant_core::ReasoningLevel::Low,
        binding: binding(),
        system_prompt: SYSTEM_PROMPT.into(),
        answering: FIXTURE_ANSWERING,
        name: NAME.into(),
        disclosure: None,
        protection: ProtectionConfig::default(),
        operators: operator_config(),
        direct_chats: assistant_core::DirectChats::default(),
        privacy_policy_address: None,
        moderation_handle: None,
        web_search: None,
    }
}

/// The one assembly call every seam above funnels into.
pub async fn start_assistant_config(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    config: assistant_core::AssemblyConfig,
) -> Fixture {
    let fixture = start_assistant_unreported(store, provider, script, tools, config).await;
    spawn_delivery_reporter(&fixture.assistant).await;
    fixture
}

/// The same assembly with NOBODY reporting its deliveries — for a test that
/// reports the platform's answer itself, because what it is about is what a
/// send that failed, or one cut short, does to the call behind it.
///
/// Two reporters would each answer the same send from their own edge, and
/// the first resolution wins: a test scripting a failure needs to be the
/// only one answering.
pub async fn start_assistant_unreported(
    store: Store,
    provider: Box<dyn ProviderModule>,
    script: ScriptHandle,
    tools: ToolSet,
    config: assistant_core::AssemblyConfig,
) -> Fixture {
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Arc::new(
        Assistant::start(
            store.clone(),
            Arc::clone(&bus),
            registry_of(provider),
            tools,
            config,
        )
        .await
        .expect("the assembly starts"),
    );
    Fixture {
        assistant,
        script,
        store,
        bus,
    }
}

/// The platform ids the suite's stand-in adapter mints, in order.
static MINTED_DELIVERIES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The platform id the stand-in adapter minted for the nth message it sent,
/// counting from one — what a test names when it replies to one of the
/// assistant's own messages.
#[must_use]
pub fn minted_delivery(nth: usize) -> String {
    format!("hers-{nth}")
}

/// Spawn the suite's stand-in adapter: one outbound edge of its own,
/// minting a platform id per message it takes and reporting it back through
/// the receipt door.
///
/// It exists because a send is a PENDING call (unit 55): the sending tool
/// files the message and the delivery report is what settles the call, so a
/// suite with nobody reporting would leave every answered turn open on its
/// own send and the next turn undispatched behind it. A real deployment has
/// an adapter doing exactly this; the suite would otherwise be testing a
/// half-connected machine.
///
/// It takes an edge of ITS OWN, so a test's edge still sees every item: two
/// edges on one adapter each keep their own cursor and each deliver
/// everything. The test reads what the chat would show; this one does what
/// the chat would answer.
pub async fn spawn_delivery_reporter(assistant: &Arc<Assistant>) {
    let mut items = assistant
        .outbound(ADAPTER)
        .await
        .expect("the reporter's outbound edge opens");
    let reporting = Arc::clone(assistant);
    tokio::spawn(async move {
        while let Some(item) = items.recv().await {
            let Outbound::Reply(reply) = item else {
                continue;
            };
            let nth = MINTED_DELIVERIES.fetch_add(1, Ordering::SeqCst) + 1;
            reporting
                .report_delivery(
                    reply.delivery,
                    &[minted_delivery(nth)],
                    &assistant_core::SendOutcome::Whole,
                )
                .await;
        }
    });
}

/// The operator every test assembly is configured with, on the suite's
/// [`ADAPTER`]: the external id [`authorize`] admits groups as.
pub const OPERATOR: &str = "operator-1";

/// The suite's default operator wiring: the one operator on the suite's
/// adapter.
pub fn operator_config() -> OperatorConfig {
    OperatorConfig {
        by_adapter: std::collections::HashMap::from([(ADAPTER.to_owned(), OPERATOR.to_owned())]),
    }
}

/// One membership observation: the assistant added to the channel by the
/// sender of the given external id.
pub fn added_by(channel: &ChannelKey, external_id: &str) -> Observation {
    Observation {
        channel: channel.clone(),
        channel_kind: ChannelKind::Group,
        fact: ObservedFact::Added {
            by: Some(SenderIdentity {
                external_id: external_id.into(),
                username: None,
                bot: false,
            }),
        },
    }
}

/// A group channel admitted by the configured operator, in one visible
/// step: the admission every group test states where it names the channel,
/// so no test speaks in a group it was admitted to silently.
pub async fn authorized_group(assistant: &Assistant, id: &str) -> ChannelKey {
    let key = channel(id);
    authorize(assistant, &key).await;
    key
}

/// Admit one group channel as the configured operator — what a test does
/// before speaking in a group, mirroring the deployment where the operator
/// adds the assistant first.
pub async fn authorize(assistant: &Assistant, channel: &ChannelKey) {
    let outcome = assistant
        .observe(added_by(channel, OPERATOR))
        .await
        .expect("the membership observation is judged");
    assert_eq!(
        outcome,
        ObserveOutcome::Observed { deliver: None },
        "the operator's add admits the group"
    );
}

/// Record what one send put in the chat, the way an adapter does (unit 38,
/// 2026-08-30): the opaque handle the core handed out with the text, and
/// the platform's ids for the messages the send became.
///
/// The suite never mints a handle of its own — it takes the one the
/// outbound edge put on the reply, or the one the ingest receipt answers
/// for a deterministic item, which is exactly what the adapter's two send
/// paths do. A handle this suite could build itself would prove nothing
/// about the seam.
pub async fn report_delivery(assistant: &Assistant, delivery: DeliveryHandle, origins: &[&str]) {
    let origins: Vec<String> = origins.iter().map(|origin| (*origin).to_owned()).collect();
    assistant
        .report_delivery(delivery, &origins, &assistant_core::SendOutcome::Whole)
        .await;
}

/// The item one judged observation delivers, or `None` where it delivers
/// nothing.
///
/// Since unit 38 (2026-08-30) the item rides with the handle its send is
/// recorded under, and that handle is the core's own bookkeeping: every
/// assertion in this suite is about WHAT is delivered, so the reading is
/// spelled once here instead of at each call site.
pub fn observed_item(outcome: &ObserveOutcome) -> Option<&DeliveryItem> {
    match outcome {
        ObserveOutcome::Observed { deliver } => deliver.as_ref().map(|observed| &observed.item),
        ObserveOutcome::Withdraw => None,
    }
}

/// Ingest one message that must be recorded, and return its receipt — the
/// shape almost every test wants. Authorization is the caller's, spelled
/// out at every group call site with [`authorize`]: an implicit admission
/// here would blind the whole suite to authorization regressions. Never
/// routes a message the test expects refused.
pub async fn ingest_recorded(assistant: &Assistant, message: InboundMessage) -> IngestReceipt {
    match assistant
        .ingest(message)
        .await
        .expect("the message ingests")
    {
        assistant_core::IngestOutcome::Recorded { receipt, .. } => receipt,
        assistant_core::IngestOutcome::Withdraw => {
            panic!("the message was refused; the test channel is not authorized")
        }
        assistant_core::IngestOutcome::Disregarded => {
            panic!("the message was disregarded; the fixture serves no direct chats")
        }
    }
}

/// A loopback address nothing listens on: a tool constructed over it can be
/// registered — and its name recorded in the tool choice — without any test
/// traffic ever succeeding against it.
pub const UNROUTABLE: &str = "http://127.0.0.1:1";

/// The production tool set, exactly as the binary assembles it — the one
/// shared default the core defines — with the network pointed at the
/// unroutable loopback address, so a suite that never calls a tool cannot
/// generate traffic. Tests that execute a tool build their own set against
/// a scripted server.
pub fn production_toolset() -> ToolSet {
    ToolSet::production_lookups(assistant_core::tools::LookupEndpoints {
        forge: UNROUTABLE.into(),
        mirror: UNROUTABLE.into(),
        mirror_token: None,
        wiki: UNROUTABLE.into(),
        wiki_index: UNROUTABLE.into(),
    })
}

/// A protection configuration from two optional `(answers, window seconds)`
/// pairs, `None` disabling that budget.
pub fn budgets(principal: Option<(u32, u64)>, channel: Option<(u32, u64)>) -> ProtectionConfig {
    let budget = |pair: Option<(u32, u64)>| {
        pair.map(|(answers, window_seconds)| Budget {
            answers: answers.try_into().expect("a test budget is nonzero"),
            window_seconds: window_seconds.try_into().expect("a test window is nonzero"),
        })
    };
    ProtectionConfig {
        principal: budget(principal),
        channel: budget(channel),
    }
}

/// The receipt-time test seam (AC7): age every recorded block by rewriting
/// the header's creation time backwards, through the same domain seam the
/// counts read it with. The budgets anchor at the count's own wall clock,
/// so backdating the receipt times is how a test crosses a window without
/// the production path carrying a clock parameter nothing real supplies.
pub async fn age_receipts(store: &Store, seconds: i64) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        // The store writes `created_at` as fixed-width RFC 3339 —
        // twenty-three wall-clock characters with milliseconds, then a
        // six-character offset. Aging shifts the wall-clock part and
        // reattaches the same offset, so every aged row keeps the exact
        // encoding production writes: the release pins must observe the
        // window over the real stored shape, not over a seam artefact in
        // SQLite's own `datetime()` form.
        conn.execute(
            "UPDATE blocks SET created_at = \
             strftime('%Y-%m-%dT%H:%M:%f', substr(created_at, 1, 23), ?1) \
             || substr(created_at, 24)",
            [format!("-{seconds} seconds")],
        )?;
        Ok(())
    })
    .await
    .expect("the receipt times age");
}

/// The retention seam: age the blocks of ONE conversation by whole days,
/// through the same encoding rewrite [`age_receipts`] performs. The
/// retention sweep measures a conversation by its newest stored block, so a
/// case that ages one conversation and not its neighbour is how a span is
/// crossed without waiting for one.
///
/// Blocks a compaction shares between an ancestor and the thread standing on
/// it are reached by either conversation's name, which is what makes them
/// shared; a thread's OWN blocks keep their times.
pub async fn age_conversation_days(store: &Store, conversation_id: i64, days: i64) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        conn.execute(
            "UPDATE blocks SET created_at = \
             strftime('%Y-%m-%dT%H:%M:%f', substr(created_at, 1, 23), ?2) \
             || substr(created_at, 24) \
             WHERE id IN (\
               SELECT block_id FROM conversation_blocks WHERE conversation_id = ?1\
             )",
            rusqlite::params![conversation_id, format!("-{days} days")],
        )?;
        Ok(())
    })
    .await
    .expect("the conversation's blocks age");
}

/// The seam's other encoding move: re-express every stored receipt time at
/// UTC-05:00 while denoting the same instant. `SQLite` normalizes the stored
/// offset to UTC before applying the '-5 hours' shift, so the rewritten
/// wall clock plus the appended offset names exactly the time each row
/// already held — only the encoding changes. This lives beside
/// [`age_receipts`] so the header's time encoding has one writer to
/// re-point when the framework's stored shape changes.
pub async fn reencode_receipts_at_utc_minus_five(store: &Store) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
        conn.execute(
            "UPDATE blocks SET created_at = \
             strftime('%Y-%m-%dT%H:%M:%f', created_at, '-5 hours') || '-05:00'",
            [],
        )?;
        Ok(())
    })
    .await
    .expect("the receipt times re-encode");
}

/// The transient append fault, injected at the store: a temporary trigger
/// fails every INSERT into the named content table while every read stays
/// live — the seam behind the redelivered-after-transient-failure proofs,
/// which show no window or cap is spent before an append stands. The
/// framework's append is one transaction, so the failed content insert rolls
/// the header back with it and the ledger keeps no half-written block.
/// [`heal_appends`] removes the fault.
///
/// The fault is an arithmetic overflow and not an aborting trigger, because
/// the CLASS has to be the passing one: a statement a database refuses by a
/// RULE is fatal to this process (2026-09-02), and what these proofs are
/// about is the failure a redelivery gets past. An overflow is an ordinary
/// runtime error, so the caller reads exactly that class.
pub async fn sabotage_appends(store: &Store, table: &'static str) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        conn.execute_batch(&format!(
            "CREATE TRIGGER sabotage_{table} BEFORE INSERT ON {table} \
             BEGIN SELECT abs(-9223372036854775807 - 1); END;"
        ))?;
        Ok(())
    })
    .await
    .expect("the sabotage trigger installs");
}

/// Remove the injected append fault; the next append stands again.
pub async fn heal_appends(store: &Store, table: &'static str) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        conn.execute_batch(&format!("DROP TRIGGER sabotage_{table};"))?;
        Ok(())
    })
    .await
    .expect("the sabotage trigger drops");
}

/// The framework's recorded migration version for the assistant's domain.
/// This helper and its rewind below live beside [`age_receipts`] on
/// purpose: the suite's knowledge of the framework's schema — the `blocks`
/// header table the aging seam rewrites and the `domain_migrations` ledger
/// named here — has this module as its one owner, so a framework rename
/// lands in one place.
pub async fn domain_migration_version(store: &Store) -> i64 {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
        Ok(conn.query_row(
            "SELECT version FROM domain_migrations WHERE domain = ?1",
            [assistant_core::schema::DOMAIN],
            |row| row.get(0),
        )?)
    })
    .await
    .expect("the migration version reads")
}

/// The seam's write half: set the domain's recorded version back, as the
/// upgrade pin's rewind to the disk shape an earlier binary left behind.
pub async fn rewind_domain_migration_version(store: &Store, version: i64) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        conn.execute(
            "UPDATE domain_migrations SET version = ?1 WHERE domain = ?2",
            (version, assistant_core::schema::DOMAIN),
        )?;
        Ok(())
    })
    .await
    .expect("the migration version rewinds");
}

/// A provider registry holding exactly the given module.
pub fn registry_of(provider: Box<dyn ProviderModule>) -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers.register(provider);
    Arc::new(providers)
}

/// The adapter name every test channel lives on — the name a test hands
/// [`Assistant::outbound`] to take the edge that serves these channels.
pub const ADAPTER: &str = "test-adapter";

/// One addressed inbound message, member authority, built from a channel and
/// a sender. Addressed is the suite's default because most tests want the
/// message answered; the addressing tests use [`inbound_unaddressed`].
pub fn inbound(
    channel: &ChannelKey,
    kind: ChannelKind,
    sender_external_id: &str,
    text: &str,
) -> InboundMessage {
    inbound_as(channel, kind, sender_external_id, Authority::Member, text)
}

/// One addressed inbound message with the sender's standing spelled out, for
/// the tests that pin the stored authority per message.
pub fn inbound_as(
    channel: &ChannelKey,
    kind: ChannelKind,
    sender_external_id: &str,
    authority: Authority,
    text: &str,
) -> InboundMessage {
    InboundMessage {
        channel: channel.clone(),
        channel_kind: kind,
        sender: SenderIdentity {
            external_id: sender_external_id.into(),
            username: None,
            bot: false,
        },
        authority: Some(authority),
        addressed: true,
        reply_target: None,
        quoted: None,
        command: None,
        text: text.into(),
        origin: Some(format!(
            "origin-{sender_external_id}-{text_len}",
            text_len = text.len()
        )),
        revises: None,
        timestamp: chrono::Utc::now(),
    }
}

/// The same message, reported as a new version of the message under the
/// given id — the one extra fact an edit carries (unit T3, 2026-08-31).
/// The revision's own origin is set to the same id, which is what this
/// platform's edits do: an edit arrives under the original's message id.
pub fn revising(mut message: InboundMessage, revised: &str) -> InboundMessage {
    message.origin = Some(revised.into());
    message.revises = Some(revised.into());
    message
}

/// The same message, reported as a new version of the message under
/// `revised` while carrying an origin of its own — the shape a platform
/// that delivers an edit as its own event produces. Nothing in the core
/// knows which platform it is talking to, so both shapes must record and
/// resolve identically.
pub fn revising_under_own_origin(
    mut message: InboundMessage,
    revised: &str,
    origin: &str,
) -> InboundMessage {
    message.origin = Some(origin.into());
    message.revises = Some(revised.into());
    message
}

/// The same message, invoking the given command — the report the adapter
/// sends beside the verbatim text, which the core matches instead of the
/// text.
pub fn with_command(mut message: InboundMessage, command: &str) -> InboundMessage {
    message.command = Some(InvokedCommand::new(command));
    message
}

/// The same message, from a sender the platform gives the given public
/// username — the suite's default sender carries none, so the speaker pins
/// name their handles where they use them.
pub fn with_username(mut message: InboundMessage, username: &str) -> InboundMessage {
    message.sender.username = Some(username.into());
    message
}

/// The same message, from a BOT account — the platform's own fact about
/// the sending account, translated by the adapter onto the identity
/// (2026-08-30). The suite's default sender is a person, so a test that
/// wants the automated sender says so here.
pub fn from_a_bot(mut message: InboundMessage) -> InboundMessage {
    message.sender.bot = true;
    message
}

/// One unaddressed group reply carrying the moderation bot's deletion
/// command, aimed at the given stored origin — the shape that triggers the
/// mirror (unit 13). Shared, because two suites drive an erasure through
/// it: the mirror's own, and the walk pins that need an erased row in the
/// middle of a ledger.
pub fn deletion_reply(
    channel: &ChannelKey,
    sender: &str,
    authority: Authority,
    target: &str,
) -> InboundMessage {
    let mut message = inbound_as(
        channel,
        ChannelKind::Group,
        sender,
        authority,
        assistant_core::mirror::DELETION_COMMAND,
    );
    message.addressed = false;
    with_command(
        with_reply(
            message,
            assistant_core::ReplyTarget::Message {
                origin: target.into(),
            },
        ),
        assistant_core::mirror::DELETION_COMMAND,
    )
}

/// The same message, replying to the given target — the translated reply
/// fact the adapter delivers beside the addressed flag.
pub fn with_reply(
    mut message: InboundMessage,
    target: assistant_core::ReplyTarget,
) -> InboundMessage {
    message.reply_target = Some(target);
    message
}

/// The same message under an exact origin, for the tests that reply to it
/// by that origin later.
pub fn with_origin(mut message: InboundMessage, origin: &str) -> InboundMessage {
    message.origin = Some(origin.into());
    message
}

/// One unaddressed inbound message — recorded, resting, never unlatching.
pub fn inbound_unaddressed(
    channel: &ChannelKey,
    kind: ChannelKind,
    sender_external_id: &str,
    text: &str,
) -> InboundMessage {
    let mut message = inbound(channel, kind, sender_external_id, text);
    message.addressed = false;
    message
}

/// A channel key on the test adapter.
pub fn channel(id: &str) -> ChannelKey {
    ChannelKey {
        adapter: ADAPTER.into(),
        channel: id.into(),
    }
}

/// Poll the ledger until `accept` says it has the shape awaited, with a
/// deadline so a stall is a named failure instead of a hung suite.
pub async fn await_ledger(
    store: &Store,
    conversation_id: i64,
    what: &str,
    accept: impl Fn(&[Block]) -> bool,
) -> Vec<Block> {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let blocks = store
            .list_blocks(conversation_id)
            .await
            .expect("the ledger reads");
        if accept(&blocks) {
            return blocks;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {what}; ledger: {:?}",
            blocks
                .iter()
                .map(|b| format!(
                    "{}{}",
                    b.block_type,
                    optional_field(b, "name").map_or(String::new(), |n| format!("({n})"))
                ))
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The ledger as the consumer's own content: every block the assistant or a
/// member put there, and none of the framework's date records nor the
/// assistant's own delivery receipts.
///
/// The framework writes a `date_marker` on the first user-voiced append of
/// a day — its own calendar entry, ordered before the block that tripped
/// it, carrying no consumer content. It is a real ledger row and the
/// consumer must survive it, but a test that spells out what the assistant
/// recorded is asserting about consumer content, and the calendar shifting
/// that content by one row is not a behavior change. So the suite's counts,
/// kind sequences and positional reads all judge this view, in one place,
/// instead of each test carrying its own arithmetic about the framework's
/// records. The kind is named through the framework leaf's own `KINDS`,
/// never a literal here.
///
/// A delivery receipt (unit 38, 2026-08-30) is filtered for a second
/// reason on top of that one: it is bookkeeping no reader meets (decision
/// 0139), and it is appended AFTER the send it records, so its arrival is
/// ordered against nothing a test drives and a shape assertion holding it
/// would race the report rather than pin the ledger. Both suites read the
/// same view here, so a test that reports a delivery and then gates on
/// [`settle`] counts content alone, on either side of the adapter seam.
///
/// What the marker itself does — that it exists, where it sits, how often
/// it is written, and how it reaches the model — is pinned by the suite's
/// `date_marker` module, and what the receipts record by its `delivery`
/// module; both read the raw ledger on purpose.
/// A SEND's own machinery (unit 55, 2026-09-02) is filtered for the
/// receipt's reason, restated: the call the model made, the outgoing block
/// it filed and the resolution that settled it are the transport of ONE
/// message, exactly as the receipt is the record of it. The suites' counts
/// and positional reads are about what the conversation holds, and every
/// answered turn gaining three transport rows would have renumbered them
/// all while saying nothing new.
///
/// Filtered NARROWLY, by pairing: a call of a sending tool, the outgoing
/// block, and the resolution naming that call's own block. Every
/// other tool's call and result stay in the view, because a lookup, a
/// report and a reaction are things the turn DID and the suites pin them
/// there.
///
/// What a send records — the block, its columns, its call and its
/// settlement — is pinned by the suite's `sending` module, which reads the
/// raw ledger on purpose.
#[must_use]
pub fn consumer_view(blocks: &[Block]) -> Vec<Block> {
    let sending = sending_calls(blocks);
    blocks
        .iter()
        .filter(|block| {
            !DateMarker::KINDS.contains(&block.block_type.as_str())
                && block.block_type != assistant_core::delivery::DELIVERED_KIND
                && block.block_type != assistant_core::outgoing::OUTGOING_MESSAGE_KIND
                && !(block.block_type == "tool_call" && names_a_sending_tool(block))
                && !settles_a_send(block, &sending)
        })
        .cloned()
        .collect()
}

/// Whether one recorded call names a sending tool.
fn names_a_sending_tool(block: &Block) -> bool {
    optional_field(block, "name").is_some_and(|name| {
        name == assistant_core::tools::send::NAME || name == assistant_core::tools::reply::NAME
    })
}

/// Every sending call one ledger holds, by BLOCK id — the identity the
/// framework pairs a call and its answer by, since a provider's echo can
/// repeat across two calls of one round.
fn sending_calls(blocks: &[Block]) -> Vec<i64> {
    blocks
        .iter()
        .filter(|block| block.block_type == "tool_call" && names_a_sending_tool(block))
        .map(|block| block.id)
        .collect()
}

/// Whether one block is the resolution of one of the given calls — read
/// through the framework's own kinds, which carry the call block each
/// resolution names.
fn settles_a_send(block: &Block, sending: &[i64]) -> bool {
    match BlockKind::from_block(block) {
        BlockKind::ToolResult(result) => result.call_block_id,
        BlockKind::ToolError(error) => error.call_block_id,
        _ => None,
    }
    .is_some_and(|call| sending.contains(&call))
}

/// One text field of a loaded block, absent where the row holds none —
/// the pairing reads above take an absence as "not a send", never as an
/// empty string.
fn optional_field(block: &Block, name: &str) -> Option<String> {
    block
        .fields
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Poll one conversation until `accept` says its CONSUMER VIEW
/// ([`consumer_view`]) has the shape awaited, and return that view.
///
/// The one place the view meets the wait: `accept` judges the same rows the
/// caller then reads positionally, so a test can never wait on a raw ledger
/// and index into a filtered one. Every awaited read that spells out consumer
/// content goes through here instead of wrapping [`await_ledger`] by hand.
pub async fn viewed_ledger(
    store: &Store,
    conversation_id: i64,
    what: &str,
    accept: impl Fn(&[Block]) -> bool,
) -> Vec<Block> {
    consumer_view(
        &await_ledger(store, conversation_id, what, |blocks| {
            accept(&consumer_view(blocks))
        })
        .await,
    )
}

/// An acceptance closure for [`await_ledger`]: the turn has settled — the
/// consumer view ([`consumer_view`]) holds exactly `len` blocks and the
/// newest is a finalized `text`
/// answer. The length alone is not settledness: an open stream's in-flight
/// answer is a real `streaming` block that counts toward the length and is
/// replaced by the final `text` block at a new id, so a gate that only
/// counts can pass mid-turn and let the next ingest land as an absorbed
/// mid-turn message.
pub fn settled(len: usize) -> impl Fn(&[Block]) -> bool {
    move |blocks: &[Block]| {
        let blocks = consumer_view(blocks);
        blocks.len() == len && blocks.last().is_some_and(|b| b.block_type == "text")
    }
}

/// Await one conversation's settled turn — `len` blocks with the finalized
/// answer last, per [`settled`] — and return the consumer view of them, so
/// a caller's positional read counts the same rows the gate did.
///
/// [`settled`] filters the blocks it is handed, and [`viewed_ledger`] hands
/// it the view already: the second pass is a no-op kept on purpose, because
/// [`settled`] is a standalone predicate that must also hold against a raw
/// ledger a caller polls itself.
pub async fn settle(store: &Store, conversation_id: i64, what: &str, len: usize) -> Vec<Block> {
    viewed_ledger(store, conversation_id, what, settled(len)).await
}

/// Await one conversation's failed turn settling: its error signal, and then
/// the latched state the runtime publishes once the dispatch closed on that
/// error. Both halves are needed — a conversation publishes a latched state
/// before its first message too, so the latch alone would be satisfied by a
/// turn that has not run yet.
///
/// This is the sync point a test needs between a failed turn and the message
/// that re-engages it, whenever the failure produces no reply to wait on. A
/// write that overtook the latch would have its unlatch applied first and
/// leave the conversation latched with an answer owed, which reads as a
/// hang.
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

/// Await one conversation's settled turn by its exact block-type shape —
/// every stored type in the consumer view ([`consumer_view`]) matches
/// `shape` in order, newest block the finalized text — and return that
/// view.
pub async fn settle_shape(
    store: &Store,
    conversation_id: i64,
    what: &str,
    shape: &[&str],
) -> Vec<Block> {
    let expected: Vec<String> = shape.iter().map(|s| (*s).to_owned()).collect();
    viewed_ledger(store, conversation_id, what, |blocks| {
        blocks.len() == expected.len()
            && blocks
                .iter()
                .zip(&expected)
                .all(|(block, want)| &block.block_type == want)
            && blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
    })
    .await
}

/// The stored field of one block, as text. Panics on an absent field —
/// the callers assert stored values, so an absence is a test failure.
pub fn field(block: &Block, name: &str) -> String {
    block.fields[name].as_str().unwrap_or_default().to_owned()
}

/// The conversation's one block of the given kind.
pub fn only(blocks: &[Block], kind: &str) -> Block {
    let mut found = blocks.iter().filter(|block| block.block_type == kind);
    let block = found.next().unwrap_or_else(|| panic!("one {kind} block"));
    assert!(found.next().is_none(), "exactly one {kind} block");
    block.clone()
}

/// The tool names ONE recorded tool-choice block carries — the whole read
/// of the block's stored shape, so every suite that reads a choice reads
/// it the same way.
pub fn choice_names(block: &Block) -> Vec<String> {
    block.fields["names"]
        .as_array()
        .expect("the recorded names are a list")
        .iter()
        .map(|name| name.as_str().expect("a name is a string").to_owned())
        .collect()
}

/// The tool names of the conversation's newest recorded tool choice.
pub fn tool_choice_names(blocks: &[Block]) -> Vec<String> {
    let block = blocks
        .iter()
        .rev()
        .find(|block| block.block_type == "tool_choice")
        .expect("the conversation records a tool choice");
    choice_names(block)
}

/// The block kinds that mean a turn is still MID-FLIGHT — the machinery a
/// round writes between its start and its end.
///
/// A conversation whose newest block is one of these has work in the air:
/// the framework is running a call, a send is filed and unsettled, or a
/// resolution is waiting for the round it re-drives. Everything else — a
/// member's message, one of the assistant's committed texts, a note, a
/// report, a reaction — is a settled tail.
const IN_FLIGHT_KINDS: &[&str] = &[
    "tool_call",
    "tool_result",
    "tool_error",
    "streaming_text",
    "streaming_tool_call",
    "outgoing_message",
    assistant_core::delivery::DELIVERED_KIND,
];

/// The longest the edge below waits for a turn to finish before handing the
/// test what it already has. A bound and not a condition to meet: a test that
/// deliberately holds a stream open never settles, and it must still read its
/// item.
const SETTLE_WAIT: Duration = Duration::from_secs(3);

/// The suite's outbound edge: the core's own edge, with every item held
/// back until the turn that produced it has finished (unit 55,
/// 2026-09-02).
///
/// A send is a PENDING call, so an answered turn now spans two rounds — the
/// round that sends, and the round the delivery report re-drives. The
/// message reaches the chat in the FIRST of them, so a test reading a reply
/// and ingesting again straight away would land its next message inside the
/// still-open turn and have it absorbed. That is honest production
/// behaviour and useless as a fixture: it makes every ledger shape depend
/// on which of two tasks won.
///
/// So the edge waits for the conversation's machinery to settle before
/// handing an item over. A turn that sends twice hands over both, in order,
/// once the turn is done; a turn that never settles hands over anyway at
/// [`SETTLE_WAIT`], because a held stream is a shape the suite drives on
/// purpose.
pub async fn outbound(fixture: &Fixture) -> mpsc::UnboundedReceiver<Outbound> {
    outbound_of(&fixture.assistant, &fixture.store).await
}

/// The same edge for a test holding its assembly and its store apart.
pub async fn outbound_of(
    assistant: &Assistant,
    store: &Store,
) -> mpsc::UnboundedReceiver<Outbound> {
    let mut items = assistant
        .outbound(ADAPTER)
        .await
        .expect("the outbound edge opens");
    let (settled, receiver) = mpsc::unbounded_channel();
    let store = store.clone();
    tokio::spawn(async move {
        while let Some(item) = items.recv().await {
            await_settled(&store).await;
            if settled.send(item).is_err() {
                break;
            }
        }
    });
    receiver
}

/// Wait until no conversation in the store has a turn in the air, or until
/// [`SETTLE_WAIT`] runs out. A read failure ends the wait: this is a test
/// fixture's patience, not an assertion.
async fn await_settled(store: &Store) {
    let deadline = std::time::Instant::now() + SETTLE_WAIT;
    while std::time::Instant::now() < deadline {
        let Ok(conversations) = store.list_conversations().await else {
            return;
        };
        let mut in_flight = false;
        for conversation in conversations {
            let Ok(blocks) = store.list_blocks(conversation.id).await else {
                return;
            };
            if blocks
                .last()
                .is_some_and(|block| IN_FLIGHT_KINDS.contains(&block.block_type.as_str()))
            {
                in_flight = true;
                break;
            }
        }
        if !in_flight {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Every message one conversation SENT, in ledger order: the stored text of
/// its outgoing blocks (unit 55, 2026-09-02).
///
/// What a suite used to read off the answer block. The two are not the same
/// text any more and must not be confused: the assistant's committed `text`
/// block is the turn's private notes, and THIS is what the chat received —
/// the disclosure line included, since that line is composed into the
/// stored outgoing text before the send.
pub async fn sent_texts(store: &Store, conversation_id: i64) -> Vec<String> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter_map(|block| {
            match <assistant_core::kind::AssistantKind as agent_ledger::FromBlock>::from_block(
                block,
            ) {
                assistant_core::kind::AssistantKind::OutgoingMessage(sent) => sent.text,
                _ => None,
            }
        })
        .collect()
}

/// Every tool resolution one ledger holds, in order, EXCLUDING a SEND's
/// own (unit 55): a send's result is the transport reporting back, not one
/// of the turn's acts, and a case counting what the model's calls came to
/// is counting the acts.
///
/// Paired by the call's own block id, the way the framework pairs a call
/// with its answer.
#[must_use]
pub fn tool_outcomes(blocks: &[Block]) -> Vec<String> {
    let sending = sending_calls(blocks);
    blocks
        .iter()
        .filter(|block| !settles_a_send(block, &sending))
        .filter_map(resolution_sentence)
        .collect()
}

/// What one block says as a tool resolution, or `None` for a block that is
/// no resolution at all — the one reading of a resolution row, so the two
/// complementary views around it differ in their filter and in nothing else.
fn resolution_sentence(block: &Block) -> Option<String> {
    match block.block_type.as_str() {
        "tool_result" => optional_field(block, "content"),
        "tool_error" => optional_field(block, "error"),
        _ => None,
    }
}

/// What one ledger's SENDS came to, in order: the result or the error
/// paired with each sending call, as the model reads it — the complement of
/// [`tool_outcomes`], which leaves exactly these out.
///
/// Paired by the call's own block id through the same reading that pairing
/// serves there, so the two views of a ledger can never disagree about
/// which resolution belongs to a send.
#[must_use]
pub fn send_settlements(blocks: &[Block]) -> Vec<String> {
    let sending = sending_calls(blocks);
    blocks
        .iter()
        .filter(|block| settles_a_send(block, &sending))
        .filter_map(resolution_sentence)
        .collect()
}

/// Await the next item on the outbound edge, or name the stall.
pub async fn recv_outbound(outbound: &mut mpsc::UnboundedReceiver<Outbound>) -> Outbound {
    tokio::time::timeout(DEADLINE, outbound.recv())
        .await
        .expect("an outbound item arrives before the deadline")
        .expect("the outbound edge outlives the test")
}

/// Await the next outbound item as the REPLY it must be, or name the
/// stall. The edge carries two arms since unit 39, and a suite awaiting
/// words says so: a reaction arriving where prose was owed is a routing
/// defect, and it fails here instead of being quietly unwrapped.
pub async fn recv_reply(outbound: &mut mpsc::UnboundedReceiver<Outbound>) -> OutboundReply {
    match recv_outbound(outbound).await {
        Outbound::Reply(reply) => reply,
        Outbound::Mark(mark) => panic!("expected a reply on the edge, got a reaction: {mark:?}"),
    }
}

/// Await the next outbound item as the REACTION it must be, or name the
/// stall — the mirror of [`recv_reply`].
pub async fn recv_mark(outbound: &mut mpsc::UnboundedReceiver<Outbound>) -> OutboundMark {
    match recv_outbound(outbound).await {
        Outbound::Mark(mark) => mark,
        Outbound::Reply(reply) => panic!("expected a reaction on the edge, got a reply: {reply:?}"),
    }
}

/// The stored text of a committed answer block.
pub fn block_text(block: &Block, field: &str) -> String {
    block
        .fields
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
