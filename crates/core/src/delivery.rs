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
//! message is the block. A deterministic item and a report's line
//! name no block — the report block declares no quotable column, and an
//! item is fixed prose — so
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
//!
//! The retraction rides beside it (unit T4, 2026-08-31), with the same
//! properties for the same reasons: one appended block recording that an
//! administrator asked for one recorded delivery to go. It is keyed on the
//! DELIVERY and never on the chunk, so an administrator replying to the
//! third message of an answer and one replying to the fifth are asking for
//! the same thing and record one fact between them.
//!
//! The retraction lookups are scoped to the channel's whole thread lineage,
//! not to one conversation. A platform message id is unique per channel, so
//! the scope has to be the channel; a compaction hands the channel a thread
//! that inherited only the second half, and a lookup scoped to that thread
//! alone would go blind on every delivery recorded before the cut. The older
//! reply-quote lookup below still scopes to one conversation and shares the
//! blindness; widening it belongs to a unit of its own.

use agent_ledger::agency::Quote;
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
/// assistant's own answer block, holding the model's own words under the
/// same disclosure line the channel read (decision 0079 writes that line
/// into the stored block before the send; the send itself cuts only a
/// leaked reasoning prefix, decision 0168). Nullable, and NULL is the
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
/// so the owed-turn frontier must read through it. This matters from day
/// one: a deterministic answer's receipt lands AT THE TAIL with no answer
/// block, and an opaque tail there would answer the debt walk with a
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
/// Scoped through ONE conversation's junction, unlike the retraction
/// lookups above: platform message ids are opaque and unique only per
/// channel, so a bare id match would reach a stranger conversation's row —
/// and after a compaction swap this narrower scope goes blind on receipts
/// below the cut, which is the recorded candidate for a unit of its own.
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

/// The stored type string of the retraction kind.
pub const RETRACTION_KIND: &str = "retraction";

/// The content table the retraction's descriptor owns.
pub const RETRACTION_TABLE: &str = "block_retraction";

/// The delivery this retraction was asked for — the same key the receipts
/// of one send share. Keyed on the delivery and never on the chunk: a reply
/// to any message of a send names the whole send, so two administrators
/// pointing at two chunks of one answer record one fact.
pub const COLUMN_RETRACTED_DELIVERY: &str = "delivery";

/// One stored retraction: the administrator's recorded ask that one
/// delivery be taken back.
#[derive(Debug, Clone)]
pub struct Retraction {
    /// The retracted send's key. Absent is a row the store did not produce.
    pub delivery: Option<String>,
}

impl Retraction {
    /// The stored shape of one retraction, named by the same column
    /// [`LeafKind::parse`] reads back.
    #[must_use]
    pub fn stored_fields(delivery: &str) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_RETRACTED_DELIVERY.into(), json!(delivery));
        fields
    }
}

impl LeafKind for Retraction {
    const KINDS: &'static [&'static str] = &[RETRACTION_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: RETRACTION_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[RETRACTION_KIND],
        columns: &[Column::new(COLUMN_RETRACTED_DELIVERY, ColumnType::Text)],
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            delivery: string_field(block, COLUMN_RETRACTED_DELIVERY),
        }
    }
}

/// Agency-inert and frontier-transparent, the receipt's twin properties for
/// the same reason: a retraction is appended by the deletion command's own
/// path at an arbitrary moment, so a debt standing behind it still owes its
/// turn.
impl Agency for Retraction {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// Bookkeeping the model never meets, and deliberately so. A projected line
/// could not name the answer it retracted without projecting the assistant's
/// own message origins, and a line appended at the tail would read as a
/// retraction of whatever answer happened to be newest. Showing nothing says
/// nothing false — and since unit T4 the retracted answer is not in the
/// model's view to be talked about anyway.
impl Projection for Retraction {}

/// What one recorded send holds, read back for a retraction: the platform's
/// ids of every message it put in the chat, in send order, and the stored
/// blocks a reply to any of those messages quotes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RecordedSend {
    /// The platform ids to take back, in send order.
    pub origins: Vec<String>,
    /// The assistant's own blocks the send carried — normally one answer
    /// block, empty for a send of fixed prose the ledger never stored.
    pub answer_blocks: Vec<i64>,
}

/// Which recorded send one delivered message belongs to: the delivery key of
/// the NEWEST receipt naming that origin anywhere in the channel's lineage.
///
/// Newest, for the reason the reply quote's own lookup states: delivery is
/// at-least-once everywhere in this repository, so the latest recorded
/// reading of an origin is the one an administrator was looking at.
///
/// `None` says the ledger recorded no delivery under that id — a message
/// sent before the receipts shipped, one whose receipt never appended, or an
/// id from another channel entirely. The command stays recognized and
/// nothing is retracted.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn delivery_of_origin(
    tx: &StoreTx,
    lineage: &[i64],
    origin: &str,
) -> Result<Option<String>, StoreError> {
    let origin = origin.to_owned();
    let held = held_by_lineage("d", lineage);
    let lineage = lineage.to_vec();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let mut arguments: Vec<rusqlite::types::Value> = vec![origin.into()];
        arguments.extend(lineage.into_iter().map(rusqlite::types::Value::from));
        Ok(conn
            .query_row(
                &format!(
                    "SELECT d.{COLUMN_DELIVERY} FROM {DELIVERED_TABLE} d \
                     WHERE d.{COLUMN_ORIGIN} = ?1 AND {held} \
                     ORDER BY d.block_id DESC LIMIT 1"
                ),
                rusqlite::params_from_iter(arguments),
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    })
    .await
}

