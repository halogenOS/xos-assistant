//! What a compact keeps and what it cuts: one pure reading of a
//! conversation's blocks, with no store and no mapping anywhere near it.
//!
//! The machinery that forks the conversation and re-points the channel lives
//! in [`crate::session`]; this module answers the only question that needs
//! judgment — which of the source's blocks the fork stops holding, and
//! whether there was anything to cut at all.
//!
//! # What survives
//!
//! - **The trailing chat rows**, up to [`COMPACT_KEPT_MESSAGES`] of them. A
//!   chat row is a member's recorded message, one of the assistant's own text
//!   blocks, or a stored quote of an earlier message: the three shapes that
//!   carry the conversation itself. Their stamps and their debts ride along
//!   untouched, so an unanswered question inside the kept tail still owes its
//!   turn.
//! - **The calendar around them**: every date record standing among the kept
//!   rows, plus the newest one before the oldest kept row, so the kept rows
//!   keep their own day instead of reading as undated.
//! - **The newest tool palette.** It is configuration, not traffic: a fork
//!   whose first wake is a turn would otherwise run with no tools admitted
//!   at all.
//! - **The newest context note per observed fact** — the group's title and
//!   its rules — so the model keeps what the group is without waiting for
//!   the platform fact to change again.
//!
//! # What is cut, and why each one
//!
//! Tool traffic — a call, its result, its error — is exactly the poison the
//! command exists to remove, so none of it survives, not even inside the
//! kept tail. Because no call crosses, the fork can never open on a call
//! with no result behind it. Join notices go because a reset session owes no
//! memory of who walked in a month ago, delivery records because they
//! project nothing to the model, and filed reports because their
//! deliveredness lives only in the outbound edge's process memory: this
//! reading cannot tell a delivered report from a pending one, and keeping
//! both would re-deliver — which the delivery contract refuses above all.
//! A report still pending at a compact is therefore lost, exactly as one
//! pending when the process dies is lost.
//!
//! Nothing here deletes a block. The caller detaches the named blocks from
//! the FORK; the source conversation keeps every one of them, readable,
//! exportable, and reachable by erasure.

use agent_ledger::Block;
use agent_ledger::agency::{DateMarker, LeafKind, Quote, Text, ToolCall, ToolError, ToolResult};

use crate::commands::COMPACT_KEPT_MESSAGES;
use crate::kind::CHAT_MESSAGE_KIND;
use crate::note::{CONTEXT_NOTE_KIND, ContextNote, NoteTopic};
use crate::tools::palette::TOOL_PALETTE_KIND;

/// Which trigger asked for the compact. The two differ in exactly one
/// reading: a command's own row must not be what pushes a session over the
/// kept bound — it arrived to ask for the compact — so the nothing-to-cut
/// count reads only the rows OLDER than it. The signal has no row of its own
/// and counts the whole readable set.
///
/// The command trigger CARRIES the invoking row's block id rather than
/// assuming it is the newest row: ids ascend in ledger order, so "older
/// than the invoking row" is a fact the reading can check, and a caller
/// that appends anything after the command's row cannot silently break the
/// count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactTrigger {
    /// A moderator asked, with the block id their command was recorded as.
    Command {
        /// The invoking command's own block id. Chat rows below it are the
        /// conversation the command found; the row itself and anything
        /// after it are not counted against the kept bound.
        invoking_row: i64,
    },
    /// The framework's forced turn end asked; nothing was typed.
    Signal,
}

/// What one compact would do to one conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompactPlan {
    /// The blocks the fork detaches, in ledger order.
    pub detach: Vec<i64>,
    /// Whether the conversation already holds nothing this operation would
    /// remove: no tool traffic, and no more countable chat rows than the
    /// kept bound. The caller does not fork at all in that case, so the
    /// [`detach`](Self::detach) list is not consulted.
    pub nothing_to_cut: bool,
}

