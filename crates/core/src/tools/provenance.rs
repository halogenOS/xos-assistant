//! The turn's provenance reading: the anchor gate of decision 0043.
//!
//! The rule of 0043 is the intent — a tool call is admitted at the MINIMUM
//! authority over the turn's summoners — and the framework's dispatch
//! anchor is its mechanism: every block a turn writes carries the id of
//! the summoning frontier, inherited across continuation rounds, so the
//! call block names its summoner in round one and in round ten alike. The
//! reading is computed over the ledger vector the admission wrapper
//! already loads for its palette check — no store round-trips of its own,
//! so the gate adds no failure surface beyond that one load. Block ids
//! are monotonic within a store, and the loaded vector is one
//! conversation's own blocks, so the ids bound the interval exactly.
//!
//! The folded set is the turn's CO-SUMMONERS, per 0043's refinements
//! (both 2026-08-22):
//!
//! - the summoner endpoint is the debt ORIGIN SET: from the anchor, the
//!   chain of answer-due chat messages at or before it is walked
//!   backwards in the loaded vector, and the fold takes the sender
//!   authorities of the chain's own-debt-takers — addressed, not limited
//!   ([`ChatMessage::own_debt_taken`]). A pure propagator — a line whose
//!   answer-due stamp only carries someone else's debt forward — sits in
//!   the chain but casts no vote, so a bystander's line that happened to
//!   become the dispatch frontier never speaks its carried min-fold as if
//!   the bystander had summoned the turn. The stamp's own
//!   min-fold stays what it is for ANSWERING (0021/0036 unchanged); this
//!   gate reads the debt's origins instead of its carrier. The walk is
//!   marker-aware (2026-08-23): a stored turn-closure marker means the
//!   turn it names ended over an UNANSWERED outcome, so a message owed
//!   behind that marker still owes — the walk reads through the marker,
//!   through the rest of a turn's machinery, through a dead turn's own
//!   narration and reasoning, and through every kind it does not name,
//!   and ends only at what answered or never owed: a completed answer,
//!   or a chat message owing nothing. The end is the exception and the
//!   read-through is the default, because the fold is a minimum: a chain
//!   cut short can only read higher ([`chain_step`] carries the verified
//!   escalation that pinned this).
//! - a chat message recorded between the anchor and the call joins the
//!   fold only when it opened a debt of its own — the same predicate —
//!   and contributes its sender's stored authority. The turn answers an
//!   absorbed addressed message, so that message co-summons it;
//!   unaddressed chatter and a line the budgets refused contribute
//!   nothing, in the span exactly as before the summons.
//!
//! Every unreadable edge folds downward to [`FLOOR`]: a null anchor (the
//! out-of-band shape), a call block or anchor missing from the vector, a
//! non-message frontier, a chain that holds no own-debt-taker at all, a
//! taker whose stored authority does not parse. The minimum cannot
//! escalate; over-declining is the accepted cost, stated in 0043 — a
//! declined tool is a degraded answer, an escalated tool is a broken
//! access model.
//!
//! [`ChatMessage::own_debt_taken`]: crate::kind::ChatMessage::own_debt_taken

use agent_ledger::agency::Status;
use agent_ledger::{Block, BlockKind, FromBlock};

use crate::kind::{AssistantKind, ChatMessage, FrameworkKind};
use crate::message::Authority;

/// The floor every unreadable shape folds to: the lowest authority, so a
/// missing fact can only ever decline more, never admit more.
pub const FLOOR: Authority = Authority::Member;

