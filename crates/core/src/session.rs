//! The channel's session: what a new conversation is created with, and how a
//! channel's session is replaced.
//!
//! A channel maps to exactly one conversation, and four things create or
//! replace that mapping: a channel's first contact, the two session-reset
//! commands, and the unattended compaction.
//! All of them need the same four configured values — the model
//! binding, the reasoning level, the composed system prompt and the tool
//! palette — so those live here, once, and the assembly reads them through
//! this type instead of keeping a second copy.
//!
//! # What a compaction is
//!
//! One mechanism, reached through three doors — `/compact`, the framework's
//! forced turn end, and the context thresholds — and it is the same
//! mechanism every time:
//!
//! 1. The ledger is cut in half by the framework's own deterministic rule
//!    ([`Store::compaction_cut`]), which never splits a message group and
//!    never splits a tool lifecycle.
//! 2. The first half is forked into a TEMPORARY conversation carrying an
//!    empty tool palette and, last, the compaction instructions — the append
//!    that summons its one turn.
//! 3. That turn's answer is the summary. The temporary conversation is
//!    retired junction-only the moment it is read; its two own blocks are all
//!    that is left for the collector, and every block of the first half lives
//!    on in the source.
//! 4. Whatever turn the source still had open is settled, so the ledger
//!    about to be copied has stopped moving.
//! 5. A new thread opens with the current prompt, a block naming the
//!    conversation it continues, the summary, and then the second half of the
//!    source's ledger verbatim.
//! 6. The channel is re-pointed at the new thread through the claim's own
//!    winner check, and the thread is served like any other.
//!
//! # Nothing is deleted
//!
//! Both resets leave the old conversation whole. A wipe stops pointing the
//! channel at it; a compaction shares its blocks with the thread that
//! succeeds it and leaves the source itself unmapped and intact. The old
//! conversation stays readable, exportable and reachable by erasure, and
//! because every one of its blocks is still referenced by it, the orphan
//! collector cannot reach any of them. The conversations ever deleted here
//! are a just-created one the channel never took — it lost its mapping claim,
//! or the swap that would have handed it over failed, and either way nothing
//! referenced it, the same exception the first-contact path has always had —
//! and a compaction's temporary conversation, junction-only, once its answer
//! has been read.
//!
//! That is one rule and not a habit: a thread this module opens is MAPPED or
//! RETIRED before the call that built it returns. A thread left in neither
//! state would be a latched conversation carrying a fresh digest that no
//! sweep and no channel ever reaches again.
//!
//! # What orders a reset against everything else
//!
//! A reset reads a ledger, writes a conversation and re-points a mapping, and
//! an ingestion interleaved between those steps would record into the
//! conversation the channel is leaving. So the swap runs under the same
//! two holds the ingestion path takes, in the same order: the erasure fence
//! shared first, the global stamp lock second.
//!
//! A compaction's CAPTURE runs outside both, deliberately. It drives a model
//! turn, and holding the one ingestion lock across a model call would stall
//! every conversation this process serves for that call's whole latency —
//! the rules acknowledgment's own recorded reasoning. The holds are taken
//! for the swap alone, and the mapping is re-read inside them: a capture
//! whose channel moved on while the summary was being written stands down
//! rather than re-pointing a channel that is somewhere else now.
//!
//! The holds order the swap against INGESTION and say nothing about a model
//! turn already in flight, so the source's own stream is SETTLED inside them
//! before its history is copied — the same interrupt-and-confirm the erasure
//! runs ahead of its deletions. A turn left running lands its answer in a
//! conversation the swap has just unmapped, which the outbound edge delivers
//! nothing from, and the streaming tail it had already written rides across
//! into the successor only to be cascaded away when the source finalizes.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use agent_ledger::agency::{AncestorReference, LeafKind, Status, Text};
use agent_ledger::providers::ReasoningLevel;
use agent_ledger::store::{
    CompactedThread, ConsumerRecord, LedgerCut, ModelOverride, StoreError, StoreTx,
    TemporaryConversation, TemporaryFork, domain_run,
};
use agent_ledger::{Block, CoreEvent, EventBus, Role, RuntimeContext};
use rusqlite::OptionalExtension;
use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError;

use crate::assembly::{ErasureFence, ModelBinding, ScriptedPause};
use crate::compaction::{COMPACTION_INSTRUCTIONS, CONTEXT_SWEEP, ContextWatch};
use crate::erasure;
use crate::error::CoreError;
use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{ChannelKey, ChannelKind};
use crate::streams;
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

/// How long a compaction waits for its temporary conversation's one turn
/// before giving up on it. Generous against a slow model on a long first
/// half, bounded so a provider that never answers cannot park the driver
/// forever: the failure leaves every conversation standing and the next
/// trigger re-derives the whole operation.
const SUMMARY_BOUND: Duration = Duration::from_mins(3);

/// What one compaction came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactOutcome {
    /// The compacted thread stands and the channel points at it.
    Compacted,
    /// The ledger does not split: it is too short to have two halves, or its
    /// cut reaches the end and leaves nothing to carry forward verbatim.
    /// Nothing was forked, no model was called and nothing changed.
    AlreadyCompact,
    /// The compacted thread lost its mapping claim to a concurrent racer and
    /// was deleted; the winner's session governs the channel. Every block
    /// lives on in the source conversation.
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
    /// The stream and activity readings, held for the two things a session
    /// replacement needs from them: the stream it must settle before it
    /// copies a conversation's history away, and the readings it must drop
    /// for every conversation it deletes — the store reissues ids, and a
    /// dead entry would arm a threshold on the id's next holder.
    context: Arc<ContextWatch>,
    /// The reset claim race's test seam, run between a reset's mapping
    /// delete and its claim — exactly the window a concurrent racer takes
    /// the channel in. Unset in production.
    reset_claim_pause: OnceLock<ScriptedPause>,
}

