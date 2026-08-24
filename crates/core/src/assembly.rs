//! The core assembly: the runtime wiring, the ingestion entry point, the
//! outbound subscription and the erasure operation, in one place.
//!
//! The assembly is constructed with its runtime wiring — the store opened on
//! the assistant's configuration, the event bus, the provider and tool
//! registries — the model binding under which first-message conversation
//! creation happens, and the system prompt every new conversation is created
//! with. The entry point draws the binding and the prompt from here, never
//! from a message. The constructor's caller owns the store, the bus and the
//! registries it passed in; the adapter edges below are the only surface an
//! adapter touches.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::num::{NonZeroU32, NonZeroU64};
use std::pin::Pin;
use std::sync::Arc;

use agent_ledger::providers::ReasoningLevel;
use agent_ledger::store::{ProviderInstance, StoreTx};
use agent_ledger::{
    CoreEvent, EventBus, FromBlock, ProviderRegistry, Role, RuntimeContext, Store, spawn_reactor,
};
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::acknowledgment;
use crate::composing;
use crate::erasure::{self, ErasureOutcome};
use crate::error::CoreError;
use crate::kind::{self, AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage};
use crate::message::{
    Authority, ChannelKey, ChannelKind, ComposingUpdate, DeliveryItem, InboundMessage,
    IngestOutcome, IngestReceipt, Observation, ObserveOutcome, ObservedFact, OutboundReply,
};
use crate::note::{self, ContextNote, NoteTopic};
use crate::privacy::{PendingDeletions, PrivacyCommand, RightsCommand};
use crate::streams::StreamObserver;
use crate::tools::report::{self, ReportTool};
use crate::tools::rights::PrivacyTool;
use crate::tools::{ToolSet, palette, palette::TOOL_PALETTE_KIND, palette::ToolPalette};
use crate::window::{
    ACKNOWLEDGMENT_WINDOW, LineWindow, PRIVACY_REPLY_CAP, PRIVACY_REPLY_WINDOW, ReplyWindow,
};
use crate::{authorization, identity, mapping, mirror, outbound, privacy, streams};

/// The erasure fence, as the shared handle the report tool receives at its
/// construction: ingestions and the tool's filing hold it shared, an
/// erasure holds it exclusively, so a report cannot re-materialize an
/// origin an erasure just nulled.
pub(crate) type ErasureFence = Arc<RwLock<()>>;

/// The kinds the owing-tail walk reads through (widened 2026-08-23 from
/// notes exactly to the consumer's delivery and supersession kinds): each
/// one is appended by an independent path at an arbitrary moment, so a
/// debt behind a run of them still owes. The framework's other transparent
/// kinds — the turn-closure markers above all — stay a settled tail, per
/// the walk's contract on [`Assistant::owing_tail_debt`].
pub(crate) const DEBT_READ_THROUGH: &[&str] = &[
    note::CONTEXT_NOTE_KIND,
    TOOL_PALETTE_KIND,
    report::REPORT_KIND,
];

/// The most conversations the palette-reconciliation memory holds. Past
/// the cap the memory is cleared whole — the established memory-cap shape:
/// it only suppresses repeat comparisons, so losing it costs one bounded
/// palette read per conversation, while an unbounded set would grow with
/// every direct chat the process ever saw.
const PALETTE_MEMORY_CAP: usize = 4096;

/// The model every new conversation is created under: one provider instance
/// and one model, named by the assembly and never by a message.
#[derive(Debug, Clone)]
pub struct ModelBinding {
    /// The provider instance's id, as registered in the store.
    pub provider_instance: String,
    /// The name a human reads for that instance.
    pub provider_display_name: String,
    /// The provider module's type id — what resolves the instance to the
    /// registered module.
    pub vendor: String,
    /// The provider's own identifier for the model.
    pub model: String,
    /// The name a human reads for the model.
    pub model_display_name: String,
}

/// The answering budgets the entry point enforces at the write — the flood
/// protection of decision 0030. Protection limits answering, never
/// recording: an over-limit addressed message is still recorded, with the
/// refusing budget named in its limited fact, and only the debt the message
/// itself would open is refused — a propagated debt passes unchanged.
///
/// A disabled budget (`None`) admits every debt. The defaults are the
/// stated product knobs of decision 0035; the embedder's configuration file
/// overrides them.
#[derive(Debug, Clone)]
pub struct ProtectionConfig {
    /// The per-sender budget, counted globally across conversations — spend
    /// is global, so heavy direct-chat use and group use share one budget.
    pub principal: Option<Budget>,
    /// The per-conversation budget, counted across that conversation's
    /// senders.
    pub channel: Option<Budget>,
}

impl ProtectionConfig {
    /// The default principal budget: answers per window.
    pub const DEFAULT_PRINCIPAL_ANSWERS: u32 = 6;
    /// The default principal window, in seconds.
    pub const DEFAULT_PRINCIPAL_WINDOW_SECONDS: u64 = 600;
    /// The default channel budget: answers per window.
    pub const DEFAULT_CHANNEL_ANSWERS: u32 = 20;
    /// The default channel window, in seconds.
    pub const DEFAULT_CHANNEL_WINDOW_SECONDS: u64 = 600;
}

impl Default for ProtectionConfig {
    fn default() -> Self {
        Self {
            principal: Some(Budget {
                answers: NonZeroU32::new(Self::DEFAULT_PRINCIPAL_ANSWERS)
                    .expect("the stated default is nonzero"),
                window_seconds: NonZeroU64::new(Self::DEFAULT_PRINCIPAL_WINDOW_SECONDS)
                    .expect("the stated default is nonzero"),
            }),
            channel: Some(Budget {
                answers: NonZeroU32::new(Self::DEFAULT_CHANNEL_ANSWERS)
                    .expect("the stated default is nonzero"),
                window_seconds: NonZeroU64::new(Self::DEFAULT_CHANNEL_WINDOW_SECONDS)
                    .expect("the stated default is nonzero"),
            }),
        }
    }
}

/// One budget: how many debts may open per window. The answer count is
/// nonzero by type — an assistant configured to answer no one is a
/// misconfiguration the embedder's parse refuses, and this type makes the
/// refusal structural. Disabling a budget is the enclosing `Option`, never
/// a zero.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// How many debts may open inside one window.
    pub answers: NonZeroU32,
    /// How far back the count looks, by receipt time, in whole seconds —
    /// the window's one unit end to end, decided with this unit: the
    /// configuration file speaks seconds, this field stores them, and the
    /// count's SQL cutoff subtracts exactly them, so a sub-second window
    /// is unrepresentable and nothing is silently truncated. Nonzero by
    /// type: disabling a budget is the enclosing `Option`, never a zero
    /// here.
    pub window_seconds: NonZeroU64,
}

/// The operator wiring of the group-context unit (decided 2026-08-23): who
/// may admit the assistant into a group, per adapter. It comes from the
/// embedder's configuration and is absent by default — with no operator
/// configured, every group add fails closed.
#[derive(Debug, Clone, Default)]
pub struct OperatorConfig {
    /// The operator's adapter-scoped external id, keyed by adapter name. A
    /// membership observation authorizes a group only when its adder's
    /// external id matches this entry for the observation's adapter.
    pub by_adapter: HashMap<String, String>,
}

/// How the assistant decides which group messages summon a turn (unit 14,
/// 2026-08-23): the embedder's `answering` key. Helpful is the default —
/// the operator's stated economics: with prompt caching the marginal read
/// is cheap at the community's traffic, so every group message reaches the
/// model and the model decides whether to speak, abstaining through the
/// sentinel. A deployment that wants the quiet shape sets `addressed`.
/// The mode enters the machinery at exactly one place: the entry point's
/// summons resolution ahead of the write-time stamp — everything past the
/// stamp reads the stored summons fact and stays mode-free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AnsweringMode {
    /// Every group message summons a turn; the model decides whether to
    /// speak and abstains through the sentinel.
    #[default]
    Helpful,
    /// A group message summons a turn only when it addresses the
    /// assistant: a mention, a reply to its message, its name.
    Addressed,
}

