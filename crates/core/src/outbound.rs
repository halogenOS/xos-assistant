//! The outbound edge: a subscription that yields one adapter's replies, each
//! bound to the channel key it belongs on.
//!
//! The framework's event subscription is the wake signal only. Events carry
//! no answer text, so on a completed stream the edge re-reads the answer
//! block from the ledger and maps the conversation back to its channel key.
//! Each edge serves exactly one adapter and skips every other adapter's
//! conversations, so two adapters run two edges and neither consumes the
//! other's replies.
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
//! On a stream error the edge yields the failure notice for that turn —
//! marked [`ReplyKind::Notice`], derived from the lossy bus event and
//! therefore at most once. One class of failure yields nothing at all and is
//! only written to the log; [`is_quiet_failure`] names it. The title
//! derivation the metadata worker runs never finalizes an answer block in
//! the conversation ledger, so it never appears here.
//!
//! # A recognized abstention delivers nothing (unit 14, 2026-08-23)
//!
//! The model chooses silence by emitting the abstention sentinel as its
//! whole answer, and this edge is where the choice takes effect: an
//! undelivered answer whose RAW stored text is the recognized sentinel —
//! judged on the trimmed content, before any disclosure resolution — is
//! accounted delivered and yields nothing: no send, no first-interaction
//! introduction, the turn already closed by its own committed answer. The
//! recognition precedes the disclosure prepend on purpose: a prepended
//! line would both un-recognize the sentinel and record an introduction
//! nobody received. An ordinary answer that merely contains the sentinel's
//! words as prose is delivered untouched — the sentinel is the whole
//! answer or nothing.
//!
//! # A recognized miss is routed by the literal addressed fact (unit 16)
//!
//! The model admits an unresolved lookup by emitting the miss sentinel as
//! its whole answer, and this edge routes the outcome — the model cannot
//! see whether a message addressed it, so it never decides this. The
//! recognition runs on the same raw stored text as the abstention's,
//! before any disclosure resolution, and reads the stored
//! literal-addressed fact of the answer's dispatch-anchor message: the
//! summoning frontier the framework stamps on every block a turn writes.
//! An unaddressed anchor — and an unreadable one: no anchor, a non-message
//! anchor, a pre-migration row without the fact — delivers nothing,
//! exactly like an abstention, introducing nobody. An addressed anchor has
//! the fixed [`DONT_KNOW_ANSWER`] written into the stored answer block
//! first, so the ledger carries what the channel sees, and the delivery
//! then flows through the ordinary answer path — the disclosure fold
//! included, so a first asker's "don't know" still opens with the
//! introduction line.
//!
//! # The first delivery introduces the assistant (2026-08-23)
//!
//! An undelivered answer whose summoning people include anyone never yet
//! introduced has the disclosure line written into its stored block before
//! the send — the disclosure module owns the resolution and the receipt,
//! and the resolved [`Disclosure`] value arrives with the edge — so the
//! ledger, the model's history and the channel carry one text. The
//! notice and the report line are fixed texts a person wrote and are never
//! touched.
//!
//! # The report's delivery (2026-08-23)
//!
//! A filed report block delivers as its stored fixed line, marked
//! [`ReplyKind::Report`] and threaded onto the reported message's origin —
//! independent of the answer: an abstained turn's report still goes out,
//! since the abstention recognition below touches only [`ReplyKind::Answer`]
//! blocks — and on BOTH stream events: with the answer on the turn's
//! completion, where ledger order puts it before the answer text, and on
//! the turn's failure ahead of
//! the notice, so a turn that dies after filing still files. The failure
//! wake runs the same stored-state read as a completion, so a dead turn's
//! already-finalized narration delivers beside its report instead of
//! waiting for the conversation's next wake. Noted 2026-08-23: the unit
//! contract's failure clause names report blocks beside the notice; the
//! full read is deliberately wider, because the cursor is one high-water
//! mark per conversation — a failure read narrowed to reports would
//! either pass the committed narration for good or repeat the report on
//! the next wake, and the contract refuses re-delivered reports above
//! all. The accepted losses are recorded plainly: a report undelivered
//! when the process dies is LOST — the restart seeding stands, and
//! re-delivering reports from history would ping every group admin
//! at-least-once; for a moderation nudge the safer direction is fewer
//! reports, never more. A report whose target an erasure nulled is
//! skipped as undeliverable.