/// Read the provenance of the turn behind one tool call, over the
/// conversation's loaded ledger: the minimum authority over the anchor's
/// debt origin set and the co-summoners absorbed between the anchor and
/// the call. Total on purpose — every absence is a documented downward
/// fold, never an error, so the gate's only fallible step stays the
/// ledger load its caller already performs. Bounded by the vector it is
/// handed: the walk visits each block at most once and never touches the
/// store.
#[must_use]
pub fn turn_reading(ledger: &[Block], call_block_id: i64) -> Authority {
    let Some(call) = ledger.iter().find(|block| block.id == call_block_id) else {
        return FLOOR;
    };
    let Some(anchor) = call.dispatch_anchor else {
        return FLOOR;
    };
    let Some(anchor_index) = ledger.iter().position(|block| block.id == anchor) else {
        return FLOOR;
    };
    let origin = origin_reading(ledger, anchor_index);
    let span = ledger
        .iter()
        .filter(|block| block.id > anchor && block.id < call_block_id)
        .filter_map(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => Some(message),
            AssistantKind::Core(_)
            | AssistantKind::ToolPalette(_)
            | AssistantKind::ContextNote(_)
            | AssistantKind::Report(_) => None,
        });
    fold(origin, span)
}

/// The turn's co-summoners as loaded rows, newest first: the span's
/// own-debt-takers between the anchor and the call, then the origin set's
/// takers in the anchor's chain — the one walk behind the privacy tool's
/// principal resolution (2026-08-23, the privacy-self-service unit), the
/// disclosure fold, and the report tool's target validation (2026-08-24,
/// the autonomous-moderation unit: a named origin must belong to one of
/// these rows, so the model can aim a report only at a message it is
/// assessing this turn). Empty for every unloadable shape — a
/// null anchor, a call or anchor missing from the vector, a non-message
/// frontier, a chain of pure propagators — each one more absence folded to
/// the refusing side, exactly as the reading folds them downward.
pub(crate) fn co_summoners(ledger: &[Block], call_block_id: i64) -> Vec<ChatMessage> {
    let Some(call) = ledger.iter().find(|block| block.id == call_block_id) else {
        return Vec::new();
    };
    let Some(anchor) = call.dispatch_anchor else {
        return Vec::new();
    };
    let Some(anchor_index) = ledger.iter().position(|block| block.id == anchor) else {
        return Vec::new();
    };
    let span = ledger
        .iter()
        .rev()
        .filter(|block| block.id > anchor && block.id < call_block_id)
        .filter_map(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => Some(message),
            _ => None,
        })
        .filter(ChatMessage::own_debt_taken);
    // The chain exists only behind a chat-message frontier, the same
    // precondition as [`origin_reading`]: machinery is read through only
    // BEHIND a real summons, never in place of one.
    let chain = matches!(
        AssistantKind::from_block(&ledger[anchor_index]),
        AssistantKind::ChatMessage(_)
    )
    .then(|| {
        ledger[..=anchor_index]
            .iter()
            .rev()
            .map(|block| (block, chain_step(block, ledger)))
            .take_while(|(_, step)| !matches!(step, ChainStep::Ends))
            .filter(|(_, step)| matches!(step, ChainStep::Votes(_)))
            .filter_map(|(block, _)| match AssistantKind::from_block(block) {
                AssistantKind::ChatMessage(message) => Some(message),
                _ => None,
            })
    })
    .into_iter()
    .flatten();
    span.chain(chain).collect()
}

/// The anchor's contribution: the minimum sender authority over the debt
/// origin set — the own-debt-takers in the chain of answer-due chat
/// messages ending at the anchor, the block at `anchor_index` (0043,
/// refined 2026-08-22; marker-aware 2026-08-23). The chain ends at the
/// first block backwards that answered a debt or never owed one — a
/// completed answer, or a chat message owing nothing — because behind
/// that point nothing contributed to summoning this turn. What lies
/// BETWEEN chain members without ending the chain is what answered
/// nothing: a dead turn's leavings, the machinery of the rounds, and any
/// kind the walk does not name, each read through by [`chain_step`]. A
/// propagator inside the chain extends
/// it and votes nothing; a chain with no taker — the non-message frontier
/// of the out-of-band shape included — reads [`FLOOR`], one more absence
/// folded downward. The whole loaded vector rides along because a dead
/// turn's narration is recognized by a turn-closure marker that sits
/// LATER in it, on the far side of the anchor included.
///
/// The frontier itself must be a chat message before any chain exists: a
/// non-message frontier contributes the floor (0043), so machinery is
/// read through only BEHIND a real summons, never in place of one.
fn origin_reading(ledger: &[Block], anchor_index: usize) -> Authority {
    if !matches!(
        AssistantKind::from_block(&ledger[anchor_index]),
        AssistantKind::ChatMessage(_)
    ) {
        return FLOOR;
    }
    ledger[..=anchor_index]
        .iter()
        .rev()
        .map(|block| chain_step(block, ledger))
        .take_while(|step| !matches!(step, ChainStep::Ends))
        .filter_map(|step| match step {
            ChainStep::Votes(authority) => Some(authority),
            ChainStep::Extends | ChainStep::Ends => None,
        })
        .min()
        .unwrap_or(FLOOR)
}

