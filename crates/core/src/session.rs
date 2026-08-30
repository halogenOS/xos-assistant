//! The channel's session: what a new conversation is created with, and how a
//! channel's session is replaced.
//!
//! A channel maps to exactly one conversation, and three things create or
//! replace that mapping: a channel's first contact, the two session-reset
//! commands, and the unattended compact the framework's forced turn end
//! triggers. All of them need the same four configured values — the model
//! binding, the reasoning level, the composed system prompt and the tool
//! palette — so those live here, once, and the assembly reads them through
//! this type instead of keeping a second copy.
//!
//! # Nothing is deleted
//!
//! Both resets leave the old conversation whole. A wipe stops pointing the
//! channel at it; a compact forks it and detaches from the FORK what the
//! fork does not keep, which removes a membership and never a block. The old
//! conversation stays readable, exportable and reachable by erasure, and
//! because every one of its blocks is still referenced by it, the orphan
//! collector cannot reach any of them. The one conversation ever deleted
//! here is a just-created one that lost its mapping claim, before anything
//! referenced it — the same exception the first-contact path has always had.
//!
//! # What orders a reset against everything else
//!
//! A reset reads a ledger, writes a fork and re-points a mapping, and an
//! ingestion interleaved between those steps would record into the
//! conversation the channel is leaving. So both triggers run under the same
//! two holds the ingestion path takes, in the same order: the erasure fence
//! shared first, the global stamp lock second. The command path is already
//! inside an ingestion and holds both; the unattended path takes them
//! itself and re-reads its conditions once it has them, so a wake that lost
//! the race finds its conversation unmapped and stands down.
//!
//! The cost is stated rather than hidden, and it is bounded: the sweep hands
//! the whole detach list to the framework's bulk door, which removes every
//! junction row in ONE round trip and one transaction. A conversation
//! carrying a thousand-call flood therefore holds the stamp lock for one
//! commit, not a thousand.

use std::sync::{Arc, OnceLock};

use agent_ledger::agency::Status;
use agent_ledger::providers::ReasoningLevel;
use agent_ledger::store::{ModelOverride, StoreError, StoreTx, domain_run};
use agent_ledger::{CoreEvent, EventBus, RuntimeContext};
use rusqlite::OptionalExtension;
use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError;

use crate::assembly::{ErasureFence, ModelBinding, ScriptedPause};
use crate::compaction::{self, CompactTrigger};
use crate::error::CoreError;
use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{ChannelKey, ChannelKind};
use crate::tools::palette::{TOOL_PALETTE_KIND, ToolPalette};

/// The framework's own table for a status row, and the column carrying its
/// machine key. Named here because the level read below is one indexed
/// query instead of a whole-ledger fold on a path that wakes at every
/// append — the deliberate framework-table coupling decision 0032 records,
/// exactly as the owing-tail walk already joins the block header and the
/// junction.
const STATUS_TABLE: &str = "block_status";
/// The status row's machine-key column.
const STATUS_COLUMN: &str = "status";

/// What one compact came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactOutcome {
    /// The fork stands and the channel points at it.
    Compacted,
    /// There was nothing to cut: no tool traffic stored, and no more chat
    /// rows than the kept bound. Nothing was forked and nothing changed.
    AlreadyCompact,
    /// The fork lost its mapping claim to a concurrent racer and was
    /// deleted; the winner's session governs the channel. Every block lives
    /// on in the source conversation.
    ClaimLost,
}

/// What one wipe came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WipeOutcome {
    /// The channel points at the fresh, empty conversation this wipe made.
    Replaced,
    /// The fresh conversation lost its mapping claim to a concurrent racer
    /// and was deleted before anything referenced it; the winner's session
    /// governs the channel, and this wipe changed nothing a caller may
    /// report as its own.
    ClaimLost,
}

/// A channel's mapping after a claim: the conversation the channel points
/// at afterwards, and whether the caller's own just-created conversation is
/// the one that took it.
///
/// The two facts travel together because they are read together: a caller
/// that only needs a conversation to write into reads the id, and a caller
/// answering a person for what IT did reads the claim as well — a winner's
/// session is not this call's doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaimedMapping {
    /// The conversation the channel maps to now.
    pub conversation_id: i64,
    /// Whether this call's conversation is the one mapped.
    pub claim_won: bool,
}

