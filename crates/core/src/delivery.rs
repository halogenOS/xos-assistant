//! The delivery receipt: the consumer block kind that records one message
//! the assistant successfully sent, as the platform took it (unit 38,
//! 2026-08-30).
//!
//! Until this unit nothing survived a send. The platform answers a send
//! with the sent message, ids included, and both send paths threw that
//! answer away — so the ledger knew what the assistant SAID and never
//! which chat message she said it as. A member replying to one of her
//! messages could therefore not be matched to the words they replied to,
//! and their reply landed as a free-standing sentence.
//!
//! One block per delivered platform message, never one per send: an answer
//! past the platform's message cap goes out as several messages, each with
//! its own id, and a lookup from ONE of those ids back to the send is the
//! reading every consumer of this record needs. Each row holds three
//! values — the platform's id for that message, the delivery key that ties
//! the messages of one send together (the send's first id, already unique
//! within the channel, minting no new identity), and the stored block a
//! reply to that message quotes, where the send carried one of the
//! assistant's own blocks.
//!
//! The third value is what makes a reply to her quotable. An answer's row
//! names the answer's own block, so a reply to ANY of its chunks resolves
//! the whole stored answer: the chunks are a transport artifact and her
//! message is the block. A deterministic item, the failure notice and a
//! report's line name no block — the notice is not stored, the report
//! block declares no quotable column, and an item is fixed prose — so
//! their rows carry NULL there and a reply to one of them lands quoteless.
//!
//! Blocks and not a side table, so the rows cascade with a deleted
//! conversation exactly like every other content row and need no cleanup
//! pass of their own. The kind is agency-inert, frontier-transparent,
//! projects nothing, and joins the owing-tail walk's read-through list:
//! bookkeeping the model never reads as content — though a receipt row,
//! like any block, still ends a contiguous same-voice run in the
//! projection's grouping (decision 0139 records that consequence) — and a
//! debt standing behind one of its rows still owes its turn.

use agent_ledger::store::{StoreError, StoreTx, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, LeafKind, Projection, Store,
};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

use crate::kind::QuotableMessage;
use crate::message::DeliveryHandle;

/// The stored type string of the delivery-receipt kind.
pub const DELIVERED_KIND: &str = "delivered";

/// The content table the kind's descriptor owns.
pub const DELIVERED_TABLE: &str = "block_delivered";

/// The platform's own id for the delivered message, opaque — what a
/// member's reply names when it points at this message of the assistant's.
/// Structure, not personal data: the id names a message the assistant
/// wrote and carries nothing of anybody, so erasure leaves it and the
/// conversation's own deletion is what removes it.
pub const COLUMN_ORIGIN: &str = "origin";

/// The key tying the messages of one send together: the send's FIRST
/// delivered id, shared by every chunk of that send. It mints no new
/// identity — the value is one of the platform's own ids — and it is what
/// lets a reader ask for the whole send from any one of its messages.
pub const COLUMN_DELIVERY: &str = "delivery";

/// The stored block a reply to this delivered message quotes — the
/// assistant's own answer block, whose text is what the channel saw
/// (decision 0079 writes the disclosure line into the stored block before
/// the send, so stored text equals sent text). Nullable, and NULL is the
/// ordinary case for everything that is not an answer.
pub const COLUMN_ANSWER_BLOCK: &str = "answer_block";

/// One stored delivery receipt. Absences are typed per the kind contract:
/// an absent answer block is the recorded fact that this send carried no
/// quotable block of the assistant's own, and everything else absent is a
/// row the store did not produce.
#[derive(Debug, Clone)]
pub struct Delivered {
    /// The platform's id for the delivered message.
    pub origin: Option<String>,
    /// The send's first delivered id, shared by its messages.
    pub delivery: Option<String>,
    /// The block a reply to this message quotes; `None` for a send that
    /// carried none.
    pub answer_block: Option<i64>,
}

impl Delivered {
    /// The stored shape of one delivery receipt: the field map the receipt
    /// append carries, named by the same columns [`LeafKind::parse`] reads
    /// back — both sides of the kind's encoding live in this module, so a
    /// column rename cannot split them.
    #[must_use]
    pub fn stored_fields(
        origin: &str,
        delivery: &str,
        answer_block: Option<i64>,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_ORIGIN.into(), json!(origin));
        fields.insert(COLUMN_DELIVERY.into(), json!(delivery));
        if let Some(answer_block) = answer_block {
            fields.insert(COLUMN_ANSWER_BLOCK.into(), json!(answer_block));
        }
        fields
    }
}

impl LeafKind for Delivered {
    const KINDS: &'static [&'static str] = &[DELIVERED_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: DELIVERED_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[DELIVERED_KIND],
        columns: &[
            Column::new(COLUMN_ORIGIN, ColumnType::Text),
            Column::new(COLUMN_DELIVERY, ColumnType::Text),
            Column::new(COLUMN_ANSWER_BLOCK, ColumnType::Integer),
        ],
        // The answer block is named as a plain value and not as a declared
        // block reference: the row and the block it names are members of
        // the same conversation and are removed together with it, so
        // nothing here keeps a block alive that its conversation no longer
        // holds. A named block the lookup below cannot resolve simply
        // yields no quote, which is this unit's recorded quoteless case.
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            origin: string_field(block, COLUMN_ORIGIN),
            delivery: string_field(block, COLUMN_DELIVERY),
            answer_block: block
                .fields
                .get(COLUMN_ANSWER_BLOCK)
                .and_then(Value::as_i64),
        }
    }
}

