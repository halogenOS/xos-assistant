//! The filing door: the one serialization every write that files a block
//! AGAINST another record's origin passes through (unit 39, 2026-08-30).
//!
//! Three writers hold a copy of a message's platform origin, and all three
//! decide what to write by reading the ledger first: the report tool, the
//! react tool, and the deletion mirror that nulls those copies when an
//! administrator deletes the named message. A read-then-write pair is only
//! as good as what keeps another writer out of the gap between its halves,
//! and the two gaps that mattered were open:
//!
//! 1. BETWEEN THE FILINGS. The runner executes one round's tool calls in
//!    parallel tasks, so two calls naming one origin both scanned before
//!    either appended and both per-origin bounds — one reaction per
//!    message, one report per message, and no reaction beside a filed
//!    report — read a ledger that was already stale. A lock per tool
//!    closed this only within one tool.
//! 2. BETWEEN A FILING AND THE MIRROR. The mirror runs inline in the
//!    ingestion path, under that path's erasure-fence READ hold, and a
//!    filing takes the fence for reading too. Read and read are
//!    concurrent, so the fence — which orders a filing against the
//!    PERSON-WIDE erasure, and does that job — ordered nothing at all
//!    between a filing and the mirror. Scan, null, append landed a fresh
//!    copy of an id the mirror had just nulled, out of reach of every
//!    later pass, which is the residual decisions 0063 and 0085 close.
//!
//! Through this door the two interleavings are the only ones left, and
//! both are correct: the mirror's nulls precede a filing's scan, which
//! then finds no such message among the turn's own and declines it; or
//! they follow its append, and null the fresh copy with the rest.
//!
//! # The lock order, stated once
//!
//! The erasure fence is taken FIRST and this door SECOND, by every holder
//! of both. The ingestion path takes the fence, then the stamp lock, then
//! this door around the mirror's nulls; a filing tool takes the fence,
//! then this door. Taking the door first anywhere would close a cycle: a
//! queued erasure makes the fence's write hold pending, tokio's fence is
//! fair, so a task holding the door would wait for a fence read behind
//! that writer while the writer waits for the ingestion's read hold and
//! the ingestion waits for the door.
//!
//! One door across all conversations, not one per conversation: a filing
//! is rare — a moderation assessment, a reaction, an administrator's
//! deletion command — and a map of locks would buy contention nobody has.

use std::sync::Arc;

use tokio::sync::Mutex;

/// The shared handle each filing writer receives at its construction: the
/// tools at registration, the ingestion path from the assembly's own
/// field. Held around a scan-then-append pair, or around the mirror's
/// nulls, and never across a model call.
pub(crate) type FilingDoor = Arc<Mutex<()>>;

/// One door, for one assembly.
pub(crate) fn door() -> FilingDoor {
    Arc::new(Mutex::new(()))
}
