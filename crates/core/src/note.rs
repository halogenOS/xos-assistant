//! The context note: the consumer block kind that carries a group's own
//! observed facts — its title and its rules — and the rules contract that
//! feeds the rules topic.
//!
//! A note is a topic and a text, appended by the observation surface only
//! when the observed text differs from the newest stored note of the same
//! topic. It is agency-inert and frontier-transparent: the owed-turn
//! decision reads through it, so a note appended over an unanswered message
//! buries nothing. It projects to the model in the system voice, following
//! the framework's date marker; notes accumulate in stream order and the
//! projection wording makes the newest authoritative. Decided 2026-08-23,
//! with the rejected alternatives in the decision record: rendering rules
//! into the system prompt block, a mutable rules row, and deferring the
//! append until no debt is open.

use agent_ledger::store::{StoreError, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, ContentPart, LeafKind, Projection, Role,
    Store,
};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

use crate::message::ObservedFact;

/// The stored type string of the context-note kind.
pub const CONTEXT_NOTE_KIND: &str = "context_note";

/// The content table the kind's descriptor owns.
pub const CONTEXT_NOTE_TABLE: &str = "block_context_note";

/// The note's topic, in the closed [`NoteTopic`] vocabulary.
pub const COLUMN_TOPIC: &str = "topic";
/// The note's text — the observed fact itself. Group governance prose, not
/// a person's conversation; the erasure boundary is recorded OPEN in its
/// own decision beside the tool-block one.
pub const COLUMN_TEXT: &str = "text";

/// What the model reads ahead of a title note's text. "is now" is the
/// supersession wording: notes accumulate in stream order, and the newest
/// statement of a topic is the authoritative one.
pub const TITLE_NOTE_LEAD: &str = "The group's title is now: ";

/// What the model reads ahead of a rules note's text, under the same
/// supersession wording as the title's.
pub const RULES_NOTE_LEAD: &str = "The group's rules are now:\n";

/// The rules prefix of the operator's contract: a pinned text whose first
/// line is exactly this, followed by a newline, is the group's rules.
/// Case-sensitive, nothing before it; a carriage return before the newline
/// is tolerated.
pub const RULES_PREFIX: &str = "Rules:";

/// The byte bound on a rules text. An over-bound text is refused whole,
/// never truncated — a cut rule is a different rule — and the bound caps
/// the surface whoever holds the group's pin right can write into the
/// system voice.
pub const RULES_TEXT_MAX_BYTES: usize = 4096;

/// The byte bound on a title text — the core's own bound (refined
/// 2026-08-23): the platform's title cap was load-bearing for the
/// system-voice surface, and the core owns what reaches its own system
/// voice. An over-bound title is refused whole with a log line, never
/// truncated.
pub const TITLE_TEXT_MAX_BYTES: usize = 512;

/// A note's topic — a closed vocabulary, one topic per observed group fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteTopic {
    /// The group's title.
    Title,
    /// The group's rules, read from the pinned announcement under the
    /// rules contract.
    Rules,
}

impl NoteTopic {
    /// Every variant, in stored-encoding order — what closes the vocabulary
    /// in the migration's CHECK constraint, so the constraint and this enum
    /// cannot drift apart.
    pub const ALL: [Self; 2] = [Self::Title, Self::Rules];

    /// The stored encoding, a closed vocabulary.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Rules => "rules",
        }
    }

    /// Parse the stored encoding back, `None` for anything outside the
    /// vocabulary.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        match stored {
            "title" => Some(Self::Title),
            "rules" => Some(Self::Rules),
            _ => None,
        }
    }
}

/// What the rules contract reads out of one pinned text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesReading {
    /// The pinned text is the group's rules: the prefix line stripped, the
    /// remainder trimmed.
    Rules(String),
    /// No rules prefix: an ordinary announcement. It is not rules and
    /// supersedes nothing.
    NotRules,
    /// Prefixed, but empty after trimming — refused with a log line at the
    /// caller; an empty rules text is not rules.
    RefusedEmpty,
    /// Prefixed, but past [`RULES_TEXT_MAX_BYTES`] — refused whole with a
    /// log line at the caller, never truncated.
    RefusedOverBound {
        /// How many bytes the refused text carried.
        bytes: usize,
    },
}

