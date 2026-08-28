//! The core-spine integration suite: the assistant as the framework's
//! consumer, proven end to end against public API alone.
//!
//! One test binary on purpose: the scripted provider, the assembly helpers
//! and the ledger-polling helpers are shared by every module here, and a
//! single compilation keeps them all exercised. The modules split by concern:
//! acknowledgment (the rules acknowledgment's bounded one-shot generation
//! and its deterministic fallback),
//! assembly (the wiring contract), storage (the composed kind and the durable
//! registry), audience (the clarifying question's ordinary delivery and
//! the two-turn disambiguation),
//! addressing (the answer-due stamp, the notice, re-engagement),
//! protection (the budgets, the limited stamp, the debt authority),
//! reasoning (the configured effort level on every created conversation
//! and on the provider's requests),
//! `date_marker` (the framework's calendar row: written once per recorded
//! date, ahead of the message that tripped it, reaching the model as its
//! own system line — the fact every other module's consumer view filters),
//! `direct_chats` (the configuration switch refusing direct channels
//! before any write), disclosure (the first-interaction line and the
//! deterministic replies' exemption),
//! helpful (the answering mode's summons, the silent empty turn and the
//! unspent window),
//! projection (role alternation under erasure), speaker (the username
//! projection), erasure with its stream
//! ordering, the end-to-end turn, tools (the lookups against the scripted
//! forge and mirror in `lookup_wire`, the palette, the anchor gate over
//! the turn's provenance), `privacy_rights` (the suppression drop, the
//! self-service commands, the spawned deletion and the privacy tool),
//! `mirror` (the deletion mirror riding the moderation bot's reply
//! command), sourcing (the lookup-backed answer discipline: the literal
//! addressed fact beside the summons, the silent empty turn and the
//! model's own spoken don't-know), threading (which message an answer is
//! delivered as a reply to, and when it goes out plain), and — behind the
//! `chat_completions`
//! feature — the framework's real `OpenRouter` module, the shared
//! chat-completions wire, against a loopback server.

mod acknowledgment;
mod addressing;
mod assembly;
mod audience;
#[cfg(feature = "chat_completions")]
mod chat_completions;
mod date_marker;
mod direct_chats;
mod disclosure;
mod end_to_end;
mod erasure;
mod erasure_streams;
mod group_context;
mod helpful;
mod lookup_wire;
mod mirror;
mod privacy_rights;
mod projection;
mod protection;
mod reasoning;
mod report;
mod sourcing;
mod speaker;
mod storage;
mod support;
mod threading;
mod tools;