/// Read one conversation's blocks and answer what a compact would do.
///
/// `blocks` is the conversation's ledger in order, exactly as the caller
/// snapshots it BEFORE the fork — which is what keeps the fork's own fresh
/// system prompt structurally out of the sweep instead of relying on a
/// filter to spare it.
pub(crate) fn plan(blocks: &[Block], trigger: CompactTrigger) -> CompactPlan {
    let chat_rows: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| is_chat_row(block))
        .map(|(index, _)| index)
        .collect();
    let carries_tool_traffic = blocks.iter().any(is_tool_traffic);
    // The command's own row is not what makes a session uncompact: it
    // arrived to ask for the compact. Counted by id against the row the
    // trigger names, so the reading holds wherever that row sits.
    let countable = match trigger {
        CompactTrigger::Command { invoking_row } => chat_rows
            .iter()
            .filter(|&&index| blocks[index].id < invoking_row)
            .count(),
        CompactTrigger::Signal => chat_rows.len(),
    };
    let nothing_to_cut = !carries_tool_traffic && countable <= COMPACT_KEPT_MESSAGES;

    let first_kept = chat_rows.len().saturating_sub(COMPACT_KEPT_MESSAGES);
    let oldest_kept_row = chat_rows.get(first_kept).copied();
    let mut kept: Vec<usize> = chat_rows[first_kept..].to_vec();

    if let Some(oldest) = oldest_kept_row {
        // The kept rows keep their own day: every date record among them,
        // and the newest one in front of the oldest kept row, which is the
        // day that row was written under.
        kept.extend(
            blocks
                .iter()
                .enumerate()
                .filter(|(index, block)| *index > oldest && is_date_record(block))
                .map(|(index, _)| index),
        );
        kept.extend(
            blocks[..oldest]
                .iter()
                .enumerate()
                .filter(|(_, block)| is_date_record(block))
                .map(|(index, _)| index)
                .next_back(),
        );
    }
    kept.extend(newest_index(blocks, |block| {
        block.block_type == TOOL_PALETTE_KIND
    }));
    for topic in NoteTopic::ALL {
        kept.extend(newest_index(blocks, |block| {
            note_topic(block) == Some(topic)
        }));
    }

    let detach = blocks
        .iter()
        .enumerate()
        .filter(|(index, _)| !kept.contains(index))
        .map(|(_, block)| block.id)
        .collect();
    CompactPlan {
        detach,
        nothing_to_cut,
    }
}

/// Whether the block carries the conversation itself: a recorded channel
/// message, one of the assistant's own text blocks, or a stored quote of an
/// earlier message. Each kind is named through its own declaration, never a
/// literal here.
fn is_chat_row(block: &Block) -> bool {
    let stored = block.block_type.as_str();
    stored == CHAT_MESSAGE_KIND || Text::KINDS.contains(&stored) || Quote::KINDS.contains(&stored)
}

/// Whether the block is tool traffic: a call, its result, or its error, and
/// nothing else.
fn is_tool_traffic(block: &Block) -> bool {
    let stored = block.block_type.as_str();
    ToolCall::KINDS.contains(&stored)
        || ToolResult::KINDS.contains(&stored)
        || ToolError::KINDS.contains(&stored)
}

/// Whether the block is the ledger's own calendar entry.
fn is_date_record(block: &Block) -> bool {
    DateMarker::KINDS.contains(&block.block_type.as_str())
}

/// The topic a context note carries, `None` for every other block and for a
/// note row whose topic falls outside the closed vocabulary — such a row
/// states nothing to the model, so keeping it would keep nothing.
fn note_topic(block: &Block) -> Option<NoteTopic> {
    if block.block_type != CONTEXT_NOTE_KIND {
        return None;
    }
    ContextNote::parse(block).topic
}

/// The index of the newest block the predicate accepts.
fn newest_index(blocks: &[Block], accepts: impl Fn(&Block) -> bool) -> Option<usize> {
    blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| accepts(block))
        .map(|(index, _)| index)
        .next_back()
}

#[cfg(test)]
mod tests {
    use agent_ledger::Role;
    use agent_ledger::agency::{Status, SystemPrompt};
    use serde_json::{Value, json};

    use super::*;
    use crate::note::{COLUMN_TEXT, COLUMN_TOPIC};

    /// The kept bound as a ledger id, for the fixtures that count rows.
    fn bound() -> i64 {
        i64::try_from(COMPACT_KEPT_MESSAGES).expect("the kept bound fits a ledger id")
    }

    /// One block of the given kind, its id its position in the fixture.
    fn block(id: i64, kind: &str) -> Block {
        Block {
            id,
            role: None,
            block_type: kind.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields: serde_json::Map::new(),
        }
    }

    /// One context note of the given topic.
    fn note(id: i64, topic: NoteTopic) -> Block {
        let mut block = block(id, CONTEXT_NOTE_KIND);
        block
            .fields
            .insert(COLUMN_TOPIC.into(), json!(topic.as_str()));
        block
            .fields
            .insert(COLUMN_TEXT.into(), Value::String("the text".into()));
        block
    }

    /// One of the assistant's own answers.
    fn answer(id: i64) -> Block {
        let mut block = block(id, Text::KINDS[0]);
        block.role = Some(Role::Assistant);
        block
    }

