//! The outbound edge: a subscription that yields what the assistant puts
//! on one adapter's channels — a message to send, or a reaction to place —
//! each bound to the channel key it belongs on.
//!
//! The framework's event subscription is the wake signal only. Events carry
//! no text, so on a completed stream the edge re-reads the conversation's
//! undelivered blocks from the ledger and maps the conversation back to its
//! channel key. Each edge serves exactly one adapter and skips every other
//! adapter's conversations, so two adapters run two edges and neither
//! consumes the other's items.
//!
//! # The model's text goes nowhere (unit 55, 2026-09-02)
//!
//! This edge no longer classifies an assistant text block as anything. The
//! framework still commits every turn's prose as one, and that prose is the
//! model's private notes: it is stored, it is projected back to the model,
//! and no chat ever sees it. What goes out is an OUTGOING MESSAGE block —
//! one message the model asked for by calling a sending tool — carrying the
//! words, the target the model aimed them at, and the call that stays
//! pending until the adapter reports what the platform did with them.
//!
//! Everything the relay used to carry moved onto that block: the leaked
//! reasoning cut, which still narrows the wire text only; the
//! first-interaction disclosure, still composed into the stored block
//! before the send; the chunking, which was always the adapter's; and the
//! reply-command protection, the same rule at the same edge. What did NOT
//! move is the derived threading — the guess at which absorbed message an
//! answer was for — because the model now says which message it is
//! answering, and the empty-answer skip, because the model now chooses
//! silence by not calling a tool at all.
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
//! A model whose reasoning escapes into its text leaves one closing
//! think-tag standing in it with the whole trace in front. This edge is the
//! one place a stored message becomes a platform's message, so this is
//! where the trace is cut: [`without_leaked_reasoning`] states the rule —
//! exactly one closer is the leak and dies with everything before it, every
//! other count stands byte for byte — the sending arm applies it in one
//! place, and the report's fixed line never meets it.
//!
//! The cut reads the model's own prose, under any disclosure line an
//! earlier delivery already stored ([`Disclosure::prose_of`]), and it runs
//! ahead of the introduction below, so the line is composed back in front
//! of the cut text instead of dying inside it.
//!
//! The stored block keeps the model's words whole — under the disclosure
//! line where a first send wrote one in, which is the one person-written
//! prefix a stored message may carry. The channel sees the same content
//! without the leaked prefix: stored and wire text differ by exactly what
//! the send cut. A text the cut empties is sent as it stands rather than
//! withheld: the model asked for this message by calling a tool, and a
//! withheld send would leave its call waiting on something nobody will ever
//! do.
//!
//! # The first send introduces the assistant (2026-08-23)
//!
//! A message whose summoning people include anyone never yet introduced has
//! the disclosure line written into its stored block before the send — the
//! disclosure module owns the resolution and the receipt, and the resolved
//! [`Disclosure`] value arrives with the edge — and the line opens the text
//! that goes out, which is the stored text with any leaked reasoning prefix
//! cut. The report line is a fixed text a person wrote and is never
//! touched.
//!
//! # A message threads where the model aimed it (unit 55, 2026-09-02)
//!
//! The reply tool's target rides on the outgoing block, so the thread is
//! the model's own statement and this edge simply carries it. One rule
//! still overrides it, unchanged: a text carrying a reply-acted command
//! shape goes out UNTHREADED ([`unless_command_shaped`]), because a
//! threaded copy of a moderation command would become a real command
//! against the message it threaded onto.
//!
//! The thread carries its own recovery, stated here and obeyed by the
//! adapter ([`ReplyThread`]): a message whose threaded send the platform
//! refuses goes out once more plain, because words must never be lost to a
//! courtesy, while a report's line is threaded or not delivered — it is the
//! moderation bot's command shape, which files nothing when it is not a
//! reply and would stand in the group as a bare command line.
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
use agent_ledger::{Block, CoreEvent, FromBlock, RuntimeContext, StoreError};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::disclosure::{Disclosure, Introduction};
use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{
    DeliveryHandle, Outbound, OutboundMark, OutboundReply, ReplyKind, ReplyThread,
};

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
                // A filed send cannot wait for the next stream event (unit
                // 55, 2026-09-02). The call it answers stays PENDING until
                // the delivery is reported, and the report cannot happen
                // until the message goes out — so a send woken only by a
                // stream event would be waiting for a stream event that is
                // waiting for it. The framework's own block-change signal
                // breaks that circle: it fires for every content-table
                // write, so it fires for the tool's append.
                //
                // Narrowed to exactly the kind that closes the circle. One
                // bounded row read per change says whether this is a send,
                // and only a send costs the full stored-state pass; a mark,
                // a report and a receipt keep riding the stream events they
                // always rode, because the turn that files them goes on.
                Ok(CoreEvent::BlocksChanged {
                    conversation_id,
                    block_id,
                }) => {
                    if names_a_send(&ctx, block_id).await
                        && let Err(error) = deliver_stored_items(
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

/// Whether one changed block is a filed send — the narrow question the
/// block-change wake asks before it costs anything (unit 55, 2026-09-02).
///
/// One bounded row read, never a ledger load: this runs for every write to
/// every block table of every conversation, and hydrating a conversation
/// per receipt row would be a different cost class. A block the store
/// cannot answer for — deleted between the change and this read, or a read
/// that failed — is not a send: the failure is logged and the wake is
/// dropped, because a lost wake costs a delivery until the next one and a
/// propagated error would take the whole edge down.
async fn names_a_send(ctx: &RuntimeContext<AssistantKind, CoreEvent>, block_id: i64) -> bool {
    match ctx.store().find_block(block_id).await {
        Ok(block) => {
            block.is_some_and(|block| block.block_type == crate::outgoing::OUTGOING_MESSAGE_KIND)
        }
        Err(error) => {
            tracing::warn!(block_id, %error, "a block-change wake could not be read; dropped");
            false
        }
    }
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
    let newest = blocks.iter().map(|block| block.id).max().unwrap_or(0);
    let cursor = seated_cursor(&tx, cursors, conversation_id, newest).await?;
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
                call_block,
            } => {
                // The send's whole path to the wire, in the order the two
                // steps must run. Only the model's own message takes it:
                // the report line is a fixed text a person wrote and passes
                // through untouched — neither cut nor introduced.
                let text = if kind == ReplyKind::Answer {
                    // The cut comes FIRST (unit 43), and it reads the
                    // model's own prose: a block re-delivered after an
                    // earlier send already carries the disclosure line, and
                    // a cut over that line would drop the introduction
                    // instead of the trace. What is left is what the
                    // channel sees, so the step below works on it.
                    //
                    // Nothing is withheld for being empty. The model chose
                    // silence by not calling a sending tool at all, so a
                    // filed send is a message it asked for; a text the cut
                    // emptied goes to the platform as it stands, and the
                    // platform's own answer settles the model's pending
                    // call — which is the honest outcome and, unlike a
                    // silent skip, one that never leaves a call waiting on
                    // a send nobody will ever make.
                    let spoken = without_leaked_reasoning(disclosure.prose_of(&text));
                    // The first send resolves the first-interaction
                    // disclosure (decision 0079): the line is written into
                    // the stored block before the send, and the resolution
                    // answers with nothing but which of the two openings
                    // this message goes out under. The composition is here,
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
                // The reply-command protection (decision 0108), the one
                // judgment left that needs the text: it runs on what goes
                // out, disclosure line included, because that is the prose
                // the moderation command shape is read from.
                let reply_target = match threading {
                    Threading::Onto(thread) => Some(thread),
                    Threading::OntoNamed(origin) => unless_command_shaped(block_id, &text)
                        .then_some(ReplyThread::OntoOrPlainly(origin)),
                    Threading::Plain => None,
                };
                let reply = OutboundReply {
                    channel: channel.clone(),
                    text,
                    kind,
                    reply_target,
                    delivery: DeliveryHandle::in_conversation(conversation_id)
                        .quoting(quotable_block)
                        .answering(call_block),
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
/// The comparison is STRICT, and equality stays: a cursor standing exactly at
/// the newest block is what every delivered conversation carries between
/// turns, so re-seeding there would send its whole history out again on the
/// next wake that brings no new block. Equality is the one shape ids alone
/// cannot decide, and it is answered in favour of the conversation that is
/// alive.
///
/// # Errors
///
/// [`StoreError`] if the boundary read fails or the store's actor has
/// stopped.
async fn seated_cursor<'a>(
    tx: &StoreTx,
    cursors: &'a mut DeliveryCursors,
    conversation_id: i64,
    newest: i64,
) -> Result<&'a mut i64, StoreError> {
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

/// Where a deliverable reply threads, as the block itself states it. Every
/// arm is now read off one block: the report carries the target its filing
/// resolved, and one of the assistant's own messages carries the target the
/// MODEL named through the reply tool — the derived threading of unit 26,
/// which guessed a target from the turn's absorbed messages, went with the
/// relay it existed to serve (unit 55, 2026-09-02).
enum Threading {
    /// The thread the block carries and cannot do without — the report's
    /// stored target, onto which the line goes or nowhere.
    Onto(ReplyThread),
    /// The origin the model aimed this message at. The send still goes out
    /// plain where the platform refuses the thread, because the words mean
    /// what they mean either way, and it goes out plain where its own text
    /// carries a reply-acted command shape.
    OntoNamed(String),
    /// Onto nothing: a message the model sent without naming a target.
    Plain,
}

/// What one undelivered block means to this edge.
enum Deliverable {
    /// A reply to yield: one of the assistant's own messages, or a
    /// report's stored line, each with where it threads, which stored
    /// block a member replying to it quotes, and which pending call the
    /// delivery report settles.
    Reply {
        text: String,
        kind: ReplyKind,
        threading: Threading,
        quotable_block: Option<i64>,
        call_block: Option<i64>,
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
/// not carry. Decoded through the composed kind's one parse path.
///
/// An assistant TEXT block is deliberately not here (unit 55, 2026-09-02).
/// The framework still commits every turn's prose as one, and it is the
/// model's private notes: nothing about it goes to a chat. What goes out is
/// a filed outgoing block — one message the model asked for through a
/// sending tool — carrying its own words, its own target and the call
/// waiting on it.
///
/// The report names its own origin, and names it as the only place its line
/// can go: the line is the moderation bot's command shape, which files
/// nothing as a plain message. One of the assistant's own messages names
/// the origin the model aimed it at, or none.
///
/// Which block a member replying to this send would quote is decided here
/// too, once per kind (unit 38, 2026-08-30): a message names its own
/// block, whose stored text is the model's own words under any disclosure
/// line the channel read, less nothing — the leaked-prefix cut narrows the
/// wire text only (unit 43) — and a report's line names none: the report
/// block declares no quotable column, and that declaration is not this
/// unit's to reopen.
fn deliverable_of(block: &Block) -> Option<Deliverable> {
    let block_id = block.id;
    match AssistantKind::from_block(block) {
        AssistantKind::OutgoingMessage(outgoing) => {
            match (outgoing.text, outgoing.call_block) {
                (Some(text), Some(call_block)) => Some(Deliverable::Reply {
                    text,
                    kind: ReplyKind::Answer,
                    threading: outgoing
                        .reply_to
                        .map_or(Threading::Plain, Threading::OntoNamed),
                    quotable_block: Some(block_id),
                    call_block: Some(call_block),
                }),
                // A row the store did not produce: its call cannot be
                // named, so nothing could ever settle a send made from it.
                // Accounted delivered so the re-reads stop meeting it.
                _ => Some(Deliverable::Skipped {
                    undeliverable: crate::outgoing::OUTGOING_MESSAGE_KIND,
                }),
            }
        }
        AssistantKind::Report(report) => match (report.line, report.target_origin) {
            (Some(line), Some(target)) => Some(Deliverable::Reply {
                text: line,
                kind: ReplyKind::Report,
                threading: Threading::Onto(ReplyThread::OntoOnly(target)),
                quotable_block: None,
                call_block: None,
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

/// Whether one of the assistant's own messages may keep the thread the
/// model aimed it with — the reply-command protection (decision 0108,
/// widened 2026-08-27), at the same edge and with the same rule it always
/// had.
///
/// A text carrying a reply-acted command shape goes out UNTHREADED. The
/// moderation bot files a report from a REPLY carrying the report shape and
/// deletes from a REPLY carrying the deletion command, so a threaded
/// message repeating either — a member asking what the command does, a
/// model slip, an injected line — would become a real command against the
/// message it threaded onto, bypassing every check the real path performs.
/// The shapes come from the one list in [`crate::reply_commands`].
///
/// This is a routing choice and not prose sanitation: nothing is rewritten,
/// stripped, refused or withheld, and the text goes out exactly as the
/// model wrote it.
fn unless_command_shaped(block_id: i64, text: &str) -> bool {
    let Some(shape) = crate::reply_commands::ACTED_FROM_REPLIES
        .iter()
        .find(|lead| text.contains(*lead))
    else {
        return true;
    };
    tracing::debug!(
        block_id,
        shape,
        "the message carries a reply-acted command shape; it goes out plain"
    );
    false
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

    use agent_ledger::store::ToolCallInsert;
    use agent_ledger::{EventBus, ProviderRegistry, Role, Store, ToolRegistry};

    use super::*;

    use crate::message::{ChannelKey, ChannelKind};
    use crate::outgoing::{OUTGOING_MESSAGE_KIND, OutgoingMessage};
    use crate::schema::store_config;

    /// One outbound item as the reply it must be. Every test in this
    /// module drives words, so a mark arriving here is a routing defect
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

    /// One mapped conversation on the quiet adapter, under the given
    /// channel id.
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

    /// One recorded member message with the given literal-addressed fact —
    /// summoned either way — and its block id, for anchoring a turn on it.
    async fn summoning_message(store: &Store, conversation: i64, literal: bool) -> i64 {
        store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                crate::kind::CHAT_MESSAGE_KIND,
                crate::kind::ChatMessage::stored_fields(
                    "the ask",
                    crate::kind::RecordedSender {
                        principal_id: 7,
                        authority: crate::message::Authority::Member,
                        speaker: None,
                    },
                    crate::kind::RecordedOrigin {
                        origin: Some("member-1"),
                        revises: None,
                    },
                    None,
                    "2026-09-02T00:00:00Z",
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

    /// One filed send, the way a sending tool files one: the model's call
    /// block, anchored on the summoning message where the caller names one,
    /// and the outgoing block answering it. Answers the outgoing block's
    /// id.
    ///
    /// The call block is real rather than a bare number, because every
    /// per-person reading behind a send — the disclosure fold above all —
    /// walks the ledger from THAT block's dispatch anchor.
    async fn filed_send(
        store: &Store,
        conversation: i64,
        anchor: Option<i64>,
        text: &str,
        reply_to: Option<&str>,
    ) -> i64 {
        let call = store
            .insert_tool_call_block(
                conversation,
                Role::Assistant,
                ToolCallInsert {
                    tool_call_id: format!("call-{text:.8}"),
                    name: crate::tools::send::NAME.into(),
                    input: "{}".into(),
                    interactive: false,
                },
                None,
            )
            .await
            .expect("the call block appends");
        if let Some(anchor) = anchor {
            // The anchor the framework's own dispatch stamps on a call, set
            // through the domain seam: the anchored destination is the
            // framework's own and no consumer door offers it.
            agent_ledger::store::domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
                conn.execute(
                    "UPDATE blocks SET dispatch_anchor = ?2 WHERE id = ?1",
                    [call, anchor],
                )?;
                Ok(())
            })
            .await
            .expect("the anchor writes");
        }
        store
            .append_consumer_block(
                conversation,
                None,
                OUTGOING_MESSAGE_KIND,
                OutgoingMessage::stored_fields(text, reply_to, call),
                None,
            )
            .await
            .expect("the outgoing block appends")
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

    /// Every stored outgoing text of one conversation, in ledger order —
    /// the ledger's own record, read back through the same block parse the
    /// edge delivers from.
    async fn stored_sends(store: &Store, conversation: i64) -> Vec<String> {
        store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads")
            .iter()
            .filter_map(|block| match AssistantKind::from_block(block) {
                AssistantKind::OutgoingMessage(outgoing) => outgoing.text,
                _ => None,
            })
            .collect()
    }

    /// The next item on the edge, inside a bounded await.
    async fn next_reply(items: &mut mpsc::UnboundedReceiver<Outbound>) -> OutboundReply {
        as_reply(
            tokio::time::timeout(std::time::Duration::from_secs(10), items.recv())
                .await
                .expect("the edge delivers before the deadline")
                .expect("the edge outlives the test"),
        )
    }

    /// The lag-recovery path, driven directly: a message filed after the
    /// seed whose completion signal fell into a dropped-event window is
    /// still delivered, because the lag notice triggers the full re-read.
    ///
    /// The runtime is single-threaded, so the edge task cannot run between
    /// the synchronous emits below: the flood provably overflows the
    /// subscriber's backlog before the task reads one event, the earliest
    /// events — the send's window — are dropped, and the task's first
    /// receive reports the lag.
    #[tokio::test]
    async fn a_lagged_edge_recovers_the_owed_message_from_stored_state() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-lag").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        filed_send(&store, conversation, None, "the owed message", None).await;

        // Flood past the subscriber backlog so the send's own window is
        // dropped and the edge's next receive is the lag notice.
        for _ in 0..300 {
            ctx.bus().emit(CoreEvent::UnlatchRequested {
                conversation_id: conversation,
            });
        }

        let reply = next_reply(&mut items).await;
        assert_eq!(reply.channel, key);
        assert_eq!(reply.text, disclosure.disclosed("the owed message"));
    }

    /// AC4's silent half: a turn whose written text is long and whose sends
    /// are none puts NOTHING in the chat. The framework's committed
    /// assistant text block is the model's private notes, and this edge no
    /// longer classifies it as anything.
    ///
    /// Proven by the ordered channel: a filed send behind the notes is the
    /// first thing that arrives, so nothing the notes could have produced
    /// sits in front of it.
    #[tokio::test]
    async fn a_turn_of_notes_and_no_send_puts_nothing_in_the_chat() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-notes").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        store
            .insert_final_text_block(
                conversation,
                Role::Assistant,
                "I have thought about this at length and will keep it to myself.".into(),
                None,
            )
            .await
            .expect("the notes store");
        filed_send(&store, conversation, None, "the one sent message", None).await;
        wake(&ctx, conversation);

        let reply = next_reply(&mut items).await;
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("the one sent message"),
            "the notes reached nobody: the filed send is the first thing on the channel"
        );
        assert!(
            items.try_recv().is_err(),
            "exactly one item: the model's text is not a message"
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
    /// own, and the empty text.
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

    /// AC7's cut half (unit 55, continuing unit 43): the live leak shape
    /// filed as a send reaches the channel as the message alone, under the
    /// first-interaction line — and the STORED block keeps the model's full
    /// text under that same line. The wire narrows; the ledger does not.
    #[tokio::test]
    async fn the_cut_narrows_the_wire_text_and_leaves_the_stored_text() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-leaked-trace").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let leaked = "they asked again, keep it short.</think>Haha, no, I am a machine.";
        filed_send(&store, conversation, None, leaked, None).await;
        wake(&ctx, conversation);

        let reply = next_reply(&mut items).await;
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("Haha, no, I am a machine."),
            "the channel sees the message alone, with the line in front of it"
        );
        assert_eq!(
            stored_sends(&store, conversation).await,
            vec![disclosure.disclosed(leaked)],
            "the stored block keeps the model's full text under the line"
        );
    }

    /// AC7's disclosure half: the line is composed into the STORED text,
    /// ONCE. The first send to a never-introduced person carries it in the
    /// ledger and on the wire; the second send of the same turn to the same
    /// person reads the receipt and goes out bare, so nothing stacks a
    /// second line anywhere.
    #[tokio::test]
    async fn the_disclosure_line_is_composed_into_the_stored_text_once() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, conversation) = mapped_conversation(&store, "dm-introduced").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let asked = summoning_message(&store, conversation, true).await;
        filed_send(&store, conversation, Some(asked), "the first message", None).await;
        filed_send(
            &store,
            conversation,
            Some(asked),
            "the second message",
            None,
        )
        .await;
        wake(&ctx, conversation);

        let first = next_reply(&mut items).await;
        assert_eq!(first.text, disclosure.disclosed("the first message"));
        let second = next_reply(&mut items).await;
        assert_eq!(
            second.text, "the second message",
            "the person was introduced by the first message of the same turn"
        );
        assert_eq!(
            stored_sends(&store, conversation).await,
            vec![
                disclosure.disclosed("the first message"),
                "the second message".to_owned()
            ],
            "one line, written into the stored text of the send that carried it"
        );
    }

    /// AC7's threading half: a send whose own text carries a reply-acted
    /// command shape goes out UNTHREADED, though the model named a target —
    /// and the stored text is untouched, because this is a routing choice
    /// and not prose sanitation. A send naming the same target with
    /// ordinary words keeps its thread.
    #[tokio::test]
    async fn a_command_shaped_message_goes_out_unthreaded_and_stored_whole() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, conversation) = mapped_conversation(&store, "dm-command-shape").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let shaped = format!(
            "You report a message like this: {shape}",
            shape = crate::reply_commands::ACTED_FROM_REPLIES[0]
        );
        filed_send(&store, conversation, None, &shaped, Some("member-1")).await;
        filed_send(
            &store,
            conversation,
            None,
            "an ordinary answer",
            Some("member-1"),
        )
        .await;
        wake(&ctx, conversation);

        let guarded = next_reply(&mut items).await;
        assert_eq!(
            guarded.reply_target, None,
            "a command-shaped text threads onto nothing"
        );
        assert_eq!(
            guarded.text,
            disclosure.disclosed(&shaped),
            "the text goes out exactly as written: routing changed, prose did not"
        );
        let threaded = next_reply(&mut items).await;
        assert_eq!(
            threaded.reply_target,
            Some(ReplyThread::OntoOrPlainly("member-1".to_owned())),
            "an ordinary message keeps the thread the model aimed it with"
        );
        assert_eq!(
            stored_sends(&store, conversation).await,
            vec![
                disclosure.disclosed(&shaped),
                disclosure.disclosed("an ordinary answer")
            ],
            "the stored text of both sends is the model's own, under the line \
             each carried"
        );
    }

    /// A text the cut empties goes out AS IT STANDS. The model asked for
    /// this message by calling a tool, so withholding it would leave the
    /// call waiting on a send nobody would ever make; the platform's own
    /// answer settles it instead.
    #[tokio::test]
    async fn a_message_the_cut_empties_still_reaches_the_adapter() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, conversation) = mapped_conversation(&store, "dm-all-reasoning").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let all_reasoning = "no rule covers this, stay quiet.</think>";
        filed_send(&store, conversation, None, all_reasoning, None).await;
        wake(&ctx, conversation);

        let reply = next_reply(&mut items).await;
        assert_eq!(
            reply.text,
            disclosure.disclosed(""),
            "the cut left the line and nothing else, and that is what goes out"
        );
        assert!(
            reply.delivery.call_block().is_some(),
            "the send carries the call its report settles"
        );
    }

    /// A lined block delivers as its line over the CUT prose. The lined
    /// state is PLANTED here, not produced by an earlier delivery: the test
    /// pins the edge's reading of a block that already opens with the line
    /// — the shape at-least-once re-delivery produces — without driving the
    /// crash window itself.
    #[tokio::test]
    async fn a_relined_message_keeps_its_introduction_and_loses_the_trace() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-relined-trace").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let lined = disclosure.disclosed("they asked again, keep it short.</think>Haha, no.");
        filed_send(&store, conversation, None, &lined, None).await;
        wake(&ctx, conversation);

        let reply = next_reply(&mut items).await;
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("Haha, no."),
            "the line the block already carried opens the cut message"
        );
        assert_eq!(
            stored_sends(&store, conversation).await,
            vec![lined],
            "the stored block keeps its one line over the model's full text"
        );
    }

    /// The cut is the sending arm's alone. A deterministic reply whose
    /// fixed text happens to carry the tag bytes — the report's stored line
    /// here — arrives whole, because the other arm never calls the cut, and
    /// it settles no call.
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

        let reply = next_reply(&mut items).await;
        assert_eq!(reply.channel, key);
        assert_eq!(reply.kind, ReplyKind::Report);
        assert_eq!(
            reply.text, line,
            "a fixed reply is a person's own text and reaches the channel whole"
        );
        assert_eq!(
            reply.delivery.call_block(),
            None,
            "nobody is waiting on a report's line"
        );
    }

    /// A conversation id its previous holder left behind (unit 53): the id's
    /// new holder has its first message DELIVERED, never swallowed as
    /// history somebody else made.
    ///
    /// The deletion here is the shape both a retention sweep and an erasure
    /// leave — the mapping row, then the conversation, then the blocks
    /// nothing holds any more. The premise is asserted, not assumed: if the
    /// store ever stops reissuing, this case says so instead of passing on
    /// a race it no longer runs.
    #[tokio::test]
    async fn a_reissued_conversation_id_delivers_its_new_holders_first_message() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, first) = mapped_conversation(&store, "dm-reissued").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut items = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // Three messages, so the cursor this edge keeps for the id ends up
        // well above the single block its next holder will hold.
        for line in ["one", "two", "three"] {
            filed_send(&store, first, None, line, None).await;
        }
        wake(&ctx, first);
        for _ in 0..3 {
            next_reply(&mut items).await;
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
        filed_send(&store, second, None, "the new holder's first message", None).await;
        wake(&ctx, second);

        let reply = next_reply(&mut items).await;
        assert_eq!(reply.channel, key);
        assert_eq!(
            reply.text,
            disclosure.disclosed("the new holder's first message"),
            "the stale cursor re-seeded, so the message reaches the chat"
        );
    }

    /// A second delivery pass over a conversation whose blocks all went out
    /// already sends nothing again — and that is why the re-seed reads a
    /// cursor standing STRICTLY above the newest block, never one standing
    /// at it.
    ///
    /// A cursor equal to the newest block id is the steady state of every
    /// delivered conversation, not a reissued id: the last block delivered
    /// IS the newest one. Re-seeding there would drop the cursor to the
    /// inherited boundary — zero for a conversation forked from nothing —
    /// and send the whole conversation to the chat again on the next pass
    /// that meets no new block, which is what a failed turn's wake and
    /// every lag recovery bring.
    ///
    /// The pass is driven directly, so the equality is the case's premise
    /// and not a scheduling accident.
    #[tokio::test]
    async fn a_second_pass_over_a_delivered_conversation_sends_nothing_again() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (key, conversation) = mapped_conversation(&store, "dm-rewoken").await;
        let disclosure = Disclosure::resolve(None, "Probe");
        let (items, mut sent) = mpsc::unbounded_channel();
        let mut cursors = DeliveryCursors::new();
        cursors.insert(conversation, 0);

        filed_send(&store, conversation, None, "the one message", None).await;
        deliver_stored_items(
            &ctx,
            "quiet",
            &disclosure,
            conversation,
            &mut cursors,
            &items,
        )
        .await
        .expect("the first pass reads stored state");
        let reply = as_reply(sent.try_recv().expect("the message goes out"));
        assert_eq!(reply.channel, key);
        assert_eq!(reply.text, disclosure.disclosed("the one message"));

        let newest = store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads")
            .iter()
            .map(|block| block.id)
            .max()
            .expect("the conversation holds blocks");
        assert_eq!(
            cursors[&conversation], newest,
            "the premise: the delivered conversation's cursor stands exactly at its newest block"
        );

        deliver_stored_items(
            &ctx,
            "quiet",
            &disclosure,
            conversation,
            &mut cursors,
            &items,
        )
        .await
        .expect("the second pass reads stored state");

        assert!(
            sent.try_recv().is_err(),
            "the conversation is delivered whole, so a second pass over it sends nothing"
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