/// What a session replacement has to agree with the rest of the process
/// about: the two holds every path takes, and the readings a deleted
/// conversation's entries have to be dropped from.
///
/// Handed over as ONE value because they are one thing — the shared state a
/// reset coordinates against — and because a bare lock travelling beside a
/// bare fence beside a bare watch is how two of them get swapped.
pub(crate) struct SessionCoordination {
    /// Serializes the answer-due stamp and every session reset; the whole
    /// contract is on [`Sessions::stamp_lock`]'s field.
    pub stamp_lock: Arc<Mutex<()>>,
    /// Orders erasure against ingestion and against a reset; the whole
    /// contract is on [`Sessions::erasure_fence`]'s field.
    pub erasure_fence: ErasureFence,
    /// The stream and activity readings; the whole contract is on
    /// [`Sessions::context`]'s field.
    pub context: Arc<ContextWatch>,
}

impl Sessions {
    pub(crate) fn new(
        ctx: RuntimeContext<AssistantKind, CoreEvent>,
        binding: ModelBinding,
        reasoning: ReasoningLevel,
        system_prompt: String,
        palette: Vec<String>,
        coordination: SessionCoordination,
    ) -> Self {
        Self {
            ctx,
            binding,
            reasoning,
            system_prompt,
            palette,
            stamp_lock: coordination.stamp_lock,
            erasure_fence: coordination.erasure_fence,
            context: coordination.context,
            reset_claim_pause: OnceLock::new(),
        }
    }

