//! The tool palette: the consumer block kind that gates tool admission.
//!
//! The framework offers tool definitions to the model registry-wide and
//! filters nothing per conversation, so which tools a conversation ADMITS is
//! the assistant's own recorded fact: one durable palette block, written at
//! every conversation's creation beside the system prompt, naming the
//! admitted tools. The palette gates admission, not exposure — the model may
//! still be offered a tool the palette will decline, which is why the
//! admission wrapper's decline wording teaches the model not to retry.
//!
//! Fail closed is the whole policy: a conversation without a palette block
//! admits nothing, and so does a palette whose stored list does not parse. A
//! public group is not an operator session — absence of the record must never
//! read as permission.
//!
//! The block projects nothing to the model and awaits nothing: it is a pure
//! record, invisible to projection, consulted only by the admission wrapper.

use agent_ledger::store::{StoreError, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, LeafKind, Projection, Store,
};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

/// The stored type string of the palette kind.
pub const TOOL_PALETTE_KIND: &str = "tool_palette";

/// The content table the kind's descriptor owns.
pub const TOOL_PALETTE_TABLE: &str = "block_tool_palette";

/// The one content column: the admitted tool names as a JSON array of
/// strings. One column, not one row per name, because the palette is
/// written once, whole, and read once, whole — a name-per-row shape would
/// invite partial writes the fail-closed rule has no reading for.
pub const COLUMN_TOOLS: &str = "tools";

/// One conversation's recorded tool admission.
#[derive(Debug, Clone)]
pub struct ToolPalette {
    /// The admitted tool names, or `None` when the stored column is absent
    /// or does not parse as a JSON string array. `None` admits nothing:
    /// [`LeafKind::parse`] is total by the framework's contract, so a
    /// malformed row cannot fail loudly here — it fails closed at the
    /// admission read instead.
    pub tools: Option<Vec<String>>,
}

impl ToolPalette {
    /// Whether the palette admits a tool of this name. A palette that never
    /// parsed admits nothing.
    #[must_use]
    pub fn admits(&self, name: &str) -> bool {
        self.tools
            .as_ref()
            .is_some_and(|tools| tools.iter().any(|tool| tool == name))
    }

    /// The stored shape of one palette block: the field map the creation
    /// write carries, encoded by the same module that decodes it back, so
    /// the encoding cannot split from the parse.
    #[must_use]
    pub fn stored_fields(names: &[String]) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TOOLS.into(), json!(json!(names).to_string()));
        fields
    }
}

impl LeafKind for ToolPalette {
    const KINDS: &'static [&'static str] = &[TOOL_PALETTE_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: TOOL_PALETTE_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[TOOL_PALETTE_KIND],
        columns: &[Column::new(COLUMN_TOOLS, ColumnType::Text)],
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            tools: block
                .fields
                .get(COLUMN_TOOLS)
                .and_then(Value::as_str)
                .and_then(|stored| serde_json::from_str::<Vec<String>>(stored).ok()),
        }
    }
}

/// Awaits nothing, and frontier-transparent on purpose (refined
/// 2026-08-23, with the on-delta supersession): a superseding palette is
/// appended at a conversation's first activity per process, at an
/// arbitrary point in its history, so the owed-turn decision must read
/// through it — a palette appended over an unanswered message buries
/// nothing. At creation the transparency is inert: the palette sits ahead
/// of every message.
impl Agency for ToolPalette {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// Invisible to the model in every mode: the palette names capabilities, and
/// capability lists are for the admission wrapper, not for the prompt.
impl Projection for ToolPalette {}

/// The newest stored palette's raw tool list of one conversation — the
/// read half of the on-delta supersession (decided 2026-08-23), serialized
/// under the stamp lock by the assembly. One bounded row by junction
/// order, like the note reads beside it: the join couples to the
/// framework's table names under decision 0032's recorded reasoning. The
/// outer `None` is a conversation with no palette block at all; the inner
/// `None` is a stored list that does not parse — both are deltas to the
/// registered set, because neither admits what the set names.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn newest_tools(
    store: &Store,
    conversation_id: i64,
) -> Result<Option<Option<Vec<String>>>, StoreError> {
    domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
        let stored: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT p.{COLUMN_TOOLS} FROM conversation_blocks cb \
                     JOIN {TOOL_PALETTE_TABLE} p ON p.block_id = cb.block_id \
                     WHERE cb.conversation_id = ?1 \
                     ORDER BY cb.id DESC LIMIT 1"
                ),
                [conversation_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(stored.map(|stored| serde_json::from_str::<Vec<String>>(&stored).ok()))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: TOOL_PALETTE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    #[test]
    fn the_stored_fields_round_trip_through_the_parse() {
        let names = vec!["lookup_commit".to_owned(), "lookup_release".to_owned()];
        let palette = ToolPalette::parse(&palette_block(ToolPalette::stored_fields(&names)));
        assert_eq!(palette.tools.as_deref(), Some(names.as_slice()));
        assert!(palette.admits("lookup_commit"));
        assert!(!palette.admits("report_spam"));
    }

    #[test]
    fn an_absent_or_malformed_list_admits_nothing() {
        let absent = ToolPalette::parse(&palette_block(serde_json::Map::new()));
        assert_eq!(absent.tools, None);
        assert!(!absent.admits("lookup_commit"));

        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TOOLS.into(), json!("not a json array"));
        let malformed = ToolPalette::parse(&palette_block(fields));
        assert_eq!(malformed.tools, None);
        assert!(!malformed.admits("lookup_commit"));
    }

    #[test]
    fn an_empty_palette_is_recorded_and_admits_nothing() {
        let palette = ToolPalette::parse(&palette_block(ToolPalette::stored_fields(&[])));
        assert_eq!(palette.tools.as_deref(), Some(&[] as &[String]));
        assert!(!palette.admits("lookup_commit"));
    }
}
