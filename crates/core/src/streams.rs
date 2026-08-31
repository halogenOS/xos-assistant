//! Per-conversation stream observation and the settle protocol every path
//! runs before it changes what a conversation is — an erasure deleting it,
//! a session replacement copying its history away and unmapping it — per
//! decision 0028.
//!
//! The observer keeps the set of conversations whose model stream looks
//! mid-turn, read from the bus events the assembly already consumes. The bus
//! is lossy, so the set is an observation, not a ledger fact, and every way
//! it can be wrong is bounded here:
//!
//! - An event is only ever lost through a lagged subscription, and a lag is
//!   visible to the subscriber. The observer answers a lag by clearing the
//!   whole set: a live stream re-marks itself on its next stream event,
//!   while a close lost inside the lag stays cleared — so a conversation
//!   cannot stay marked open forever on a dropped close.
//! - A live stream unmarked during that gap still shows its streaming tail
//!   in stored state, which [`settle_stream`] reads as its second
//!   source. The residual hole — a stream between its opening and its first
//!   stored tail, with every status event lost to one lag — leaves an
//!   erasure deleting under the stream, which then fails loudly on the
//!   stream's own store error instead of silently retaining anything.
//! - The widest window needs no lag at all: between the actor dispatching a
//!   turn and the provider's first connected event, nothing marks the
//!   conversation — no observation, no stored tail — so
//!   [`settle_stream`] sees no open stream and an erasure in that
//!   window deletes the conversation under a stream about to write. The
//!   failure direction is a loud error on that stream's write, possibly an
//!   orphaned answer block — never silent retention, because the prose is
//!   nulled by principal id before any deletion. Decision 0028 records the
//!   window as accepted.
//! - The store reissues a deleted conversation's id, so every path that
//!   deletes a conversation calls [`StreamObserver::forget`] for it; a dead
//!   entry never shadows the id's next holder.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_ledger::{CoreEvent, EventBus, Store};
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Instant;

use crate::error::CoreError;

/// How long a caller waits for an interrupted stream to settle before it
/// fails loudly, changing nothing. Generous against a slow store, small
/// against a person awaiting their erasure.
const STREAM_SETTLE_BOUND: Duration = Duration::from_secs(5);

/// How often the settle re-read polls the ledger within its bound.
const SETTLE_POLL: Duration = Duration::from_millis(20);

/// The prefix shared by the framework's stored streaming-tail types — plain
/// text, thinking and tool-call tails alike. The framework's own checks
/// match this prefix and its full type list is not exported, so the prefix
/// is matched here the same way; a single full type string would read a
/// thinking or tool-call tail as already drained.
const STREAMING_PREFIX: &str = "streaming";

/// The framework's stored type string for the interrupt's status append —
/// the write whose arrival the settle re-read confirms.
const STATUS_BLOCK: &str = "status";

/// The most conversations the per-turn measurements are held for. Past the
/// cap the map is cleared whole — the established memory-cap shape: losing a
/// measurement costs one delayed compaction, restored by the conversation's
/// next turn, while an unbounded map would grow with every channel this
/// process ever served.
const MEASUREMENT_CAP: usize = 4096;

/// What one conversation's last observed turn measured.
#[derive(Debug, Clone, Copy)]
struct TurnMeasurement {
    /// The tokens that turn occupied of the context window: the request's
    /// input plus the response's output. `None` until a turn reports usage
    /// — a provider that says nothing leaves the last known number
    /// standing rather than overwriting it with a fabricated zero.
    tokens_used: Option<u32>,
    /// When the conversation last dispatched — the moment its prefix was
    /// last put in the provider's prompt cache. Every stream event on the
    /// conversation is evidence of a live dispatch and re-warms it.
    last_dispatch: Instant,
}

/// The conversations whose model stream is currently mid-turn, observed
/// from the bus: a turn opens on its first stream-status event and closes on
/// the turn's done, error or closed signal. The module doc states how the
/// observation can be wrong and what bounds each case.
///
/// Beside the open set it keeps what each conversation's last turn MEASURED
/// (unit 48, 2026-08-31): the tokens it occupied and when it dispatched. The
/// framework persists no usage — it rides `StreamDone` and is dropped unless
/// somebody holds it — and the compaction thresholds are the somebody. Both
/// facts are stream facts, read off the events this observer already
/// consumes, so they live here rather than in a second subscriber that would
/// see the same bus twice.
#[derive(Default)]
pub(crate) struct StreamObserver {
    open: Mutex<HashSet<i64>>,
    measured: Mutex<HashMap<i64, TurnMeasurement>>,
}