/// The rules contract, in one pure reading: a pinned text whose first line
/// is exactly [`RULES_PREFIX`] followed by a newline — case-sensitive, a
/// carriage return before the newline tolerated, nothing before the prefix
/// — is the group's rules; the prefix line is stripped and the trimmed
/// remainder becomes the rules text, bounded by [`RULES_TEXT_MAX_BYTES`].
#[must_use]
pub fn read_rules(pinned: &str) -> RulesReading {
    let Some(after_prefix) = pinned.strip_prefix(RULES_PREFIX) else {
        return RulesReading::NotRules;
    };
    let after_return = after_prefix.strip_prefix('\r').unwrap_or(after_prefix);
    let Some(remainder) = after_return.strip_prefix('\n') else {
        return RulesReading::NotRules;
    };
    let rules = remainder.trim();
    if rules.is_empty() {
        return RulesReading::RefusedEmpty;
    }
    if rules.len() > RULES_TEXT_MAX_BYTES {
        return RulesReading::RefusedOverBound { bytes: rules.len() };
    }
    RulesReading::Rules(rules.to_owned())
}

/// The note one observed fact yields, if any: a topic and the exact text
/// the on-delta rule compares. A title is its own note when non-empty; a
/// pinned announcement is a note only when [`read_rules`] reads it as rules
/// — the contract's refusals are logged here, beside the contract that
/// refuses them, and yield nothing. A membership fact is authorization, not
/// a note, and a join is a block of its own, never a note: a note's text is
/// group governance, beyond every erasure pass (decision 0055), while a
/// join carries a person.
pub(crate) fn note_of(fact: &ObservedFact) -> Option<(NoteTopic, String)> {
    match fact {
        ObservedFact::Title(title) => {
            let title = title.trim();
            if title.is_empty() {
                tracing::debug!("an empty observed title yields no note");
                return None;
            }
            if title.len() > TITLE_TEXT_MAX_BYTES {
                tracing::info!(
                    bytes = title.len(),
                    bound = TITLE_TEXT_MAX_BYTES,
                    "an over-bound observed title is refused whole; no note appended"
                );
                return None;
            }
            Some((NoteTopic::Title, title.to_owned()))
        }
        ObservedFact::PinnedAnnouncement(text) => match read_rules(text) {
            RulesReading::Rules(rules) => Some((NoteTopic::Rules, rules)),
            RulesReading::NotRules => None,
            RulesReading::RefusedEmpty => {
                tracing::info!("a rules pin empty after trimming is refused; no note appended");
                None
            }
            RulesReading::RefusedOverBound { bytes } => {
                tracing::info!(
                    bytes,
                    bound = RULES_TEXT_MAX_BYTES,
                    "an over-bound rules pin is refused whole; no note appended"
                );
                None
            }
        },
        ObservedFact::Added { .. } | ObservedFact::MembersJoined { .. } => None,
    }
}

/// One stored context note. Absences are typed per the kind contract: a
/// topic outside the closed vocabulary or a missing text parses to `None`,
/// and such a row projects nothing and matches no topic read — fail closed,
/// never invented.
#[derive(Debug, Clone)]
pub struct ContextNote {
    /// The note's topic. `None` only for a row the store did not produce.
    pub topic: Option<NoteTopic>,
    /// The observed text. `None` only for a row the store did not produce.
    pub text: Option<String>,
}

impl ContextNote {
    /// The stored shape of one note: the field map the observation append
    /// carries, encoded by the module that decodes it back.
    #[must_use]
    pub fn stored_fields(topic: NoteTopic, text: &str) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TOPIC.into(), json!(topic.as_str()));
        fields.insert(COLUMN_TEXT.into(), json!(text));
        fields
    }

    /// The system line this note projects, `None` for a row the store did
    /// not produce — a malformed note says nothing instead of a fragment.
    fn line(&self) -> Option<String> {
        let text = self.text.as_deref()?;
        Some(match self.topic? {
            NoteTopic::Title => format!("{TITLE_NOTE_LEAD}{text}"),
            NoteTopic::Rules => format!("{RULES_NOTE_LEAD}{text}"),
        })
    }
}

