//! The assistant's block kind, composed with the framework's kinds.
//!
//! One consumer kind exists in this unit: [`ChatMessage`], a recorded channel
//! message. Its descriptor declares the content table, its projection renders
//! the message for the model, and its agency hook makes an appended message
//! await a turn — every recorded message awaits the model until the acting
//! policy arrives with the live-model unit.
//!
//! The content table is also the personal-data table of decision 0003: the
//! block header row is the immutable ledger entry, the content row carries the
//! personal payload. The `text` column is nullable for exactly one reason —
//! erasure nulls it — so a message without stored text is an erased message,
//! projects nothing to the model, and awaits nothing. Because that write is
//! part of the kind's own contract, it lives here too, as the crate-private
//! `erase_principal_content` the erasure operation composes.

use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{
    Agency, Awaiting, Block, BlockKind, Column, ColumnType, ContentDescriptor, ContentPart,
    LeafKind, Projection, Role, StoreError,
};
use serde_json::{Value, json};

use crate::message::Authority;

/// The stored type string of the assistant's message kind.
pub const CHAT_MESSAGE_KIND: &str = "chat_message";

/// The content table the kind's descriptor owns.
pub const CHAT_MESSAGE_TABLE: &str = "block_chat_message";

/// The block's voice — written from the append's role argument, read back
/// into the block's role, never a content field.
pub const COLUMN_ROLE: &str = "role";
/// What was said. Nullable: NULL is the one legal absence and means erased.
pub const COLUMN_TEXT: &str = "text";
/// The sender's principal id in the identity tables.
pub const COLUMN_PRINCIPAL_ID: &str = "principal_id";
/// The sender's standing at receipt, in the closed [`Authority`] vocabulary.
pub const COLUMN_AUTHORITY: &str = "authority";
/// The platform's own id for the message, opaque.
pub const COLUMN_ORIGIN: &str = "origin";
/// When the platform says the message was sent, RFC 3339. The block header's
/// own `created_at` is the store's insertion time, so the ledger keeps both:
/// the platform's send time here, the store's receipt time on the header.
pub const COLUMN_SENT_AT: &str = "sent_at";

/// One recorded channel message: who said it (by principal id only), with what
/// standing, and what was said.
///
/// The sender's identity never appears here — it lives in the identity tables,
/// keyed by the principal id this block carries, so identity erasure deletes
/// rows there and touches no block.
///
/// # Absences are typed, never defaulted
///
/// [`LeafKind::parse`] is total by the framework's contract — the framework
/// itself hands it synthetic blocks with empty fields — so a field the stored
/// row must carry cannot fail the parse. The loud failure for a malformed row
/// is the store's: a header without its content row refuses to load, the
/// schema's NOT NULL and CHECK constraints refuse the write that would omit or
/// mangle a required column. What reaches this struct is therefore either a
/// well-formed stored row or a synthetic block, and every absence stays an
/// absence: no principal id is invented, no text becomes an empty string.
/// `text` is the one absence with stored meaning — erased.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// The block's voice, read back from the stored row.
    pub role: Option<Role>,
    /// What was said. `None` is an erased message: the stored text was nulled
    /// by erasure, and the message projects nothing to the model.
    pub text: Option<String>,
    /// The sender's principal id in the identity tables. `None` only for a
    /// block the store did not produce (the schema stores it NOT NULL);
    /// predicates matching on it reject `None` instead of comparing it.
    pub principal_id: Option<i64>,
    /// The sender's standing at receipt. `None` only for a block the store
    /// did not produce (the schema closes the vocabulary with a CHECK).
    pub authority: Option<Authority>,
    /// The platform's own id for the message, opaque.
    pub origin: Option<String>,
    /// The platform's send time, RFC 3339. The store's insertion time lives
    /// on the block header. `None` only for a block the store did not
    /// produce.
    pub sent_at: Option<String>,
}

