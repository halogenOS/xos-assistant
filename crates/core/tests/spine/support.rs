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
    Assistant, Authority, Budget, ChannelKey, ChannelKind, InboundMessage, ModelBinding,
    OutboundReply, ProtectionConfig, SenderIdentity,
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

/// The system prompt every test assembly is started with — a fixed fixture
/// string the prompt-recording assertions match against.
pub const SYSTEM_PROMPT: &str = "You are the suite's scripted assistant fixture.";

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

/// What a test observes of the scripted provider: turn count, every turn
/// request's projected messages, and the failure script.
#[derive(Clone)]
pub struct ScriptHandle {
    pub turns: Arc<AtomicUsize>,
    pub seen: Arc<std::sync::Mutex<Vec<Vec<Message>>>>,
    /// How many upcoming turns fail with a scripted stream error before the
    /// script answers normally again.
    pub failures: Arc<AtomicUsize>,
}

impl ScriptHandle {
    /// Script the next `count` turns to fail with a stream error.
    pub fn fail_next_turns(&self, count: usize) {
        self.failures.store(count, Ordering::SeqCst);
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
/// every turn answers with [`answer_to`] the newest projected message's
/// text, every title derivation with [`TITLE`], deterministically, so
/// turn-count and block-by-block assertions stay exact.
pub fn scripted_provider(hold: Option<Arc<TurnHold>>) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = ScriptHandle {
        turns: Arc::new(AtomicUsize::new(0)),
        seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        failures: Arc::new(AtomicUsize::new(0)),
    };
    let script = handle.clone();
    let provider = provider_stub("Scripted", "answers from a script", move || {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        let turns = Arc::clone(&script.turns);
        let seen = Arc::clone(&script.seen);
        let failures = Arc::clone(&script.failures);
        let hold = hold.clone();
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
                // Connected mirrors the real wire's first stream event; the
                // runtime surfaces it as stream status, which is what the
                // assembly's stream observer keys on.
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
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
                let answer = answer_to(&messages.last().map(message_text).unwrap_or_default());
                let turn = turns.fetch_add(1, Ordering::SeqCst) + 1;
                seen.lock().unwrap().push(messages);
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                    text: answer,
                }));
                if let Some(hold) = &hold {
                    let _ = hold.started_tx.send(turn);
                    // The hold ends on the test's release or on the turn's
                    // teardown: an interrupt drops the request sender, and a
                    // scripted provider that kept the stream open past that
                    // would hold the settle protocol under test hostage.
                    tokio::select! {
                        permit = hold.permits.acquire() => {
                            permit.expect("the hold outlives the test").forget();
                        }
                        _ = requests.recv() => {
                            // An interrupt or a closed channel: end the turn
                            // without a message end, as a torn-down wire does.
                            break;
                        }
                    }
                }
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                    usage: agent_ledger::providers::Usage::default(),
                    stop_reason: StopReason::EndTurn,
                }));
            }
        });
        (request_tx, responses)
    });
    (provider, handle)
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
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let (provider, script) = scripted_provider(hold);
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        registry_of(provider),
        Arc::new(ToolRegistry::new()),
        binding(),
        SYSTEM_PROMPT.into(),
        protection,
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
/// [`Assistant::replies`] to take the edge that serves these channels.
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
            display_name: format!("Sender {sender_external_id}"),
            username: None,
        },
        authority,
        addressed: true,
        text: text.into(),
        origin: Some(format!(
            "origin-{sender_external_id}-{text_len}",
            text_len = text.len()
        )),
        timestamp: chrono::Utc::now(),
    }
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