impl StreamObserver {
    fn set(&self, conversation_id: i64, open: bool) {
        // A poisoned lock is recoverable here: the set holds plain ids and
        // every holder only inserts, removes or clears.
        let mut streams = self
            .open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if open {
            streams.insert(conversation_id);
        } else {
            streams.remove(&conversation_id);
        }
    }

    fn is_open(&self, conversation_id: i64) -> bool {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&conversation_id)
    }

    /// Drop a deleted conversation's observation and its measurement. The
    /// store reissues conversation ids, so an entry left behind would mark
    /// the id's next holder open before its first stream ever ran, and
    /// would hand it a stranger's token count.
    pub(crate) fn forget(&self, conversation_id: i64) {
        self.set(conversation_id, false);
        self.measurements().remove(&conversation_id);
    }

    /// What this conversation's last reporting turn occupied of the context
    /// window, or `None` when no turn has ever reported usage for it.
    pub(crate) fn tokens_used(&self, conversation_id: i64) -> Option<u32> {
        self.measurements()
            .get(&conversation_id)
            .and_then(|measured| measured.tokens_used)
    }

    /// When this conversation last dispatched, or `None` when it has not
    /// under this process — a prompt cache that was never warmed.
    pub(crate) fn last_dispatch(&self, conversation_id: i64) -> Option<Instant> {
        self.measurements()
            .get(&conversation_id)
            .map(|measured| measured.last_dispatch)
    }

    /// Every conversation a turn has been measured for — the candidate set
    /// a threshold sweep reads. A conversation nothing is known about can
    /// arm nothing, so it is not a candidate.
    pub(crate) fn measured_conversations(&self) -> Vec<i64> {
        self.measurements().keys().copied().collect()
    }

    /// Record that a dispatch just happened on this conversation, carrying
    /// what it reported using where it reported anything.
    ///
    /// A turn whose provider reported NO usage re-warms the cache reading
    /// and leaves the token count exactly as it was: silence is not a
    /// measurement, and overwriting a known number with a fabricated zero
    /// would disarm both threshold arms on a full conversation.
    fn measure(&self, conversation_id: i64, tokens_used: Option<u32>) {
        let mut measured = self.measurements();
        if measured.len() >= MEASUREMENT_CAP && !measured.contains_key(&conversation_id) {
            tracing::debug!("the turn measurement memory reached its cap and was cleared");
            measured.clear();
        }
        let entry = measured.entry(conversation_id).or_insert(TurnMeasurement {
            tokens_used: None,
            last_dispatch: Instant::now(),
        });
        entry.last_dispatch = Instant::now();
        if let Some(tokens) = tokens_used {
            entry.tokens_used = Some(tokens);
        }
    }

    /// A poisoned lock is recoverable here for the reason the open set's
    /// is: the map holds plain numbers and every holder only reads,
    /// inserts, removes or clears.
    fn measurements(&self) -> std::sync::MutexGuard<'_, HashMap<i64, TurnMeasurement>> {
        self.measured
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The lag answer: clear everything. A live stream re-marks itself on
    /// its next stream event; a close lost inside the lag stays cleared.
    fn resync(&self) {
        self.open
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

/// Watch the bus and keep an observer's open-stream set current. The task
/// holds the observer weakly and ends with the assembly or with the bus,
/// whichever goes first.
pub(crate) fn spawn_observer(bus: &Arc<EventBus<CoreEvent>>) -> Arc<StreamObserver> {
    let observer = Arc::new(StreamObserver::default());
    let mut events = bus.subscribe();
    let weak = Arc::downgrade(&observer);
    tokio::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => {
                    let Some(observer) = weak.upgrade() else {
                        break;
                    };
                    observer.resync();
                    continue;
                }
                Err(RecvError::Closed) => break,
            };
            let Some(observer) = weak.upgrade() else {
                break;
            };
            match event {
                CoreEvent::StreamStatus {
                    conversation_id, ..
                } => {
                    observer.set(conversation_id, true);
                    // A live stream is a dispatch that has landed, so the
                    // provider's prompt cache holds this conversation's
                    // prefix from here.
                    observer.measure(conversation_id, None);
                }
                CoreEvent::StreamDone {
                    conversation_id,
                    usage,
                    ..
                } => {
                    observer.set(conversation_id, false);
                    // The one place the framework's usage is caught: it
                    // rides this signal and is dropped by everyone who does
                    // not hold it, and the compaction thresholds hold it.
                    observer.measure(
                        conversation_id,
                        usage.map(|usage| usage.input_tokens.saturating_add(usage.output_tokens)),
                    );
                }
                CoreEvent::StreamError {
                    conversation_id, ..
                }
                | CoreEvent::StreamClosed {
                    conversation_id, ..
                } => {
                    observer.set(conversation_id, false);
                    observer.measure(conversation_id, None);
                }
                _ => {}
            }
        }
    });
    observer
}

