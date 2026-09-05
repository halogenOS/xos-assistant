//! One channel's thread lineage: the conversation serving it, and every
//! conversation that one continues, walked to the root (unit T4,
//! 2026-08-31).
//!
//! A compaction hands a channel a NEW conversation carrying a digest of the
//! first half and the second half verbatim, so a channel that has been
//! compacted twice is served by a conversation that continues a conversation
//! that continues a conversation. Every one of them held the same channel's
//! messages, and a platform message id is unique per CHANNEL and nowhere
//! else — so a reader that scopes a lookup to the serving conversation alone
//! goes blind on everything the newest thread did not inherit, while a
//! reader that scopes to nothing at all would reach a stranger channel's
//! row.
//!
//! The lineage is that scope. The compacted-digest scrub walks the same
//! chain for its own reason with a loop of its own, but it reads each hop's
//! opening through this module, so the two agree on what a thread continues;
//! folding the scrub's walk into this one is a recorded candidate.

use agent_ledger::agency::{AncestorReference, LeafKind};
use agent_ledger::{Block, Store};

use crate::error::CoreError;

/// Where one thread's own opening is: the conversation its ancestor
/// reference names, and the block that opening ends at.
#[derive(Debug)]
pub(crate) struct ThreadOpening {
    /// The conversation this thread continues.
    pub ancestor: i64,
    /// The digest behind the reference — the last block of the thread's own
    /// opening. Everything past it is inherited history.
    pub opening_ends: i64,
}

/// The thread's OWN ancestor reference, read by BLOCK ID, not by ledger
/// position.
///
/// A compaction's appends are the newest blocks it writes while the rows it
/// inherits are older, so a thread's own opening carries the HIGHEST ids in
/// its ledger and sits at the FRONT of it — ids descend at that seam. Ledger
/// order therefore cannot pick the reference out: a reference that rode
/// across inside an inherited half would be the last one in ledger order and
/// still not this thread's. The greatest id is the thread's own, which is the
/// same reading the forced-turn-end door takes of the same block.
///
/// `Ok(None)` is a conversation that continues nothing — the root of a
/// lineage, or a conversation that was never compacted.
///
/// # Errors
///
/// [`CoreError::AncestorUnnamed`] for a reference row naming NO
/// conversation, which is fatal: the failure ends the process wherever it is
/// read, because the stored shape is one this code never writes and every
/// lineage-scoped read behind here would otherwise go silently blind on the
/// inherited half — a thread that continues something would answer as one
/// that continues nothing, and the reader would find no trace of why.
pub(crate) fn own_opening(blocks: &[Block]) -> Result<Option<ThreadOpening>, CoreError> {
    let Some(opening) = blocks
        .iter()
        .filter(|block| AncestorReference::KINDS.contains(&block.block_type.as_str()))
        .max_by_key(|block| block.id)
    else {
        return Ok(None);
    };
    let Some(opening_ends) = blocks
        .iter()
        .skip_while(|block| block.id != opening.id)
        .nth(1)
        .map(|block| block.id)
    else {
        return Ok(None);
    };
    let ancestor = AncestorReference::parse(opening)
        .conversation_id
        .ok_or_else(|| CoreError::AncestorUnnamed {
            block_id: opening.id,
            block_type: opening.block_type.clone(),
        })?;
    Ok(Some(ThreadOpening {
        ancestor,
        opening_ends,
    }))
}

/// Every conversation that has served this channel in one line, NEWEST
/// FIRST: the serving conversation, then each conversation it continues,
/// down to the root.
///
/// A conversation that continues nothing answers with itself alone, which is
/// the ordinary case for a channel that was never compacted.
///
/// The walk stops at a reference whose conversation no longer reads — a
/// retired ancestor, whose blocks went with it — because there is nothing
/// further to scope against. Conversation ids are reissued after a deletion,
/// so a reference could in principle point back into the chain; the walked
/// set is what keeps that from looping.
///
/// # Errors
///
/// [`CoreError::Store`] if a read fails or the store's actor has stopped;
/// [`CoreError::AncestorUnnamed`] for the stored shape [`own_opening`]
/// refuses.
pub(crate) async fn serving_lineage(store: &Store, serving: i64) -> Result<Vec<i64>, CoreError> {
    let mut walked = vec![serving];
    let mut blocks = store.list_blocks(serving).await?;
    while let Some(opening) = own_opening(&blocks)? {
        if walked.contains(&opening.ancestor) {
            break;
        }
        let ancestor_blocks = store.list_blocks(opening.ancestor).await?;
        if ancestor_blocks.is_empty() {
            break;
        }
        walked.push(opening.ancestor);
        blocks = ancestor_blocks;
    }
    Ok(walked)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One block of the given kind, with the given fields.
    fn block(
        id: i64,
        block_type: &str,
        fields: serde_json::Map<String, serde_json::Value>,
    ) -> Block {
        Block {
            id,
            role: None,
            block_type: block_type.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// A stored reference names its ancestor: the walk reads it and the
    /// opening ends at the block behind it.
    #[test]
    fn a_reference_names_the_conversation_it_continues() {
        let mut fields = serde_json::Map::new();
        fields.insert("ancestor_conversation_id".into(), serde_json::json!(7));
        let opening = own_opening(&[
            block(4, AncestorReference::KINDS[0], fields),
            block(5, "text", serde_json::Map::new()),
        ])
        .expect("the reference reads")
        .expect("the reference is the thread's own opening");
        assert_eq!(opening.ancestor, 7);
        assert_eq!(opening.opening_ends, 5);
    }

    /// A reference naming NO conversation answers the failure that ends the
    /// process, naming the row, instead of reading as a thread that
    /// continues nothing: the column is NOT NULL, so the shape is a
    /// corrupted database and every lineage-scoped read behind it would go
    /// blind with no trace.
    ///
    /// What is asserted is the CLASS: the callers hand this failure to the
    /// same reading every unattended path takes, and only a fatal class ends
    /// the process there. A panic would have ended the task alone and left
    /// the process serving.
    #[test]
    fn a_reference_naming_no_conversation_answers_a_fatal_failure() {
        let failure = own_opening(&[
            block(4, AncestorReference::KINDS[0], serde_json::Map::new()),
            block(5, "text", serde_json::Map::new()),
        ])
        .expect_err("a reference naming no conversation is refused");
        assert!(
            matches!(
                failure,
                CoreError::AncestorUnnamed { block_id, ref block_type }
                    if block_id == 4 && block_type == AncestorReference::KINDS[0]
            ),
            "the failure names the row: {failure}"
        );
        assert_eq!(
            failure.failure_kind(),
            crate::error::FailureKind::Fatal,
            "the process ends on it"
        );
    }
}
