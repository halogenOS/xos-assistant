//! The outgoing message: the consumer block kind one sending tool appends,
//! and the reading of what became of it (unit 55, 2026-09-02).
//!
//! Until this unit the assistant's answer WAS its written text: the
//! framework committed a turn's prose as an assistant text block and the
//! outbound edge relayed it to the chat. From here the model's text is
//! private notes, and a message reaches the group only through the two
//! sending tools — each of which appends one block of this kind and returns
//! a pending call. The edge classifies the block exactly as it classifies a
//! reaction, hands it to the adapter, and the delivery receipt resolves the
//! pending call with what the platform did.
//!
//! Three stored values, and no fourth. The `text` is what goes out, stored
//! as the model wrote it — the send narrows the wire text where a leaked
//! reasoning prefix stands, and the first-interaction disclosure line is
//! composed into this stored text before the send, so the ledger carries
//! the introduction the channel received. The `reply_to` names the message
//! this one threads onto, validated by the tool against the conversation's
//! own ledger and NULL for a plain send. The `call_block` names the tool
//! call this block answers: it is what makes the tool body idempotent — a
//! re-run of one call after a restart finds its own block and appends no
//! second one — and it is what every later reading keys on, because a
//! consumer block carries no dispatch anchor of its own and the call block
//! does.
//!
//! The kind is frontier-transparent, for the reaction's own reason: the
//! block is written INTO a live turn's window by the tool, so the owed-turn
//! decision must read through it. It projects nothing: the model's sends
//! appear to it as what they are — a tool call carrying the text and the
//! target, and a result carrying the ids the platform assigned — and a
//! second rendering of the same words under a different voice would read as
//! a message it did not send.
//!
//! What the block declares that a reaction does not is its quotable column.
//! A member replying to one of the assistant's messages quotes the words
//! she actually sent, and those words live here now; the delivery receipt
//! names this block, and the framework resolves the span against this
//! column.

use agent_ledger::agency::CallOutcome;
use agent_ledger::store::{StoreError, StoreTx, domain_run};
use agent_ledger::{
    Agency, Block, BlockKind, Column, ColumnType, ContentDescriptor, FromBlock, LeafKind,
    Projection,
};
use serde_json::{Value, json};

use crate::kind::AssistantKind;

/// The stored type string of the outgoing-message kind.
pub const OUTGOING_MESSAGE_KIND: &str = "outgoing_message";

/// The content table the kind's descriptor owns.
pub const OUTGOING_MESSAGE_TABLE: &str = "block_outgoing_message";

/// What goes to the chat, as the model wrote it — under the disclosure line
/// where a first send composed one in, which is the one person-written
/// prefix a stored outgoing text may carry. NOT NULL: a send with no words
/// is not a send, and the tool refuses one before this row exists.
pub const COLUMN_TEXT: &str = "text";

/// The platform's own id of the message this one threads onto, opaque, or
/// NULL for a plain send. The tool validates it against the serving
/// conversation's ledger before the row exists, so a stored value named
/// something the ledger held at the moment of the call.
pub const COLUMN_REPLY_TO: &str = "reply_to";

/// The block id of the tool call this send answers. NOT NULL: a send with
/// no call behind it could never be resolved, completed or failed, and the
/// idempotence that keeps a re-run from sending twice is a match on exactly
/// this column.
pub const COLUMN_CALL_BLOCK: &str = "call_block";

/// One filed outgoing message. Absences are typed per the kind contract:
/// every one of them is a row the store did not produce, except the reply
/// target, whose absence is the stored fact that the send goes out plain.
#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    /// The words to send. `None` only for a row the store did not produce.
    pub text: Option<String>,
    /// The message this one threads onto; `None` is a plain send.
    pub reply_to: Option<String>,
    /// The tool call this send answers. `None` only for a row the store did
    /// not produce.
    pub call_block: Option<i64>,
}

