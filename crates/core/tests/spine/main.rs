//! The core-spine integration suite: the assistant as the framework's
//! consumer, proven end to end against public API alone.
//!
//! One test binary on purpose: the scripted provider, the assembly helpers
//! and the ledger-polling helpers are shared by every module here, and a
//! single compilation keeps them all exercised. The modules split by concern:
//! assembly (the wiring contract), storage (the composed kind and the durable
//! registry), addressing (the answer-due stamp, the notice, re-engagement),
//! protection (the budgets, the limited stamp, the debt authority),
//! projection (role alternation under erasure), erasure with its stream
//! ordering, the end-to-end turn, and — behind the openrouter feature — the
//! real `OpenRouter` module against a loopback server.

mod addressing;
mod assembly;
mod end_to_end;
mod erasure;
mod erasure_streams;
#[cfg(feature = "openrouter")]
mod openrouter;
mod projection;
mod protection;
mod storage;
mod support;
