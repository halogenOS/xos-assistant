//! The composing edge: a subscription that yields one adapter's composing
//! transitions — the assistant began working on an answer in a channel, it
//! stopped — derived from the turn lifecycle the framework broadcasts.
//!
//! The framework's conversation-state event carries the three facts the
//! signal is made of: `work_due` turns true when the scheduler derives an
//! owed turn — the dispatch's beginning, ahead of any provider traffic —
//! and turns false when the turn's answer commits; a failed turn latches
//! the conversation instead; and `awaiting` names who owes the turn's next
//! move. Composing is `work_due && !latched` with the not-the-model windows
//! carved out: the cue is on while the model is composing — the warranted
//! thinking window (`awaiting == Model`) and the streaming tail, which awaits
//! nobody so `awaiting` is `None` once the provider's first delta lands — and
//! off only where the assistant is not composing: while a tool call is
//! unresolved (`awaiting == System`) — a lookup against an external service is
//! not the assistant composing — and while a human owes a reply or an approval
//! (`User` / `OutOfBand`). So the cue holds through both thinking and
//! streaming, exactly the two phases "typing" should cover, and drops for the
//! tool-execution and human-wait windows. A turn with tool calls therefore
//! yields one begin/stop pair around each tool-execution window — the cue
//! resuming for the model's thinking and streaming after each result — and
//! both ends of a turn — completion and failure — end the signal
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
//! re-marks itself on its next state change. The stop is still delivered
//! at most once end-to-end, and a turn can end without any further state
//! event reaching this edge — the quiet failure above all — so no open
//! signal may depend on a stop arriving: every begin carries a deadline of
//! [`COMPOSING_SIGNAL_LIFETIME`], and a signal still open at its deadline
//! is stopped on the edge's own clock (refined 2026-08-23, after a lost
//! stop left an adapter refreshing an indicator on an idle conversation).
//! The expiry also clears the edge's own entry, so a stale open signal
//! never swallows the next turn's begin; a turn genuinely running past
//! the deadline re-begins on its next state change, with a fresh deadline.
//! A mapping read that
//! fails is logged and that transition dropped: the cue must never disturb
//! answering, so no failure of this edge propagates anywhere.

use std::collections::HashMap;
use std::time::Duration;

use agent_ledger::{Awaiting, CoreEvent, RuntimeContext};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::kind::AssistantKind;
use crate::mapping;
use crate::message::{ChannelKey, ComposingState, ComposingUpdate};

/// The longest one composing signal may hold open. The stop transition is
/// delivered at most once — a lost final state event, an idle conversation
/// after it, and nothing would ever end the signal — so the edge stops any
/// signal still open at this deadline on its own clock. Generous against a
/// real turn on purpose: a turn that genuinely outlasts it loses the cue
/// until its next state change re-begins the signal, while an orphaned
/// signal ends here unconditionally. Public because an adapter's own
/// refresh bound derives from it: the core owns how long the signal may
/// live, the adapter only obeys.
pub const COMPOSING_SIGNAL_LIFETIME: Duration = Duration::from_mins(5);

