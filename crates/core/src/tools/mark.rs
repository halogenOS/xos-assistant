//! The react tool and the message-mark block kind: one emoji the
//! assistant puts on a message where a reply would add nothing (unit 39,
//! 2026-08-30; the trigger redefined by unit 54, 2026-09-02).
//!
//! The flow: the prompt teaches the model that a response to the assistant
//! which needs no further response — the thanks that closes an exchange
//! already answered — can be stamped off with one reaction instead of the
//! empty turn the silence default would otherwise end in, and the model
//! calls this tool naming the message by its projected id and giving the
//! emoji it chose. The named origin is validated against the turn's own
//! co-summoning messages, so the model can mark a message that opened this
//! turn and nothing else: not an old message, not an invented id, not
//! another channel's, not a join notice — a join is its own block kind and
//! is never among the chat messages this check reads — and not an
//! unsummoned bot's line, which summons nothing since decision 0153.
//! Executing the tool appends a [`MessageMark`] block carrying the target
//! origin, the marked message's principal id and the chosen emoji; the
//! consumer's outbound edge places the reaction on the platform.
//!
//! The window the model READS is wider than the set it may aim at, so no
//! wording here says otherwise: a join notice and an unsummoned bot's
//! message are both projected, bracketed id and all, while neither is a
//! co-summoner. The decline therefore states what the model may react to,
//! never what it is reading — a decline that claimed the model was not
//! reading a line the projection just showed it would be false in two
//! shapes it can reach on any turn.
//!
//! The emoji is CONTENT, not vocabulary. The core records the string
//! exactly as it records answer text and owns no emoji list of its own —
//! which platform tokens are placeable is a platform fact, and the adapter
//! is where it lives. What the core owns is the BOUND: a non-empty
//! argument of at most [`EMOJI_BYTE_LIMIT`] bytes, taught in the refusal
//! and mirrored by the stored table's own CHECK.
//!
//! A mark the platform cannot carry is dropped by the adapter with a log
//! line and the model is never told: the tool has already returned, and an
//! act whose whole point is being cheap earns no delivery report. The
//! composite is stated plainly, as the accepted consequence it is — a
//! mis-picked emoji files the mark, the drop loses it, and the per-origin
//! existence check then refuses every later attempt on that message, so
//! one bad pick permanently unmarks it, the same accepted permanence as
//! the loss of a mark undelivered at process death.
//!
//! Filings are bounded per ORIGIN, the report's own shape: the filing
//! scans the loaded ledger for an existing mark of the named origin and
//! declines a duplicate. That is what makes "one reaction per message"
//! true for as long as the mark stands — and only for that long, which is
//! deliberate: an erasure or the deletion mirror empties the stored
//! origin, the scan then matches nothing, and a later turn may react to
//! that message again. Erasure must leave no shadow saying something was
//! here, and a later mark is a fresh act, not the old one returning.
//!
//! The overlap with a report is bounded the same way and points ONE way:
//! this tool declines an origin that already carries a report, while a
//! report filed on an already-marked message stands — the design's own
//! ruling of 2026-08-25, on the ground that a cosmetic acknowledgement
//! must never suppress a moderation assessment. So the honest statement is
//! the asymmetric one: a reaction never joins a standing report, and a
//! report may land beside a standing reaction, where it is noise on a
//! record the group can already see.
//!
//! WORDS beside a reaction are a different kind of rule, and the copy says
//! so rather than dressing it up: nothing here enforces it and nothing
//! could, since the answer is written after this tool has returned. The
//! description and the composed teaching INSTRUCT the model not to answer
//! a message it reacted to, and decision 0155's rule holds — the teaching
//! is the control, stated as a teaching.
//!
//! The scan-then-append pair runs under the shared filing door — the
//! crate-private `filing` module, which states its whole contract and the
//! lock order every holder obeys. That door is what makes the enforced
//! direction hold at all: the runner executes a round's calls in parallel
//! tasks, so without
//! one door a react and a report naming one message both scan before
//! either appends. The door also orders this filing against the deletion
//! mirror's nulls, which the erasure fence cannot — both take the fence
//! for READING. The fence is held across the whole filing for the reach it
//! does have: the person-wide erasure takes it exclusively, so a mark
//! cannot re-materialize an origin THAT operation just nulled. The marked
//! message's principal id is stored precisely so erasure can reach the
//! block, through the crate-private `erase_marked_origin` pass the erasure
//! operation composes.
//!
//! The assistant's own message needs no refusal of its own: her voice
//! writes no chat rows, so her message ids are never among the
//! co-summoners, and the anti-aiming decline catches the attempt before
//! anything else is read.

