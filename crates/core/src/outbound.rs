//! The outbound edge: a subscription that yields what the assistant puts
//! on one adapter's channels — a reply of words, or a reaction to place —
//! each bound to the channel key it belongs on.
//!
//! The framework's event subscription is the wake signal only. Events carry
//! no answer text, so on a completed stream the edge re-reads the answer
//! block from the ledger and maps the conversation back to its channel key.
//! Each edge serves exactly one adapter and skips every other adapter's
//! conversations, so two adapters run two edges and neither consumes the
//! other's items.
//!
//! # The delivery contract, stated honestly
//!
//! Answers already stored when the edge is taken are history and stay off it
//! — a restarted process must not repeat a channel's whole answer history.
//! An answer stored afterwards is delivered when its conversation next wakes
//! this edge: on the completed stream's own signal in the common case, or on
//! the full re-read a lag triggers — a subscriber that fell behind the event
//! backlog is told so by the bus and recovers from stored state, never by
//! replaying events. Nothing stronger holds: an answer whose wake signal
//! fell into a dropped-event window is owed until that channel's next wake
//! or the next lag pass. Delivery is therefore at-least-once from stored
//! state, with re-reads deduplicated by a per-conversation cursor.
//!
//! # The cursor's ordering assumption
//!
//! The cursor is the highest delivered block id per conversation, and the
//! seed takes it from the framework's newest-block read — which answers by
//! junction order, not by id. The two coincide because every block enters a
//! conversation through an append that allocates a fresh, higher id; the
//! framework's own re-derive discipline rests on the same monotonicity. If a
//! write path ever backfilled an older block id into a newer junction slot,
//! the seed would mark too little as history and re-deliver, never lose.
//!
//! # A conversation this edge has never seen (unit 45, 2026-08-30)
//!
//! A conversation with no cursor here starts at its INHERITED BOUNDARY: the
//! newest of its blocks that another conversation also holds, or zero when
//! it holds none. Zero alone rested on a premise that held while every
//! conversation was born empty and stopped holding the moment one could be
//! born from a fork. A fork inherits its source's history through the
//! junction, so its oldest blocks are answers this edge already delivered
//! from the source: seeding at zero would send every one of them to the chat
//! a second time, and would write a first-delivery disclosure line into
//! blocks the source still holds — the edit through a fork that detaching
//! exists to avoid.
//!
//! The boundary is exact rather than approximate, and it is a fact about
//! the blocks instead of a moment anyone has to catch. A junction row is
//! what makes a block part of a conversation, so a block two conversations
//! hold is a block this one inherited, and ids ascend along junction order —
//! so every inherited block sits at or below the newest shared one, and
//! every block this conversation authored for itself sits above it. A
//! conversation created fresh shares nothing with anybody and seeds at zero,
//! so first contact delivers its first answer normally.
//!
//! # A conversation id its previous holder left behind (unit 53, 2026-09-02)
//!
//! The store reissues conversation ids, and a process can delete a mapped
//! conversation while it runs — an erasure request has always been able to,
//! and the retention sweep now does it on a schedule, so this repairs the
//! older hole as well as the new one. An id whose cursor this edge holds can
//! therefore come back as somebody else's fresh session. A conversation
//! never loses a block while it lives, so a cursor standing above everything
//! its conversation holds is the previous holder's, and it re-seeds at the
//! inherited boundary like a conversation this edge has never seen. The
//! check is a level read of stored state, not a moment to catch: no deletion
//! has to tell this edge anything.
//!
//! The durable ratchet cursor is deliberately NOT the seed. It is the
//! frontier of what the model has been driven through, it moves with every
//! turn, and by the time a completed stream wakes this edge it already
//! stands past the very answer the wake is about — reading it here would
//! swallow that answer.
//!
//! # A conversation with no channel
//!
//! An item resolves its channel from the mapping at delivery time, so a
//! conversation the channel has moved off — a retired one, or the source of
//! a session reset — has nothing to deliver to and its stored items are
//! dropped, at warn level. That is the point of a reset and not a defect:
//! the session being replaced owes its unsent products to the record, not to
//! the chat.
//!
//! # A failed turn is silent in the chat (unit 49, 2026-08-31)
//!
//! On a stream error the edge delivers what the dead turn already put on
//! the ledger and then says nothing — for every cause, in every situation,
//! per decision 0192. No line goes out, so nothing about the failure is
//! addressed to the people in the chat; the latch still closes the
//! conversation and the next addressed message still re-engages it.
//!
//! The record moves to the log instead: the arm writes one info line per
//! stream error, naming the conversation and carrying the framework's own
//! rendering of the error. That line is unconditional because one of the
//! framework's emit sites writes nothing of its own, and a failure logged
//! nowhere would leave no trace at all. The title derivation the metadata
//! worker runs never finalizes an answer block in the conversation ledger,
//! so it never appears here.
//!
//! # The wire text drops a leaked reasoning prefix (unit 43, 2026-08-30)
//!
//! A model whose reasoning escapes into its answer leaves one closing
//! think-tag standing in the text with the whole trace in front of it.
//! This edge is the one place an answer's text becomes a platform's
//! message, so this is where the trace is cut: [`without_leaked_reasoning`]
//! states the rule — exactly one closer is the leak and dies with
//! everything before it, every other count stands byte for byte — the
//! answer arm applies it in one place, and the deterministic replies never
//! meet it.
//!
//! The cut reads the model's own prose, under any disclosure line an
//! earlier delivery already stored ([`Disclosure::prose_of`]), and it runs
//! ahead of both judgments below. So an answer that is nothing but
//! reasoning is silence, and the introduction is composed back in front of
//! the cut text instead of dying inside it — the wire text of an answer is
//! decided here, in one expression, and nowhere else.
//!
//! The stored block keeps the model's words whole — under the disclosure
//! line where a first delivery wrote one in (that line is the one
//! person-written prefix a stored answer may carry). The channel sees the
//! same content without the leaked prefix: stored and wire text differ by
//! exactly what the send cut.
//!
//! # An empty answer delivers nothing (unit 22, 2026-08-24)
//!
//! The model chooses silence by ending its turn without writing any text,
//! and the framework commits that turn as a real empty assistant text
//! block. This edge is where the choice takes effect: an undelivered
//! answer whose spoken text trims to nothing — the stored text with a
//! leaked reasoning prefix already cut, judged before any disclosure
//! resolution — is accounted delivered and yields nothing: no
//! empty send, no first-interaction introduction, the turn already closed
//! by its own committed block. The check precedes the disclosure prepend
//! on purpose: a prepended line would both fill the empty answer and
//! record an introduction nobody received. When the model was addressed
//! and could not back an answer with a lookup, it says "I don't know" in
//! its own words — ordinary prose to this edge, delivered like any answer
//! with no special routing.
//!
//! # The first delivery introduces the assistant (2026-08-23)
//!
//! An undelivered answer whose summoning people include anyone never yet
//! introduced has the disclosure line written into its stored block before
//! the send — the disclosure module owns the resolution and the receipt,
//! and the resolved [`Disclosure`] value arrives with the edge — and the
//! line opens the text that goes out, which is the stored answer with any
//! leaked reasoning prefix cut. The report line is a fixed text a person
//! wrote and is never touched.
//!
//! # An answer threads onto the one person who addressed the assistant
//! (unit 26, 2026-08-24)
//!
//! An answer is delivered as a reply to the message it answers when the
//! turn absorbed exactly one message that literally addressed the
//! assistant; [`answer_target`] below states the whole rule. The
//! summoning frontier is deliberately not consulted — it names whatever
//! line the dispatch woke on, routinely a bystander's — so the target is
//! read from the turn's absorbed messages instead, where the addressed
//! fact is stored per message. Nobody addressed the assistant, or several
//! did, and the answer goes out plain; in no case is it withheld.
//!
//! The thread carries its own recovery, stated here and obeyed by the
//! adapter ([`ReplyThread`]): an answer whose threaded send the platform
//! refuses goes out once more plain, because an answer must never be lost
//! to a courtesy, while a report's line is threaded or not delivered —
//! it is the moderation bot's command shape, which files nothing when it
//! is not a reply and would stand in the group as a bare command line.
//!
//! # The report's delivery (2026-08-23)
//!
//! A filed report block delivers as its stored fixed line, marked
//! [`ReplyKind::Report`] and threaded onto the reported message's origin —
//! independent of the answer: a silent turn's report still goes out,
//! since the empty-answer check below touches only [`ReplyKind::Answer`]
//! blocks — and on BOTH stream events: with the answer on the turn's
//! completion, where ledger order puts it before the answer text, and on
//! the turn's failure, where it is the whole of what that wake sends, so a
//! turn that dies after filing still files. The failure wake runs the same
//! stored-state read as a completion, so a dead turn's already-finalized
//! narration delivers beside its report instead of waiting for the
//! conversation's next wake. That read stays the full one and is never
//! narrowed to reports, because the cursor is one high-water mark per
//! conversation: a read narrowed to reports would either pass the
//! committed narration for good or repeat the report on the next wake,
//! and the contract refuses re-delivered reports above all. The accepted
//! losses are recorded plainly: a report undelivered
//! when the process dies is LOST — the restart seeding stands, and
//! re-delivering reports from history would ping every group admin
//! at-least-once; for a moderation nudge the safer direction is fewer
//! reports, never more. A report whose target an erasure nulled is
//! skipped as undeliverable.
//!
//! # The mark's placement (unit 39, 2026-08-30)
//!
//! A filed mark block yields the second arm of [`Outbound`]: the stored
//! emoji and the marked message's origin, and nothing else — no thread, no
//! disclosure, no delivery handle. It rides the same cursor, the same
//! wakes and the same lag recovery as a reply, so everything the delivery
//! contract above states holds for it word for word: a mark undelivered
//! when the process dies is LOST, and under the tool's per-origin
//! existence check that message then stays unmarked for good — accepted
//! for an act whose whole point is being cheap. A mark whose target an
//! erasure or the deletion mirror nulled is skipped as undeliverable,
//! exactly as a report is.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{Block, BlockKind, CoreEvent, FromBlock, Role, RuntimeContext, StoreError};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::disclosure::{Disclosure, Introduction};
use crate::kind::{AssistantKind, FrameworkKind};
use crate::mapping;
use crate::message::{
    DeliveryHandle, Outbound, OutboundMark, OutboundReply, ReplyKind, ReplyThread,
};
use crate::tools::provenance;

