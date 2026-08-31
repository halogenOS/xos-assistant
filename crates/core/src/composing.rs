//! The composing edge: a subscription that yields one adapter's composing
//! transitions — the assistant began writing an answer in a channel, it
//! stopped — derived from the turn lifecycle the framework broadcasts.
//!
//! The begin is the framework's `responding` stream status (unit 22,
//! 2026-08-24): raised once per stream at the first non-empty text delta
//! — the moment real user-visible text starts flowing — and never during
//! thinking, never at a text block's open, never for a stream that
//! finalizes empty. So the cue lights exactly when a reply is actually
//! coming: a turn that says nothing raises no cue at all, and the
//! pre-text thinking window stays dark. The stop is the stream's terminal
//! — its done, its error, or its close, the same terminal set the stream
//! observer keys on — so the completion that committed the streamed text
//! and the failure that killed it clear the signal through one reading.
//! Two derivations were rejected with the unit: the begin from the
//! conversation state, because that state cannot tell thinking from
//! flowing text — the one distinction the cue exists for — and the stop
//! from the conversation state's `work_due`, because the scheduler drops
//! `work_due` the moment the owed turn is dispatched, which is while the
//! text still streams; a stop keyed on it would end the cue the instant
//! it lit. A turn whose text flows in several streams around its tool
//! calls raises one begin/stop pair per text-bearing stream: the cue is
//! on exactly while a reply's text is on the wire.
//! A deterministic reply never composes by construction: a command-stamped
//! or unaddressed message opens no debt, so no turn is ever owed for it.
//!
//! # A presence cue, stated honestly
//!
//! The signal is live-only. Nothing is stored, nothing is seeded from
//! history, and nothing is owed across a restart — a presence cue about a
//! stream that died with the process would be a lie. The bus is lossy, so
//! the edge answers a lag the way the stream observer does: every open
//! channel is stopped and the set cleared; a stop lost inside the lag
//! stays stopped, the failure direction a presence cue wants. The stop is
//! still delivered at most once end-to-end, and a turn can end without
//! any further state event reaching this edge, so no open signal may depend on a stop arriving: every begin
//! carries a deadline of [`COMPOSING_SIGNAL_LIFETIME`], and a signal
//! still open at its deadline is stopped on the edge's own clock (refined
//! 2026-08-23, after a lost stop left an adapter refreshing an indicator
//! on an idle conversation). The expiry also clears the edge's own entry,
//! so a stale open signal never swallows the next turn's begin.
//! The signal stays keyed per conversation, which suffices: a
//! conversation's turns run serially, so one open signal per conversation
//! is one open signal per turn, and the deadline bounds any missed clear.
//! A mapping read that
//! fails is logged and that transition dropped: the cue must never disturb
//! answering, so no failure of this edge propagates anywhere.

use std::collections::HashMap;
use std::time::Duration;

