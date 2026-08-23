//! The composing edge: a subscription that yields one adapter's composing
//! transitions — the assistant began working on an answer in a channel, it
//! stopped — derived from the turn lifecycle the framework broadcasts.
//!
//! The framework's conversation-state event carries the two facts the
//! signal is made of: `work_due` turns true when the scheduler derives an
//! owed turn — the dispatch's beginning, ahead of any provider traffic —
//! and turns false when the turn's answer commits; a failed turn latches
//! the conversation instead. Composing is therefore `work_due && !latched`,
//! and both ends of a turn — completion and failure — end the signal
//! through the same derivation, without this edge naming failure at all.
//! A deterministic reply never composes by construction: a command-stamped
//! or unaddressed message opens no debt, so no turn is ever owed for it.
//!
//! # A presence cue, stated honestly
//!
//! The signal is live-only. Nothing is stored, nothing is seeded from
//! history, and nothing is owed across a restart — a presence cue about a
//! stream that died with the process would be a lie. The bus is lossy, so
//! the edge answers a lag the way the stream observer does: every open
//! channel is stopped and the set cleared, and a still-running turn
//! re-marks itself on its next state change. The residual hole — a turn
//! whose every state change fell into one lag window — leaves the platform
//! indicator to its own expiry; the adapter's stop-on-answer rule is a
//! backstop only for a turn that delivers an answer — a turn that ends
//! without one, the quiet failure above all, has this edge's stop
//! transition as its only stop. A mapping read that
//! fails is logged and that transition dropped: the cue must never disturb
//! answering, so no failure of this edge propagates anywhere.

use std::collections::HashMap;

use agent_ledger::{CoreEvent, RuntimeContext};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{ChannelKey, ComposingState, ComposingUpdate};