    /// The ids the plan keeps: every id in the fixture that is not detached.
    fn kept(blocks: &[Block], plan: &CompactPlan) -> Vec<i64> {
        blocks
            .iter()
            .map(|block| block.id)
            .filter(|id| !plan.detach.contains(id))
            .collect()
    }

    /// A conversation carrying tool traffic and more chat than the bound:
    /// the tool rows go, the trailing bound of chat rows stays, and the
    /// palette and both notes ride across.
    #[test]
    fn the_fork_keeps_the_trailing_chat_the_palette_and_the_newest_notes() {
        let mut blocks = vec![
            block(1, SystemPrompt::KINDS[0]),
            block(2, TOOL_PALETTE_KIND),
            note(3, NoteTopic::Title),
            note(4, NoteTopic::Rules),
            block(5, DateMarker::KINDS[0]),
        ];
        let mut next = 6;
        for _ in 0..30 {
            blocks.push(block(next, CHAT_MESSAGE_KIND));
            next += 1;
            blocks.push(block(next, ToolCall::KINDS[0]));
            next += 1;
            blocks.push(block(next, ToolResult::KINDS[0]));
            next += 1;
            blocks.push(answer(next));
            next += 1;
        }
        let planned = plan(
            &blocks,
            CompactTrigger::Command {
                invoking_row: next - 1,
            },
        );
        assert!(!planned.nothing_to_cut, "a tool flood is something to cut");
        let kept = kept(&blocks, &planned);
        let kept_blocks: Vec<&Block> = blocks
            .iter()
            .filter(|block| kept.contains(&block.id))
            .collect();
        assert!(
            !kept_blocks.iter().any(|block| is_tool_traffic(block)),
            "no tool traffic survives a compact"
        );
        assert_eq!(
            kept_blocks
                .iter()
                .filter(|block| is_chat_row(block))
                .count(),
            COMPACT_KEPT_MESSAGES,
            "exactly the kept bound of chat rows survives"
        );
        assert!(kept.contains(&2), "the palette rides across");
        assert!(kept.contains(&3), "the title note rides across");
        assert!(kept.contains(&4), "the rules note rides across");
        assert!(
            kept.contains(&5),
            "the newest date record before the oldest kept row rides across"
        );
        assert!(
            !kept.contains(&1),
            "the inherited system prompt is detached; the fork records the current one"
        );
    }

    /// The calendar around the kept rows, both halves of it: a date record
    /// standing AMONG the kept tail rides across, and in front of the
    /// oldest kept row only the newest one does — the day that row was
    /// written under. Every older marker is cut with the rows it dated.
    #[test]
    fn the_kept_rows_keep_the_calendar_among_them_and_the_day_they_opened_on() {
        let mut blocks = vec![
            block(1, DateMarker::KINDS[0]),
            block(2, CHAT_MESSAGE_KIND),
            block(3, CHAT_MESSAGE_KIND),
            block(4, DateMarker::KINDS[0]),
            block(5, CHAT_MESSAGE_KIND),
            block(6, CHAT_MESSAGE_KIND),
            block(7, CHAT_MESSAGE_KIND),
        ];
        // The kept bound of rows, split by the marker that opens the next
        // day among them: the oldest kept row leads the first half.
        let opening_half = COMPACT_KEPT_MESSAGES / 2;
        let mut next = 8;
        for _ in 0..opening_half {
            blocks.push(block(next, CHAT_MESSAGE_KIND));
            next += 1;
        }
        let marker_among_the_kept = next;
        blocks.push(block(marker_among_the_kept, DateMarker::KINDS[0]));
        next += 1;
        for _ in 0..COMPACT_KEPT_MESSAGES - opening_half {
            blocks.push(block(next, CHAT_MESSAGE_KIND));
            next += 1;
        }

        let planned = plan(&blocks, CompactTrigger::Signal);
        assert!(
            !planned.nothing_to_cut,
            "five rows past the bound is something to cut"
        );
        let kept = kept(&blocks, &planned);
        assert!(
            kept.contains(&marker_among_the_kept),
            "the date record standing among the kept rows rides across"
        );
        assert!(
            kept.contains(&4),
            "the newest date record before the oldest kept row rides across"
        );
        assert_eq!(
            planned.detach,
            vec![1, 2, 3, 5, 6, 7],
            "the superseded marker goes with the rows it dated"
        );
        assert_eq!(
            kept.iter()
                .filter(|id| blocks
                    .iter()
                    .any(|block| block.id == **id && is_chat_row(block)))
                .count(),
            COMPACT_KEPT_MESSAGES,
            "exactly the kept bound of chat rows survives"
        );
    }

