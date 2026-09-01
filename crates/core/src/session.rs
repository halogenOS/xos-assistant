//! The channel's session: what a new conversation is created with, and how a
//! channel's session is replaced.
//!
//! A channel maps to exactly one conversation, and four things create or
//! replace that mapping: a channel's first contact, the two session-reset
//! commands, and the unattended compaction.
//! All of them need the same four configured values — the model
//! binding, the reasoning level, the composed system prompt and the tool
//! names — so those live here, once, and the assembly reads them through
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
//! 2. The first half is forked into a TEMPORARY conversation carrying the
//!    empty tool choice the framework's own fork door records and, last, the
//!    compaction instructions — the append that summons its one turn.
//! 3. That turn's answer is the summary. The temporary conversation is
//!    retired junction-only the moment it is read — its turn interrupted and
//!    settled first, so nothing is still writing into a conversation that is
//!    about to go — and its two own blocks are all that is left for the
//!    collector, while every block of the first half lives on in the source.
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

use agent_ledger::agency::{AgencyCtx, AncestorReference, LeafKind, Status, Text, ratchet};
use agent_ledger::providers::ReasoningLevel;
use agent_ledger::store::{
    CompactedThread, LedgerCut, ModelOverride, StoreError, StoreTx, TemporaryConversation,
    TemporaryFork, domain_run,
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
    /// The tool names every new conversation records as its tool choice.
    tool_names: Vec<String>,
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
        tool_names: Vec<String>,
        coordination: SessionCoordination,
    ) -> Self {
        Self {
            ctx,
            binding,
            reasoning,
            system_prompt,
            tool_names,
            stamp_lock: coordination.stamp_lock,
            erasure_fence: coordination.erasure_fence,
            context: coordination.context,
            reset_claim_pause: OnceLock::new(),
        }
    }

    /// Stop whatever turn is still writing into this conversation, delete
    /// it, and drop what was measured for it.
    ///
    /// The settle comes FIRST and that ordering is the point (unit 51,
    /// 2026-09-01): a conversation deleted under a running turn takes every
    /// later write of that turn with it — each one funnels through a
    /// junction insert that references the row now gone — and the writes
    /// keep coming, because nothing told the turn. A settle that FAILS
    /// deletes nothing and fails the caller: deleting anyway would reopen
    /// exactly the race the settle exists to close.
    ///
    /// The forget is the third step and is never separate from the deletion:
    /// the store reissues conversation ids, so a measurement left behind
    /// hands the id's next holder a stranger's token count and its dispatch
    /// time, and both threshold arms can arm on it before that
    /// conversation's own first turn ever reports.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the conversation's turn did not
    /// settle before its bound — nothing is deleted; [`CoreError::Store`] if
    /// the deletion fails.
    async fn retire(&self, conversation_id: i64) -> Result<(), CoreError> {
        self.settle(conversation_id).await?;
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

    /// The tool names every new conversation records as its tool choice.
    pub(crate) fn tool_names(&self) -> &[String] {
        &self.tool_names
    }

    /// First contact on a channel: create the conversation under the
    /// binding, record the system prompt and the tool choice as its first
    /// blocks, claim the mapping, and set the winner's reasoning level.
    /// Direct and group channels take the identical path, so both get the
    /// same tools and the same level.
    ///
    /// Two callers can race here; the mapping's claim decides, and the
    /// loser's conversation is deleted — its prompt and choice blocks with
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
    /// back; [`CoreError::StreamUnsettled`] if the loser's conversation did
    /// not settle before its deletion, which leaves it standing — it was
    /// created moments ago and has run no turn, so nothing can be writing
    /// into it, and the variant is here because the retirement raises it;
    /// [`CoreError::Store`] if a write fails.
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
            .append_tool_choice(created, self.tool_names.clone())
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
    /// current prompt, the current tools — with no history inherited.
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
    /// [`CoreError::StreamUnsettled`] if the temporary turn did not settle,
    /// which leaves that conversation standing, or if the source's own
    /// stream did not; [`CoreError::ClaimLost`] if the re-claim finds no
    /// winner; [`CoreError::Store`] if a read or a write fails.
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
                    // The design's "dont provide any tools" is the fork
                    // door's own word now: the framework records the empty
                    // tool choice into the temporary conversation ahead of
                    // the instructions, so this consumer supplies nothing
                    // here and cannot forget it.
                    records: Vec::new(),
                    instructions: COMPACTION_INSTRUCTIONS.to_owned(),
                },
            )
            .await?;
        let Some(summary) = self.capture_and_retire(temporary).await? else {
            return Err(CoreError::CompactionUnsummarized {
                conversation_id: source,
            });
        };
        self.install_compacted_thread(source, cut, summary, channel, kind)
            .await
    }

    /// Capture a temporary conversation's summary and retire the
    /// conversation, in that order, answering what was captured.
    ///
    /// The two steps are one method because the second is what makes the
    /// first safe to have run: the capture drives a model turn inside a
    /// conversation nothing else will ever serve, and the retirement is what
    /// stops that turn and takes the conversation away. Both doors that fork
    /// a temporary conversation come through here, so the ordering is
    /// decided once.
    ///
    /// A retirement that FAILS is what the caller hears, even when the
    /// capture failed too (unit 51, 2026-09-01): the conversation is still
    /// standing with a turn that may still be writing into it, and nothing
    /// may be built on top of that.
    ///
    /// This is also where the capture's outer bound is stated, once.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the temporary turn did not settle,
    /// at the capture's bound or at the retirement;
    /// [`CoreError::Store`] if a read or a write fails.
    async fn capture_and_retire(
        &self,
        temporary: TemporaryConversation,
    ) -> Result<Option<String>, CoreError> {
        let captured = self
            .capture_summary(temporary, tokio::time::Instant::now() + SUMMARY_BOUND)
            .await;
        // The retirement runs even when the capture's own settle at the bound
        // FAILED, and the deletion that follows is still sound. That settle
        // dropped its observation as it failed, and its interrupt latched the
        // conversation's writes and swept its streaming tails — the framework
        // discards a provider response arriving while latched — so this
        // second settle reads a conversation nothing can write into and
        // returns at once. If the tail is somehow still there, this settle
        // fails in turn and deletes nothing, which is A2's rule holding.
        match self.retire(temporary.conversation_id).await {
            Ok(()) => captured,
            Err(retirement) => {
                if let Err(error) = captured {
                    tracing::warn!(
                        temporary = temporary.conversation_id,
                        %error,
                        "the compaction's capture failed as well; the retirement's failure is what is reported"
                    );
                }
                Err(retirement)
            }
        }
    }

    /// Run the temporary conversation's one turn and read its answer.
    ///
    /// The conversation was born latched like every other, so nothing has
    /// driven it: the unlatch below is what starts the turn, and subscribing
    /// BEFORE it is what keeps a change the turn fires at once from being
    /// missed.
    ///
    /// The capture is over when the summary is DURABLE (unit 51,
    /// 2026-09-01), which is two stored facts and needs both. The framework
    /// answers the first — the turn is durably over: nothing streaming, and
    /// no tool outcome awaiting the round it summons. The ledger answers the
    /// second — prose is there. Neither alone is the answer: the predicate
    /// reads true over a conversation whose turn has not started, because a
    /// forked history is all durable, and prose is there at every MESSAGE
    /// end, while a tool-use stop's lifecycles arrive after one and the turn
    /// carries on.
    ///
    /// A turn that ends having written NO prose is over too, and saying so
    /// takes a THIRD fact: that a stream terminal for this conversation has
    /// been seen since the unlatch, which is [`CaptureWakes`]'s. Without it
    /// the two stored facts cannot tell "the turn ran and wrote nothing" from
    /// "the turn has not started" — a freshly forked temporary conversation
    /// is all durable and owes no outcome — and the capture would conclude
    /// every compaction before its turn ran. With it, the incident's own
    /// shape ends the capture in milliseconds: a provider error ends the turn
    /// writing nothing, and this method runs INLINE in the one shared
    /// compaction driver, so parking that case to the bound would stall every
    /// other conversation's door for three minutes.
    ///
    /// This does not make a stream event the answer. The terminal decides
    /// nothing by itself — over a tool-use stop the predicate is false, and
    /// over a turn with prose the ledger answers — and a terminal a lagged
    /// subscription drops costs only the bound, which is the wait the capture
    /// would have spent anyway.
    ///
    /// Every read is a level read off the store, woken by the changes this
    /// conversation's own writes emit — the compaction driver's own pattern.
    /// A lagged or dropped wake costs no CORRECTNESS, because the next one
    /// re-reads the same standing facts; it is not free of WORK, which is why
    /// a burst of wakes collapses into one read ([`CaptureWakes::next`]).
    ///
    /// `deadline` is the outer bound. On its expiry the turn is interrupted
    /// and that interrupt's own settle is awaited before anything else
    /// happens, so a turn nothing ended is STOPPED instead of left writing
    /// into a conversation the caller is about to retire; whatever prose the
    /// ledger holds by then is the answer.
    ///
    /// What counts as the answer is the newest assistant-voiced text past
    /// the instructions block — past it, because everything before it is the
    /// inherited history the turn was asked ABOUT, including the source's
    /// own earlier answers.
    ///
    /// `None` is a turn that produced no prose: it failed, it was silent, or
    /// it never ran. The caller changes nothing on that answer.
    ///
    /// # Errors
    ///
    /// [`CoreError::StreamUnsettled`] if the interrupt at the bound did not
    /// settle; [`CoreError::Store`] if a read fails.
    async fn capture_summary(
        &self,
        temporary: TemporaryConversation,
        deadline: tokio::time::Instant,
    ) -> Result<Option<String>, CoreError> {
        // Subscribed BEFORE the unlatch below, which is what starts the turn,
        // so a change or a terminal the turn fires at once is already in this
        // queue.
        let mut wakes = CaptureWakes::subscribed(self.ctx.bus(), temporary.conversation_id);
        self.ctx.bus().emit(CoreEvent::UnlatchRequested {
            conversation_id: temporary.conversation_id,
        });
        loop {
            match self.captured_turn(temporary).await? {
                CapturedTurn::Wrote(summary) => return Ok(Some(summary)),
                CapturedTurn::Silent if wakes.turn_ended() => {
                    tracing::warn!(
                        temporary = temporary.conversation_id,
                        "the compaction's turn ended having written nothing; the capture answers with no summary"
                    );
                    return Ok(None);
                }
                CapturedTurn::Silent | CapturedTurn::Running => {}
            }
            if !wakes.next(deadline).await {
                break;
            }
        }
        tracing::warn!(
            temporary = temporary.conversation_id,
            "the compaction's turn was not durably over at its bound; it is interrupted and settled, and the ledger decides what it wrote"
        );
        self.settle(temporary.conversation_id).await?;
        let blocks = self
            .ctx
            .store()
            .list_blocks(temporary.conversation_id)
            .await?;
        Ok(summary_of(&blocks, temporary.instructions_block_id))
    }

    /// What the temporary conversation's ledger says about its turn right
    /// now: the framework's durable-turn predicate first — prose that is
    /// there before the turn ends is prose the turn may still add to — and
    /// then, only past it, what the turn wrote.
    ///
    /// Three answers, not two, because the caller acts differently on each
    /// and one `Option` cannot carry them: a predicate that reads false and a
    /// turn that is over having written nothing are the same silence to a
    /// reader that only asks for a summary, and the second is a conclusion
    /// while the first is a wait.
    ///
    /// # Errors
    ///
    /// [`CoreError::Store`] if a read fails.
    async fn captured_turn(
        &self,
        temporary: TemporaryConversation,
    ) -> Result<CapturedTurn, CoreError> {
        let agency = self.agency(temporary.conversation_id);
        if !ratchet::turn_durably_over::<AssistantKind, _>(&agency).await? {
            return Ok(CapturedTurn::Running);
        }
        let blocks = self
            .ctx
            .store()
            .list_blocks(temporary.conversation_id)
            .await?;
        Ok(match summary_of(&blocks, temporary.instructions_block_id) {
            Some(summary) => CapturedTurn::Wrote(summary),
            None => CapturedTurn::Silent,
        })
    }

    /// One conversation's framework collaborators, for the ledger questions
    /// only the framework can answer.
    fn agency(&self, conversation_id: i64) -> AgencyCtx<CoreEvent> {
        AgencyCtx {
            conversation_id,
            store: self.ctx.store().clone(),
            bus: Arc::clone(self.ctx.bus()),
        }
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
    /// [`CoreError::StreamUnsettled`] if the loser's conversation did not
    /// settle before its deletion, which leaves it standing unmapped — a
    /// successor is latched and has run no turn, so nothing can be writing
    /// into it, and the variant is here because the retirement raises it;
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
    /// nothing; [`CoreError::StreamUnsettled`] if a stream did not settle —
    /// the serving thread's own before the rebuild, or, since unit 51, a
    /// retired lineage member's after the swap, which stops the retirement
    /// walk with the swap already verified and the members past it standing
    /// unmapped; [`CoreError::Store`] if a read or a write fails.
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
    /// nothing; [`CoreError::StreamUnsettled`] if a stream did not settle —
    /// the serving thread's own before the rebuild, or, since unit 51, a
    /// retired lineage member's after the swap, which stops the retirement
    /// walk with the swap already verified and the members past it standing
    /// unmapped; [`CoreError::Store`] if a read or a write fails.
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
    /// collector. A retirement that itself fails is logged and not
    /// raised, and it fails two ways now (unit 51, 2026-09-01): the settle
    /// ahead of the deletion can fail, in which case nothing is deleted and
    /// the conversation stands, or the deletion can. Neither is raised — the
    /// caller is already reporting why nothing was installed, and an unmapped
    /// conversation nothing serves is not a second failure to hand back.
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
                    "an unused conversation was not retired; it holds no mapping and nothing serves it, and a settle that failed leaves it standing"
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
    /// nothing; [`CoreError::StreamUnsettled`] if the temporary turn did not
    /// settle, which leaves that conversation standing;
    /// [`CoreError::Store`] if a read or a write fails.
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
                    records: Vec::new(),
                    instructions: COMPACTION_INSTRUCTIONS.to_owned(),
                },
            )
            .await?;
        match self.capture_and_retire(temporary).await? {
            Some(summary) => Ok(Some(summary)),
            // Capture-first: nothing established has been touched yet, and
            // the clones built so far go with the failure at the caller.
            None => Err(CoreError::CompactionUnsummarized {
                conversation_id: successor,
            }),
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
    /// dropped costs nothing.
    ///
    /// It is also HALF of what limits the unattended operation, and saying
    /// so plainly is the point (unit 51, 2026-09-01). A compaction that
    /// SUCCEEDS unmaps its source the moment the successor claims the
    /// channel, so however many late appends wake that conversation, it is
    /// never compacted again. A compaction that FAILS changes nothing: the
    /// source keeps its mapping and its history, the door it came through
    /// still stands, and the next block change on it starts another attempt.
    /// That repetition is unbounded by decision 0165 and carries no backoff,
    /// no cooldown and no stand-down; what keeps it from spinning is that a
    /// wake now means real activity in the conversation.
    ///
    /// One failure DOES leave something behind that wakes the driver, and
    /// this read is what makes it cost a read and nothing more. When the
    /// temporary conversation's turn will not settle, that conversation is
    /// deliberately not deleted — deleting under a live turn is the race the
    /// settle exists to close — so it stands unmapped but unlatched, and a
    /// turn deaf to the interrupt keeps writing into it. Every one of those
    /// writes wakes the driver on the TEMPORARY's id, and a temporary
    /// conversation is mapped to no channel, so this read answers `None` and
    /// the attempt ends there. The source is untouched by any of it: it keeps
    /// its mapping and its history, and its own next genuine change is what
    /// starts the next attempt.
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

/// The captured summary in one temporary conversation's ledger: the newest
/// non-empty assistant-voiced text past the instructions block, or `None`
/// when the turn wrote no prose.
///
/// Past the instructions block, because everything before it is the
/// inherited history the turn was asked ABOUT, including the source's own
/// earlier answers.
fn summary_of(blocks: &[Block], instructions_block_id: i64) -> Option<String> {
    blocks
        .iter()
        .rev()
        .take_while(|block| block.id != instructions_block_id)
        .filter(|block| {
            block.role == Some(Role::Assistant) && Text::KINDS.contains(&block.block_type.as_str())
        })
        .map(|block| Text::parse(block).content.trim().to_owned())
        .find(|content| !content.is_empty())
}

/// What one temporary conversation's ledger says about its turn.
enum CapturedTurn {
    /// The turn is not durably over: something is still streaming, or an
    /// outcome is awaiting the round it summons.
    Running,
    /// The turn is durably over, and this is the prose it wrote.
    Wrote(String),
    /// The turn is durably over and there is no prose past the instructions.
    /// Read alone this is also what a conversation whose turn never started
    /// looks like, which is why the capture needs [`CaptureWakes`] beside it.
    Silent,
}

/// The capture's subscription to the bus, and the one fact it carries across
/// wakes: whether a stream terminal for this conversation has been seen since
/// the unlatch.
///
/// A wake is a WAKEUP and nothing else — what it carries is never the answer,
/// because the capture re-reads stored state either way — with one exception
/// that is a fact and not a decision: a terminal for this conversation says
/// the turn RAN, and no ledger row records that for a turn that wrote
/// nothing. So the terminals are folded in as they pass, and the capture
/// still concludes on the store.
///
/// Two kinds of event wake the capture: a block change on this conversation,
/// which is new stored state to read, and a terminal, which is the third fact
/// changing. The waiting itself — and what a lag or a closed bus costs — is
/// [`streams::event_before`]'s, shared with the settle's own wait.
struct CaptureWakes {
    events: tokio::sync::broadcast::Receiver<CoreEvent>,
    conversation_id: i64,
    turn_ended: bool,
}

impl CaptureWakes {
    /// Subscribe. The caller unlatches AFTER this, so nothing the turn fires
    /// can arrive before the queue exists.
    fn subscribed(bus: &EventBus<CoreEvent>, conversation_id: i64) -> Self {
        Self {
            events: bus.subscribe(),
            conversation_id,
            turn_ended: false,
        }
    }

    /// Whether a stream terminal for this conversation has been seen since
    /// the unlatch — done, error or closed, the same terminal set the stream
    /// observer keys on, because they are the three ways a turn can end.
    ///
    /// A terminal lost to a lagged subscription leaves this `false`, so the
    /// capture waits out its bound: the cost of a dropped wake is the wait,
    /// never a wrong answer.
    fn turn_ended(&self) -> bool {
        self.turn_ended
    }

    /// Wait for the next wake, then take every wake already queued behind it,
    /// answering whether one arrived before the deadline.
    ///
    /// The second half is what keeps the cost off the store (unit 51,
    /// 2026-09-01). Every streamed delta is a block change, so a turn writing
    /// prose wakes this once per delta, and each read materializes the whole
    /// forked ledger — thousands of blocks — on the store's single thread,
    /// queued behind every other conversation's traffic. The wake is
    /// level-triggered, so a burst carries no more information than its last
    /// event does: taking the queue empty here turns the burst into ONE read.
    /// Nothing about the level read weakens — the read still happens, still
    /// off stored state — and a wake dropped inside the drain costs what a
    /// lagged wake costs, which is nothing.
    async fn next(&mut self, deadline: tokio::time::Instant) -> bool {
        let woken = {
            let Self {
                events,
                conversation_id,
                turn_ended,
            } = self;
            let conversation_id = *conversation_id;
            streams::event_before(events, deadline, |event| {
                Self::wakes_on(event, conversation_id, turn_ended)
            })
            .await
        };
        if woken {
            self.take_queued();
        }
        woken
    }

    /// Take every event already queued, without waiting for one.
    fn take_queued(&mut self) {
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    Self::wakes_on(&event, self.conversation_id, &mut self.turn_ended);
                }
                // A lag inside the drain drops events the caller's own read
                // covers, so the drain simply carries on from what is left.
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                Err(
                    tokio::sync::broadcast::error::TryRecvError::Empty
                    | tokio::sync::broadcast::error::TryRecvError::Closed,
                ) => return,
            }
        }
    }

    /// Fold one event into `turn_ended`, answering whether it wakes the
    /// capture.
    ///
    /// Which event is a stream's terminal is asked of `streams`, never
    /// matched here: the stream protocol's event identity is that module's
    /// one answer, and a copy of it in this one would drift from the
    /// settle's reading of the same set.
    fn wakes_on(event: &CoreEvent, conversation_id: i64, turn_ended: &mut bool) -> bool {
        if streams::is_stream_terminal(event, conversation_id) {
            *turn_ended = true;
            return true;
        }
        matches!(
            event,
            CoreEvent::BlocksChanged {
                conversation_id: id,
                ..
            } if *id == conversation_id
        )
    }
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