/// Whether the assembly serves direct channels (decided 2026-08-23): the
/// embedder's one switch for the whole direct-chat surface. Off, the entry
/// point refuses a direct-channel inbound before anything is written — no
/// mapping, no principal row, no ledger block, no answer, no deterministic
/// reply — mirroring the unauthorized-group refusal's fail-closed shape.
/// The default is on, so the generic assembly behaves as it always has; a
/// deployment turns the switch off until its direct-chat feature set ships.
/// Group channels are untouched by the switch either way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DirectChats {
    /// Direct channels are served: mapped, recorded and answered as ever.
    #[default]
    On,
    /// Direct channels are refused fail-closed before any write.
    Off,
}

/// A scripted pause inside an entry point's read-then-write window — the
/// race seams' shared shape, mirroring the adapter's injectable sleep: a
/// suite pins a lock's serialization without racing the scheduler. Two
/// seams take it — the on-delta note read's, and the pre-lock suppression
/// read's. Production installs nothing and nothing pauses.
pub type ScriptedPause = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Everything the embedder decides about the assistant beyond its runtime
/// wiring: the model binding, the system prompt, the answering budgets, the
/// operator wiring and the privacy policy address — one carrier, so a later
/// policy knob joins here instead of growing the constructor.
#[derive(Debug, Clone)]
pub struct AssemblyConfig {
    /// The model every new conversation is created under.
    pub binding: ModelBinding,
    /// The reasoning-effort level every new conversation is created under
    /// (decided 2026-08-23). Set on the conversation at its creation, so the
    /// framework carries it into every provider request the conversation
    /// makes; a conversation created before the level existed keeps its
    /// deferring null.
    pub reasoning: ReasoningLevel,
    /// The embedder's prompt prose. The assembly records it with the
    /// composed identity and answering sections behind it
    /// ([`crate::teaching::composed_system_prompt`]); a conversation gets
    /// the composition current at its creation.
    pub system_prompt: String,
    /// How group messages summon a turn: helpful by default, addressed for
    /// the quiet shape.
    pub answering: AnsweringMode,
    /// The assistant's resolved name — the configured `name` key, or the
    /// display name the embedder read from the platform at startup. Already
    /// trimmed and non-empty by the embedder's validation; it feeds the
    /// prompt identity and the disclosure fill.
    pub name: String,
    /// The disclosure line override; absent composes the line from the
    /// name. Already trimmed and non-empty by the embedder's validation.
    pub disclosure: Option<String>,
    /// The answering budgets the stamp consults for summoned messages.
    pub protection: ProtectionConfig,
    /// The operator wiring: who may admit the assistant into a group.
    pub operators: OperatorConfig,
    /// Whether direct channels are served at all; off refuses them
    /// fail-closed before any write.
    pub direct_chats: DirectChats,
    /// The address the privacy command answers with; absent answers the
    /// not-yet-published line.
    pub privacy_policy_address: Option<String>,
    /// The moderation bot's handle, already trimmed with its leading `@`
    /// stripped by the configuration layer, which also refuses an empty
    /// one. Present UNDER HELPFUL ANSWERING, the assembly registers the
    /// report tool against it and the composed prompt teaches the
    /// autonomous assessment (unit 15, 2026-08-24; the shared predicate is
    /// [`crate::teaching::moderation_taught`]); absent, or under addressed
    /// answering, the report tool does not register — only helpful mode
    /// shows the model every message it would judge — and the
    /// palette-delta mechanism removes it from conversations that had it.
    /// One global handle: one deployment serves one community (decided
    /// 2026-08-23).
    pub moderation_handle: Option<String>,
}

/// The running core: the framework runtime spawned over the assistant's
/// composed kind, plus the assistant's two edges and the erasure operation.
pub struct Assistant {
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    binding: ModelBinding,
    /// The reasoning-effort level every new conversation is set to at its
    /// creation. Read-only after start.
    reasoning: ReasoningLevel,
    /// The system prompt recorded into every conversation at its creation,
    /// through the framework's system-prompt kind — already composed with
    /// the identity and answering sections. The framework records a
    /// conversation's prompt exactly once, so an edited prompt, name or
    /// mode reaches new conversations only.
    system_prompt: String,
    /// How group messages summon a turn. Read-only after start; consulted
    /// at exactly one place, the ingest entry point's summons resolution.
    answering: AnsweringMode,
    /// The assistant's resolved name, kept past the prompt and disclosure
    /// compositions because the rules acknowledgment's one-shot instruction
    /// speaks in it (unit 20). Read-only after start.
    name: String,
    /// The resolved first-interaction disclosure, handed to every outbound
    /// edge. Read-only after start.
    disclosure: Arc<crate::disclosure::Disclosure>,
    /// The tool names every new conversation's palette block records —
    /// exactly the set the assembly registered, derived from the one
    /// [`ToolSet`] so the palette and the registry cannot name different
    /// tools. Written at creation beside the system prompt; a conversation
    /// created before the palette existed carries no block and admits
    /// nothing.
    palette: Vec<String>,
    /// The streaming state the erasure ordering reads; the observation's
    /// contract and its lossy edges are stated on the streams module.
    streams: Arc<StreamObserver>,
    /// The answering budgets the stamp consults for summoned messages.
    /// Read-only after start; the budget counts themselves are derived
    /// from the ledger at every write — the reply windows below are the
    /// one in-memory tally, and they bound courtesy lines, never budgets.
    protection: ProtectionConfig,
    /// The operator wiring: who may admit the assistant into a group.
    /// Read-only after start.
    operators: OperatorConfig,
    /// Whether direct channels are served at all. Read-only after start.
    direct_chats: DirectChats,
    /// The address the privacy command answers with; absent answers the
    /// not-yet-published line. Read-only after start.
    privacy_policy_address: Option<String>,
    /// The privacy notice's per-channel window (refined 2026-08-23): the
    /// command stamp keeps the notice out of both budget counts, so
    /// without this a quiet channel would answer every repeat. The notice
    /// alone rides here — the four rights commands are bounded per person
    /// by [`Self::privacy_replies`], and the rules acknowledgment carries
    /// no window at all since 2026-08-23: pinning is an admin-only right,
    /// so its on-delta comparison is the whole admission check.
    notice_answered: LineWindow,
    /// The rights replies' per-person bound (decided 2026-08-23): the four
    /// self-service commands and the privacy tool draw at most the cap per
    /// principal per window, shared with the tool so one person's flood is
    /// one bound. The budgets never gate the family; this window is the
    /// whole bound, and a withheld reply withholds its state change too.
    privacy_replies: Arc<ReplyWindow>,
    /// The pending deletion confirmations, keyed by principal and shared
    /// with the privacy tool: `/privacydelete` and the tool's
    /// `request_deletion` file here, `/confirmdelete` consumes here. Process
    /// memory, forgotten on restart — deletion is the flow where forgetting
    /// errs safe.
    pending_deletions: Arc<PendingDeletions>,
    /// The observation race's test seam; `None` in production.
    note_read_pause: Option<ScriptedPause>,
    /// The suppression race's test seam, run between the pre-lock standing
    /// read and the stamp lock; `None` in production.
    standing_read_pause: Option<ScriptedPause>,
    /// The conversations whose stored palette this process already
    /// compared against the registered set — the once-per-process memory
    /// of the on-delta supersession (decided 2026-08-23), bounded by
    /// [`PALETTE_MEMORY_CAP`] and cleared whole at the cap. Guarded by the
    /// stamp lock's serialization: every reader holds it.
    palette_reconciled: Mutex<HashSet<i64>>,
    /// Serializes the answer-due stamp against other ingestions: the tail
    /// read and the append it feeds are a read-then-write, and two
    /// concurrent ingestions into one conversation could both observe the
    /// pre-append tail — the later, unaddressed write would then be stamped
    /// false, cancelling exactly the owed answer decision 0021 exists to
    /// protect. Ingestion against ingestion is all this lock orders: the
    /// runtime commits its answer blocks outside it, so a tail read can see
    /// an answer-due tail whose answer commits a moment later, and the
    /// stamp then summons one extra turn over an already-answered tail —
    /// never a lost answer. One lock across all conversations, because
    /// ingestion runs at chat scale and a per-conversation lock map would
    /// buy contention nobody has.
    stamp_lock: Mutex<()>,
    /// Orders erasure against ingestion. An erasure reads, nulls and deletes
    /// in several store operations, and an ingestion interleaved between
    /// them could record a new message or map a new direct channel for the
    /// person being erased, leaving personal data behind after the erasure
    /// returns. Ingestions hold this shared, an erasure holds it
    /// exclusively, so the erasure's steps see no half-recorded message.
    /// Shared as a handle (2026-08-23) because the report tool holds it
    /// across its resolution and its append, under the same reasoning.
    erasure_fence: ErasureFence,
}