use std::sync::Arc;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, CoreEvent, FromBlock, LeafKind,
    Projection, StoreError, ToolContext, ToolHandler, ToolOutcome,
};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::filing::FilingDoor;
use crate::kind::AssistantKind;
use crate::message::Authority;
use crate::tools::provenance::co_summoners;

// ─── The block kind ──────────────────────────────────────────────────────

/// The stored type string of the message-mark kind.
pub const MESSAGE_MARK_KIND: &str = "message_mark";

/// The content table the kind's descriptor owns.
pub const MESSAGE_MARK_TABLE: &str = "block_message_mark";

/// The marked record's platform origin — what the edge places the reaction
/// on. Nullable for exactly two reasons, both erasure's: the marked
/// person's erasure nulls it, and the deletion mirror nulls it when an
/// administrator removes the marked message. The edge skips a targetless
/// mark.
pub const COLUMN_TARGET_ORIGIN: &str = "target_origin";
/// The marked person in the identity tables — stored precisely so erasure
/// can reach this block by the marked principal. NOT NULL: a mark erasure
/// could never reach must not exist, so the tool declines a record naming
/// nobody.
pub const COLUMN_MARKED_PRINCIPAL_ID: &str = "marked_principal_id";
/// The emoji the model chose, stored verbatim within its bound. Content,
/// exactly as an answer's text is content — which is why erasure leaves
/// it: it says what the ASSISTANT expressed and names nobody. An emptied
/// mark states nothing to the model either way, the block projecting
/// nothing at all.
pub const COLUMN_EMOJI: &str = "emoji";

/// One filed mark awaiting placement. Absences are typed per the kind
/// contract: a nulled target origin is the one absence with stored meaning
/// — erased, and therefore unplaceable.
#[derive(Debug, Clone)]
pub struct MessageMark {
    /// The marked message's origin. `None` after the marked person's
    /// erasure or the deletion mirror's pass — the edge skips the mark —
    /// or for a row the store did not produce.
    pub target_origin: Option<String>,
    /// The marked person. `None` only for a row the store did not produce:
    /// the column is NOT NULL.
    pub marked_principal_id: Option<i64>,
    /// The chosen emoji. `None` only for a row the store did not produce.
    pub emoji: Option<String>,
}

impl MessageMark {
    /// The stored shape of one mark block: the field map the tool's append
    /// carries, named by the same columns [`LeafKind::parse`] reads back —
    /// both sides of the kind's encoding live in this module, so a column
    /// rename cannot split them.
    #[must_use]
    pub fn stored_fields(
        target_origin: &str,
        marked_principal_id: i64,
        emoji: &str,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TARGET_ORIGIN.into(), json!(target_origin));
        fields.insert(
            COLUMN_MARKED_PRINCIPAL_ID.into(),
            json!(marked_principal_id),
        );
        fields.insert(COLUMN_EMOJI.into(), json!(emoji));
        fields
    }
}

impl LeafKind for MessageMark {
    const KINDS: &'static [&'static str] = &[MESSAGE_MARK_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: MESSAGE_MARK_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[MESSAGE_MARK_KIND],
        columns: &[
            Column::new(COLUMN_TARGET_ORIGIN, ColumnType::Text),
            Column::new(COLUMN_MARKED_PRINCIPAL_ID, ColumnType::Integer),
            Column::new(COLUMN_EMOJI, ColumnType::Text),
        ],
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            target_origin: block
                .fields
                .get(COLUMN_TARGET_ORIGIN)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            marked_principal_id: block
                .fields
                .get(COLUMN_MARKED_PRINCIPAL_ID)
                .and_then(Value::as_i64),
            emoji: block
                .fields
                .get(COLUMN_EMOJI)
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    }
}

