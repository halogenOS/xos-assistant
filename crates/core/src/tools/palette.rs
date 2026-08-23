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

use agent_ledger::{Agency, Block, Column, ColumnType, ContentDescriptor, LeafKind, Projection};
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

/// Awaits nothing: the palette is a record, never a summons.
impl Agency for ToolPalette {}

/// Invisible to the model in every mode: the palette names capabilities, and
/// capability lists are for the admission wrapper, not for the prompt.
impl Projection for ToolPalette {}

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
