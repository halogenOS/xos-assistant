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
use std::time::Instant;

use agent_ledger::providers::ReasoningLevel;
use agent_ledger::store::{ProviderInstance, StoreTx};
use agent_ledger::{
    BlockKind, CoreEvent, EventBus, FromBlock, ProviderRegistry, Role, RuntimeContext, Store,
    spawn_reactor,
};
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::acknowledgment;
use crate::commands::{self, Command};
use crate::compaction::CompactTrigger;
use crate::composing;
use crate::erasure::{self, ErasureOutcome};
use crate::error::CoreError;
use crate::filing::{self, FilingDoor};
use crate::join::JoinNotice;
use crate::kind::{
    self, AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage, NEVER_ANSWERABLE,
};
use crate::message::{
    Authority, ChannelKey, ChannelKind, ChannelReset, ComposingUpdate, DeliveryHandle,
    DeliveryItem, InboundMessage, IngestOutcome, IngestReceipt, JoinedMember, Observation,
    ObserveOutcome, ObservedDelivery, ObservedFact, Outbound,
};
use crate::note::{self, ContextNote, NoteTopic};
use crate::privacy::{PendingDeletions, PrivacyCommand, RightsCommand};
use crate::quoting;
use crate::session::{CompactOutcome, Sessions, WipeOutcome};
use crate::streams::StreamObserver;
use crate::tools::changelog::HarnessChangelog;
use crate::tools::mark::{self, MarkTool};
use crate::tools::report::{self, ReportTool};
use crate::tools::rights::PrivacyTool;
use crate::tools::runtime::RuntimeFacts;
use crate::tools::search::{SearchConfig, WebSearch};
use crate::tools::standing::StandingLookup;
use crate::tools::{ToolSet, palette, palette::TOOL_PALETTE_KIND, palette::ToolPalette};
use crate::window::{
    ACKNOWLEDGMENT_WINDOW, LineWindow, PRIVACY_REPLY_CAP, PRIVACY_REPLY_WINDOW, RESET_REPLY_CAP,
    RESET_REPLY_WINDOW, ReplyWindow,
};
use crate::{
    authorization, delivery, identity, join, mapping, mirror, outbound, privacy, session, streams,
};

/// The erasure fence, as the shared handle the report tool receives at its
/// construction: ingestions and the tool's filing hold it shared, the
/// person-wide erasure holds it exclusively, so a report cannot
/// re-materialize an origin THAT operation just nulled. What it does not
/// order is two shared holders against each other — a filing against the
/// deletion mirror, which runs inside an ingestion — and that is what the
/// filing door beside it is for ([`crate::filing`]).
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
    // The join notice (unit 36, 2026-08-29): the observation path appends
    // it whenever someone walks in, which is exactly the arbitrary moment
    // this list exists for — a member's unanswered question behind a run
    // of joins still owes its turn.
    join::JOIN_NOTICE_KIND,
    // The delivery receipt (unit 38, 2026-08-30): the adapter reports one
    // the moment the platform takes a message, so a receipt can land at
    // the tail behind anything. Load-bearing on day one — a failed turn's
    // failure notice records its own delivery AT THE TAIL, and an opaque
    // tail there would bury the standing question behind it.
    delivery::DELIVERED_KIND,
    // The message mark (unit 39, 2026-08-30): a reaction is placed
    // precisely on turns that answer nothing, so its block is the ledger
    // tail more often than a report's is. Without this entry a member's
    // unanswered question behind a run of reactions would stop owing its
    // turn.
    mark::MESSAGE_MARK_KIND,
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
/// model and the model decides whether to speak, staying silent by ending
/// its turn with no text. A deployment that wants the quiet shape sets
/// `addressed`.
/// The mode enters the machinery at exactly one place: the entry point's
/// summons resolution ahead of the write-time stamp — everything past the
/// stamp reads the stored summons fact and stays mode-free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AnsweringMode {
    /// Every group message summons a turn; the model decides whether to
    /// speak and stays silent by ending its turn with no text.
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
    /// The monotonic instant the process started, captured once by the
    /// binary and carried here (unit 32, 2026-08-28). The runtime-facts
    /// tool measures uptime from it, so what the assistant states is the
    /// age of the process and not of this assembly call.
    pub started_at: Instant,
    /// The web search's wiring: the vendor's address, the resolved key and
    /// the locale (unit 27, 2026-08-27). Present, the assembly registers
    /// the search tool and the composed prompt teaches it; absent, neither
    /// exists — one predicate for both, the report tool's precedent, so
    /// there is no call path on which an unconfigured search can answer.
    /// The key is a resolved secret and never a value in a file: the
    /// config type's own `Debug` redacts it, because this carrier derives
    /// one.
    pub web_search: Option<SearchConfig>,
}

/// The running core: the framework runtime spawned over the assistant's
/// composed kind, plus the assistant's two edges and the erasure operation.
pub struct Assistant {
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    /// The channel's conversation lifecycle: the model binding, the
    /// reasoning level, the composed system prompt and the tool palette
    /// every new conversation is created with, plus the operations that
    /// create or replace one. Shared with the unattended compaction
    /// watcher, which holds it weakly and ends with this assembly.
    ///
    /// It is also the one home of the two holds this assembly runs under —
    /// the ingestion stamp lock and the erasure fence (unit 45,
    /// 2026-08-30). They ordered ingestion before they ordered a reset, and
    /// a reset is where the two meet, so every path here takes them through
    /// [`Sessions::stamp_lock`] and [`Sessions::erasure_fence`] instead of
    /// this type keeping a second handle to each.
    sessions: Arc<Sessions>,
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
    /// The session resets' per-person bound (unit 45, 2026-08-30): `/wipe`
    /// and `/compact` share ONE instance, the privacy family's
    /// one-window-per-family shape, and it is deliberately not the privacy
    /// family's own — a flood of resets must not silence somebody else's
    /// rights command. The reset is applied exactly with the granted reply,
    /// so a withheld reply withholds the reset.
    reset_replies: ReplyWindow,
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
    /// Orders the deletion mirror's nulls against the tools that file a
    /// block naming a message origin — the same door those tools take
    /// around their own scan-then-append, because the fence cannot order
    /// them: the mirror runs under this path's SHARED fence hold and a
    /// filing takes the fence shared too. The whole contract, and the lock
    /// order this path obeys, are on [`crate::filing`].
    filing_door: FilingDoor,
}

