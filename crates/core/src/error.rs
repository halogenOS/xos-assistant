//! The core's error type, how far each failure reaches, and the signal an
//! unattended path raises when its failure reaches the whole process.

use agent_ledger::StoreError;
use tokio::sync::watch;

use crate::message::ChannelKind;

/// Everything a core operation can fail with.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The store refused or the storage layer failed.
    #[error("storage failed: {0}")]
    Store(#[from] StoreError),

    /// The assembly's model binding names a vendor no registered provider
    /// module answers to. The vendor is what resolves a conversation to its
    /// module, so started anyway, every conversation created under the
    /// binding would stay silent with nothing to point at the cause.
    #[error("the binding's vendor `{vendor}` matches no registered provider module")]
    UnknownVendor {
        /// The vendor the binding names.
        vendor: String,
    },

    /// The store the assembly was handed was not opened with the assistant's
    /// configuration: the message kind's content table is missing from the
    /// store's effective content-table list. Every append would fail later
    /// and further from the cause, so the assembly refuses to start.
    #[error(
        "the store lacks the content table `{table}`: it was not opened with \
         the assistant's store configuration"
    )]
    MissingContentTable {
        /// The table the kind's descriptor declares.
        table: &'static str,
    },

    /// A message arrived on a known channel claiming a different channel kind
    /// than the mapping recorded at creation. The kind decides what erasure
    /// does with the channel key, so a silent disagreement would corrupt the
    /// privacy contract; the message is refused instead.
    #[error(
        "the channel is mapped as `{}` but the message claims `{}`",
        stored.as_str(),
        claimed.as_str()
    )]
    ChannelKindMismatch {
        /// The kind the mapping recorded at creation.
        stored: ChannelKind,
        /// The kind the inbound message carries.
        claimed: ChannelKind,
    },

    /// The message's sender authority arrived unresolved — the adapter's
    /// authority source failed — for a channel the core admitted. Authority
    /// is never defaulted into the ledger, so the message is refused and
    /// nothing is recorded; the refusal is transient, and the adapter's
    /// batch discipline halts on it so the message redelivers once the
    /// source answers. Judged after admission on purpose (refined
    /// 2026-08-23): an unadmitted group is refused with the withdraw
    /// directive before authority is ever read, so a stranger group whose
    /// authority source keeps failing can never wedge the batch.
    #[error("the sender's authority is unresolved; the message was not recorded")]
    AuthorityUnresolved,

    /// A first-message claim found its channel mapping gone between the
    /// insert and the read back — the row was deleted mid-claim. The
    /// ingestion cannot tell which conversation now owns the channel, so it
    /// reports the lost claim instead of a bare missing-row error.
    #[error("the channel mapping vanished mid-claim; the message was not recorded")]
    ClaimLost,

    /// A path that had to stop a conversation's stream before changing what
    /// the conversation is — an erasure deleting it, a session replacement
    /// copying its history onto a successor and unmapping it — emitted the
    /// interrupt, and the stream did not settle before the bound. Nothing
    /// was deleted and nothing was swapped: acting under a still-writing
    /// stream would race the stream's own appends. A retry can succeed even
    /// against a provider that never answers the interrupt — the timed-out
    /// observation is dropped with this failure, so the retry decides from
    /// stored state and completes once the interrupt's teardown is in the
    /// ledger.
    #[error(
        "the open stream of conversation {conversation_id} did not settle \
         before the bound; nothing was changed"
    )]
    StreamUnsettled {
        /// The conversation whose stream stayed open.
        conversation_id: i64,
    },

    /// A compaction's temporary conversation produced no summary before its
    /// bound: the turn failed, ended silently, or never ran. Nothing was
    /// swapped and nothing was deleted — the conversation the channel is on
    /// stands exactly as it did, and the next trigger re-derives the whole
    /// operation from the ledger.
    #[error(
        "the compaction of conversation {conversation_id} captured no summary; \
         nothing was changed"
    )]
    CompactionUnsummarized {
        /// The conversation the compaction was for.
        conversation_id: i64,
    },
}