/// Agency-inert and frontier-transparent, the join notice's twin
/// properties for the same reason: a receipt is appended by an independent
/// path at an arbitrary moment — the moment the platform took a message —
/// so the owed-turn frontier must read through it. This is load-bearing on
/// day one: a failed turn's failure notice records its own delivery AT THE
/// TAIL, and an opaque tail there would answer the debt walk with a
/// settled reading and bury the standing question behind it.
impl Agency for Delivered {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// Bookkeeping the model never meets: the default projection, which shows
/// nothing. A receipt says which chat message one of the assistant's
/// blocks became, which is a fact about the transport and not about the
/// conversation — and the block it names already projects its own words.
impl Projection for Delivered {}

/// Record what one send put in the chat: one block per delivered platform
/// message, in send order, all under the send's first id as the delivery
/// key (unit 38, 2026-08-30).
///
/// An empty list records nothing — a send that reached the chat with
/// nothing has no delivery to record — and the delivery key is read from
/// the first origin, so the messages of one send answer as one send from
/// any of them.
///
/// # Errors
///
/// [`StoreError`] if an append fails, the conversation no longer exists,
/// or the store's actor has stopped.
pub(crate) async fn record(
    store: &Store,
    delivery: DeliveryHandle,
    origins: &[String],
) -> Result<(), StoreError> {
    let Some(key) = origins.first() else {
        return Ok(());
    };
    for origin in origins {
        store
            .append_consumer_block(
                delivery.conversation_id(),
                None,
                DELIVERED_KIND,
                Delivered::stored_fields(origin, key, delivery.quotable_block()),
                None,
            )
            .await?;
    }
    Ok(())
}

/// The assistant's own message one origin names in one conversation, for
/// the reply quote of unit 38 (2026-08-30) — the NEWEST recorded delivery
/// of that origin that carried a quotable block, as the block a span
/// points at and the stored text a span is measured against.
///
/// Newest, for the reason the member-side lookup states: the platform's
/// ids are reused by nobody, but delivery is at-least-once everywhere in
/// this repository, and the latest recorded reading of an origin is the
/// one the member was looking at — the newest recorded delivery of that
/// origin THAT CARRIES A QUOTABLE ANSWER, since the text join skips a row
/// whose answer column is empty or whose block has no text row. The planned indexes are non-unique and
/// this tolerates a duplicate row rather than forbidding one.
///
/// The text comes from the framework's own text table beside the receipt
/// row, which is the one place the assistant's stored prose lives — the
/// deliberate coupling decision 0032 records for the framework's tables,
/// extended to the text table by decision 0079, whose write is what makes
/// the stored text equal to the sent text. A receipt whose block carries
/// no text row, and an origin this conversation never recorded, both
/// answer `None`: the reply lands quoteless and nothing is invented in
/// place of a message the ledger cannot show.
///
/// Scoped through the conversation junction like every other origin match:
/// platform message ids are opaque and unique only per channel, so a bare
/// id match would reach a stranger conversation's row.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn newest_answer_of_origin(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<Option<QuotableMessage>, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                &format!(
                    "SELECT d.{COLUMN_ANSWER_BLOCK}, t.content \
                     FROM {DELIVERED_TABLE} d \
                     JOIN conversation_blocks cb ON cb.block_id = d.block_id \
                     JOIN block_text t ON t.block_id = d.{COLUMN_ANSWER_BLOCK} \
                     WHERE cb.conversation_id = ?1 \
                     AND d.{COLUMN_ORIGIN} = ?2 \
                     ORDER BY cb.id DESC LIMIT 1"
                ),
                (conversation_id, &origin),
                |row| {
                    Ok(QuotableMessage {
                        block_id: row.get(0)?,
                        text: row.get(1)?,
                    })
                },
            )
            .optional()?)
    })
    .await
}

/// A text field read by column name from a loaded block's fields, absent
/// when the row holds no value — a NULL column never surfaces as a field,
/// and this keeps it from surfacing as an invented empty string.
fn string_field(block: &Block, name: &str) -> Option<String> {
    block
        .fields
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use agent_ledger::ContentPart;

    use super::*;

    fn delivered_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: DELIVERED_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// The stored shape round-trips through the parse, and the absent
    /// answer block stays absent: an item's receipt records that the send
    /// carried no quotable block, and nothing invents one.
    #[test]
    fn the_stored_fields_round_trip_and_an_item_names_no_block() {
        let answer = Delivered::parse(&delivered_block(Delivered::stored_fields(
            "12",
            "11",
            Some(88),
        )));
        assert_eq!(answer.origin.as_deref(), Some("12"));
        assert_eq!(answer.delivery.as_deref(), Some("11"));
        assert_eq!(answer.answer_block, Some(88));

        let item = Delivered::parse(&delivered_block(Delivered::stored_fields("13", "13", None)));
        assert_eq!(item.origin.as_deref(), Some("13"));
        assert_eq!(item.delivery.as_deref(), Some("13"));
        assert_eq!(item.answer_block, None, "an item's receipt names no block");
    }

    /// The kind is inert, transparent and invisible: it summons nothing,
    /// the frontier reads through it, it is a durable ledger row, and it
    /// projects nothing at all to the model.
    #[test]
    fn a_receipt_is_inert_transparent_and_shows_the_model_nothing() {
        let receipt = Delivered::parse(&delivered_block(Delivered::stored_fields(
            "12",
            "11",
            Some(88),
        )));
        assert_eq!(receipt.awaiting(), None, "a receipt summons nothing");
        assert!(
            receipt.frontier_transparent(),
            "the frontier reads through it"
        );
        assert!(receipt.durable(), "a receipt is a durable ledger row");
        assert_eq!(receipt.group_role(), None);
        assert_eq!(receipt.llm_text(), None);
        assert_eq!(receipt.llm_parts(), None::<Vec<ContentPart>>);
    }
}