/// Agency-inert, and frontier-transparent on purpose: the block is written
/// INTO a live turn's window by the tool, so the owed-turn decision must
/// read through it — and a mark is placed precisely on turns that answer
/// nothing, so it stands at the ledger tail more often than a report does.
/// Taking the default would bury an unanswered question behind a reaction.
impl Agency for MessageMark {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// Invisible to the model in every mode: the filed mark is machinery, and
/// the model's knowledge of it is the tool result.
impl Projection for MessageMark {}

/// Where this kind holds a copy of ANOTHER record's platform id: the
/// marked message's origin. The column is the kind's own; the deletion
/// mirror's pass over it is driven by the erasure composition, which knows
/// when a deletion removed a record.
pub(crate) const TARGET_ORIGIN_SITE: crate::erasure::ReferenceSite =
    crate::erasure::ReferenceSite {
        table: MESSAGE_MARK_TABLE,
        column: COLUMN_TARGET_ORIGIN,
    };

/// Null the target origin of every mark naming this principal as the
/// marked person — erasure's whole reach into this kind. The reaction
/// already visible in the chat is not withdrawn: doing so would need a
/// network call from inside an operation that is store-only by design, and
/// would rest on an unproven platform behaviour. The residual is stated in
/// the records of processing instead of hidden. The emoji stays; it says
/// what the assistant expressed and names nobody. Nulling already-null
/// columns is a no-op, so the step is idempotent.
///
/// One pass reaches every mark, unlike the report's two: a mark's stored
/// principal IS the author of the marked message, never a third party's,
/// so there is no filing this key cannot match.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_marked_origin(tx: &StoreTx, principal_id: i64) -> Result<(), StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        conn.execute(
            &format!(
                "UPDATE {MESSAGE_MARK_TABLE} SET {COLUMN_TARGET_ORIGIN} = NULL \
                 WHERE {COLUMN_MARKED_PRINCIPAL_ID} = ?1"
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

/// Null the target of every mark in one conversation that names the
/// deleted origin — the deletion mirror's reach into this kind, decision
/// 0085's rule applied to another holder of a copy: deleting the marked
/// message takes the record the reaction sits on, and the copy left behind
/// would be an identifier no later erasure could reach. The unplaceable
/// mark is skipped by the edge, exactly as after the marked person's
/// erasure.
///
/// Returns how many targets were nulled. Idempotent: a second run finds
/// nothing left naming the origin.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_mark_references_naming(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<usize, StoreError> {
    crate::erasure::null_references_to(tx, TARGET_ORIGIN_SITE, conversation_id, origin).await
}

// ─── The tool ────────────────────────────────────────────────────────────

/// The registered name the model calls the tool by. The name says what the
/// tool does: it reacts to a message with an emoji of the model's own
/// choosing.
pub const NAME: &str = "react";

/// The authority this tool requires — member: the turn behind a reaction
/// is summoned by ordinary members' messages. The admission check supplies
/// no extra protection at this bar; the tool sits under it because every
/// tool does (stated, not implied).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The target parameter: the message named by the id the projection shows
/// in brackets ahead of it.
pub const PARAMETER_MESSAGE_ID: &str = "message_id";

/// The vocabulary parameter: the emoji itself. The core states the bound
/// and nothing else about it — which emoji a platform can place is the
/// adapter's fact.
pub const PARAMETER_EMOJI: &str = "emoji";

/// The most bytes a stored emoji may take. Every entry of every platform
/// reaction set this repository knows fits well inside it, joined
/// sequences included; the bound exists so an unbounded string can never
/// enter the ledger through a vocabulary parameter. The stored table's own
/// CHECK is this number's schema twin.
pub const EMOJI_BYTE_LIMIT: usize = 32;

/// The filed result the model reads. It states what the ledger knows and
/// nothing beyond it: the mark is FILED. Not that it arrives — the edge
/// places it on the next wake, a platform the emoji does not suit drops it
/// silently, and a process that dies first loses it, none of which this
/// tool learns. No never-again teaching for the TOOL — a turn reading
/// several messages may react to more than one, each through its own
/// call — and an explicit one for THIS message, which takes a reaction at
/// most once while its mark stands.
pub const MARKED_RESULT: &str = "The reaction is filed. Do not react to this message again, \
     and do not also answer it in words.";

/// The anti-aiming decline: the named origin is not among the messages
/// that opened this turn — an old message, an arbitrary id, another
/// channel's, a join notice, an unsummoned bot's line — so it is not one
/// the model may react to. This is also what refuses the assistant's own
/// message: her voice writes no chat rows, so her ids are never in the set.
///
/// It speaks of what may be reacted to, not of what the model is reading,
/// and the distinction is load-bearing: the projection shows a join notice
/// and an unsummoned bot's message with their bracketed ids, so a decline
/// telling the model it is not reading one of those would state a
/// falsehood about a line right in front of it.
pub const NOT_ASSESSED_ERROR: &str = "declined: that message is not one you may react to this \
     turn — a reaction lands only on a message that opened this turn. Do not call this tool \
     again this turn; answer from what you already have.";

/// The duplicate decline: a reaction of the named origin already stands in
/// this conversation, and a message takes one reaction at most. Written in
/// the LEDGER's tense — a reaction is filed for that message — because
/// that is what the scan read: whether the reaction ever reached the chat
/// is a fact no filing knows, and a decline asserting one would be a claim
/// about the platform.
pub const ALREADY_MARKED_ERROR: &str = "declined: a reaction is already filed for that \
     message, and a message takes at most one. Do not call this tool again this turn; \
     answer from what you already have.";

/// The report-overlap decline: the named message is already reported, and
/// a reaction beside a filed report is noise on a moderation record.
pub const ALREADY_REPORTED_ERROR: &str = "declined: that message is already reported, and a \
     reported message takes no reaction. Do not call this tool again this turn; answer from \
     what you already have.";

/// The missing-target decline: the call named no message id, and a
/// reaction names its target.
pub const NEEDS_TARGET_ERROR: &str = "declined: a reaction names its target — the id shown in \
     brackets ahead of the message. Do not call this tool again this turn; answer from what \
     you already have.";

/// The unrecorded-target refusal: the named message resolves to no
/// recorded principal — a row the store did not produce whole — so no mark
/// can name a person erasure could later reach, and filing one would ship
/// an identifier out of erasure's reach (the exact gap decision 0003
/// exists to prevent).
pub const UNRECORDED_TARGET_ERROR: &str = "declined: the named message is not in the \
     assistant's records, so no reaction can name it. Do not call this tool again this \
     turn; answer from what you already have.";

/// The emoji refusal, naming the bound: an absent argument, a non-string,
/// an empty one, and one past [`EMOJI_BYTE_LIMIT`] are one refusal,
/// because each of them is the same missing thing — a usable emoji.
pub const NEEDS_EMOJI_ERROR: &str = "declined: a reaction is one emoji, given as text, at \
     most 32 bytes long. Do not call this tool again this turn; answer from what you \
     already have.";

/// The transient failure: a read or the append did not stand, so nothing
/// was filed. No no-retry line — the fact may not hold beyond this
/// failure, and the per-origin dedup finds nothing filed, so a later turn
/// files cleanly.
fn transient_error() -> String {
    "the reaction could not be filed right now; nothing was filed.".to_owned()
}

/// What one call named: the trimmed, non-empty message id, and the emoji
/// exactly as the model wrote it.
///
/// The id is trimmed because it is an identifier the projection showed and
/// whitespace around it is a transcription artifact. The emoji is NOT
/// trimmed: it is content, and the core stores content verbatim. A
/// whitespace-carrying pick is therefore stored as written and drops at
/// the adapter's membership check, which is the same accepted outcome as
/// any other unplaceable pick.
struct NamedMark {
    origin: String,
    emoji: String,
}

/// Read one call's input. `Err` is the refusal the model reads: every
/// unusable target shape — a missing field, a non-string, an empty id,
/// input that is not a JSON object — is [`NEEDS_TARGET_ERROR`], and every
/// unusable emoji shape, the byte bound included, is [`NEEDS_EMOJI_ERROR`].
/// The target is read first, so a call naming neither is told about the
/// target it must name.
fn named_mark(input: &str) -> Result<NamedMark, &'static str> {
    let value: Value = serde_json::from_str(input).map_err(|_| NEEDS_TARGET_ERROR)?;
    let origin = value
        .get(PARAMETER_MESSAGE_ID)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .ok_or(NEEDS_TARGET_ERROR)?
        .to_owned();
    let emoji = value
        .get(PARAMETER_EMOJI)
        .and_then(Value::as_str)
        .filter(|emoji| !emoji.is_empty() && emoji.len() <= EMOJI_BYTE_LIMIT)
        .ok_or(NEEDS_EMOJI_ERROR)?
        .to_owned();
    Ok(NamedMark { origin, emoji })
}

