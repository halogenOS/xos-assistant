//! Per-conversation stream observation and the settle protocol erasure runs
//! before it deletes a conversation, per decision 0028.
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
//!   in stored state, which [`settle_for_deletion`] reads as its second
//!   source. The residual hole — a stream between its opening and its first
//!   stored tail, with every status event lost to one lag — leaves an
//!   erasure deleting under the stream, which then fails loudly on the
//!   stream's own store error instead of silently retaining anything.
//! - The widest window needs no lag at all: between the actor dispatching a
//!   turn and the provider's first connected event, nothing marks the
//!   conversation — no observation, no stored tail — so
//!   [`settle_for_deletion`] sees no open stream and an erasure in that
//!   window deletes the conversation under a stream about to write. The
//!   failure direction is a loud error on that stream's write, possibly an
//!   orphaned answer block — never silent retention, because the prose is
//!   nulled by principal id before any deletion. Decision 0028 records the
//!   window as accepted.
//! - The store reissues a deleted conversation's id, so the erasure path
//!   calls [`StreamObserver::forget`] for every conversation it deleted; a
//!   dead entry never shadows the id's next holder.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_ledger::{CoreEvent, EventBus, Store};
use tokio::sync::broadcast::error::RecvError;

use crate::error::CoreError;

/// How long an erasure waits for an interrupted stream to settle before it
/// fails loudly, deleting nothing. Generous against a slow store, small
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

/// The conversations whose model stream is currently mid-turn, observed
/// from the bus: a turn opens on its first stream-status event and closes on
/// the turn's done, error or closed signal. The module doc states how the
/// observation can be wrong and what bounds each case.
#[derive(Default)]
pub(crate) struct StreamObserver {
    open: Mutex<HashSet<i64>>,
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

    /// Drop a deleted conversation's observation. The store reissues
    /// conversation ids, so an entry left behind would mark the id's next
    /// holder open before its first stream ever ran.
    pub(crate) fn forget(&self, conversation_id: i64) {
        self.set(conversation_id, false);
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
                } => observer.set(conversation_id, true),
                CoreEvent::StreamDone {
                    conversation_id, ..
                }
                | CoreEvent::StreamError {
                    conversation_id, ..
                }
                | CoreEvent::StreamClosed {
                    conversation_id, ..
                } => observer.set(conversation_id, false),
                _ => {}
            }
        }
    });
    observer
}

/// Settle one conversation ahead of its deletion. Idle — neither observed
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
/// [`CoreError::ErasureUnsettled`] if the stream did not settle before the
/// bound; [`CoreError::Store`] if a read fails.
pub(crate) async fn settle_for_deletion(
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
    if observed_open
        && let Err(error) = await_stream_end(&mut events, conversation_id, deadline).await
    {
        // The observation is dropped with the failure: a provider deaf to
        // the interrupt emits no end signal ever, so the entry would
        // otherwise stay open forever and every retry would burn the bound
        // on a wait that cannot end. The interrupt above has already torn
        // the stream's binding down, so a retry decides from stored state —
        // and a stream somehow still live re-marks itself on its next
        // stream event, per this observer's own resync rule.
        observer.forget(conversation_id);
        return Err(error);
    }
    confirm_settled(store, conversation_id, statuses_before, deadline).await
}

/// Await the conversation's stream-end signal — done, error or closed, the
/// terminal set the observer keys on — or fail at the deadline. The
/// stored-state confirmation after this wait is the check that decides; the
/// wait only spares that check its polling while the stream still runs.
async fn await_stream_end(
    events: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
    conversation_id: i64,
    deadline: tokio::time::Instant,
) -> Result<(), CoreError> {
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
            )) if ended == conversation_id => return Ok(()),
            Ok(Ok(_)) => {}
            // A lagged subscriber may have dropped the end signal; the
            // stored-state confirmation is the check that decides, so the
            // wait falls through instead of failing here.
            Ok(Err(RecvError::Lagged(_))) => return Ok(()),
            Ok(Err(RecvError::Closed)) | Err(_) => {
                return Err(CoreError::ErasureUnsettled { conversation_id });
            }
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
            return Err(CoreError::ErasureUnsettled { conversation_id });
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
    use super::*;

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
        await_stream_end(&mut events, 7, deadline).await.is_ok()
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