impl Assistant {
    /// Assemble and start the core: check the wiring, record the binding's
    /// provider instance in the store, and spawn the runtime over the given
    /// registries.
    ///
    /// # Errors
    ///
    /// [`CoreError::MissingContentTable`] if the store's effective
    /// content-table list lacks the message kind's table — the store was
    /// opened without [`crate::schema::store_config`], and every append
    /// would fail later and further from the cause.
    /// [`CoreError::UnknownVendor`] if no registered provider module answers
    /// to the binding's vendor — the vendor is what resolves a conversation
    /// to its module, so a mismatch accepted here would silently strand
    /// every conversation. [`CoreError::Store`] if recording the provider
    /// instance fails.
    pub async fn start(
        store: Store,
        bus: Arc<EventBus<CoreEvent>>,
        providers: Arc<ProviderRegistry>,
        tools: ToolSet,
        config: AssemblyConfig,
    ) -> Result<Self, CoreError> {
        let AssemblyConfig {
            binding,
            reasoning,
            system_prompt,
            answering,
            name,
            disclosure,
            protection,
            operators,
            direct_chats,
            privacy_policy_address,
            moderation_handle,
        } = config;
        // The two configured-value compositions, resolved once: the prompt
        // every new conversation records — the moderation teaching riding
        // it exactly when the tool below registers — and the disclosure
        // every outbound edge introduces with.
        let system_prompt = crate::teaching::composed_system_prompt(
            &system_prompt,
            &name,
            answering,
            moderation_handle.is_some(),
        );
        let disclosure = Arc::new(crate::disclosure::Disclosure::resolve(
            disclosure.as_deref(),
            &name,
        ));
        if !store.content_tables().contains(&CHAT_MESSAGE_TABLE) {
            return Err(CoreError::MissingContentTable {
                table: CHAT_MESSAGE_TABLE,
            });
        }
        if providers.get(&binding.vendor).is_none() {
            return Err(CoreError::UnknownVendor {
                vendor: binding.vendor,
            });
        }
        let erasure_fence: ErasureFence = Arc::new(RwLock::new(()));
        let privacy_replies = Arc::new(ReplyWindow::new(PRIVACY_REPLY_WINDOW, PRIVACY_REPLY_CAP));
        let pending_deletions = Arc::new(PendingDeletions::new());
        // The report tool joins the set here, where the tool set's assembly
        // finishes: the erasure fence is injected at this registration, so
        // the tool never reaches into the assembly. Registration takes the
        // same predicate the prompt composition took above — a handle AND
        // helpful answering (unit 15): without both, the tool is absent,
        // the palette derived below names no report tool, and the delta
        // mechanism removes it from conversations that had it — so the
        // prompt never teaches a tool the palette does not carry.
        let mut tools = tools;
        if let Some(handle) = moderation_handle
            && crate::teaching::moderation_taught(true, answering)
        {
            tools.admit(
                report::REQUIRED_AUTHORITY,
                ReportTool::new(handle, Arc::clone(&erasure_fence)),
            );
        }
        // The privacy tool joins unconditionally (decided 2026-08-23): the
        // rights it reaches exist in every deployment, so no configuration
        // switches it. The pending memory and the reply bound are shared
        // with the command family's own handling, injected here so the tool
        // and the commands act on one state.
        tools.admit(
            crate::tools::rights::REQUIRED_AUTHORITY,
            PrivacyTool::new(
                Arc::clone(&pending_deletions),
                Arc::clone(&privacy_replies),
                Arc::clone(&erasure_fence),
            ),
        );
        // One source for what tools exist: the registry the runtime resolves
        // calls against and the palette every new conversation records are
        // both derived from the set right here.
        let (registry, palette) = tools.into_registry();
        // Title derivation is switched off for good (decision 0077): nobody
        // reads a group chat's derived title, so no conversation excerpt is
        // ever sent anywhere for naming — zero title requests by
        // construction, not by configuration.
        let ctx: RuntimeContext<AssistantKind, CoreEvent> =
            RuntimeContext::new(store, bus, providers, Arc::new(registry))
                .without_title_derivation();
        ctx.store()
            .save_provider_instance(ProviderInstance {
                id: binding.provider_instance.clone(),
                provider_type: binding.vendor.clone(),
                name: binding.provider_display_name.clone(),
            })
            .await?;
        let streams = streams::spawn_observer(ctx.bus());
        spawn_reactor(ctx.clone());
        Ok(Self {
            ctx,
            binding,
            reasoning,
            system_prompt,
            answering,
            name,
            disclosure,
            palette,
            streams,
            protection,
            operators,
            direct_chats,
            privacy_policy_address,
            notice_answered: LineWindow::new(ACKNOWLEDGMENT_WINDOW),
            privacy_replies,
            pending_deletions,
            note_read_pause: None,
            standing_read_pause: None,
            palette_reconciled: Mutex::new(HashSet::new()),
            stamp_lock: Mutex::new(()),
            erasure_fence,
        })
    }

    /// Install the observation race's test seam: the given pause runs
    /// between the on-delta newest-note read and its append, inside the
    /// stamp lock — which is exactly why a suite can prove the lock holds
    /// the read-then-append window. Production never calls this.
    pub fn pause_between_note_read_and_append(&mut self, pause: ScriptedPause) {
        self.note_read_pause = Some(pause);
    }

    /// Install the suppression race's test seam: the given pause runs
    /// between the pre-lock standing read and the stamp lock, which is
    /// exactly the window a peer ingestion's flag write can land in — so a
    /// suite proves the under-lock re-read drops the racing message.
    /// Production never calls this.
    pub fn pause_between_standing_read_and_append(&mut self, pause: ScriptedPause) {
        self.standing_read_pause = Some(pause);
    }