/// The rules acknowledgment's deterministic fallback (unit 20, 2026-08-24;
/// the fixed primary of 2026-08-23 until then). A real rules delta is
/// acknowledged with a bounded one-shot model completion in the
/// assistant's own voice; when that call fails, times out, or returns
/// nothing usable, this line delivers instead — so every real delta still
/// draws a visible acknowledgment, exactly the guarantee the fixed wording
/// carried. The on-delta comparison stays the whole admission check, and
/// an identical re-pin draws nothing.
pub const RULES_ACKNOWLEDGMENT: &str =
    "Rules noted. The assistant follows the pinned rules of this group.";

/// What the privacy command's answer opens with when an address is
/// configured; the address follows directly.
pub const PRIVACY_ANSWER_LEAD: &str = "Privacy policy: ";

/// The privacy command's answer when no address is configured.
pub const PRIVACY_UNPUBLISHED: &str = "The privacy policy is not published yet.";

/// The highest block id already accounted for, per conversation. Replies are
/// read from the ledger, so the cursor is what keeps a re-read from
/// repeating what this subscriber already delivered. Seeded from stored
/// state when the edge is taken, which is what marks everything before that
/// moment as history.
type DeliveryCursors = HashMap<i64, i64>;

/// Spawn the edge for one adapter and return its receiving end. The task
/// ends when the receiver is dropped — noticed even on an idle bus — or when
/// the bus closes.
///
/// The bus subscription is taken before the cursors are seeded, so an
/// answer finalized between the two reads is either inside the seed (and
/// history) or woken by its buffered event — never lost between them.
///
/// # Errors
///
/// [`StoreError`] if seeding the cursors from stored state fails.
pub(crate) async fn spawn_edge(
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    adapter: String,
    disclosure: Arc<Disclosure>,
) -> Result<mpsc::UnboundedReceiver<Outbound>, StoreError> {
    let mut events = ctx.bus().subscribe();
    let mut cursors = seed_cursors(&ctx, &adapter).await?;
    let (items, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let event = tokio::select! {
                // A dropped receiver ends the task even while the bus idles;
                // recv alone would park until the next event arrives.
                () = items.closed() => break,
                event = events.recv() => event,
            };
            match event {
                Ok(CoreEvent::StreamDone {
                    conversation_id, ..
                }) => {
                    if let Err(error) = deliver_stored_items(
                        &ctx,
                        &adapter,
                        &disclosure,
                        conversation_id,
                        &mut cursors,
                        &items,
                    )
                    .await
                    {
                        tracing::error!(conversation_id, %error, "outbound delivery failed");
                    }
                }
                // A failed turn tells the chat nothing (decision 0192).
                // What the dead turn already put on the ledger still goes
                // out, a filed report above all: the failure wake runs the
                // same stored-state read as a completion, so a turn that
                // dies after filing still files. Past that delivery the arm
                // only writes its record — one line per stream error,
                // whatever the cause, since one framework emit site logs
                // nothing of its own and the failure would otherwise be
                // written down nowhere.
                Ok(CoreEvent::StreamError {
                    conversation_id,
                    error,
                    ..
                }) => {
                    if let Err(failure) = deliver_stored_items(
                        &ctx,
                        &adapter,
                        &disclosure,
                        conversation_id,
                        &mut cursors,
                        &items,
                    )
                    .await
                    {
                        tracing::error!(conversation_id, %failure, "outbound delivery failed");
                    }
                    tracing::info!(
                        conversation_id,
                        %error,
                        "the failed turn passes without a word in the chat"
                    );
                }
                Ok(_) => {}
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "outbound edge lagged; re-reading stored state");
                    if let Err(error) =
                        recover_from_lag(&ctx, &adapter, &disclosure, &mut cursors, &items).await
                    {
                        tracing::error!(%error, "outbound lag recovery failed");
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    Ok(receiver)
}

/// The history boundary: every conversation mapped for this adapter starts
/// its cursor at its newest stored block — one row per conversation through
/// the framework's newest-block read, never a whole ledger — so nothing
/// already stored when the edge is taken is ever delivered.
async fn seed_cursors(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
) -> Result<DeliveryCursors, StoreError> {
    let tx = ctx.store().tx();
    let mut cursors = DeliveryCursors::new();
    for record in mapping::all(&tx).await? {
        if record.adapter != adapter {
            continue;
        }
        let newest = ctx
            .store()
            .latest_block(record.conversation_id)
            .await?
            .map_or(0, |block| block.id);
        cursors.insert(record.conversation_id, newest);
    }
    Ok(cursors)
}

/// Read the conversation's ledger and yield every undelivered answer,
/// report and mark block, in ledger order — which puts a turn's filed
/// blocks ahead of its answer, because the tool filed before the answer
/// finalized — bound to
/// the conversation's channel key. A report whose target origin an erasure
/// nulled is skipped as undeliverable and accounted delivered, so the
/// re-reads stop meeting it. A conversation that is not mapped, or is
/// mapped for another adapter, is none of this edge's business and yields
/// nothing.
///
/// # Errors
///
/// [`StoreError`] if a read fails or the store's actor has stopped. The send
/// half never errors here: a dropped receiver ends the task at the loop
/// instead.
async fn deliver_stored_items(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
    disclosure: &Disclosure,
    conversation_id: i64,
    cursors: &mut DeliveryCursors,
    items: &mpsc::UnboundedSender<Outbound>,
) -> Result<(), StoreError> {
    let tx = ctx.store().tx();
    let Some(channel) = mapping::channel_for_conversation(&tx, conversation_id).await? else {
        tracing::warn!(
            conversation_id,
            "the conversation maps to no channel; its stored items are not delivered"
        );
        return Ok(());
    };
    if channel.adapter != adapter {
        return Ok(());
    }
    let mut blocks = ctx.store().list_blocks(conversation_id).await?;
    let cursor = seated_cursor(&tx, cursors, conversation_id, &blocks).await?;
    for index in 0..blocks.len() {
        let block_id = blocks[index].id;
        if block_id <= *cursor {
            continue;
        }
        let Some(deliverable) = deliverable_of(&blocks[index]) else {
            continue;
        };
        match deliverable {
            Deliverable::Reply {
                text,
                kind,
                threading,
                quotable_block,
            } => {
                // An answer's whole path to the wire, in the order the
                // three steps must run. Only the model's own prose takes
                // it: the report line and every other deterministic reply
                // is a fixed text a person wrote and passes through the
                // other branch untouched — neither cut nor introduced.
                let text = if kind == ReplyKind::Answer {
                    // The cut comes FIRST (unit 43), and it reads the
                    // model's own prose: a block re-delivered after an
                    // earlier send already carries the disclosure line, and
                    // a cut over that line would drop the introduction
                    // instead of the trace. What is left is what the
                    // channel sees, so both steps below work on it.
                    let spoken = without_leaked_reasoning(disclosure.prose_of(&text));
                    // The empty-answer check (unit 22): the model ended
                    // its turn without speaking, so the answer is
                    // accounted delivered and yields nothing — and no
                    // disclosure resolution runs, since a spoken-to-nobody
                    // answer introduces nobody.
                    if spoken.trim().is_empty() {
                        tracing::debug!(
                            conversation_id,
                            block_id,
                            "the answer is empty; the turn speaks nothing"
                        );
                        *cursor = block_id;
                        continue;
                    }
                    // The first delivery resolves the first-interaction
                    // disclosure (decision 0079): the line is written into
                    // the stored block before the send, and the resolution
                    // answers with nothing but which of the two openings
                    // this answer goes out under. The composition is here,
                    // over the spoken text, so no seam can be handed a
                    // text the cut never saw.
                    match disclosure
                        .introduction_for(ctx.store(), conversation_id, &mut blocks, index)
                        .await?
                    {
                        Introduction::Lined => disclosure.disclosed(spoken),
                        Introduction::Bare => spoken.to_owned(),
                    }
                } else {
                    text
                };
                // The thread's target, where the block named a rule instead
                // of an origin (unit 26): the lookup walks the turn's
                // absorbed messages, so it belongs here, where the loaded
                // ledger already is, and not in the block-pure reading
                // below. It runs on the text that goes out, disclosure line
                // included, because that is the prose the moderation
                // command shape is read from.
                let reply_target = match threading {
                    Threading::Onto(thread) => Some(thread),
                    Threading::OntoTheAddressedMessage => {
                        answer_target(&blocks, block_id, &text).map(ReplyThread::OntoOrPlainly)
                    }
                };
                let reply = OutboundReply {
                    channel: channel.clone(),
                    text,
                    kind,
                    reply_target,
                    delivery: DeliveryHandle::in_conversation(conversation_id)
                        .quoting(quotable_block),
                };
                if items.send(Outbound::Reply(reply)).is_err() {
                    return Ok(());
                }
            }
            Deliverable::Mark {
                emoji,
                target_origin,
            } => {
                let mark = OutboundMark {
                    channel: channel.clone(),
                    emoji,
                    target_origin,
                };
                if items.send(Outbound::Mark(mark)).is_err() {
                    return Ok(());
                }
            }
            Deliverable::Skipped { undeliverable } => {
                tracing::debug!(
                    conversation_id,
                    block_id,
                    undeliverable,
                    "a targetless block is undeliverable; skipped"
                );
            }
        }
        *cursor = block_id;
    }
    Ok(())
}

/// This conversation's delivery position in the map, seated against what it
/// actually holds.
///
/// A conversation with no entry starts at its inherited boundary, which is
/// zero for a session created fresh. An entry standing ABOVE everything the
/// conversation holds cannot be this conversation's: the store reissues
/// conversation ids after a deletion, and a conversation never loses a block
/// while it lives, so the entry belongs to the id's previous holder and is
/// re-seeded the same way (unit 53, 2026-09-02). Without that, the id's new
/// holder would have its first answers swallowed as history somebody else
/// made.
///
/// The position compared against is the highest block ID, never the last in
/// ledger order: a compacted thread's own opening carries the highest ids and
/// sits at the FRONT of its ledger, so the two readings part company there.
///
/// # Errors
///
/// [`StoreError`] if the boundary read fails or the store's actor has
/// stopped.
async fn seated_cursor<'a>(
    tx: &StoreTx,
    cursors: &'a mut DeliveryCursors,
    conversation_id: i64,
    blocks: &[Block],
) -> Result<&'a mut i64, StoreError> {
    let newest = blocks.iter().map(|block| block.id).max().unwrap_or(0);
    match cursors.entry(conversation_id) {
        Entry::Occupied(entry) => {
            let cursor = entry.into_mut();
            if *cursor > newest {
                tracing::info!(
                    conversation_id,
                    "the conversation id was reissued; the delivery cursor re-seeds"
                );
                *cursor = inherited_boundary(tx, conversation_id).await?;
            }
            Ok(cursor)
        }
        Entry::Vacant(entry) => Ok(entry.insert(inherited_boundary(tx, conversation_id).await?)),
    }
}