/// How far a failure reaches: this message, or everything after it. This is
/// the statement an adapter's batch discipline reads — never the variant
/// names, which are the core's own vocabulary and free to grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Provably deterministic: the same input fails the same way every time,
    /// so retrying it forever would wedge everything behind it. The caller
    /// records the refusal and moves on.
    Terminal,
    /// Everything else, storage and timing included. Conservative on
    /// purpose: an error that is not provably deterministic is treated as
    /// retryable, because dropping a message over a passing condition is the
    /// worse failure.
    Transient,
    /// The failure is not about this message: the process cannot serve any
    /// message from here on, and no retry of anything will change that. The
    /// caller stops instead of retrying, leaving the message unacknowledged
    /// so it redelivers to the replacement process (2026-09-01).
    Fatal,
}

impl CoreError {
    /// How far this failure reaches, for the message that caused it.
    ///
    /// The store hands a database failure back sorted into its own class and
    /// acts on none of them — see `agent_ledger::StoreError`. This is where
    /// the reaction is decided, because this is the first place that knows
    /// what the failure was scoped to: one inbound message, or the process.
    #[must_use]
    pub fn failure_kind(&self) -> FailureKind {
        match self {
            // The mapping recorded the channel's kind at creation and the
            // message claims another; no retry changes either side.
            Self::ChannelKindMismatch { .. } => FailureKind::Terminal,
            // Three classes, one answer. The database refused a statement by
            // a rule this code violated — a constraint and never contention
            // (2026-09-02) — or it is damaged, is not a database, was used
            // against its contract, or its one writer is gone (2026-09-01).
            // Nothing here passes on a retry: a refused statement is refused
            // the same way every time and leaves the ledger in a shape this
            // code cannot continue from, and a database that cannot answer
            // meets every later message with the same wall. So the process
            // ends and the supervisor starts a replacement over the durable
            // state, where the startup walk runs before anything is served.
            Self::Store(
                StoreError::Rejected(_) | StoreError::Unusable(_) | StoreError::ActorStopped,
            ) => FailureKind::Fatal,
            // Storage, wiring and timing failures can all pass on a retry;
            // the start-time refusals never reach a per-message caller. A
            // race with another writer is among them: it is about what this
            // message asked for, and the next attempt can win it.
            Self::Store(_)
            | Self::UnknownVendor { .. }
            | Self::MissingContentTable { .. }
            | Self::AuthorityUnresolved
            | Self::ClaimLost
            | Self::StreamUnsettled { .. }
            | Self::CompactionUnsummarized { .. } => FailureKind::Transient,
        }
    }
}

/// The one signal a failure with no caller raises: this process cannot go
/// on, and something outside it has to end it.
///
/// A [`FailureKind::Fatal`] failure on a per-message path already ends the
/// process: the intake stops its run with the message unacknowledged, the
/// binary exits, and the supervisor starts a replacement. An unattended path
/// — the compaction driver — has no caller to stop and no message to leave
/// unacknowledged, so it states the same thing here, and the binary waits on
/// this beside the termination signal.
///
/// The signal latches: it is raised once, and a wait that starts after the
/// raise answers immediately, so nothing depends on somebody listening at
/// the moment the failure happens.
pub(crate) struct FatalExit(watch::Sender<bool>);

impl FatalExit {
    /// A signal nobody has raised.
    pub(crate) fn new() -> Self {
        Self(watch::channel(false).0)
    }

    /// State that this process cannot go on, naming the failure in the log
    /// — the one record of what it was, since nothing downstream carries the
    /// error itself.
    ///
    /// The FIRST raise is the whole event: it writes the record and wakes
    /// every wait. A later one changes nothing at all, logging included — the
    /// process is already ending, and a second line about a second failure on
    /// the way out reads like a second incident. The log is written under the
    /// signal's own lock, ahead of the wake, so the record is in the log
    /// before anything waiting on the exit can act on it.
    pub(crate) fn raise(&self, failure: &CoreError) {
        self.0.send_if_modified(|raised| {
            if *raised {
                return false;
            }
            tracing::error!(%failure, "the core cannot serve; the process ends for a restart");
            *raised = true;
            true
        });
    }