#[cfg(test)]
mod tests {
    use agent_ledger::event::stream_status;
    use agent_ledger::store::ToolCallInsert;
    use agent_ledger::{ProviderRegistry, Store, ToolRegistry};

    use super::*;
    use crate::schema::store_config;

    /// A sessions object over an in-memory store, a bus of this test's own
    /// and nothing registered: no provider, no tool, no reactor. Every event
    /// on this bus is one the test emitted and every block in the ledger is
    /// one the test wrote, which is what makes the capture's reads below
    /// readings of stored state and of nothing else.
    fn quiet_sessions() -> (
        Arc<Sessions>,
        Store,
        Arc<EventBus<CoreEvent>>,
        Arc<ContextWatch>,
    ) {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let bus = Arc::new(EventBus::new());
        let ctx = RuntimeContext::new(
            store.clone(),
            Arc::clone(&bus),
            Arc::new(ProviderRegistry::new()),
            Arc::new(ToolRegistry::new()),
        );
        let context = Arc::new(ContextWatch::new(streams::spawn_observer(&bus), None));
        let sessions = Sessions::new(
            ctx,
            ModelBinding {
                provider_instance: "p".into(),
                provider_display_name: "P".into(),
                vendor: "v".into(),
                model: "m".into(),
                model_display_name: "M".into(),
                context_window: None,
            },
            ReasoningLevel::Low,
            "the system prompt".into(),
            Vec::new(),
            SessionCoordination {
                stamp_lock: Arc::new(Mutex::new(())),
                erasure_fence: Arc::new(tokio::sync::RwLock::new(())),
                context: Arc::clone(&context),
            },
        );
        (Arc::new(sessions), store, bus, context)
    }

