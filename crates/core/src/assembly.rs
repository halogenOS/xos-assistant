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
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use agent_ledger::agency::{LeafKind, ToolChoice};
use agent_ledger::providers::ReasoningLevel;
use agent_ledger::store::{ProviderInstance, StoreTx};
use agent_ledger::{
    Block, BlockKind, CoreEvent, EventBus, FromBlock, ProviderRegistry, Role, RuntimeContext,
    Store, ToolCallResult, spawn_reactor,
};
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::acknowledgment;
use crate::commands::{self, Command};
use crate::compaction::ContextWatch;
use crate::composing;
use crate::contract::{self, ContractNotice};
use crate::erasure::{self, ErasureOutcome};
use crate::error::{CoreError, FatalExit};
use crate::filing::{self, FilingDoor};
use crate::join::JoinNotice;
use crate::kind::{
    self, AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage, NEVER_ANSWERABLE,
};
use crate::message::{
    Authority, ChannelKey, ChannelKind, ChannelReset, ComposingUpdate, DeliveryHandle,
    DeliveryItem, InboundMessage, IngestOutcome, IngestReceipt, JoinedMember, Observation,
    ObserveOutcome, ObservedDelivery, ObservedFact, Outbound, SendOutcome,
};
use crate::note::{self, ContextNote, NoteTopic};
use crate::outgoing;
use crate::privacy::{PendingDeletions, PrivacyCommand, RightsCommand};
use crate::quoting;
use crate::retention::{self, RetentionConfig};
use crate::session::{CompactOutcome, InheritedRows, SessionCoordination, Sessions, WipeOutcome};
use crate::streams::StreamObserver;
use crate::tools::ToolSet;
use crate::tools::changelog::HarnessChangelog;
use crate::tools::mark::{self, MarkTool};
use crate::tools::no_reply_needed::NoReplyNeeded;
use crate::tools::reply::ReplyMessage;
use crate::tools::report::{self, ReportTool};
use crate::tools::rights::PrivacyTool;
use crate::tools::runtime::RuntimeFacts;
use crate::tools::search::{SearchConfig, WebSearch};
use crate::tools::send::SendMessage;
use crate::tools::standing::StandingLookup;
use crate::tools::work_is_done::WorkIsDone;
use crate::window::{
    ACKNOWLEDGMENT_WINDOW, LineWindow, PRIVACY_REPLY_CAP, PRIVACY_REPLY_WINDOW, RESET_REPLY_CAP,
    RESET_REPLY_WINDOW, ReplyWindow,
};
use crate::{
    authorization, delivery, identity, join, lineage, mapping, mirror, outbound, privacy, session,
    streams,
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
    // The recorded tool choice (unit 52, 2026-09-01): the delta append
    // lands on a conversation's first activity per process, at whatever
    // point its history had reached — including behind an ask nobody has
    // answered yet. That ask still owes its turn.
    TOOL_CHOICE_KIND,
    // The contract notice (unit 55, 2026-09-02): appended beside the tool
    // choice above it, at the same arbitrary moment and for the same
    // reason — a conversation's first activity per process, which can land
    // behind an ask nobody has answered yet.
    contract::CONTRACT_NOTICE_KIND,
    // The outgoing message (unit 55, 2026-09-02): a sending tool writes it
    // INTO a live turn's window, so a message absorbed while the send was
    // in flight must still summon its own turn.
    crate::outgoing::OUTGOING_MESSAGE_KIND,
    // The kind unit 52 withdrew, spelled as the literal string a previous
    // build stored, because no kind of this assistant claims it any more.
    // The withdrawal drops the content table and the registry row; the
    // header rows in a database that build wrote are ledger history, which
    // nothing deletes, and history stays transparent to this walk. Without
    // the entry such a row parses as an unknown kind, and an unanswered ask
    // directly behind one would stop owing its turn.
    "tool_palette",
    report::REPORT_KIND,
    // The join notice (unit 36, 2026-08-29): the observation path appends
    // it whenever someone walks in, which is exactly the arbitrary moment
    // this list exists for — a member's unanswered question behind a run
    // of joins still owes its turn.
    join::JOIN_NOTICE_KIND,
    // The delivery receipt (unit 38, 2026-08-30): the adapter reports one
    // the moment the platform takes a message, so a receipt can land at
    // the tail behind anything. Crucial on day one — a deterministic
    // answer's receipt can sit AT THE TAIL with no answer block, and an
    // opaque tail there would bury the standing question behind it.
    delivery::DELIVERED_KIND,
    // The message mark (unit 39, 2026-08-30): a reaction is placed
    // precisely on turns that answer nothing, so its block is the ledger
    // tail more often than a report's is. Without this entry a member's
    // unanswered question behind a run of reactions would stop owing its
    // turn.
    mark::MESSAGE_MARK_KIND,
    // The retraction (unit T4, 2026-08-31): appended by the deletion
    // command's own path, ahead of the command row and therefore behind
    // whatever the conversation was already owing. A debt standing behind
    // one still owes its turn.
    delivery::RETRACTION_KIND,
];

/// The recorded tool choice's stored type string, read from the framework's
/// own kind so the two spellings cannot drift.
const TOOL_CHOICE_KIND: &str = ToolChoice::KINDS[0];

/// The most conversations the tool-choice reconciliation memory holds. Past
/// the cap the memory is cleared whole — the established memory-cap shape:
/// it only suppresses repeat comparisons, so losing it costs one bounded
/// choice read per conversation, while an unbounded set would grow with
/// every direct chat the process ever saw.
const CHOICE_MEMORY_CAP: usize = 4096;

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
    /// How many tokens the model's context window holds, as the deployment
    /// configured it (unit 48, 2026-08-31). No provider reports it, so it
    /// is a stated fact of the binding like the model's own name.
    ///
    /// Absent keeps BOTH compaction thresholds silent: the trigger never
    /// fires blind, and a deployment that has not said how big its window is
    /// gets the two explicit doors into the mechanism and no automatic one.
    pub context_window: Option<NonZeroU32>,
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
/// its turn without calling a sending tool. A deployment that wants the
/// quiet shape sets `addressed`.
/// The mode enters the machinery at exactly one place: the entry point's
/// summons resolution ahead of the write-time stamp — everything past the
/// stamp reads the stored summons fact and stays mode-free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AnsweringMode {
    /// Every group message summons a turn; the model decides whether to
    /// speak and stays silent by ending its turn without sending anything:
    /// its written text reaches nobody, so silence is a turn that files no
    /// message.
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
    /// How long a conversation may lie untouched before the retention sweep
    /// deletes it (unit 53, 2026-09-02). An absent span runs no sweep at
    /// all: nothing expires, and the assembly spawns no task for it.
    pub retention: RetentionConfig,
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
    /// delta mechanism removes it from conversations that had it.
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
    /// reasoning level, the composed system prompt and the tool choice
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
    /// The signal an unattended path raises when its failure is the whole
    /// process's: read through [`Assistant::cannot_serve`], which the binary
    /// waits on beside the termination signal. Shared with both paths here
    /// that have no caller to fail — the compaction driver and the retention
    /// sweep — because each of them would otherwise meet a refused statement
    /// again on its next wake and never get past it.
    fatal: Arc<FatalExit>,
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
    /// What the compaction thresholds and their timing read (unit 48,
    /// 2026-08-31): the observer's per-turn measurements, the inbound
    /// activity this entry point records, and the configured window size.
    /// Shared with the compaction driver, which holds it weakly.
    context: Arc<ContextWatch>,
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
    /// The observation race's test seam; unset in production. A
    /// write-once cell, the session's own seam shape: a seam is installed
    /// before the assembly serves anything, so it needs no reach past a
    /// shared handle and never changes under a running conversation.
    note_read_pause: OnceLock<ScriptedPause>,
    /// The suppression race's test seam, run between the pre-lock standing
    /// read and the stamp lock; unset in production, write-once like the
    /// seam above it.
    standing_read_pause: OnceLock<ScriptedPause>,
    /// The conversations whose recorded tool choice this process already
    /// compared against the registered set — the once-per-process memory
    /// of the on-delta supersession (decided 2026-08-23), bounded by
    /// [`CHOICE_MEMORY_CAP`] and cleared whole at the cap. Guarded by the
    /// stamp lock's serialization: every reader holds it.
    choice_reconciled: Mutex<HashSet<i64>>,
    /// Orders the deletion mirror's nulls against the tools that file a
    /// block naming a message origin — the same door those tools take
    /// around their own scan-then-append, because the fence cannot order
    /// them: the mirror runs under this path's SHARED fence hold and a
    /// filing takes the fence shared too. The whole contract, and the lock
    /// order this path obeys, are on [`crate::filing`].
    filing_door: FilingDoor,
    /// Where the receipt door tells every composing edge that a send is
    /// DONE (unit 55, 2026-09-02), whichever way it ended — delivered,
    /// failed, or cut short partway. The sending tools hold the same
    /// channel for the calls they refuse before filing anything. Its whole
    /// contract is on [`composing::SendStops`].
    send_stops: composing::SendStops,
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
    /// The composing cue's stop channel, for the sending pair: a call
    /// refused before it filed anything ends the cue its start lit.
    send_stops: composing::SendStops,
}