    /// Resolves once the signal is raised, at once when it already is.
    pub(crate) async fn raised(&self) {
        let mut listener = self.0.subscribe();
        // The wait reads the current value before it waits for a change,
        // which is what makes an earlier raise answer this call.
        //
        // The discarded result is the closed channel, which cannot happen
        // here: the sender is this very object and the caller is holding it
        // borrowed for the whole await, so the only way the wait ends is the
        // raise it is waiting for.
        let _ = listener.wait_for(|raised| *raised).await;
    }
}

// The classification is a contract an adapter's batch discipline acts on
// blind — a variant silently reclassified would wedge or drop messages with
// no other test noticing — so every variant's answer is pinned here, one by
// one, on the source side of the contract.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_mismatch_is_the_one_terminal_refusal() {
        let refusal = CoreError::ChannelKindMismatch {
            stored: ChannelKind::Direct,
            claimed: ChannelKind::Group,
        };
        assert_eq!(refusal.failure_kind(), FailureKind::Terminal);
    }

    /// The class the store put on a database failure decides how far it
    /// reaches: a contended write is about the message that made it, while a
    /// refused one, a damaged database and a departed writer are about
    /// everything after it. The refusal and the contention are asserted side
    /// by side because they are the pair that is easy to confuse: a rule the
    /// code violated is answered the same way every time, and a race is not.
    #[test]
    fn the_stores_classes_split_the_message_from_the_process() {
        let scripted = || {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                Some("scripted".to_owned()),
            )
        };
        for (error, expected) in [
            (
                CoreError::Store(StoreError::Rejected(scripted())),
                FailureKind::Fatal,
            ),
            (
                CoreError::Store(StoreError::Contended(scripted())),
                FailureKind::Transient,
            ),
            (
                CoreError::Store(StoreError::Sqlite(scripted())),
                FailureKind::Transient,
            ),
            (
                CoreError::Store(StoreError::Unusable(scripted())),
                FailureKind::Fatal,
            ),
            (
                CoreError::Store(StoreError::ActorStopped),
                FailureKind::Fatal,
            ),
        ] {
            assert_eq!(error.failure_kind(), expected, "`{error}` is misjudged");
        }
    }

    /// The exit signal latches. The failure happens on a task of its own and
    /// the binary reaches its wait whenever the start sequence gets there, so
    /// a raise that came first has to answer the wait that follows it — a
    /// signal only a listener present at the moment could hear would lose
    /// exactly the failure that happens during startup.
    #[tokio::test]
    async fn the_fatal_exit_answers_a_wait_that_starts_after_the_raise() {
        let fatal = FatalExit::new();
        fatal.raise(&CoreError::Store(StoreError::Rejected(
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER),
                Some("scripted".to_owned()),
            ),
        )));
        tokio::time::timeout(std::time::Duration::from_secs(5), fatal.raised())
            .await
            .expect("the wait answers the signal raised before it");
    }

    #[test]
    fn every_other_variant_stays_transient() {
        let errors = [
            CoreError::UnknownVendor {
                vendor: "unregistered".into(),
            },
            CoreError::MissingContentTable { table: "absent" },
            CoreError::AuthorityUnresolved,
            CoreError::ClaimLost,
            CoreError::StreamUnsettled { conversation_id: 1 },
            CoreError::CompactionUnsummarized { conversation_id: 1 },
        ];
        for error in errors {
            assert_eq!(
                error.failure_kind(),
                FailureKind::Transient,
                "`{error}` must classify transient: it is not provably \
                 deterministic for the message that caused it"
            );
        }
    }
}