/// One backward step's verdict on a block in the debt chain walk.
enum ChainStep {
    /// An own-debt-taker: the chain continues and this sender's stored
    /// authority joins the fold.
    Votes(Authority),
    /// The walk reads through this block: a propagator's line, a turn's
    /// machinery, a dead turn's narration or reasoning, or a kind the
    /// walk does not name — nothing here answered the debt, so the chain
    /// continues silently.
    Extends,
    /// The chain ends here: this block answered a debt or never owed one,
    /// so nothing behind it contributed to summoning this turn.
    Ends,
}

/// Classify one block for the backward walk (0043, marker-aware
/// 2026-08-23; the edge inverted the same day, after the verified
/// escalation). An answer-due chat message is the chain itself — a taker
/// votes, a propagator extends. The chain ends ONLY at what answered a
/// debt or never owed one: a completed answer — a text no turn-closure
/// marker disowns — or a chat message owing nothing. EVERYTHING else
/// extends silently: a turn's machinery, a dead turn's narration and its
/// reasoning, and every kind this walk does not name — anchored on a
/// disowned turn or a live one, anchored at all or not. Extending is the
/// judged bound for the unnamed and the anchor-less alike, stated
/// deliberately: this walk feeds a MINIMUM fold, so ending the chain
/// early can only RAISE the reading — the first cut defaulted unnamed
/// kinds to the end, and a dead turn's thinking block, an ordinary
/// reasoning-model product, cut the owed member out of the fold and read
/// the next turn as admin — while reading one block further can only add
/// a voter to a minimum, the over-declining direction 0043 accepts.
// The report arm restates the catch-all on purpose — the explicitness is
// the point, stated in the arm's own comment.
#[allow(clippy::match_same_arms)]
fn chain_step(block: &Block, ledger: &[Block]) -> ChainStep {
    match AssistantKind::from_block(block) {
        AssistantKind::ChatMessage(message) if message.answer_due == Some(true) => {
            if message.own_debt_taken() {
                ChainStep::Votes(message.authority.unwrap_or(FLOOR))
            } else {
                ChainStep::Extends
            }
        }
        AssistantKind::ChatMessage(_) => ChainStep::Ends,
        AssistantKind::Core(FrameworkKind(BlockKind::Text(_))) if !turn_died(block, ledger) => {
            ChainStep::Ends
        }
        // The report block extends explicitly, not by the default (decided
        // 2026-08-23): the kind is written INTO a live turn's window by the
        // report tool itself, so its classification decides admission on the
        // very turn that wrote it — a report can never answer a debt, and
        // naming it here keeps that judgment visible beside the rule
        // instead of buried in the catch-all.
        AssistantKind::Report(_) => ChainStep::Extends,
        _ => ChainStep::Extends,
    }
}

/// Whether a stored turn-closure marker later in the loaded vector
/// declares this narration's turn dead: a marker carrying the same
/// dispatch anchor at a higher id. A narration without an anchor is never
/// matched — an out-of-band text belongs to no turn that could have died
/// — and reads as a completed answer, the downward fold.
fn turn_died(narration: &Block, ledger: &[Block]) -> bool {
    narration.dispatch_anchor.is_some_and(|anchor| {
        ledger.iter().any(|later| {
            later.id > narration.id
                && later.dispatch_anchor == Some(anchor)
                && turn_end_marker(later)
        })
    })
}

