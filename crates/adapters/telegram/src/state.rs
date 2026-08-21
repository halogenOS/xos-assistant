//! The persisted update offset, per decision 0014: the state file holds the
//! next offset to send — the highest acknowledged update id plus one —
//! written after a batch's messages are ingested. A crash between ingest and
//! write redelivers, and the duplicates are the accepted outcome; the file
//! makes that redelivery window explicit and testable.

use std::path::Path;

/// The stored offset, or `None` when the file is absent, empty or
/// malformed — all three read as absent by decision, with the two
/// abnormal cases logged so an operator can see the redelivery coming.
pub(crate) fn read(path: &Path) -> Option<i64> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(%error, "the state file did not read; treating it as absent");
            return None;
        }
    };
    let trimmed = content.trim();
    if trimmed.is_empty() {
        tracing::warn!("the state file is empty; treating it as absent");
        return None;
    }
    if let Ok(offset) = trimmed.parse() {
        Some(offset)
    } else {
        tracing::warn!("the state file is malformed; treating it as absent");
        None
    }
}

/// Persist the next offset: written whole to a sibling file, then renamed
/// over the state file. The rename keeps a process crash mid-write from
/// leaving a torn file — the old offset survives, and a torn file would
/// read as absent and widen the redelivery window for no reason. Power
/// loss is not defended: nothing is fsynced, because the worst a lost
/// write yields is the accepted duplicate window, the exact outcome an
/// fsync would be paying to avoid.
pub(crate) fn write(path: &Path, next_offset: i64) -> std::io::Result<()> {
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push(".next");
    std::fs::write(&sidecar, next_offset.to_string())?;
    std::fs::rename(&sidecar, path)
}