    /// A source conversation with a short history, and the temporary
    /// conversation a compaction forks off it — the same door
    /// [`Sessions::compact`] takes, so the ledger under test has the fork's
    /// own junction, tool-choice and instructions blocks in it.
    async fn forked_temporary(store: &Store) -> TemporaryConversation {
        let source = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let first = store
            .insert_final_text_block(source, Role::User, "the first half".into(), None)
            .await
            .expect("the first half stores");
        store
            .insert_final_text_block(source, Role::Assistant, "an earlier answer".into(), None)
            .await
            .expect("the earlier answer stores");
        store
            .fork_temporary(
                source,
                first,
                TemporaryFork {
                    records: Vec::new(),
                    instructions: COMPACTION_INSTRUCTIONS.to_owned(),
                },
            )
            .await
            .expect("the temporary conversation forks")
    }

    /// Record a block's dispatch anchor — the turn it belongs to. The
    /// anchored destination is the framework's own writer surface, so a test
    /// writes the same header through the domain seam, as this crate's other
    /// suites do.
    async fn anchor_on(store: &Store, block_id: i64, anchor: i64) {
        domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
            conn.execute(
                "UPDATE blocks SET dispatch_anchor = ?2 WHERE id = ?1",
                [block_id, anchor],
            )?;
            Ok(())
        })
        .await
        .expect("the anchor writes");
    }

    /// Assistant prose in the temporary conversation's turn — anchored on
    /// the instructions block, which is what the runtime anchors a summoned
    /// turn's products on.
    async fn answer(store: &Store, temporary: TemporaryConversation, content: &str) -> i64 {
        let block = store
            .insert_final_text_block(
                temporary.conversation_id,
                Role::Assistant,
                content.to_owned(),
                None,
            )
            .await
            .expect("the prose stores");
        anchor_on(store, block, temporary.instructions_block_id).await;
        block
    }

    /// A tool call and an error outcome answering it, under the same turn.
    /// The outcome copies the call's anchor at the resolution write and
    /// leaves the model owed the round it summons, which is the second half
    /// of the framework's durable-turn predicate and the whole reason these
    /// tests write one.
    ///
    /// It is an error outcome and NOT a consumer's own decline, which is a
    /// typed refusal since unit 51 and reaches the ledger marked as one. The
    /// framework's marked writer is crate-private there on purpose — a
    /// consumer's decline arrives through `ToolOutcome::Refused` and the
    /// runner sets the fact — so a test outside the framework has no door to
    /// the refused shape and writes the unmarked one. Nothing here reads that
    /// mark: a turn is owed its continuation over either shape, which is what
    /// the capture asks.
    async fn failed_call(store: &Store, temporary: TemporaryConversation) {
        let call = store
            .insert_tool_call_block(
                temporary.conversation_id,
                Role::Assistant,
                ToolCallInsert {
                    tool_call_id: "call-0".into(),
                    name: "probe".into(),
                    input: "{}".into(),
                    interactive: false,
                },
                None,
            )
            .await
            .expect("the call stores");
        anchor_on(store, call, temporary.instructions_block_id).await;
        store
            .fail_tool_call_block(
                temporary.conversation_id,
                "call-0".into(),
                "the tool call failed".into(),
                call,
            )
            .await
            .expect("the decline stores");
    }

    /// The block-change wake the runtime fires after a write, which is what
    /// the capture listens for.
    fn wake(bus: &EventBus<CoreEvent>, conversation_id: i64) {
        bus.emit(CoreEvent::BlocksChanged {
            conversation_id,
            block_id: 0,
        });
    }

    /// The runtime's own answer to an interrupt, spawned: the streaming tail
    /// is swept and the interrupt records itself, which is the stored state
    /// every settle confirms. One helper because both settle paths under test
    /// need the identical answer, and two hand-rolled copies would eventually
    /// answer differently.
    ///
    /// `takes` is how long the answer takes about it, so a caller that
    /// emitted the interrupt without awaiting its settle would answer while
    /// the tail still stands — which is what a test asserting on stored state
    /// afterwards reads.
    ///
    /// `tail` is `None` for a turn that already committed its text and so has
    /// no tail left to sweep: the runtime's interrupt still records itself
    /// there, which is exactly the case where the status block alone carries
    /// the settle.
    ///
    /// The task answers whether the conversation still existed at the moment
    /// the interrupt arrived: it is read before either write, so it is read
    /// before any settle can return and before any deletion can follow one.
    /// It panics if the interrupt never arrives, because a settle nobody
    /// asked for is the failure these tests are about.
    fn answers_the_interrupt(
        store: &Store,
        bus: &EventBus<CoreEvent>,
        temporary: TemporaryConversation,
        tail: Option<i64>,
        takes: Duration,
    ) -> tokio::task::JoinHandle<bool> {
        let store = store.clone();
        let mut events = bus.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                if !matches!(
                    event,
                    CoreEvent::InterruptRequested { conversation_id }
                        if conversation_id == temporary.conversation_id
                ) {
                    continue;
                }
                let stood = store
                    .find_conversation(temporary.conversation_id)
                    .await
                    .expect("the conversation table reads")
                    .is_some();
                tokio::time::sleep(takes).await;
                if let Some(tail) = tail {
                    store
                        .discard_streaming_block(tail)
                        .await
                        .expect("the tail is swept");
                }
                store
                    .insert_status_block(
                        temporary.conversation_id,
                        Status::TURN_ENDED_CLOSED.to_owned(),
                        None,
                    )
                    .await
                    .expect("the interrupt records itself");
                return stood;
            }
            panic!("nothing ever asked the turn to stop");
        })
    }

    /// Give a spawned capture room to run and assert it has NOT concluded.
    async fn still_open(capture: &tokio::task::JoinHandle<Result<Option<String>, CoreError>>) {
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !capture.is_finished(),
            "the capture concluded over a turn that is not durably over"
        );
    }

    /// What a spawned capture came to, under a bound short enough that a
    /// capture waiting for [`SUMMARY_BOUND`] fails this instead of passing.
    async fn concluded(
        capture: tokio::task::JoinHandle<Result<Option<String>, CoreError>>,
    ) -> Option<String> {
        tokio::time::timeout(Duration::from_secs(10), capture)
            .await
            .expect("the capture concludes on the change that ended the turn")
            .expect("the capture ran to its answer")
            .expect("the capture read the ledger")
    }

    /// The compaction fork's tool record is the LIBRARY's, and this
    /// assistant supplies none of its own (unit 52, 2026-09-01): the
    /// temporary conversation carries exactly one recorded choice, it is
    /// empty, and it sits ahead of the instructions block whose append
    /// summons the turn. Two of them would mean this consumer wrote one
    /// beside the library's; a non-empty one would mean the compaction turn
    /// was offered something to call.
    #[tokio::test]
    async fn the_compaction_fork_carries_exactly_the_librarys_empty_choice() {
        let (_sessions, store, _bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;

        let blocks = store
            .list_blocks(temporary.conversation_id)
            .await
            .expect("the ledger reads");
        let choices: Vec<usize> = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| block.block_type == "tool_choice")
            .map(|(at, _)| at)
            .collect();
        assert_eq!(
            choices.len(),
            1,
            "one recorded choice, the library's: {:?}",
            blocks
                .iter()
                .map(|block| block.block_type.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            store
                .newest_tool_choice(temporary.conversation_id)
                .await
                .expect("the recorded choice reads"),
            Some(Vec::new()),
            "the compaction turn has no tools"
        );
        let instructions_at = blocks
            .iter()
            .position(|block| block.id == temporary.instructions_block_id)
            .expect("the instructions block stands");
        assert!(
            choices[0] < instructions_at,
            "the record is in place before the append that summons the turn"
        );
    }

    /// A message's end is not a turn's end. The turn writes prose, reaches
    /// for a tool, gets an outcome back, and carries on — and the capture must be open
    /// at the end of the first message and answer with what the SECOND one
    /// wrote. This is the early capture the unit fixes: the old capture ended
    /// on `StreamDone` and took the half-written prose.
    #[tokio::test]
    async fn a_message_end_mid_tool_round_does_not_conclude_the_capture() {
        let (sessions, store, bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        answer(&store, temporary, "half of what was asked for").await;
        failed_call(&store, temporary).await;

        let capture = tokio::spawn({
            let sessions = Arc::clone(&sessions);
            async move {
                sessions
                    .capture_summary(
                        temporary,
                        tokio::time::Instant::now() + Duration::from_secs(30),
                    )
                    .await
            }
        });

        bus.emit(CoreEvent::StreamDone {
            conversation_id: temporary.conversation_id,
            usage: None,
            stop_reason: None,
            generation: None,
        });
        wake(&bus, temporary.conversation_id);
        still_open(&capture).await;

        answer(&store, temporary, "the whole summary").await;
        wake(&bus, temporary.conversation_id);
        assert_eq!(
            concluded(capture).await,
            Some("the whole summary".to_owned()),
            "the capture answers with what the turn wrote after its tool round"
        );
    }

    /// The bus drops events under load, and a dropped one may be the very
    /// change that ended the turn. A lag wakes the capture into another level
    /// read and decides nothing itself: over a turn still owed a round it
    /// concludes nothing, and the capture is still there to answer the next
    /// real change.
    #[tokio::test]
    async fn a_lagged_wake_concludes_nothing_and_leaves_the_capture_running() {
        let (sessions, store, bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        answer(&store, temporary, "half of what was asked for").await;
        failed_call(&store, temporary).await;

        let mut watch = bus.subscribe();
        let capture = tokio::spawn({
            let sessions = Arc::clone(&sessions);
            async move {
                sessions
                    .capture_summary(
                        temporary,
                        tokio::time::Instant::now() + Duration::from_secs(30),
                    )
                    .await
            }
        });
        // The capture's own unlatch is the signal that it has subscribed, so
        // the flood below overruns a subscription that exists.
        loop {
            match watch.recv().await {
                Ok(CoreEvent::UnlatchRequested { conversation_id })
                    if conversation_id == temporary.conversation_id =>
                {
                    break;
                }
                Ok(_) => {}
                Err(error) => panic!("the capture never asked for its turn: {error}"),
            }
        }

        // More events than the bus holds, emitted without yielding: every
        // subscription that was parked misses some of them.
        for _ in 0..300 {
            bus.emit(CoreEvent::UnlatchRequested {
                conversation_id: -1,
            });
        }
        assert!(
            matches!(watch.recv().await, Err(RecvError::Lagged(_))),
            "the flood has to overrun a subscription for this test to read anything"
        );
        still_open(&capture).await;

        answer(&store, temporary, "the whole summary").await;
        wake(&bus, temporary.conversation_id);
        assert_eq!(
            concluded(capture).await,
            Some("the whole summary".to_owned()),
            "a capture that lagged still answers on the next change"
        );
    }

    /// A compaction whose turn finished writing is over: the capture
    /// concludes on the stored facts and the conversation is retired at once,
    /// nowhere near [`SUMMARY_BOUND`].
    #[tokio::test]
    async fn a_finished_toolless_turn_is_captured_and_retired_before_the_bound() {
        let (sessions, store, _bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        answer(&store, temporary, "the whole summary").await;

        let captured = tokio::time::timeout(
            Duration::from_secs(10),
            sessions.capture_and_retire(temporary),
        )
        .await
        .expect("a durable summary ends the capture at once, not at the bound")
        .expect("the capture and the retirement both succeed");

        assert_eq!(captured, Some("the whole summary".to_owned()));
        assert!(
            store
                .find_conversation(temporary.conversation_id)
                .await
                .expect("the conversation table reads")
                .is_none(),
            "a captured temporary conversation is retired"
        );
    }

    /// A turn that has not started yet must NOT end the capture, and that is
    /// the whole reason the empty-turn answer below needs a third fact. The
    /// ledger here is a fresh fork: every block durable, no outcome owed, no
    /// prose past the instructions — which is bit for bit what a turn that
    /// ran and wrote nothing looks like. No terminal has been seen, so the
    /// capture waits, and it is still there to answer when the turn writes.
    #[tokio::test]
    async fn a_turn_that_has_not_started_does_not_conclude_the_capture() {
        let (sessions, store, bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;

        let capture = tokio::spawn({
            let sessions = Arc::clone(&sessions);
            async move {
                sessions
                    .capture_summary(
                        temporary,
                        tokio::time::Instant::now() + Duration::from_secs(30),
                    )
                    .await
            }
        });

        // Block changes, and plenty of them, with no terminal among them: a
        // wake is not evidence that the turn ran.
        for _ in 0..5 {
            wake(&bus, temporary.conversation_id);
        }
        still_open(&capture).await;

        answer(&store, temporary, "the whole summary").await;
        wake(&bus, temporary.conversation_id);
        assert_eq!(
            concluded(capture).await,
            Some("the whole summary".to_owned()),
            "the capture that waited out the empty ledger answers what the turn wrote"
        );
    }

    /// A turn that ENDS having written nothing ends the capture at once, and
    /// this is the incident's own shape: the provider errors, the framework
    /// discards the streaming tails and stores no prose, and from then on the
    /// ledger reads durably over and empty forever. The three facts are all
    /// in — the predicate true, no prose, a terminal seen — so the capture
    /// answers `None` and the compaction fails unsummarized.
    ///
    /// The capture's own bound here is [`SUMMARY_BOUND`], three minutes, and
    /// the whole compaction is awaited under five seconds: a capture that
    /// parked this turn to its bound fails this test instead of passing it
    /// slowly. The stall is the point — the capture runs inline in the one
    /// shared compaction driver, so three minutes here is three minutes of
    /// every other conversation's door.
    #[tokio::test]
    async fn a_turn_that_ends_writing_nothing_fails_the_compaction_at_once() {
        let (sessions, store, bus, _context) = quiet_sessions();
        let source = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        for turn in 0..3 {
            store
                .insert_final_text_block(source, Role::User, format!("question {turn}"), None)
                .await
                .expect("the question stores");
            store
                .insert_final_text_block(source, Role::Assistant, format!("answer {turn}"), None)
                .await
                .expect("the answer stores");
        }

        // Subscribed before the compaction starts, so the unlatch that names
        // the temporary conversation cannot be missed.
        let mut watch = bus.subscribe();
        let compaction = tokio::spawn({
            let sessions = Arc::clone(&sessions);
            async move {
                sessions
                    .compact(
                        source,
                        &ChannelKey {
                            adapter: "test".into(),
                            channel: "c".into(),
                        },
                        ChannelKind::Group,
                    )
                    .await
            }
        });

        let temporary = loop {
            match watch.recv().await {
                Ok(CoreEvent::UnlatchRequested { conversation_id }) => break conversation_id,
                Ok(_) => {}
                Err(error) => panic!("the compaction never asked for its turn: {error}"),
            }
        };
        // What the framework does to a turn its provider fails: the error IS
        // the turn's end, the streaming tails are discarded, and no prose is
        // ever written.
        bus.emit(CoreEvent::StreamError {
            conversation_id: temporary,
            error: "the provider failed the turn".into(),
            generation: None,
        });

        let failure = tokio::time::timeout(Duration::from_secs(5), compaction)
            .await
            .expect("the capture answers on the turn's end, not at SUMMARY_BOUND")
            .expect("the compaction ran to its answer")
            .expect_err("a turn that wrote nothing fails the compaction");
        assert!(
            matches!(
                failure,
                CoreError::CompactionUnsummarized { conversation_id }
                    if conversation_id == source
            ),
            "the failure names the conversation nothing could be summarized for: {failure}"
        );
        assert!(
            store
                .find_conversation(temporary)
                .await
                .expect("the conversation table reads")
                .is_none(),
            "the temporary conversation is retired even though its turn wrote nothing"
        );
    }

    /// A turn nothing ended reaches the bound, and the bound STOPS it: the
    /// capture interrupts and waits for that interrupt's own settle before it
    /// answers, so the conversation the caller retires next is not still
    /// being written into. Whatever prose the ledger holds by then is the
    /// answer.
    #[tokio::test]
    async fn the_bound_interrupts_the_turn_and_awaits_its_settle() {
        let (sessions, store, bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        answer(&store, temporary, "what the turn managed to write").await;
        let tail = store
            .insert_streaming_block(temporary.conversation_id, Role::Assistant)
            .await
            .expect("the streaming tail stores");

        // The answer takes its time, so a capture that emitted the interrupt
        // without awaiting the settle would answer while the tail still
        // stands — which is what the assertions below read.
        let interrupted = answers_the_interrupt(
            &store,
            &bus,
            temporary,
            Some(tail),
            Duration::from_millis(150),
        );

        let captured = sessions
            .capture_summary(
                temporary,
                tokio::time::Instant::now() + Duration::from_millis(100),
            )
            .await
            .expect("the interrupt settles");

        assert_eq!(
            captured,
            Some("what the turn managed to write".to_owned()),
            "the ledger's prose at the bound is the answer"
        );

        // The stored state the settle waits for, read at the moment the
        // capture answered: a capture that emitted the interrupt without
        // awaiting its settle would have answered before either write.
        assert!(
            !streams::has_streaming_tail(&store, temporary.conversation_id)
                .await
                .expect("the ledger reads"),
            "the capture answers only once the interrupt has swept the streaming tail"
        );
        let blocks = store
            .list_blocks(temporary.conversation_id)
            .await
            .expect("the ledger reads");
        assert!(
            blocks
                .iter()
                .any(|block| Status::KINDS.contains(&block.block_type.as_str())),
            "the capture answers only once the interrupt has recorded itself"
        );
        let _stood = interrupted.await.expect("the interrupt was answered");
    }

    /// The retirement's ordering, watched from inside: when the turn is told
    /// to stop, the conversation it writes into is still there. That is what
    /// makes it impossible for a stopping turn's own writes to target a
    /// deleted conversation id.
    #[tokio::test]
    async fn a_retirement_settles_the_turn_before_it_deletes_the_conversation() {
        let (sessions, store, bus, _context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        let tail = store
            .insert_streaming_block(temporary.conversation_id, Role::Assistant)
            .await
            .expect("the streaming tail stores");

        let interrupted =
            answers_the_interrupt(&store, &bus, temporary, Some(tail), Duration::ZERO);

        sessions
            .retire(temporary.conversation_id)
            .await
            .expect("a settled turn retires");

        assert!(
            interrupted.await.expect("the interrupt was answered"),
            "the turn is told to stop while its conversation still stands"
        );
        assert!(
            store
                .find_conversation(temporary.conversation_id)
                .await
                .expect("the conversation table reads")
                .is_none(),
            "a settled conversation is deleted"
        );
    }

    /// A capture that beat the terminal still retires. The framework commits
    /// a turn's final text BEFORE it emits the stream's terminal, so a
    /// capture concluding off that committed text reaches the retirement
    /// while the observation can still read open — and the terminal it would
    /// wait for has already fired, before the settle subscribed, where no
    /// subscription can ever see it. Nothing here is a lag: it is the plain
    /// order of two writes.
    ///
    /// Scripted as that order leaves things: the observation is open, no
    /// terminal is ever emitted on this bus, and the runtime answers the
    /// interrupt for a turn with no tail left to sweep. A settle deciding
    /// from its wait spends the whole settle bound and then throws the good
    /// summary away; the settle reads the missing tail, takes no wait, and
    /// confirms from stored state — so the summary comes back, promptly, and
    /// the conversation is gone.
    #[tokio::test]
    async fn a_capture_that_beat_the_streams_terminal_still_retires() {
        let (sessions, store, bus, context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        answer(&store, temporary, "the whole summary").await;

        bus.emit(CoreEvent::StreamStatus {
            conversation_id: temporary.conversation_id,
            label: stream_status::WAITING_FOR_RESPONSE.to_owned(),
            subtitle: None,
        });
        // The observation has to be in before the settle reads it; the
        // measurement the same event records is what says it is.
        while context
            .streams()
            .last_dispatch(temporary.conversation_id)
            .is_none()
        {
            tokio::task::yield_now().await;
        }

        let interrupted = answers_the_interrupt(&store, &bus, temporary, None, Duration::ZERO);

        let summary = sessions
            .capture_and_retire(temporary)
            .await
            .expect("a committed summary survives an observation left open")
            .expect("the capture read the committed prose");
        assert_eq!(summary, "the whole summary");

        assert!(
            interrupted.await.expect("the interrupt was answered"),
            "the turn is told to stop while its conversation still stands"
        );
        assert!(
            store
                .find_conversation(temporary.conversation_id)
                .await
                .expect("the conversation table reads")
                .is_none(),
            "the retired conversation is deleted"
        );
    }

    /// A settle that FAILS deletes nothing. The compaction fails with the
    /// unsettled stream, the temporary conversation is still standing, and a
    /// write from the turn nobody could stop still has a live conversation to
    /// write into — which is the failure this ordering exists to prevent,
    /// read from the store instead of argued.
    ///
    /// The stream is observed open and no end signal ever arrives, so the
    /// settle spends its whole bound before failing; that wait is this test's
    /// running time.
    #[tokio::test]
    async fn a_stream_that_never_settles_leaves_the_conversation_standing() {
        let (sessions, store, bus, context) = quiet_sessions();
        let temporary = forked_temporary(&store).await;
        answer(&store, temporary, "the whole summary").await;

        let mut events = bus.subscribe();
        bus.emit(CoreEvent::StreamStatus {
            conversation_id: temporary.conversation_id,
            label: stream_status::WAITING_FOR_RESPONSE.to_owned(),
            subtitle: None,
        });
        // The observation has to be in before the settle reads it; the
        // measurement the same event records is what says it is.
        while context
            .streams()
            .last_dispatch(temporary.conversation_id)
            .is_none()
        {
            tokio::task::yield_now().await;
        }

        let failure = sessions
            .capture_and_retire(temporary)
            .await
            .expect_err("a stream that never settles fails the compaction");
        assert!(
            matches!(
                failure,
                CoreError::StreamUnsettled { conversation_id }
                    if conversation_id == temporary.conversation_id
            ),
            "the failure names the unsettled stream: {failure}"
        );

        let mut interrupted = false;
        while let Ok(event) = events.try_recv() {
            interrupted |= matches!(
                event,
                CoreEvent::InterruptRequested { conversation_id }
                    if conversation_id == temporary.conversation_id
            );
        }
        assert!(interrupted, "the retirement asked the turn to stop");
        assert!(
            store
                .find_conversation(temporary.conversation_id)
                .await
                .expect("the conversation table reads")
                .is_some(),
            "a conversation whose turn would not settle is not deleted"
        );
        store
            .insert_final_text_block(
                temporary.conversation_id,
                Role::Assistant,
                "a late write from the turn nobody stopped".into(),
                None,
            )
            .await
            .expect("the turn's late write still has a live conversation to write into");
    }
}