/// Add the assembly's own tools to the embedder's set, in one place: each
/// conditional registration takes exactly the predicate the prompt
/// composition took, so the prompt can never teach a tool the conversation
/// does not have, and the delta mechanism removes an unconfigured tool from
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
/// - The two SENDING tools join unconditionally (unit 55, 2026-09-02), and
///   they have to: from this unit on they are the ONLY way the model's
///   words reach a chat, so an assembly missing one would be an assistant
///   that cannot speak. They validate a reply target against stored
///   origins, so they take the erasure fence, and they file against those
///   origins, so they take the same filing door the report and the reaction
///   take.
fn admit_assembled_tools(tools: &mut ToolSet, assembled: AssembledTools) {
    let AssembledTools {
        moderation_handle,
        answering,
        web_search,
        pending_deletions,
        privacy_replies,
        erasure_fence,
        filing_door,
        send_stops,
    } = assembled;
    if let Some(handle) = moderation_handle
        && crate::teaching::moderation_taught(true, answering)
    {
        tools.admit(ReportTool::new(
            handle,
            Arc::clone(&erasure_fence),
            Arc::clone(&filing_door),
        ));
    }
    if let Some(search) = web_search {
        tools.admit(WebSearch::new(
            search,
            crate::tools::search::DEFAULT_TIMEOUT,
        ));
    }
    tools.admit(StandingLookup::new(Arc::clone(&erasure_fence)));
    tools.admit(MarkTool::new(Arc::clone(&erasure_fence), filing_door));
    tools.admit(SendMessage::new(
        Arc::clone(&erasure_fence),
        send_stops.clone(),
    ));
    tools.admit(ReplyMessage::new(Arc::clone(&erasure_fence), send_stops));
    tools.admit(PrivacyTool::new(
        pending_deletions,
        privacy_replies,
        erasure_fence,
    ));
}

/// The tools no assembly is without (unit 32, unit 47, unit 54): the
/// runtime facts and the harness changelog join unconditionally, because
/// neither has a configuration to be absent — the questions they answer are
/// asked wherever the assistant runs, the one value the changelog states is
/// embedded in the build, and a build that passed no changelog gets a tool
/// that answers that absence, not a tool that vanishes.
///
/// The two TURN-ENDING tools join on the same terms (unit 54, 2026-09-02):
/// a turn that was asked nothing and a turn whose actions are the whole
/// answer happen wherever the assistant runs, and neither tool reads or
/// writes anything an assembly would have to hand it. They are registered
/// here and never in the lookups set, which answers what lookups ship and
/// nothing else.
///
/// The runtime-facts tool takes the binary's start instant here, the one
/// fact it cannot reach for itself. The model is not injected: that one
/// belongs to the conversation being answered, which the tool has in hand
/// at the call, and the binding decides it only for the conversations
/// this assembly goes on to create.
fn admit_unconditional_tools(tools: &mut ToolSet, started_at: Instant) {
    tools.admit(RuntimeFacts::new(started_at));
    tools.admit(HarnessChangelog::new());
    tools.admit(NoReplyNeeded::new());
    tools.admit(WorkIsDone::new());
}

/// The release lookup's own rate window (the operator's numbers, decision
/// 0169): bound before the context is shared, and only when the embedder's
/// set registered the tool — the builder refuses a name nothing registered,
/// and a test assembly with no lookups is not misconfigured.
fn with_release_window(
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    tool_names: &[String],
) -> RuntimeContext<AssistantKind, CoreEvent> {
    if tool_names
        .iter()
        .any(|name| name == crate::tools::release::NAME)
    {
        ctx.with_tool_window(
            crate::tools::release::NAME,
            crate::tools::release::WINDOW_CALLS,
            crate::tools::release::WINDOW_SECONDS,
        )
    } else {
        ctx
    }
}

/// Whether a recorded tool choice already names BOTH sending tools — the
/// contract notice's one condition beyond the delta (unit 55, 2026-09-02).
///
/// A record naming both is a conversation that already had the tools, and
/// therefore already ran under this contract: nothing to explain. A record
/// naming one of them and not the other is a conversation whose model could
/// not speak the way this build speaks, so it reads as pre-contract too —
/// the honest reading of a half-set that no build ever registered. Which
/// names those are is the sending pair's own enumeration, so this reading
/// and the typing cue's cannot part company.
fn names_the_sending_tools(recorded: &[String]) -> bool {
    crate::tools::sending::NAMES
        .iter()
        .all(|sending| recorded.iter().any(|name| name == sending))
}