/// One open composing signal: the channel resolved at the begin, reused
/// for the stop, and the deadline past which the signal stops unasked.
struct OpenSignal {
    channel: ChannelKey,
    expires_at: Instant,
}

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
        let mut open: HashMap<i64, OpenSignal> = HashMap::new();
        loop {
            let deadline = open.values().map(|signal| signal.expires_at).min();
            let event = tokio::select! {
                // A dropped receiver ends the task even while the bus
                // idles; recv alone would park until the next event.
                () = updates.closed() => break,
                // The lifetime bound: a signal still open at its deadline
                // stops on this clock, so a lost stop event ends the
                // signal instead of never — and clearing the entry lets
                // the next genuine begin through.
                () = earliest(deadline) => {
                    let now = Instant::now();
                    let due: Vec<i64> = open
                        .iter()
                        .filter(|(_, signal)| signal.expires_at <= now)
                        .map(|(&conversation_id, _)| conversation_id)
                        .collect();
                    for conversation_id in due {
                        if let Some(signal) = open.remove(&conversation_id) {
                            tracing::warn!(
                                conversation_id,
                                "a composing signal reached its lifetime without a stop; stopped"
                            );
                            let _ = updates.send(ComposingUpdate {
                                channel: signal.channel,
                                state: ComposingState::Stopped,
                            });
                        }
                    }
                    continue;
                }
                event = events.recv() => event,
            };
            match event {
                Ok(CoreEvent::ConversationState {
                    conversation_id,
                    latched,
                    work_due,
                    awaiting,
                }) => {
                    // On while the model owes the turn's next move — the
                    // warranted thinking window (`Model`) and the streaming
                    // tail that awaits nobody (`None`) — and off only for the
                    // windows that are not the model composing: an unresolved
                    // tool call (`System`) and a human owing a reply or an
                    // approval (`User` / `OutOfBand`).
                    let composing = work_due
                        && !latched
                        && !matches!(
                            awaiting,
                            Some(Awaiting::System | Awaiting::User | Awaiting::OutOfBand)
                        );
                    if composing == open.contains_key(&conversation_id) {
                        continue;
                    }
                    if composing {
                        // An unmapped conversation, another adapter's, or a
                        // failed read answers `None`: none of this edge's
                        // business — or logged inside and dropped.
                        if let Some(channel) = channel_of(&ctx, &adapter, conversation_id).await {
                            open.insert(
                                conversation_id,
                                OpenSignal {
                                    channel: channel.clone(),
                                    expires_at: Instant::now() + COMPOSING_SIGNAL_LIFETIME,
                                },
                            );
                            let _ = updates.send(ComposingUpdate {
                                channel,
                                state: ComposingState::Composing,
                            });
                        }
                    } else if let Some(signal) = open.remove(&conversation_id) {
                        let _ = updates.send(ComposingUpdate {
                            channel: signal.channel,
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
                    for (_, signal) in open.drain() {
                        let _ = updates.send(ComposingUpdate {
                            channel: signal.channel,
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

/// Resolves at the earliest lifetime deadline among the open signals, and
/// pends forever when nothing is open — the select's other branches keep
/// the loop responsive either way.
async fn earliest(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
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

    fn state_event(
        conversation_id: i64,
        work_due: bool,
        latched: bool,
        awaiting: Option<Awaiting>,
    ) -> CoreEvent {
        CoreEvent::ConversationState {
            conversation_id,
            latched,
            work_due,
            awaiting,
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

        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        // A second derivation with the same facts: the dedup keeps the
        // repeated state from repeating the begin.
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        ctx.bus()
            .emit(state_event(conversation, false, false, None));

        let begun = updates.recv().await.expect("the edge yields the begin");
        assert_eq!(begun.channel, key);
        assert_eq!(begun.state, ComposingState::Composing);
        let stopped = updates.recv().await.expect("the edge yields the stop");
        assert_eq!(stopped.channel, key);
        assert_eq!(stopped.state, ComposingState::Stopped);
    }

    /// A tool-bearing turn yields the stop-and-resume shape: the cue is on
    /// only while the model owes the turn's next move, so it begins when
    /// the model runs, stops while the tool call is unresolved, resumes on
    /// the tool result, and stops for good at the answer's commit — one
    /// begin/stop pair around the tool-execution window, in order.
    #[tokio::test]
    async fn a_tool_call_stops_the_cue_and_its_result_resumes_it() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-tool").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        // The model runs, a tool call goes out, its result returns the
        // turn to the model, the answer commits.
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::System),
        ));
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        ctx.bus()
            .emit(state_event(conversation, false, false, None));

        for expected in [
            ComposingState::Composing,
            ComposingState::Stopped,
            ComposingState::Composing,
            ComposingState::Stopped,
        ] {
            let update = updates
                .recv()
                .await
                .expect("the edge yields the transition");
            assert_eq!(update.channel, key);
            assert_eq!(update.state, expected);
        }
    }

    /// The streaming tail holds the cue. Once the provider's first delta
    /// lands the frontier awaits nobody, so `awaiting` is `None` while the
    /// answer streams — and the cue must be on, because streaming is the
    /// model composing. This drives the realistic shape that also proves it:
    /// the model thinks (begin), a tool call goes out (stop), and its result
    /// returns straight into the streamed answer with `awaiting == None`,
    /// which must RESUME the cue — a narrower `awaiting == Some(Model)` rule
    /// would leave it dark through the whole stream — before the commit stops
    /// it. The ordered begin/stop/begin/stop proves the streaming state turned
    /// the cue back on.
    #[tokio::test]
    async fn the_streaming_tail_holds_the_cue() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-stream").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        // Think, tool call, then the result streams the answer (awaiting
        // None), then the answer commits.
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::System),
        ));
        ctx.bus().emit(state_event(conversation, true, false, None));
        ctx.bus()
            .emit(state_event(conversation, false, false, None));

        for expected in [
            ComposingState::Composing,
            ComposingState::Stopped,
            ComposingState::Composing,
            ComposingState::Stopped,
        ] {
            let update = updates
                .recv()
                .await
                .expect("the edge yields the transition");
            assert_eq!(update.channel, key);
            assert_eq!(
                update.state, expected,
                "the streaming None state resumes the cue, it does not leave it dark"
            );
        }
    }

    /// A human wait is not composing: an owed, unlatched turn awaiting a
    /// human reply yields no begin, and a human approval owed mid-signal
    /// stops the open one — the cue tracks the model's own activity, never
    /// a wait on a person. The no-begin half is proven by the ordered
    /// channel: the first update is the stop of the other conversation's
    /// open signal, emitted after the human-wait state.
    #[tokio::test]
    async fn a_human_wait_holds_the_cue_off() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (waiting, _) = mapped_conversation(&store, "quiet", "dm-user-wait").await;
        let (open, open_key) = mapped_conversation(&store, "quiet", "dm-approval").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus()
            .emit(state_event(open, true, false, Some(Awaiting::Model)));
        let begun = updates.recv().await.expect("the begin arrives");
        assert_eq!(begun.channel, open_key);
        assert_eq!(begun.state, ComposingState::Composing);

        // A human owes a reply on one conversation and an approval on the
        // other: neither is the model composing.
        ctx.bus()
            .emit(state_event(waiting, true, false, Some(Awaiting::User)));
        ctx.bus()
            .emit(state_event(open, true, false, Some(Awaiting::OutOfBand)));

        let first = updates.recv().await.expect("the stop arrives");
        assert_eq!(
            first.channel, open_key,
            "the human-wait state may not begin a signal ahead of the stop"
        );
        assert_eq!(first.state, ComposingState::Stopped);
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

        ctx.bus()
            .emit(state_event(latched, true, true, Some(Awaiting::Model)));
        ctx.bus()
            .emit(state_event(foreign, true, false, Some(Awaiting::Model)));
        ctx.bus()
            .emit(state_event(marker, true, false, Some(Awaiting::Model)));

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

        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
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

    /// The lifetime bound: after the begin, NO further event reaches the
    /// edge — the exact shape of a stop transition lost without a lag, on
    /// a conversation that then idles — and the stop still arrives, on the
    /// edge's own clock, inside a bounded await. The clock is paused after
    /// the begin, so the five-minute deadline elapses virtually and the
    /// bound is proven, not waited out. A state change after the expiry
    /// then re-begins the signal: a turn genuinely running past the
    /// deadline gets the cue back instead of staying dark.
    #[tokio::test]
    async fn a_lost_stop_expires_on_the_edges_own_clock() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-lost-stop").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        let begun = updates.recv().await.expect("the begin arrives");
        assert_eq!(begun.state, ComposingState::Composing);

        // From here the edge receives nothing: the turn's end is never
        // delivered. Pausing the clock lets the deadline fire virtually;
        // the timeout outlasts it, so a missing expiry fails the test
        // instead of hanging it.
        tokio::time::pause();
        let stopped = tokio::time::timeout(
            COMPOSING_SIGNAL_LIFETIME + std::time::Duration::from_mins(1),
            updates.recv(),
        )
        .await
        .expect("the expiry stop arrives without any event reaching the edge")
        .expect("the edge outlives the test");
        assert_eq!(stopped.channel, key);
        assert_eq!(stopped.state, ComposingState::Stopped);

        // The expiry cleared the edge's entry: the still-running turn's
        // next state change re-begins the signal instead of being
        // swallowed by a stale open entry.
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(Awaiting::Model),
        ));
        let rearmed = updates.recv().await.expect("the re-begin arrives");
        assert_eq!(rearmed.channel, key);
        assert_eq!(rearmed.state, ComposingState::Composing);
    }
}
