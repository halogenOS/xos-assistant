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
//!   (answering with an [`IngestOutcome`], its stamp bounded by the
//!   [`ProtectionConfig`] budgets and its group admission checked against
//!   the [`OperatorConfig`]), the observation entry point (answering with an
//!   [`ObserveOutcome`]), the per-adapter outbound subscription, the
//!   per-adapter composing subscription (yielding [`ComposingUpdate`]
//!   transitions), and erasure with its [`ErasureOutcome`].
//! - [`CoreError`] — what a core operation fails with.
//!
//! The public modules stay addressable by path, because their items read by
//! their module's name:
//!
//! - [`join`] — the join-notice kind: one person's recorded entry into a
//!   group, stored through the observation surface and erased with the
//!   person.
//! - [`kind`] — the assistant's block kind, composed with the framework's
//!   kinds through the derive.
//! - [`mirror`] — the deletion mirror: the moderation bot's reply deletion
//!   command, recognized from an administrator and answered with a silent
//!   one-row erasure of the named message.
//! - [`note`] — the context-note kind carrying a group's observed facts,
//!   and the rules contract that reads the pinned announcement.
//! - [`privacy`] — the privacy command family's recognition and its fixed
//!   lines: the notice, the opt-out and opt-in, and the deletion with its
//!   programmatic confirm.
//! - [`schema`] — the store configuration and the domain tables; identity
//!   lives apart from the ledger so erasure never touches a block.
//! - [`tools`] — the project lookups, the web search behind its own
//!   configured key, the palette kind that gates their admission, the
//!   provenance reading the anchor gate takes, and the admission wrapper
//!   enforcing both; the assembly takes its [`tools::ToolSet`] and
//!   registers every tool behind that wrapper.
//! - `provider` (behind the `chat_completions` feature) — the framework's
//!   `OpenRouter` module, reused as the shared chat-completions wire against
//!   any OpenAI-compatible endpoint, wrapped around an in-memory
//!   configuration so the API key never enters the store.

mod acknowledgment;
mod assembly;
mod authorization;
mod composing;
mod disclosure;
mod erasure;
mod error;
mod identity;
pub mod join;
pub mod kind;
mod mapping;
mod message;
pub mod mirror;
pub mod note;
mod outbound;
pub mod privacy;
#[cfg(feature = "chat_completions")]
pub mod provider;
mod quoting;
mod reply_commands;
pub mod schema;
mod streams;
mod teaching;
pub mod tools;
mod window;

pub use assembly::{
    AnsweringMode, AssemblyConfig, Assistant, Budget, DirectChats, ModelBinding, OperatorConfig,
    ProtectionConfig, ScriptedPause,
};
// Re-exported so the embedder names the reasoning level through the core's
// own surface, beside the rest of the assembly configuration's vocabulary.
pub use agent_ledger::providers::ReasoningLevel;
pub use composing::COMPOSING_SIGNAL_LIFETIME;
pub use disclosure::{Disclosure, composed_disclosure_line};
pub use erasure::ErasureOutcome;
pub use error::{CoreError, FailureKind};
pub use message::{
    Authority, ChannelKey, ChannelKind, ComposingState, ComposingUpdate, DeliveryItem,
    InboundMessage, IngestOutcome, IngestReceipt, InvokedCommand, JoinedMember, Observation,
    ObserveOutcome, ObservedFact, OutboundReply, QuotedExcerpt, ReplyKind, ReplyTarget,
    ReplyThread, SenderIdentity,
};
pub use outbound::{
    FAILURE_NOTICE, PRIVACY_ANSWER_LEAD, PRIVACY_UNPUBLISHED, RULES_ACKNOWLEDGMENT,
};
pub use teaching::{
    Capabilities, MODERATION_TEACHING, SEARCH_TEACHING, composed_system_prompt, moderation_taught,
};
pub use window::{
    ACKNOWLEDGMENT_WINDOW, PRIVACY_REPLY_CAP, PRIVACY_REPLY_WINDOW, SEARCH_BUDGET_CAP,
    SEARCH_BUDGET_WINDOW,
};