/// What the assembly itself adds to the embedder's tool set: the shared
/// state a tool is constructed over, and the configuration each conditional
/// registration reads. One carrier, so a later capability joins the list
/// instead of growing the start sequence.
struct AssembledTools {
    moderation_handle: Option<String>,
    answering: AnsweringMode,
    web_search: Option<SearchConfig>,
    pending_deletions: Arc<PendingDeletions>,
    privacy_replies: Arc<ReplyWindow>,
    erasure_fence: ErasureFence,
    filing_door: FilingDoor,
}

/// Add the assembly's own tools to the embedder's set, in one place: each
/// conditional registration takes exactly the predicate the prompt
/// composition took, so the prompt can never teach a tool the palette does
/// not carry, and the delta mechanism removes an unconfigured tool from
/// conversations that had it.
///
/// - The REPORT tool needs a moderation handle AND helpful answering (unit
///   15): the report line goes nowhere without a handle, and only helpful
///   answering shows the model every message it would judge. The erasure
///   fence is injected here, so the tool never reaches into the assembly,
///   and the filing door beside it — the two tools that file against a
///   message origin take the SAME door, which is what orders a report and
///   a reaction naming one message against each other and against the
///   deletion mirror ([`crate::filing`]).
/// - The WEB SEARCH needs a configured key and nothing else (unit 27). It
///   owns its own per-person budget and its own cache, so nothing but the
///   configuration is handed to it.
/// - The PRIVACY tool joins unconditionally (decided 2026-08-23): the
///   rights it reaches exist in every deployment. Its pending memory and
///   its reply bound are shared with the command family, injected here so
///   the tool and the commands act on one state.
/// - The STANDING lookup joins unconditionally too (unit 29): the standing
///   it reads is recorded in every deployment, and the question it answers
///   — whether the person claiming authority holds it — is asked wherever
///   the assistant runs. It reads person data, so it takes the erasure
///   fence here, exactly as the report and privacy tools do.
/// - The REACT tool joins unconditionally as well (unit 39): a reaction
///   needs nothing but a chat. It sits here and NOT under the report's
///   moderation predicate, which is about a capability that needs a
///   moderation bot to receive it — inheriting that condition would tie a
///   cosmetic acknowledgement to a moderation deployment for no reason
///   anyone could state, and would remove it from the addressed mode. It
///   writes a block naming a person, so it takes the erasure fence too,
///   and the filing door it shares with the report.
fn admit_assembled_tools(tools: &mut ToolSet, assembled: AssembledTools) {
    let AssembledTools {
        moderation_handle,
        answering,
        web_search,
        pending_deletions,
        privacy_replies,
        erasure_fence,
        filing_door,
    } = assembled;
    if let Some(handle) = moderation_handle
        && crate::teaching::moderation_taught(true, answering)
    {
        tools.admit(
            report::REQUIRED_AUTHORITY,
            ReportTool::new(handle, Arc::clone(&erasure_fence), Arc::clone(&filing_door)),
        );
    }
    if let Some(search) = web_search {
        tools.admit(
            crate::tools::search::REQUIRED_AUTHORITY,
            WebSearch::new(search, crate::tools::search::DEFAULT_TIMEOUT),
        );
    }
    tools.admit(
        crate::tools::standing::REQUIRED_AUTHORITY,
        StandingLookup::new(Arc::clone(&erasure_fence)),
    );
    tools.admit(
        mark::REQUIRED_AUTHORITY,
        MarkTool::new(Arc::clone(&erasure_fence), filing_door),
    );
    tools.admit(
        crate::tools::rights::REQUIRED_AUTHORITY,
        PrivacyTool::new(pending_deletions, privacy_replies, erasure_fence),
    );
}

/// The tools no assembly is without (unit 32, unit 47): the runtime facts
/// and the harness changelog join unconditionally, because neither has a
/// configuration to be absent — the questions they answer are asked
/// wherever the assistant runs, the one value the changelog states is
/// embedded in the build, and a build that passed no changelog gets a tool
/// that answers that absence, not a tool that vanishes.
///
/// The runtime-facts tool takes the binary's start instant here, the one
/// fact it cannot reach for itself. The model is not injected: that one
/// belongs to the conversation being answered, which the tool has in hand
/// at the call, and the binding decides it only for the conversations
/// this assembly goes on to create.
fn admit_unconditional_tools(tools: &mut ToolSet, started_at: Instant) {
    tools.admit(
        crate::tools::runtime::REQUIRED_AUTHORITY,
        RuntimeFacts::new(started_at),
    );
    tools.admit(
        crate::tools::changelog::REQUIRED_AUTHORITY,
        HarnessChangelog::new(),
    );
}