/// Whether a block is a stored turn-closure marker: a status row carrying
/// one of the exact two machine keys the framework's close writes —
/// nothing broader, so an interrupt's status or a consumer status never
/// declares a turn dead.
fn turn_end_marker(block: &Block) -> bool {
    matches!(
        AssistantKind::from_block(block),
        AssistantKind::Core(FrameworkKind(BlockKind::Status(status)))
            if status.status == Status::TURN_ENDED_CLOSED
                || status.status == Status::TURN_ENDED_ERRORED
    )
}

/// The minimum over the origin reading and every co-summoner in the span:
/// a message that opened a debt of its own lowers the reading by its
/// sender's stored authority, an unreadable authority on such a message
/// folds to [`FLOOR`], and every other message contributes nothing. The
/// origin always contributes, so the span can lower the reading but never
/// raise it.
fn fold(origin: Authority, span: impl Iterator<Item = ChatMessage>) -> Authority {
    span.filter(ChatMessage::own_debt_taken)
        .map(|message| message.authority.unwrap_or(FLOOR))
        .fold(origin, Ord::min)
}

#[cfg(test)]
mod tests {
    use agent_ledger::store::ToolCallInsert;
    use agent_ledger::{Role, Store};

    use super::*;
    use crate::kind::{LimitedBy, TailDebt};

    /// One span message with the given stamp facts — the co-summoner
    /// predicate's three inputs, everything else a fixed well-formed row.
    fn line(
        addressed: bool,
        limited: Option<LimitedBy>,
        authority: Option<Authority>,
    ) -> ChatMessage {
        ChatMessage {
            role: Some(Role::User),
            text: Some("a recorded line".into()),
            principal_id: Some(1),
            authority,
            speaker: None,
            origin: None,
            sent_at: Some("2026-08-22T00:00:00Z".into()),
            addressed: Some(addressed),
            literal_addressed: Some(addressed),
            answer_due: Some(addressed && limited.is_none()),
            limited,
            debt_authority: None,
            reply_target: None,
            reply_to_assistant: None,
        }
    }

    #[test]
    fn the_fold_takes_the_minimum_over_the_co_summoners() {
        assert_eq!(
            fold(Authority::Admin, std::iter::empty()),
            Authority::Admin,
            "a clean interval reads the origin's own contribution"
        );
        assert_eq!(
            fold(
                Authority::Admin,
                [line(true, None, Some(Authority::Member))].into_iter()
            ),
            Authority::Member,
            "an absorbed addressed member co-summons and lowers an admin summons"
        );
        assert_eq!(
            fold(
                FLOOR,
                [line(true, None, Some(Authority::Admin))].into_iter()
            ),
            FLOOR,
            "a floored summoner is never raised by the span"
        );
        assert_eq!(
            fold(Authority::Admin, [line(true, None, None)].into_iter()),
            FLOOR,
            "a co-summoner with an unreadable stored authority folds to the floor"
        );
    }

    #[test]
    fn a_message_outside_the_opened_debt_predicate_contributes_nothing() {
        assert_eq!(
            fold(
                Authority::Admin,
                [
                    line(false, None, Some(Authority::Member)),
                    line(false, None, Some(Authority::Member)),
                ]
                .into_iter()
            ),
            Authority::Admin,
            "unaddressed bystander lines are not a veto"
        );
        assert_eq!(
            fold(
                Authority::Admin,
                [
                    line(true, Some(LimitedBy::Principal), Some(Authority::Member)),
                    line(true, Some(LimitedBy::Channel), Some(Authority::Member)),
                ]
                .into_iter()
            ),
            Authority::Admin,
            "a line the budgets refused opened no debt and does not veto"
        );
    }