impl ChatMessage {
    /// The stored shape of one recorded message: the field map a consumer
    /// append carries, named by the same columns [`LeafKind::parse`] reads
    /// back — both sides of the kind's encoding live in this module, so a
    /// column rename cannot split them. The role travels as the append's own
    /// argument, never as a field.
    #[must_use]
    pub fn stored_fields(
        text: &str,
        principal_id: i64,
        authority: Authority,
        origin: Option<&str>,
        sent_at: &str,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TEXT.into(), json!(text));
        fields.insert(COLUMN_PRINCIPAL_ID.into(), json!(principal_id));
        fields.insert(COLUMN_AUTHORITY.into(), json!(authority.as_str()));
        if let Some(origin) = origin {
            fields.insert(COLUMN_ORIGIN.into(), json!(origin));
        }
        fields.insert(COLUMN_SENT_AT.into(), json!(sent_at));
        fields
    }
}

impl LeafKind for ChatMessage {
    const KINDS: &'static [&'static str] = &[CHAT_MESSAGE_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: CHAT_MESSAGE_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[CHAT_MESSAGE_KIND],
        columns: &[
            Column::new(COLUMN_ROLE, ColumnType::Text),
            Column::new(COLUMN_TEXT, ColumnType::Text),
            Column::new(COLUMN_PRINCIPAL_ID, ColumnType::Integer),
            Column::new(COLUMN_AUTHORITY, ColumnType::Text),
            Column::new(COLUMN_ORIGIN, ColumnType::Text),
            Column::new(COLUMN_SENT_AT, ColumnType::Text),
        ],
        reference_columns: &[],
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            role: block.role,
            text: string_field(block, COLUMN_TEXT),
            principal_id: block
                .fields
                .get(COLUMN_PRINCIPAL_ID)
                .and_then(Value::as_i64),
            authority: block
                .fields
                .get(COLUMN_AUTHORITY)
                .and_then(Value::as_str)
                .and_then(Authority::parse),
            origin: string_field(block, COLUMN_ORIGIN),
            sent_at: string_field(block, COLUMN_SENT_AT),
        }
    }
}

impl Agency for ChatMessage {
    fn awaiting(&self) -> Option<Awaiting> {
        // Every recorded message awaits the model in this unit; the acting
        // policy — record all, answer some — arrives with the live-model
        // unit, where the addressing rules exist to express it. An erased
        // message is the exception: with its text gone it has nothing to be
        // answered, so it must not summon a turn either.
        (self.role == Some(Role::User) && self.text.is_some()).then_some(Awaiting::Model)
    }
}

impl Projection for ChatMessage {
    fn group_role(&self) -> Option<Role> {
        // An erased message is boundary-invisible: it opens no message group,
        // so the model never learns the erased row existed.
        self.text.as_ref().and(self.role)
    }

    fn llm_parts(&self) -> Option<Vec<ContentPart>> {
        self.text
            .as_ref()
            .map(|text| vec![ContentPart::Text { text: text.clone() }])
    }

    fn llm_text(&self) -> Option<String> {
        self.text.clone()
    }
}

/// Null the personal columns — text, origin reference and platform send
/// time — of every message a principal wrote, in every conversation: the
/// first of erasure's three steps (decision 0012), owned by the kind because
/// the nullable columns are the kind's own contract. The block header rows
/// are never touched; each affected content row keeps its shape and its
/// message reads back erased. Nulling already-null columns is a no-op, so
/// the step is idempotent.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_principal_content(
    tx: &StoreTx,
    principal_id: i64,
) -> Result<(), StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        conn.execute(
            &format!(
                "UPDATE {CHAT_MESSAGE_TABLE} SET {COLUMN_TEXT} = NULL, \
                 {COLUMN_ORIGIN} = NULL, {COLUMN_SENT_AT} = NULL \
                 WHERE {COLUMN_PRINCIPAL_ID} = ?1"
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

/// The composed kind set the runtime is instantiated over: the framework's
/// kinds through the delegate, the assistant's beside them.
#[derive(Agency)]
pub enum AssistantKind {
    /// The framework's own kinds, resolved through the delegate.
    #[agency(delegate)]
    Core(BlockKind),
    /// The assistant's recorded channel message.
    ChatMessage(ChatMessage),
}

/// A text field read by column name from a loaded block's fields, absent when
/// the row holds no value — a NULL column never surfaces as a field, and this
/// keeps it from surfacing as an invented empty string.
fn string_field(block: &Block, name: &str) -> Option<String> {
    block
        .fields
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}