    /// The ingestion edge: record one inbound message and answer with what
    /// the write decided — the resolved ids, and any deterministic item the
    /// adapter carries out.
    ///
    /// A group message is admitted first: a group channel with no
    /// authorization row is refused fail-closed with
    /// [`IngestOutcome::Withdraw`], touching neither the ledger nor the
    /// identity tables — the row-or-refusal shape is what makes the check
    /// survive lost leave calls and restarts alike. Direct channels are
    /// untouched by the check; they carry their own admission instead
    /// (decided 2026-08-23): with the assembly's [`DirectChats`] switch
    /// off, a direct-channel inbound is refused the same fail-closed way
    /// with [`IngestOutcome::Disregarded`] — nothing written, nothing
    /// delivered, no directive to perform. On, the default, direct
    /// channels pass exactly as they always have.
    ///
    /// An admitted message resolves or creates the sender's principal, maps
    /// the channel — creating the conversation under the assembly's
    /// binding, with the assembly's system prompt recorded first, on first
    /// message — stamps the message, and appends the message block through
    /// the framework's consumer write path. The stamp order is fixed:
    /// the summons first — the adapter's addressed fact, or helpful
    /// answering's every-message evaluation (unit 14, 2026-08-23),
    /// resolved here once and stored as one fact; the privacy command —
    /// and an administrator's
    /// mirrored deletion command (2026-08-23, the deletion mirror) —
    /// stamped with the command kind
    /// ahead of any budget — a command takes no debt by its nature; budgets
    /// consulted only for summoned non-command messages, principal before
    /// channel, the first refusing budget naming the limited fact — under
    /// helpful answering a rate-limited member's message therefore opens
    /// no turn and costs no model read, the free quiet of the existing
    /// mechanism; then
    /// answer-due by the composition rule — due when the message's own debt
    /// was taken (summoned, not limited) or when the tail owes, so a
    /// refused sender's message can be denied its own answer but can never
    /// cancel a debt it merely propagates; then the debt authority by the
    /// minimum rule. Nothing past admission ever drops a message:
    /// protection limits answering, never recording.
    ///
    /// Authority is read only past admission (refined 2026-08-23): an
    /// unadmitted group is refused with the withdraw directive before this
    /// method ever looks at the sender's standing, and an admitted message
    /// whose authority arrived unresolved is refused with the transient
    /// [`CoreError::AuthorityUnresolved`] — never recorded with a default.
    ///
    /// The ingestion ordering is stated whole (2026-08-23): channel-kind
    /// mismatch; group authorization; the direct-chat admission; then the
    /// READ-ONLY identity lookup by adapter and external id, which yields
    /// the standing suppression flag if any; then the privacy command
    /// family, exempt from suppression; then, flag standing, the drop —
    /// [`IngestOutcome::Disregarded`], the full no-write claim. Only past
    /// all of that does the writing resolution run, the channel map, the
    /// stamp lock get taken, the palette reconcile — and under the stamp
    /// lock the standing is read once more for a non-command message
    /// (2026-08-23), because a peer ingestion's flag write is serialized
    /// under that very lock and can land after the pre-lock read: the
    /// re-read drops the racing message before its append, so the flag
    /// suppresses from the moment it stands. Past the re-read runs the
    /// deletion mirror (2026-08-23) — behind the suppression drop and the
    /// direct-channel admission by this very order, ahead of the tail read
    /// so the stamp sees the post-mirror world; its trigger and its
    /// silence are the mirror module's contract. Opt-out does not
    /// reach backward: what was stored before stands until deletion, keeps
    /// being projected to the model with later turns, and a pre-flag
    /// unanswered question may still draw its one answer through a later
    /// propagated debt.
    ///
    /// The privacy command's fixed answer rides the returned outcome — the
    /// return-value transport of decision 2026-08-23, never the event edge
    /// — and is granted only while no budget refuses the sender AND the
    /// channel's answer window admits it: the command shares the
    /// acknowledgment-window mechanism (refined 2026-08-23), at most one
    /// answer per channel per window, recorded silence within it — the
    /// notice discipline of the protection unit. The four self-service
    /// commands answer differently (2026-08-23): never budget-consulted —
    /// a rights request is answered even from a sender the flood budgets
    /// have silenced — and bounded per PRINCIPAL by their own reply
    /// window, with the state change applied exactly when the reply is
    /// granted.
    ///
    /// A message whose own debt was taken then emits the unlatch intent,
    /// always: a summons IS the deliberate re-engagement — a person
    /// addressing the assistant, or under helpful answering any message
    /// the mode summons, since the mode's whole point is that every
    /// message may draw a turn — the intent is idempotent, and the same
    /// emission releases a fresh conversation's boot latch and a stream
    /// error's re-latch alike. An unsummoned message never unlatches, and
    /// neither does a limited or command-stamped one — neither is
    /// re-engagement.
    ///
    /// # Errors
    ///
    /// [`CoreError::ChannelKindMismatch`] if the channel is already mapped
    /// under a different kind than the message claims.
    /// [`CoreError::ClaimLost`] if a first-message claim lost its mapping
    /// row mid-claim. [`CoreError::Store`] if identity resolution, mapping
    /// or the append fails.
    pub async fn ingest(&self, message: InboundMessage) -> Result<IngestOutcome, CoreError> {
        let _no_erasure_mid_message = self.erasure_fence.read().await;
        let tx = self.ctx.store().tx();

        // The channel admission runs whole before the sender is looked at;
        // its checks and their order are on [`Self::admit_channel`].
        let (mapped, refused) = self.admit_channel(&tx, &message).await?;
        if let Some(refusal) = refused {
            return Ok(refusal);
        }
        let Some(sender) = self.resolve_writing_sender(&tx, &message).await? else {
            return Ok(IngestOutcome::Disregarded);
        };
        let WritingSender {
            principal_id,
            authority,
            family,
            suppressed,
        } = sender;
        if let Some(pause) = &self.standing_read_pause {
            pause().await;
        }

        let conversation_id = match mapped {
            Some((existing, _)) => existing,
            None => {
                self.map_new_channel(&message.channel, message.channel_kind)
                    .await?
            }
        };

        // Held from the tail read and the budget counts through the append:
        // the stamp is decided against the tail this write is appended
        // behind, and the counts must see every earlier taken debt — so no
        // concurrent ingestion may slide a block in between, and two racing
        // messages cannot both take the last budget slot. The lock's
        // contract is on its field.
        let _one_stamp_at_a_time = self.stamp_lock.lock().await;
        // The under-lock re-read closes the suppression race; its whole
        // reasoning is on [`Self::suppressed_under_lock`]. The command
        // family stays exempt, exactly as before the lock.
        if family.is_none() && self.suppressed_under_lock(&tx, &message).await? {
            return Ok(IngestOutcome::Disregarded);
        }
        // The palette supersession, on the conversation's first activity
        // per process (decided 2026-08-23): the delta append lands ahead of
        // the message, so this very turn's admission reads the fresh
        // palette.
        self.reconcile_palette(conversation_id).await?;
        // The deletion mirror (decided 2026-08-23), past the suppression
        // and channel admissions on purpose and ahead of the tail read on
        // purpose: an administrator's reply carrying the moderation bot's
        // own deletion command nulls the named row here, inline under this
        // ingestion's erasure-fence read hold — one row's nulls, not the
        // person-wide operation, so no spawn is needed and no deadlock
        // shape exists — and the stamp below is then decided against the
        // post-mirror tail: a debt the deleted message itself owed dies
        // with its text, exactly as the shared owes-answer reading already
        // cancels an erased debt, while a debt the deleted row merely
        // carried reads through to the live ask behind it, and a debt
        // carried by any other row still propagates (decision 0086).
        // Silent throughout: the admin addressed the
        // moderation bot, and the command row appended below is the lawful
        // record of the request.
        let mirrored = mirror::mirrored_target(&message, authority);
        if let Some(target) = mirrored {
            self.mirror_named_deletion(&tx, conversation_id, target)
                .await?;
        }
        let owing_tail = self.owing_tail_debt(conversation_id).await?;
        let summons = self.resolved_summons(&message);
        let limited = if family.is_some() || mirrored.is_some() {
            Some(kind::LimitedBy::Command)
        } else if summons.summoned {
            self.refusing_budget(principal_id, conversation_id).await?
        } else {
            None
        };
        // The composition rule and the minimum rule live on the kind, as
        // one pure composition beside the stamp's readers.
        let stamp = kind::Stamp::compose(summons, authority, limited, owing_tail);
        // The notice command's budget half is decided inside the stamp
        // serialization, so the consultation it shares with the stamp sees
        // the same recorded history: answered only while every budget
        // admits the sender — recorded silence otherwise. The notice alone
        // consults budgets: the four self-service commands are a rights
        // mechanism, answered even from a sender the flood budgets have
        // silenced, and bounded by their own per-person window instead.
        let notice_admitted = family == Some(PrivacyCommand::Notice)
            && self
                .refusing_budget(principal_id, conversation_id)
                .await?
                .is_none();
        // The speaker is the sender's public username as delivered at this
        // receipt — the handle as it was when the person spoke (decision
        // 0065). A handleless sender stores NULL and projects bare — no
        // substitute identifier is minted (decision 0056) — and the kind's
        // storable-speaker bound refuses a handle whose shape would blur
        // the projected prefix. A suppressed sender's exempt command
        // records no speaker at all: the freeze covers the delivered
        // handle too, so after a deletion no command re-materializes the
        // field the erasure emptied (decided 2026-08-23).
        let fields = ChatMessage::stored_fields(
            &message.text,
            kind::RecordedSender {
                principal_id,
                authority,
                speaker: if suppressed {
                    None
                } else {
                    message.sender.username.as_deref()
                },
            },
            message.origin.as_deref(),
            message.reply_target.as_ref(),
            &message.timestamp.to_rfc3339(),
            stamp,
        );
        self.ctx
            .store()
            .append_consumer_block(
                conversation_id,
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                fields,
                None,
            )
            .await?;
        // The windows are consulted last, after the append stands: a
        // budget-refused notice never spends its channel window, a
        // self-service command whose append failed transiently spends no
        // per-person slot — a grant spent before the append would answer
        // the redelivered command with silence — and a self-service state
        // change applies exactly when its reply is granted, never silently.
        // The split on the recognized kind is total: the notice keeps its
        // channel-keyed answer, the rights commands take the per-person
        // reply path.
        let deliver = match family {
            Some(PrivacyCommand::Notice) if notice_admitted => {
                self.notice_answer(conversation_id).await
            }
            Some(PrivacyCommand::SelfService(command)) => {
                self.rights_reply(&tx, command, principal_id).await
            }
            Some(PrivacyCommand::Notice) | None => None,
        };
        if stamp.own_debt_taken() {
            self.ctx
                .bus()
                .emit(CoreEvent::UnlatchRequested { conversation_id });
        }
        Ok(IngestOutcome::Recorded {
            receipt: IngestReceipt {
                principal_id,
                conversation_id,
            },
            deliver,
        })
    }