/// The newest block of this conversation that another conversation also
/// holds — the position everything this conversation INHERITED sits at or
/// below — or 0 when it inherited nothing (unit 45, 2026-08-30).
///
/// One query instead of a remembered moment: a junction row is what makes a
/// block part of a conversation, and a block two conversations hold is one
/// this conversation was forked with. The framework-table names it joins
/// carry the deliberate coupling decision 0032 records.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
async fn inherited_boundary(tx: &StoreTx, conversation_id: i64) -> Result<i64, StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let newest: Option<i64> = conn.query_row(
            "SELECT MAX(cb.block_id) FROM conversation_blocks cb \
             WHERE cb.conversation_id = ?1 \
             AND EXISTS (\
               SELECT 1 FROM conversation_blocks other \
               WHERE other.block_id = cb.block_id \
               AND other.conversation_id != ?1\
             )",
            [conversation_id],
            |row| row.get(0),
        )?;
        Ok(newest.unwrap_or(0))
    })
    .await
}

/// The lag recovery: re-read this adapter's mapped conversations from stored
/// state. One conversation's failed read is logged and skipped, so it cannot
/// keep the remaining conversations' owed answers from delivering.
///
/// # Errors
///
/// [`StoreError`] if the mapping read fails.
async fn recover_from_lag(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
    disclosure: &Disclosure,
    cursors: &mut DeliveryCursors,
    items: &mpsc::UnboundedSender<Outbound>,
) -> Result<(), StoreError> {
    let tx = ctx.store().tx();
    for record in mapping::all(&tx).await? {
        if record.adapter != adapter {
            continue;
        }
        if let Err(error) = deliver_stored_items(
            ctx,
            adapter,
            disclosure,
            record.conversation_id,
            cursors,
            items,
        )
        .await
        {
            tracing::error!(
                conversation_id = record.conversation_id,
                %error,
                "outbound lag recovery skipped a conversation"
            );
        }
    }
    Ok(())
}