impl LeafKind for ContextNote {
    const KINDS: &'static [&'static str] = &[CONTEXT_NOTE_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: CONTEXT_NOTE_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[CONTEXT_NOTE_KIND],
        columns: &[
            Column::new(COLUMN_TOPIC, ColumnType::Text),
            Column::new(COLUMN_TEXT, ColumnType::Text),
        ],
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            topic: block
                .fields
                .get(COLUMN_TOPIC)
                .and_then(Value::as_str)
                .and_then(NoteTopic::parse),
            text: block
                .fields
                .get(COLUMN_TEXT)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    }
}

/// Agency-inert, and frontier-transparent on purpose: a note is appended by
/// an independent path at an arbitrary moment, so the owed-turn frontier
/// must read through it — a note on top of an unanswered message leaves the
/// turn owed, and the entry point's own owing-tail read walks past context
/// notes exactly (refined 2026-08-23; the framework's other transparent
/// kinds keep their settled-tail meaning there).
impl Agency for ContextNote {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// A system-voiced line, following the framework's date marker: providers
/// join system lines instead of overwriting, so a note never erases the
/// system prompt. A malformed row is boundary-invisible and contributes
/// nothing.
///
/// The rules note is guaranteed in the model's context while one exists
/// (unit 15, 2026-08-24): the projection folds the conversation's whole
/// loaded ledger — no window trims history — and a note is a durable
/// block, so every stored rules note rides every later request the way
/// the system prompt does, the newest one authoritative under the
/// supersession wording. The autonomous moderation assessment rests on
/// exactly this: the model judges against rules it can always see.
impl Projection for ContextNote {
    fn group_role(&self) -> Option<Role> {
        self.line().map(|_| Role::System)
    }

    fn llm_parts(&self) -> Option<Vec<ContentPart>> {
        Some(vec![ContentPart::Text { text: self.line()? }])
    }