    /// The observation edge: judge one platform-neutral observation and
    /// answer with what was decided — the fail-closed refusal, or the
    /// appended note's acknowledgment. Everything deterministic rides this
    /// return; the event edge stays the model's answers and the failure
    /// notice (decided 2026-08-23).
    ///
    /// A membership observation is the authorization table's one writer: an
    /// add whose adder matches the configured operator records the group's
    /// admission and stands across restarts. A foreign adder, a missing
    /// adder, or no configured operator answers [`ObserveOutcome::Withdraw`]
    /// and records nothing — and so does any title or announcement
    /// observation for a group without an authorization row, so a lost
    /// leave call is healed by the group's next contact.
    ///
    /// An admitted title or announcement observation derives its note — the
    /// rules contract reads the pinned announcement here, in the core — and
    /// appends it on-delta: only when the observed text differs from the
    /// newest stored note of the same topic. The on-delta comparison is the
    /// whole admission check (the operator decided, 2026-08-23): pinning is an
    /// administrator-only right, so no rate window or append cap sits on
    /// this path — an identical re-pin appends nothing, and every real
    /// delta records. The read-then-append is serialized under the stamp
    /// lock, so two equal racing observations append one note; an
    /// authorized, unmapped group channel takes the same winner-only
    /// creation path a first message does, system prompt and palette
    /// included. Every appended rules note carries an acknowledgment back
    /// to the adapter — since unit 20 the bounded one-shot generation's
    /// in-voice text, with the fixed line as the deterministic fallback, so
    /// the delivery guarantee is unchanged; a title note is never
    /// acknowledged.
    ///
    /// A direct-channel observation observes nothing: group facts belong to
    /// groups, and the direct path is untouched by this unit.
    ///
    /// # Errors
    ///
    /// [`CoreError::ChannelKindMismatch`] if the channel is already mapped
    /// under a different kind than the observation claims.
    /// [`CoreError::ClaimLost`] if a first-contact claim lost its mapping
    /// row mid-claim. [`CoreError::Store`] if a read or the append fails.
    pub async fn observe(&self, observation: Observation) -> Result<ObserveOutcome, CoreError> {
        // Named without an underscore because the rules path below releases
        // it explicitly ahead of the acknowledgment generation; every other
        // path holds it to its return.
        let no_erasure_mid_observation = self.erasure_fence.read().await;
        let tx = self.ctx.store().tx();

        if let Some((_, stored_kind)) = mapping::find(&tx, &observation.channel).await?
            && stored_kind != observation.channel_kind
        {
            return Err(CoreError::ChannelKindMismatch {
                stored: stored_kind,
                claimed: observation.channel_kind,
            });
        }
        if observation.channel_kind != ChannelKind::Group {
            tracing::debug!("a direct-channel observation observes nothing");
            return Ok(ObserveOutcome::Observed { deliver: None });
        }

        match observation.fact {
            ObservedFact::Added { by } => {
                let operator = self
                    .operators
                    .by_adapter
                    .get(&observation.channel.adapter)
                    .map(String::as_str);
                if !authorization::operator_admits(operator, by.as_ref()) {
                    return Ok(ObserveOutcome::Withdraw);
                }
                authorization::authorize(&tx, &observation.channel).await?;
                Ok(ObserveOutcome::Observed { deliver: None })
            }
            ObservedFact::Title(_) | ObservedFact::PinnedAnnouncement(_) => {
                if !authorization::is_authorized(&tx, &observation.channel).await? {
                    return Ok(ObserveOutcome::Withdraw);
                }
                let Some((topic, text)) = note::note_of(&observation.fact) else {
                    return Ok(ObserveOutcome::Observed { deliver: None });
                };
                // The on-delta read-then-append, serialized against every
                // other stamp-locked write: without the lock two equal
                // observations would both read the pre-append newest note
                // and both append. Mapping resolution sits inside the lock
                // for the same reason — the loser of a creation race must
                // see the winner's note, not its own empty conversation.
                let one_stamp_at_a_time = self.stamp_lock.lock().await;
                let conversation_id = match mapping::find(&tx, &observation.channel).await? {
                    Some((existing, _)) => existing,
                    None => {
                        self.map_new_channel(&observation.channel, ChannelKind::Group)
                            .await?
                    }
                };
                // The palette supersession fires on observed activity too
                // (decided 2026-08-23): a conversation whose next contact
                // is a pin or a title change gains the current tools the
                // same way an ingested message grants them.
                self.reconcile_palette(conversation_id).await?;
                let newest = note::newest_text(self.ctx.store(), conversation_id, topic).await?;
                if let Some(pause) = &self.note_read_pause {
                    pause().await;
                }
                if newest.as_deref() == Some(text.as_str()) {
                    return Ok(ObserveOutcome::Observed { deliver: None });
                }
                self.ctx
                    .store()
                    .append_consumer_block(
                        conversation_id,
                        None,
                        note::CONTEXT_NOTE_KIND,
                        ContextNote::stored_fields(topic, &text),
                        None,
                    )
                    .await?;
                // The stamp lock covers exactly the read-then-append window
                // above; the acknowledgment generation below is a bounded
                // model call, and holding the one ingestion lock across it
                // would stall every conversation for the call's whole bound.
                drop(one_stamp_at_a_time);
                // The erasure fence releases with the lock: the note stands,
                // and the generation reads no personal data — holding the
                // fence would only queue an erasure (and, behind it, every
                // ingestion) on a model's latency.
                drop(no_erasure_mid_observation);
                // Every real rules delta is acknowledged (the operator decided,
                // 2026-08-23): pinning is an administrator-only right, so
                // no non-admin can trigger this line, and a rate window
                // here would only silence legitimate rules edits. The
                // on-delta comparison above is the whole admission check — an
                // identical re-pin appends nothing, says nothing, and calls
                // nothing. Since unit 20 the acknowledgment's text is the
                // bounded one-shot generation's, with the fixed line as the
                // deterministic fallback — the acknowledgment module states
                // the bounds and the guarantee.
                let deliver = match topic {
                    NoteTopic::Rules => Some(DeliveryItem::Acknowledgment(
                        self.rules_acknowledgment(conversation_id, &text).await,
                    )),
                    NoteTopic::Title => None,
                };
                Ok(ObserveOutcome::Observed { deliver })
            }
        }
    }