/// The pure target resolution over one loaded ledger, in the order of the
/// claims: the named origin must belong to the messages that opened this
/// turn, the anti-aiming bound — which is also what refuses a join notice,
/// another channel's message, an unsummoned bot's line and the assistant's
/// own words, none of which are co-summoners in this set, however visible
/// the first two are to the model; the record must name someone
/// erasure could reach; no mark of the origin may already stand, the
/// per-origin dedup that makes one-reaction-ever true; and no report of it
/// may stand either, because a reaction beside a moderation record is
/// noise on it. `Ok` is the marked principal; `Err` is the decline the
/// model reads.
fn resolve_markable(
    ledger: &[Block],
    call_block_id: i64,
    origin: &str,
) -> Result<i64, &'static str> {
    let Some(target) = co_summoners(ledger, call_block_id)
        .into_iter()
        .find(|message| message.origin.as_deref() == Some(origin))
    else {
        return Err(NOT_ASSESSED_ERROR);
    };
    let marked = target.principal_id.ok_or(UNRECORDED_TARGET_ERROR)?;
    for block in ledger {
        match AssistantKind::from_block(block) {
            AssistantKind::MessageMark(mark) if mark.target_origin.as_deref() == Some(origin) => {
                return Err(ALREADY_MARKED_ERROR);
            }
            AssistantKind::Report(report) if report.target_origin.as_deref() == Some(origin) => {
                return Err(ALREADY_REPORTED_ERROR);
            }
            _ => {}
        }
    }
    Ok(marked)
}