use agent_ledger::event::stream_status;
use agent_ledger::{CoreEvent, RuntimeContext};
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
/// for its remainder — the `responding` begin is once per turn — while an
/// orphaned signal ends here unconditionally, and the cleared entry lets
/// the next turn's begin through. Public because an adapter's own
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
                // The begin: real user-visible text started flowing — the
                // framework raises this once per stream at its first
                // non-empty text delta, never for thinking and never for a
                // stream that finalizes empty, so a turn that says nothing
                // lights no cue. The dedup keeps a repeated status (a
                // re-begin after the expiry aside) from repeating the
                // transition.
                Ok(CoreEvent::StreamStatus {
                    conversation_id,
                    label,
                    ..
                }) if label == stream_status::RESPONDING => {
                    if open.contains_key(&conversation_id) {
                        continue;
                    }
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
                }
                // The stop: the stream's terminal — the completion whose
                // finalize committed the streamed text, the error that
                // killed it, or the close — the same terminal set the
                // stream observer keys on. A conversation with no open
                // signal has nothing to stop, and every non-terminal event
                // between the begin and the terminal leaves the signal
                // alone.
                Ok(
                    CoreEvent::StreamDone {
                        conversation_id, ..
                    }
                    | CoreEvent::StreamError {
                        conversation_id, ..
                    }
                    | CoreEvent::StreamClosed {
                        conversation_id, ..
                    },
                ) => {
                    if let Some(signal) = open.remove(&conversation_id) {
                        let _ = updates.send(ComposingUpdate {
                            channel: signal.channel,
                            state: ComposingState::Stopped,
                        });
                    }
                }
                Ok(_) => {}
                // The lag answer, mirroring the stream observer: stop
                // everything open. A still-running turn loses its cue for
                // the remainder — the begin is once per turn — and a stop
                // lost inside the lag stays stopped: both are the failure
                // direction a presence cue wants.
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
        awaiting: Option<agent_ledger::Awaiting>,
    ) -> CoreEvent {
        CoreEvent::ConversationState {
            conversation_id,
            latched,
            work_due,
            awaiting,
        }
    }

    /// The framework's `responding` stream status for one conversation —
    /// the begin the edge keys on.
    fn responding_event(conversation_id: i64) -> CoreEvent {
        status_event(conversation_id, stream_status::RESPONDING)
    }

    fn status_event(conversation_id: i64, label: &str) -> CoreEvent {
        CoreEvent::StreamStatus {
            conversation_id,
            label: label.into(),
            subtitle: None,
        }
    }

    /// The stream's completed terminal for one conversation — the stop the
    /// edge keys on, beside the error and the close.
    fn done_event(conversation_id: i64) -> CoreEvent {
        CoreEvent::StreamDone {
            conversation_id,
            usage: None,
            stop_reason: None,
            generation: None,
        }
    }

    /// The whole shape in one pass (AC7): real text begins the signal, a
    /// repeated status does not repeat it, and the stream's terminal stops
    /// it — exactly one transition each way.
    #[tokio::test]
    async fn real_text_begins_the_cue_and_the_streams_terminal_stops_it() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-compose").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(responding_event(conversation));
        // A repeated status: the dedup keeps it from repeating the begin.
        ctx.bus().emit(responding_event(conversation));
        ctx.bus().emit(done_event(conversation));

        let begun = updates.recv().await.expect("the edge yields the begin");
        assert_eq!(begun.channel, key);
        assert_eq!(begun.state, ComposingState::Composing);
        let stopped = updates.recv().await.expect("the edge yields the stop");
        assert_eq!(stopped.channel, key);
        assert_eq!(stopped.state, ComposingState::Stopped);
        assert!(
            updates.try_recv().is_err(),
            "one begin, one stop, nothing else"
        );
    }

    /// The dark windows (AC7): the pre-text turn states — the owed turn,
    /// the thinking window, the tool window — and every non-`responding`
    /// stream status raise no cue, and a whole turn that says nothing (no
    /// `responding` ever fires) yields no transition at all. Proven by the
    /// ordered channel: the first update is the marker conversation's
    /// begin, emitted after all of them.
    #[tokio::test]
    async fn pre_text_windows_and_a_turn_that_says_nothing_raise_no_cue() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (silent, _) = mapped_conversation(&store, "quiet", "dm-silent").await;
        let (marker, marker_key) = mapped_conversation(&store, "quiet", "dm-marker").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        // The silent turn's whole lifecycle: owed and thinking, the tool
        // window, the pre-text statuses, then the end — real text never
        // flowed, so no `responding` ever fires and no cue may light.
        ctx.bus().emit(state_event(
            silent,
            true,
            false,
            Some(agent_ledger::Awaiting::Model),
        ));
        ctx.bus().emit(status_event(silent, stream_status::SENDING));
        ctx.bus()
            .emit(status_event(silent, stream_status::WAITING_FOR_RESPONSE));
        ctx.bus().emit(state_event(
            silent,
            true,
            false,
            Some(agent_ledger::Awaiting::System),
        ));
        ctx.bus()
            .emit(status_event(silent, stream_status::RUNNING_TOOLS));
        ctx.bus().emit(status_event(silent, ""));
        ctx.bus().emit(state_event(silent, false, false, None));
        ctx.bus().emit(done_event(silent));

        ctx.bus().emit(responding_event(marker));

        let first = updates.recv().await.expect("the marker's begin arrives");
        assert_eq!(
            first.channel, marker_key,
            "no pre-text window and no silent turn may signal ahead of the marker"
        );
        assert_eq!(first.state, ComposingState::Composing);
    }

    /// Conversation-state changes never touch the open cue: the scheduler
    /// drops `work_due` the moment the owed turn is dispatched — while the
    /// text still streams — so a stop keyed on the state would end the cue
    /// the instant it lit. The edge ignores the state entirely; only the
    /// stream's terminal stops the signal. Proven by the ordered channel:
    /// the marker's begin, emitted after the mid-stream states — the
    /// dispatched `work_due == false` shape included — arrives ahead of
    /// the main conversation's stop.
    #[tokio::test]
    async fn conversation_state_changes_leave_the_open_cue_alone() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-mid-turn").await;
        let (marker, marker_key) = mapped_conversation(&store, "quiet", "dm-mid-marker").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(responding_event(conversation));
        // The dispatched turn's real mid-stream state — `work_due` already
        // false while the text flows — and a tool window behind it: the
        // stream is still open, so the cue holds.
        ctx.bus()
            .emit(state_event(conversation, false, false, None));
        ctx.bus().emit(state_event(
            conversation,
            true,
            false,
            Some(agent_ledger::Awaiting::System),
        ));
        ctx.bus().emit(responding_event(marker));
        ctx.bus().emit(done_event(conversation));

        let begun = updates.recv().await.expect("the begin arrives");
        assert_eq!(begun.channel, key);
        assert_eq!(begun.state, ComposingState::Composing);
        let second = updates.recv().await.expect("the second update arrives");
        assert_eq!(
            second.channel, marker_key,
            "the mid-stream states stopped nothing: the marker's begin \
             arrives ahead of the main conversation's stop"
        );
        assert_eq!(second.state, ComposingState::Composing);
        let stopped = updates.recv().await.expect("the stop arrives");
        assert_eq!(stopped.channel, key);
        assert_eq!(stopped.state, ComposingState::Stopped);
    }

    /// A failed stream stops the open cue through its error terminal — the
    /// failure ending clears the signal through the same reading as the
    /// completion. A foreign adapter's `responding` is none of this edge's
    /// business: the marker proves it produced nothing.
    #[tokio::test]
    async fn a_stream_error_stops_the_cue_and_foreign_conversations_yield_nothing() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (failing, failing_key) = mapped_conversation(&store, "quiet", "dm-failing").await;
        let (foreign, _) = mapped_conversation(&store, "elsewhere", "dm-foreign").await;
        let (marker, marker_key) = mapped_conversation(&store, "quiet", "dm-error-marker").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(responding_event(failing));
        ctx.bus().emit(responding_event(foreign));
        // The failure: the turn dies after its first text.
        ctx.bus().emit(CoreEvent::StreamError {
            conversation_id: failing,
            error: "the scripted stream failure".into(),
            generation: None,
        });
        ctx.bus().emit(responding_event(marker));

        let begun = updates.recv().await.expect("the begin arrives");
        assert_eq!(begun.channel, failing_key);
        assert_eq!(begun.state, ComposingState::Composing);
        let stopped = updates.recv().await.expect("the stop arrives");
        assert_eq!(
            stopped.channel, failing_key,
            "the foreign conversation may not signal between the pair"
        );
        assert_eq!(stopped.state, ComposingState::Stopped);
        let third = updates.recv().await.expect("the marker's begin arrives");
        assert_eq!(third.channel, marker_key);
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

        ctx.bus().emit(responding_event(conversation));
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
    /// bound is proven, not waited out. The next turn's `responding` then
    /// re-begins the signal: the expiry cleared the edge's entry instead
    /// of swallowing the begin.
    #[tokio::test]
    async fn a_lost_stop_expires_on_the_edges_own_clock() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let ctx = quiet_ctx(store.clone());
        let (conversation, key) = mapped_conversation(&store, "quiet", "dm-lost-stop").await;
        let mut updates = spawn_edge(ctx.clone(), "quiet".into());

        ctx.bus().emit(responding_event(conversation));
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

        // The expiry cleared the edge's entry: the next turn's real text
        // re-begins the signal instead of being swallowed by a stale open
        // entry.
        ctx.bus().emit(responding_event(conversation));
        let rearmed = updates.recv().await.expect("the re-begin arrives");
        assert_eq!(rearmed.channel, key);
        assert_eq!(rearmed.state, ComposingState::Composing);
    }
}