    /// Quotes count as chat rows, on both readings: they fill the kept
    /// bound and they count toward the nothing-to-cut check.
    #[test]
    fn a_quote_counts_as_a_chat_row() {
        let mut blocks: Vec<Block> = (1..=bound()).map(|id| block(id, Quote::KINDS[0])).collect();
        assert!(
            plan(&blocks, CompactTrigger::Signal).nothing_to_cut,
            "the bound of quotes is already compact"
        );
        blocks.push(block(bound() + 1, Quote::KINDS[0]));
        let planned = plan(&blocks, CompactTrigger::Signal);
        assert!(
            !planned.nothing_to_cut,
            "one quote past the bound is something to cut"
        );
        assert_eq!(planned.detach, vec![1], "the oldest quote is the one cut");
    }

    /// Only the NEWEST palette and the newest note of each topic survive;
    /// the superseded ones ride the cut side.
    #[test]
    fn only_the_newest_palette_and_note_of_each_topic_survive() {
        let blocks = vec![
            block(1, TOOL_PALETTE_KIND),
            note(2, NoteTopic::Title),
            note(3, NoteTopic::Title),
            note(4, NoteTopic::Rules),
            block(5, TOOL_PALETTE_KIND),
            block(6, ToolError::KINDS[0]),
        ];
        assert_eq!(plan(&blocks, CompactTrigger::Signal).detach, vec![1, 2, 6]);
    }

    /// The command's own row does not push a session over the bound: a
    /// conversation holding the bound plus the invoking row is already
    /// compact, while the same conversation read from the signal path is
    /// one row past it.
    #[test]
    fn the_invoking_row_counts_inside_the_bound() {
        let blocks: Vec<Block> = (1..=bound() + 1)
            .map(|id| block(id, CHAT_MESSAGE_KIND))
            .collect();
        assert!(
            plan(
                &blocks,
                CompactTrigger::Command {
                    invoking_row: bound() + 1
                }
            )
            .nothing_to_cut,
            "the bound plus the command's own row is already compact"
        );
        assert!(
            !plan(&blocks, CompactTrigger::Signal).nothing_to_cut,
            "the same rows with no command among them are one past the bound"
        );
    }

    /// The count is rows OLDER than the row the trigger names, not the
    /// tail minus one: the same ledger read against the newest row and
    /// against the one below it answers differently, and neither answer
    /// depends on where the invoking row happens to sit.
    #[test]
    fn the_count_reads_the_rows_older_than_the_named_row() {
        let blocks: Vec<Block> = (1..=bound() + 2)
            .map(|id| block(id, CHAT_MESSAGE_KIND))
            .collect();
        assert!(
            !plan(
                &blocks,
                CompactTrigger::Command {
                    invoking_row: bound() + 2
                }
            )
            .nothing_to_cut,
            "the bound plus one below the newest row is something to cut"
        );
        assert!(
            plan(
                &blocks,
                CompactTrigger::Command {
                    invoking_row: bound() + 1
                }
            )
            .nothing_to_cut,
            "read against the row below the tail, exactly the bound is older"
        );
    }

    /// Tool traffic alone makes a short conversation worth compacting, and
    /// an empty conversation is already compact.
    #[test]
    fn tool_traffic_alone_is_something_to_cut() {
        assert!(plan(&[], CompactTrigger::Signal).nothing_to_cut);
        let blocks = vec![block(1, CHAT_MESSAGE_KIND), block(2, ToolCall::KINDS[0])];
        let planned = plan(&blocks, CompactTrigger::Command { invoking_row: 1 });
        assert!(
            !planned.nothing_to_cut,
            "one stored call is something to cut"
        );
        assert_eq!(planned.detach, vec![2]);
    }

    /// Everything the fork does not keep, named: the notice of a join, a
    /// delivery record, a filed report, a placed reaction, a status row and
    /// a superseded prompt all ride the cut side.
    #[test]
    fn the_cut_side_holds_every_kind_the_fork_does_not_keep() {
        let blocks = vec![
            block(1, crate::join::JOIN_NOTICE_KIND),
            block(2, crate::delivery::DELIVERED_KIND),
            block(3, crate::tools::report::REPORT_KIND),
            block(4, crate::tools::mark::MESSAGE_MARK_KIND),
            block(5, Status::KINDS[0]),
            block(6, SystemPrompt::KINDS[0]),
            block(7, ToolCall::KINDS[0]),
            block(8, CHAT_MESSAGE_KIND),
        ];
        let planned = plan(&blocks, CompactTrigger::Signal);
        assert_eq!(planned.detach, vec![1, 2, 3, 4, 5, 6, 7]);
        assert!(!planned.nothing_to_cut);
    }
}