use std::collections::HashMap;
use std::sync::Arc;

use agent_ledger::store::domain_run;
use agent_ledger::{
    Block, BlockKind, CoreEvent, FromBlock, Role, RuntimeContext, Store, StoreError,
};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::abstention;
use crate::disclosure::Disclosure;
use crate::kind::{AssistantKind, FrameworkKind};
use crate::mapping;
use crate::message::{OutboundReply, ReplyKind};

/// The one failure notice a failed turn yields, uniform across failure
/// causes: the wire flattens a provider's refusal to prose before the core
/// sees it, so wording distinctions would rest on string-matching text
/// nobody owns. The latch already stops further spending, which is the
/// substance; this line only makes the silence explicit. The next message
/// that addresses the assistant re-engages it.
///
/// The wording stays uniform; what varies is whether the line goes out at
/// all. One failure class yields no notice — `is_quiet_failure` in this
/// module names it — and that classification reads a rendering the framework
/// owns, never a provider's own prose.
pub const FAILURE_NOTICE: &str =
    "I could not finish that answer. Mention me or message me again and I will retry.";

/// How the framework renders a provider's payment-class refusal. A
/// non-success provider response reaches the consumer as
/// `api error {status}: {body}` — the framework's own `Display` for that
/// error — so the status is readable from the event's error text, and this
/// prefix is the entire contract the classification below rests on.
const PAYMENT_REQUIRED_RENDERING: &str = "api error 402:";

/// What the log line calls a suppressed failure. The chat learns nothing, so
/// the log is the only place the cause is recorded.
const PAYMENT_REQUIRED_CLASS: &str = "payment required";

/// Whether a failed turn passes without a word in the chat.
///
/// A payment-class refusal means the provider account has no balance. That
/// condition holds until someone tops the balance up, so every mention in
/// the meantime fails the same way and every one of them would draw its own
/// notice — the chat fills with the same line while nothing about it is
/// actionable by the people reading it. The operator asked for silence
/// there (decided 2026-08-23): the log keeps the record, the chat stays
/// quiet. Every other failure keeps its notice, the latch is unaffected, and
/// the next addressed message re-engages exactly as before.
fn is_quiet_failure(error: &str) -> bool {
    error.starts_with(PAYMENT_REQUIRED_RENDERING)
}

/// The fixed answer an addressed miss delivers (unit 16, 2026-08-24): the
/// model looked, confirmed nothing, and the asker literally addressed the
/// assistant, so silence would read as being ignored. A named constant on
/// purpose — the wording is product behavior, not a model answer, and it
/// carries no trained-knowledge tail: no "as far as I know", no partial
/// answer from memory. It delivers as a first answer like any other,
/// disclosure fold included; an unaddressed miss delivers nothing instead.
pub const DONT_KNOW_ANSWER: &str =
    "I don't know. I looked this up and could not find an answer in the project's sources.";