/// Settle one conversation's stream before the caller changes what that
/// conversation is or holds — an erasure deleting it, or a session
/// replacement copying its history onto a successor and unmapping it. Both
/// need the same fact: no stream is still writing into it.
///
/// Idle — neither observed
/// open nor holding a stored streaming tail — costs nothing: no interrupt,
/// no wait. Otherwise the interrupt is emitted and the settle is confirmed
/// from stored state within one shared bound: no streaming tail remains and
/// the interrupt's own status block has been appended, so the interrupt's
/// ledger writes cannot race the deletion. The status append is counted, not
/// id-compared: deleting the streaming tail frees the newest ids, so the
/// append can arrive on a reissued id at or below the old tail's.
///
/// An observed-open stream first awaits the stream's end signal — the same
/// terminal set the observer closes on: the turn's done, its error, or the
/// stream's close. An errored turn emits no close at all (the framework ends
/// the turn on the error itself), so a wait keyed on the close alone would
/// burn the whole bound for a stream that already ended. A provider deaf to
/// the interrupt emits none of the three and therefore still fails the
/// settle loudly at the bound; the timed-out observation is dropped as the
/// failure returns, so a retry decides from stored state — the interrupt's
/// teardown has swept the tail by then — instead of failing forever. A
/// stored tail with no observed stream is a leftover from a runtime that is
/// gone (a crash's residue), so no end signal can ever arrive and the
/// stored-state confirmation alone decides; the interrupt is still emitted
/// first, because its handling is what sweeps the tail and latches any
/// stream the observation might have missed.
///
/// # Errors
///
/// [`CoreError::StreamUnsettled`] if the stream did not settle before the
/// bound; [`CoreError::Store`] if a read fails.
pub(crate) async fn settle_stream(
    store: &Store,
    bus: &Arc<EventBus<CoreEvent>>,
    observer: &StreamObserver,
    conversation_id: i64,
) -> Result<(), CoreError> {
    // Subscribed before the observation is read, so an end signal firing
    // between the read and the wait below still reaches the wait.
    let mut events = bus.subscribe();
    let observed_open = observer.is_open(conversation_id);
    if !observed_open && !has_streaming_tail(store, conversation_id).await? {
        return Ok(());
    }
    let deadline = tokio::time::Instant::now() + STREAM_SETTLE_BOUND;
    let statuses_before = count_status_blocks(store, conversation_id).await?;
    bus.emit(CoreEvent::InterruptRequested { conversation_id });
    if observed_open && !await_stream_end(&mut events, conversation_id, deadline).await {
        // The observation is dropped with the failure: a provider deaf to
        // the interrupt emits no end signal ever, so the entry would
        // otherwise stay open forever and every retry would burn the bound
        // on a wait that cannot end. The interrupt above has already torn
        // the stream's binding down, so a retry decides from stored state —
        // and a stream somehow still live re-marks itself on its next
        // stream event, per this observer's own resync rule.
        observer.forget(conversation_id);
        return Err(CoreError::StreamUnsettled { conversation_id });
    }
    confirm_settled(store, conversation_id, statuses_before, deadline).await
}