    /// One stored chat message as a loaded block, at the given id, its
    /// stamp composed by the production rule from the row's own facts and
    /// the tail it was written behind — `None` for a row behind no owed
    /// debt, `Some` for one written onto an owing tail.
    fn chat_block(id: i64, authority: Authority, addressed: bool, tail: Option<TailDebt>) -> Block {
        Block {
            id,
            role: Some(Role::User),
            block_type: crate::kind::CHAT_MESSAGE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields: ChatMessage::stored_fields(
                "a recorded line",
                crate::kind::RecordedSender {
                    principal_id: 1,
                    authority,
                    speaker: None,
                },
                None,
                None,
                "2026-08-22T00:00:00Z",
                crate::kind::Stamp::compose(
                    crate::kind::Summons {
                        summoned: addressed,
                        literal_addressed: addressed,
                    },
                    authority,
                    None,
                    tail,
                ),
            ),
        }
    }

    /// One stored chat message under an exact origin — the [`chat_block`]
    /// shape plus the platform id the report tool's validation matches.
    fn origin_chat_block(id: i64, authority: Authority, addressed: bool, origin: &str) -> Block {
        Block {
            id,
            role: Some(Role::User),
            block_type: crate::kind::CHAT_MESSAGE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields: ChatMessage::stored_fields(
                "a recorded line",
                crate::kind::RecordedSender {
                    principal_id: 1,
                    authority,
                    speaker: None,
                },
                Some(origin),
                None,
                "2026-08-22T00:00:00Z",
                crate::kind::Stamp::compose(
                    crate::kind::Summons {
                        summoned: addressed,
                        literal_addressed: addressed,
                    },
                    authority,
                    None,
                    None,
                ),
            ),
        }
    }

    /// The co-summoner set the validators read, pinned directly: the
    /// span's own-debt-takers newest first, then the anchor chain's — and
    /// never the bystander, whose unaddressed line co-summons nothing.
    /// This set is the report tool's whole aiming bound: an origin outside
    /// it names no reportable message, however real the message is.
    #[test]
    fn the_co_summoner_set_holds_the_takers_newest_first_and_no_bystander() {
        let ledger = vec![
            origin_chat_block(1, Authority::Member, false, "origin-bystander"),
            origin_chat_block(2, Authority::Admin, true, "origin-anchor"),
            origin_chat_block(3, Authority::Member, true, "origin-absorbed"),
            call_block(5, Some(2)),
        ];
        let origins: Vec<Option<String>> = co_summoners(&ledger, 5)
            .into_iter()
            .map(|message| message.origin)
            .collect();
        assert_eq!(
            origins,
            vec![
                Some("origin-absorbed".to_owned()),
                Some("origin-anchor".to_owned())
            ],
            "the absorbed taker reads first, the anchor's chain second, \
             and the bystander's line is in no turn's assessment set"
        );

        assert!(
            co_summoners(&ledger, 999).is_empty(),
            "a call the vector does not hold folds to the empty set"
        );
    }

    /// The owing tail an admin's taken debt hands the next write.
    fn admin_tail() -> TailDebt {
        TailDebt {
            authority: Some(Authority::Admin),
        }
    }