    /// The outbound edge for one adapter: a subscription yielding the
    /// assistant's replies on that adapter's channels, each bound to its
    /// channel key. Each adapter takes one edge under its own name and never
    /// sees another adapter's replies. Answers already stored when the
    /// subscription is taken are history and stay off it; every answer
    /// stored afterwards is delivered at least once, re-read from the ledger
    /// — the outbound module's doc states the exact delivery contract,
    /// including the failure notice's at-most-once nature.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if reading the stored state that marks the
    /// history boundary fails.
    pub async fn replies(
        &self,
        adapter: &str,
    ) -> Result<mpsc::UnboundedReceiver<OutboundReply>, CoreError> {
        Ok(outbound::spawn_edge(
            self.ctx.clone(),
            adapter.to_owned(),
            Arc::clone(&self.disclosure),
        )
        .await?)
    }

    /// The composing edge for one adapter: a subscription yielding the
    /// composing transitions on that adapter's channels — the assistant
    /// began working on an answer, it stopped — derived from the turn
    /// lifecycle, so a deterministic reply never signals. A live presence
    /// cue with no history, no persistence and no failure path: the
    /// composing module states the exact contract, its lag answer
    /// included. Each adapter takes one edge under its own name, beside
    /// its [`Self::replies`] edge.
    pub fn composing(&self, adapter: &str) -> mpsc::UnboundedReceiver<ComposingUpdate> {
        composing::spawn_edge(self.ctx.clone(), adapter.to_owned())
    }

    /// Erase one principal, in one call, per decision 0012: the personal
    /// columns of the principal's messages — text, origin reference and
    /// platform send time — are nulled in every conversation (the block
    /// headers keep their positions and references, and an erased message
    /// projects none of its prose to the model), the principal's direct
    /// conversations are removed entirely with their channel mappings, and
    /// the identity rows are concluded — deleted, or emptied to the
    /// suppression stub when the opt-out flag stands (2026-08-23), so the
    /// flag survives its own person's deletion. Reports
    /// [`ErasureOutcome::NotFound`] — touching nothing — when no identity
    /// row matches, an unflagged person's completed earlier erasure
    /// included; a flagged person's surviving stub keeps matching, so their
    /// repeat re-runs over emptiness and reports completion (the
    /// idempotency refinement recorded on decision 0012).
    ///
    /// A direct conversation showing an open stream — observed on the bus,
    /// or holding a stored streaming tail a gone runtime left behind — is
    /// settled first, per the streams module's protocol: the interrupt goes
    /// out and a bounded stored-state re-read confirms the interrupt's
    /// ledger writes have finished before anything is deleted, so the
    /// stream's appends cannot race the deletion. Past the bound the erasure
    /// fails loudly with [`CoreError::ErasureUnsettled`], deleting nothing.
    /// An idle principal pays no wait.
    ///
    /// Not covered, recorded OPEN in decision 0012: a group conversation's
    /// derived title may have been shaped by since-erased prose and is not
    /// regenerated here.
    ///
    /// The call holds the erasure fence exclusively, so no ingestion can
    /// record a message or a mapping for the person between the steps.
    ///
    /// # Errors
    ///
    /// [`CoreError::ErasureUnsettled`] if an open stream did not settle
    /// before the bound; [`CoreError::Store`] if a read, a write or a
    /// deletion fails.
    pub async fn erase_principal(&self, principal_id: i64) -> Result<ErasureOutcome, CoreError> {
        erase_behind_the_fence(
            self.ctx.clone(),
            Arc::clone(&self.streams),
            Arc::clone(&self.erasure_fence),
            principal_id,
        )
        .await
    }

    /// The suppression check and the writing resolution, in the stated
    /// order (decided 2026-08-23): first the READ-ONLY identity lookup — a
    /// select, never a write — so the check precedes every write the
    /// ingestion path can make; then the privacy command family; then, flag
    /// standing, the drop, answered as `None` — no message row, no identity
    /// refresh, no principal write, no conversation creation, no palette
    /// append, no mapping, no answer; the adapter acknowledges and its
    /// offset advances. The family alone is exempt from the drop: an
    /// opted-out person's `/unblockprivacy` must work, or the door never
    /// reopens from inside, and their `/privacy` keeps answering. An
    /// exempted command resolves its principal through the read-only
    /// standing — the request itself is the lawful processing of honoring
    /// it, recorded with the username frozen, so after a deletion no
    /// command re-materializes the emptied field — while every unflagged
    /// sender takes the resolving lookup as ever.
    ///
    /// Past suppression, the write needs the sender's authority; delivered
    /// unresolved, the message is refused transient and redelivers — the
    /// never-default rule, without letting a stranger group's failing
    /// authority source starve the batch (see the error's doc).
    async fn resolve_writing_sender(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
    ) -> Result<Option<WritingSender>, CoreError> {
        let standing = identity::find_standing(
            tx,
            message.channel.adapter.clone(),
            message.sender.external_id.clone(),
        )
        .await?;
        let family = privacy::family_command(message.command.as_ref());
        let suppressed = standing.is_some_and(|standing| standing.opted_out);
        if suppressed && family.is_none() {
            return Ok(None);
        }
        let Some(authority) = message.authority else {
            return Err(CoreError::AuthorityUnresolved);
        };
        let principal_id = match standing {
            Some(standing) if standing.opted_out => standing.principal_id,
            _ => {
                identity::resolve_principal(
                    tx,
                    message.channel.adapter.clone(),
                    message.sender.clone(),
                )
                .await?
            }
        };
        Ok(Some(WritingSender {
            principal_id,
            authority,
            family,
            suppressed,
        }))
    }

    /// The channel admission ahead of any write, in the stated order: the
    /// mapping's stored kind refuses a mis-claimed channel before anything
    /// else — the mapping knows what the channel is, and every later step
    /// decides personal-data handling by the kind; an unauthorized group
    /// is refused with the withdraw directive; a direct channel with the
    /// switch off is refused the same fail-closed way, before the sender's
    /// principal exists, before the channel maps, before any block appends
    /// — so a deployment with the switch off keeps a stranger's direct
    /// contact out of every table, not merely unanswered. Returns the
    /// mapping read alongside the refusal, if any, so the entry point
    /// reads the mapping exactly once.
    async fn admit_channel(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
    ) -> Result<(Option<(i64, ChannelKind)>, Option<IngestOutcome>), CoreError> {
        let mapped = mapping::find(tx, &message.channel).await?;
        if let Some((_, stored_kind)) = mapped
            && stored_kind != message.channel_kind
        {
            return Err(CoreError::ChannelKindMismatch {
                stored: stored_kind,
                claimed: message.channel_kind,
            });
        }
        if message.channel_kind == ChannelKind::Group
            && !authorization::is_authorized(tx, &message.channel).await?
        {
            return Ok((mapped, Some(IngestOutcome::Withdraw)));
        }
        if message.channel_kind == ChannelKind::Direct && self.direct_chats == DirectChats::Off {
            return Ok((mapped, Some(IngestOutcome::Disregarded)));
        }
        Ok((mapped, None))
    }

    /// The mirror's erasure, run for a triggering deletion command
    /// (2026-08-23): the named row's personal columns and the reply
    /// references pointing at it are nulled through the kind's mirror pass
    /// (decision 0085), and both counts are traced at info — a destructive
    /// act on stored personal data leaves a record in the default log, and
    /// zero counts name the silent no-ops' shape: a target the store never
    /// held or holds no longer. The caller's ordering reasoning sits at
    /// the call site.
    async fn mirror_named_deletion(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        target: &str,
    ) -> Result<(), CoreError> {
        let nulled = kind::erase_message_named(tx, conversation_id, target).await?;
        tracing::info!(
            conversation_id,
            target_rows = nulled.target_rows,
            reply_references = nulled.reply_references,
            "the deletion mirror ran over an administrator's reply command"
        );
        Ok(())
    }