/// The react tool: member authority, two validated parameters, every
/// conversation. Constructed by the assembly unconditionally — a reaction
/// needs nothing but a chat — with the erasure fence and the shared filing
/// door injected here, at registration, so the tool never reaches into the
/// assembly.
pub(crate) struct MarkTool {
    /// The erasure fence, held shared across the resolution and the append
    /// so a mark cannot re-materialize an origin the person-wide erasure
    /// just nulled — that operation takes the fence exclusively. Taken as
    /// the bare shared lock, not as the assembly's own alias for it — a
    /// leaf tool names nothing in the module that registers it.
    fence: Arc<RwLock<()>>,
    /// The shared filing door: one scan-then-append at a time, across
    /// every writer that files against a message origin — the sibling
    /// react call, the report tool, and the deletion mirror. Its whole
    /// contract, the lock order included, is stated in the crate-private
    /// `filing` module.
    door: FilingDoor,
}

impl MarkTool {
    pub(crate) fn new(fence: Arc<RwLock<()>>, door: FilingDoor) -> Self {
        Self { fence, door }
    }

    /// The whole filing, under the erasure fence and the filing door, in
    /// that order — the one the door's module states and every holder of
    /// both obeys. `Err` carries the tool error the runner records and the
    /// model reads.
    async fn file(
        &self,
        ctx: &ToolContext<'_, CoreEvent>,
        named: &NamedMark,
    ) -> Result<&'static str, String> {
        let _no_erasure_mid_filing = self.fence.read().await;
        let _one_filing_at_a_time = self.door.lock().await;
        let conversation_id = ctx.agency.conversation_id;
        let ledger = match ctx.agency.store.list_blocks(conversation_id).await {
            Ok(ledger) => ledger,
            Err(error) => {
                tracing::warn!(conversation_id, %error, "the reaction's ledger read failed");
                return Err(transient_error());
            }
        };
        let marked_principal_id = match resolve_markable(&ledger, ctx.block_id, &named.origin) {
            Ok(principal) => principal,
            Err(decline) => return Err(decline.to_owned()),
        };
        let appended = ctx
            .agency
            .store
            .append_consumer_block(
                conversation_id,
                None,
                MESSAGE_MARK_KIND,
                MessageMark::stored_fields(&named.origin, marked_principal_id, &named.emoji),
                None,
            )
            .await;
        if let Err(error) = appended {
            tracing::warn!(conversation_id, %error, "the reaction's append failed; nothing filed");
            return Err(transient_error());
        }
        Ok(MARKED_RESULT)
    }
}

