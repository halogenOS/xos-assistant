//! The core-spine integration suite: the assistant as the framework's
//! consumer, proven end to end against public API alone.
//!
//! One test binary on purpose: the scripted provider, the assembly helpers
//! and the ledger-polling helpers are shared by every module here, and a
//! single compilation keeps them all exercised. The modules split by concern:
//! assembly (the wiring contract), storage (the composed kind and the durable
//! registry), erasure, and the end-to-end turn.

mod assembly;
mod end_to_end;
mod erasure;
mod storage;
mod support;
