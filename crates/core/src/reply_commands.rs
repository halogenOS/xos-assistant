//! The command shapes a moderation bot acts on when they arrive as a reply.
//!
//! One enumeration, consumed by the answer-threading guard: an answer whose
//! prose carries any of these shapes is delivered plain (decision 0108,
//! widened 2026-08-27), because threading such an answer would turn stray
//! prose into a real command reply — a filed report or a deletion —
//! bypassing every check the real path performs. Each shape's definition
//! stays with the module that owns its behaviour; this list is the one
//! record of which shapes are acted on from replies. A new reply-acted
//! command is added here, or the guard goes blind to it.

use crate::mirror::DELETION_COMMAND;
use crate::tools::report::REPORT_LINE_LEAD;

/// Every command shape acted on when it arrives as a reply.
pub const ACTED_FROM_REPLIES: &[&str] = &[REPORT_LINE_LEAD, DELETION_COMMAND];

#[cfg(test)]
mod tests {
    use super::*;

    /// The list carries both shapes the core records as reply-acted. A
    /// shape leaving this list must be a decision, not a refactor.
    #[test]
    fn the_list_names_both_reply_acted_shapes() {
        assert!(ACTED_FROM_REPLIES.contains(&REPORT_LINE_LEAD));
        assert!(ACTED_FROM_REPLIES.contains(&DELETION_COMMAND));
        assert_eq!(ACTED_FROM_REPLIES.len(), 2);
    }
}