/// The two wiring checks the assembly refuses to start without, and the
/// binding's provider instance recorded in the store.
///
/// Both refusals are loud on purpose: a store opened without the message
/// kind's content table would fail every append later and further from the
/// cause, and a vendor no registered module answers to would silently
/// strand every conversation the binding creates.
///
/// # Errors
///
/// [`CoreError::MissingContentTable`], [`CoreError::UnknownVendor`], or
/// [`CoreError::Store`] if recording the instance fails.
async fn check_and_record_wiring(
    store: &Store,
    providers: &ProviderRegistry,
    binding: &ModelBinding,
) -> Result<(), CoreError> {
    if !store.content_tables().contains(&CHAT_MESSAGE_TABLE) {
        return Err(CoreError::MissingContentTable {
            table: CHAT_MESSAGE_TABLE,
        });
    }
    if providers.get(&binding.vendor).is_none() {
        return Err(CoreError::UnknownVendor {
            vendor: binding.vendor.clone(),
        });
    }
    store
        .save_provider_instance(ProviderInstance {
            id: binding.provider_instance.clone(),
            provider_type: binding.vendor.clone(),
            name: binding.provider_display_name.clone(),
        })
        .await?;
    Ok(())
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
            started_at,
            web_search,
        } = config;
        // The two configured-value compositions, resolved once: the prompt
        // every new conversation records — each capability's teaching
        // riding it exactly when the tool below registers — and the
        // disclosure every outbound edge introduces with.
        let system_prompt = crate::teaching::composed_system_prompt(
            &system_prompt,
            &name,
            answering,
            crate::teaching::Capabilities {
                moderation_handle: moderation_handle.is_some(),
                web_search: web_search.is_some(),
            },
        );
        let disclosure = Arc::new(crate::disclosure::Disclosure::resolve(
            disclosure.as_deref(),
            &name,
        ));
        check_and_record_wiring(&store, &providers, &binding).await?;
        let erasure_fence: ErasureFence = Arc::new(RwLock::new(()));
        let filing_door = filing::door();
        let privacy_replies = Arc::new(ReplyWindow::new(PRIVACY_REPLY_WINDOW, PRIVACY_REPLY_CAP));
        let pending_deletions = Arc::new(PendingDeletions::new());
        let mut tools = tools;
        admit_assembled_tools(
            &mut tools,
            AssembledTools {
                moderation_handle,
                answering,
                web_search,
                pending_deletions: Arc::clone(&pending_deletions),
                privacy_replies: Arc::clone(&privacy_replies),
                erasure_fence: Arc::clone(&erasure_fence),
                filing_door: Arc::clone(&filing_door),
            },
        );
        admit_unconditional_tools(&mut tools, started_at);
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
        // The release lookup's own rate window (the operator's numbers,
        // decision 0169): bound here, before the context is shared, and
        // only when the embedder's set registered the tool — the builder
        // refuses a name nothing registered, and a test assembly with no
        // lookups is not misconfigured.
        let ctx = if palette
            .iter()
            .any(|tool| tool == crate::tools::release::NAME)
        {
            ctx.with_tool_window(
                crate::tools::release::NAME,
                crate::tools::release::WINDOW_CALLS,
                crate::tools::release::WINDOW_SECONDS,
            )
        } else {
            ctx
        };
        let streams = streams::spawn_observer(ctx.bus());
        // The two holds are handed to the sessions and kept there: every
        // path in this assembly takes them back through it, so neither has
        // a second home to drift from.
        let sessions = Arc::new(Sessions::new(
            ctx.clone(),
            binding,
            reasoning,
            system_prompt,
            palette,
            Arc::new(Mutex::new(())),
            erasure_fence,
        ));
        // The unattended compaction watcher, beside the stream observer and
        // on the same broadcast: a turn the framework ended over a spent
        // tool-call window has left the conversation in the shape the
        // command exists to clear, and nobody is going to type anything.
        session::spawn_auto_compact(&sessions, ctx.bus());
        spawn_reactor(ctx.clone());
        Ok(Self {
            ctx,
            sessions,
            answering,
            name,
            disclosure,
            streams,
            protection,
            operators,
            direct_chats,
            privacy_policy_address,
            notice_answered: LineWindow::new(ACKNOWLEDGMENT_WINDOW),
            privacy_replies,
            reset_replies: ReplyWindow::new(RESET_REPLY_WINDOW, RESET_REPLY_CAP),
            pending_deletions,
            note_read_pause: None,
            standing_read_pause: None,
            palette_reconciled: Mutex::new(HashSet::new()),
            filing_door,
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

    /// Install the reset claim race's test seam: the given pause runs
    /// between a session reset's mapping delete and its claim, which is the
    /// window a concurrent racer takes the channel in — so a suite proves
    /// what a reset that lost the claim answers. Production never calls
    /// this.
    pub fn pause_between_reset_delete_and_claim(&mut self, pause: ScriptedPause) {
        self.sessions.pause_between_reset_delete_and_claim(pause);
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
        let _no_erasure_mid_message = self.sessions.erasure_fence().read().await;
        let tx = self.ctx.store().tx();

        // The channel admission runs whole before the sender is looked at;
        // its checks and their order are on [`Self::admit_channel`].
        if let Some(refusal) = self.admit_channel(&tx, &message).await? {
            return Ok(refusal);
        }
        let Some(sender) = self.resolve_writing_sender(&tx, &message).await? else {
            return Ok(IngestOutcome::Disregarded);
        };
        let WritingSender {
            principal_id,
            authority,
            command,
            family,
            suppressed,
        } = sender;
        if let Some(pause) = &self.standing_read_pause {
            pause().await;
        }

        // Held from the tail read and the budget counts through the append:
        // the stamp is decided against the tail this write is appended
        // behind, and the counts must see every earlier taken debt — so no
        // concurrent ingestion may slide a block in between, and two racing
        // messages cannot both take the last budget slot. The lock's whole
        // contract is on the field it lives on, [`Sessions::stamp_lock`].
        let _one_stamp_at_a_time = self.sessions.stamp_lock().lock().await;
        let conversation_id = self.conversation_under_lock(&tx, &message).await?;
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
        // person-wide operation, so no spawn is needed — and behind the
        // filing door the pass takes for itself, in the order the door's
        // module fixes: fence first, door second, exactly as a filing tool
        // takes them, so no cycle exists. The stamp below is then decided against the
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
        let summons = self.resolved_summons(&message);
        let owing_tail = self.owed_tail(conversation_id, &message, summons).await?;
        // The command stamp covers ANY recognized command since unit 45
        // (2026-08-30), not the privacy family alone: recognition is global
        // and the audience reading decides only the ANSWER, so a command
        // invoked where it is not offered still takes no debt, opens no
        // turn and never unlatches.
        let limited = if command.is_some() || mirrored.is_some() {
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
        // The reply's context lands first (unit 31, 2026-08-28): a reply
        // to a message this conversation holds is preceded by a quote
        // block referencing it, so the model reads the quoted words above
        // the member's own instead of a sentence with its subject
        // missing. Inside the stamp lock with the append below, and after
        // the stamp is decided, so the pair is serialized against every
        // other ingestion and the quote never enters the tail read the
        // stamp was taken against. Everything it decides — whether there
        // is anything to quote at all, which span, and the crash-retry
        // skip — is the quoting module's; nothing about it reaches the
        // stamp, the windows or the answer.
        quoting::land_reply_quote(self.ctx.store(), conversation_id, &message).await?;
        // The recorded row's own id travels to the command answer below: a
        // `/compact` counts the conversation it FOUND, which is the rows
        // older than this very block, and naming the row beats assuming it
        // is the newest one.
        let recorded_row = self
            .ctx
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
        //
        // The audience reading decides the answer (unit 45, 2026-08-30): a
        // command invoked below its floor, or in a kind of channel it does
        // not serve, answers SILENCE. No refusal line goes out, because a
        // refusal line advertises a surface the person cannot use — the
        // stamp above already took the debt out of the message.
        let (deliver, reset) = self
            .command_answer(
                &tx,
                &message,
                RecordedRow {
                    conversation_id,
                    block_id: recorded_row,
                },
                sender,
                notice_admitted,
            )
            .await;
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
            reset,
        })
    }

    /// The conversation an ingested message is written into, resolved with
    /// the stamp lock HELD — the observation path's own idiom, for the same
    /// reason and one more.
    ///
    /// A session reset moves a channel from one conversation to another
    /// under this very lock, so a mapping read taken before the lock can
    /// name a conversation the channel has already left; the message would
    /// then be appended into a retired ledger the model never reads again,
    /// and the adapter's acknowledgment of that update means nothing
    /// redelivers it. Resolved here, a message queued across a reset lands
    /// in the session that survived it.
    ///
    /// The channel's first message creates the conversation, and the loser
    /// of a creation race reads the winner's — the claim decides, here as
    /// everywhere.
    ///
    /// # Errors
    ///
    /// [`CoreError::ClaimLost`] if a first-message claim lost its mapping
    /// row mid-claim; [`CoreError::Store`] if the read or the creation
    /// fails.
    async fn conversation_under_lock(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
    ) -> Result<i64, CoreError> {
        Ok(match mapping::find(tx, &message.channel).await? {
            Some((existing, _)) => existing,
            None => {
                self.sessions
                    .map_new_channel(&message.channel, message.channel_kind)
                    .await?
                    .conversation_id
            }
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
    /// Retire every mapped conversation whose recorded system prompt is no
    /// longer the one this process composes, so an edited prompt reaches the
    /// groups already being served.
    ///
    /// A conversation records the composed prompt once, when it is created.
    /// That was the whole story while a deployment's wording never moved, and
    /// it stopped being the whole story the moment one did: the operator
    /// edited the prose, the deployment shipped, and every live group kept
    /// answering under the wording it had started with, with nothing saying
    /// so. The prompt had changed and the assistant had not.
    ///
    /// This runs once at startup, which is exactly when the composed prompt
    /// can differ — it is composed from configuration and from files read at
    /// boot, so nothing can change it while the process runs. Each mapped
    /// channel's conversation is read for its system prompt; where that text
    /// differs from the one composed now, the channel's mapping is dropped.
    /// The next message on that channel then takes the ordinary unmapped
    /// path and creates a fresh conversation carrying the current prompt.
    ///
    /// Nothing is rewritten and nothing is deleted. The old conversation
    /// stays in the ledger exactly as it was — readable, exportable, and
    /// reachable by erasure through the same principal it always was — and
    /// the channel simply moves on from it. That is the append-only answer
    /// to a changed prompt: not a mutated record, but a new conversation
    /// beside the old one.
    ///
    /// The cost is stated rather than hidden: the group's earlier
    /// conversation no longer rides in the model's context, so the assistant
    /// starts that channel fresh. A prompt edit is a change of instructions,
    /// and carrying half a conversation under new instructions is its own
    /// kind of wrong; a model swap is a change of the dispatch itself, and a
    /// channel left on the old binding keeps talking — and billing — through
    /// it, which the operator watched happen before this walk learned to
    /// look (2026-08-29).
    ///
    /// Returns how many channels were retired.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if a mapping read, a block read or the unmapping
    /// fails.
    pub async fn retire_stale_channels(&self) -> Result<usize, CoreError> {
        let store = self.ctx.store();
        let tx = store.tx();
        let mut retired = 0;
        for record in mapping::all(&tx).await? {
            let blocks = store.list_blocks(record.conversation_id).await?;
            let recorded = blocks
                .iter()
                .find_map(|block| match AssistantKind::from_block(block) {
                    AssistantKind::Core(kind::FrameworkKind(BlockKind::SystemPrompt(prompt))) => {
                        Some(prompt.content)
                    }
                    _ => None,
                });
            // An absent prompt retires too: a mapped conversation that never
            // recorded one cannot be serving the current wording either, and
            // leaving it mapped would keep that silence permanent.
            let prompt_current = recorded.as_deref() == Some(self.sessions.system_prompt());
            // The stored model is what the dispatch actually sends, and a
            // conversation keeps the binding it was created with — so a
            // configured swap reaches an existing channel only through this
            // walk. The operator watched an old channel still billing the
            // previous model while the introspection truthfully reported it
            // (2026-08-29); staleness in EITHER the prompt or the model
            // retires the channel.
            let model_current = match store.find_conversation(record.conversation_id).await? {
                Some(conversation) => {
                    conversation.model.external_id == self.sessions.binding().model
                        && conversation.model.provider_id
                            == self.sessions.binding().provider_instance
                }
                None => true,
            };
            if prompt_current && model_current {
                continue;
            }
            let Some(channel) =
                mapping::channel_for_conversation(&tx, record.conversation_id).await?
            else {
                continue;
            };
            let Some(last) = blocks.last().map(|block| block.id) else {
                continue;
            };
            // Fork instead of starting over. The group's conversation is the
            // context every answer is built from, and a prompt edit is not a
            // reason to forget what was said — the fork inherits the history
            // through the junction, so nothing is copied and nothing is lost.
            // The inherited prompt blocks are what the fork detaches: an
            // appended replacement would sit behind them, read second and
            // obeyed unevenly. Everything else about the fork — the current
            // binding, the current prompt in their place, the configured
            // reasoning level — is the session module's one recording of
            // what a fork under the current deployment is.
            let inherited_prompts: Vec<i64> = blocks
                .iter()
                .filter(|block| {
                    matches!(
                        AssistantKind::from_block(block),
                        AssistantKind::Core(kind::FrameworkKind(BlockKind::SystemPrompt(_)))
                    )
                })
                .map(|block| block.id)
                .collect();
            let successor = self
                .sessions
                .forked_with_current_prompt(record.conversation_id, last, &inherited_prompts)
                .await?;
            mapping::delete_by_conversation(&tx, record.conversation_id).await?;
            mapping::claim(&tx, &channel, record.kind, successor).await?;
            retired += 1;
            tracing::info!(
                conversation_id = record.conversation_id,
                successor,
                prompt_current,
                model_current,
                "the recorded prompt or model is stale; the channel forks and takes the current ones"
            );
        }
        Ok(retired)
    }

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
        let no_erasure_mid_observation = self.sessions.erasure_fence().read().await;
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
            ObservedFact::MembersJoined {
                joiners,
                origin,
                timestamp,
            } => {
                if !authorization::is_authorized(&tx, &observation.channel).await? {
                    return Ok(ObserveOutcome::Withdraw);
                }
                self.record_join_event(
                    &tx,
                    &observation.channel,
                    &joiners,
                    &origin,
                    &timestamp.to_rfc3339(),
                )
                .await?;
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
                let one_stamp_at_a_time = self.sessions.stamp_lock().lock().await;
                let conversation_id = match mapping::find(&tx, &observation.channel).await? {
                    Some((existing, _)) => existing,
                    None => {
                        self.sessions
                            .map_new_channel(&observation.channel, ChannelKind::Group)
                            .await?
                            .conversation_id
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
                    // The item rides with the handle its send is recorded
                    // under (unit 38, 2026-08-30): the acknowledgment is
                    // one of the assistant's own messages like any other,
                    // so its delivery is recorded like any other — and it
                    // names no quotable block, an item being the core's
                    // fixed prose.
                    NoteTopic::Rules => Some(ObservedDelivery {
                        delivery: DeliveryHandle::in_conversation(conversation_id),
                        item: DeliveryItem::Acknowledgment(
                            self.rules_acknowledgment(conversation_id, &text).await,
                        ),
                    }),
                    NoteTopic::Title => None,
                };
                Ok(ObserveOutcome::Observed { deliver })
            }
        }
    }

    /// Record what one send put in the chat (unit 38, 2026-08-30): the
    /// third entry point beside [`Assistant::ingest`] and
    /// [`Assistant::observe`], taking the handle the core handed out with
    /// the text and the platform's own ids for the messages that actually
    /// reached the chat, in send order.
    ///
    /// One [`crate::delivery::Delivered`] block per reported origin, all
    /// under the first one as the delivery key, each naming the stored
    /// block a reply to that message quotes where the send carried one.
    /// The adapter reports after every successful send on either path,
    /// including a send cut short partway: the reported list is exactly
    /// what reached the chat, so an empty list records nothing.
    ///
    /// This answers nothing and never fails outward. A conversation that no
    /// longer exists — erasure can delete a direct conversation between the
    /// send and the report — and a failed append are both logged and
    /// dropped here, because the alternative is an adapter deciding what to
    /// do about the core's bookkeeping. The consequence is stated rather
    /// than hidden: the message is then unrecorded, so a member's reply to
    /// it lands quoteless, exactly as a reply to a message sent before this
    /// unit does.
    pub async fn report_delivery(&self, delivery: DeliveryHandle, origins: &[String]) {
        if let Err(error) = delivery::record(self.ctx.store(), delivery, origins).await {
            tracing::warn!(
                conversation_id = delivery.conversation_id(),
                delivered = origins.len(),
                %error,
                "the delivery was not fully recorded; a reply to an unrecorded message lands quoteless"
            );
        }
    }

    /// Record one join event, past the authorization gate (unit 36,
    /// 2026-08-29): one block per joiner, all under the event's own shared
    /// origin, in the group's conversation.
    ///
    /// The mapping resolution sits under the stamp lock for the reason the
    /// note path states: the loser of a creation race must write into the
    /// winner's conversation, not into its own empty one. The appends stay
    /// inside the lock, so one event's joiners land as one contiguous run
    /// instead of interleaving with another writer's blocks — and the
    /// event's own redelivery check is read inside it, so of two deliveries
    /// of one service message the second reads the first's stored rows
    /// instead of racing them.
    ///
    /// A join is a first activity like any other, so the conversation it
    /// may have just created carries the current palette the same way an
    /// ingested message's and a pin's do: the palette supersession runs
    /// here, past the mapping and ahead of the appends, exactly as the
    /// note path runs it.
    ///
    /// Both transports promise at-least-once delivery, so one service
    /// message arrives twice whenever an acknowledgment is lost. The event
    /// origin is the platform's own id for it, shared by every joiner, so
    /// a stored notice under that origin means the event is recorded and
    /// the redelivery stores nothing at all — never a second block, never a
    /// second principal refresh. Nothing else is skipped: the palette above
    /// already ran, and a redelivery is not a new fact.
    async fn record_join_event(
        &self,
        tx: &StoreTx,
        channel: &ChannelKey,
        joiners: &[JoinedMember],
        origin: &str,
        joined_at: &str,
    ) -> Result<(), CoreError> {
        let _one_stamp_at_a_time = self.sessions.stamp_lock().lock().await;
        let conversation_id = match mapping::find(tx, channel).await? {
            Some((existing, _)) => existing,
            None => {
                self.sessions
                    .map_new_channel(channel, ChannelKind::Group)
                    .await?
                    .conversation_id
            }
        };
        self.reconcile_palette(conversation_id).await?;
        if join::event_recorded(tx, conversation_id, origin).await? {
            tracing::debug!("the join event is already recorded; the redelivery stores nothing");
            return Ok(());
        }
        for joiner in joiners {
            self.record_join(
                tx,
                conversation_id,
                &channel.adapter,
                joiner,
                origin,
                joined_at,
            )
            .await?;
        }
        Ok(())
    }

    /// Record ONE joiner of one join event — or record nothing at all
    /// (unit 36, 2026-08-29).
    ///
    /// The suppression flag is consulted first, through the READ-ONLY
    /// identity lookup, before anything is resolved or refreshed: the
    /// processing record promises that collection stops with the flag, and
    /// this unit does not bend that promise to gain a feature. A flagged
    /// joiner's notice is skipped whole — no block, no name, no principal
    /// refresh — and skipped is all it is: no departure, no reaction, no
    /// reply. The group still sees the platform's own join line; the
    /// assistant simply keeps no record. In a mixed event the skip is per
    /// joiner, so the co-joiners' blocks and the shared event stand.
    ///
    /// Past the flag the joiner's principal resolves through the same path
    /// a sender's does — a joiner is a member — and one block lands under
    /// the event's shared origin.
    async fn record_join(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        adapter: &str,
        joiner: &JoinedMember,
        origin: &str,
        joined_at: &str,
    ) -> Result<(), CoreError> {
        let standing =
            identity::find_standing(tx, adapter.to_owned(), joiner.identity.external_id.clone())
                .await?;
        if standing.is_some_and(|standing| standing.opted_out) {
            tracing::debug!("a joiner's suppression flag stands; the join is not recorded");
            return Ok(());
        }
        let principal_id =
            identity::resolve_principal(tx, adapter.to_owned(), joiner.identity.clone()).await?;
        self.ctx
            .store()
            .append_consumer_block(
                conversation_id,
                None,
                join::JOIN_NOTICE_KIND,
                JoinNotice::stored_fields(
                    join::RecordedJoiner {
                        principal_id,
                        name: &joiner.name,
                        handle: joiner.identity.username.as_deref(),
                    },
                    origin,
                    joined_at,
                ),
                None,
            )
            .await?;
        Ok(())
    }

    /// The outbound edge for one adapter: a subscription yielding what the
    /// assistant puts on that adapter's channels — words, and since unit
    /// 39 a reaction — each bound to its channel key. Each adapter takes
    /// one edge under its own name and never sees another adapter's
    /// items. Anything already stored when the subscription is taken is
    /// history and stays off it; everything stored afterwards is delivered
    /// at least once, re-read from the ledger — the outbound module's doc
    /// states the exact delivery contract, including the failure notice's
    /// at-most-once nature and the mark's accepted losses.
    ///
    /// Named for what it yields: [`Outbound`], whose arms are a reply of
    /// words and a reaction. It was `replies` while words were the only
    /// arm; a name that promised replies and handed out reactions would
    /// leave every reader to discover the second arm from the type.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if reading the stored state that marks the
    /// history boundary fails.
    pub async fn outbound(
        &self,
        adapter: &str,
    ) -> Result<mpsc::UnboundedReceiver<Outbound>, CoreError> {
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
    /// its [`Self::outbound`] edge.
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
            Arc::clone(self.sessions.erasure_fence()),
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
        // One recognition for the whole write (unit 45, 2026-08-30): the
        // stamp reads whether ANY command was invoked, the delivery reads
        // which, and the suppression exemption reads the privacy family
        // projected out of it.
        let command = commands::recognized(message.command.as_ref());
        let family = command.and_then(privacy::family_of);
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
            command,
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
    /// contact out of every table, not merely unanswered. Answers the
    /// refusal, if any, and nothing else: the conversation this message is
    /// written into is resolved by the entry point INSIDE the stamp lock,
    /// because a mapping read taken out here can be stale by the time the
    /// append runs.
    async fn admit_channel(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
    ) -> Result<Option<IngestOutcome>, CoreError> {
        if let Some((_, stored_kind)) = mapping::find(tx, &message.channel).await?
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
            return Ok(Some(IngestOutcome::Withdraw));
        }
        if message.channel_kind == ChannelKind::Direct && self.direct_chats == DirectChats::Off {
            return Ok(Some(IngestOutcome::Disregarded));
        }
        Ok(None)
    }

    /// The mirror's erasure, run for a triggering deletion command
    /// (2026-08-23): the named record's personal columns and the reply
    /// references pointing at it are nulled through the kinds' own passes
    /// (decision 0085), and every count is traced at info — a destructive
    /// act on stored personal data leaves a record in the default log, and
    /// zero counts name the silent no-ops' shape: a target the store never
    /// held or holds no longer. The caller's ordering reasoning sits at
    /// the call site.
    ///
    /// Two records can carry the named origin, so the mirror asks both
    /// (unit 36, 2026-08-29): the ONE message row under that id, and the
    /// WHOLE join event under it — deleting a join service message removes
    /// the event, not one joiner's part of it. The passes over the held
    /// copies of that id follow exactly when either was present — a target
    /// the store never held leaves the command a full no-op — and they
    /// reach every holder: a replier's stored reply target, a filed
    /// report's stored target, and a placed mark's stored target (unit 39,
    /// 2026-08-30), each of which points at the very record that just went
    /// away and would otherwise keep the deleted id in a row no later
    /// erasure joins on. The composition decides the order and the
    /// condition; each kind owns only its own table's write.
    ///
    /// The whole pass runs behind the filing door, which is the only thing
    /// that orders it against a tool filing a fresh copy of the same id:
    /// this path holds the erasure fence for READING and so does a filing,
    /// so the fence orders nothing between the two. Behind the door a
    /// filing either scans after these nulls — and finds no such message
    /// among the turn's own — or appends before them and is nulled here
    /// with the rest.
    async fn mirror_named_deletion(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        target: &str,
    ) -> Result<(), CoreError> {
        let _one_filing_at_a_time = self.filing_door.lock().await;
        let message_rows = kind::erase_message_named(tx, conversation_id, target).await?;
        let join_rows = join::erase_event_named(tx, conversation_id, target).await?;
        let (reply_references, report_targets, mark_targets) = if message_rows > 0 || join_rows > 0
        {
            (
                kind::erase_reply_references_naming(tx, conversation_id, target).await?,
                report::erase_report_references_naming(tx, conversation_id, target).await?,
                mark::erase_mark_references_naming(tx, conversation_id, target).await?,
            )
        } else {
            (0, 0, 0)
        };
        tracing::info!(
            conversation_id,
            target_rows = message_rows,
            join_rows,
            reply_references,
            report_targets,
            mark_targets,
            "the deletion mirror ran over an administrator's reply command"
        );
        Ok(())
    }

    /// The summons resolution — the ONE place the answering mode enters
    /// the machinery (unit 14): a message summons the assistant when it
    /// addressed it, or when helpful answering evaluates every message.
    /// The literal addressed fact rides beside it (unit 16) — the
    /// adapter's own flag, before the mode folded in — stored with the
    /// stamp for the outbound answer threading alone. Everything past this
    /// resolution — the budget consultation, the stamp, the unlatch, and
    /// every later reader of the stored summons — is mode-free.
    ///
    /// A BOT sender is summoned if and only if it addressed the assistant
    /// (2026-08-30): the mode's clause is for people. An automated account
    /// says a great deal without meaning any of it — a captcha prompt, a
    /// join announcement — and none of it is an ask, so the mode that
    /// evaluates every message evaluates none of theirs. Their messages
    /// are recorded and projected exactly as before: what changes is that
    /// they trigger nothing.
    fn resolved_summons(&self, message: &InboundMessage) -> kind::Summons {
        let summoned_by_mode = self.answering == AnsweringMode::Helpful && !message.sender.bot;
        kind::Summons {
            summoned: message.addressed || summoned_by_mode,
            literal_addressed: message.addressed,
        }
    }

    /// The debt already owed behind this message — the tail its stamp is
    /// composed against, read under the stamp lock: the conversation's own
    /// owing tail
    /// ([`Self::owing_tail_debt`]) for every message a turn may open for,
    /// and nothing at all for an unsummoned bot message (2026-08-30). A
    /// selector and nothing else: it reads one value for the write about to
    /// happen and writes nothing itself.
    ///
    /// The stamp composes `answer_due` as own debt OR the owing tail, so a
    /// bot's plain message appended while the tail owes would open a turn
    /// on someone else's unanswered ask — a bot triggering the assistant
    /// through a message it never addressed. Withholding the tail here
    /// stamps that row false outright, and the debt behind it stays owed:
    /// the walk reads through a false-stamped live row
    /// ([`kind::newest_block_id_past_transparent`] and the tail condition
    /// above), so the next message that may carry the debt — anyone's, or
    /// a bot's with the mention — opens the turn with it intact.
    async fn owed_tail(
        &self,
        conversation_id: i64,
        message: &InboundMessage,
        summons: kind::Summons,
    ) -> Result<Option<kind::TailDebt>, CoreError> {
        if message.sender.bot && !summons.summoned {
            return Ok(None);
        }
        self.owing_tail_debt(conversation_id).await
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
            self.sessions.binding(),
            self.sessions.reasoning(),
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
                        let fence = Arc::clone(self.sessions.erasure_fence());
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

    /// What one recognized command answers, and what it did to the
    /// channel's session — the delivery half of the ingestion, run after
    /// the message row stands so a transiently failed append spends no
    /// window and a granted change never applies into recorded silence.
    ///
    /// The audience reading decides the answer, not only who is told about
    /// the command: a command invoked below its floor, or in a kind of
    /// channel it does not serve, is recognized, stamped and answered with
    /// silence.
    ///
    /// The caller holds the erasure fence shared and the stamp lock, which
    /// is what lets the two session resets swap a channel's conversation
    /// from here with no ingestion halfway through one.
    async fn command_answer(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
        recorded: RecordedRow,
        sender: WritingSender,
        notice_admitted: bool,
    ) -> (Option<DeliveryItem>, ChannelReset) {
        let WritingSender {
            principal_id,
            authority,
            command,
            ..
        } = sender;
        let RecordedRow {
            conversation_id,
            block_id,
        } = recorded;
        let offered = command.filter(|command| command.offered(message.channel_kind, authority));
        match offered {
            Some(Command::Wipe) => {
                self.reset_reply(principal_id, Command::Wipe, async {
                    Ok(
                        match self
                            .sessions
                            .wipe(conversation_id, &message.channel, message.channel_kind)
                            .await?
                        {
                            // The fresh conversation carries none of the
                            // channel's standing observations, so the
                            // adapter is told to forget what it looked up
                            // for the old session.
                            WipeOutcome::Replaced => {
                                Some((commands::WIPE_DONE, ChannelReset::Replaced))
                            }
                            // A lost claim made nothing: the fresh
                            // conversation is gone and the channel holds
                            // whatever the racer left it with. Answering
                            // the done line would report a replacement this
                            // command did not make, and firing the
                            // directive would make the adapter forget for a
                            // session that never arrived.
                            WipeOutcome::ClaimLost => None,
                        },
                    )
                })
                .await
            }
            Some(Command::Compact) => {
                self.reset_reply(principal_id, Command::Compact, async {
                    Ok(
                        match self
                            .sessions
                            .compact(
                                conversation_id,
                                &message.channel,
                                message.channel_kind,
                                CompactTrigger::Command {
                                    invoking_row: block_id,
                                },
                            )
                            .await?
                        {
                            CompactOutcome::AlreadyCompact => {
                                Some((commands::COMPACT_ALREADY, ChannelReset::Kept))
                            }
                            // The fork keeps the newest context notes, so
                            // the channel's standing observations cross
                            // with it and the adapter has nothing to
                            // forget.
                            CompactOutcome::Compacted => {
                                Some((commands::COMPACT_DONE, ChannelReset::Kept))
                            }
                            // A lost claim trimmed nothing: the fork was
                            // dropped, and the surviving session is the
                            // racer's doing, not this command's.
                            CompactOutcome::ClaimLost => None,
                        },
                    )
                })
                .await
            }
            // Every other recognized command is the privacy family's, read
            // through the family's own projection so the mapping between
            // the catalogue and the family stays recorded once. The split
            // there is total: the notice keeps its channel-keyed answer,
            // the rights commands take the per-person reply path.
            Some(command) => (
                match privacy::family_of(command) {
                    Some(PrivacyCommand::Notice) if notice_admitted => {
                        self.notice_answer(conversation_id).await
                    }
                    Some(PrivacyCommand::SelfService(rights)) => {
                        self.rights_reply(tx, rights, principal_id).await
                    }
                    Some(PrivacyCommand::Notice) | None => None,
                },
                ChannelReset::Kept,
            ),
            None => (None, ChannelReset::Kept),
        }
    }

    /// One session-reset command's reply, on the resets' own per-person
    /// window (unit 45, 2026-08-30), through the same
    /// grant-exactly-with-the-change operation the rights commands ride: a
    /// withheld reply withholds the reset, so a flood never resets a
    /// session into recorded silence, and a reset that failed hands its
    /// grant back before it is logged and answered with nothing.
    ///
    /// The silence claims nothing about atomicity, and the claim would be
    /// false: the swap is several store calls. The sweep itself is one
    /// transaction, so no fork is ever half-swept; what is left is the
    /// fork-then-claim window the creation race already has, where a
    /// failure can leave a fork nothing points at — harmless, never
    /// cleaned — or a channel with no mapping, which the adapter's
    /// redelivery of the unacknowledged update converges on at the next
    /// attempt. The failure log says that instead of promising nothing
    /// moved.
    ///
    /// The change answers what to say AND what the adapter must forget, so
    /// the directive is decided by the operation that made it and not
    /// re-derived from the command afterwards. It answers `None` when the
    /// reset made nothing of its own — a mapping claim lost to a concurrent
    /// racer — and then the chat hears nothing and no directive fires: the
    /// surviving session is the racer's, and this command has nothing to
    /// report as its own. That case is not a failure, so the grant stays
    /// spent, and the record of it is the reset's own warn log where the
    /// claim was lost.
    async fn reset_reply(
        &self,
        principal_id: i64,
        command: Command,
        change: impl Future<Output = Result<Option<(&'static str, ChannelReset)>, CoreError>>,
    ) -> (Option<DeliveryItem>, ChannelReset) {
        match self.reset_replies.grant_with(principal_id, change).await {
            Some(Ok(Some((line, reset)))) => {
                (Some(DeliveryItem::CommandAnswer(line.to_owned())), reset)
            }
            // Two silences of one shape: a claim lost to a racer, where
            // this command made nothing to report, and an exhausted window,
            // where the reply was withheld and its change with it. Neither
            // says anything and neither fires a directive.
            Some(Ok(None)) | None => (None, ChannelReset::Kept),
            Some(Err(error)) => {
                tracing::warn!(
                    principal_id,
                    invocation = command.invocation(),
                    %error,
                    "the session reset failed partway; the chat hears nothing, and what stands is in the log above"
                );
                (None, ChannelReset::Kept)
            }
        }
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
            stored.is_some_and(|tools| tools.as_deref() == Some(self.sessions.palette()));
        if !already_current {
            self.ctx
                .store()
                .append_consumer_block(
                    conversation_id,
                    None,
                    TOOL_PALETTE_KIND,
                    ToolPalette::stored_fields(self.sessions.palette()),
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
    /// the deletion mirror), and so are live chat rows whose stored stamp
    /// is false (2026-08-30) — both readings live on the kind's own
    /// [`ChatMessage::transparent_to_the_walk`], because both are shapes,
    /// not kinds, and the kind's query skips them for every caller. Why a
    /// false-stamped row is safe to read through is argued once there,
    /// split by the two classes of row it covers: the rows whose stamp was
    /// composed against the tail this read hands back, which certify the
    /// frontier behind them, and the one row composed against no tail at
    /// all — an unsummoned bot's ([`Self::owed_tail`]), which certifies
    /// nothing and whose buried debt this widening exists to preserve.
    /// A framework date record is
    /// transparent under a rule of its own, [`kind::NEVER_ANSWERABLE`]: it
    /// is the calendar, not a voice, and it can stand anywhere the walk looks
    /// — interposed above the owing message it rides in front of, or, on
    /// the framework's own fork and empty-append paths, as the tail itself.
    /// Both places are read through here, from that one recording: the tail
    /// condition below admits it, and the query behind excludes it for
    /// every caller. The framework's other transparent kinds, the
    /// turn-closure markers above all, stay a settled tail here: the
    /// framework's own walk governs turn liveness,
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
            || NEVER_ANSWERABLE.contains(&tail.block_type.as_str())
            || matches!(
                AssistantKind::from_block(&tail),
                AssistantKind::ChatMessage(message) if message.transparent_to_the_walk()
            );
        let tail = if transparent {
            match kind::newest_block_id_past_transparent(
                &store.tx(),
                conversation_id,
                DEBT_READ_THROUGH,
            )
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
                | AssistantKind::JoinNotice(_)
                | AssistantKind::Report(_)
                | AssistantKind::Delivered(_)
                | AssistantKind::MessageMark(_),
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
/// principal, the delivered authority, the command the message invokes with
/// the privacy family it projects to, and whether the suppression flag
/// stands — the facts [`Assistant::resolve_writing_sender`] hands the stamp,
/// the stored fields and the delivery.
#[derive(Debug, Clone, Copy)]
struct WritingSender {
    principal_id: i64,
    authority: Authority,
    /// The catalogue command the message invokes, if any — the one
    /// recognition of the write. The stamp reads whether it is present at
    /// all; the delivery reads which one it is.
    command: Option<Command>,
    /// The privacy family member the command projects to, if any: the
    /// suppression exemption's own reading.
    family: Option<PrivacyCommand>,
    /// The standing suppression flag: `true` only on an exempt command,
    /// since every other suppressed message was dropped before this. The
    /// stored fields read it — a suppressed sender's command records no
    /// speaker, so after a deletion no command re-materializes the emptied
    /// handle.
    suppressed: bool,
}

/// Where one ingested message landed: the conversation it was written into,
/// resolved under the stamp lock, and the id of its own recorded row.
///
/// The two travel together because the delivery half needs both and neither
/// answers the other's question: a `/compact` acts on the conversation and
/// counts against the row, and a reading that inferred the row from the
/// conversation's tail would be assuming exactly what this states.
#[derive(Debug, Clone, Copy)]
struct RecordedRow {
    conversation_id: i64,
    block_id: i64,
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
