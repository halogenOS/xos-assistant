//! The adapter suite: the scripted Bot API server on the loopback interface,
//! the real core assembly with this unit's own scripted provider, and the
//! adapter under test between them. Every test is parallel-safe — its own
//! server port, its own in-memory store, its own state file — and nothing
//! reaches past the loopback interface.
//!
//! The token scan (AC6) lives in its own test target, `token_scan`, because
//! it owns the process-wide subscriber; see its module doc.

mod addressing;
mod classification;
mod end_to_end;
mod group_context;
mod offset;
mod sending;
mod server;
mod support;
mod tools;
mod translation;
