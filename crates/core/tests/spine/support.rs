//! Shared fixtures: the scripted provider, the assembly helpers, and the
//! ledger-polling helpers.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_ledger::providers::{
    BoxFuture, ContentPart as WirePart, Message, MessageContent, ModelInfo, ProviderRx, ProviderTx,
};
use agent_ledger::{
    Block, CoreEvent, EventBus, LlmError, ProviderModule, ProviderRegistry, ProviderRequest,
    ProviderResponse, StopReason, Store, StoreError, StreamEvent, ToolRegistry,
};
use assistant_core::schema::store_config;
use assistant_core::{
    Assistant, Authority, ChannelKey, ChannelKind, InboundMessage, ModelBinding, OutboundReply,
    SenderIdentity,
};
use serde_json::{Value, json};
use tokio::sync::{Semaphore, mpsc};

/// The scripted title every derivation streams.
pub const TITLE: &str = "A derived title";

/// The provider module's type id — the binding's vendor.
pub const VENDOR: &str = "scripted";

/// The marker that tells a title derivation from a turn. With no tools
/// registered in this unit, the request content is the one discriminator: the
/// framework's title request appends its title instruction to the
/// conversation's prose, a turn does not. The text mirrors the framework's
/// instruction; if that wording changes, this suite fails loudly on an
/// unanswered title instead of silently miscounting turns.
pub const TITLE_INSTRUCTION_MARK: &str = "Generate a concise title";

/// How long a poll or an awaited event may take before the test names a
/// stall.
pub const DEADLINE: Duration = Duration::from_secs(10);

/// The answer the script streams for a turn whose newest projected message
/// carries this text. Deriving the answer from the request is what lets a
/// test pin a reply's text and channel key together: two channels' answers
/// differ, so a reply bound to the wrong key cannot pass by carrying an
/// identical constant.
#[must_use]
pub fn answer_to(text: &str) -> String {
    format!("The scripted answer to: {text}")
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
}

/// What a test observes of the scripted provider: turn count and every turn
/// request's projected messages.
#[derive(Clone)]
pub struct ScriptHandle {
    pub turns: Arc<AtomicUsize>,
    pub seen: Arc<std::sync::Mutex<Vec<Vec<Message>>>>,
}

/// The scripted provider: answers every turn with [`answer_to`] the newest
/// projected message's text, every title derivation with [`TITLE`],
/// deterministically, so turn-count and block-by-block assertions stay
/// exact.
struct ScriptedChat {
    turns: Arc<AtomicUsize>,
    seen: Arc<std::sync::Mutex<Vec<Vec<Message>>>>,
    hold: Option<Arc<TurnHold>>,
}

/// Build the provider and the handle a test observes it through.
pub fn scripted_provider(hold: Option<Arc<TurnHold>>) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = ScriptHandle {
        turns: Arc::new(AtomicUsize::new(0)),
        seen: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let provider = ScriptedChat {
        turns: Arc::clone(&handle.turns),
        seen: Arc::clone(&handle.seen),
        hold,
    };
    (Box::new(provider), handle)
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
        let turns = Arc::clone(&self.turns);
        let seen = Arc::clone(&self.seen);
        let hold = self.hold.clone();
        tokio::spawn(async move {
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
                let answer = answer_to(&messages.last().map(message_text).unwrap_or_default());
                let turn = turns.fetch_add(1, Ordering::SeqCst) + 1;
                seen.lock().unwrap().push(messages);
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                    text: answer,
                }));
                if let Some(hold) = &hold {
                    let _ = hold.started_tx.send(turn);
                    hold.permits
                        .acquire()
                        .await
                        .expect("the hold outlives the test")
                        .forget();
                }
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                    usage: agent_ledger::providers::Usage::default(),
                    stop_reason: StopReason::EndTurn,
                }));
            }
        });
        (request_tx, responses)
    }
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
    }
}

/// One test's assembled core, together with the wiring the test passed in.
/// The store and bus handles are the constructor's caller's own — the
/// assembly exposes no accessor for them, so a test reads the ledger and the
/// event order through the clones it kept.
pub struct Fixture {
    pub assistant: Assistant,
    pub script: ScriptHandle,
    pub store: Store,
    pub bus: Arc<EventBus<CoreEvent>>,
}

/// Assemble a running assistant over a fresh in-memory store.
pub async fn start_assistant(hold: Option<Arc<TurnHold>>) -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    start_assistant_on(store, hold).await
}

/// Assemble a running assistant over the given store — a file-backed one
/// when a test proves the durable path.
pub async fn start_assistant_on(store: Store, hold: Option<Arc<TurnHold>>) -> Fixture {
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let (provider, script) = scripted_provider(hold);
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        registry_of(provider),
        Arc::new(ToolRegistry::new()),
        binding(),
    )
    .await
    .expect("the assembly starts");
    Fixture {
        assistant,
        script,
        store,
        bus,
    }
}

/// A provider registry holding exactly the given module.
pub fn registry_of(provider: Box<dyn ProviderModule>) -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers.register(provider);
    Arc::new(providers)
}

/// The adapter name every test channel lives on — the name a test hands
/// [`Assistant::replies`] to take the edge that serves these channels.
pub const ADAPTER: &str = "test-adapter";

/// One inbound message, member authority, built from a channel and a sender.
pub fn inbound(
    channel: &ChannelKey,
    kind: ChannelKind,
    sender_external_id: &str,
    text: &str,
) -> InboundMessage {
    inbound_as(channel, kind, sender_external_id, Authority::Member, text)
}

/// One inbound message with the sender's standing spelled out, for the tests
/// that pin the stored authority per message.
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
            display_name: format!("Sender {sender_external_id}"),
            username: None,
        },
        authority,
        text: text.into(),
        origin: Some(format!(
            "origin-{sender_external_id}-{text_len}",
            text_len = text.len()
        )),
        timestamp: chrono::Utc::now(),
    }
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
                .map(|b| b.block_type.as_str())
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// An acceptance closure for [`await_ledger`]: the turn has settled — the
/// ledger holds exactly `len` blocks and the newest is a finalized `text`
/// answer. The length alone is not settledness: an open stream's in-flight
/// answer is a real `streaming` block that counts toward the length and is
/// replaced by the final `text` block at a new id, so a gate that only
/// counts can pass mid-turn and let the next ingest land as an absorbed
/// mid-turn message.
pub fn settled(len: usize) -> impl Fn(&[Block]) -> bool {
    move |blocks: &[Block]| {
        blocks.len() == len && blocks.last().is_some_and(|b| b.block_type == "text")
    }
}

/// Await one conversation's settled turn — `len` blocks with the finalized
/// answer last, per [`settled`] — and return the blocks.
pub async fn settle(store: &Store, conversation_id: i64, what: &str, len: usize) -> Vec<Block> {
    await_ledger(store, conversation_id, what, settled(len)).await
}

/// Await the next outbound reply, or name the stall.
pub async fn recv_reply(replies: &mut mpsc::UnboundedReceiver<OutboundReply>) -> OutboundReply {
    tokio::time::timeout(DEADLINE, replies.recv())
        .await
        .expect("a reply arrives before the deadline")
        .expect("the outbound edge outlives the test")
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