/// Await one conversation's stream-end signal — done, error or closed, the
/// terminal set the observer keys on — answering whether it arrived before
/// the deadline.
///
/// Two callers wait on a turn ending and they wait the same way (widened to
/// crate visibility 2026-08-31, unit 48): the erasure's settle, which then
/// confirms from stored state, and the compaction's capture, which then
/// reads the answer off the ledger. Both decide from the LEDGER afterwards
/// and use this only to spare that read its polling, so a second wait loop
/// would be a second reading of the same signal set.
///
/// A LAGGED subscriber answers `true`: the end signal may have been dropped
/// inside the lag, and the caller's stored-state read is the check that
/// decides. A closed bus and the deadline both answer `false`.
pub(crate) async fn await_stream_end(
    events: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
    conversation_id: i64,
    deadline: tokio::time::Instant,
) -> bool {
    loop {
        let event = tokio::time::timeout_at(deadline, events.recv()).await;
        match event {
            Ok(Ok(
                CoreEvent::StreamDone {
                    conversation_id: ended,
                    ..
                }
                | CoreEvent::StreamError {
                    conversation_id: ended,
                    ..
                }
                | CoreEvent::StreamClosed {
                    conversation_id: ended,
                    ..
                },
            )) if ended == conversation_id => return true,
            Ok(Ok(_)) => {}
            Ok(Err(RecvError::Lagged(_))) => return true,
            Ok(Err(RecvError::Closed)) | Err(_) => return false,
        }
    }
}

/// Re-read the ledger until the interrupt's writes are in — no streaming
/// tail remains and a status block was appended beyond the count seen before
/// the interrupt — or fail at the deadline.
async fn confirm_settled(
    store: &Store,
    conversation_id: i64,
    statuses_before: usize,
    deadline: tokio::time::Instant,
) -> Result<(), CoreError> {
    loop {
        let blocks = store.list_blocks(conversation_id).await?;
        let streaming_tail = blocks
            .iter()
            .any(|block| block.block_type.starts_with(STREAMING_PREFIX));
        let interrupt_recorded = blocks
            .iter()
            .filter(|block| block.block_type == STATUS_BLOCK)
            .count()
            > statuses_before;
        if !streaming_tail && interrupt_recorded {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(CoreError::StreamUnsettled { conversation_id });
        }
        tokio::time::sleep(SETTLE_POLL).await;
    }
}

/// Whether the conversation's ledger holds an unfinalized streaming tail
/// right now — the stored trace of an in-flight turn.
async fn has_streaming_tail(store: &Store, conversation_id: i64) -> Result<bool, CoreError> {
    Ok(store
        .list_blocks(conversation_id)
        .await?
        .iter()
        .any(|block| block.block_type.starts_with(STREAMING_PREFIX)))
}

/// How many status blocks the conversation's ledger holds.
async fn count_status_blocks(store: &Store, conversation_id: i64) -> Result<usize, CoreError> {
    Ok(store
        .list_blocks(conversation_id)
        .await?
        .iter()
        .filter(|block| block.block_type == STATUS_BLOCK)
        .count())
}

#[cfg(test)]
mod tests {
    use agent_ledger::StreamUsage;

    use super::*;

    /// One completed turn's event, reporting usage or reporting none.
    fn done(conversation_id: i64, usage: Option<(u32, u32)>) -> CoreEvent {
        CoreEvent::StreamDone {
            conversation_id,
            usage: usage.map(|(input_tokens, output_tokens)| StreamUsage {
                input_tokens,
                output_tokens,
                reasoning_tokens: None,
            }),
            stop_reason: None,
            generation: None,
        }
    }

    /// Feed a live observer these events, in order, through the bus it
    /// actually subscribes to — the whole capture path, not the private
    /// recorder behind it.
    fn observing(events: &[CoreEvent]) -> Arc<StreamObserver> {
        let bus = Arc::new(EventBus::<CoreEvent>::new());
        let observer = spawn_observer(&bus);
        for event in events {
            bus.emit(event.clone());
        }
        observer
    }