    /// One loaded call block anchored on the given id.
    fn call_block(id: i64, anchor: Option<i64>) -> Block {
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "tool_call".into(),
            created_at: String::new(),
            dispatch_anchor: anchor,
            fields: serde_json::Map::new(),
        }
    }

    /// One stored turn-closure marker with the given machine key, anchored
    /// on the summons of the turn it declares dead — the framework's close
    /// writes it exactly so.
    fn marker_block(id: i64, anchor: i64, key: &str) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("status".into(), serde_json::json!(key));
        Block {
            id,
            role: None,
            block_type: "status".into(),
            created_at: String::new(),
            dispatch_anchor: Some(anchor),
            fields,
        }
    }

    /// One assistant narration block, carrying its turn's anchor.
    fn text_block(id: i64, anchor: i64) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("content".into(), serde_json::json!("a recorded answer"));
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "text".into(),
            created_at: String::new(),
            dispatch_anchor: Some(anchor),
            fields,
        }
    }

    /// One tool outcome block from a turn's machinery, anchored on its
    /// summons.
    fn tool_error_block(id: i64, anchor: i64) -> Block {
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "tool_error".into(),
            created_at: String::new(),
            dispatch_anchor: Some(anchor),
            fields: serde_json::Map::new(),
        }
    }

    /// The interval is strict on both ends: a resting member line before
    /// the debt chain and an addressed member after the call each lie
    /// outside it, while the addressed moderator between the anchor and
    /// the call co-summons and lowers the admin summons. Drop either
    /// bound and one of the outside members folds in — this reading would
    /// answer member.
    #[test]
    fn the_reading_keeps_both_bounds_of_the_interval() {
        let ledger = vec![
            chat_block(1, Authority::Member, false, None),
            chat_block(2, Authority::Admin, true, None),
            chat_block(3, Authority::Moderator, true, None),
            call_block(5, Some(2)),
            chat_block(6, Authority::Member, true, None),
        ];
        assert_eq!(turn_reading(&ledger, 5), Authority::Moderator);
    }

    /// The debt origin set (0043, refined 2026-08-22): a propagating
    /// frontier — the unaddressed line whose stamp only carries the
    /// admin's debt forward, min-folded to member at the write — anchors
    /// the turn, and the reading is the ORIGIN's authority, not the
    /// carrier's fold: the chain walks back to the admin who took the
    /// debt, the propagator casts no vote, and the admin turn admits.
    #[test]
    fn a_propagating_frontier_reads_its_debts_origins() {
        let propagator = chat_block(3, Authority::Member, false, Some(admin_tail()));
        assert_eq!(
            <ChatMessage as agent_ledger::LeafKind>::parse(&propagator).carried_debt_authority(),
            Some(Authority::Member),
            "the premise: the carrier's own min-folded stamp reads member"
        );
        let ledger = vec![
            chat_block(2, Authority::Admin, true, None),
            propagator,
            call_block(5, Some(3)),
        ];
        assert_eq!(turn_reading(&ledger, 5), Authority::Admin);
    }

    /// The chain is contiguous: a resting line between an old taker and
    /// the anchor ends the walk, so the old taker — an author whose debt
    /// this turn does not carry — stays outside the origin set. And a
    /// chain that holds no taker at all reads the floor: a debt with no
    /// readable origin admits nothing above member.
    #[test]
    fn the_chain_breaks_at_a_message_owing_nothing_and_a_takerless_chain_reads_the_floor() {
        let ledger = vec![
            chat_block(1, Authority::Member, true, None),
            chat_block(2, Authority::Moderator, false, None),
            chat_block(3, Authority::Admin, true, None),
            call_block(5, Some(3)),
        ];
        assert_eq!(
            turn_reading(&ledger, 5),
            Authority::Admin,
            "the resting line at 2 ends the chain before the member at 1"
        );

        let orphan = vec![
            chat_block(3, Authority::Admin, false, Some(admin_tail())),
            call_block(5, Some(3)),
        ];
        assert_eq!(
            turn_reading(&orphan, 5),
            FLOOR,
            "a chain of pure propagators holds no origin to read"
        );
    }

    /// The escalation probe of the marker-aware walk (0043, 2026-08-23):
    /// a member's addressed message summoned a turn that died over an
    /// unanswered outcome, so the member still owes behind the stored
    /// turn-closure marker; when an admin's message anchors the next
    /// turn, the walk reads through the marker and the member votes. The
    /// member's text rides in the dispatched request, so a chain that
    /// ended at the marker would answer admin — the exact escalation the
    /// minimum forbids.
    #[test]
    fn a_member_owed_behind_a_turn_end_marker_still_votes() {
        let ledger = vec![
            chat_block(1, Authority::Member, true, None),
            marker_block(2, 1, Status::TURN_ENDED_CLOSED),
            chat_block(3, Authority::Admin, true, None),
            call_block(5, Some(3)),
        ];
        assert_eq!(turn_reading(&ledger, 5), Authority::Member);
    }

    /// A completed answer still bounds the chain: a finished turn's text —
    /// one no turn-closure marker disowns — answered the debt in front of
    /// it, so the member it answered stays out of the fold and the admin's
    /// own summons reads admin. The stamp on the answered member still
    /// says answer-due, because stamps are written once; the narration is
    /// what records that the debt was met.
    #[test]
    fn a_completed_answer_still_bounds_the_chain() {
        let ledger = vec![
            chat_block(1, Authority::Member, true, None),
            text_block(2, 1),
            chat_block(3, Authority::Admin, true, None),
            call_block(5, Some(3)),
        ];
        assert_eq!(turn_reading(&ledger, 5), Authority::Admin);
    }

    /// A dead turn's narration is read through: the text whose anchor a
    /// later turn-closure marker names answered nothing, so the member
    /// owed behind it still votes — the same text without the marker is
    /// the bound of the pin above, and the marker is what tells the two
    /// apart. The dead turn's tool outcome between them is machinery and
    /// extends the chain the same way.
    #[test]
    fn a_dead_turns_narration_is_read_through() {
        let ledger = vec![
            chat_block(1, Authority::Member, true, None),
            text_block(2, 1),
            tool_error_block(3, 1),
            marker_block(4, 1, Status::TURN_ENDED_ERRORED),
            chat_block(5, Authority::Admin, true, None),
            call_block(7, Some(5)),
        ];
        assert_eq!(turn_reading(&ledger, 7), Authority::Member);
    }

    /// One block of an arbitrary stored kind with no fields, anchored where
    /// its turn's dispatch put it — every parse in the chain is total, so an
    /// empty row reads as that kind with absent facts, the leanest shape the
    /// walk can meet.
    fn bare_block(id: i64, kind: &str, anchor: Option<i64>) -> Block {
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: kind.into(),
            created_at: String::new(),
            dispatch_anchor: anchor,
            fields: serde_json::Map::new(),
        }
    }

    /// The escalation probe of the inverted chain edge (0043, refined
    /// 2026-08-23): a reasoning model thinks before it calls, so a dead
    /// turn's window holds a thinking block between the summons and the
    /// tool machinery. A walk that ends the chain on the thinking kind
    /// truncates the minimum-fold in front of the member whose unanswered
    /// summons still owes — and a truncated minimum can only read HIGHER,
    /// so the next turn would answer admin with the member's text riding
    /// in the dispatched request.
    #[test]
    fn a_member_owed_behind_a_dead_turns_thinking_still_votes() {
        let ledger = vec![
            chat_block(1, Authority::Member, true, None),
            bare_block(2, "thinking", Some(1)),
            bare_block(3, "tool_call", Some(1)),
            tool_error_block(4, 1),
            marker_block(5, 1, Status::TURN_ENDED_ERRORED),
            chat_block(6, Authority::Admin, true, None),
            call_block(8, Some(6)),
        ];
        assert_eq!(
            turn_reading(&ledger, 8),
            Authority::Member,
            "the dead turn's thinking breaks the chain — the owed member stops voting"
        );
    }

    /// A reasoning-only dead turn — the model thought and the turn closed
    /// over the unanswered outcome, no narration, no tool round — leaves
    /// exactly one block between the owed member and the marker, and the
    /// walk must read through it the same way.
    #[test]
    fn a_reasoning_only_dead_turn_is_read_through() {
        let ledger = vec![
            chat_block(1, Authority::Member, true, None),
            bare_block(2, "thinking", Some(1)),
            marker_block(3, 1, Status::TURN_ENDED_CLOSED),
            chat_block(4, Authority::Admin, true, None),
            call_block(6, Some(4)),
        ];
        assert_eq!(
            turn_reading(&ledger, 6),
            Authority::Member,
            "the reasoning-only dead turn buries the member's unanswered summons"
        );
    }

    /// The sweep over every stored kind this build knows: the framework's
    /// whole claimed set, the consumer's palette kind, and a kind no build
    /// has stored, each sitting in a dead turn's window. Every one must
    /// read member — the dead turn answered nothing, whatever it wrote —
    /// so no kind, present or future, can truncate the fold and raise it.
    /// The consumer's chat-message kind is the one deliberate absence: a
    /// debt-free chat message ends the chain BY the rule, and a dead turn
    /// cannot write one into its own window.
    #[test]
    fn every_kind_in_a_dead_turns_window_is_read_through() {
        let mut kinds: Vec<&str> = <BlockKind as FromBlock>::CLAIMED_KINDS.to_vec();
        kinds.push(crate::tools::palette::TOOL_PALETTE_KIND);
        kinds.push("a_kind_this_build_has_never_stored");
        for kind in kinds {
            let ledger = vec![
                chat_block(1, Authority::Member, true, None),
                bare_block(2, kind, Some(1)),
                marker_block(3, 1, Status::TURN_ENDED_ERRORED),
                chat_block(4, Authority::Admin, true, None),
                call_block(6, Some(4)),
            ];
            assert_eq!(
                turn_reading(&ledger, 6),
                Authority::Member,
                "the {kind} block in the dead turn's window breaks the chain"
            );
        }
    }

    /// A frontier that is not a chat message contributes the floor itself
    /// (0043): machinery is read through only BEHIND a real summons, so a
    /// call anchored on a turn-closure marker — a shape no framework
    /// dispatch produces — reads member even with an admin's owed message
    /// behind the marker.
    #[test]
    fn a_machinery_frontier_contributes_the_floor() {
        let ledger = vec![
            chat_block(1, Authority::Admin, true, None),
            marker_block(2, 1, Status::TURN_ENDED_CLOSED),
            call_block(5, Some(2)),
        ];
        assert_eq!(turn_reading(&ledger, 5), FLOOR);
    }

    /// The framework's frontier transparency must survive the consumer's
    /// own derived kind enum: the ratchet resolves blocks through
    /// [`AssistantKind`], so a delegation that dropped the framework's
    /// answer would leave the burial fix inert in this product — a
    /// turn-closure marker would cap the frontier again and an absorbed
    /// message behind it would never dispatch. The two marker keys read
    /// transparent through the delegate; an interrupt's status and a chat
    /// message stay opaque.
    #[test]
    fn the_consumers_kind_delegates_frontier_transparency() {
        use agent_ledger::Agency;
        for (name, block, expected) in [
            (
                "a closed marker",
                marker_block(1, 1, Status::TURN_ENDED_CLOSED),
                true,
            ),
            (
                "an errored marker",
                marker_block(2, 1, Status::TURN_ENDED_ERRORED),
                true,
            ),
            ("an interrupt", marker_block(3, 1, "interrupted"), false),
            (
                "a chat message",
                chat_block(4, Authority::Member, true, None),
                false,
            ),
        ] {
            assert_eq!(
                AssistantKind::from_block(&block).frontier_transparent(),
                expected,
                "{name} answers the wrong transparency through the delegate"
            );
        }
    }

    /// A null anchor reads as the floor: a call block written through the
    /// public write surface — the out-of-band shape, recording no anchor —
    /// folds the reading to member, the lowest authority, so an
    /// out-of-band call can never reach an above-member tool. A call id
    /// the vector does not hold folds the same way.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_null_anchor_and_a_missing_call_block_read_as_the_floor() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation(
                "test-provider".into(),
                "test-model".into(),
                "Test Model".into(),
                "test-vendor".into(),
            )
            .await
            .expect("a conversation row");
        let call = store
            .insert_tool_call_block(
                conversation,
                Role::Assistant,
                ToolCallInsert {
                    tool_call_id: "call-out-of-band".into(),
                    name: "admin_probe".into(),
                    input: "{}".into(),
                    interactive: false,
                },
                None,
            )
            .await
            .expect("the call block inserts");
        let ledger = store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let stored = ledger
            .iter()
            .find(|block| block.id == call)
            .expect("the call block loads");
        assert_eq!(
            stored.dispatch_anchor, None,
            "the public write surface records no anchor — the out-of-band shape"
        );
        assert_eq!(turn_reading(&ledger, call), FLOOR);
        assert_eq!(
            turn_reading(&ledger, call + 1000),
            FLOOR,
            "a call id the vector does not hold folds to the floor"
        );
        assert_eq!(
            FLOOR,
            Authority::Member,
            "the floor is the lowest authority"
        );
    }
}
