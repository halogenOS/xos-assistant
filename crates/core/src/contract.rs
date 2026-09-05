//! The contract notice: one system-voiced block recording the moment a
//! conversation crossed from the old speaking contract into this one (unit
//! 55, 2026-09-02).
//!
//! Before this unit the assistant's written answers were relayed to the
//! group. A conversation that ran under that contract holds real answers of
//! hers in its history, and the model reads them back on every later turn.
//! Told nothing, it reads its own past prose as messages that reached
//! nobody — or, worse, keeps writing prose and believing it arrives.
//!
//! Nothing rewrites those answers. They stand exactly as they were sent,
//! because they WERE sent, and an append-only ledger does not edit its own
//! history to make a later rule look older than it is. What is appended
//! instead is one block stating where the line falls: everything above it
//! was posted as it stands, and from here on the written text is private
//! and a message reaches the group only through the two sending tools.
//!
//! It is appended in the same act as the tool-choice DELTA that grants the
//! conversation the two tools — the one moment the conversation's own
//! record changes — and only when that append is a delta whose prior choice
//! LACKED them. A conversation born under this build records its first
//! choice already naming them, has no pre-contract answer to explain, and
//! gets no notice.
//!
//! Under compaction the notice sits after every raw answer it explains, so
//! a cut that keeps such an answer in view keeps the notice with it, and a
//! cut that summarizes the notice summarizes those answers too.
//!
//! One column, holding the sentence as it was written at the append. The
//! notice is a stored FACT about this conversation, so the words it was
//! made with belong on the row: a later edit of the constant changes what
//! new conversations are told and leaves every recorded notice saying what
//! it said.

use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, ContentPart, LeafKind, Projection, Role,
};
use serde_json::{Value, json};

/// The stored type string of the contract-notice kind.
pub const CONTRACT_NOTICE_KIND: &str = "contract_notice";

/// The content table the kind's descriptor owns.
pub const CONTRACT_NOTICE_TABLE: &str = "block_contract_notice";

/// The sentence this notice was recorded with. NOT NULL: a notice that says
/// nothing is not a notice, and the append always carries the wording.
pub const COLUMN_NOTICE: &str = "notice";

/// What the notice says, as this build writes it: where the line falls, and
/// what holds on each side of it.
///
/// Addressed to the model in the second person, like every other system
/// line it reads. It states two facts and no instruction: the answers above
/// it went to the group as they stand, and from here the written text is
/// private. What to DO about that is the system prompt's business, which
/// teaches the contract in full.
pub const CONTRACT_NOTICE: &str = "A note about this conversation: the answers you wrote above this line were posted to \
     the group exactly as they stand. From here on that is no longer so. What you write is \
     your own private notes and reaches nobody, and a message reaches the group only when \
     you send it with the send_message or reply_message tool.";

/// One stored contract notice. The absent sentence is a row the store did
/// not produce; such a row projects nothing rather than a guess at what it
/// once said.
#[derive(Debug, Clone)]
pub struct ContractNotice {
    /// The recorded sentence. `None` only for a row the store did not
    /// produce (the schema stores it NOT NULL).
    pub notice: Option<String>,
}

impl ContractNotice {
    /// The stored shape of one notice, named by the same column
    /// [`LeafKind::parse`] reads back.
    #[must_use]
    pub fn stored_fields(notice: &str) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_NOTICE.into(), json!(notice));
        fields
    }
}

impl LeafKind for ContractNotice {
    const KINDS: &'static [&'static str] = &[CONTRACT_NOTICE_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: CONTRACT_NOTICE_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[CONTRACT_NOTICE_KIND],
        columns: &[Column::new(COLUMN_NOTICE, ColumnType::Text)],
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            notice: block
                .fields
                .get(COLUMN_NOTICE)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    }
}

/// Agency-inert, and frontier-transparent for the tool choice's own reason:
/// the notice is appended beside a choice delta on a conversation's first
/// activity per process, at whatever point its history had reached —
/// including behind an ask nobody has answered yet. That ask still owes its
/// turn.
impl Agency for ContractNotice {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// A system-voiced line, like the join notice's and the context note's: the
/// ledger states a fact about the conversation in its own voice, and
/// providers join system lines instead of overwriting, so the notice never
/// displaces the system prompt. A row the store did not produce is
/// boundary-invisible and contributes nothing.
impl Projection for ContractNotice {
    fn group_role(&self) -> Option<Role> {
        self.notice.as_ref().map(|_| Role::System)
    }

    fn llm_parts(&self) -> Option<Vec<ContentPart>> {
        Some(vec![ContentPart::Text {
            text: self.notice.clone()?,
        }])
    }

    fn llm_text(&self) -> Option<String> {
        self.notice.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: CONTRACT_NOTICE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// The stored sentence round-trips and projects in the system voice,
    /// and the wording is pinned where it is written: it names both sides
    /// of the line and both tools, and instructs nothing.
    #[test]
    fn the_notice_projects_the_recorded_sentence_in_the_system_voice() {
        let notice = ContractNotice::parse(&notice_block(ContractNotice::stored_fields(
            CONTRACT_NOTICE,
        )));
        assert_eq!(notice.group_role(), Some(Role::System));
        assert_eq!(notice.llm_text().as_deref(), Some(CONTRACT_NOTICE));
        assert_eq!(
            notice.llm_parts(),
            Some(vec![ContentPart::Text {
                text: CONTRACT_NOTICE.to_owned()
            }])
        );
        assert_eq!(
            CONTRACT_NOTICE,
            "A note about this conversation: the answers you wrote above this line were \
             posted to the group exactly as they stand. From here on that is no longer so. \
             What you write is your own private notes and reaches nobody, and a message \
             reaches the group only when you send it with the send_message or reply_message \
             tool."
        );
        for named in [crate::tools::send::NAME, crate::tools::reply::NAME] {
            assert!(
                CONTRACT_NOTICE.contains(named),
                "the notice names the tool a message now reaches the group through: {named}"
            );
        }
    }

    /// The kind is inert and transparent: it summons nothing and the
    /// owed-turn frontier reads through it — the notice lands on a
    /// conversation's first activity, which can be behind an ask nobody has
    /// answered.
    #[test]
    fn the_notice_is_inert_and_transparent() {
        let notice = ContractNotice::parse(&notice_block(ContractNotice::stored_fields(
            CONTRACT_NOTICE,
        )));
        assert_eq!(notice.awaiting(), None);
        assert!(notice.frontier_transparent());
        assert!(notice.durable());
    }

    /// A row the store did not produce projects nothing at all, rather than
    /// a guess at what it once said.
    #[test]
    fn a_row_the_store_did_not_produce_projects_nothing() {
        let empty = ContractNotice::parse(&notice_block(serde_json::Map::new()));
        assert_eq!(empty.group_role(), None);
        assert_eq!(empty.llm_text(), None);
        assert_eq!(empty.llm_parts(), None);
    }
}
