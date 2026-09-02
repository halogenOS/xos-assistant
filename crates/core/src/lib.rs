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
//! - [`InboundMessage`] and [`Outbound`] with their parts — the core's
//!   own vocabulary, which adapters translate into and out of. What the
//!   outbound edge yields is [`Outbound`]: an [`OutboundReply`] of words,
//!   or an [`OutboundMark`] of one emoji to put on a message.
//! - [`Assistant`] — the assembly: runtime wiring, the ingestion entry point
//!   (answering with an [`IngestOutcome`], its stamp bounded by the
//!   [`ProtectionConfig`] budgets and its group admission checked against
//!   the [`OperatorConfig`]), the observation entry point (answering with an
//!   [`ObserveOutcome`]), the per-adapter outbound subscription, the
//!   per-adapter composing subscription (yielding [`ComposingUpdate`]
//!   transitions), erasure with its [`ErasureOutcome`], and the retention
//!   sweep enforcing the [`RetentionConfig`] span.
//! - [`CoreError`] — what a core operation fails with.
//!
//! The public modules stay addressable by path, because their items read by
//! their module's name:
//!
//! - [`commands`] — the command catalogue: the one list of commands this
//!   assistant answers, the recognition that folds ASCII case, and the
//!   reading of who is offered each command in which kind of channel.
//! - [`delivery`] — the delivery-receipt kind: one message the assistant
//!   successfully sent, recorded through [`Assistant::report_delivery`] so
//!   a reply to it can quote her stored words.
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
//! - [`tools`] — the project lookups, the react tool putting one emoji on
//!   a message, the web search behind its own configured key, and the
//!   provenance reading the anchor gate takes; the assembly takes its
//!   [`tools::ToolSet`], registers every tool in it, and records the
//!   framework's tool choice naming exactly those tools on every
//!   conversation it creates.
//! - `provider` (behind the `chat_completions` feature) — the framework's
//!   `OpenRouter` module, reused as the shared chat-completions wire against
//!   any OpenAI-compatible endpoint, wrapped around an in-memory
//!   configuration so the API key never enters the store.

mod acknowledgment;
mod assembly;
mod authorization;
pub mod commands;
mod compaction;
mod composing;
pub mod delivery;
mod disclosure;
mod erasure;
mod error;
mod filing;
mod identity;
pub mod join;
pub mod kind;
mod lineage;
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
mod retention;
pub mod schema;
mod session;
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
    Authority, ChannelKey, ChannelKind, ChannelReset, ComposingState, ComposingUpdate,
    DeliveryHandle, DeliveryItem, InboundMessage, IngestOutcome, IngestReceipt, InvokedCommand,
    JoinedMember, Observation, ObserveOutcome, ObservedDelivery, ObservedFact, Outbound,
    OutboundMark, OutboundReply, QuotedExcerpt, ReplyKind, ReplyTarget, ReplyThread,
    SenderIdentity,
};
pub use outbound::{PRIVACY_ANSWER_LEAD, PRIVACY_UNPUBLISHED, RULES_ACKNOWLEDGMENT};
pub use retention::RetentionConfig;
pub use teaching::{
    Capabilities, MODERATION_TEACHING, REACT_TEACHING, SEARCH_TEACHING, composed_system_prompt,
    moderation_taught,
};
pub use window::{
    ACKNOWLEDGMENT_WINDOW, PRIVACY_REPLY_CAP, PRIVACY_REPLY_WINDOW, RESET_REPLY_CAP,
    RESET_REPLY_WINDOW, SEARCH_BUDGET_CAP, SEARCH_BUDGET_WINDOW,
};