    /// Delete one conversation and drop what was measured for it, which is
    /// ONE act here and never two: the store reissues conversation ids, so a
    /// measurement left behind hands the id's next holder a stranger's token
    /// count and its dispatch time, and both threshold arms can arm on it
    /// before that conversation's own first turn ever reports.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the deletion fails.
    async fn retire(&self, conversation_id: i64) -> Result<(), CoreError> {
        self.ctx
            .store()
            .delete_conversation(conversation_id)
            .await?;
        self.context.forget(conversation_id);
        Ok(())
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

    /// The runtime this assembly runs on. Handed out because the paths
    /// that outlive an assembly call — the spawned erasure above all — need
    /// the store and the bus without borrowing the assembly itself.
    pub(crate) fn context(&self) -> &RuntimeContext<AssistantKind, CoreEvent> {
        &self.ctx
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
            self.retire(created).await?;
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
            .fork_conversation(source, up_to_block_id, self.current_model())
            .await?;
        store.detach_blocks(successor, detach.to_vec()).await?;
        store
            .insert_system_prompt(successor, self.system_prompt.clone())
            .await?;
        Ok(successor)
    }

    /// Clone a conversation minus a named set of blocks: every other
    /// junction row is SHARED, never copied, and the named ones are simply
    /// absent from the clone.
    ///
    /// The copy-on-write erasure of the design's scrub, and it is the
    /// junction table's own sharing rather than a mechanism beside it — a
    /// block only ever exists once, and a conversation is which blocks it
    /// holds. The source keeps every one of them: a clone does not edit what
    /// it came from.
    ///
    /// An empty conversation clones to an empty conversation. Which model
    /// the clone runs under is the CALLER's, because the two callers mean
    /// different things by a clone: an ancestor rebuilt underneath a scrub
    /// is a copy of a history and inherits, so re-pointing its model would
    /// change what it is a copy of, while a clone that takes the channel is
    /// a session being replaced and carries the current binding like every
    /// other replacement door.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if the read, the clone or the detaching fails.
    pub(crate) async fn cloned_without(
        &self,
        source: i64,
        stripped: &[i64],
        model: ModelOverride,
    ) -> Result<i64, CoreError> {
        let store = self.ctx.store();
        let blocks = store.list_blocks(source).await?;
        let Some(last) = blocks.last().map(|block| block.id) else {
            return Ok(store
                .create_conversation(
                    self.binding.provider_instance.clone(),
                    self.binding.model.clone(),
                    self.binding.model_display_name.clone(),
                    self.binding.vendor.clone(),
                )
                .await?);
        };
        let clone = store.fork_conversation(source, last, model).await?;
        store.detach_blocks(clone, stripped.to_vec()).await?;
        Ok(clone)
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

    /// Compact a channel's session: summarize the first half of its ledger
    /// and hand the channel a thread carrying that summary and the second
    /// half verbatim. The whole mechanism is on this module's own
    /// documentation.
    ///
    /// The caller holds NEITHER hold. The capture drives a model turn, and
    /// this method takes the two holds itself for the swap alone — which is
    /// also where the channel's mapping is re-read, so a capture whose
    /// channel moved on stands down instead of re-pointing it.
    ///
    /// # Errors
    ///
    /// [`CoreError::CompactionUnsummarized`] if the temporary conversation
    /// produced no summary — nothing is swapped and nothing is deleted;
    /// [`CoreError::ClaimLost`] if the re-claim finds no winner;
    /// [`CoreError::Store`] if a read or a write fails.
    pub(crate) async fn compact(
        &self,
        source: i64,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<CompactOutcome, CoreError> {
        let store = self.ctx.store();
        let Some(cut) = store.compaction_cut(source).await? else {
            return Ok(CompactOutcome::AlreadyCompact);
        };
        let temporary = store
            .fork_temporary(
                source,
                cut.first_half_ends,
                TemporaryFork {
                    // The design's "dont provide any tools", recorded as
                    // well as offered: the empty palette admits nothing, and
                    // the instructions block's own kind is what makes the
                    // turn offered nothing in the first place. Recorded
                    // AHEAD of the instructions, so the turn they summon is
                    // already governed by it.
                    records: vec![ConsumerRecord {
                        kind: TOOL_PALETTE_KIND,
                        role: None,
                        fields: ToolPalette::stored_fields(&[]),
                    }],
                    instructions: COMPACTION_INSTRUCTIONS.to_owned(),
                },
            )
            .await?;
        let captured = self.capture_summary(temporary).await;
        // Retired junction-only the moment its answer has been read,
        // whatever the answer was: the first half's blocks all live on in
        // the source, and the two blocks this conversation owns — the
        // instructions and the captured answer — are all the collector is
        // left.
        if let Err(error) = self.retire(temporary.conversation_id).await {
            tracing::warn!(
                temporary = temporary.conversation_id,
                %error,
                "the compaction's temporary conversation was not retired; it holds no mapping and nothing serves it"
            );
        }
        let Some(summary) = captured? else {
            return Err(CoreError::CompactionUnsummarized {
                conversation_id: source,
            });
        };
        self.install_compacted_thread(source, cut, summary, channel, kind)
            .await
    }

    /// Run the temporary conversation's one turn and read its answer.
    ///
    /// The conversation was born latched like every other, so nothing has
    /// driven it: the unlatch below is what starts the turn, and subscribing
    /// BEFORE it is what keeps a fast turn's end signal from being missed.
    ///
    /// The answer is read from the LEDGER, never from the event: the wait is
    /// there to spare the read its polling, and a lagged or dropped signal
    /// costs nothing because the blocks decide. What counts as the answer is
    /// the newest assistant-voiced text past the instructions block — past
    /// it, because everything before it is the inherited history the turn
    /// was asked ABOUT, including the source's own earlier answers.
    ///
    /// `None` is a turn that produced no prose: it failed, it was silent, or
    /// it never ran. The caller changes nothing on that answer.
    async fn capture_summary(
        &self,
        temporary: TemporaryConversation,
    ) -> Result<Option<String>, CoreError> {
        let mut events = self.ctx.bus().subscribe();
        self.ctx.bus().emit(CoreEvent::UnlatchRequested {
            conversation_id: temporary.conversation_id,
        });
        let deadline = tokio::time::Instant::now() + SUMMARY_BOUND;
        let ended =
            streams::await_stream_end(&mut events, temporary.conversation_id, deadline).await;
        if !ended {
            tracing::warn!(
                temporary = temporary.conversation_id,
                "the compaction's turn did not end before its bound; the ledger decides what it wrote"
            );
        }
        let blocks = self
            .ctx
            .store()
            .list_blocks(temporary.conversation_id)
            .await?;
        Ok(blocks
            .iter()
            .rev()
            .take_while(|block| block.id != temporary.instructions_block_id)
            .filter(|block| {
                block.role == Some(Role::Assistant)
                    && Text::KINDS.contains(&block.block_type.as_str())
            })
            .map(|block| Text::parse(block).content.trim().to_owned())
            .find(|content| !content.is_empty()))
    }

    /// Open the compacted thread and hand it the channel, both under the two
    /// holds.
    ///
    /// The mapping is re-read first: the summary took a model turn to write,
    /// and a `/wipe`, a racing compaction or an erasure may have moved the
    /// channel meanwhile. A channel that is no longer on this source is one
    /// this compaction has nothing to say about, so it stands down and
    /// leaves everything as it found it.
    ///
    /// The source's own stream is settled next, and that is what keeps the
    /// copy below from being taken from underneath a live turn. Left running,
    /// the turn lands its answer in the source AFTER the second half was
    /// copied — in a conversation the swap has unmapped, which the outbound
    /// edge delivers nothing from — and the streaming tail it had already
    /// written would ride across into the thread only to be swept out from
    /// under it when the source finalizes. Stopping it is what the operator
    /// contract already states a reset does: the answer it was working on is
    /// cut.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the source's stream did not settle
    /// before its bound — nothing is copied and nothing is swapped;
    /// [`CoreError::Store`] if a read or a write fails.
    async fn install_compacted_thread(
        &self,
        source: i64,
        cut: LedgerCut,
        summary: String,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<CompactOutcome, CoreError> {
        let store = self.ctx.store();
        let _no_erasure_mid_reset = self.erasure_fence.read().await;
        let _one_reset_at_a_time = self.stamp_lock.lock().await;
        let tx = store.tx();
        if mapping::find(&tx, channel).await? != Some((source, kind)) {
            tracing::debug!(
                source,
                "the channel left this conversation while its summary was written; the compaction stands down"
            );
            return Ok(CompactOutcome::ClaimLost);
        }
        self.settle(source).await?;
        // The second half is copied from the ledger AS IT STANDS NOW, not as
        // it stood when the cut was derived: everything a member said while
        // the summary was being written sits past the cut and rides across
        // verbatim with the rest of it.
        let successor = store
            .open_compacted_thread(
                source,
                cut.first_half_ends,
                CompactedThread {
                    ancestor_conversation_id: source,
                    system_prompt: Some(self.system_prompt.clone()),
                    compaction_message: summary,
                    model: self.current_model(),
                },
            )
            .await?;
        // Past the creation, every exit accounts for the thread: the swap
        // maps it, the swap's own loser branch retires it, and a swap that
        // FAILS leaves it here — built, unmapped, latched, serving nobody and
        // reached by no sweep. So this path retires it, exactly as the
        // capture path retires its temporary conversation.
        let swapped = match self.swap_channel(source, successor, channel, kind).await {
            Ok(swapped) => swapped,
            Err(error) => {
                self.discard(&[successor]).await;
                return Err(error);
            }
        };
        if !swapped {
            tracing::warn!(
                source,
                successor,
                "the compacted thread lost the mapping claim and was dropped; the winner's session governs"
            );
            return Ok(CompactOutcome::ClaimLost);
        }
        tracing::info!(
            source,
            successor,
            first_half_ends = cut.first_half_ends,
            "the channel's session was compacted; the old conversation stays on record"
        );
        Ok(CompactOutcome::Compacted)
    }

    /// Move a channel from one conversation to another through the claim's
    /// own winner check, answering whether `successor` is the conversation
    /// the channel ended up on.
    ///
    /// THE swap, and the only one: the compaction takes it, and so does the
    /// erasure scrub that replaces a whole lineage. A loser's conversation is
    /// deleted before anything referenced it — it owes turns nobody can
    /// deliver — and every block it shared lives on in what it was built
    /// from.
    ///
    /// The caller holds the stamp lock and the erasure fence.
    ///
    /// # Errors
    ///
    /// [`CoreError::ClaimLost`] if the claim finds no winner;
    /// [`CoreError::Store`] if a read or a write fails.
    pub(crate) async fn swap_channel(
        &self,
        retired: i64,
        successor: i64,
        channel: &ChannelKey,
        kind: ChannelKind,
    ) -> Result<bool, CoreError> {
        let store = self.ctx.store();
        let tx = store.tx();
        mapping::delete_by_conversation(&tx, retired).await?;
        self.reset_claim_seam().await;
        let winner = mapping::claim(&tx, channel, kind, successor).await?;
        if winner != successor {
            self.retire(successor).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// Stop whatever model turn is still writing into this conversation,
    /// before its history is copied away and its mapping moves. The whole
    /// protocol — what an observed-open stream costs, what a stored
    /// streaming tail costs, and what an idle conversation costs, which is
    /// nothing — is [`streams::settle_stream`]'s own, and the erasure takes
    /// the identical call ahead of its deletions.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the stream did not settle before
    /// its bound; [`CoreError::Store`] if a read fails.
    async fn settle(&self, conversation_id: i64) -> Result<(), CoreError> {
        streams::settle_stream(
            self.ctx.store(),
            self.ctx.bus(),
            self.context.streams(),
            conversation_id,
        )
        .await
    }

    /// The model every conversation this deployment creates runs on, as the
    /// fork doors take it: the current binding and the configured reasoning
    /// level, in one value. Written once, because a fork that took the
    /// binding and left the level behind is half a replacement.
    pub(crate) fn current_model(&self) -> ModelOverride {
        ModelOverride {
            provider_id: Some(self.binding.provider_instance.clone()),
            external_id: Some(self.binding.model.clone()),
            display_name: Some(self.binding.model_display_name.clone()),
            vendor: Some(self.binding.vendor.clone()),
            reasoning: Some(self.reasoning.as_key().to_owned()),
        }
    }

    /// Take a named set of blocks out of what a channel's session shows the
    /// model, by forking the session without them (unit T4, 2026-08-31).
    ///
    /// Which blocks go is the caller's reading, taken per conversation over
    /// that conversation's own ledger, so this door knows nothing about
    /// answers, retractions or people. Answers whether anything was
    /// replaced.
    ///
    /// Two shapes, one condition, and the condition is whether a DIGEST was
    /// written from the stripped blocks. Prose a model wrote about a stretch
    /// of conversation cannot be edited free of one message the way a
    /// junction row is dropped, so a lineage whose digests hold the blocks
    /// takes the regenerating scrub, at its per-hop model-turn cost — the
    /// same mechanism and the same cost an erasure already accepts. When
    /// nothing below the serving thread's own opening is stripped, no digest
    /// can hold the blocks and the serving clone alone is the whole fork.
    ///
    /// The caller holds NEITHER hold: the scrub takes a model turn per hop,
    /// and the swap takes the two holds for itself.
    ///
    /// # Errors
    ///
    /// [`CoreError::CompactionUnsummarized`] if a regeneration captured
    /// nothing; [`CoreError::StreamUnsettled`] if the serving thread's own
    /// stream did not settle; [`CoreError::Store`] if a read or a write
    /// fails.
    pub(crate) async fn strip_from_view(
        &self,
        serving: i64,
        stripped: &(dyn Fn(&[Block]) -> Vec<i64> + Sync),
    ) -> Result<bool, CoreError> {
        let store = self.ctx.store();
        if let Some(lineage) = erasure::stripped_lineage(store, serving, stripped)
            .await?
            .filter(StrippedLineage::reaches_a_digest)
        {
            return self.scrub_compacted_digest(&lineage).await;
        }
        let blocks = store.list_blocks(serving).await?;
        self.replace_with_clone(serving, &stripped(&blocks)).await
    }

    /// Hand a channel a clone of its own session minus the named blocks, and
    /// retire what it replaced.
    ///
    /// The two holds are taken here and the mapping is re-read inside them,
    /// for the reason every other replacement re-reads it: a `/wipe`, a
    /// compaction or an erasure may have moved the channel since the caller
    /// read it. The serving conversation's own stream is settled next,
    /// exactly as a compaction settles its source — a turn still writing
    /// into the conversation would land its answer in one this swap has
    /// unmapped, and the streaming tail it had already written would ride
    /// into the clone only to be swept out from under it. An administrator's
    /// command arriving mid-answer therefore cuts that answer short, the same
    /// way a reset does.
    ///
    /// The retirement is what makes the strip mean anything: the clone shares
    /// every block it kept, so the stripped ones are held by the retired
    /// conversation alone and the collector is what finally removes them.
    /// Nothing here deletes a block by hand.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the stream did not settle before its
    /// bound; [`CoreError::Store`] if a read or a write fails.
    async fn replace_with_clone(&self, serving: i64, stripped: &[i64]) -> Result<bool, CoreError> {
        if stripped.is_empty() {
            return Ok(false);
        }
        let store = self.ctx.store();
        let _no_erasure_mid_reset = self.erasure_fence.read().await;
        let _one_reset_at_a_time = self.stamp_lock.lock().await;
        let Some((channel, kind)) = self.mapped_channel(serving).await? else {
            tracing::warn!(
                serving,
                "the conversation serves no channel any more, so the strip did not \
                 run; if the stripped blocks ride in the channel's current session, \
                 a repeat of the command strips them there"
            );
            return Ok(false);
        };
        self.settle(serving).await?;
        let clone = self
            .cloned_without(serving, stripped, self.current_model())
            .await?;
        // A swap that FAILS leaves the clone built, unmapped and reached by
        // no sweep — the same exit the compacted door accounts for — so the
        // error arm discards it before the failure travels.
        let swapped = match self.swap_channel(serving, clone, &channel, kind).await {
            Ok(swapped) => swapped,
            Err(error) => {
                self.discard(&[clone]).await;
                return Err(error);
            }
        };
        if !swapped {
            tracing::warn!(
                serving,
                "the forked session lost the mapping claim and was dropped; the winner's session governs"
            );
            return Ok(false);
        }
        self.retire(serving).await?;
        store.gc_orphan_blocks().await?;
        tracing::info!(
            serving,
            clone,
            stripped = stripped.len(),
            "the channel's session was forked without the stripped blocks"
        );
        Ok(true)
    }

    /// Scrub an erased principal's words out of every compacted digest they
    /// fed — the design's clone-strip-delete, copy-on-write.
    ///
    /// A digest is prose a model wrote about a stretch of conversation, so it
    /// cannot be edited free of one voice the way a stored column is nulled:
    /// it is REGENERATED from the history minus that person, and the old one
    /// is deleted with the conversation that held it.
    ///
    /// The lineage is the WHOLE ancestry, not a pair. The words that fed the
    /// oldest digest live on the ROOT — the first half nobody inherited — and
    /// every thread above it carries a digest written from the half below,
    /// which HOLDS the digest below. So the chain is rebuilt from the root
    /// upward: the root is cloned without the erased blocks, and each hop in
    /// turn gets a clone whose digest is regenerated from the clone beneath
    /// it and whose reference names that clone. Stopping at one hop would
    /// leave the older digest inside the very history the newer one is
    /// regenerated from, and the erased words would come back as prose about
    /// prose.
    ///
    /// Sharing is the erasure's own economy: a clone is junction rows, and
    /// only what CHANGED is written fresh — each thread's two opening
    /// appends, and nothing else. Every other block is the same block.
    ///
    /// Ordering is capture-first. Nothing is swapped and nothing established
    /// is deleted until every regenerated summary is in hand, so a failed or
    /// empty capture at any depth leaves the whole lineage standing exactly
    /// as it was — the clones built so far go with the failure — and the
    /// scrub can simply be run again.
    ///
    /// Returns whether a lineage was scrubbed.
    ///
    /// # Errors
    ///
    /// [`CoreError::CompactionUnsummarized`] if a regeneration captured
    /// nothing; [`CoreError::StreamUnsettled`] if the serving thread's own
    /// stream did not settle; [`CoreError::Store`] if a read or a write
    /// fails.
    pub(crate) async fn scrub_compacted_digest(
        &self,
        stripped: &StrippedLineage,
    ) -> Result<bool, CoreError> {
        let store = self.ctx.store();
        let serving = stripped.serving.conversation;
        let Some((channel, kind)) = self.mapped_channel(serving).await? else {
            tracing::warn!(
                serving,
                "the conversation serves no channel any more, so the scrub did not \
                 run; if the scrubbed words ride in the channel's current session, \
                 a repeat of the command scrubs them there"
            );
            return Ok(false);
        };
        // Built with NEITHER hold: every hop below the serving thread costs a
        // model turn, and no hold may be held across one.
        let mut created = Vec::new();
        let rebuilt = match self.rebuilt_ancestry(stripped, &mut created).await {
            Ok(Some(rebuilt)) => rebuilt,
            Ok(None) => {
                self.discard(&created).await;
                return Ok(false);
            }
            Err(error) => {
                self.discard(&created).await;
                return Err(error);
            }
        };

        let _no_erasure_mid_reset = self.erasure_fence.read().await;
        let _one_reset_at_a_time = self.stamp_lock.lock().await;
        let serving_clone = match self
            .install_scrubbed_thread(&stripped.serving, rebuilt, &channel, kind, &mut created)
            .await
        {
            Ok(Some(serving_clone)) => serving_clone,
            Ok(None) => {
                self.discard(&created).await;
                return Ok(false);
            }
            Err(error) => {
                self.discard(&created).await;
                return Err(error);
            }
        };
        // ONLY past the verified swap: every conversation carrying the erased
        // words is deleted, junction-only. The blocks their clones share live
        // on; the ones nobody shares — the old digests above all, which are
        // the prose being scrubbed — are left to the collector, which runs
        // here because deleting that prose IS the erasure.
        for retired in retired_lineage(stripped) {
            self.retire(retired).await?;
        }
        store.gc_orphan_blocks().await?;
        tracing::info!(
            serving,
            root = stripped.root,
            depth = stripped.below.len() + 1,
            serving_clone,
            "a compacted lineage was rebuilt without the stripped blocks' words"
        );
        Ok(true)
    }

    /// Rebuild everything BELOW the serving thread, with neither hold: the
    /// root's scrubbed clone, then one scrubbed clone per intermediate hop
    /// carrying a digest regenerated from the clone beneath it, and finally
    /// the digest the serving thread's own clone will open with.
    ///
    /// The intermediate hops are safe to build holds-free for the reason the
    /// root is: nothing maps them, so no ingestion can be writing into them
    /// and no swap can move them. Only the serving thread is live, and its
    /// clone is built under the holds by [`Self::install_scrubbed_thread`].
    ///
    /// Every conversation this creates is pushed to `created` as it is made,
    /// so a caller that stands down or fails deletes exactly what was built,
    /// however deep the walk got.
    ///
    /// `None` is a regeneration with nothing to be about — a span the
    /// successor had inherited whole, which the mechanism cannot produce and
    /// which is answered rather than assumed away.
    ///
    /// # Errors
    ///
    /// [`CoreError::CompactionUnsummarized`] if a capture produced nothing;
    /// [`CoreError::Store`] if a read or a write fails.
    async fn rebuilt_ancestry(
        &self,
        stripped: &StrippedLineage,
        created: &mut Vec<i64>,
    ) -> Result<Option<RebuiltAncestry>, CoreError> {
        let store = self.ctx.store();
        // The root's clone IS the post-erasure history the oldest digest is
        // written from.
        let mut ancestor_clone = self
            .cloned_without(stripped.root, &stripped.in_root, ModelOverride::default())
            .await?;
        created.push(ancestor_clone);
        for hop in &stripped.below {
            let Some(summary) = self
                .regenerated_digest(ancestor_clone, hop.conversation)
                .await?
            else {
                return Ok(None);
            };
            let clone = store
                .open_compacted_thread(
                    hop.conversation,
                    hop.opening_ends,
                    CompactedThread {
                        ancestor_conversation_id: ancestor_clone,
                        system_prompt: Some(self.system_prompt.clone()),
                        compaction_message: summary,
                        model: self.current_model(),
                    },
                )
                .await?;
            store.detach_blocks(clone, hop.stripped.clone()).await?;
            created.push(clone);
            ancestor_clone = clone;
        }
        let Some(serving_digest) = self
            .regenerated_digest(ancestor_clone, stripped.serving.conversation)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(RebuiltAncestry {
            ancestor_clone,
            serving_digest,
        }))
    }

    /// Open the serving thread's scrubbed clone and hand it the channel, both
    /// under the two holds. Answers the clone, or `None` when the scrub stood
    /// down — the channel moved on, or the claim went to a racer.
    ///
    /// The mapping is re-read first: the regeneration took a model turn per
    /// hop, and a `/wipe`, a compaction or another erasure may have moved the
    /// channel meanwhile.
    ///
    /// The serving thread's own stream is settled next, for the reason the
    /// compaction settles its source: the clone copies the thread's history
    /// as it stands, and a turn still writing into the thread would land its
    /// answer in a conversation this swap unmaps.
    ///
    /// The clone joins `created` the moment it exists, like every hop below
    /// it: a failure past its creation — the detach, the swap's own store
    /// calls — leaves a thread nothing maps and no sweep reaches, and the
    /// caller's discard is what retires it. Answering with it never hands it
    /// over twice: `created` is discarded only where the caller installs
    /// nothing.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the thread's stream did not settle;
    /// [`CoreError::Store`] if a read or a write fails.
    async fn install_scrubbed_thread(
        &self,
        serving_hop: &StrippedHop,
        rebuilt: RebuiltAncestry,
        channel: &ChannelKey,
        kind: ChannelKind,
        created: &mut Vec<i64>,
    ) -> Result<Option<i64>, CoreError> {
        let store = self.ctx.store();
        let serving = serving_hop.conversation;
        if mapping::find(&store.tx(), channel).await? != Some((serving, kind)) {
            tracing::debug!(
                serving,
                "the channel left this thread while its digest was regenerated; the scrub stands down"
            );
            return Ok(None);
        }
        self.settle(serving).await?;
        // The serving clone: the new reference and the new digest in place of
        // the old pair, everything past them shared, and the erased person's
        // own blocks detached from the copy.
        let serving_clone = store
            .open_compacted_thread(
                serving,
                serving_hop.opening_ends,
                CompactedThread {
                    ancestor_conversation_id: rebuilt.ancestor_clone,
                    system_prompt: Some(self.system_prompt.clone()),
                    compaction_message: rebuilt.serving_digest,
                    model: self.current_model(),
                },
            )
            .await?;
        created.push(serving_clone);
        store
            .detach_blocks(serving_clone, serving_hop.stripped.clone())
            .await?;
        if !self
            .swap_channel(serving, serving_clone, channel, kind)
            .await?
        {
            tracing::warn!(
                serving,
                "the scrubbed thread lost the mapping claim and was dropped; the winner's session governs"
            );
            return Ok(None);
        }
        Ok(Some(serving_clone))
    }

    /// Drop conversations a reset built and will not use, newest first — a
    /// scrub standing down or failing past its first clone, and a compaction
    /// whose swap failed with its thread already open. Junction-only, like
    /// every other retirement here: the blocks they share live on in what
    /// they were cloned from, and their own fresh appends are left to the
    /// collector. A deletion that itself fails is logged rather than raised:
    /// the caller is already reporting why nothing was installed, and an
    /// unmapped clone nothing serves is not a second failure to hand back.
    ///
    /// Every door that opens a conversation it may not hand over ends here.
    /// A thread built and neither mapped nor retired is a latched
    /// conversation with a fresh digest that no sweep ever reaches, so
    /// "retired on every exit past its creation" is one rule with one
    /// implementation rather than a habit each path keeps separately.
    async fn discard(&self, created: &[i64]) {
        for &conversation_id in created.iter().rev() {
            if let Err(error) = self.retire(conversation_id).await {
                tracing::warn!(
                    conversation_id,
                    %error,
                    "a scrub's unused clone was not retired; it holds no mapping and nothing serves it"
                );
            }
        }
    }

    /// One thread's regenerated digest: captured from `ancestor_clone` over
    /// exactly the span `successor` never inherited.
    ///
    /// The span is PINNED, not re-derived: exactly the ancestor clone's
    /// blocks the successor never inherited — that thread's original first
    /// half as it stands after the strip. The boundary needs no stored
    /// position, because it is the complement of what the successor holds:
    /// nothing silently drops out of the regenerated view, and nothing is
    /// reported twice beside the verbatim second half. Those blocks are a
    /// PREFIX of the clone's ledger — a thread's own opening appends sit at
    /// the front and everything it inherited follows — so the last
    /// non-inherited block is where the temporary fork ends.
    ///
    /// `None` means there was nothing to regenerate — a span the successor
    /// had inherited whole, which the mechanism cannot produce and which is
    /// answered rather than assumed away.
    ///
    /// # Errors
    ///
    /// [`CoreError::CompactionUnsummarized`] if the capture produced
    /// nothing; [`CoreError::Store`] if a read or a write fails.
    async fn regenerated_digest(
        &self,
        ancestor_clone: i64,
        successor: i64,
    ) -> Result<Option<String>, CoreError> {
        let store = self.ctx.store();
        let held: std::collections::HashSet<i64> = store
            .list_blocks(successor)
            .await?
            .iter()
            .map(|block| block.id)
            .collect();
        let span_ends = store
            .list_blocks(ancestor_clone)
            .await?
            .iter()
            .map(|block| block.id)
            .rfind(|id| !held.contains(id));
        let Some(span_ends) = span_ends else {
            // The successor inherited everything the clone still holds: there
            // is nothing left for a digest to be about. Unreachable through
            // the mechanism — a first half always keeps at least the
            // conversation's own prompt — and answered rather than assumed.
            tracing::warn!(
                successor,
                ancestor_clone,
                "the regeneration span is empty; the digest is left as it stands"
            );
            return Ok(None);
        };
        let temporary = store
            .fork_temporary(
                ancestor_clone,
                span_ends,
                TemporaryFork {
                    records: vec![ConsumerRecord {
                        kind: TOOL_PALETTE_KIND,
                        role: None,
                        fields: ToolPalette::stored_fields(&[]),
                    }],
                    instructions: COMPACTION_INSTRUCTIONS.to_owned(),
                },
            )
            .await?;
        let captured = self.capture_summary(temporary).await;
        if let Err(error) = self.retire(temporary.conversation_id).await {
            tracing::warn!(
                temporary = temporary.conversation_id,
                %error,
                "the regeneration's temporary conversation was not retired"
            );
        }
        match captured {
            Ok(Some(summary)) => Ok(Some(summary)),
            captured => {
                // Capture-first: nothing established has been touched yet, and
                // the clones built so far go with the failure at the caller.
                captured?;
                Err(CoreError::CompactionUnsummarized {
                    conversation_id: successor,
                })
            }
        }
    }

    /// The compaction the driver runs for one conversation, behind whichever
    /// door woke it.
    ///
    /// Nothing is answered in chat: nobody invoked anything, and a line
    /// nobody asked for in a group is noise. The record is this method's own
    /// log.
    async fn unattended_compact(&self, conversation_id: i64, door: &'static str) {
        let Ok(Some((channel, kind))) = self.mapped_channel(conversation_id).await.inspect_err(
            |error| {
                tracing::warn!(
                    conversation_id,
                    %error,
                    "the unattended compaction could not read its channel; the next wake re-reads"
                );
            },
        ) else {
            return;
        };
        match self.compact(conversation_id, &channel, kind).await {
            Ok(outcome) => tracing::info!(
                conversation_id,
                door,
                ?outcome,
                "the session was compacted unattended"
            ),
            Err(error) => tracing::warn!(
                conversation_id,
                door,
                %error,
                "the unattended compaction failed; the session stands and the next wake retries"
            ),
        }
    }

    /// The channel this conversation is currently mapped to, or `None` when
    /// it is mapped to none.
    ///
    /// Read from durable state, never from an event, so a wake the lossy bus
    /// dropped costs nothing. It is also what makes the operation
    /// self-limiting from the other side: a compacted source is unmapped
    /// from the moment its successor claims the channel, so however many
    /// late appends wake it, it is never compacted again.
    async fn mapped_channel(
        &self,
        conversation_id: i64,
    ) -> Result<Option<(ChannelKey, ChannelKind)>, CoreError> {
        let tx = self.ctx.store().tx();
        let (Some(channel), Some(kind)) = (
            mapping::channel_for_conversation(&tx, conversation_id).await?,
            mapping::kind_for_conversation(&tx, conversation_id).await?,
        ) else {
            return Ok(None);
        };
        Ok(Some((channel, kind)))
    }
}

/// Watch for the two unattended doors into the compaction and drive them —
/// the framework's forced turn end, and the context thresholds.
///
/// One task, and the compactions it runs are awaited inline: two captures for
/// one conversation would spend two model turns to produce one summary, and
/// the second would find the channel already moved. The task holds the
/// sessions weakly and ends with the assembly or with the bus, whichever goes
/// first — the stream observer's own shape.
///
/// **The forced turn end** (the design's runaway case) is level-read from
/// stored state on every block change, so a dropped or lagged event heals on
/// the next change instead of losing the incident the door exists for. It
/// fires on DIRECT chats as well as groups, deliberately, where `/compact` is
/// fenced to groups: the command's fence is about authority — a moderator
/// floor states nothing in a room with no moderators — while this healing is
/// about a conversation whose history has gone bad, which happens wherever a
/// mapped conversation exhausts its tool-call window.
///
/// **The thresholds** are time-based readings, and nothing on the bus
/// announces the moment one starts holding: the sweep is what notices, and it
/// asks the context watch, which answers from what the stream observer
/// measured and when the channel last heard from anyone.
pub(crate) fn spawn_compaction_driver(
    sessions: &Arc<Sessions>,
    bus: &Arc<EventBus<CoreEvent>>,
    watch: &Arc<ContextWatch>,
) {
    let mut events = bus.subscribe();
    let weak = Arc::downgrade(sessions);
    let watch = Arc::downgrade(watch);
    // A process that stated no context window has both threshold arms
    // permanently silent, so it gets no sweep and no periodic timer at all —
    // the door simply does not exist there, rather than existing and
    // answering no every time.
    let mut sweep = watch
        .upgrade()
        .is_some_and(|watch| watch.sweeps())
        .then(|| {
            let mut sweep = tokio::time::interval(CONTEXT_SWEEP);
            sweep.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            sweep
        });
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = events.recv() => {
                    let event = match event {
                        Ok(event) => event,
                        Err(RecvError::Lagged(missed)) => {
                            tracing::warn!(
                                missed,
                                "the compaction driver lagged; the next change re-reads stored state"
                            );
                            continue;
                        }
                        Err(RecvError::Closed) => break,
                    };
                    let CoreEvent::BlocksChanged { conversation_id, .. } = event else {
                        continue;
                    };
                    let Some(sessions) = weak.upgrade() else {
                        break;
                    };
                    match exhausted_turn_since_the_thread_opened(
                        &sessions.ctx.store().tx(),
                        conversation_id,
                    )
                    .await
                    {
                        Ok(true) => {
                            sessions.unattended_compact(conversation_id, "forced turn end").await;
                        }
                        Ok(false) => {}
                        Err(error) => tracing::warn!(
                            conversation_id,
                            %error,
                            "the forced-turn-end read failed; the next change re-reads"
                        ),
                    }
                }
                () = next_sweep(&mut sweep) => {
                    let (Some(sessions), Some(watch)) = (weak.upgrade(), watch.upgrade()) else {
                        break;
                    };
                    for conversation_id in watch.observed() {
                        if !watch.due(conversation_id) {
                            continue;
                        }
                        sessions.unattended_compact(conversation_id, "context threshold").await;
                        // The compacted source is unmapped and its successor
                        // is a fresh id with no measurement of its own; the
                        // stale reading would otherwise keep answering for a
                        // conversation nothing serves.
                        watch.forget(conversation_id);
                    }
                }
            }
        }
    });
}

/// What one erased principal costs one compacted lineage: the whole
/// ancestry the serving thread stands on, and the blocks each conversation
/// in it must lose.
///
/// A lineage is not two conversations. A thread compacted twice continues a
/// thread that continues a conversation, and each hop's digest was written
/// from the half below it — a half that HOLDS the digest below — so prose
/// about an erased person's words survives one generation on as prose about
/// that prose. Every digest in the chain is therefore regenerated, from the
/// root upward.
///
/// Read once, by the caller that finds the lineage, and handed here whole —
/// so the scrub never re-derives what it is scrubbing while it scrubs it.
#[derive(Debug, Clone)]
pub(crate) struct StrippedLineage {
    /// The oldest conversation in the chain: the one that continues
    /// nothing, and the history every digest above it is ultimately written
    /// from.
    pub root: i64,
    /// The erased principal's blocks in the root.
    pub in_root: Vec<i64>,
    /// The threads BETWEEN the root and the serving one, each continuing the
    /// one before it, OLDEST FIRST. Empty for a lineage one compaction deep.
    pub below: Vec<StrippedHop>,
    /// The thread the channel is on, and the one whose clone takes it. A
    /// field of its own rather than the last of a list, because a lineage
    /// always has exactly one and no caller should have to answer what an
    /// empty one would mean.
    pub serving: StrippedHop,
}

impl StrippedLineage {
    /// Whether any digest in this lineage was written from stripped blocks
    /// — that is, whether anything BELOW the serving thread's own opening
    /// goes.
    ///
    /// Every digest was written from the half beneath it, so a strip that
    /// touches nothing beneath the serving thread cannot be inside any
    /// digest's prose, and the serving clone alone answers it. A strip that
    /// does reach down needs the whole chain regenerated: stopping at one
    /// hop would leave the older digest inside the very history the newer
    /// one is rewritten from.
    pub(crate) fn reaches_a_digest(&self) -> bool {
        !self.in_root.is_empty() || self.below.iter().any(|hop| !hop.stripped.is_empty())
    }
}

/// Every conversation a scrubbed lineage retires, oldest first: the root
/// and every thread that stood on it. Written once, because the deletion
/// set and the set the clones replaced are the same set, and two spellings
/// of it would eventually disagree.
fn retired_lineage(stripped: &StrippedLineage) -> impl Iterator<Item = i64> + '_ {
    std::iter::once(stripped.root)
        .chain(stripped.below.iter().map(|hop| hop.conversation))
        .chain(std::iter::once(stripped.serving.conversation))
}