impl OutgoingMessage {
    /// The stored shape of one outgoing message: the field map the sending
    /// tools' append carries, named by the same columns [`LeafKind::parse`]
    /// reads back — both sides of the kind's encoding live in this module,
    /// so a column rename cannot split them.
    #[must_use]
    pub fn stored_fields(
        text: &str,
        reply_to: Option<&str>,
        call_block: i64,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TEXT.into(), json!(text));
        if let Some(reply_to) = reply_to {
            fields.insert(COLUMN_REPLY_TO.into(), json!(reply_to));
        }
        fields.insert(COLUMN_CALL_BLOCK.into(), json!(call_block));
        fields
    }
}

impl LeafKind for OutgoingMessage {
    const KINDS: &'static [&'static str] = &[OUTGOING_MESSAGE_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: OUTGOING_MESSAGE_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[OUTGOING_MESSAGE_KIND],
        columns: &[
            Column::new(COLUMN_TEXT, ColumnType::Text),
            Column::new(COLUMN_REPLY_TO, ColumnType::Text),
            Column::new(COLUMN_CALL_BLOCK, ColumnType::Integer),
        ],
        // The call block is named as a plain value and not as a declared
        // block reference: the row and the call it names are members of the
        // same conversation and go with it, so nothing here keeps a block
        // alive its conversation no longer holds.
        reference_columns: &[],
        // What a member's reply to one of the assistant's messages quotes:
        // the words she actually sent, which is this row's own text. The
        // delivery receipt names this block, and the framework resolves the
        // span against the column declared here.
        quoted_text_column: Some(COLUMN_TEXT),
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            text: string_field(block, COLUMN_TEXT),
            reply_to: string_field(block, COLUMN_REPLY_TO),
            call_block: block.fields.get(COLUMN_CALL_BLOCK).and_then(Value::as_i64),
        }
    }
}

/// Agency-inert, and frontier-transparent for the reaction's own reason:
/// the tool writes this block INTO a live turn's window, so the owed-turn
/// decision must read through it. A send is filed precisely on the turns
/// that answer somebody, and a message absorbed while the send was in
/// flight must still summon its own turn.
impl Agency for OutgoingMessage {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// Invisible to the model, deliberately: what the model knows about its own
/// send is the tool call it made and the result it was handed. A projected
/// copy of the same words in another voice would read as a message it never
/// wrote.
impl Projection for OutgoingMessage {}

/// What became of one filed send, as the conversation's own ledger records
/// it — the reading the caps count and the startup sweep share, so neither
/// can decide it a second way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendState {
    /// The call is still open: no result and no error answers it. Either
    /// the platform has not spoken yet, or the process died before it did —
    /// which is what the startup sweep exists to settle.
    Pending,
    /// The call completed: the platform took the message and the receipt
    /// door handed the ids back.
    Delivered,
    /// The call failed: nothing reached the chat, or a send cut short, or
    /// the restart sentence settled it. A failed send posted nothing that
    /// the caps count.
    Failed,
}

/// What became of the send one call block filed, read over a loaded ledger.
///
/// A call is paired with its outcome by the call's own BLOCK id, and that
/// pairing lives in the framework: this asks [`ToolCall::outcome_in`] and
/// reads its answer as the send's state. The app keeps no walk of its own
/// and compares no provider echo — an echo can repeat across two calls of
/// one round, and a second implementation of the pairing would answer
/// differently the first time either side changed.
///
/// A call the ledger no longer holds reads [`SendState::Pending`]: the send
/// was filed and nothing in this ledger says it settled, which is the
/// counting direction the caps want and the sweeping direction the startup
/// pass wants.
pub(crate) fn send_state(ledger: &[Block], call_block: i64) -> SendState {
    let call = ledger
        .iter()
        .find(|block| block.id == call_block)
        .and_then(|block| match BlockKind::from_block(block) {
            BlockKind::ToolCall(call) => Some(call),
            _ => None,
        });
    match call.and_then(|call| call.outcome_in(ledger)) {
        Some(CallOutcome::Result(_)) => SendState::Delivered,
        Some(CallOutcome::Error(_)) => SendState::Failed,
        None => SendState::Pending,
    }
}