/// Everything one recorded send put in the chat, read across the channel's
/// whole lineage.
///
/// The rows come back in block order, which is send order: the receipts of
/// one send are appended one per delivered message as the platform takes
/// them. A block held by two conversations of the lineage — every block past
/// a compaction's cut is — is read once, because the membership is asked as
/// an existence question and never joined.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn recorded_send(
    tx: &StoreTx,
    lineage: &[i64],
    delivery: &str,
) -> Result<RecordedSend, StoreError> {
    let delivery = delivery.to_owned();
    let held = held_by_lineage("d", lineage);
    let lineage = lineage.to_vec();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let mut arguments: Vec<rusqlite::types::Value> = vec![delivery.into()];
        arguments.extend(lineage.into_iter().map(rusqlite::types::Value::from));
        let rows = conn
            .prepare(&format!(
                "SELECT d.{COLUMN_ORIGIN}, d.{COLUMN_ANSWER_BLOCK} \
                 FROM {DELIVERED_TABLE} d \
                 WHERE d.{COLUMN_DELIVERY} = ?1 AND {held} \
                 ORDER BY d.block_id"
            ))?
            .query_map(rusqlite::params_from_iter(arguments), |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut send = RecordedSend::default();
        for (origin, answer_block) in rows {
            if let Some(origin) = origin {
                send.origins.push(origin);
            }
            if let Some(answer_block) = answer_block
                && !send.answer_blocks.contains(&answer_block)
            {
                send.answer_blocks.push(answer_block);
            }
        }
        Ok(send)
    })
    .await
}

/// Whether a retraction already stands for one delivery anywhere in the
/// channel's lineage.
///
/// The recorded fact is that an administrator asked for this delivery to go,
/// and asking twice is one fact — so a repeat command appends no second
/// block. On the wire the repeat still re-issues the call, because the first
/// one may have failed; that half lives with the directive, not here.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn retraction_stands(
    tx: &StoreTx,
    lineage: &[i64],
    delivery: &str,
) -> Result<bool, StoreError> {
    let delivery = delivery.to_owned();
    let held = held_by_lineage("r", lineage);
    let lineage = lineage.to_vec();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let mut arguments: Vec<rusqlite::types::Value> = vec![delivery.into()];
        arguments.extend(lineage.into_iter().map(rusqlite::types::Value::from));
        Ok(conn
            .query_row(
                &format!(
                    "SELECT 1 FROM {RETRACTION_TABLE} r \
                     WHERE r.{COLUMN_RETRACTED_DELIVERY} = ?1 AND {held} LIMIT 1"
                ),
                rusqlite::params_from_iter(arguments),
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some())
    })
    .await
}

/// Record that one delivery was asked back — one appended block, whatever
/// the platform later does with the request. The ask certainly happened, so
/// it is recorded whether or not the chat obeys.
///
/// # Errors
///
/// [`StoreError`] if the append fails, the conversation no longer exists, or
/// the store's actor has stopped.
pub(crate) async fn record_retraction(
    store: &Store,
    conversation_id: i64,
    delivery: &str,
) -> Result<(), StoreError> {
    store
        .append_consumer_block(
            conversation_id,
            None,
            RETRACTION_KIND,
            Retraction::stored_fields(delivery),
            None,
        )
        .await?;
    Ok(())
}

/// Which blocks of one ledger a retracted send takes with it: the assistant's
/// own answer blocks the send carried, and every quote block derived from
/// one of them.
///
/// The derived quotes are the second half of taking the answer out of the
/// model's view. A quote block stores a SPAN into the block it quotes, so a
/// quote of a retracted answer resolves that answer's own words at read time
/// — a member's reply quote, and the deletion command's own reply quote above
/// all, which every retraction creates by construction. Left behind, they
/// would show the model the retracted words under somebody else's message.
///
/// The delivery receipts themselves are NOT here: a receipt is a record of
/// what the platform took, and a receipt whose answer block is gone simply
/// resolves no quote, which is the correct reading of a message that was
/// taken back.
pub(crate) fn retracted_blocks(blocks: &[Block], answer_blocks: &[i64]) -> Vec<i64> {
    blocks
        .iter()
        .filter(|block| {
            answer_blocks.contains(&block.id)
                || (Quote::KINDS.contains(&block.block_type.as_str())
                    && quoted_endpoints(block).any(|endpoint| answer_blocks.contains(&endpoint)))
        })
        .map(|block| block.id)
        .collect()
}

/// The blocks one quote points at, read off the stored endpoints the
/// framework writes. Both endpoints name the same block for every quote this
/// assistant lands, and both are read anyway: this reading answers what the
/// row says, not what the writer intended.
fn quoted_endpoints(block: &Block) -> impl Iterator<Item = i64> + '_ {
    ["start_block_id", "end_block_id"]
        .into_iter()
        .filter_map(|name| block.fields.get(name).and_then(Value::as_i64))
}

/// The membership predicate every lookup here shares, written against the
/// queried row's own alias: the row's block is held by one of the channel's
/// lineage conversations.
///
/// Spelled as an existence question and never as a join, so a block held by
/// two conversations of one lineage — every block past a compaction's cut —
/// is answered once instead of once per membership. An empty lineage cannot
/// arise: a caller always knows the conversation it is asking about, so the
/// predicate always names at least one.
fn held_by_lineage(row: &str, lineage: &[i64]) -> String {
    let placeholders = (0..lineage.len())
        .map(|index| format!("?{}", index + 2))
        .collect::<Vec<String>>()
        .join(", ");
    format!(
        "EXISTS (SELECT 1 FROM conversation_blocks cb \
         WHERE cb.block_id = {row}.block_id \
         AND cb.conversation_id IN ({placeholders}))"
    )
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