impl ToolHandler<CoreEvent> for MarkTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "Put one emoji reaction on a message, instead of writing a reply \
                 that would add nothing. Name the message by its id, shown in brackets \
                 ahead of it; it must be one of the messages that opened this turn, which \
                 is not every message you can see. Give the emoji you choose. React to a \
                 message at most once — a second call naming it is declined — and never \
                 react to a message you also answer in words."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    PARAMETER_MESSAGE_ID: {
                        "type": "string",
                        "description": "The message's id, exactly as shown in brackets \
                             ahead of it"
                    },
                    PARAMETER_EMOJI: {
                        "type": "string",
                        "description": "The single emoji to put on that message, as text"
                    }
                },
                "required": [PARAMETER_MESSAGE_ID, PARAMETER_EMOJI]
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(NAME, REQUIRED_AUTHORITY);

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            let named = match named_mark(input) {
                Ok(named) => named,
                Err(refusal) => return ToolOutcome::Error(refusal.to_owned()),
            };
            match self.file(&ctx, &named).await {
                Ok(filed) => ToolOutcome::Done(filed.into()),
                Err(error) => ToolOutcome::Error(error),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use agent_ledger::Role;

    use super::*;
    use crate::tools::admission::NO_RETRY;

    fn mark_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: MESSAGE_MARK_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// AC-D's storage half at the encoding seam: an accepted emoji is
    /// stored verbatim and reads back byte for byte, beside the target and
    /// the marked person.
    #[test]
    fn the_stored_fields_round_trip_and_the_emoji_is_verbatim() {
        let mark = MessageMark::parse(&mark_block(MessageMark::stored_fields(
            "origin-77",
            42,
            "\u{1F44D}",
        )));
        assert_eq!(mark.target_origin.as_deref(), Some("origin-77"));
        assert_eq!(mark.marked_principal_id, Some(42));
        assert_eq!(
            mark.emoji.as_deref(),
            Some("\u{1F44D}"),
            "the chosen emoji is stored exactly as the model wrote it"
        );

        let joined = MessageMark::parse(&mark_block(MessageMark::stored_fields(
            "origin-78",
            43,
            "\u{2764}\u{FE0F}\u{200D}\u{1F525}",
        )));
        assert_eq!(
            joined.emoji.as_deref(),
            Some("\u{2764}\u{FE0F}\u{200D}\u{1F525}"),
            "a joined sequence keeps every codepoint, selector included"
        );
    }

    /// The kind is inert, transparent and invisible: it summons nothing,
    /// the owed-turn frontier reads through it, it is a durable ledger
    /// row, and it shows the model nothing at all.
    #[test]
    fn a_mark_is_inert_transparent_and_invisible() {
        let mark = MessageMark::parse(&mark_block(MessageMark::stored_fields("o", 1, "x")));
        assert_eq!(mark.awaiting(), None, "a mark summons nothing");
        assert!(
            mark.frontier_transparent(),
            "the owed-turn frontier reads through it"
        );
        assert!(mark.durable(), "a mark is a durable ledger row");
        assert_eq!(mark.group_role(), None, "invisible to projection");
        assert_eq!(mark.llm_text(), None);
        assert_eq!(mark.llm_parts(), None);
    }

    /// AC-D's bound at the parameter seam: a well-formed call answers its
    /// trimmed id and its verbatim emoji; every unusable target shape is
    /// the target refusal; and an absent, non-string, empty or over-bound
    /// emoji is the taught bound refusal. The boundary is pinned on both
    /// sides — thirty-two bytes pass, thirty-three refuse — so the bound
    /// is the stated number and not an approximation of it.
    #[test]
    fn the_parameters_read_the_id_and_the_emoji_and_refuse_every_unusable_shape() {
        let well_formed = json!({
            PARAMETER_MESSAGE_ID: "  origin-9  ",
            PARAMETER_EMOJI: "\u{1F44D}",
        })
        .to_string();
        let named = named_mark(&well_formed).expect("a well-formed call reads");
        assert_eq!(named.origin, "origin-9", "the id is trimmed");
        assert_eq!(named.emoji, "\u{1F44D}", "the emoji is not");

        for unusable in [
            "{}",
            r#"{"emoji":"x"}"#,
            r#"{"message_id":"","emoji":"x"}"#,
            r#"{"message_id":"   ","emoji":"x"}"#,
            r#"{"message_id":7,"emoji":"x"}"#,
            "not json",
            "",
        ] {
            assert_eq!(
                named_mark(unusable).err(),
                Some(NEEDS_TARGET_ERROR),
                "the target refusal covers: {unusable:?}"
            );
        }

        let at_the_bound = "a".repeat(EMOJI_BYTE_LIMIT);
        assert_eq!(
            named_mark(&format!(r#"{{"message_id":"o","emoji":"{at_the_bound}"}}"#))
                .expect("the bound itself is accepted")
                .emoji,
            at_the_bound
        );
        let past_the_bound = "a".repeat(EMOJI_BYTE_LIMIT + 1);
        for unusable in [
            r#"{"message_id":"o"}"#.to_owned(),
            r#"{"message_id":"o","emoji":""}"#.to_owned(),
            r#"{"message_id":"o","emoji":7}"#.to_owned(),
            format!(r#"{{"message_id":"o","emoji":"{past_the_bound}"}}"#),
        ] {
            assert_eq!(
                named_mark(&unusable).err(),
                Some(NEEDS_EMOJI_ERROR),
                "the bound refusal covers: {unusable:?}"
            );
        }
    }

    /// AC-D's copy half: every fixed result pinned verbatim. The filed
    /// result claims FILING and nothing past it — the arrival is the
    /// edge's, the platform's and the process's, none of which this tool
    /// hears back from — and it teaches the two rules that bound a
    /// reaction; every decline closes with the admission module's
    /// no-retry teaching; the emoji refusal names the bound in words; and
    /// the transient failure names the moment and teaches no never-again.
    #[test]
    fn the_result_wording_is_pinned_verbatim() {
        assert_eq!(
            MARKED_RESULT,
            "The reaction is filed. Do not react to this message again, and do not also \
             answer it in words."
        );
        for unfiled_claim in ["goes out", "sent", "placed", "arrives"] {
            assert!(
                !MARKED_RESULT.contains(unfiled_claim),
                "the filed result states what the ledger knows and claims no delivery: \
                 {unfiled_claim}"
            );
        }
        assert_eq!(
            NOT_ASSESSED_ERROR,
            "declined: that message is not one you may react to this turn — a reaction lands \
             only on a message that opened this turn. Do not call this tool again this turn; \
             answer from what you already have."
        );
        assert!(
            !NOT_ASSESSED_ERROR.contains("reading"),
            "the decline states what may be reacted to, never what the model is reading: a \
             join notice and an unsummoned bot's message are both projected with their \
             bracketed ids while neither is markable, so the reading claim would be false"
        );
        assert_eq!(
            ALREADY_MARKED_ERROR,
            "declined: a reaction is already filed for that message, and a message takes at \
             most one. Do not call this tool again this turn; answer from what you already \
             have."
        );
        assert_eq!(
            ALREADY_REPORTED_ERROR,
            "declined: that message is already reported, and a reported message takes no \
             reaction. Do not call this tool again this turn; answer from what you already \
             have."
        );
        assert_eq!(
            NEEDS_TARGET_ERROR,
            "declined: a reaction names its target — the id shown in brackets ahead of the \
             message. Do not call this tool again this turn; answer from what you already \
             have."
        );
        assert_eq!(
            UNRECORDED_TARGET_ERROR,
            "declined: the named message is not in the assistant's records, so no reaction \
             can name it. Do not call this tool again this turn; answer from what you \
             already have."
        );
        assert_eq!(
            NEEDS_EMOJI_ERROR,
            "declined: a reaction is one emoji, given as text, at most 32 bytes long. Do \
             not call this tool again this turn; answer from what you already have."
        );
        assert!(
            NEEDS_EMOJI_ERROR.contains(&EMOJI_BYTE_LIMIT.to_string()),
            "the emoji refusal states the bound the code enforces"
        );
        for closes_with_no_retry in [
            NOT_ASSESSED_ERROR,
            ALREADY_MARKED_ERROR,
            ALREADY_REPORTED_ERROR,
            NEEDS_TARGET_ERROR,
            UNRECORDED_TARGET_ERROR,
            NEEDS_EMOJI_ERROR,
        ] {
            assert!(
                closes_with_no_retry.ends_with(NO_RETRY),
                "every decline closes with the no-retry teaching: {closes_with_no_retry}"
            );
        }
        assert!(
            !MARKED_RESULT.contains(NO_RETRY),
            "the filed result forbids repeating THIS reaction, not the tool: a turn \
             reading several messages may react to more than one"
        );
        let transient = transient_error();
        assert!(
            transient.contains("right now") && !transient.contains(NO_RETRY),
            "a transient fact names the moment and teaches no never-again: {transient}"
        );
    }

    /// One synthetic chat row for the resolution pins: origin, voice,
    /// principal and the summoned stamp, everything else absent.
    fn chat_row(
        id: i64,
        role: Role,
        origin: &str,
        principal: Option<i64>,
        addressed: bool,
    ) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("text".into(), json!("a recorded line"));
        fields.insert("origin".into(), json!(origin));
        fields.insert("authority".into(), json!("member"));
        fields.insert("addressed".into(), json!(addressed));
        fields.insert("answer_due".into(), json!(addressed));
        if let Some(principal) = principal {
            fields.insert("principal_id".into(), json!(principal));
        }
        Block {
            id,
            role: Some(role),
            block_type: crate::kind::CHAT_MESSAGE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// One loaded call block anchored on the given id.
    fn call_row(id: i64, anchor: i64) -> Block {
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "tool_call".into(),
            created_at: String::new(),
            dispatch_anchor: Some(anchor),
            fields: serde_json::Map::new(),
        }
    }

    /// AC-D's aiming half, every claim in its order: a message this turn
    /// reads resolves to its principal; an origin outside the set — a
    /// bystander's included — declines as not-read; a row without a
    /// recorded principal (a shape the schema's NOT NULL keeps out of
    /// every stored ledger, so it is pinned at the pure seam where it is
    /// reachable) declines as unrecorded; a standing mark declines the
    /// repeat; and a standing report declines the reaction beside it.
    #[test]
    fn the_resolution_validates_the_set_the_record_and_both_dedups() {
        let mut ledger = vec![
            chat_row(1, Role::User, "origin-bystander", Some(3), false),
            chat_row(2, Role::User, "origin-anchor", Some(5), true),
            chat_row(3, Role::User, "origin-share", Some(7), true),
            chat_row(4, Role::User, "origin-broken", None, true),
            call_row(9, 2),
        ];
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-share"),
            Ok(7),
            "a message this turn reads resolves to its recorded principal"
        );
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-anchor"),
            Ok(5),
            "the anchor's own message is one the turn reads too"
        );
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-nobody"),
            Err(NOT_ASSESSED_ERROR),
            "an arbitrary id is not one the turn is reading"
        );
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-bystander"),
            Err(NOT_ASSESSED_ERROR),
            "a bystander's line co-summons nothing and cannot be aimed at"
        );
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-broken"),
            Err(UNRECORDED_TARGET_ERROR),
            "a row without a recorded principal names nobody erasure can reach"
        );

        ledger.push(Block {
            id: 5,
            role: None,
            block_type: MESSAGE_MARK_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: Some(2),
            fields: MessageMark::stored_fields("origin-share", 7, "\u{1F44D}"),
        });
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-share"),
            Err(ALREADY_MARKED_ERROR),
            "one reaction per message, ever"
        );
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-anchor"),
            Ok(5),
            "the dedup is per origin: another message still files"
        );

        ledger.push(Block {
            id: 6,
            role: None,
            block_type: crate::tools::report::REPORT_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: Some(2),
            fields: crate::tools::report::Report::stored_fields(
                "origin-anchor",
                Some(5),
                "/report@m",
            ),
        });
        assert_eq!(
            resolve_markable(&ledger, 9, "origin-anchor"),
            Err(ALREADY_REPORTED_ERROR),
            "a reported message takes no reaction beside the report"
        );
    }

    /// The two shapes the model can SEE and cannot mark: a join notice and
    /// an unsummoned bot's message. The assistant's OWN message is the
    /// third case this one refusal answers and the reason no separate
    /// own-message decline exists: her voice writes no chat rows, so an id
    /// of hers is an id outside the set, which the arbitrary-id case above
    /// already pins. Both are projected with their bracketed
    /// ids — the join in the system voice, the bot's line as an ordinary
    /// user row summoning nothing since decision 0153 — and neither is a
    /// co-summoner, so both take the anti-aiming decline with no clause of
    /// their own. This is the reachable case the decline's wording is
    /// written for: it says what the model may react to, because saying the
    /// model is not reading these would be false.
    #[test]
    fn a_join_notice_and_an_unsummoned_bot_line_are_visible_and_still_unmarkable() {
        let ledger = vec![
            chat_row(1, Role::User, "origin-anchor", Some(5), true),
            // The unsummoned bot's line: recorded and projected exactly as
            // any member's, stamped unaddressed, summoning nothing.
            chat_row(2, Role::User, "origin-bot-line", Some(11), false),
            Block {
                id: 3,
                role: None,
                block_type: crate::join::JOIN_NOTICE_KIND.into(),
                created_at: String::new(),
                dispatch_anchor: Some(1),
                fields: crate::join::JoinNotice::stored_fields(
                    crate::join::RecordedJoiner {
                        principal_id: 13,
                        name: "A joiner",
                        handle: Some("joiner"),
                    },
                    "origin-join",
                    "2026-08-30T00:00:00Z",
                ),
            },
            call_row(9, 1),
        ];
        for visible_but_unmarkable in ["origin-bot-line", "origin-join"] {
            assert_eq!(
                resolve_markable(&ledger, 9, visible_but_unmarkable),
                Err(NOT_ASSESSED_ERROR),
                "the model reads it and still may not mark it: {visible_but_unmarkable}"
            );
        }
    }

    /// The definition teaches the validated shape: both parameters, the
    /// turn bound, the one-reaction rule and the no-words-beside-it rule.
    ///
    /// Both rules are written as INSTRUCTIONS, and one of them says what
    /// enforces it: a repeat call is declined, while nothing in the
    /// mechanism can stop words landing beside a reaction — the model
    /// decides that, and decision 0155 forbids dressing a teaching up as a
    /// mechanism. So the description is asserted free of the fact-shaped
    /// phrasing that would claim otherwise.
    #[test]
    fn the_definition_teaches_both_parameters_and_both_bounds() {
        let definition =
            MarkTool::new(Arc::new(RwLock::new(())), crate::filing::door()).definition();
        assert_eq!(definition.name, NAME);
        assert_eq!(definition.name, "react");
        for instruction in [
            "instead of writing a reply that would add nothing",
            "one of the messages that opened this turn",
            "not every message you can see",
            "React to a message at most once",
            "a second call naming it is declined",
            "never react to a message you also answer in words",
        ] {
            assert!(
                definition.description.contains(instruction),
                "the description carries: {instruction}"
            );
        }
        for asserted_as_mechanism in ["A message takes at most one reaction, ever", "takes none"] {
            assert!(
                !definition.description.contains(asserted_as_mechanism),
                "the description instructs and never states an unenforced rule as a fact \
                 about the world: {asserted_as_mechanism}"
            );
        }
        let required = definition.parameters["required"]
            .as_array()
            .expect("the schema names its required list");
        assert_eq!(
            required,
            &[json!(PARAMETER_MESSAGE_ID), json!(PARAMETER_EMOJI)],
            "both parameters are required: a reaction is a target and an emoji"
        );
    }
}