    /// The summons resolution — the ONE place the answering mode enters
    /// the machinery (unit 14): a message summons the assistant when it
    /// addressed it, or when helpful answering evaluates every message.
    /// The literal addressed fact rides beside it (unit 16) — the
    /// adapter's own flag, before the mode folded in — stored with the
    /// stamp for the outbound miss-routing alone. Everything past this
    /// resolution — the budget consultation, the stamp, the unlatch, and
    /// every later reader of the stored summons — is mode-free.
    fn resolved_summons(&self, message: &InboundMessage) -> kind::Summons {
        kind::Summons {
            summoned: message.addressed || self.answering == AnsweringMode::Helpful,
            literal_addressed: message.addressed,
        }
    }

    /// The under-lock suppression re-read (2026-08-23): whether the
    /// sender's standing flag stands NOW, consulted while the caller holds
    /// the stamp lock. The pre-lock standing is read outside that lock,
    /// and a peer ingestion's flag write — a rights reply, serialized
    /// under the stamp lock — can land between the pre-lock read and the
    /// append; once the lock is held the flag is settled, so this second
    /// read is what makes "from the moment it stands" hold against the
    /// race, with the pre-lock check kept as the cheap early path that
    /// spares a suppressed flood the writing resolution. Only a
    /// non-command message consults it: the family is exempt.
    async fn suppressed_under_lock(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
    ) -> Result<bool, CoreError> {
        Ok(identity::find_standing(
            tx,
            message.channel.adapter.clone(),
            message.sender.external_id.clone(),
        )
        .await?
        .is_some_and(|standing| standing.opted_out))
    }

    /// The rules acknowledgment's text (unit 20): the bounded one-shot
    /// generation against the assembly's own binding and configured
    /// reasoning level, or the deterministic fallback — the acknowledgment
    /// module owns the bounds, the usability judgment and the guarantee.
    /// Called outside the stamp lock and the erasure fence on purpose: a
    /// model call holds no ingestion resource.
    async fn rules_acknowledgment(&self, conversation_id: i64, rules_text: &str) -> String {
        acknowledgment::rules_acknowledgment(
            self.ctx.providers(),
            &self.binding,
            self.reasoning,
            &self.name,
            conversation_id,
            rules_text,
        )
        .await
    }

    /// The admitted notice's channel-windowed answer: the fixed policy
    /// pointer, at most once per channel per window — recorded silence
    /// within it.
    async fn notice_answer(&self, conversation_id: i64) -> Option<DeliveryItem> {
        self.notice_answered.grants(conversation_id).await.then(|| {
            DeliveryItem::CommandAnswer(privacy::privacy_answer(
                self.privacy_policy_address.as_deref(),
            ))
        })
    }