/// One filed send of a loaded ledger: the block itself and the call it
/// answers, read through the composed kind's one parse path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FiledSend {
    /// The outgoing block's own id.
    pub block_id: i64,
    /// The call block it answers.
    pub call_block: i64,
    /// The block header's store-clock creation time, RFC 3339 as the store
    /// writes it — what the caps measure their trailing spans against.
    pub created_at: String,
}

/// Every send one loaded ledger holds, oldest first — the one walk the
/// idempotence check, the caps count and the settling passes share.
///
/// A row without its call block is skipped rather than guessed at: the
/// column is NOT NULL, so such a row is one the store did not produce, and
/// nothing downstream can key on a call it cannot name.
pub(crate) fn filed_sends(ledger: &[Block]) -> Vec<FiledSend> {
    ledger
        .iter()
        .filter_map(|block| match AssistantKind::from_block(block) {
            AssistantKind::OutgoingMessage(outgoing) => {
                outgoing.call_block.map(|call_block| FiledSend {
                    block_id: block.id,
                    call_block,
                    created_at: block.created_at.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

/// The send one call already filed, if any — the tool body's idempotence
/// read (unit 55): a call re-run after a restart finds its own block here
/// and appends no second one.
pub(crate) fn send_of_call(ledger: &[Block], call_block: i64) -> Option<FiledSend> {
    filed_sends(ledger)
        .into_iter()
        .find(|send| send.call_block == call_block)
}

// ─── What a send came to, said to the model ──────────────────────────────
//
// Every one of these sentences is the RESULT of a tool call: the model
// reads it as the answer to the send it made, so each states what actually
// happened to the message and nothing beyond it. The success carries the
// ids, which is the whole reason a send is a pending call and not a
// fire-and-forget append — a model that does not learn the id of its own
// message cannot answer a member who replies to it.

/// The whole send's result: the platform took the message, under these ids.
///
/// An answer past the platform's message cap goes out as several messages,
/// so the plural is real and not defensive. A whole send that reported no
/// id at all is possible only from an adapter that took a message and told
/// the core nothing about it; the sentence then says exactly that, rather
/// than claiming an id it does not have.
#[must_use]
pub fn sent_result(origins: &[String]) -> String {
    match origins {
        [] => "The message was sent. The chat reported no id for it, so you cannot reply to \
               it by id."
            .to_owned(),
        [single] => format!("The message was sent. Its id in this chat is {single}."),
        several => format!(
            "The message was sent as {count} messages. Their ids in this chat are {ids}.",
            count = several.len(),
            ids = several.join(", "),
        ),
    }
}

/// The failed send's result: nothing reached the chat, and why.
#[must_use]
pub fn send_failed(reason: &str) -> String {
    format!("The message was not sent: {reason}. Nothing of it reached the chat.")
}

/// The cut-short send's result: some of the message stands in the chat and
/// the rest does not (unit 55, 2026-09-02).
///
/// It is a FAILURE and it names the ids that posted, because both halves
/// matter to the model: the message it meant to send is not what the group
/// read, and a member who replies to the part that did post is replying to
/// one of these ids.
#[must_use]
pub fn send_cut_short(origins: &[String], reason: &str) -> String {
    format!(
        "The message was only partly sent: {ids} reached the chat and the rest did not, \
         because {reason}. A member replying to what posted replies to one of those ids.",
        ids = origins.join(", "),
    )
}

/// The restarted process's settlement, the same trade decision 0014 made
/// for a redelivered update: a possible duplicate over a possible silence.
///
/// A send the process died with is never delivered late — the outbound
/// edge's startup seed marks everything already stored as history, so the
/// block would sit undelivered forever — and the model is told so plainly
/// instead of waiting on a call nothing will ever answer.
pub const RESTARTED_BEFORE_CONFIRMED: &str = "The assistant restarted before the chat confirmed this message, so it counts as unsent \
     and will not be sent now. Send it again on a later turn if it still matters.";

/// The retired conversation's settlement: the channel moved on to a fresh
/// session before the chat confirmed the send, so the block it was filed in
/// no longer serves anything and the send will not happen.
pub const RETIRED_BEFORE_CONFIRMED: &str = "This conversation was retired before the chat confirmed this message, so it counts as \
     unsent and will not be sent now.";

/// Settle one pending send through the framework's one door for it, naming
/// the call by its block id.
///
/// A misdirected id — a call this conversation does not hold — is logged
/// and dropped rather than propagated: the settlement is bookkeeping about
/// a message that has already happened or already failed, and there is
/// nothing a caller could do about a call it cannot name. An already
/// settled call is left exactly as it stands: the framework's door answers
/// `None` and appends nothing, so a repeated report is a no-op and never a
/// second outcome.
///
/// # Errors
///
/// [`StoreError`] if the write fails or the store's actor has stopped.
pub(crate) async fn settle(
    store: &agent_ledger::Store,
    conversation_id: i64,
    call_block: i64,
    outcome: agent_ledger::ToolCallResult,
) -> Result<(), StoreError> {
    match store
        .resolve_tool_call(conversation_id, call_block, outcome)
        .await
    {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            tracing::debug!(
                conversation_id,
                call_block,
                "the send's call was already settled; nothing was appended"
            );
            Ok(())
        }
        Err(StoreError::NoSuchToolCall { .. }) => {
            tracing::warn!(
                conversation_id,
                call_block,
                "a send names a call this conversation does not hold; the settlement is dropped"
            );
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Fail every send this conversation holds whose call is still unresolved,
/// with the given sentence — the settling pass the startup sweep and the
/// retirement both run (unit 55, 2026-09-02).
///
/// Answers how many calls it settled. Idempotent by the framework's own
/// door: a call already carrying an outcome is left as it stands, so a
/// second pass over the same conversation settles nothing and appends
/// nothing.
///
/// # Errors
///
/// [`StoreError`] if the ledger read or a settlement write fails, or the
/// store's actor has stopped.
pub(crate) async fn fail_pending_sends(
    store: &agent_ledger::Store,
    conversation_id: i64,
    reason: &str,
) -> Result<usize, StoreError> {
    let ledger = store.list_blocks(conversation_id).await?;
    let mut settled = 0;
    for send in filed_sends(&ledger) {
        if send_state(&ledger, send.call_block) != SendState::Pending {
            continue;
        }
        settle(
            store,
            conversation_id,
            send.call_block,
            agent_ledger::ToolCallResult::Error {
                error: reason.to_owned(),
            },
        )
        .await?;
        settled += 1;
    }
    Ok(settled)
}

/// Write the disclosure line into one stored outgoing block, in one
/// idempotent statement: the prepend applies only while the text does not
/// already open with the prefix, so a repeated send cannot stack a second
/// line. The table is this kind's own, which is why the statement lives
/// here and the decision lives with the disclosure.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn prepend_line(
    tx: &StoreTx,
    block_id: i64,
    prefix: &str,
) -> Result<(), StoreError> {
    let prefix = prefix.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        conn.execute(
            &format!(
                "UPDATE {OUTGOING_MESSAGE_TABLE} SET {COLUMN_TEXT} = ?2 || {COLUMN_TEXT} \
                 WHERE block_id = ?1 AND substr({COLUMN_TEXT}, 1, length(?2)) <> ?2"
            ),
            rusqlite::params![block_id, prefix],
        )?;
        Ok(())
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
    use agent_ledger::{ContentPart, Role};

    use super::*;

    fn outgoing_block(id: i64, fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id,
            role: None,
            block_type: OUTGOING_MESSAGE_KIND.into(),
            created_at: "2026-09-02T00:00:00Z".into(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// One recorded tool call under the given provider echo.
    fn call_block(id: i64, echo: &str) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("tool_call_id".into(), json!(echo));
        fields.insert("name".into(), json!("send_message"));
        fields.insert("input".into(), json!("{}"));
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "tool_call".into(),
            created_at: String::new(),
            dispatch_anchor: Some(1),
            fields,
        }
    }

    /// One recorded resolution naming the call BLOCK it answers, on the arm
    /// the kind names — the pairing the framework writes and reads.
    fn resolution(id: i64, kind: &str, call_block: i64) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("source_block_id".into(), json!(call_block));
        fields.insert(
            if kind == "tool_result" {
                "content"
            } else {
                "error"
            }
            .into(),
            json!("whatever the model reads"),
        );
        Block {
            id,
            role: None,
            block_type: kind.into(),
            created_at: String::new(),
            dispatch_anchor: Some(1),
            fields,
        }
    }

    /// The stored shape round-trips, and the absent reply target stays
    /// absent: a plain send records that it threads nowhere, and nothing
    /// invents a target for it.
    #[test]
    fn the_stored_fields_round_trip_and_a_plain_send_names_no_target() {
        let threaded = OutgoingMessage::parse(&outgoing_block(
            2,
            OutgoingMessage::stored_fields("the answer", Some("12345"), 9),
        ));
        assert_eq!(threaded.text.as_deref(), Some("the answer"));
        assert_eq!(threaded.reply_to.as_deref(), Some("12345"));
        assert_eq!(threaded.call_block, Some(9));

        let plain = OutgoingMessage::parse(&outgoing_block(
            3,
            OutgoingMessage::stored_fields("the answer", None, 9),
        ));
        assert_eq!(plain.reply_to, None, "a plain send names no target");
    }

    /// The kind is inert, transparent and invisible: it summons nothing,
    /// the owed-turn frontier reads through it, it is a durable ledger row,
    /// and the model reads its own send through the tool call and its
    /// result rather than through a second rendering of the words.
    #[test]
    fn a_send_is_inert_transparent_and_invisible() {
        let send = OutgoingMessage::parse(&outgoing_block(
            2,
            OutgoingMessage::stored_fields("the answer", None, 9),
        ));
        assert_eq!(send.awaiting(), None, "a filed send summons nothing");
        assert!(
            send.frontier_transparent(),
            "the owed-turn frontier reads through it"
        );
        assert!(send.durable(), "a filed send is a durable ledger row");
        assert_eq!(send.group_role(), None);
        assert_eq!(send.llm_text(), None);
        assert_eq!(send.llm_parts(), None::<Vec<ContentPart>>);
    }

    /// The three states, each read off the ledger the caps and the sweeps
    /// read: an unanswered call is pending, a result is a delivery, an
    /// error is a failure — and a call the ledger does not hold reads
    /// pending, the counting and sweeping direction both want.
    ///
    /// The two calls carry ONE provider echo (AC19): a repeated echo is a
    /// shape a provider may emit, and each call still reads only the
    /// resolution naming its own block, because the pairing is the
    /// framework's block-id one and no echo is compared anywhere.
    #[test]
    fn the_send_state_reads_the_calls_own_resolution() {
        let mut ledger = vec![
            call_block(10, "one-echo"),
            outgoing_block(11, OutgoingMessage::stored_fields("one", None, 10)),
            call_block(12, "one-echo"),
            outgoing_block(13, OutgoingMessage::stored_fields("two", None, 12)),
        ];
        assert_eq!(send_state(&ledger, 10), SendState::Pending);
        assert_eq!(send_state(&ledger, 12), SendState::Pending);
        assert_eq!(
            send_state(&ledger, 99),
            SendState::Pending,
            "a call this ledger does not hold reads as unsettled"
        );

        ledger.push(resolution(14, "tool_result", 10));
        assert_eq!(send_state(&ledger, 10), SendState::Delivered);
        assert_eq!(
            send_state(&ledger, 12),
            SendState::Pending,
            "the sibling shares the echo and is settled by none of it"
        );

        ledger.push(resolution(15, "tool_error", 12));
        assert_eq!(send_state(&ledger, 12), SendState::Failed);
    }

    /// The idempotence read: one call's own filed send is found by its
    /// call block, and a call that filed nothing finds nothing.
    #[test]
    fn a_calls_own_send_is_found_by_its_call_block() {
        let ledger = vec![
            call_block(10, "echo-a"),
            outgoing_block(11, OutgoingMessage::stored_fields("one", None, 10)),
        ];
        assert_eq!(
            send_of_call(&ledger, 10),
            Some(FiledSend {
                block_id: 11,
                call_block: 10,
                created_at: "2026-09-02T00:00:00Z".to_owned(),
            })
        );
        assert_eq!(send_of_call(&ledger, 12), None);
    }
}
