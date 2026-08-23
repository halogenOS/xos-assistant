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

use std::collections::HashMap;

use agent_ledger::{Block, BlockKind, CoreEvent, FromBlock, Role, RuntimeContext, StoreError};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::kind::AssistantKind;
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

/// The fixed acknowledgment a rules change draws in the chat — deterministic
/// product behavior, not a model answer, so the wording cannot drift
/// (decided 2026-08-23). Delivered at most once per channel per
/// acknowledgment window; a further delta inside the window appends its
/// note silently.
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
                    if let Err(error) =
                        deliver_new_answers(&ctx, &adapter, conversation_id, &mut cursors, &replies)
                            .await
                    {
                        tracing::error!(conversation_id, %error, "outbound delivery failed");
                    }
                }
                // A failed turn tells the chat once. The notice derives from
                // this event alone and the bus is lossy, so it is at most
                // once by construction: a lagged edge may drop it, a late
                // error from a torn-down predecessor stream may produce a
                // spurious one — both accepted for a courtesy line. The
                // durable record of failed turns is framework work.
                Ok(CoreEvent::StreamError {
                    conversation_id,
                    error,
                    ..
                }) => {
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
                        recover_from_lag(&ctx, &adapter, &mut cursors, &replies).await
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

/// Read the conversation's ledger and yield every answer block this edge has
/// not delivered yet, bound to the conversation's channel key. A
/// conversation that is not mapped, or is mapped for another adapter, is
/// none of this edge's business and yields nothing.
///
/// # Errors
///
/// [`StoreError`] if a read fails or the store's actor has stopped. The send
/// half never errors here: a dropped receiver ends the task at the loop
/// instead.
async fn deliver_new_answers(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
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
    let blocks = ctx.store().list_blocks(conversation_id).await?;
    let cursor = cursors.entry(conversation_id).or_insert(0);
    for block in &blocks {
        if block.id > *cursor
            && let Some(text) = answer_text(block)
        {
            let reply = OutboundReply {
                channel: channel.clone(),
                text,
                kind: ReplyKind::Answer,
            };
            if replies.send(reply).is_err() {
                return Ok(());
            }
            *cursor = block.id;
        }
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
    cursors: &mut DeliveryCursors,
    replies: &mpsc::UnboundedSender<OutboundReply>,
) -> Result<(), StoreError> {
    let tx = ctx.store().tx();
    for record in mapping::all(&tx).await? {
        if record.adapter != adapter {
            continue;
        }
        if let Err(error) =
            deliver_new_answers(ctx, adapter, record.conversation_id, cursors, replies).await
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

/// The prose of a finalized answer, `None` for every other block. Decoded
/// through the composed kind's one parse path: the framework ingests a
/// completed stream as a committed text block in the assistant's voice, and
/// streaming tails parse to their own kinds, so they never match here.
fn answer_text(block: &Block) -> Option<String> {
    match AssistantKind::from_block(block) {
        AssistantKind::Core(BlockKind::Text(text)) if text.role == Some(Role::Assistant) => {
            Some(text.content)
        }
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

        let mut replies = spawn_edge(ctx.clone(), "quiet".into())
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
        assert_eq!(reply.text, "the owed answer");
    }

    /// Dropping the receiver ends the edge task even though the bus stays
    /// idle — nothing ever wakes the subscription again, so only the closed
    /// send half can end it.
    #[tokio::test]
    async fn a_dropped_receiver_ends_the_edge_on_an_idle_bus() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store);
        let bus = Arc::clone(ctx.bus());

        let replies = spawn_edge(ctx, "quiet".into())
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