/// The fixed acknowledgment a rules change draws in the chat — deterministic
/// product behavior, not a model answer, so the wording cannot drift
/// (decided 2026-08-23). Every real delta draws one (the operator decided,
/// 2026-08-23): the on-delta comparison is the whole admission check, and an identical
/// re-pin draws nothing.
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
) -> Result<mpsc::UnboundedReceiver<OutboundReply>, StoreError> {
    let mut events = ctx.bus().subscribe();
    let mut cursors = seed_cursors(&ctx, &adapter).await?;
    let (replies, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let event = tokio::select! {
                // A dropped receiver ends the task even while the bus idles;
                // recv alone would park until the next event arrives.
                () = replies.closed() => break,
                event = events.recv() => event,
            };
            match event {
                Ok(CoreEvent::StreamDone {
                    conversation_id, ..
                }) => {
                    if let Err(error) = deliver_answers_and_reports(
                        &ctx,
                        &adapter,
                        &disclosure,
                        conversation_id,
                        &mut cursors,
                        &replies,
                    )
                    .await
                    {
                        tracing::error!(conversation_id, %error, "outbound delivery failed");
                    }
                }
                // A failed turn tells the chat once — after delivering what
                // the dead turn already put on the ledger, a filed report
                // above all: the failure wake runs the same stored-state
                // read as a completion, so a turn that dies after filing
                // still files, ahead of the notice. The notice itself
                // derives from this event alone and the bus is lossy, so it
                // is at most once by construction: a lagged edge may drop
                // it, a late error from a torn-down predecessor stream may
                // produce a spurious one — both accepted for a courtesy
                // line. The durable record of failed turns is framework
                // work.
                Ok(CoreEvent::StreamError {
                    conversation_id,
                    error,
                    ..
                }) => {
                    if let Err(error) = deliver_answers_and_reports(
                        &ctx,
                        &adapter,
                        &disclosure,
                        conversation_id,
                        &mut cursors,
                        &replies,
                    )
                    .await
                    {
                        tracing::error!(conversation_id, %error, "outbound delivery failed");
                    }
                    if is_quiet_failure(&error) {
                        tracing::info!(
                            conversation_id,
                            class = PAYMENT_REQUIRED_CLASS,
                            "the failed turn stays quiet in the chat"
                        );
                    } else if let Err(error) =
                        deliver_notice(&ctx, &adapter, conversation_id, &replies).await
                    {
                        tracing::error!(conversation_id, %error, "the failure notice did not deliver");
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "outbound edge lagged; re-reading stored state");
                    if let Err(error) =
                        recover_from_lag(&ctx, &adapter, &disclosure, &mut cursors, &replies).await
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
/// already stored when the edge is taken is ever delivered. A conversation
/// mapped after the seed has no row here and starts at zero: all of its
/// blocks postdate the edge.
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

/// Read the conversation's ledger and yield every undelivered answer and
/// report block, in ledger order — which puts a turn's report ahead of its
/// answer, because the tool filed before the answer finalized — bound to
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
async fn deliver_answers_and_reports(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
    disclosure: &Disclosure,
    conversation_id: i64,
    cursors: &mut DeliveryCursors,
    replies: &mpsc::UnboundedSender<OutboundReply>,
) -> Result<(), StoreError> {
    let tx = ctx.store().tx();
    let Some(channel) = mapping::channel_for_conversation(&tx, conversation_id).await? else {
        return Ok(());
    };
    if channel.adapter != adapter {
        return Ok(());
    }
    let mut blocks = ctx.store().list_blocks(conversation_id).await?;
    let cursor = cursors.entry(conversation_id).or_insert(0);
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
                reply_target,
            } => {
                // The abstention recognition comes FIRST, on the raw stored
                // text (unit 14): the model chose silence, so the answer is
                // accounted delivered and yields nothing — and no disclosure
                // resolution runs, since a spoken-to-nobody answer
                // introduces nobody.
                if kind == ReplyKind::Answer && abstention::is_abstention(&text) {
                    tracing::debug!(
                        conversation_id,
                        block_id,
                        "the model abstained; the turn speaks nothing"
                    );
                    *cursor = block_id;
                    continue;
                }
                // The miss recognition runs on the same raw stored text
                // (unit 16), and the ROUTING is the machine's: the stored
                // literal-addressed fact of the answer's dispatch-anchor
                // message decides, never the model. Unaddressed — or
                // unreadable, the silent fold — delivers nothing, like an
                // abstention. Addressed rewrites the stored answer to the
                // fixed don't-know line and falls through the ordinary
                // delivery below, disclosure fold included: from here on
                // the miss IS a first answer like any other, and a re-read
                // after a crash between the rewrite and the send meets an
                // ordinary undelivered answer, never a doubled decision.
                if kind == ReplyKind::Answer && abstention::is_miss(&text) {
                    if !anchor_literally_addressed(&blocks, index) {
                        tracing::debug!(
                            conversation_id,
                            block_id,
                            "an unaddressed turn's lookup missed; the turn speaks nothing"
                        );
                        *cursor = block_id;
                        continue;
                    }
                    store_dont_know(ctx.store(), block_id).await?;
                    blocks[index]
                        .fields
                        .insert("content".into(), json!(DONT_KNOW_ANSWER));
                }
                // An answer's first delivery resolves the first-interaction
                // disclosure (decision 0079): the line is written into the
                // stored block before the send, so the ledger carries what
                // the channel saw. Only the model's answers are introduced;
                // the notice, the report line and every other deterministic
                // reply stays exactly the fixed text a person wrote.
                let text = if kind == ReplyKind::Answer {
                    disclosure
                        .deliverable_answer(ctx.store(), conversation_id, &mut blocks, index)
                        .await?
                } else {
                    text
                };
                let reply = OutboundReply {
                    channel: channel.clone(),
                    text,
                    kind,
                    reply_target,
                };
                if replies.send(reply).is_err() {
                    return Ok(());
                }
            }
            Deliverable::Skipped => {
                tracing::debug!(
                    conversation_id,
                    block_id,
                    "a targetless report is undeliverable; skipped"
                );
            }
        }
        *cursor = block_id;
    }
    Ok(())
}

/// Yield the one failure notice for a failed turn on this adapter's channel.
/// A conversation that is not mapped, or is mapped for another adapter, is
/// none of this edge's business — same as an answer's delivery.
///
/// # Errors
///
/// [`StoreError`] if the mapping read fails.
async fn deliver_notice(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
    conversation_id: i64,
    replies: &mpsc::UnboundedSender<OutboundReply>,
) -> Result<(), StoreError> {
    let tx = ctx.store().tx();
    let Some(channel) = mapping::channel_for_conversation(&tx, conversation_id).await? else {
        return Ok(());
    };
    if channel.adapter != adapter {
        return Ok(());
    }
    let _ = replies.send(OutboundReply {
        channel,
        text: FAILURE_NOTICE.into(),
        kind: ReplyKind::Notice,
        reply_target: None,
    });
    Ok(())
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
    replies: &mpsc::UnboundedSender<OutboundReply>,
) -> Result<(), StoreError> {
    let tx = ctx.store().tx();
    for record in mapping::all(&tx).await? {
        if record.adapter != adapter {
            continue;
        }
        if let Err(error) = deliver_answers_and_reports(
            ctx,
            adapter,
            disclosure,
            record.conversation_id,
            cursors,
            replies,
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

/// Whether the answer's dispatch-anchor message LITERALLY addressed the
/// assistant — the miss routing's one question (unit 16), answered from the
/// loaded ledger alone: the answer block carries the anchor id the
/// framework stamped, and the anchor row carries the stored literal fact
/// the ingestion wrote beside the summons. Everything unreadable answers
/// false, the silent fold: an answer without an anchor (the public write
/// surface), an anchor outside the loaded ledger, a non-message anchor,
/// and a pre-migration anchor row whose column reads NULL — a "don't know"
/// spoken to nobody in particular is noise, while a swallowed line to a
/// literally addressed asker needs a row this unit's own write path never
/// produces.
fn anchor_literally_addressed(blocks: &[Block], index: usize) -> bool {
    let Some(anchor) = blocks[index].dispatch_anchor else {
        return false;
    };
    blocks
        .iter()
        .find(|block| block.id == anchor)
        .is_some_and(|block| {
            matches!(
                AssistantKind::from_block(block),
                AssistantKind::ChatMessage(message) if message.literal_addressed == Some(true)
            )
        })
}

/// Replace a recognized miss answer's stored text with the fixed
/// [`DONT_KNOW_ANSWER`], before its first delivery — the same
/// resolve-into-the-stored-block shape as the disclosure prepend, and the
/// same deliberate `block_text` coupling decision 0079 records: the ledger,
/// the model's later history and the channel carry one text, and the raw
/// sentinel — a machinery token — never leaves the machine.
///
/// # Errors
///
/// [`StoreError`] if the rewrite fails or the store's actor has stopped.
async fn store_dont_know(store: &Store, block_id: i64) -> Result<(), StoreError> {
    domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
        conn.execute(
            "UPDATE block_text SET content = ?2 WHERE block_id = ?1",
            rusqlite::params![block_id, DONT_KNOW_ANSWER],
        )?;
        Ok(())
    })
    .await
}

/// What one undelivered block means to this edge.
enum Deliverable {
    /// A reply to yield: an answer's prose unthreaded, or a report's
    /// stored line threaded onto its target.
    Reply {
        text: String,
        kind: ReplyKind,
        reply_target: Option<String>,
    },
    /// A report gone undeliverable — its target origin nulled by the
    /// reported person's erasure, or a row the store did not produce —
    /// accounted delivered so the re-reads stop meeting it.
    Skipped,
}

/// The delivery reading of one block, `None` for everything this edge does
/// not carry. Decoded through the composed kind's one parse path: the
/// framework ingests a completed stream as a committed text block in the
/// assistant's voice, streaming tails parse to their own kinds, and the
/// report kind carries its stored line and target. The answer stays
/// unthreaded on purpose — decision 0018's judgment stands; only the
/// report's delivery threads.
fn deliverable_of(block: &Block) -> Option<Deliverable> {
    match AssistantKind::from_block(block) {
        AssistantKind::Core(FrameworkKind(BlockKind::Text(text)))
            if text.role == Some(Role::Assistant) =>
        {
            Some(Deliverable::Reply {
                text: text.content,
                kind: ReplyKind::Answer,
                reply_target: None,
            })
        }
        AssistantKind::Report(report) => match (report.line, report.target_origin) {
            (Some(line), Some(target)) => Some(Deliverable::Reply {
                text: line,
                kind: ReplyKind::Report,
                reply_target: Some(target),
            }),
            _ => Some(Deliverable::Skipped),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::providers::LlmError;
    use agent_ledger::{EventBus, ProviderRegistry, Store, ToolRegistry};

    use super::*;

    /// The classification boundary, pinned against the framework's own
    /// rendering on both sides: the payment status is quiet, the neighboring
    /// server error is not, and an empty body keeps the prefix. A widened
    /// predicate or a reworded framework Display attribute fails here
    /// instead of drifting silently.
    #[test]
    fn the_quiet_class_is_the_payment_rendering_and_nothing_wider() {
        let payment = LlmError::Api {
            status: 402,
            message: r#"{"error":{"message":"Insufficient credits"}}"#.into(),
        };
        assert!(
            is_quiet_failure(&payment.to_string()),
            "the framework-rendered payment failure stays quiet"
        );
        let empty_body = LlmError::Api {
            status: 402,
            message: String::new(),
        };
        assert!(
            is_quiet_failure(&empty_body.to_string()),
            "an empty body keeps the rendered prefix and stays quiet"
        );
        let server = LlmError::Api {
            status: 500,
            message: "upstream failed".into(),
        };
        assert!(
            !is_quiet_failure(&server.to_string()),
            "a server failure speaks; only the payment status is quiet"
        );
        assert!(
            !is_quiet_failure("rate limited"),
            "a rate-limit rendering speaks"
        );
        let body_collision = LlmError::Api {
            status: 500,
            message: "upstream billing subsystem returned 402 internally".into(),
        };
        assert!(
            !is_quiet_failure(&body_collision.to_string()),
            "the payment number inside another status's body does not quiet it"
        );
    }
    use crate::message::{ChannelKey, ChannelKind};
    use crate::schema::store_config;

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
        let mut replies = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
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

        let reply = tokio::time::timeout(std::time::Duration::from_secs(10), replies.recv())
            .await
            .expect("the lag recovery delivers before the deadline")
            .expect("the edge outlives the test");
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
                    None,
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

    /// One mapped conversation on the quiet adapter, under the given
    /// channel id. The mirrored miss cases each take their own, so a wake
    /// for one never meets the other's half-written turn: the production
    /// finalize commits an answer with its anchor in one transaction,
    /// while the helpers above write them in two steps.
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

    /// AC3 and AC4 (unit 16), deterministically at the edge over the two
    /// mirrored cases of one miss sentinel: the routing reads the stored
    /// literal-addressed fact of the answer's dispatch anchor, never model
    /// text. The unaddressed miss delivers nothing, prepends no disclosure
    /// and introduces nobody; the addressed miss delivers exactly the fixed
    /// don't-know line — verbatim-pinned here, with no trained-knowledge
    /// tail — carrying the first asker's disclosure line, and the stored
    /// block is rewritten to the delivered text, so the ledger and the
    /// channel agree.
    #[tokio::test]
    async fn a_miss_is_routed_by_the_anchors_stored_literal_addressed_fact() {
        assert_eq!(
            DONT_KNOW_ANSWER,
            "I don't know. I looked this up and could not find an answer \
             in the project's sources.",
            "the fixed don't-know copy is pinned verbatim"
        );

        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (_, silent_side) = mapped_conversation(&store, "dm-miss-silent").await;
        let (spoken_key, spoken_side) = mapped_conversation(&store, "dm-miss-spoken").await;
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut replies = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        // The unaddressed miss: summoned, literal fact false — the
        // helpful-mode shape of the live failure.
        let unaddressed = summoning_message(&store, silent_side, false).await;
        anchored_answer(
            &store,
            silent_side,
            unaddressed,
            &format!("  {}\n", crate::abstention::MISS_SENTINEL),
        )
        .await;
        wake(&ctx, silent_side);

        // The addressed miss over the SAME sentinel: only the stored fact
        // differs, and the edge works the wakes in order, so whatever
        // arrives first proves the split.
        let addressed = summoning_message(&store, spoken_side, true).await;
        anchored_answer(
            &store,
            spoken_side,
            addressed,
            crate::abstention::MISS_SENTINEL,
        )
        .await;
        wake(&ctx, spoken_side);

        let reply = tokio::time::timeout(std::time::Duration::from_secs(10), replies.recv())
            .await
            .expect("the addressed miss delivers before the deadline")
            .expect("the edge outlives the test");
        assert_eq!(
            reply.channel, spoken_key,
            "the first delivery is the addressed side's: the unaddressed \
             miss, worked first, delivered nothing"
        );
        assert_eq!(
            reply.text,
            disclosure.disclosed(DONT_KNOW_ANSWER),
            "the addressed miss delivers the don't-know line, disclosure included"
        );
        assert!(
            replies.try_recv().is_err(),
            "exactly one reply: the unaddressed miss stays silent"
        );

        // The ledger carries what the channel saw: the addressed miss's
        // stored answer was rewritten to the delivered text, while the
        // unaddressed miss keeps its raw sentinel record.
        let stored_answer = |blocks: &[Block]| {
            blocks
                .iter()
                .filter(|block| block.block_type == "text")
                .map(super::deliverable_of)
                .find_map(|deliverable| match deliverable {
                    Some(Deliverable::Reply { text, .. }) => Some(text),
                    _ => None,
                })
                .expect("the side stores one answer")
        };
        let silent_blocks = store
            .list_blocks(silent_side)
            .await
            .expect("the silent ledger reads");
        assert_eq!(
            stored_answer(&silent_blocks),
            format!("  {}\n", crate::abstention::MISS_SENTINEL),
            "the unaddressed miss's stored record is untouched"
        );
        let spoken_blocks = store
            .list_blocks(spoken_side)
            .await
            .expect("the spoken ledger reads");
        assert_eq!(
            stored_answer(&spoken_blocks),
            disclosure.disclosed(DONT_KNOW_ANSWER),
            "the addressed miss's block holds the delivered line"
        );
    }

    /// AC5 (unit 16): the two sentinels stay distinct at the edge — an
    /// ADDRESSED turn whose answer is the abstention sentinel delivers
    /// nothing and is never converted to the don't-know line; the next
    /// spoken answer is the first thing the channel hears.
    #[tokio::test]
    async fn an_addressed_abstention_is_not_converted_to_dont_know() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let key = ChannelKey {
            adapter: "quiet".into(),
            channel: "dm-abstain".into(),
        };
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        mapping::claim(&store.tx(), &key, ChannelKind::Direct, conversation)
            .await
            .expect("the mapping claims");
        let disclosure = Arc::new(Disclosure::resolve(None, "Probe"));
        let mut replies = spawn_edge(ctx.clone(), "quiet".into(), Arc::clone(&disclosure))
            .await
            .expect("the edge opens");

        let addressed = summoning_message(&store, conversation, true).await;
        anchored_answer(
            &store,
            conversation,
            addressed,
            crate::abstention::ABSTENTION_SENTINEL,
        )
        .await;
        wake(&ctx, conversation);

        let spoken = summoning_message(&store, conversation, true).await;
        anchored_answer(&store, conversation, spoken, "the spoken answer").await;
        wake(&ctx, conversation);

        let reply = tokio::time::timeout(std::time::Duration::from_secs(10), replies.recv())
            .await
            .expect("the spoken answer delivers before the deadline")
            .expect("the edge outlives the test");
        assert_eq!(
            reply.text,
            disclosure.disclosed("the spoken answer"),
            "the addressed abstention delivered nothing and introduced \
             nobody; social silence is never turned into a don't-know"
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

        let replies = spawn_edge(
            ctx,
            "quiet".into(),
            Arc::new(Disclosure::resolve(None, "Probe")),
        )
        .await
        .expect("the edge opens");
        drop(replies);

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