/// Where a deliverable reply threads, as far as one block can state it:
/// the block either names the thread itself or names the rule that
/// resolves one. A rule exists because [`deliverable_of`] reads a single
/// block and holds no ledger, while the answer's target is a fact about
/// the turn around it.
enum Threading {
    /// The thread the block carries — the report's stored target, onto
    /// which the line goes or nowhere.
    Onto(ReplyThread),
    /// Onto the one message that addressed the assistant this turn,
    /// resolved by the caller over the loaded ledger ([`answer_target`]).
    OntoTheAddressedMessage,
}

/// What one undelivered block means to this edge.
enum Deliverable {
    /// A reply to yield: an answer's prose, or a report's stored line,
    /// each with where it threads and which stored block a member
    /// replying to it quotes.
    Reply {
        text: String,
        kind: ReplyKind,
        threading: Threading,
        quotable_block: Option<i64>,
    },
    /// A reaction to place: the stored emoji and the message it goes on
    /// (unit 39, 2026-08-30).
    Mark {
        emoji: String,
        target_origin: String,
    },
    /// A block gone undeliverable — its target origin nulled by the named
    /// person's erasure or by the deletion mirror, or a row the store did
    /// not produce — accounted delivered so the re-reads stop meeting it.
    /// The kind names itself, so one reading serves every target-bearing
    /// kind and the log line still says which one was skipped.
    Skipped { undeliverable: &'static str },
}

/// The delivery reading of one block, `None` for everything this edge does
/// not carry. Decoded through the composed kind's one parse path: the
/// framework ingests a completed stream as a committed text block in the
/// assistant's voice, streaming tails parse to their own kinds, and the
/// report kind carries its stored line and target. The report names its
/// own origin, and names it as the only place its line can go — the line
/// is the moderation bot's command shape, which files nothing as a plain
/// message; the answer names the rule instead of an origin, since the
/// message it answers is a fact about the turn and this reading holds one
/// block.
///
/// Which block a member replying to this send would quote is decided here
/// too, once per kind (unit 38, 2026-08-30): an answer names its own
/// block, whose stored text is the model's own words under any disclosure
/// line the channel read, less nothing — the leaked-prefix cut narrows the
/// wire text only (unit 43) — and a report's
/// line names none: the report block declares no quotable column, and that
/// declaration is not this unit's to reopen.
fn deliverable_of(block: &Block) -> Option<Deliverable> {
    let block_id = block.id;
    match AssistantKind::from_block(block) {
        AssistantKind::Core(FrameworkKind(BlockKind::Text(text)))
            if text.role == Some(Role::Assistant) =>
        {
            Some(Deliverable::Reply {
                text: text.content,
                kind: ReplyKind::Answer,
                threading: Threading::OntoTheAddressedMessage,
                quotable_block: Some(block_id),
            })
        }
        AssistantKind::Report(report) => match (report.line, report.target_origin) {
            (Some(line), Some(target)) => Some(Deliverable::Reply {
                text: line,
                kind: ReplyKind::Report,
                threading: Threading::Onto(ReplyThread::OntoOnly(target)),
                quotable_block: None,
            }),
            _ => Some(Deliverable::Skipped {
                undeliverable: crate::tools::report::REPORT_KIND,
            }),
        },
        // The mark names its own target and nothing else: a reaction
        // threads nowhere, carries no prose to introduce, and names no
        // quotable block — a member replying to a reaction is replying to
        // the message under it, which is theirs.
        AssistantKind::MessageMark(mark) => match (mark.emoji, mark.target_origin) {
            (Some(emoji), Some(target_origin)) => Some(Deliverable::Mark {
                emoji,
                target_origin,
            }),
            _ => Some(Deliverable::Skipped {
                undeliverable: crate::tools::mark::MESSAGE_MARK_KIND,
            }),
        },
        _ => None,
    }
}

/// The origin an answer threads onto, `None` for a plain send (unit 26,
/// 2026-08-24).
///
/// The turn's absorbed messages are walked — the same reading the tool
/// admission and the report's aiming take, over the ledger already loaded
/// — and the target is the stored origin of the one message that
/// literally addressed the assistant. Exactly one yields a target: none
/// means nobody addressed it, which is every helpful-mode answer to
/// ordinary chatter, and several mean the turn answered a crowd, where
/// naming one tells the others they were ignored. Never the newest, never
/// the summoning frontier: the frontier is the line the dispatch woke on,
/// which is routinely a bystander's. An addressed message whose origin an
/// erasure nulled, or one recorded before the origin was stored, yields
/// nothing and the answer goes out plain.
///
/// An answer whose own prose carries a reply-acted command shape threads
/// onto nothing. The moderation bot files a report from a REPLY carrying
/// the report shape and deletes from a REPLY carrying the deletion
/// command, so a threaded answer repeating either — a member asking what
/// the command does, a model slip, an injected line — would become a real
/// command against the message it threaded onto, bypassing every check
/// the real path performs. The shapes come from the one list in
/// [`crate::reply_commands`] (decision 0108, widened 2026-08-27). This is
/// a routing choice and not prose sanitation: nothing is rewritten,
/// stripped, refused or withheld, and the text goes out exactly as
/// written.
fn answer_target(ledger: &[Block], answer_block_id: i64, text: &str) -> Option<String> {
    if let Some(shape) = crate::reply_commands::ACTED_FROM_REPLIES
        .iter()
        .find(|lead| text.contains(*lead))
    {
        tracing::debug!(
            answer_block_id,
            shape,
            "the answer's prose carries a reply-acted command shape; it goes out plain"
        );
        return None;
    }
    let mut addressed = provenance::co_summoners(ledger, answer_block_id)
        .into_iter()
        .filter(|message| message.literal_addressed == Some(true));
    match (addressed.next(), addressed.next()) {
        (Some(only), None) => only.origin,
        _ => None,
    }
}

/// The closing reasoning tag, exactly as a model emits it. Matched as
/// these bytes and no other form: the leak is one literal token, so no
/// case folding and no attribute spelling belong here.
const REASONING_CLOSER: &str = "</think>";

/// The answer as the channel sees it: a leaked reasoning prefix removed,
/// everything else byte-identical (unit 43, rule decided 2026-08-30).
///
/// The rule is a count of closing tags, and nothing else. EXACTLY ONE
/// closer is the leak: that tag and everything before it are dropped, and
/// what follows is the answer. This is the shape the live leak had — a
/// decoder lost the opening tag and the whole trace stood in front of the
/// prose — and the cut is unconditional on it.
///
/// EVERY OTHER COUNT is returned whole. No closer is an ordinary answer.
/// Two or more is a shape nobody has seen leak, so the send delivers it
/// untouched rather than guessing which one was the trace. The rule's
/// accepted cost: an answer legitimately mentioning the tag EXACTLY ONCE
/// loses everything before the mention — the count cannot tell that
/// answer from the leak, and the decided trade cuts it; a shape that
/// surfaces in practice gets its own decision then.
///
/// No opening tag is consulted, because none of the two answers above
/// depends on one: a single closer is cut whether or not something opened
/// it, and a pair of closers stands whether or not they are paired with
/// openers.
fn without_leaked_reasoning(text: &str) -> &str {
    let mut closers = text.match_indices(REASONING_CLOSER);
    match (closers.next(), closers.next()) {
        (Some((at, _)), None) => &text[at + REASONING_CLOSER.len()..],
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{EventBus, ProviderRegistry, Store, ToolRegistry};

    use super::*;

    use crate::message::{ChannelKey, ChannelKind};
    use crate::schema::store_config;

    /// One outbound item as the reply it must be. Every test in this
    /// module drives prose, so a mark arriving here is a routing defect
    /// and says so instead of being quietly unwrapped.
    fn as_reply(item: Outbound) -> OutboundReply {
        match item {
            Outbound::Reply(reply) => reply,
            Outbound::Mark(mark) => panic!("expected a reply on the edge, got a mark: {mark:?}"),
        }
    }

    /// A runtime context with no reactor and nothing registered: the edge
    /// under test is the only task, so every event on the bus is one this
    /// test put there.
    fn quiet_ctx(store: Store) -> RuntimeContext<AssistantKind, CoreEvent> {
        RuntimeContext::new(
            store,
            Arc::new(EventBus::new()),
            Arc::new(ProviderRegistry::new()),
            Arc::new(ToolRegistry::new()),
        )
    }

    /// The lag-recovery path, driven directly: an answer stored after the
    /// seed whose completion signal fell into a dropped-event window is
    /// still delivered, because the lag notice triggers the full re-read.
    /// The answer here is written through the public write surface with no
    /// dispatch anchor, so its summoners are unreadable and the delivery
    /// carries the disclosure line — the documented fold toward the line.
    ///
    /// The runtime is single-threaded, so the edge task cannot run between
    /// the synchronous emits below: the flood provably overflows the
    /// subscriber's backlog before the task reads one event, the earliest
    /// events — the answer's window — are dropped, and the task's first
    /// receive reports the lag.
    #[tokio::test]
    async fn a_lagged_edge_recovers_the_owed_answer_from_stored_state() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let key = ChannelKey {
            adapter: "quiet".into(),
            channel: "dm-lag".into(),
        };
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        mapping::claim(&store.tx(), &key, ChannelKind::Direct, conversation)
            .await
            .expect("the mapping claims");

        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // The answer arrives after the seed, so the edge owes it.
        store
            .insert_final_text_block(
                conversation,
                Role::Assistant,
                "the owed answer".into(),
                None,
            )
            .await
            .expect("the answer stores");

        // Flood past the subscriber backlog so the answer's own window is
        // dropped and the edge's next receive is the lag notice.
        for _ in 0..300 {
            ctx.bus().emit(CoreEvent::UnlatchRequested {
                conversation_id: conversation,
            });
        }

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the lag recovery delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(reply.channel, key);
        assert_eq!(reply.text, disclosure.disclosed("the owed answer"));
    }

    /// One recorded member message with the given literal-addressed fact —
    /// summoned either way, the two mirrored cases' one difference — and
    /// its block id, for anchoring the turn's answer on it.
    async fn summoning_message(store: &Store, conversation: i64, literal: bool) -> i64 {
        store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                crate::kind::CHAT_MESSAGE_KIND,
                crate::kind::ChatMessage::stored_fields(
                    "the unanswerable ask",
                    crate::kind::RecordedSender {
                        principal_id: 7,
                        authority: crate::message::Authority::Member,
                        speaker: None,
                    },
                    crate::kind::RecordedOrigin::default(),
                    None,
                    "2026-08-24T00:00:00Z",
                    crate::kind::Stamp::compose(
                        crate::kind::Summons {
                            summoned: true,
                            literal_addressed: literal,
                        },
                        crate::message::Authority::Member,
                        None,
                        None,
                    ),
                ),
                None,
            )
            .await
            .expect("the message appends")
    }