/// Whether a recorded tool choice already names the registered set, read
/// as a SET and not as a sequence.
///
/// This assembly writes its own records sorted, but it is no longer the
/// only writer: the framework appends choices of its own at the compaction
/// forks, and a record this assembly did not write owes it no order. An
/// ordered comparison would read a permutation as a delta and append a
/// duplicate of what the ledger already says, on every process that ever
/// serves that conversation. Duplicate names are compared too — sorted
/// copies, not two membership tests — so a record naming one tool twice
/// stays a delta against a registered set that names it once.
fn names_the_same_set(recorded: &[String], registered: &[String]) -> bool {
    if recorded.len() != registered.len() {
        return false;
    }
    let mut recorded: Vec<&str> = recorded.iter().map(String::as_str).collect();
    let mut registered: Vec<&str> = registered.iter().map(String::as_str).collect();
    recorded.sort_unstable();
    registered.sort_unstable();
    recorded == registered
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
            retention,
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
        // The two shared handles the tools are registered with: the filing
        // door, and the composing cue's one stop channel — the sending
        // tools and the receipt door both say a send is over on it.
        let (filing_door, send_stops) = (filing::door(), composing::stops());
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
                send_stops: send_stops.clone(),
            },
        );
        admit_unconditional_tools(&mut tools, started_at);
        // One source for what tools exist: the registry the runtime resolves
        // calls against and the tool choice every new conversation records
        // are both derived from the set right here.
        let (registry, tool_names) = tools.into_registry();
        // Title derivation is switched off for good (decision 0077): nobody
        // reads a group chat's derived title, so no conversation excerpt is
        // ever sent anywhere for naming — zero title requests by
        // construction, not by configuration.
        let ctx: RuntimeContext<AssistantKind, CoreEvent> =
            RuntimeContext::new(store, bus, providers, Arc::new(registry))
                .without_title_derivation();
        let ctx = with_release_window(ctx, &tool_names);
        let streams = streams::spawn_observer(ctx.bus());
        // The one home of the compaction readings, built over the observer
        // that already consumes the bus rather than a second subscriber
        // seeing the same events.
        let context = Arc::new(ContextWatch::new(
            Arc::clone(&streams),
            binding.context_window,
        ));
        // The two holds are handed to the sessions and kept there: every
        // path in this assembly takes them back through it, so neither has
        // a second home to drift from.
        let sessions = Arc::new(Sessions::new(
            ctx.clone(),
            binding,
            reasoning,
            system_prompt,
            tool_names,
            SessionCoordination {
                stamp_lock: Arc::new(Mutex::new(())),
                erasure_fence,
                context: Arc::clone(&context),
            },
        ));
        // What an unattended path raises instead of failing a caller, and
        // what the binary waits on: the driver below has no message to
        // leave unacknowledged, so a fatal failure there ends the process
        // through this.
        let fatal = Arc::new(FatalExit::new());
        // The compaction's two unattended doors, beside the stream observer
        // and on the same broadcast: a turn the framework ended over a spent
        // tool-call window has left the conversation in the shape the
        // mechanism exists to clear, and a conversation running out of
        // context window needs clearing whether or not anyone notices.
        session::spawn_compaction_driver(&sessions, ctx.bus(), &context, &fatal);
        // The retention rule's own task, beside the compaction driver and
        // deliberately not inside it: the driver's tick is thirty seconds of
        // monotonic time serving context pressure, and retention is
        // wall-clock days. A deployment that configured no span gets no task
        // here at all.
        // The handle is dropped: the task ends with the assembly, or with the
        // exit signal it raises on a fatal failure of its own.
        drop(retention::spawn_sweep(&sessions, retention, &fatal));
        spawn_reactor(ctx.clone());
        Ok(Self {
            ctx,
            sessions,
            fatal,
            answering,
            name,
            disclosure,
            streams,
            context,
            protection,
            operators,
            direct_chats,
            privacy_policy_address,
            notice_answered: LineWindow::new(ACKNOWLEDGMENT_WINDOW),
            privacy_replies,
            reset_replies: ReplyWindow::new(RESET_REPLY_WINDOW, RESET_REPLY_CAP),
            pending_deletions,
            note_read_pause: OnceLock::new(),
            standing_read_pause: OnceLock::new(),
            choice_reconciled: Mutex::new(HashSet::new()),
            filing_door,
            send_stops,
        })
    }

    /// Resolves when this assembly can no longer serve anything: a failure
    /// on a path with no caller reached a class no retry gets past, so the
    /// process has to end and the supervisor has to start a replacement over
    /// the durable state.
    ///
    /// The per-message paths need nothing like this. Their fatal failures
    /// travel as [`CoreError`] to the intake that asked, and the intake ends
    /// its run with the message unacknowledged. The compaction's unattended
    /// doors have nobody to answer, and a failure their next wake would
    /// simply repeat is what this states instead. What it was is in the log
    /// line raised where it happened; nothing rides on this signal but the
    /// fact.
    ///
    /// It resolves once and then always: a raise before anybody waits is
    /// answered by the next wait.
    pub async fn cannot_serve(&self) {
        self.fatal.raised().await;
    }

    /// Install the observation race's test seam: the given pause runs
    /// between the on-delta newest-note read and its append, inside the
    /// stamp lock — which is exactly why a suite can prove the lock holds
    /// the read-then-append window. Production never calls this.
    pub fn pause_between_note_read_and_append(&self, pause: ScriptedPause) {
        let _ = self.note_read_pause.set(pause);
    }

    /// Install the suppression race's test seam: the given pause runs
    /// between the pre-lock standing read and the stamp lock, which is
    /// exactly the window a peer ingestion's flag write can land in — so a
    /// suite proves the under-lock re-read drops the racing message.
    /// Production never calls this.
    pub fn pause_between_standing_read_and_append(&self, pause: ScriptedPause) {
        let _ = self.standing_read_pause.set(pause);
    }

    /// Install the reset claim race's test seam: the given pause runs
    /// between a session reset's mapping delete and its claim, which is the
    /// window a concurrent racer takes the channel in — so a suite proves
    /// what a reset that lost the claim answers. Production never calls
    /// this.
    pub fn pause_between_reset_delete_and_claim(&self, pause: ScriptedPause) {
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
    /// stamp lock get taken, the tool-choice reconcile — and under the stamp
    /// lock the standing is read once more for a non-command message
    /// (2026-08-23), because a peer ingestion's flag write is serialized
    /// under that very lock and can land after the pre-lock read: the
    /// re-read drops the racing message before its append, so the flag
    /// suppresses from the moment it stands. Beside it, and still ahead of
    /// the tool-choice reconcile, run a revision's two drops (unit T3,
    /// 2026-08-31): a message revising one whose newest recorded version
    /// carries the same text, and one revising a message the store holds
    /// no version of, are both disregarded with nothing written. The same
    /// one read settles the author invariant — a reviser who did not write
    /// the version the store holds records an ordinary new message, with no
    /// revision reference stored. All of it reads through one under-lock
    /// reading, behind the one privacy family exemption the drops share.
    /// Past those runs the deletion mirror (2026-08-23) — behind the
    /// suppression drop and the direct-channel admission by this very
    /// order, ahead of the tail read
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
        // Named without an underscore because the compaction and retraction
        // commands below release both holds explicitly before their
        // mechanisms run; every other path holds them to its return.
        let no_erasure_mid_message = self.sessions.erasure_fence().read().await;
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
            deletion,
            family,
            suppressed,
        } = sender;
        if let Some(pause) = self.standing_read_pause.get() {
            pause().await;
        }

        // Held from the tail read and the budget counts through the append:
        // the stamp is decided against the tail this write is appended
        // behind, and the counts must see every earlier taken debt — so no
        // concurrent ingestion may slide a block in between, and two racing
        // messages cannot both take the last budget slot. The lock's whole
        // contract is on the field it lives on, [`Sessions::stamp_lock`].
        let one_stamp_at_a_time = self.sessions.stamp_lock().lock().await;
        let conversation_id = self.conversation_under_lock(&tx, &message).await?;
        // Every under-lock drop, in one reading and behind one exemption,
        // and with it whether this row stores the revision reference the
        // adapter reported — see [`Self::under_lock_reading`]. Placed HERE
        // on purpose: ahead of the tool-choice reconciliation below, which
        // appends a delta block on the conversation's first activity per
        // process, so a drop taken past it would have written a block and
        // still claimed [`IngestOutcome::Disregarded`], whose documented
        // meaning is that nothing touched the ledger.
        let revision_link = match self
            .under_lock_reading(&tx, conversation_id, &message, family, principal_id)
            .await?
        {
            UnderLock::Disregarded => return Ok(IngestOutcome::Disregarded),
            UnderLock::Recorded(link) => link,
        };
        // The tool-choice supersession, on the conversation's first
        // activity per process (decided 2026-08-23): the delta append lands
        // ahead of the message, so this very turn is offered and resolves
        // against the fresh choice.
        self.reconcile_tool_choice(conversation_id).await?;
        // The deletion mirror (decided 2026-08-23), past the suppression
        // and channel admissions on purpose and ahead of the tail read on
        // purpose: an administrator's reply carrying the moderation bot's
        // own deletion command nulls every recorded version of the named
        // message here, inline under this ingestion's erasure-fence read
        // hold — one message's nulls, not the person-wide operation, so no
        // spawn is needed — and behind the
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
        //
        // Recognising the command and acting on it are two readings (unit
        // T3, 2026-08-31). The RECOGNITION was taken with the sender and is
        // what the stamp below reads; the effect this call runs is the
        // narrower question. A deletion command arriving as a revision
        // therefore stays a command — silent, no debt, no budget slot —
        // while nothing is erased and nothing is retracted.
        //
        // One command, two effects, decided by what the reply names (unit
        // T4, 2026-08-31). A reply naming a person's message erases that
        // row here and now. A reply naming one of the assistant's own
        // messages appends the retraction fact here — under this ingestion's
        // holds, where the ledger is serialized — and answers the origins
        // the chat must lose. The FORK that takes the retracted answer out
        // of the model's view runs past the holds, with the compaction, for
        // the reason stated there.
        let retraction = self
            .perform_deletion(&tx, conversation_id, &message, deletion)
            .await?;
        let summons = self.resolved_summons(&message);
        let owing_tail = self.owed_tail(conversation_id, &message, summons).await?;
        let commanded = command.is_some() || deletion.is_some();
        let limited = self
            .limiting_fact(commanded, summons, principal_id, conversation_id)
            .await?;
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
        let fields = recorded_fields(
            &message,
            principal_id,
            authority,
            suppressed,
            stamp,
            revision_link,
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
        //
        // The audience reading decides the answer (unit 45, 2026-08-30): a
        // command invoked below its floor, or in a kind of channel it does
        // not serve, answers SILENCE. No refusal line goes out, because a
        // refusal line advertises a surface the person cannot use — the
        // stamp above already took the debt out of the message.
        let answered = self
            .command_answer(
                &tx,
                &message,
                RecordedRow { conversation_id },
                sender,
                notice_admitted,
                retraction,
            )
            .await;
        if stamp.own_debt_taken() {
            self.ctx
                .bus()
                .emit(CoreEvent::UnlatchRequested { conversation_id });
        }
        // The channel's quiet window measures INBOUND traffic, and this is
        // where inbound traffic is: recorded past the append, so a message
        // that was refused before it reached the ledger does not count as
        // activity on a channel it never joined.
        self.context.record_inbound(conversation_id);
        // The two holds are released HERE, before the mechanisms that
        // replace a session run. Holding the single ingestion lock across a
        // model call would stall every conversation this process serves for
        // that call's whole latency — the rules acknowledgment's own
        // recorded reasoning, and the reason `/compact` cannot be answered
        // from inside the lock the rest of the ingestion needs. The
        // retraction's fork is here for a second reason as well: it re-takes
        // both holds for its own swap, and the compacted case re-takes them
        // through the digest scrub, so calling it under them would deadlock
        // on this very lock.
        drop(one_stamp_at_a_time);
        drop(no_erasure_mid_message);
        let (deliver, reset) = self
            .released_answer(answered, &message, principal_id, conversation_id)
            .await;
        Ok(IngestOutcome::Recorded {
            receipt: IngestReceipt {
                principal_id,
                conversation_id,
            },
            deliver,
            reset,
        })
    }

    /// What the write's answer came to, once the ingestion's holds are
    /// released: an answer already in hand, or the mechanism that still owes
    /// one.
    ///
    /// Both mechanisms here replace a channel's session, and neither may run
    /// under the holds. The compaction takes a model turn, and no hold may
    /// be held across one. The retraction's fork re-takes both holds for its
    /// own swap — through the digest scrub, in the compacted case — so
    /// calling it inside them would deadlock on the ingestion's own lock.
    async fn released_answer(
        &self,
        answered: CommandAnswer,
        message: &InboundMessage,
        principal_id: i64,
        conversation_id: i64,
    ) -> (Option<DeliveryItem>, ChannelReset) {
        match answered {
            CommandAnswer::Settled(deliver, reset) => (deliver, reset),
            CommandAnswer::Compaction => {
                self.compaction_answer(message, principal_id, conversation_id)
                    .await
            }
            CommandAnswer::Retraction(retraction) => {
                self.retraction_answer(retraction, conversation_id).await
            }
        }
    }

    /// `/compact`, run once the ingestion's holds are released: the one
    /// mechanism, with the thresholds and their timing ignored — the person
    /// asked for it now.
    ///
    /// The reply is granted exactly with the compaction, through the resets'
    /// own window, so a withheld reply withholds the operation and a failure
    /// hands its grant back. The line reports what happened because it is
    /// spoken after it happened: the capture, the thread and the swap are
    /// all behind this await.
    async fn compaction_answer(
        &self,
        message: &InboundMessage,
        principal_id: i64,
        conversation_id: i64,
    ) -> (Option<DeliveryItem>, ChannelReset) {
        self.reset_reply(principal_id, Command::Compact, async {
            Ok(
                match self
                    .sessions
                    .compact(conversation_id, &message.channel, message.channel_kind)
                    .await?
                {
                    // The honest answer for a ledger that does not split.
                    // A group's conversation opens with a system prompt and
                    // a tool choice — two groups before anyone speaks — so this
                    // is not a state a served channel reaches; it is the
                    // outcome's answer, not a line the mechanism aims at.
                    CompactOutcome::AlreadyCompact => {
                        Some((commands::COMPACT_ALREADY, ChannelReset::Kept))
                    }
                    // The compacted thread carries the second half of the
                    // ledger, so the channel's standing observations cross
                    // with it and the adapter has nothing to forget.
                    CompactOutcome::Compacted => Some((commands::COMPACT_DONE, ChannelReset::Kept)),
                    // A lost claim compacted nothing: the thread was
                    // dropped, and the surviving session is the racer's
                    // doing, not this command's.
                    CompactOutcome::ClaimLost => None,
                },
            )
        })
        .await
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
    /// creation path a first message does, system prompt and tool choice
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
    /// differs from the one composed now, the channel FORKS: a successor
    /// composed under the current deployment takes the mapping, and the
    /// group's history rides along through the junction, everything past the
    /// prompt the source opened with. A conversation that recorded no prompt
    /// at all has no head to compose a successor past; it is unmapped
    /// instead, and its channel's next message opens a fresh conversation.
    ///
    /// A channel is re-forked for a third reason: its conversation's prompt
    /// is not its FIRST row, whether or not the wording moved. A system
    /// prompt joins a conversation that holds nothing yet and is refused
    /// anywhere else, so a ledger carrying one further in can be neither
    /// compacted nor dispatched, and it is what every conversation forked
    /// before that rule existed carries. The successor is the same
    /// composition the other two reasons take, with the range chosen around
    /// the misplaced row: everything written ahead of it comes across, the
    /// row itself does not, and the fresh prompt is the head. One walk, once,
    /// before anything is served — no message, no model turn, nothing paid
    /// for.
    ///
    /// Nothing is rewritten and nothing is deleted. The old conversation
    /// stays in the ledger exactly as it was — readable, exportable, and
    /// reachable by erasure through the same principal it always was — and
    /// no block is copied: the successor holds the same rows through the
    /// junction. That is the append-only answer to a changed prompt: not a
    /// mutated record, but a new conversation beside the old one.
    ///
    /// What the successor does NOT take is the inherited prompt. The current
    /// one stands in its place, at the head, where it reads first — an
    /// appended one would sit behind the inherited one and be obeyed
    /// unevenly. The rest of what the successor is — the current binding,
    /// the configured reasoning level — is the session module's one
    /// recording of what a fork under the current deployment means. A model
    /// swap is a change of the dispatch itself, and a channel left on the
    /// old binding keeps talking — and billing — through it, which the
    /// operator watched happen before this walk learned to look
    /// (2026-08-29).
    ///
    /// Returns how many channels were retired.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if a mapping read, a block read, a settlement,
    /// the fork or the re-mapping fails.
    pub async fn retire_stale_channels(&self) -> Result<usize, CoreError> {
        let store = self.ctx.store();
        let tx = store.tx();
        let mut retired = 0;
        for record in mapping::all(&tx).await? {
            let blocks = store.list_blocks(record.conversation_id).await?;
            let recorded =
                blocks
                    .iter()
                    .enumerate()
                    .find_map(|(row, block)| match AssistantKind::from_block(block) {
                        AssistantKind::Core(kind::FrameworkKind(BlockKind::SystemPrompt(
                            prompt,
                        ))) => Some((row, prompt.content)),
                        _ => None,
                    });
            // A ledger with blocks but no prompt row among them retires too:
            // it cannot be serving the current wording either, and leaving it
            // mapped would keep that silence permanent. A ledger with no
            // blocks at all is the other case, and the `last` read below is
            // where it is answered.
            let prompt_current = recorded.as_ref().map(|(_, content)| content.as_str())
                == Some(self.sessions.system_prompt());
            // The position is a reason of its own: a prompt anywhere but the
            // head is a shape no door builds any more and no compaction and
            // no dispatch accepts, so the channel takes a successor whatever
            // the wording says.
            let prompt_first = matches!(recorded, Some((0, _)));
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
            if prompt_current && model_current && prompt_first {
                continue;
            }
            let Some(channel) =
                mapping::channel_for_conversation(&tx, record.conversation_id).await?
            else {
                continue;
            };
            // A conversation whose junction holds nothing keeps its mapping:
            // there is no history to carry and no prompt row to lift out, so
            // a successor of it would be another empty conversation and the
            // channel would gain nothing by moving. No door here builds that
            // shape — every one of them writes the prompt before the channel
            // is ever claimed — and the dispatch's head check is what answers
            // for it if a foreign ledger ever brings one.
            let Some(last) = blocks.last().map(|block| block.id) else {
                continue;
            };
            // Fork instead of starting over. The group's conversation is the
            // context every answer is built from, and a prompt edit is not a
            // reason to forget what was said — the successor inherits the
            // history through the junction, so nothing is copied and nothing
            // is lost. Everything else about the fork — the current prompt at
            // its head, and the current model binding, whose reasoning level
            // travels with it into the successor's stored settings — is the
            // session module's one recording of what a fork under the current
            // deployment is.
            //
            // The range is what this walk knows and that module does not:
            // where the old prompt sits, and therefore which rows are the
            // history it is being lifted out of.
            //
            // The two indexed reads below are safe by the arm ORDER and
            // nothing else: `blocks[0]` is reached only where a prompt was
            // found at row 0, so the ledger has that row, and `blocks[row-1]`
            // only after the `Some((0, _))` arm above has taken every row
            // that would underflow. Both indices come from the enumeration
            // over `blocks` itself, so neither can run past its end.
            let inherited = match recorded {
                // The prompt is the head: the history is everything past it.
                Some((0, _)) => InheritedRows::After(blocks[0].id),
                // The prompt sits further in, which is what a fork built
                // before the head rule carries. The rows ahead of it are the
                // history, and the row before it is the last of them.
                Some((row, _)) => InheritedRows::UpTo(blocks[row - 1].id),
                // No prompt at all: there is no row to lift out, so the whole
                // ledger rides across behind the fresh one.
                None => InheritedRows::UpTo(last),
            };
            // The settle, ahead of the fork (unit 55, 2026-09-02): a send
            // this conversation filed and nobody confirmed will never
            // happen now — the channel is about to move to a fresh session
            // and the outbound edge seeds the successor past everything
            // already stored — so the call waiting on it is failed with the
            // sentence naming the retirement instead of being left open on
            // a conversation nothing serves.
            let settled = outgoing::fail_pending_sends(
                store,
                record.conversation_id,
                outgoing::RETIRED_BEFORE_CONFIRMED,
            )
            .await?;
            if settled > 0 {
                tracing::info!(
                    conversation_id = record.conversation_id,
                    settled,
                    "the retiring conversation held unconfirmed messages; they count as unsent"
                );
            }
            let successor = self
                .sessions
                .forked_with_current_prompt(record.conversation_id, inherited)
                .await?;
            mapping::delete_by_conversation(&tx, record.conversation_id).await?;
            mapping::claim(&tx, &channel, record.kind, successor).await?;
            retired += 1;
            tracing::info!(
                conversation_id = record.conversation_id,
                successor,
                prompt_current,
                model_current,
                prompt_first,
                "the recorded prompt, its position or the model is stale; the channel forks and \
                 takes the current ones"
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
                // The tool-choice supersession fires on observed activity
                // too (decided 2026-08-23): a conversation whose next
                // contact is a pin or a title change gains the current tools
                // the same way an ingested message grants them.
                self.reconcile_tool_choice(conversation_id).await?;
                let newest = note::newest_text(self.ctx.store(), conversation_id, topic).await?;
                if let Some(pause) = self.note_read_pause.get() {
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
    /// The adapter reports after every send on either path, whole or cut
    /// short partway: the reported list is exactly what reached the chat,
    /// so an empty list records nothing.
    ///
    /// # It also settles the model's call (unit 55, 2026-09-02)
    ///
    /// A message the model asked for through a sending tool left that call
    /// PENDING, and this is the door that answers it — the one place either
    /// send path passes through, so the record and the settlement cannot
    /// drift apart:
    ///
    /// - a WHOLE send completes the call with the ids the platform assigned,
    ///   which is how the model learns what to name when it answers a
    ///   member replying to its own message;
    /// - a FAILED send fails the call with the adapter's reason, so the
    ///   model learns its words never arrived;
    /// - a CUT-SHORT send fails the call too, with a sentence carrying the
    ///   ids that did post: the message the group read is not the message
    ///   the model wrote, and a member replying to the part that posted
    ///   replies to one of those ids.
    ///
    /// A send nobody asked for — a report's line, a deterministic item —
    /// carries no call on its handle, settles nothing and stops no cue: no
    /// tool call started, so nothing lit one.
    ///
    /// Every one of the three endings also stops the composing cue, on the
    /// one channel every composing edge listens to: the send is done, and
    /// the chat stops showing the assistant typing whether the platform
    /// took the message or not.
    ///
    /// This answers nothing and never fails outward. A conversation that no
    /// longer exists — erasure can delete a direct conversation between the
    /// send and the report — and a failed append are both logged and
    /// dropped here, because the alternative is an adapter deciding what to
    /// do about the core's bookkeeping. The consequence is stated rather
    /// than hidden: the message is then unrecorded, so a member's reply to
    /// it lands quoteless, and a settlement that failed to write leaves the
    /// call open until the next startup sweep fails it.
    pub async fn report_delivery(
        &self,
        delivery: DeliveryHandle,
        origins: &[String],
        outcome: &SendOutcome,
    ) {
        if let Err(error) = delivery::record(self.ctx.store(), delivery, origins).await {
            tracing::warn!(
                conversation_id = delivery.conversation_id(),
                delivered = origins.len(),
                %error,
                "the delivery was not fully recorded; a reply to an unrecorded message lands quoteless"
            );
        }
        let Some(call_block) = delivery.call_block() else {
            return;
        };
        let settlement = match outcome {
            SendOutcome::Whole => ToolCallResult::Success {
                content: outgoing::sent_result(origins),
            },
            SendOutcome::Failed { reason } if origins.is_empty() => ToolCallResult::Error {
                error: outgoing::send_failed(reason),
            },
            SendOutcome::Failed { reason } => ToolCallResult::Error {
                error: outgoing::send_cut_short(origins, reason),
            },
        };
        // The composing cue stops here, whichever way the send ended: this
        // send's own start lit the indicator, and this door is the one
        // place every ending passes through, so one carrier ends every one
        // of them (the operator, 2026-09-02: "it should stop typing when
        // the send is done, regardless of its success"). A send with no
        // listening edge answers an error, which is nothing to act on: the
        // cue is live-only.
        let _ = self.send_stops.send(delivery.conversation_id());
        if let Err(error) = outgoing::settle(
            self.ctx.store(),
            delivery.conversation_id(),
            call_block,
            settlement,
        )
        .await
        {
            tracing::warn!(
                conversation_id = delivery.conversation_id(),
                call_block,
                %error,
                "the send's call could not be settled; it stays open until the next start"
            );
        }
    }

    /// Settle every send this process could not finish (unit 55,
    /// 2026-09-02): before serving, every outgoing block whose call is
    /// still unresolved is FAILED with the restart sentence, in every
    /// mapped conversation.
    ///
    /// A pending send the process died with is not delivered late and never
    /// could be: the outbound edge's startup seed marks everything already
    /// stored as history, so the block would sit undelivered forever while
    /// its call kept the turn open. The trade is decision 0014's, the one
    /// it made for a redelivered update — a possible duplicate over a
    /// possible silence — and the model is told plainly that it may send
    /// again.
    ///
    /// Run BEFORE the edges are taken, so the sweep never races a live
    /// delivery report for the same call; and idempotent through the
    /// framework's own resolution door, so a repeat settles nothing.
    ///
    /// Returns how many calls were settled.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the mapping read, a ledger read or a
    /// settlement write fails.
    pub async fn fail_unfinished_sends(&self) -> Result<usize, CoreError> {
        let store = self.ctx.store();
        let mut settled = 0;
        for record in mapping::all(&store.tx()).await? {
            let failed = outgoing::fail_pending_sends(
                store,
                record.conversation_id,
                outgoing::RESTARTED_BEFORE_CONFIRMED,
            )
            .await?;
            if failed > 0 {
                tracing::info!(
                    conversation_id = record.conversation_id,
                    failed,
                    "the process restarted before the chat confirmed these messages; \
                     they count as unsent"
                );
            }
            settled += failed;
        }
        Ok(settled)
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
    /// may have just created carries the current tools the same way an
    /// ingested message's and a pin's do: the tool-choice supersession runs
    /// here, past the mapping and ahead of the appends, exactly as the
    /// note path runs it.
    ///
    /// Both transports promise at-least-once delivery, so one service
    /// message arrives twice whenever an acknowledgment is lost. The event
    /// origin is the platform's own id for it, shared by every joiner, so
    /// a stored notice under that origin means the event is recorded and
    /// the redelivery stores nothing at all — never a second block, never a
    /// second principal refresh. Nothing else is skipped: the choice above
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
        self.reconcile_tool_choice(conversation_id).await?;
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
    /// states the exact delivery contract, including the mark's accepted
    /// losses.
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
        composing::spawn_edge(self.ctx.clone(), adapter.to_owned(), &self.send_stops)
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
    /// [`ErasureOutcome::NotFound`] — erasing nothing — when no identity
    /// row matches, an unflagged person's completed earlier erasure
    /// included; a flagged person's surviving stub keeps matching, so their
    /// repeat re-runs over emptiness and reports completion (the
    /// idempotency refinement recorded on decision 0012).
    ///
    /// A repeat that reports it still runs the compacted-digest scrub, and
    /// that is the one thing such a repeat does: a scrub whose regeneration
    /// failed leaves the erased person's words standing inside a digest, and
    /// this call is the retry that reaches them. The lineages are read off
    /// the BLOCKS, which keep the principal id the identity row no longer
    /// has.
    ///
    /// A direct conversation showing an open stream — observed on the bus,
    /// or holding a stored streaming tail a gone runtime left behind — is
    /// settled first, per the streams module's protocol: the interrupt goes
    /// out and a bounded stored-state re-read confirms the interrupt's
    /// ledger writes have finished before anything is deleted, so the
    /// stream's appends cannot race the deletion. Past the bound the erasure
    /// fails loudly with [`CoreError::StreamUnsettled`], deleting nothing.
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
    /// [`CoreError::StreamUnsettled`] if an open stream did not settle
    /// before the bound; [`CoreError::Store`] if a read, a write or a
    /// deletion fails.
    pub async fn erase_principal(&self, principal_id: i64) -> Result<ErasureOutcome, CoreError> {
        erase_behind_the_fence(
            Arc::clone(&self.sessions),
            Arc::clone(&self.streams),
            Arc::clone(&self.context),
            principal_id,
        )
        .await
    }

    /// The suppression check and the writing resolution, in the stated
    /// order (decided 2026-08-23): first the READ-ONLY identity lookup — a
    /// select, never a write — so the check precedes every write the
    /// ingestion path can make; then the privacy command family; then, flag
    /// standing, the drop, answered as `None` — no message row, no identity
    /// refresh, no principal write, no conversation creation, no tool-choice
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
    async fn resolve_writing_sender<'a>(
        &self,
        tx: &StoreTx,
        message: &'a InboundMessage,
    ) -> Result<Option<WritingSender<'a>>, CoreError> {
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
            // The moderation bot's token is no catalogue command, so it has
            // its own recognition — taken HERE, beside the catalogue's, so
            // one write asks the message once and the stamp, the mirror and
            // the retraction all read the same answer. It needs the resolved
            // authority, which is why it stands past the refusal above.
            deletion: mirror::recognized_deletion(message, authority),
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

    /// Which fact, if any, limited this write's own debt — the one place
    /// the write's limit is decided.
    ///
    /// The command stamp covers ANY recognized command since unit 45
    /// (2026-08-30), not the privacy family alone: recognition is global
    /// and the audience reading decides only the ANSWER, so a command
    /// invoked where it is not offered still takes no debt, opens no turn
    /// and never unlatches. The moderation bot's deletion token is no
    /// catalogue command, so it reaches this stamp through its own
    /// RECOGNITION — never through whether the mirror acted (unit T3,
    /// 2026-08-31); the caller passes the two recognitions folded into
    /// one.
    ///
    /// Budgets are consulted for summoned non-command messages only,
    /// principal before channel, and the first refusing budget names the
    /// limited fact — so under helpful answering a rate-limited member's
    /// message opens no turn and costs no model read.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if a budget count fails.
    async fn limiting_fact(
        &self,
        commanded: bool,
        summons: kind::Summons,
        principal_id: i64,
        conversation_id: i64,
    ) -> Result<Option<kind::LimitedBy>, CoreError> {
        if commanded {
            return Ok(Some(kind::LimitedBy::Command));
        }
        if summons.summoned {
            return self.refusing_budget(principal_id, conversation_id).await;
        }
        Ok(None)
    }

    /// What the recognized deletion command DOES, decided by what its reply
    /// names (unit T4, 2026-08-31) and narrowed by the revision reading of
    /// decision 0180.
    ///
    /// A reply naming a person's message erases that stored row and answers
    /// nothing. A reply naming one of the assistant's own messages resolves
    /// the recorded delivery that message belonged to, appends the retraction
    /// fact unless one already stands, and answers what the chat must lose.
    /// A reply the platform carried no id for, and one naming a message no
    /// delivery was ever recorded for, resolve nothing and answer nothing —
    /// the command stays recognized either way, so the row still records
    /// silently and takes no debt.
    ///
    /// The recognition rides in from the sender resolution instead of being
    /// asked again, so the stamp and this reading are one recognition per
    /// write. The ordering reasoning for WHERE this runs is at the call site;
    /// what the mirror erases is [`Self::mirror_named_deletion`] above.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if a lookup, a null or the retraction's append
    /// fails.
    async fn perform_deletion(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        message: &InboundMessage,
        recognized: Option<mirror::DeletionAsk<'_>>,
    ) -> Result<Option<ResolvedRetraction>, CoreError> {
        match mirror::performed_deletion(message, recognized) {
            Some(mirror::DeletionAsk::Message { origin }) => {
                self.mirror_named_deletion(tx, conversation_id, origin)
                    .await?;
                Ok(None)
            }
            Some(mirror::DeletionAsk::AssistantMessage {
                origin: Some(origin),
            }) => self.resolve_retraction(tx, conversation_id, origin).await,
            Some(mirror::DeletionAsk::AssistantMessage { origin: None }) | None => Ok(None),
        }
    }

    /// Resolve the delivery an administrator's reply named and record the
    /// retraction of it.
    ///
    /// The lookups are scoped to the channel's whole thread lineage, not to
    /// this one conversation: a platform message id is unique per channel,
    /// and a compaction leaves the serving thread holding only the second
    /// half, so a conversation-scoped lookup would go blind on every delivery
    /// recorded before the cut.
    ///
    /// The fact is appended at most once per delivery. Asking twice is one
    /// ask, and the recorded fact is the ask — while the WIRE call is
    /// re-issued on every repeat, because the first one may have failed and
    /// an administrator who sees the message still standing is telling us so.
    /// That is why the origins are answered whether or not a retraction
    /// already stood.
    async fn resolve_retraction(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        origin: &str,
    ) -> Result<Option<ResolvedRetraction>, CoreError> {
        let store = self.ctx.store();
        let lineage = lineage::serving_lineage(store, conversation_id).await?;
        let Some(key) = delivery::delivery_of_origin(tx, &lineage, origin).await? else {
            tracing::debug!(
                conversation_id,
                "the reply names a message this channel recorded no delivery for; \
                 the command is recorded and nothing is retracted"
            );
            return Ok(None);
        };
        let send = delivery::recorded_send(tx, &lineage, &key).await?;
        if delivery::retraction_stands(tx, &lineage, &key).await? {
            tracing::info!(
                conversation_id,
                messages = send.origins.len(),
                "the delivery already carries a retraction; the ask is re-issued and no \
                 second fact is appended"
            );
        } else {
            delivery::record_retraction(store, conversation_id, &key).await?;
            tracing::info!(
                conversation_id,
                messages = send.origins.len(),
                "an administrator's reply retracted one of the assistant's own deliveries"
            );
        }
        Ok(Some(ResolvedRetraction {
            origins: send.origins,
            answer_blocks: send.answer_blocks,
        }))
    }

    /// The retraction, run once the ingestion's holds are released: the fork
    /// that takes the retracted answer out of the model's view, and then the
    /// directive that takes its messages out of the chat.
    ///
    /// The fork runs FIRST and unconditionally. What the platform does with
    /// the deletion request is the platform's — a message past its 48-hour
    /// window cannot be taken back — and the assistant's own reading must not
    /// depend on it: an answer that was retracted is one the assistant no
    /// longer speaks from, whether or not the chat still shows it. A failed
    /// fork is logged and the directive still goes out, for the same reason
    /// in reverse.
    ///
    /// The directive is answered on every repeat, including one whose fork
    /// found nothing left to strip. The retraction fact stands from the first
    /// ask; the wire call is what an administrator's repeat is asking for.
    async fn retraction_answer(
        &self,
        retraction: ResolvedRetraction,
        conversation_id: i64,
    ) -> (Option<DeliveryItem>, ChannelReset) {
        let answer_blocks = retraction.answer_blocks.clone();
        let stripped = move |blocks: &[Block]| delivery::retracted_blocks(blocks, &answer_blocks);
        match self
            .sessions
            .strip_from_view(conversation_id, &stripped)
            .await
        {
            Ok(true) => {}
            Ok(false) => tracing::debug!(
                conversation_id,
                "the retracted answer was already out of the session's view, or the \
                 channel had moved on"
            ),
            Err(error) => tracing::warn!(
                conversation_id,
                %error,
                "the retracted answer could not be forked out of the session's view; it \
                 stands there, and a repeat of the command runs the fork again"
            ),
        }
        (
            Some(DeliveryItem::Retraction {
                origins: retraction.origins,
            }),
            // The fork carries every block but the retracted answer's, so
            // the channel's standing observations cross with it and the
            // adapter has nothing to forget.
            ChannelReset::Kept,
        )
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

    /// What the stamp lock's own reading decides for this write: whether it
    /// is disregarded — nothing written, nothing delivered, the update
    /// acknowledged — and, when it is recorded, whether the revision
    /// reference the adapter reported is stored with it.
    ///
    /// Every under-lock drop reads here, in the order it is decided, behind
    /// the ONE exemption they share: the privacy command family is answered
    /// whatever the store holds, resolved once into `exempt` and passed
    /// down, so the drops cannot drift apart.
    ///
    /// The suppression re-read comes first, being the cheaper question and
    /// the one about the sender rather than the message; the revision
    /// reading follows and answers the rest.
    async fn under_lock_reading(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        message: &InboundMessage,
        family: Option<PrivacyCommand>,
        reviser: i64,
    ) -> Result<UnderLock, CoreError> {
        let exempt = family.is_some();
        if !exempt && self.suppressed_under_lock(tx, message).await? {
            return Ok(UnderLock::Disregarded);
        }
        self.revision_reading(tx, conversation_id, message, exempt, reviser)
            .await
    }

    /// What one revision's ONE store read decides (unit T3, 2026-08-31):
    /// the editing unit's two drops, and whether the reported reference is
    /// stored. An ordinary message revises nothing, never reaches the read,
    /// and records exactly as it always did.
    ///
    /// No recorded version at all is the ERASURE GUARD. Erasure nulls a
    /// row's origin and its revision reference along with its text, so an
    /// erased message matches nothing — and the platform fires edit
    /// updates for changes nobody asked for, a link preview attaching
    /// hours later among them. Recording such a revision as a fresh
    /// statement would write a person's erased words, and their erased
    /// identifier, back into the ledger with no human act anywhere in the
    /// path. What is given up is the case where an edit adds text to a
    /// message the store never held: nothing about the group's memory
    /// silently changes there, while an erased message resurrecting itself
    /// is a defect against a published promise.
    ///
    /// Text identical to that newest version is a REDELIVERY of content
    /// the ledger already holds, byte for byte, under that same message —
    /// so no statement a person made goes unrecorded, and decision 0030 is
    /// untouched: this is not a protection mechanism, and a genuinely
    /// different edit always records, however many a person makes. It also
    /// makes a redelivered update after a halted batch idempotent for that
    /// tail version, where a redelivered new message still duplicates.
    ///
    /// A reviser who is NOT the author of the version the store holds gets
    /// the author invariant enforced instead of assumed: the message
    /// records as an ordinary new one, with no reference at all. Recording
    /// is never refused — a person's words are not dropped because a
    /// platform reported an implausible relation — and what falls away is
    /// only the link, which the erasure passes and the report resolution
    /// read as one person's own data. On this platform the case cannot
    /// arise; [`kind::COLUMN_REVISES`] states the rule, and this is the one
    /// place that keeps it.
    ///
    /// The privacy family is exempt from the two drops and never from the
    /// read: a rights command is answered whatever the store holds, so it
    /// records where an ordinary revision would be dropped — and it takes
    /// the same author check, because a rights request about one's own data
    /// is exactly the message that must not carry a link to somebody
    /// else's.
    ///
    /// Fail-closed on the read, as every other admission read is
    /// (decisions 0041, 0052): a store failure propagates and the
    /// ingestion refuses, because recording anyway would duplicate a row
    /// and, under helpful answering, spend a model turn on it. A store the
    /// read cannot answer is one the append below could not use either.
    async fn revision_reading(
        &self,
        tx: &StoreTx,
        conversation_id: i64,
        message: &InboundMessage,
        exempt: bool,
        reviser: i64,
    ) -> Result<UnderLock, CoreError> {
        let Some(named) = message.revises.as_deref() else {
            return Ok(UnderLock::Recorded(RevisionLink::Kept));
        };
        let Some(newest) = kind::newest_recorded_version(tx, conversation_id, named).await? else {
            if exempt {
                return Ok(UnderLock::Recorded(RevisionLink::Kept));
            }
            tracing::debug!(
                conversation_id,
                "a revision names a message the store holds no version of; nothing recorded"
            );
            return Ok(UnderLock::Disregarded);
        };
        if newest.principal_id != reviser {
            tracing::debug!(
                conversation_id,
                "a revision names a message written by somebody else; recorded unlinked"
            );
            return Ok(UnderLock::Recorded(RevisionLink::Dropped));
        }
        if !exempt && newest.text == message.text {
            return Ok(UnderLock::Disregarded);
        }
        Ok(UnderLock::Recorded(RevisionLink::Kept))
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
                        let sessions = Arc::clone(&self.sessions);
                        let streams = Arc::clone(&self.streams);
                        let context = Arc::clone(&self.context);
                        tokio::spawn(async move {
                            match erase_behind_the_fence(sessions, streams, context, principal_id)
                                .await
                            {
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
    /// A resolved retraction answers ahead of the catalogue, and the two
    /// cannot both stand for one write: the moderation bot's token is no
    /// catalogue command, so a message that resolved a retraction is a
    /// message the catalogue recognized nothing in. Its own mechanism runs
    /// past the holds, like the compaction's.
    ///
    /// The caller holds the erasure fence shared and the stamp lock, which
    /// is what lets the two session resets swap a channel's conversation
    /// from here with no ingestion halfway through one.
    async fn command_answer(
        &self,
        tx: &StoreTx,
        message: &InboundMessage,
        recorded: RecordedRow,
        sender: WritingSender<'_>,
        notice_admitted: bool,
        retraction: Option<ResolvedRetraction>,
    ) -> CommandAnswer {
        if let Some(retraction) = retraction {
            return CommandAnswer::Retraction(retraction);
        }
        let WritingSender {
            principal_id,
            authority,
            command,
            ..
        } = sender;
        let RecordedRow {
            conversation_id, ..
        } = recorded;
        let offered = command.filter(|command| command.offered(message.channel_kind, authority));
        let (deliver, reset) = match offered {
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
            // The one command the ingestion cannot finish: it drives a
            // model turn, and this runs under the holds. Answered here as
            // the deferral it is, and run by the entry point once both are
            // released.
            Some(Command::Compact) => return CommandAnswer::Compaction,
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
        };
        CommandAnswer::Settled(deliver, reset)
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

    /// The tool-choice supersession on delta (decided 2026-08-23), under
    /// the stamp lock every caller holds: on a conversation's first activity
    /// per process, the newest recorded choice is compared against the
    /// registered tool set, and a fresh choice is appended when they differ
    /// — one write per real change, the context note's on-delta shape. A
    /// conversation created before a tool existed gains it on its next
    /// activity; a tool the handle no longer configures is removed the same
    /// way, because the registered set is the comparison's one side. A
    /// conversation carrying no choice at all reads as a delta, and
    /// recording one is the correction: by the scoping decision of
    /// 2026-09-01 the record decides EXPOSURE and the framework's admission
    /// hook decides ENFORCEMENT, so a ledger holding no record filters
    /// nothing and every call still faces the authority check that never
    /// depended on the record. The memory is marked only after the append
    /// stands — a transiently failed append leaves the conversation
    /// unreconciled, and the redelivered activity retries.
    ///
    /// # The contract notice rides the delta (unit 55, 2026-09-02)
    ///
    /// A conversation whose newest prior choice EXISTED and lacked the two
    /// sending tools ran under the old contract, where the assistant's
    /// written answers were relayed to the group. In the same act as the
    /// choice that grants it the tools, one system-voiced
    /// [`ContractNotice`] is appended stating where the line falls: the
    /// answers above it were posted as they stand, and from there the
    /// written text is private.
    ///
    /// The two conditions are exactly the delta's own: a prior choice must
    /// have existed — a conversation carrying no record at all is one this
    /// process has no evidence about, and a notice explaining a change
    /// nobody can show would be a claim rather than a record — and it must
    /// have LACKED the two tools, since a conversation born under this
    /// build has no relayed answer to explain and gets no notice. Both
    /// blocks are appended now, after every raw answer the notice explains,
    /// so under compaction the notice sits with them.
    ///
    /// The one act is two writes, and no door spans both, so the ORDER
    /// carries what a transaction would: the notice is written first, and
    /// only when the conversation holds none yet. A process that dies
    /// between the two leaves the delta unwritten, so the next activity
    /// reads the same pre-contract choice, finds the notice already
    /// standing, and appends only what is missing. Written the other way
    /// round the delta would land alone and every later reconcile would see
    /// a choice that already names the tools — the crossing recorded
    /// nowhere, the notice lost for good.
    async fn reconcile_tool_choice(&self, conversation_id: i64) -> Result<(), CoreError> {
        if self
            .choice_reconciled
            .lock()
            .await
            .contains(&conversation_id)
        {
            return Ok(());
        }
        let recorded = self.ctx.store().newest_tool_choice(conversation_id).await?;
        let crossed_into_sending = recorded
            .as_ref()
            .is_some_and(|names| !names_the_sending_tools(names));
        let already_current =
            recorded.is_some_and(|names| names_the_same_set(&names, self.sessions.tool_names()));
        if !already_current {
            if crossed_into_sending && !self.holds_contract_notice(conversation_id).await? {
                self.ctx
                    .store()
                    .append_consumer_block(
                        conversation_id,
                        None,
                        contract::CONTRACT_NOTICE_KIND,
                        ContractNotice::stored_fields(contract::CONTRACT_NOTICE),
                        None,
                    )
                    .await?;
                tracing::info!(
                    conversation_id,
                    "the conversation crossed into the sending contract; the notice is recorded"
                );
            }
            self.ctx
                .store()
                .append_tool_choice(conversation_id, self.sessions.tool_names().to_vec())
                .await?;
            tracing::info!(
                conversation_id,
                "the conversation's tool choice was superseded to the registered tool set"
            );
        }
        let mut reconciled = self.choice_reconciled.lock().await;
        if reconciled.len() >= CHOICE_MEMORY_CAP {
            tracing::debug!("the tool-choice memory reached its cap and was cleared");
            reconciled.clear();
        }
        reconciled.insert(conversation_id);
        Ok(())
    }

    /// Whether this conversation already holds its contract notice — read
    /// off the ledger and not off the process's memory, because the fact the
    /// once-only rule rests on is a stored one: a process that appended the
    /// notice and died before the choice must find it here.
    ///
    /// One ledger read, taken only on the crossing itself, which happens at
    /// most once in a conversation's life.
    async fn holds_contract_notice(&self, conversation_id: i64) -> Result<bool, CoreError> {
        Ok(self
            .ctx
            .store()
            .list_blocks(conversation_id)
            .await?
            .iter()
            .any(|block| block.block_type == contract::CONTRACT_NOTICE_KIND))
    }

    /// The conversation's owing tail, if any — the one-block read behind the
    /// write-time stamp, deciding through the kind's own
    /// [`ChatMessage::owes_answer`] so this read and the awaiting hook can
    /// never disagree about one stamp: an erased tail's OWN debt, which the
    /// hook cancels, propagates nothing here either, while a live debt a
    /// third party's row still owes behind an erased run reads through
    /// (decision 0086) — someone else's deletion erases one message's ask, not
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
    /// the report and the tool-choice supersession, the report block and a
    /// superseding choice ([`DEBT_READ_THROUGH`]): each is appended by an
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
                | AssistantKind::ContextNote(_)
                | AssistantKind::JoinNotice(_)
                | AssistantKind::Report(_)
                | AssistantKind::Delivered(_)
                | AssistantKind::MessageMark(_)
                | AssistantKind::Retraction(_)
                | AssistantKind::OutgoingMessage(_)
                | AssistantKind::ContractNotice(_),
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

/// The stored row one ingested message becomes: its text, the three sender
/// facts, the two platform identifiers, the reply fact, the platform send
/// time and the composed stamp — every one of them encoded into columns by
/// the kind, which owns that map.
///
/// The speaker is the sender's public username as delivered at this receipt
/// — the handle as it was when the person spoke (decision 0065). A
/// handleless sender stores NULL and projects bare — no substitute
/// identifier is minted (decision 0056) — and the kind's storable-speaker
/// bound refuses a handle whose shape would blur the projected prefix. A
/// suppressed sender's exempt command records no speaker at all: the freeze
/// covers the delivered handle too, so after a deletion no command
/// re-materializes the field the erasure emptied (decided 2026-08-23).
///
/// The revision reference is stored only where the under-lock reading kept
/// the link ([`RevisionLink`]): a message revising one that somebody else
/// wrote becomes an ordinary new row here, which is where
/// [`kind::COLUMN_REVISES`]'s author invariant is enforced.
fn recorded_fields(
    message: &InboundMessage,
    principal_id: i64,
    authority: Authority,
    suppressed: bool,
    stamp: kind::Stamp,
    revision_link: RevisionLink,
) -> serde_json::Map<String, serde_json::Value> {
    ChatMessage::stored_fields(
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
        kind::RecordedOrigin {
            origin: message.origin.as_deref(),
            revises: match revision_link {
                RevisionLink::Kept => message.revises.as_deref(),
                RevisionLink::Dropped => None,
            },
        },
        message.reply_target.as_ref(),
        &message.timestamp.to_rfc3339(),
        stamp,
    )
}

/// What the stamp lock's reading decided for one write (unit T3,
/// 2026-08-31): either nothing is recorded at all, or the write proceeds
/// carrying what that same reading settled about its revision reference.
///
/// One value, because the drops and the reference are decided from ONE
/// store read: two returns would be two chances for them to disagree about
/// the row they both read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnderLock {
    /// Nothing written, nothing delivered, the update acknowledged —
    /// [`IngestOutcome::Disregarded`]'s full no-write claim.
    Disregarded,
    /// The write proceeds, storing the revision reference this says.
    Recorded(RevisionLink),
}

/// Whether a recorded row keeps the revision reference the adapter reported
/// (unit T3, 2026-08-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RevisionLink {
    /// Stored as reported. Every ordinary message reads this way too,
    /// having no reference to store.
    Kept,
    /// Not stored: the reviser is not the author of the version the store
    /// holds, so the message records as an ordinary new one. The words are
    /// never refused — only the link falls away, which is the author
    /// invariant [`kind::COLUMN_REVISES`] states.
    Dropped,
}

/// What the writing path knows about an admitted sender: the resolved
/// principal, the delivered authority, the command the message invokes with
/// the privacy family it projects to, and whether the suppression flag
/// stands — the facts [`Assistant::resolve_writing_sender`] hands the stamp,
/// the stored fields and the delivery.
#[derive(Debug, Clone, Copy)]
struct WritingSender<'a> {
    principal_id: i64,
    authority: Authority,
    /// The catalogue command the message invokes, if any — the one
    /// recognition of the write. The stamp reads whether it is present at
    /// all; the delivery reads which one it is.
    command: Option<Command>,
    /// The moderation bot's deletion command and what it names, if the
    /// message is one — the write's OTHER recognition, taken here beside the
    /// catalogue's so no later reader asks the message a second time. The
    /// stamp reads whether it is present at all; the mirror and the
    /// retraction read which side of the ask it names.
    deletion: Option<mirror::DeletionAsk<'a>>,
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

/// What one recognized command's answer came to: an answer the ingestion
/// already has, or a compaction to run once the ingestion's holds are
/// released.
///
/// The split is not about which command it is — it is about what the answer
/// NEEDS. Every other command is answered from stored state under the two
/// holds, and answering it anywhere else would let an ingestion slide
/// between a read and its write. The compaction drives a model turn, which
/// no hold may be held across, so it says so instead of pretending to be
/// settled.
enum CommandAnswer {
    /// The command is answered, with whatever the adapter must forget.
    Settled(Option<DeliveryItem>, ChannelReset),
    /// `/compact` was invoked and offered: the mechanism runs past the
    /// holds, and its own reply reports what it did.
    Compaction,
    /// The moderation bot's deletion command named one of the assistant's
    /// own deliveries, and the retraction fact for it is already on the
    /// ledger: the fork that takes the answer out of the model's view runs
    /// past the holds — it re-takes both for its own swap — and the chat's
    /// own directive is answered with it.
    Retraction(ResolvedRetraction),
}

/// One resolved retraction, carried from the ingestion's held stretch to the
/// mechanisms that run past the holds: what the chat must lose, and what the
/// assistant's own reading must lose.
///
/// The two travel together because they are one act read two ways — the
/// messages an administrator asked back, and the stored blocks those messages
/// were the transport for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRetraction {
    /// The platform ids of every message of the retracted delivery, in send
    /// order.
    origins: Vec<String>,
    /// The assistant's own blocks the delivery carried. Empty for a send of
    /// fixed prose, which the ledger never stored and the fork has nothing
    /// to take out.
    answer_blocks: Vec<i64>,
}

/// Where one ingested message landed: the conversation it was written into,
/// resolved under the stamp lock.
///
/// It carries one field and stays a type because the question it answers —
/// "which conversation did this write reach" — is not the one the caller's
/// other ids answer, and a bare `i64` travelling beside three others is how
/// two of them get swapped.
#[derive(Debug, Clone, Copy)]
struct RecordedRow {
    conversation_id: i64,
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
    sessions: Arc<Sessions>,
    streams: Arc<StreamObserver>,
    context: Arc<ContextWatch>,
    principal_id: i64,
) -> Result<ErasureOutcome, CoreError> {
    let ctx = sessions.context().clone();
    let store = ctx.store();
    // The lineages are read under the same hold, BEFORE the nulling and
    // before the identity is even consulted. Two things ride on that order.
    //
    // What a digest was written from is a fact about the blocks, and the
    // nulling does not change which blocks a person's are — reading it here
    // keeps the whole decision on one consistent view.
    //
    // And it is what makes the scrub RETRYABLE. The principal id stands on
    // every message row the nulling leaves behind, while the identity row is
    // what `conclude_erasure` DELETES for an unflagged person. Reading the
    // lineages behind the plan would mean a scrub that failed could never be
    // run again: the repeat call would find no identity, report NotFound and
    // walk past the very prose it left standing — the failure path's own
    // promise, unkeepable.
    let (outcome, lineages) = {
        let _no_ingestion_mid_erasure = sessions.erasure_fence().write().await;
        let lineages = erasure::compacted_lineages(store, principal_id).await?;
        match erasure::plan(store, principal_id).await? {
            // Nothing left to erase is not nothing left to do: the scrub
            // below runs on this answer too, which is the retry the failure
            // path promises.
            None => (ErasureOutcome::NotFound, lineages),
            Some(plan) => {
                // The plan's conversations are exactly the deletion set, so
                // settling them is settling everything the execute step will
                // remove.
                for &conversation_id in plan.direct_conversations() {
                    streams::settle_stream(store, ctx.bus(), &streams, conversation_id).await?;
                }
                let outcome = erasure::execute(store, plan).await?;
                if let ErasureOutcome::Erased {
                    deleted_conversations,
                } = &outcome
                {
                    // The store reissues conversation ids: a deleted
                    // conversation's stream observation and its context
                    // readings must not survive to shadow the id's next
                    // holder.
                    for &deleted in deleted_conversations {
                        context.forget(deleted);
                    }
                }
                (outcome, lineages)
            }
        }
    };
    // The scrub runs PAST the fence, and past the whole data erasure, for
    // two reasons that both point the same way. It drives a model turn, and
    // the erasure fence is the one hold no model call may be made under —
    // an ingestion stalled for a summary's latency is a stalled assistant.
    // And the stored personal data is erased immediately: the scrub
    // completes the erasure of a DIGEST, and it never delays the erasure of
    // the data.
    //
    // The residual is stated rather than hidden: a scrub whose regeneration
    // fails leaves the old digest standing, logged with the lineage it could
    // not rewrite. A repeat erasure of the same principal reaches it — the
    // lineages above are read off the blocks, so the retry works for a
    // person whose identity row this call already concluded as well as for
    // the opt-out stub that survives one.
    for stripped in lineages {
        let serving = stripped.serving.conversation;
        match sessions.scrub_compacted_digest(&stripped).await {
            Ok(true) => {}
            Ok(false) => tracing::info!(
                serving,
                "the compacted lineage needed no scrub, or its channel had moved on"
            ),
            Err(error) => tracing::warn!(
                serving,
                root = stripped.root,
                %error,
                "a compacted digest could not be scrubbed of the erased words; it stands, \
                 and a repeat erasure of the same principal runs the scrub again"
            ),
        }
    }
    Ok(outcome)
}