/// The conversation lifecycle of a channel, and the values every new
/// conversation is created with.
pub(crate) struct Sessions {
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    /// The model every new conversation is created under.
    binding: ModelBinding,
    /// The reasoning-effort level every new conversation is set to.
    reasoning: ReasoningLevel,
    /// The composed system prompt every new conversation records.
    system_prompt: String,
    /// The tool names every new conversation's palette block records.
    palette: Vec<String>,
    /// Serializes the answer-due stamp against other ingestions, and every
    /// session reset against both. This is the ONE home of that lock: the
    /// assembly's ingestion, observation and join paths take it through
    /// here, and so does the unattended compaction.
    ///
    /// The ingestion contract it carries: the tail read and the append it
    /// feeds are a read-then-write, and two concurrent ingestions into one
    /// conversation could both observe the pre-append tail — the later,
    /// unaddressed write would then be stamped false, cancelling exactly the
    /// owed answer decision 0021 exists to protect. Ingestion against
    /// ingestion is all this orders: the runtime commits its answer blocks
    /// outside it, so a tail read can see an answer-due tail whose answer
    /// commits a moment later, and the stamp then summons one extra turn
    /// over an already-answered tail — never a lost answer. One lock across
    /// all conversations, because ingestion runs at chat scale and a
    /// per-conversation lock map would buy contention nobody has.
    ///
    /// The reset contract it carries (unit 45, 2026-08-30): a reset moves a
    /// channel from one conversation to another, which no ingestion may be
    /// halfway through — so every path that resolves a channel's
    /// conversation resolves it INSIDE this lock, and the unattended
    /// compaction takes the lock before it acts.
    stamp_lock: Arc<Mutex<()>>,
    /// Orders erasure against ingestion and against a reset, held shared by
    /// both and exclusively by an erasure. This is the ONE home of that
    /// fence; the assembly reads it through here.
    ///
    /// An erasure reads, nulls and deletes in several store operations, and
    /// an ingestion interleaved between them could record a new message or
    /// map a new direct channel for the person being erased, leaving
    /// personal data behind after the erasure returned. A reset holds it for
    /// the same reason: it forks and re-points while an erasure would be
    /// walking the very conversations it moves.
    erasure_fence: ErasureFence,
    /// The reset claim race's test seam, run between a reset's mapping
    /// delete and its claim — exactly the window a concurrent racer takes
    /// the channel in. Unset in production.
    reset_claim_pause: OnceLock<ScriptedPause>,
}

impl Sessions {
    pub(crate) fn new(
        ctx: RuntimeContext<AssistantKind, CoreEvent>,
        binding: ModelBinding,
        reasoning: ReasoningLevel,
        system_prompt: String,
        palette: Vec<String>,
        stamp_lock: Arc<Mutex<()>>,
        erasure_fence: ErasureFence,
    ) -> Self {
        Self {
            ctx,
            binding,
            reasoning,
            system_prompt,
            palette,
            stamp_lock,
            erasure_fence,
            reset_claim_pause: OnceLock::new(),
        }
    }

    /// Install the reset claim race's test seam: the given pause runs
    /// between a reset's mapping delete and its claim, which is exactly the
    /// window a suite needs to make a concurrent racer win the channel.
    /// Production never calls this.
    pub(crate) fn pause_between_reset_delete_and_claim(&self, pause: ScriptedPause) {
        let _ = self.reset_claim_pause.set(pause);
    }

    /// Run the reset claim race's seam, if a suite installed one.
    async fn reset_claim_seam(&self) {
        if let Some(pause) = self.reset_claim_pause.get() {
            pause().await;
        }
    }

    /// The ingestion serialization every path takes, reset and ingestion
    /// alike; the whole contract is on the field.
    pub(crate) fn stamp_lock(&self) -> &Mutex<()> {
        &self.stamp_lock
    }

    /// The erasure fence every path takes, reset and ingestion alike; the
    /// whole contract is on the field.
    pub(crate) fn erasure_fence(&self) -> &ErasureFence {
        &self.erasure_fence
    }