    /// One finalized assistant answer anchored on the given summons, the
    /// way the framework's dispatch stamps it — the anchor set through the
    /// domain seam, since the anchored destination is the framework's own.
    async fn anchored_answer(store: &Store, conversation: i64, anchor: i64, content: &str) {
        let answer = store
            .insert_final_text_block(conversation, Role::Assistant, content.into(), None)
            .await
            .expect("the answer inserts");
        agent_ledger::store::domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
            conn.execute(
                "UPDATE blocks SET dispatch_anchor = ?2 WHERE id = ?1",
                [answer, anchor],
            )?;
            Ok(())
        })
        .await
        .expect("the anchor writes");
    }

    /// Wake the edge for one conversation, the completed-stream way.
    fn wake(ctx: &RuntimeContext<AssistantKind, CoreEvent>, conversation_id: i64) {
        ctx.bus().emit(CoreEvent::StreamDone {
            conversation_id,
            usage: None,
            stop_reason: None,
            generation: None,
        });
    }

    /// Every stored answer text of one conversation, in ledger order: the
    /// ledger's own record, read back through the same block parse the
    /// edge delivers from.
    async fn stored_answers(store: &Store, conversation: i64) -> Vec<String> {
        store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads")
            .iter()
            .filter(|block| block.block_type == "text")
            .filter_map(|block| match super::deliverable_of(block) {
                Some(Deliverable::Reply { text, .. }) => Some(text),
                _ => None,
            })
            .collect()
    }

    /// One mapped conversation on the quiet adapter, under the given
    /// channel id. Each empty-answer case takes its own, so a wake for one
    /// never meets another's half-written turn: the production finalize
    /// commits an answer with its anchor in one transaction, while the
    /// helpers above write them in two steps.
    async fn mapped_conversation(store: &Store, channel: &str) -> (ChannelKey, i64) {
        let key = ChannelKey {
            adapter: "quiet".into(),
            channel: channel.into(),
        };
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        mapping::claim(&store.tx(), &key, ChannelKind::Direct, conversation)
            .await
            .expect("the mapping claims");
        (key, conversation)
    }

    /// AC2's first-asker half (unit 22), deterministically at the edge: a
    /// turn whose committed answer block is empty — whitespace included,
    /// the trim's boundary — delivers nothing, prepends no disclosure and
    /// introduces nobody. The next spoken answer is the first thing the
    /// channel hears, and it still carries the first-interaction line,
    /// proving the empty answer resolved no disclosure. The stored empty
    /// block stays untouched: the ledger keeps the honest empty record.
    #[tokio::test]
    async fn a_first_askers_empty_answer_delivers_nothing_and_introduces_nobody() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-empty-first").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // The silent turn: an addressed summons whose committed answer is
        // whitespace-only — the framework writes the truly empty block;
        // the whitespace here pins the trimmed boundary on top of it. The
        // spoken turn behind it is written before the one wake, so the
        // edge's single ordered pass meets both fully written turns — the
        // helpers write an answer and its anchor in two steps, so a wake
        // between them would meet a half-written turn.
        let asked = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, asked, "  \n").await;
        let spoken = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, spoken, "the spoken answer").await;
        wake(&ctx, conversation);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the spoken answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("the spoken answer"),
            "the empty answer delivered nothing and introduced nobody: the \
             spoken answer behind it is the introduction"
        );
        assert!(
            items.try_recv().is_err(),
            "exactly one reply: the empty answer never reached the channel"
        );

        // The honest record stands: the empty block keeps its stored text.
        assert_eq!(
            stored_answers(&store, conversation).await,
            vec!["  \n".to_owned(), disclosure.disclosed("the spoken answer")],
            "no delivery rewrote the empty answer's stored block"
        );
    }

    /// AC2's returning-asker half (unit 22): once a person is introduced,
    /// a later empty answer still delivers nothing — no empty send reaches
    /// the channel — and the follow-up spoken answer arrives bare, without
    /// a repeated line. The ordered reply channel is the proof: nothing
    /// sits between the two spoken deliveries.
    #[tokio::test]
    async fn a_returning_askers_empty_answer_sends_nothing() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, conversation) = mapped_conversation(&store, "dm-empty-return").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // The introduction: the asker's first answer carries the line.
        let first = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, first, "the first answer").await;
        wake(&ctx, conversation);
        let introduced = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the first answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(introduced.text, disclosure.disclosed("the first answer"));

        // The silent turn, then a follow-up spoken one — both fully
        // written before the one wake, since the helpers write an answer
        // and its anchor in two steps. The next delivery is the
        // follow-up's bare text, so the empty answer sent nothing.
        let quiet = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, quiet, "").await;
        let followed = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, followed, "the follow-up answer").await;
        wake(&ctx, conversation);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the follow-up delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(
            reply.text, "the follow-up answer",
            "the returning asker's empty answer produced no send, and the \
             follow-up arrives bare — the person was already introduced"
        );
        assert!(
            items.try_recv().is_err(),
            "no empty message sits on the channel"
        );
    }

    /// AC1 and AC4 at the function (unit 43): one closing tag is the leak,
    /// and it takes itself and everything before it — the live shape, a
    /// trace the model opened before closing it, and a trace with nothing
    /// behind it at all.
    #[test]
    fn exactly_one_closer_takes_itself_and_everything_before_it() {
        assert_eq!(
            without_leaked_reasoning("the model muses.</think>Here is the answer."),
            "Here is the answer.",
            "the leak shape delivers the prose behind the tag alone"
        );
        assert_eq!(
            without_leaked_reasoning("<think>the model's own tags</think>the answer"),
            "the answer",
            "one closer cuts even where an opener precedes it: the rule \
             counts closers and reads no opener"
        );
        assert_eq!(
            without_leaked_reasoning("nothing but reasoning</think>"),
            "",
            "a trace with no answer behind it cuts to nothing"
        );
    }

    /// AC2 at the function: every other closer count leaves the text
    /// exactly as the model wrote it — none, two, three, an opener on its
    /// own, and the empty text. Two and more is a shape nobody has seen
    /// leak, so the send delivers it instead of guessing.
    #[test]
    fn any_other_closer_count_passes_through_whole() {
        for text in [
            "the answer with no tags at all",
            "first trace</think>second trace</think>the answer",
            "<think>one</think>two</think>three",
            "one</think>two</think>three</think>four",
            "<think>an opener and no closer",
            "",
        ] {
            assert_eq!(
                without_leaked_reasoning(text),
                text,
                "only a single closer is a leak; every other count stands"
            );
        }
    }

    /// AC1 at the edge: the live leak shape — a trace ending in the closing
    /// tag, the real answer behind it — reaches the channel as the answer
    /// alone, under the first-interaction line. The stored block keeps the
    /// model's full text under that same line: the ledger and the model's
    /// history are what the model wrote, and only the send cut anything.
    #[tokio::test]
    async fn a_leaked_reasoning_prefix_never_reaches_the_channel() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-leaked-trace").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let leaked = "they asked again, keep it short.</think>Haha, no, I am a machine.";
        let asked = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, asked, leaked).await;
        wake(&ctx, conversation);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("Haha, no, I am a machine."),
            "the channel sees the answer alone, with the line in front of it"
        );
        assert_eq!(
            stored_answers(&store, conversation).await,
            vec![disclosure.disclosed(leaked)],
            "the stored block keeps the model's full text under the line"
        );
    }

    /// AC2 at the edge: an answer carrying two closing tags and one
    /// carrying none are delivered byte for byte, the first under the
    /// introduction and the second bare. Two closers is not the leak shape,
    /// and the send delivers what it was given rather than guessing which
    /// tag a trace ended at.
    #[tokio::test]
    async fn a_two_closer_answer_and_a_clean_answer_go_out_byte_identical() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, conversation) = mapped_conversation(&store, "dm-two-closers").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // Both turns are written before the one wake, since the helpers
        // write an answer and its anchor in two steps.
        let tagged = "A block reads <think>like this</think>, and a bare </think> closes nothing.";
        let asked = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, asked, tagged).await;
        let again = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, again, "a clean answer").await;
        wake(&ctx, conversation);

        let first = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the tagged answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(
            first.text,
            disclosure.disclosed(tagged),
            "a two-closer answer goes out exactly as the model wrote it"
        );
        let second = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the clean answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(
            second.text, "a clean answer",
            "a text with no tags is untouched, and the asker is introduced once"
        );
    }

    /// AC3: an answer that is nothing but reasoning takes the empty-answer
    /// path, now reading the text the channel would see — nothing is sent,
    /// the answer is accounted delivered, and no disclosure is resolved, so
    /// the next spoken answer is still the asker's introduction. The stored
    /// block keeps the whole trace, unlined.
    #[tokio::test]
    async fn an_all_reasoning_answer_delivers_nothing_and_introduces_nobody() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, conversation) = mapped_conversation(&store, "dm-all-reasoning").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let all_reasoning = "no rule covers this, stay quiet.</think>  \n";
        let asked = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, asked, all_reasoning).await;
        let spoke = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, spoke, "the spoken answer").await;
        wake(&ctx, conversation);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the spoken answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(
            reply.text,
            disclosure.disclosed("the spoken answer"),
            "the all-reasoning answer introduced nobody: the spoken answer \
             behind it carries the line"
        );
        assert_eq!(
            stored_answers(&store, conversation).await,
            vec![
                all_reasoning.to_owned(),
                disclosure.disclosed("the spoken answer")
            ],
            "the silent answer's stored trace is untouched and unlined"
        );

        // Nothing sat between the two: the reply channel keeps the order it
        // was written in, so the next item to arrive proves what did not.
        // One more spoken turn, and the next reply is its text — had the
        // all-reasoning answer sent anything, that would be arriving here.
        let asked_again = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, asked_again, "the follow-up answer").await;
        wake(&ctx, conversation);
        let next = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the follow-up delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(
            next.text, "the follow-up answer",
            "the follow-up is the very next item on the channel: the \
             all-reasoning answer put nothing there"
        );
    }

    /// A lined block delivers as its line over the CUT prose (unit 43).
    /// The lined state is PLANTED here, not produced by an earlier
    /// delivery: the test pins the edge's reading of a block that already
    /// opens with the line — the shape at-least-once re-delivery produces —
    /// without driving the crash window itself. The cut reads the model's
    /// words under the line, so the trace dies and the introduction does
    /// not die with it, and the block gains no second line.
    #[tokio::test]
    async fn a_relined_answer_keeps_its_introduction_and_loses_the_trace() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-relined-trace").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let lined = disclosure.disclosed("they asked again, keep it short.</think>Haha, no.");
        let asked = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, asked, &lined).await;
        wake(&ctx, conversation);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("Haha, no."),
            "the line the block already carried opens the cut answer"
        );
        assert_eq!(
            stored_answers(&store, conversation).await,
            vec![lined],
            "the stored block keeps its one line over the model's full text"
        );
    }

    /// AC5: the cut is the answer arm's alone. A deterministic reply whose
    /// fixed text happens to carry the tag bytes — the report's stored line
    /// here — arrives whole, because the other arm never calls the cut.
    #[tokio::test]
    async fn a_fixed_reply_carrying_the_tag_bytes_arrives_whole() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-fixed-reply").await;
        let mut items = spawn_edge(
            ctx.clone(),
            "quiet".into(),
            Arc::new(Disclosure::resolve(None, "Probe")),
        )
        .await
        .expect("the edge opens");

        let line = "machinery text</think>and the command shape behind it";
        store
            .append_consumer_block(
                conversation,
                None,
                crate::tools::report::REPORT_KIND,
                crate::tools::report::Report::stored_fields("origin-violator", Some(7), line),
                None,
            )
            .await
            .expect("the report block appends");
        wake(&ctx, conversation);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the report delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(reply.channel, key);
        assert_eq!(reply.kind, ReplyKind::Report);
        assert_eq!(
            reply.text, line,
            "a fixed reply is a person's own text and reaches the channel whole"
        );
    }

    /// A conversation id its previous holder left behind (unit 53): the id's
    /// new holder has its first answer DELIVERED, never swallowed as history
    /// somebody else made.
    ///
    /// The deletion here is the shape both a retention sweep and an erasure
    /// leave — the mapping row, then the conversation, then the blocks
    /// nothing holds any more — which is what puts the reissued id in front
    /// of a cursor standing above every block that id now holds. The
    /// premise is asserted, not assumed: if the store ever stops reissuing,
    /// this case says so instead of passing on a race it no longer runs.
    #[tokio::test]
    async fn a_reissued_conversation_id_delivers_its_new_holders_first_answer() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, first) = mapped_conversation(&store, "dm-reissued").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // Three answers, so the cursor this edge keeps for the id ends up
        // well above the single block its next holder will hold.
        for line in ["one", "two", "three"] {
            store
                .insert_final_text_block(first, Role::Assistant, line.into(), None)
                .await
                .expect("the answer stores");
        }
        wake(&ctx, first);
        for _ in 0..3 {
            as_reply(
                tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                    .await
                    .expect("the first holder's answers deliver before the deadline")
                    .expect("the edge outlives the test"),
            );
        }

        mapping::delete_by_conversation(&store.tx(), first)
            .await
            .expect("the mapping row goes first");
        store
            .delete_conversation(first)
            .await
            .expect("the conversation goes");
        store
            .gc_orphan_blocks()
            .await
            .expect("the blocks nothing holds go");

        let (again, second) = mapped_conversation(&store, "dm-reissued").await;
        assert_eq!(
            second, first,
            "the premise: the store hands the freed id to the next conversation"
        );
        assert_eq!(again, key, "and the same channel maps to it");
        store
            .insert_final_text_block(
                second,
                Role::Assistant,
                "the new holder's first answer".into(),
                None,
            )
            .await
            .expect("the answer stores");
        wake(&ctx, second);

        let reply = as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the new holder's first answer delivers before the deadline")
                .expect("the edge outlives the test"),
        );
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("the new holder's first answer"),
            "the stale cursor re-seeded, so the answer reaches the chat"
        );
    }

    /// Dropping the receiver ends the edge task even though the bus stays
    /// idle — nothing ever wakes the subscription again, so only the closed
    /// send half can end it.
    #[tokio::test]
    async fn a_dropped_receiver_ends_the_edge_on_an_idle_bus() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store);
        let bus = Arc::clone(ctx.bus());

        let items = spawn_edge(
            ctx,
            "quiet".into(),
            Arc::new(Disclosure::resolve(None, "Probe")),
        )
        .await
        .expect("the edge opens");
        drop(items);

        // The bus handle count proves the task's exit: the edge task owns
        // the one clone of the context, and its end releases it.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while Arc::strong_count(&bus) > 1 {
            assert!(
                std::time::Instant::now() < deadline,
                "the edge task must end once its receiver is dropped"
            );
            tokio::task::yield_now().await;
        }
    }
}