/// What a scrub's holds-free half hands to its held half: the ancestor the
/// serving thread's clone will name, and the digest that clone opens with.
struct RebuiltAncestry {
    /// The newest clone below the serving thread — the root's clone when the
    /// lineage is one hop deep, an intermediate thread's clone otherwise.
    ancestor_clone: i64,
    /// The serving clone's regenerated digest, captured from
    /// `ancestor_clone`.
    serving_digest: String,
}

/// One compacted thread inside a lineage.
#[derive(Debug, Clone)]
pub(crate) struct StrippedHop {
    /// The thread itself.
    pub conversation: i64,
    /// Its own opening: its ancestor reference and the digest behind it.
    /// Everything past this block is what the thread inherited, and it is
    /// what the scrubbed clone inherits in turn.
    pub opening_ends: i64,
    /// The erased principal's blocks in this thread.
    pub stripped: Vec<i64>,
}

/// The next threshold sweep, or a future that never completes when this
/// process has no sweep — which is what lets the driver's one loop carry a
/// door that may not exist without a second loop beside it.
async fn next_sweep(sweep: &mut Option<tokio::time::Interval>) {
    match sweep {
        Some(interval) => {
            interval.tick().await;
        }
        None => std::future::pending().await,
    }
}

/// Whether the conversation holds a status row recording the framework's
/// forced turn end over a spent tool-call window, recorded INSIDE this
/// thread's own life.
///
/// The scoping is what keeps the door from re-opening on an incident that
/// has already been answered (2026-08-31). A compacted thread inherits the
/// second half of its source's ledger, and the forced end's marker sits at
/// the end of the turn it ended — so it usually rides across, and an
/// unscoped read would find it in the successor, compact that, find it in
/// ITS successor, and burn a model turn per round until the ledger ran out.
///
/// A thread's own life begins at its ancestor-reference block, which is the
/// first thing a compaction writes; block ids ascend with insertion, so
/// every inherited block is older than that reference and every later
/// incident is newer. A thread that carries no such reference has never been
/// compacted, and its whole ledger is its own life.
///
/// The framework-table names it joins carry the deliberate coupling decision
/// 0032 records, exactly as the owing-tail walk's do, and the kinds are
/// named through the framework's own declarations rather than literals.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
async fn exhausted_turn_since_the_thread_opened(
    tx: &StoreTx,
    conversation_id: i64,
) -> Result<bool, StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let found: Option<i64> = conn
            .query_row(
                &format!(
                    "SELECT 1 FROM conversation_blocks cb \
                     JOIN {STATUS_TABLE} s ON s.block_id = cb.block_id \
                     WHERE cb.conversation_id = ?1 AND s.{STATUS_COLUMN} = ?2 \
                     AND cb.block_id > COALESCE(( \
                       SELECT MAX(opening.block_id) FROM conversation_blocks opening \
                       JOIN blocks b ON b.id = opening.block_id \
                       WHERE opening.conversation_id = ?1 AND b.block_type = ?3 \
                     ), 0) \
                     LIMIT 1"
                ),
                rusqlite::params![
                    conversation_id,
                    Status::TOOL_CALLS_EXHAUSTED,
                    AncestorReference::KINDS[0]
                ],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    })
    .await
}