    fn llm_text(&self) -> Option<String> {
        self.line()
    }
}

// ─── The bounded hot-path reads ──────────────────────────────────────────
//
// Both reads below join the framework's `blocks` and `conversation_blocks`
// tables by name, like the budget counts do: junction order IS ledger
// order per the framework's own latest-block read, neither fact lives in
// the note's content table, and the coupling is the deliberate, recorded
// one of decision 0032. Each read is one indexed row — never a
// conversation hydration; the observation surface and the ingestion stamp
// sit on the hot path and pay these reads per call.

/// The newest stored note text of one topic in one conversation — the read
/// half of the on-delta rule, serialized under the stamp lock by the
/// observation surface. One bounded row: the newest matching content row by
/// junction order. The schema's NOT NULL and CHECK constraints keep every
/// stored row whole, so what this answers is the newest note or nothing.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn newest_text(
    store: &Store,
    conversation_id: i64,
    topic: NoteTopic,
) -> Result<Option<String>, StoreError> {
    domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                &format!(
                    "SELECT n.{COLUMN_TEXT} FROM conversation_blocks cb \
                     JOIN {CONTEXT_NOTE_TABLE} n ON n.block_id = cb.block_id \
                     WHERE cb.conversation_id = ?1 AND n.{COLUMN_TOPIC} = ?2 \
                     ORDER BY cb.id DESC LIMIT 1"
                ),
                (conversation_id, topic.as_str()),
                |row| row.get(0),
            )
            .optional()?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: CONTEXT_NOTE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    #[test]
    fn the_stored_fields_round_trip_through_the_parse() {
        let note = ContextNote::parse(&note_block(ContextNote::stored_fields(
            NoteTopic::Rules,
            "Be kind.",
        )));
        assert_eq!(note.topic, Some(NoteTopic::Rules));
        assert_eq!(note.text.as_deref(), Some("Be kind."));
    }

    #[test]
    fn the_topic_vocabulary_round_trips_and_rejects_strangers() {
        for topic in NoteTopic::ALL {
            assert_eq!(NoteTopic::parse(topic.as_str()), Some(topic));
        }
        assert_eq!(NoteTopic::parse("description"), None);
    }

    #[test]
    fn a_note_is_inert_transparent_and_system_voiced() {
        let note = ContextNote::parse(&note_block(ContextNote::stored_fields(
            NoteTopic::Title,
            "The kernel room",
        )));
        assert_eq!(note.awaiting(), None, "a note summons nothing");
        assert!(note.frontier_transparent(), "the frontier reads through it");
        assert!(note.durable(), "a note is a durable ledger row");
        assert_eq!(note.group_role(), Some(Role::System));
        assert_eq!(
            note.llm_text().as_deref(),
            Some("The group's title is now: The kernel room")
        );
    }

    #[test]
    fn the_rules_note_projects_the_newest_authoritative_wording() {
        let note = ContextNote::parse(&note_block(ContextNote::stored_fields(
            NoteTopic::Rules,
            "1. Be kind.\n2. Stay on topic.",
        )));
        assert_eq!(
            note.llm_text().as_deref(),
            Some("The group's rules are now:\n1. Be kind.\n2. Stay on topic.")
        );
    }

    #[test]
    fn a_malformed_note_projects_nothing() {
        let absent = ContextNote::parse(&note_block(serde_json::Map::new()));
        assert_eq!(absent.group_role(), None, "no boundary from a broken row");
        assert_eq!(absent.llm_text(), None);
        assert_eq!(absent.llm_parts(), None);

        let mut stranger = serde_json::Map::new();
        stranger.insert(COLUMN_TOPIC.into(), json!("description"));
        stranger.insert(COLUMN_TEXT.into(), json!("some text"));
        let stranger = ContextNote::parse(&note_block(stranger));
        assert_eq!(stranger.llm_text(), None, "a stranger topic says nothing");
    }

    // ─── The rules contract (AC3, the pure half) ─────────────────────────

    #[test]
    fn a_prefixed_pinned_text_strips_to_its_rules() {
        assert_eq!(
            read_rules("Rules:\n1. Be kind.\n2. Stay on topic."),
            RulesReading::Rules("1. Be kind.\n2. Stay on topic.".into())
        );
    }

    #[test]
    fn the_prefix_is_case_sensitive_and_tolerates_a_carriage_return() {
        assert_eq!(read_rules("rules:\nBe kind."), RulesReading::NotRules);
        assert_eq!(read_rules("RULES:\nBe kind."), RulesReading::NotRules);
        assert_eq!(
            read_rules("Rules:\r\nBe kind."),
            RulesReading::Rules("Be kind.".into())
        );
    }

    #[test]
    fn text_before_the_prefix_or_a_missing_newline_is_not_rules() {
        assert_eq!(
            read_rules("Our Rules:\nBe kind."),
            RulesReading::NotRules,
            "nothing may precede the prefix"
        );
        assert_eq!(read_rules(" Rules:\nBe kind."), RulesReading::NotRules);
        assert_eq!(
            read_rules("Rules:"),
            RulesReading::NotRules,
            "the prefix line must be followed by a newline"
        );
        assert_eq!(read_rules("Rules: be kind"), RulesReading::NotRules);
        assert_eq!(
            read_rules("Welcome to the group!"),
            RulesReading::NotRules,
            "a plain announcement is not rules and supersedes nothing"
        );
    }

    #[test]
    fn an_empty_remainder_is_refused() {
        assert_eq!(read_rules("Rules:\n"), RulesReading::RefusedEmpty);
        assert_eq!(read_rules("Rules:\n   \n\t\n"), RulesReading::RefusedEmpty);
    }

    #[test]
    fn a_fact_yields_its_note_and_a_refused_or_foreign_fact_yields_none() {
        assert_eq!(
            note_of(&ObservedFact::Title("  The kernel room  ".into())),
            Some((NoteTopic::Title, "The kernel room".into())),
            "a title is its own note, trimmed"
        );
        assert_eq!(
            note_of(&ObservedFact::Title("   ".into())),
            None,
            "an empty title yields no note"
        );
        assert_eq!(
            note_of(&ObservedFact::PinnedAnnouncement("Rules:\nBe kind.".into())),
            Some((NoteTopic::Rules, "Be kind.".into())),
            "a rules pin yields the rules note"
        );
        assert_eq!(
            note_of(&ObservedFact::PinnedAnnouncement("Welcome!".into())),
            None,
            "a plain announcement yields no note"
        );
        assert_eq!(
            note_of(&ObservedFact::PinnedAnnouncement("Rules:\n \n".into())),
            None,
            "a refused rules pin yields no note"
        );
        assert_eq!(
            note_of(&ObservedFact::Added { by: None }),
            None,
            "a membership fact is authorization, not a note"
        );
        assert_eq!(
            note_of(&ObservedFact::MembersJoined {
                joiners: Vec::new(),
                origin: "origin-join".into(),
                timestamp: chrono::Utc::now(),
            }),
            None,
            "a join is a block of its own, never a note"
        );
    }

    // ─── The bounded hot-path reader ─────────────────────────────────────

    /// A conversation whose tail is a run of notes: the newest-text reader
    /// answers the newest note per topic from its one bounded query, never
    /// a hydration. The kind-agnostic read behind the whole run lives in
    /// the ledger module, with its own pin.
    #[tokio::test]
    async fn the_bounded_reader_answers_the_newest_note_per_topic() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        assert_eq!(
            newest_text(&store, conversation, NoteTopic::Rules)
                .await
                .expect("the empty read runs"),
            None,
            "an empty conversation holds no note"
        );

        for (topic, text) in [
            (NoteTopic::Title, "The first title"),
            (NoteTopic::Rules, "The first rules"),
            (NoteTopic::Rules, "The newest rules"),
            (NoteTopic::Title, "The newest title"),
        ] {
            store
                .append_consumer_block(
                    conversation,
                    None,
                    CONTEXT_NOTE_KIND,
                    ContextNote::stored_fields(topic, text),
                    None,
                )
                .await
                .expect("the note appends");
        }

        assert_eq!(
            newest_text(&store, conversation, NoteTopic::Rules)
                .await
                .expect("the rules read runs")
                .as_deref(),
            Some("The newest rules")
        );
        assert_eq!(
            newest_text(&store, conversation, NoteTopic::Title)
                .await
                .expect("the title read runs")
                .as_deref(),
            Some("The newest title")
        );
    }

    #[test]
    fn an_over_bound_title_is_refused_whole_never_truncated() {
        let over = "t".repeat(TITLE_TEXT_MAX_BYTES + 1);
        assert_eq!(
            note_of(&ObservedFact::Title(over)),
            None,
            "an over-bound title yields no note"
        );
        // The bound itself still reads as a title.
        let at_bound = "t".repeat(TITLE_TEXT_MAX_BYTES);
        assert_eq!(
            note_of(&ObservedFact::Title(at_bound.clone())),
            Some((NoteTopic::Title, at_bound))
        );
    }

    #[test]
    fn an_over_bound_rules_text_is_refused_whole_never_truncated() {
        let text = "r".repeat(RULES_TEXT_MAX_BYTES + 1);
        assert_eq!(
            read_rules(&format!("Rules:\n{text}")),
            RulesReading::RefusedOverBound {
                bytes: RULES_TEXT_MAX_BYTES + 1
            }
        );
        // The bound itself still reads as rules.
        let at_bound = "r".repeat(RULES_TEXT_MAX_BYTES);
        assert_eq!(
            read_rules(&format!("Rules:\n{at_bound}")),
            RulesReading::Rules(at_bound)
        );
    }
}
