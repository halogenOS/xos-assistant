//! The shutdown's one fact, in a module of its own: whether this process
//! has begun stopping (decision 0200, 2026-09-04).
//!
//! It lives here and not with the shutdown that raises it because two
//! leaves read it — the erasure spawn and the deletion flow's pending
//! memory — and a leaf names nothing in the module that assembles it.

/// Whether this process has begun shutting down.
///
/// The assembly's shutdown raises it, inside the worker lock, so no erasure
/// is spawned past the list that shutdown took. Two readers ask it: the
/// erasure spawn, which starts none once it stands, and the deletion flow's
/// pending memory ([`crate::privacy::PendingDeletions`], handed the fact at
/// construction), which files no pending whose confirm this same process
/// would refuse. Nobody is promised an erasure this process will not run.
///
/// The fact is raised inside the worker lock by the shutdown, re-checked
/// there by the erasure spawn, and read by the pending memory.
#[derive(Debug, Default)]
pub(crate) struct ServiceStopping(std::sync::atomic::AtomicBool);

impl ServiceStopping {
    /// Record that the shutdown has begun. It never goes back: a process
    /// that started stopping serves no erasure again.
    pub(crate) fn begin(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether the shutdown has begun.
    pub(crate) fn begun(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}
