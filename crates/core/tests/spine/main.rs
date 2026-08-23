//! The core-spine integration suite: the assistant as the framework's
//! consumer, proven end to end against public API alone.
//!
//! One test binary on purpose: the scripted provider, the assembly helpers
//! and the ledger-polling helpers are shared by every module here, and a
//! single compilation keeps them all exercised. The modules split by concern:
//! assembly (the wiring contract), storage (the composed kind and the durable
//! registry), addressing (the answer-due stamp, the notice, re-engagement),
//! protection (the budgets, the limited stamp, the debt authority),
//! `direct_chats` (the configuration switch refusing direct channels
//! before any write), disclosure (the first-interaction line and the
//! deterministic replies' exemption),
//! projection (role alternation under erasure), speaker (the username
//! projection), erasure with its stream
//! ordering, the end-to-end turn, tools (the lookups against the scripted
//! forge and mirror in `lookup_wire`, the palette, the anchor gate over
//! the turn's provenance), `privacy_rights` (the suppression drop, the
//! self-service commands, the spawned deletion and the privacy tool), and —
//! behind the openrouter feature — the real `OpenRouter` module against a
//! loopback server.

mod addressing;
mod assembly;
mod direct_chats;
mod disclosure;
mod end_to_end;
mod erasure;
mod erasure_streams;
mod group_context;
mod lookup_wire;
#[cfg(feature = "openrouter")]
mod openrouter;
mod privacy_rights;
mod projection;
mod protection;
mod report;
mod speaker;
mod storage;
mod support;
mod tools;
