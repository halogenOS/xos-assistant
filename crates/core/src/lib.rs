//! Platform-neutral core of the halogenOS Group Assistant.
//!
//! The core consumes the ledger framework: a platform-neutral inbound message
//! becomes a ledger block, takes a turn against the registered provider, and
//! comes back as an outbound reply on a subscription edge. Adapters translate
//! their platform's types into the core's message model at the boundary and
//! never past it.
//!
//! Invariant: the core contains no platform vocabulary. The committed word
//! list in `docs/platform-vocabulary.txt` makes that checkable, and a test
//! greps this crate against it.
//!
//! Every public item has exactly one path. The message vocabulary, the
//! assembly and the error type live at the crate root:
//!
//! - [`InboundMessage`] and [`OutboundReply`] with their parts — the core's
//!   own vocabulary, which adapters translate into and out of.
//! - [`Assistant`] — the assembly: runtime wiring, the ingestion entry point
//!   (answering with an [`IngestReceipt`]), the per-adapter outbound
//!   subscription, and erasure with its [`ErasureOutcome`].
//! - [`CoreError`] — what a core operation fails with.
//!
//! Two modules stay addressable by path, because their items read by their
//! module's name:
//!
//! - [`kind`] — the assistant's block kind, composed with the framework's
//!   kinds through the derive.
//! - [`schema`] — the store configuration and the domain tables; identity
//!   lives apart from the ledger so erasure never touches a block.

mod assembly;
mod erasure;
mod error;
mod identity;
pub mod kind;
mod mapping;
mod message;
mod outbound;
pub mod schema;

pub use assembly::{Assistant, IngestReceipt, ModelBinding};
pub use erasure::ErasureOutcome;
pub use error::CoreError;
pub use message::{
    Authority, ChannelKey, ChannelKind, InboundMessage, OutboundReply, SenderIdentity,
};