    /// The model every new conversation is created under.
    pub(crate) fn binding(&self) -> &ModelBinding {
        &self.binding
    }

    /// The reasoning-effort level every new conversation is set to.
    pub(crate) fn reasoning(&self) -> ReasoningLevel {
        self.reasoning
    }

    /// The composed system prompt every new conversation records.
    pub(crate) fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// The tool names every new conversation's palette block records.
    pub(crate) fn palette(&self) -> &[String] {
        &self.palette
    }

    /// First contact on a channel: create the conversation under the
    /// binding, record the system prompt and the tool palette as its first
    /// blocks, claim the mapping, and set the winner's reasoning level.
    /// Direct and group channels take the identical path, so both get the
    /// same palette and the same level.
    ///
    /// Two callers can race here; the mapping's claim decides, and the
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
    ///
    /// # Errors
    ///
    /// [`CoreError::ClaimLost`] if the mapping row is gone again by the read
    /// back; [`CoreError::Store`] if a write fails.
    pub(crate) async fn map_new_channel(
        &self,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<ClaimedMapping, CoreError> {
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
        Ok(ClaimedMapping {
            conversation_id: winner,
            claim_won: winner == created,
        })
    }

    /// Fork a conversation under the CURRENT deployment: the current
    /// binding, the given blocks detached from the fork, the current system
    /// prompt recorded in their place, and the configured reasoning level
    /// set on the fork.
    ///
    /// The fork inherits its source's prompt along with everything else, and
    /// an appended replacement would sit behind it, read second and obeyed
    /// unevenly — so the inherited one is among the blocks the caller names
    /// for detaching, and the current prompt is recorded after. The source
    /// keeps every block: a fork does not edit what it came from.
    ///
    /// The fork always carries the current binding. Inheriting would keep a
    /// stale model alive through the very operations that exist to replace
    /// a session, and a session replaced under the current prompt but the
    /// previous model is half a replacement.
    ///
    /// However long the list, the detaching is ONE round trip and one
    /// transaction: the framework's bulk door takes the whole set, and the
    /// per-row door would serialize a thousand transactions behind the
    /// stamp lock this runs under.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the fork, the detaching, the prompt insert or
    /// the reasoning write fails.
    pub(crate) async fn forked_with_current_prompt(
        &self,
        source: i64,
        up_to_block_id: i64,
        detach: &[i64],
    ) -> Result<i64, CoreError> {
        let store = self.ctx.store();
        let successor = store
            .fork_conversation(
                source,
                up_to_block_id,
                ModelOverride {
                    provider_id: Some(self.binding.provider_instance.clone()),
                    external_id: Some(self.binding.model.clone()),
                    display_name: Some(self.binding.model_display_name.clone()),
                    vendor: Some(self.binding.vendor.clone()),
                    reasoning: None,
                },
            )
            .await?;
        store.detach_blocks(successor, detach.to_vec()).await?;
        store
            .insert_system_prompt(successor, self.system_prompt.clone())
            .await?;
        store
            .set_conversation_reasoning(successor, Some(self.reasoning.as_key().to_owned()))
            .await?;
        Ok(successor)
    }

    /// Replace a channel's session with an empty one: drop the mapping row
    /// and run first contact for the same channel, so the channel gets
    /// exactly what a newly admitted one gets — a fresh conversation, the
    /// current prompt, the current palette — with no history inherited.
    ///
    /// The caller holds the stamp lock and the erasure fence.
    ///
    /// The claim can still be lost — a concurrent racer between the delete
    /// and the claim — and then this wipe made nothing: its just-created
    /// conversation is deleted before anything referenced it, the winner's
    /// session governs, and the outcome says so rather than handing back a
    /// conversation somebody else's operation produced.
    ///
    /// # Errors
    ///
    /// [`CoreError::ClaimLost`] if the fresh mapping claim finds no winner;
    /// [`CoreError::Store`] if a read or a write fails.
    pub(crate) async fn wipe(
        &self,
        conversation_id: i64,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<WipeOutcome, CoreError> {
        mapping::delete_by_conversation(&self.ctx.store().tx(), conversation_id).await?;
        self.reset_claim_seam().await;
        let claimed = self.map_new_channel(channel, kind).await?;
        if !claimed.claim_won {
            tracing::warn!(
                retired = conversation_id,
                winner = claimed.conversation_id,
                "the wiped channel was re-claimed by a concurrent racer; the fresh conversation was dropped and the winner's session governs"
            );
            return Ok(WipeOutcome::ClaimLost);
        }
        tracing::info!(
            retired = conversation_id,
            fresh = claimed.conversation_id,
            "the channel starts a fresh session; the old conversation stays on record"
        );
        Ok(WipeOutcome::Replaced)
    }

    /// Trim a channel's session: fork the conversation with its full
    /// history, detach from the fork everything the compaction reading does
    /// not keep, and point the channel at the fork.
    ///
    /// The blocks are snapshotted BEFORE the fork, so the fork's own fresh
    /// system prompt is structurally outside the sweep. A conversation with
    /// nothing to cut is left alone entirely: no fork, no mapping write.
    ///
    /// The caller holds the stamp lock and the erasure fence.
    ///
    /// # Errors
    ///
    /// [`CoreError::ClaimLost`] if the re-claim finds no winner;
    /// [`CoreError::Store`] if a read or a write fails.
    pub(crate) async fn compact(
        &self,
        source: i64,
        channel: &ChannelKey,
        kind: ChannelKind,
        trigger: CompactTrigger,
    ) -> Result<CompactOutcome, CoreError> {
        let store = self.ctx.store();
        let blocks = store.list_blocks(source).await?;
        let planned = compaction::plan(&blocks, trigger);
        if planned.nothing_to_cut {
            return Ok(CompactOutcome::AlreadyCompact);
        }
        // An empty conversation reads as nothing to cut above, so the tail
        // is present whenever the sweep runs; the absence is answered the
        // same way instead of being unwrapped.
        let Some(last) = blocks.last().map(|block| block.id) else {
            return Ok(CompactOutcome::AlreadyCompact);
        };
        let successor = self
            .forked_with_current_prompt(source, last, &planned.detach)
            .await?;
        let tx = store.tx();
        mapping::delete_by_conversation(&tx, source).await?;
        self.reset_claim_seam().await;
        let winner = mapping::claim(&tx, channel, kind, successor).await?;
        if winner != successor {
            // A concurrent racer already claimed the channel. The fork owes
            // turns nobody can deliver, so it goes — junction rows alone,
            // every block living on in the source.
            store.delete_conversation(successor).await?;
            tracing::warn!(
                source,
                successor,
                winner,
                "the compacted fork lost the mapping claim and was dropped; the winner's session governs"
            );
            return Ok(CompactOutcome::ClaimLost);
        }
        tracing::info!(
            source,
            successor,
            detached = planned.detach.len(),
            "the channel's session was compacted; the old conversation stays on record"
        );
        Ok(CompactOutcome::Compacted)
    }

    /// The unattended compact, run for one conversation the bus woke us
    /// about.
    ///
    /// Nothing is answered in chat: nobody invoked anything, and a line
    /// nobody asked for in a group is noise. The record is this method's own
    /// log.
    ///
    /// The eligibility read runs first WITHOUT the holds, because the wake
    /// arrives on every block change in every conversation and taking the
    /// ingestion lock that often would stall the chat. It is read again
    /// under the holds, which is what makes a lost race stand down instead
    /// of forking a conversation the winner already replaced.
    async fn auto_compact(&self, conversation_id: i64) {
        match self.auto_compact_target(conversation_id).await {
            Ok(None) => return,
            Ok(Some(_)) => {}
            Err(error) => {
                tracing::warn!(
                    conversation_id,
                    %error,
                    "the unattended compact could not read its conditions; the next change re-reads"
                );
                return;
            }
        }
        match self.auto_compact_behind_the_holds(conversation_id).await {
            Ok(Some(outcome)) => {
                tracing::warn!(
                    conversation_id,
                    ?outcome,
                    "a turn ended on a spent tool-call window; the session was compacted unattended"
                );
            }
            Ok(None) => tracing::debug!(
                conversation_id,
                "the unattended compact stood down; the conversation is no longer eligible"
            ),
            Err(error) => tracing::warn!(
                conversation_id,
                %error,
                "the unattended compact failed; the session stands and the next change retries"
            ),
        }
    }

    /// The unattended compact's acting half, behind the two holds, with the
    /// conditions read again inside them. `None` says the conversation
    /// stopped being eligible between the two reads.
    async fn auto_compact_behind_the_holds(
        &self,
        conversation_id: i64,
    ) -> Result<Option<CompactOutcome>, CoreError> {
        let _no_erasure_mid_reset = self.erasure_fence.read().await;
        let _one_reset_at_a_time = self.stamp_lock.lock().await;
        let Some((channel, kind)) = self.auto_compact_target(conversation_id).await? else {
            return Ok(None);
        };
        self.compact(conversation_id, &channel, kind, CompactTrigger::Signal)
            .await
            .map(Some)
    }

    /// Whether this conversation is eligible for an unattended compact, and
    /// where it would be re-claimed: its stored status blocks record the
    /// framework's forced turn end, AND it is currently mapped to a channel.
    ///
    /// Both halves are read from durable state, never from the event, so a
    /// wake the lossy bus dropped costs nothing: the next change on that
    /// conversation reads the same standing fact. The mapped half is what
    /// makes the operation self-limiting from the other side — a swept
    /// source is unmapped from the moment its fork claims the channel, so
    /// however many late appends wake it, it is never compacted again.
    async fn auto_compact_target(
        &self,
        conversation_id: i64,
    ) -> Result<Option<(ChannelKey, ChannelKind)>, CoreError> {
        let tx = self.ctx.store().tx();
        if !exhausted_turn_recorded(&tx, conversation_id).await? {
            return Ok(None);
        }
        let (Some(channel), Some(kind)) = (
            mapping::channel_for_conversation(&tx, conversation_id).await?,
            mapping::kind_for_conversation(&tx, conversation_id).await?,
        ) else {
            return Ok(None);
        };
        Ok(Some((channel, kind)))
    }
}

/// Watch the bus and compact a mapped conversation whose turn the framework
/// ended over a spent tool-call window. The task holds the sessions weakly
/// and ends with the assembly or with the bus, whichever goes first — the
/// stream observer's own shape.
///
/// The trigger is level-read from stored state on every block change, so a
/// dropped or lagged event heals on the next change instead of losing the
/// incident the watcher exists for.
///
/// It fires on DIRECT chats as well as groups, deliberately, where the two
/// commands are fenced to groups: the commands' fence is about authority —
/// a moderator floor states nothing in a room with no moderators — while
/// this healing is about a conversation whose history has gone bad, which
/// happens wherever a mapped conversation exhausts its tool-call window.
pub(crate) fn spawn_auto_compact(sessions: &Arc<Sessions>, bus: &Arc<EventBus<CoreEvent>>) {
    let mut events = bus.subscribe();
    let weak = Arc::downgrade(sessions);
    tokio::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(
                        missed,
                        "the compaction watcher lagged; the next change re-reads stored state"
                    );
                    continue;
                }
                Err(RecvError::Closed) => break,
            };
            let CoreEvent::BlocksChanged {
                conversation_id, ..
            } = event
            else {
                continue;
            };
            let Some(sessions) = weak.upgrade() else {
                break;
            };
            sessions.auto_compact(conversation_id).await;
        }
    });
}

/// Whether the conversation holds a status row recording the framework's
/// forced turn end over a spent tool-call window.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
async fn exhausted_turn_recorded(tx: &StoreTx, conversation_id: i64) -> Result<bool, StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let found: Option<i64> = conn
            .query_row(
                &format!(
                    "SELECT 1 FROM conversation_blocks cb \
                     JOIN {STATUS_TABLE} s ON s.block_id = cb.block_id \
                     WHERE cb.conversation_id = ?1 AND s.{STATUS_COLUMN} = ?2 \
                     LIMIT 1"
                ),
                rusqlite::params![conversation_id, Status::TOOL_CALLS_EXHAUSTED],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    })
    .await
}
