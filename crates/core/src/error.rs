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
}