    /// One rights command's reply, with its state change applied exactly
    /// when the reply is granted (decided 2026-08-23), through the reply
    /// window's one grant-with-the-change operation: a withheld reply
    /// withholds the change — a destructive action never runs into recorded
    /// silence — and a state-change write that fails has its grant handed
    /// back before it is logged and answered with nothing: the command is
    /// idempotent and re-asking works. The confirm's consumed pending
    /// spawns the erasure as its own task, so it runs after this ingestion
    /// returns — the ingestion path holds the erasure fence for reading,
    /// the erasure takes it for writing, and running it inline would
    /// deadlock on this very call.
    async fn rights_reply(
        &self,
        tx: &StoreTx,
        command: RightsCommand,
        principal_id: i64,
    ) -> Option<DeliveryItem> {
        let change = async {
            match command {
                RightsCommand::OptOut => {
                    identity::set_opt_out(tx, principal_id).await.map(|raised| {
                        if raised {
                            privacy::OPT_OUT_DONE
                        } else {
                            privacy::OPT_OUT_ALREADY
                        }
                    })
                }
                RightsCommand::OptIn => {
                    identity::clear_opt_out(tx, principal_id)
                        .await
                        .map(|cleared| {
                            if cleared {
                                privacy::OPT_IN_DONE
                            } else {
                                privacy::OPT_IN_ALREADY
                            }
                        })
                }
                RightsCommand::Delete => {
                    self.pending_deletions.file(principal_id).await;
                    Ok(privacy::CONFIRM_INSTRUCTION)
                }
                RightsCommand::Confirm => {
                    if self.pending_deletions.take(principal_id).await {
                        let ctx = self.ctx.clone();
                        let streams = Arc::clone(&self.streams);
                        let fence = Arc::clone(&self.erasure_fence);
                        tokio::spawn(async move {
                            match erase_behind_the_fence(ctx, streams, fence, principal_id).await {
                                Ok(outcome) => {
                                    tracing::info!(
                                        principal_id,
                                        ?outcome,
                                        "the confirmed erasure ran"
                                    );
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        principal_id,
                                        %error,
                                        "the confirmed erasure failed; the stored data stands, \
                                         and a fresh /privacydelete re-asks"
                                    );
                                }
                            }
                        });
                        Ok(privacy::DELETION_STARTED)
                    } else {
                        Ok(privacy::NOTHING_PENDING)
                    }
                }
            }
        };
        match self
            .privacy_replies
            .grant_with(principal_id, change)
            .await?
        {
            Ok(line) => Some(DeliveryItem::CommandAnswer(line.to_owned())),
            Err(error) => {
                tracing::warn!(
                    principal_id,
                    ?command,
                    %error,
                    "the rights command's write failed; nothing changed"
                );
                None
            }
        }
    }

    /// First message on a channel: create the conversation under the
    /// assembly's binding, record the system prompt and the tool palette as
    /// its first blocks, claim the mapping, and set the winner's reasoning
    /// level to the assembly's configured one. Direct and group channels
    /// take the identical path, so both get the same palette and the same
    /// level.
    ///
    /// Two ingestions can race here; the mapping's claim decides, and the
    /// loser's conversation is deleted — its prompt and palette blocks with
    /// it — before anything referenced it. Recording both before the claim
    /// is what makes them the winner's first blocks: the losing racer's
    /// message arrives in the winning conversation only after the winner's
    /// records are in. The reasoning set follows the claim instead, on the
    /// winner: both racers write the one configured value, so the second
    /// write repeats the first, and the loser's deleted row never needed
    /// one. Conversations created before the level shipped keep their
    /// deferring null — no backfill, because a stored null already reads as
    /// the provider's own default.
    async fn map_new_channel(
        &self,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<i64, CoreError> {
        let store = self.ctx.store();
        let created = store
            .create_conversation(
                self.binding.provider_instance.clone(),
                self.binding.model.clone(),
                self.binding.model_display_name.clone(),
                self.binding.vendor.clone(),
            )
            .await?;
        store
            .insert_system_prompt(created, self.system_prompt.clone())
            .await?;
        store
            .append_consumer_block(
                created,
                None,
                TOOL_PALETTE_KIND,
                ToolPalette::stored_fields(&self.palette),
                None,
            )
            .await?;
        let winner = mapping::claim(&store.tx(), channel, kind, created).await?;
        if winner != created {
            store.delete_conversation(created).await?;
        }
        store
            .set_conversation_reasoning(winner, Some(self.reasoning.as_key().to_owned()))
            .await?;
        Ok(winner)
    }

    /// The palette supersession on delta (decided 2026-08-23), under the
    /// stamp lock every caller holds: on a conversation's first activity
    /// per process, the newest stored palette is compared against the
    /// registered tool set, and a fresh palette block is appended when
    /// they differ — one write per real change, the context note's
    /// on-delta shape. A conversation created before a tool existed
    /// admits it on its next activity; a tool the handle no longer
    /// configures is removed the same way, because the registered set is
    /// the comparison's one side. A palette that never parsed reads as a
    /// delta: it admits nothing, so superseding it is the correction. The
    /// memory is marked only after the append stands — a transiently
    /// failed append leaves the conversation unreconciled, and the
    /// redelivered activity retries.
    async fn reconcile_palette(&self, conversation_id: i64) -> Result<(), CoreError> {
        if self
            .palette_reconciled
            .lock()
            .await
            .contains(&conversation_id)
        {
            return Ok(());
        }
        let stored = palette::newest_tools(self.ctx.store(), conversation_id).await?;
        let already_current =
            stored.is_some_and(|tools| tools.as_deref() == Some(self.palette.as_slice()));
        if !already_current {
            self.ctx
                .store()
                .append_consumer_block(
                    conversation_id,
                    None,
                    TOOL_PALETTE_KIND,
                    ToolPalette::stored_fields(&self.palette),
                    None,
                )
                .await?;
            tracing::info!(
                conversation_id,
                "the conversation's palette was superseded to the registered tool set"
            );
        }
        let mut reconciled = self.palette_reconciled.lock().await;
        if reconciled.len() >= PALETTE_MEMORY_CAP {
            tracing::debug!("the palette memory reached its cap and was cleared");
            reconciled.clear();
        }
        reconciled.insert(conversation_id);
        Ok(())
    }

    /// The conversation's owing tail, if any — the one-block read behind the
    /// write-time stamp, deciding through the kind's own
    /// [`ChatMessage::owes_answer`] so this read and the awaiting hook can
    /// never disagree about one stamp: an erased tail's OWN debt, which the
    /// hook cancels, propagates nothing here either, while a live debt a
    /// third party's row still owes behind an erased run reads through
    /// (decision 0086) — someone else's deletion erases one row's ask, not
    /// the standing question behind it. The tail carrying the debt
    /// IS it being unanswered: an answer, a streaming tail or any later
    /// block would be the tail instead, and mid-turn absorption keeps its
    /// own semantics for those — an addressed message absorbed mid-turn
    /// opens a fresh debt at its own authority, correct for answering (the
    /// next turn pays it; decision 0021), while tool admission folds such
    /// a co-summoner in by its stored sender authority under the
    /// opened-debt predicate, not through this debt stamp (decision 0043).
    /// An owing tail hands over the authority its debt carries, folded
    /// through the kind's pre-migration rule.
    ///
    /// The read walks past the consumer's own mid-history kinds exactly —
    /// context notes (refined 2026-08-23), and, widened the same day with
    /// the report and the palette supersession, the report block and a
    /// superseding palette ([`DEBT_READ_THROUGH`]): each is appended by an
    /// independent path at an arbitrary moment, so a debt behind a run of
    /// them still owes and must propagate through to the next message's
    /// stamp. Erased chat rows are transparent the same way (2026-08-23,
    /// the deletion mirror): they share the live rows' kind, so the kind's
    /// own query skips them by shape instead. The framework's other
    /// transparent kinds, the turn-closure markers above all, stay a
    /// settled tail here: the framework's own walk governs turn liveness,
    /// and reading debt through a closed turn's marker would widen
    /// propagation past failed turns. Every read is bounded — the tail row,
    /// then at most one query past the whole transparent run, never a
    /// conversation hydration — because this sits on ingestion's hot path
    /// and the framework leaves a transparent turn-end marker as the tail
    /// of every answered conversation.
    async fn owing_tail_debt(
        &self,
        conversation_id: i64,
    ) -> Result<Option<kind::TailDebt>, CoreError> {
        let store = self.ctx.store();
        let Some(tail) = store.latest_block(conversation_id).await? else {
            return Ok(None);
        };
        let transparent = DEBT_READ_THROUGH.contains(&tail.block_type.as_str())
            || matches!(
                AssistantKind::from_block(&tail),
                AssistantKind::ChatMessage(message) if message.erased()
            );
        let tail = if transparent {
            match kind::newest_block_id_past_erased(&store.tx(), conversation_id, DEBT_READ_THROUGH)
                .await?
            {
                Some(behind_the_run) => store.find_block(behind_the_run).await?,
                None => None,
            }
        } else {
            Some(tail)
        };
        Ok(match tail.map(|block| AssistantKind::from_block(&block)) {
            Some(AssistantKind::ChatMessage(message)) if message.owes_answer() => {
                Some(kind::TailDebt {
                    authority: message.carried_debt_authority(),
                })
            }
            Some(
                AssistantKind::ChatMessage(_)
                | AssistantKind::Core(_)
                | AssistantKind::ToolPalette(_)
                | AssistantKind::ContextNote(_)
                | AssistantKind::Report(_),
            )
            | None => None,
        })
    }

    /// The first budget refusing this message's own debt, principal before
    /// channel, or `None` when every enabled budget admits it. Consulted for
    /// summoned messages only, inside the stamp serialization; each count
    /// is derived from the ledger at this write — no counter table, no
    /// in-memory tally — so the budget's whole state is the recent recorded
    /// history itself.
    async fn refusing_budget(
        &self,
        principal_id: i64,
        conversation_id: i64,
    ) -> Result<Option<kind::LimitedBy>, CoreError> {
        let tx = self.ctx.store().tx();
        if let Some(budget) = &self.protection.principal {
            let opened =
                kind::opened_debts_by_principal(&tx, principal_id, budget.window_seconds).await?;
            if opened >= i64::from(budget.answers.get()) {
                return Ok(Some(kind::LimitedBy::Principal));
            }
        }
        if let Some(budget) = &self.protection.channel {
            let opened =
                kind::opened_debts_in_conversation(&tx, conversation_id, budget.window_seconds)
                    .await?;
            if opened >= i64::from(budget.answers.get()) {
                return Ok(Some(kind::LimitedBy::Channel));
            }
        }
        Ok(None)
    }
}

/// What the writing path knows about an admitted sender: the resolved
/// principal, the delivered authority, the privacy command family member
/// the message invokes, if any, and whether the suppression flag stands —
/// the facts [`Assistant::resolve_writing_sender`] hands the stamp, the
/// stored fields and the delivery.
struct WritingSender {
    principal_id: i64,
    authority: Authority,
    family: Option<PrivacyCommand>,
    /// The standing suppression flag: `true` only on an exempt command,
    /// since every other suppressed message was dropped before this. The
    /// stored fields read it — a suppressed sender's command records no
    /// speaker, so after a deletion no command re-materializes the emptied
    /// handle.
    suppressed: bool,
}

/// The one erasure body, behind the fence taken for writing: what
/// [`Assistant::erase_principal`] runs inline and what a confirmed
/// `/confirmdelete` spawns as its own task after its ingestion returned —
/// the spawn-outside-the-fence execution model (decided 2026-08-23). The
/// ingestion path holds the fence for reading, this takes it for writing,
/// so an erasure run inline from ingestion would deadlock on itself; a
/// spawned run simply waits for the ingestion's read hold to release. The
/// arguments are the owned handles a detached task needs — the runtime
/// context, the stream observer and the fence — so the task outlives the
/// call that spawned it without borrowing the assembly.
async fn erase_behind_the_fence(
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    streams: Arc<StreamObserver>,
    fence: ErasureFence,
    principal_id: i64,
) -> Result<ErasureOutcome, CoreError> {
    let _no_ingestion_mid_erasure = fence.write().await;
    let store = ctx.store();
    let Some(plan) = erasure::plan(store, principal_id).await? else {
        return Ok(ErasureOutcome::NotFound);
    };
    // The plan's conversations are exactly the deletion set, so settling
    // them is settling everything the execute step will remove.
    for &conversation_id in plan.direct_conversations() {
        streams::settle_for_deletion(store, ctx.bus(), &streams, conversation_id).await?;
    }
    let outcome = erasure::execute(store, plan).await?;
    if let ErasureOutcome::Erased {
        deleted_conversations,
    } = &outcome
    {
        // The store reissues conversation ids: a deleted conversation's
        // stream observation must not survive to shadow the id's next
        // holder.
        for &deleted in deleted_conversations {
            streams.forget(deleted);
        }
    }
    Ok(outcome)
}
