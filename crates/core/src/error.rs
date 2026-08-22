//! The core's error type.

use agent_ledger::StoreError;

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

    /// A first-message claim found its channel mapping gone between the
    /// insert and the read back — the row was deleted mid-claim. The
    /// ingestion cannot tell which conversation now owns the channel, so it
    /// reports the lost claim instead of a bare missing-row error.
    #[error("the channel mapping vanished mid-claim; the message was not recorded")]
    ClaimLost,

    /// An erasure found a direct conversation with an open stream, emitted
    /// the interrupt, and the stream did not settle before the bound. The
    /// erasure deleted nothing: deleting under a still-writing stream would
    /// race the stream's own appends. A retry can succeed even against a
    /// provider that never answers the interrupt — the timed-out
    /// observation is dropped with this failure, so the retry decides from
    /// stored state and completes once the interrupt's teardown is in the
    /// ledger.
    #[error(
        "erasure could not settle the open stream of conversation \
         {conversation_id} before the bound; nothing was deleted"
    )]
    ErasureUnsettled {
        /// The direct conversation whose stream stayed open.
        conversation_id: i64,
    },
}

/// Whether retrying the same operation can come out differently. This is the
/// statement an adapter's batch discipline reads — never the variant names,
/// which are the core's own vocabulary and free to grow.
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
}

impl CoreError {
    /// Terminal or transient, for the message that caused this error.
    #[must_use]
    pub fn failure_kind(&self) -> FailureKind {
        match self {
            // The mapping recorded the channel's kind at creation and the
            // message claims another; no retry changes either side.
            Self::ChannelKindMismatch { .. } => FailureKind::Terminal,
            // Storage, wiring and timing failures can all pass on a retry;
            // the start-time refusals never reach a per-message caller.
            Self::Store(_)
            | Self::UnknownVendor { .. }
            | Self::MissingContentTable { .. }
            | Self::ClaimLost
            | Self::ErasureUnsettled { .. } => FailureKind::Transient,
        }
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

    #[test]
    fn every_other_variant_stays_transient() {
        let errors = [
            CoreError::Store(StoreError::ActorStopped),
            CoreError::UnknownVendor {
                vendor: "unregistered".into(),
            },
            CoreError::MissingContentTable { table: "absent" },
            CoreError::ClaimLost,
            CoreError::ErasureUnsettled { conversation_id: 1 },
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