/// Spawn the composing edge for one adapter and return its receiving end.
/// The task ends when the receiver is dropped — noticed even on an idle bus
/// — or when the bus closes. No seed and no error path: the signal has no
/// history to mark, and every read failure inside is logged and swallowed.
pub(crate) fn spawn_edge(
    ctx: RuntimeContext<AssistantKind, CoreEvent>,
    adapter: String,
) -> mpsc::UnboundedReceiver<ComposingUpdate> {
    let mut events = ctx.bus().subscribe();
    let (updates, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        // The channels currently composing, by conversation: the key is
        // resolved once at the transition to composing and reused for the
        // stop, so a conversation whose mapping row an erasure removed
        // mid-turn still gets its stop.
        let mut open: HashMap<i64, ChannelKey> = HashMap::new();
        loop {
            let event = tokio::select! {
                // A dropped receiver ends the task even while the bus
                // idles; recv alone would park until the next event.
                () = updates.closed() => break,
                event = events.recv() => event,
            };
            match event {
                Ok(CoreEvent::ConversationState {
                    conversation_id,
                    latched,
                    work_due,
                    ..
                }) => {
                    let composing = work_due && !latched;
                    if composing == open.contains_key(&conversation_id) {
                        continue;
                    }
                    if composing {
                        // An unmapped conversation, another adapter's, or a
                        // failed read answers `None`: none of this edge's
                        // business — or logged inside and dropped.
                        if let Some(channel) = channel_of(&ctx, &adapter, conversation_id).await {
                            open.insert(conversation_id, channel.clone());
                            let _ = updates.send(ComposingUpdate {
                                channel,
                                state: ComposingState::Composing,
                            });
                        }
                    } else if let Some(channel) = open.remove(&conversation_id) {
                        let _ = updates.send(ComposingUpdate {
                            channel,
                            state: ComposingState::Stopped,
                        });
                    }
                }
                Ok(_) => {}
                // The lag answer, mirroring the stream observer: stop
                // everything open. A still-running turn re-marks itself on
                // its next state change; a stop lost inside the lag stays
                // stopped — the failure direction a presence cue wants.
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "the composing edge lagged; stopping every signal");
                    for (_, channel) in open.drain() {
                        let _ = updates.send(ComposingUpdate {
                            channel,
                            state: ComposingState::Stopped,
                        });
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    receiver
}

/// The channel a composing conversation signals on, when it is this
/// adapter's: `None` for an unmapped conversation, another adapter's, or a
/// failed read — the failure logged here, because a presence cue's read
/// error must never travel further.
async fn channel_of(
    ctx: &RuntimeContext<AssistantKind, CoreEvent>,
    adapter: &str,
    conversation_id: i64,
) -> Option<ChannelKey> {
    match mapping::channel_for_conversation(&ctx.store().tx(), conversation_id).await {
        Ok(Some(channel)) if channel.adapter == adapter => Some(channel),
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(
                conversation_id,
                %error,
                "the composing signal's mapping read failed; the transition is dropped"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{EventBus, ProviderRegistry, Store, ToolRegistry};

    use super::*;
    use crate::message::ChannelKind;
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

    async fn mapped_conversation(store: &Store, adapter: &str, channel: &str) -> (i64, ChannelKey) {
        let key = ChannelKey {
            adapter: adapter.into(),
            channel: channel.into(),
        };
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        mapping::claim(&store.tx(), &key, ChannelKind::Direct, conversation)
            .await
            .expect("the mapping claims");
        (conversation, key)
    }

    fn state_event(conversation_id: i64, work_due: bool, latched: bool) -> CoreEvent {
        CoreEvent::ConversationState {
            conversation_id,
            latched,
            work_due,
            awaiting: None,
        }
    }

    /// The whole shape in one pass: an owed, unlatched turn begins the
    /// signal, a repeated state change does not repeat it, and the turn's
    /// end stops it — exactly one transition each way.
    #[tokio::test]
    async fn a_turns_lifecycle_yields_one_begin_and_one_stop() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-compose").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(state_event(conversation, true, false));
        // A second derivation with the same facts — the awaiting field can
        // change without the composing facts changing.
        ctx.bus().emit(state_event(conversation, true, false));
        ctx.bus().emit(state_event(conversation, false, false));

        let begun = updates.recv().await.expect("the edge yields the begin");
        assert_eq!(begun.channel, key);
        assert_eq!(begun.state, ComposingState::Composing);
        let stopped = updates.recv().await.expect("the edge yields the stop");
        assert_eq!(stopped.channel, key);
        assert_eq!(stopped.state, ComposingState::Stopped);
    }

    /// A latched conversation is not composing — a failed turn latches, so
    /// this is also how a failure ends the signal — and another adapter's
    /// conversation is none of this edge's business. Both proven by the
    /// ordered channel: the only update received is the marker emitted
    /// after them.
    #[tokio::test]
    async fn latched_and_foreign_conversations_yield_nothing() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (latched, _) = mapped_conversation(&store, "quiet", "dm-latched").await;
        let (foreign, _) = mapped_conversation(&store, "elsewhere", "dm-foreign").await;
        let (marker, marker_key) = mapped_conversation(&store, "quiet", "dm-marker").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(state_event(latched, true, true));
        ctx.bus().emit(state_event(foreign, true, false));
        ctx.bus().emit(state_event(marker, true, false));

        let first = updates.recv().await.expect("the marker's begin arrives");
        assert_eq!(
            first.channel, marker_key,
            "neither the latched nor the foreign conversation may signal first"
        );
    }

    /// The lag answer: every open signal is stopped, so a dropped stop
    /// event cannot leave an adapter refreshing an indicator forever. The
    /// flood works exactly like the outbound edge's lag test: the
    /// single-threaded runtime keeps the edge task parked while the
    /// synchronous emits overflow its backlog.
    #[tokio::test]
    async fn a_lagged_edge_stops_every_open_signal() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-lag").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(state_event(conversation, true, false));
        let begun = updates.recv().await.expect("the begin arrives");
        assert_eq!(begun.state, ComposingState::Composing);

        for _ in 0..300 {
            ctx.bus().emit(CoreEvent::UnlatchRequested {
                conversation_id: conversation,
            });
        }

        let stopped = tokio::time::timeout(std::time::Duration::from_secs(10), updates.recv())
            .await
            .expect("the lag stop arrives before the deadline")
            .expect("the edge outlives the test");
        assert_eq!(stopped.channel, key);
        assert_eq!(stopped.state, ComposingState::Stopped);
    }
}