    /// Wait until the observer's task has caught up to the fact this test is
    /// about, bounded so a regression fails loudly instead of hanging.
    async fn until(observer: &StreamObserver, ready: impl Fn(&StreamObserver) -> bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !ready(observer) {
            assert!(
                tokio::time::Instant::now() < deadline,
                "the observer consumed the events it was given"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// The bridge the compaction thresholds stand on: `StreamDone` carries
    /// the only usage this process ever sees — the framework persists none —
    /// and THIS subscription is the one place it is caught. The tokens the
    /// turn occupied are the request's input plus the response's output, and
    /// a live stream is a dispatch, so the cache reading is warmed too.
    ///
    /// Unpinned, a capture that silently stopped landing would disarm both
    /// threshold arms while the trigger reported itself calm, because "no
    /// usage known" is exactly how the arms are told to stay silent.
    #[tokio::test]
    async fn a_reported_turn_lands_in_the_measurement_the_thresholds_read() {
        let observer = observing(&[done(11, Some((1_000, 200)))]);
        until(&observer, |observer| {
            observer.tokens_used(11) == Some(1_200)
        })
        .await;
        assert!(
            observer.last_dispatch(11).is_some(),
            "a completed turn is a dispatch that landed, so the cache reading is warm"
        );
        assert_eq!(
            observer.measured_conversations(),
            vec![11],
            "a measured conversation is a candidate the threshold sweep reads"
        );
        assert!(
            !observer.is_open(11),
            "the turn's done closes the stream it measured"
        );
    }

    /// A turn whose provider reported NO usage leaves the last known number
    /// standing: silence is not a measurement, and a fabricated zero would
    /// read as an empty context window on a conversation that is nearly
    /// full. The third event is for another conversation and the observer
    /// consumes one queue in order, so seeing it proves the silent turn was
    /// already handled.
    #[tokio::test]
    async fn a_turn_that_reported_no_usage_leaves_the_last_number_standing() {
        let observer = observing(&[
            done(11, Some((1_000, 200))),
            CoreEvent::StreamStatus {
                conversation_id: 11,
                label: agent_ledger::event::stream_status::WAITING_FOR_RESPONSE.into(),
                subtitle: None,
            },
            done(11, None),
            done(12, Some((7, 0))),
        ]);
        until(&observer, |observer| observer.tokens_used(12) == Some(7)).await;
        assert_eq!(
            observer.tokens_used(11),
            Some(1_200),
            "the unreported turn left the last known number standing"
        );
    }

    /// A deleted conversation's measurement goes with it: the store reissues
    /// ids, and the id's next holder must not arm a threshold on a
    /// stranger's token count.
    #[tokio::test]
    async fn forgetting_a_conversation_drops_what_was_measured_for_it() {
        let observer = observing(&[done(11, Some((1_000, 200)))]);
        until(&observer, |observer| {
            observer.tokens_used(11) == Some(1_200)
        })
        .await;
        observer.forget(11);
        assert_eq!(observer.tokens_used(11), None);
        assert_eq!(observer.last_dispatch(11), None);
        assert!(observer.measured_conversations().is_empty());
    }

    /// The wait must end on every signal a turn can end with, because an
    /// errored turn emits its error and nothing after it — a wait keyed on
    /// the close alone would burn the whole bound for a stream that already
    /// ended. The event is queued before the wait starts, so an accepted
    /// signal returns without consuming any of the deadline; the rejection
    /// case uses a deliberately short deadline instead of the real bound.
    async fn ends_the_wait(event: CoreEvent, deadline_in: Duration) -> bool {
        let (sender, mut events) = tokio::sync::broadcast::channel(8);
        sender.send(event).expect("the subscriber is live");
        let deadline = tokio::time::Instant::now() + deadline_in;
        await_stream_end(&mut events, 7, deadline).await
    }

    #[tokio::test]
    async fn every_stream_end_signal_ends_the_wait() {
        assert!(
            ends_the_wait(
                CoreEvent::StreamClosed {
                    conversation_id: 7,
                    generation: None,
                },
                STREAM_SETTLE_BOUND,
            )
            .await
        );
        assert!(
            ends_the_wait(
                CoreEvent::StreamError {
                    conversation_id: 7,
                    error: "the provider failed the turn".into(),
                    generation: None,
                },
                STREAM_SETTLE_BOUND,
            )
            .await
        );
        assert!(
            ends_the_wait(
                CoreEvent::StreamDone {
                    conversation_id: 7,
                    usage: None,
                    stop_reason: None,
                    generation: None,
                },
                STREAM_SETTLE_BOUND,
            )
            .await
        );
    }

    #[tokio::test]
    async fn another_conversations_end_signal_does_not_end_the_wait() {
        assert!(
            !ends_the_wait(
                CoreEvent::StreamClosed {
                    conversation_id: 8,
                    generation: None,
                },
                Duration::from_millis(50),
            )
            .await
        );
    }
}
