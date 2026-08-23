//! The assistant's block kind, composed with the framework's kinds.
//!
//! One consumer kind exists in this unit: [`ChatMessage`], a recorded channel
//! message. Its descriptor declares the content table, its projection renders
//! the message for the model, and its agency hook makes a message await a
//! turn exactly when its stored answer-due stamp says one is owed — the
//! acting policy is record all, answer some, and the stamp is decided once by
//! the entry point at the write.
//!
//! The content table is also the personal-data table of decision 0003: the
//! block header row is the immutable ledger entry, the content row carries the
//! personal payload. The `text` column is nullable for exactly one reason —
//! erasure nulls it — so a message without stored text is an erased message,
//! projects only the fixed marker to the model, and awaits nothing. Because
//! that write is part of the kind's own contract, it lives here too, as the
//! crate-private `erase_principal_content` the erasure operation composes.

use std::num::NonZeroU64;
use std::sync::LazyLock;

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
/// Whether the message addressed the assistant, as the adapter resolved it.
/// Structure, not personal data: erasure leaves it.
pub const COLUMN_ADDRESSED: &str = "addressed";
/// Whether the message owes the model a turn — the write-time stamp the
/// entry point decides once at insert: true when the message is addressed
/// and no budget refused it, or when the block behind it carries an
/// unanswered answer-due, so a message arriving on the heels of an
/// addressed one propagates the debt instead of cancelling it. Structure,
/// not personal data: erasure leaves it. A decision recorded at the write,
/// not a derivable-fact column — the per-block hook that consumes it cannot
/// fold history.
pub const COLUMN_ANSWER_DUE: &str = "answer_due";
/// Which budget refused this message's own debt, in the closed [`LimitedBy`]
/// vocabulary; NULL when no budget refused — every unaddressed message, and
/// every addressed one the budgets admitted. Structure, not personal data:
/// erasure leaves it. Added by the protection migration step, so
/// pre-migration rows read NULL.
pub const COLUMN_LIMITED: &str = "limited";
/// The authority of the debt this message carries, in the [`Authority`]
/// vocabulary; NULL when the message carries no debt. Stamped at the write
/// by the minimum rule: a fresh debt opens at its sender's own authority,
/// and a carried debt takes the minimum of the tail's debt authority and
/// the incoming sender's — the lowest authority that contributed to
/// summoning the turn, recorded before the turn exists. Structure, not
/// personal data: erasure leaves it. Added by the protection migration
/// step, so a pre-migration owing tail reads NULL here and the fold in
/// [`ChatMessage::carried_debt_authority`] answers with the row's own
/// stored sender authority instead.
pub const COLUMN_DEBT_AUTHORITY: &str = "debt_authority";
/// The sender's public username as the platform delivered it at receipt —
/// the handle as it was when the person spoke, which is the historically
/// honest value (decision 0065): the projection reads one block with no
/// ledger access, so the handle the model sees must live on the row. A
/// projection fact, not an identity fact — the identity tables keep owning
/// who is who. Personal data: the author-keyed erasure pass nulls it
/// beside the text and the origin. NULL for a sender the platform gave no
/// handle, for a handle outside [`storable_speaker`]'s bound, for every
/// pre-migration row, and for an erased row alike; a NULL speaker projects
/// bare. Added by the speaker migration step.
pub const COLUMN_SPEAKER: &str = "speaker";

/// Whether a delivered handle may be stored as the speaker. The stored
/// handle becomes the projected prefix `speaker: text`, so the prefix must
/// be unambiguous, and three shapes would blur it: an empty handle projects
/// a bare `: text` line, a handle carrying the prefix separator (':')
/// projects a double colon nothing downstream can parse apart — the shape
/// of a second platform's fully-qualified ids — and a whitespace-bearing
/// handle lets one handle read as two. The current platform's username
/// alphabet can produce none of these; a second platform's could, and the
/// core owns this bound instead of trusting every adapter. A refused handle
/// stores NULL, exactly like no handle at all: the message projects bare
/// and no substitute identifier is minted (decision 0056).
#[must_use]
pub fn storable_speaker(handle: &str) -> bool {
    !handle.is_empty() && !handle.contains(':') && !handle.chars().any(char::is_whitespace)
}
/// The platform's own id for the message this one replies to, opaque —
/// what the report tool resolves its target through and what reply
/// threading names (decided 2026-08-23). NULL for a non-reply, for a reply
/// without a usable id, and for a reply to one of the assistant's own
/// messages. Personal data of TWO people at once: the row's author chose
/// to store it, and its value is the replied-to person's own message
/// identifier — so erasure reaches it from both ends: the author-keyed
/// pass nulls it with the rest of the row, and the crate-private
/// target-keyed pass `erase_reply_targets_naming` nulls it when the
/// replied-to person is erased. The target-keyed reach is exactly as wide
/// as its join: a stored value matching none of the erased person's
/// recorded origins — a reply recorded after their erasure completed,
/// one recorded inside a failed erasure's retry window, or one naming a
/// message the assistant never recorded — keeps an unreachable copy.
/// Decision 0063's refinements record that residual and its decided
/// follow-up, a reach key resolved when the reply is recorded. Added by
/// the reply-target migration step, so pre-migration rows read NULL.
pub const COLUMN_REPLY_TARGET: &str = "reply_target";
/// Whether the message replies to one of the assistant's own messages —
/// the fact the report tool's self-report refusal reads (decided
/// 2026-08-23). Structure, not personal data: erasure leaves it. Added by
/// the reply-target migration step, so pre-migration rows read NULL.
pub const COLUMN_REPLY_TO_ASSISTANT: &str = "reply_to_assistant";

/// What an erased message contributes to a projected request in place of its
/// nulled prose. Non-empty on purpose: the live vendors whose strict
/// alternation decision 0027 protects also reject a message whose content is
/// empty, and a run in which every message is erased projects exactly one
/// message built from these contributions alone. A fixed marker carries none
/// of the person's words, so the erasure's promise holds.
pub const ERASED_MARKER: &str = "[message erased]";

/// Why a message's own debt was never taken — the stored vocabulary of the
/// limited stamp. The two budget kinds read as "this message's own debt was
/// refused"; the command kind (added 2026-08-23) reads as "this message is
/// a command and takes no debt by its nature". Under every kind, a true
/// answer-due beside it is a propagated debt, not a contradiction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitedBy {
    /// The sender's own budget refused, counted globally across
    /// conversations.
    Principal,
    /// The conversation's budget refused.
    Channel,
    /// The message is a command with a deterministic answer: it opens no
    /// debt, counts against no answer window, and unlatches nothing — a
    /// pending tail debt propagates past it exactly as past any non-owing
    /// message.
    Command,
}

impl LimitedBy {
    /// Every variant, in stored-encoding order — what closes the vocabulary
    /// in the widening migration's CHECK constraint, so the constraint and
    /// this enum cannot drift apart. The protection unit's shipped step
    /// quotes its own frozen two-kind list instead, per the appended-steps
    /// discipline.
    pub const ALL: [Self; 3] = [Self::Principal, Self::Channel, Self::Command];

    /// The stored encoding, a closed vocabulary.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Principal => "principal",
            Self::Channel => "channel",
            Self::Command => "command",
        }
    }

    /// Parse the stored encoding back, `None` for anything outside the
    /// vocabulary.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        match stored {
            "principal" => Some(Self::Principal),
            "channel" => Some(Self::Channel),
            "command" => Some(Self::Command),
            _ => None,
        }
    }
}

/// The debt an owing tail hands the next message's stamp: the authority its
/// unanswered answer-due carries, already folded through the pre-migration
/// rule by [`ChatMessage::carried_debt_authority`]. `authority` is absent
/// only for a tail block the store did not produce — a stored owing tail
/// always folds to some authority — and such a tail contributes nothing to
/// the minimum rule.
#[derive(Debug, Clone, Copy)]
pub struct TailDebt {
    /// The carried debt's authority, folded.
    pub authority: Option<Authority>,
}

/// The sender as one write records them: the principal id the identity
/// tables resolved, the standing at receipt, and the public username the
/// platform delivered — three facts that enter the row together, carried
/// as one value so the append's field map cannot take them apart.
#[derive(Debug, Clone, Copy)]
pub struct RecordedSender<'a> {
    /// The resolved principal id.
    pub principal_id: i64,
    /// The sender's standing at receipt.
    pub authority: Authority,
    /// The sender's public username at receipt; `None` stores NULL — no
    /// substitute identifier is minted (decision 0056) — and a handle
    /// outside [`storable_speaker`]'s bound stores NULL the same way.
    pub speaker: Option<&'a str>,
}

/// The write-time stamp: the four facts the entry point decides once at the
/// insert, composed here so the composition rule and the minimum rule live
/// beside their readers ([`ChatMessage::owes_answer`],
/// [`ChatMessage::carried_debt_authority`]) as one pure value a test can
/// call directly.
#[derive(Debug, Clone, Copy)]
pub struct Stamp {
    /// Whether the message addressed the assistant.
    pub addressed: bool,
    /// Which budget refused the message's own debt, if any.
    pub limited: Option<LimitedBy>,
    /// Whether the message owes the model a turn — the composition rule:
    /// its own debt was taken, or a tail's debt propagates through it.
    pub answer_due: bool,
    /// The authority of the debt the message carries, absent when it
    /// carries none.
    pub debt_authority: Option<Authority>,
}

impl Stamp {
    /// Compose the stamp from the write's inputs: the message's addressed
    /// fact, its sender's authority, the first refusing budget (already
    /// `None` for unaddressed messages — budgets are consulted for
    /// addressed ones only), and the owing tail's debt if the conversation
    /// carries one.
    ///
    /// The composition rule: answer-due = (addressed and not limited) or
    /// tail-owes — a refused own debt never cancels a propagated one. The
    /// minimum rule (decision 0036): a carried debt takes the lowest
    /// authority that contributed to summoning the turn — the tail's debt
    /// authority against the incoming sender's, regardless of the incoming
    /// message's own addressed fact — and a fresh taken debt opens at its
    /// sender's own authority.
    #[must_use]
    pub fn compose(
        addressed: bool,
        sender: Authority,
        limited: Option<LimitedBy>,
        owing_tail: Option<TailDebt>,
    ) -> Self {
        let own_debt_taken = addressed && limited.is_none();
        Self {
            addressed,
            limited,
            answer_due: own_debt_taken || owing_tail.is_some(),
            debt_authority: match owing_tail {
                Some(tail) => Some(tail.authority.map_or(sender, |carried| carried.min(sender))),
                None => own_debt_taken.then_some(sender),
            },
        }
    }

    /// Whether this message's own debt was taken — addressed and no budget
    /// refused. The same conjunction [`Self::compose`] feeds the
    /// composition rule; restated here for readers of a composed stamp,
    /// like the unlatch emission: only a taken debt is re-engagement.
    #[must_use]
    pub fn own_debt_taken(&self) -> bool {
        self.addressed && self.limited.is_none()
    }
}

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
    /// by erasure, and the message projects [`ERASED_MARKER`] to the model,
    /// never the prose.
    pub text: Option<String>,
    /// The sender's principal id in the identity tables. `None` only for a
    /// block the store did not produce (the schema stores it NOT NULL);
    /// predicates matching on it reject `None` instead of comparing it.
    pub principal_id: Option<i64>,
    /// The sender's standing at receipt. `None` only for a block the store
    /// did not produce (the schema closes the vocabulary with a CHECK).
    pub authority: Option<Authority>,
    /// The sender's public username at receipt. `None` is a stored meaning
    /// three ways: the platform gave the sender no handle, the row predates
    /// the speaker migration, or erasure nulled it — and in every one of
    /// them the message projects bare, unprefixed.
    pub speaker: Option<String>,
    /// The platform's own id for the message, opaque.
    pub origin: Option<String>,
    /// The platform's send time, RFC 3339. The store's insertion time lives
    /// on the block header. `None` only for a block the store did not
    /// produce.
    pub sent_at: Option<String>,
    /// Whether the message addressed the assistant. `None` only for a block
    /// the store did not produce (the schema stores it NOT NULL).
    pub addressed: Option<bool>,
    /// Whether the message owes the model a turn, stamped at the write.
    /// `None` only for a block the store did not produce; the awaiting hook
    /// treats that absence as owing nothing.
    pub answer_due: Option<bool>,
    /// Which budget refused this message's own debt. `None` is the stored
    /// meaning: no budget refused — pre-migration rows included.
    pub limited: Option<LimitedBy>,
    /// The authority of the debt this message carries. `None` when the
    /// message carries no debt, and on every pre-migration row; an owing
    /// pre-migration tail is folded to its stored sender authority by
    /// [`Self::carried_debt_authority`].
    pub debt_authority: Option<Authority>,
    /// The origin of the message this one replies to. `None` for a
    /// non-reply, a reply without a usable id, a reply to the assistant's
    /// own message, every pre-migration row — and for an erased row, whose
    /// author-keyed pass nulled it.
    pub reply_target: Option<String>,
    /// Whether the reply points at one of the assistant's own messages.
    /// `None` on a non-reply and on every pre-migration row.
    pub reply_to_assistant: Option<bool>,
}

impl ChatMessage {
    /// The stored shape of one recorded message: the field map a consumer
    /// append carries, named by the same columns [`LeafKind::parse`] reads
    /// back — both sides of the kind's encoding live in this module, so a
    /// column rename cannot split them. The role travels as the append's own
    /// argument, never as a field; the three sender facts travel together
    /// as the [`RecordedSender`]; the four decided facts travel together as
    /// the composed [`Stamp`]; the reply fact travels as the translated
    /// [`ReplyTarget`](crate::message::ReplyTarget), encoded into its two
    /// columns here.
    #[must_use]
    pub fn stored_fields(
        text: &str,
        sender: RecordedSender<'_>,
        origin: Option<&str>,
        reply_target: Option<&crate::message::ReplyTarget>,
        sent_at: &str,
        stamp: Stamp,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_TEXT.into(), json!(text));
        fields.insert(COLUMN_PRINCIPAL_ID.into(), json!(sender.principal_id));
        fields.insert(COLUMN_AUTHORITY.into(), json!(sender.authority.as_str()));
        if let Some(speaker) = sender.speaker.filter(|handle| storable_speaker(handle)) {
            fields.insert(COLUMN_SPEAKER.into(), json!(speaker));
        }
        if let Some(origin) = origin {
            fields.insert(COLUMN_ORIGIN.into(), json!(origin));
        }
        match reply_target {
            Some(crate::message::ReplyTarget::Message { origin }) => {
                fields.insert(COLUMN_REPLY_TARGET.into(), json!(origin));
            }
            Some(crate::message::ReplyTarget::AssistantMessage) => {
                fields.insert(COLUMN_REPLY_TO_ASSISTANT.into(), json!(true));
            }
            None => {}
        }
        fields.insert(COLUMN_SENT_AT.into(), json!(sent_at));
        fields.insert(COLUMN_ADDRESSED.into(), json!(stamp.addressed));
        fields.insert(COLUMN_ANSWER_DUE.into(), json!(stamp.answer_due));
        if let Some(limited) = stamp.limited {
            fields.insert(COLUMN_LIMITED.into(), json!(limited.as_str()));
        }
        if let Some(debt_authority) = stamp.debt_authority {
            fields.insert(COLUMN_DEBT_AUTHORITY.into(), json!(debt_authority.as_str()));
        }
        fields
    }

    /// Whether this message still owes the model a turn — the one reading of
    /// the write-time stamp, shared by the awaiting hook and the entry
    /// point's tail read so the stamp's two consumers cannot disagree. A
    /// message owes a turn when it spoke in the user's voice, was stamped
    /// answer-due at the write, and still has its text: an erased message
    /// has nothing left to answer, so erasure cancels the debt for the hook
    /// and the tail read alike — the stamp itself stays, being structure.
    #[must_use]
    pub fn owes_answer(&self) -> bool {
        self.role == Some(Role::User) && self.text.is_some() && self.answer_due == Some(true)
    }

    /// The authority of the debt an owing tail hands the next message — the
    /// stored debt authority, with the pre-migration fold: a row written
    /// before the protection migration carries NULL there while every
    /// pre-migration row does carry its sender's authority, so that stored
    /// standing answers for the missing stamp. Read on a tail
    /// [`Self::owes_answer`] affirms; a debt-free row's absent stamp folds
    /// to its sender's own authority, the row's only stored voice. Tool
    /// admission never reads this fold: the carried stamp is the ANSWERING
    /// fact, and the anchor gate walks the debt's origin set instead
    /// (decision 0043, refined 2026-08-22).
    #[must_use]
    pub fn carried_debt_authority(&self) -> Option<Authority> {
        self.debt_authority.or(self.authority)
    }

    /// Whether this message's own debt was taken — the row-side reading of
    /// the opened-debt predicate: addressed, and no budget refused. One
    /// predicate, three spellings that must agree: [`Stamp::own_debt_taken`]
    /// at the write, the budget counts' SQL fragment over the stored rows,
    /// and this reading of one loaded row — which is also decision 0043's
    /// co-summoner rule, since exactly the messages this predicate affirms
    /// join a turn's provenance when absorbed into its span. A row the
    /// store did not produce reads `None` for `addressed` and answers
    /// false: a message that provably opened nothing joins nothing.
    #[must_use]
    pub fn own_debt_taken(&self) -> bool {
        self.addressed == Some(true) && self.limited.is_none()
    }

    /// The message's projected contribution: its text, or [`ERASED_MARKER`]
    /// when erasure nulled it. Only erasure produces the absent text — the
    /// adapter never records an empty message and the schema stores the
    /// column NOT NULL — so the marker speaks exactly for erased messages.
    ///
    /// A user-voiced message with a stored speaker projects as the speaker,
    /// a colon and a space, then the text (decision 0066) — the handle the
    /// model may address the person by. Everything else projects bare: a
    /// handleless sender's message (no substitute identifier leaves the
    /// machine, per decision 0056), any non-user voice, and the erased
    /// marker — the erasure pass nulls the speaker with the text, and even
    /// a row it half-reached keeps the placeholder exactly as it is.
    fn projected_text(&self) -> String {
        let Some(text) = &self.text else {
            return ERASED_MARKER.to_owned();
        };
        match &self.speaker {
            Some(speaker) if self.role == Some(Role::User) => format!("{speaker}: {text}"),
            _ => text.clone(),
        }
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
            Column::new(COLUMN_SPEAKER, ColumnType::Text),
            Column::new(COLUMN_ORIGIN, ColumnType::Text),
            Column::new(COLUMN_SENT_AT, ColumnType::Text),
            Column::new(COLUMN_ADDRESSED, ColumnType::Boolean),
            Column::new(COLUMN_ANSWER_DUE, ColumnType::Boolean),
            Column::new(COLUMN_LIMITED, ColumnType::Text),
            Column::new(COLUMN_DEBT_AUTHORITY, ColumnType::Text),
            Column::new(COLUMN_REPLY_TARGET, ColumnType::Text),
            Column::new(COLUMN_REPLY_TO_ASSISTANT, ColumnType::Boolean),
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
            speaker: string_field(block, COLUMN_SPEAKER),
            origin: string_field(block, COLUMN_ORIGIN),
            sent_at: string_field(block, COLUMN_SENT_AT),
            addressed: block.fields.get(COLUMN_ADDRESSED).and_then(Value::as_bool),
            answer_due: block.fields.get(COLUMN_ANSWER_DUE).and_then(Value::as_bool),
            limited: block
                .fields
                .get(COLUMN_LIMITED)
                .and_then(Value::as_str)
                .and_then(LimitedBy::parse),
            debt_authority: block
                .fields
                .get(COLUMN_DEBT_AUTHORITY)
                .and_then(Value::as_str)
                .and_then(Authority::parse),
            reply_target: string_field(block, COLUMN_REPLY_TARGET),
            reply_to_assistant: block
                .fields
                .get(COLUMN_REPLY_TO_ASSISTANT)
                .and_then(Value::as_bool),
        }
    }
}

impl Agency for ChatMessage {
    fn awaiting(&self) -> Option<Awaiting> {
        // The acting policy — record all, answer some — reads the write-time
        // stamp through the one shared predicate: only a message that still
        // owes an answer summons a turn. The framework owes a turn from the
        // newest block alone, which is why the stamp propagates debt at the
        // write instead of this hook folding history.
        self.owes_answer().then_some(Awaiting::Model)
    }
}

impl Projection for ChatMessage {
    fn group_role(&self) -> Option<Role> {
        // The stored role, erased or not — the role-alternation shape that
        // closes decision 0012's first OPEN. An erased message keeps its
        // place in the grouping pass under its own voice while contributing
        // only the fixed marker, so the contiguous run of its role survives
        // the erasure: no two same-role messages split apart, and a request
        // never opens with the assistant's voice. Where an erased run stands
        // alone between two other-role neighbours it projects one marker-only
        // message — the non-empty separator that keeps strict alternation
        // intact; the prose itself stays gone.
        self.role
    }

    fn llm_parts(&self) -> Option<Vec<ContentPart>> {
        Some(vec![ContentPart::Text {
            text: self.projected_text(),
        }])
    }

    fn llm_text(&self) -> Option<String> {
        Some(self.projected_text())
    }
}

/// Null the personal columns — text, origin reference, platform send time,
/// the reply-target reference and the speaker (both extended 2026-08-23) —
/// of every message a principal wrote, in every conversation: the first of
/// erasure's three steps (decision 0012), owned by the kind because the
/// nullable columns are the kind's own contract. The block header rows are
/// never touched; each affected content row keeps its shape and its message
/// reads back erased. Nulling already-null columns is a no-op, so the step
/// is idempotent.
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
                 {COLUMN_ORIGIN} = NULL, {COLUMN_SENT_AT} = NULL, \
                 {COLUMN_REPLY_TARGET} = NULL, {COLUMN_SPEAKER} = NULL \
                 WHERE {COLUMN_PRINCIPAL_ID} = ?1"
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

/// Null the reply-target reference of every message that replies to one of
/// this principal's messages — the target-keyed half of the reply-target
/// erasure (2026-08-23): the stored
/// value is the replied-to person's own message identifier, so the
/// author-keyed pass alone would null it on the person's rows while
/// leaving a verbatim copy on every row that replied to them. The match
/// runs through the origin column within the same conversation — platform
/// message ids are opaque and unique only per channel, so a bare id match
/// across conversations would null a stranger's reference — which is why
/// erasure runs this pass BEFORE [`erase_principal_content`] nulls the
/// origins it joins on. Nulling already-null columns is a no-op, so the
/// step is idempotent; the framework-table names it joins carry the
/// deliberate coupling decision 0032 records.
pub(crate) async fn erase_reply_targets_naming(
    tx: &StoreTx,
    principal_id: i64,
) -> Result<(), StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        conn.execute(
            &format!(
                "UPDATE {CHAT_MESSAGE_TABLE} SET {COLUMN_REPLY_TARGET} = NULL \
                 WHERE {COLUMN_REPLY_TARGET} IS NOT NULL \
                 AND EXISTS (\
                   SELECT 1 FROM {CHAT_MESSAGE_TABLE} author \
                   JOIN conversation_blocks acb ON acb.block_id = author.block_id \
                   JOIN conversation_blocks rcb \
                     ON rcb.block_id = {CHAT_MESSAGE_TABLE}.block_id \
                   WHERE author.{COLUMN_PRINCIPAL_ID} = ?1 \
                   AND author.{COLUMN_ORIGIN} = {CHAT_MESSAGE_TABLE}.{COLUMN_REPLY_TARGET} \
                   AND acb.conversation_id = rcb.conversation_id\
                 )"
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

// ─── The budget counts ───────────────────────────────────────────────────
//
// Two bounded counts over the kind's own table, each joined by name to two
// of the framework's tables: `blocks` for the receipt time (the header's
// creation time, assigned by the store at the write, unforgeable and never
// null) and `conversation_blocks` for the conversation id. Neither fact
// lives in the content table, and duplicating either there would be a
// second record of a framework-owned fact. The framework does not contract
// these names — the coupling is deliberate, recorded with its risk in
// decision 0032, and surfaced to the framework's improvements list.
//
// The counted predicate is opened debts: addressed, not limited. A refused
// debt consumed no spend, so it never consumes budget either; a propagated
// debt is the same debt carried forward, not a second spend intent. The
// window anchors at `datetime('now')` evaluated inside the count — the
// stamp's own wall clock, since both counts run inside the entry point's
// stamp serialization. The header's `created_at` is not one encoding: the
// store's insert writes RFC 3339 with milliseconds and a local offset,
// while the column's schema default is `datetime('now')`'s space-separated
// UTC form — so the stored value goes through `datetime()` before the
// comparison. Comparing the raw text instead would order the two encodings
// by their differing tenth byte and degrade the window into a
// calendar-date test.

/// The counted-debt predicate both budget counts share, over the message
/// alias `m` and the block-header alias `b`: an opened debt — addressed,
/// not limited — younger than the window, whose modifier arrives as the
/// query's second parameter. One fragment on purpose: what consumes budget
/// is one definition, and two spellings of it could drift apart.
static COUNTED_DEBT_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "m.{COLUMN_ADDRESSED} = 1 AND m.{COLUMN_LIMITED} IS NULL \
         AND datetime(b.created_at) > datetime('now', ?2)"
    )
});

/// How many debts this principal opened younger than the window, across
/// every conversation — spend is global, so heavy direct-chat use and group
/// use share one budget. Runs on the (principal id, addressed) index the
/// protection migration adds.
///
/// # Errors
///
/// [`StoreError`] if the count fails or the store's actor has stopped.
pub(crate) async fn opened_debts_by_principal(
    tx: &StoreTx,
    principal_id: i64,
    window_seconds: NonZeroU64,
) -> Result<i64, StoreError> {
    let cutoff = window_modifier(window_seconds);
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM {CHAT_MESSAGE_TABLE} m \
                 JOIN blocks b ON b.id = m.block_id \
                 WHERE m.{COLUMN_PRINCIPAL_ID} = ?1 AND {counted}",
                counted = COUNTED_DEBT_SQL.as_str(),
            ),
            (principal_id, cutoff),
            |row| row.get(0),
        )?)
    })
    .await
}

/// How many debts were opened in this conversation younger than the window,
/// by any sender. Rides the framework's existing junction index on the
/// conversation id.
///
/// # Errors
///
/// [`StoreError`] if the count fails or the store's actor has stopped.
pub(crate) async fn opened_debts_in_conversation(
    tx: &StoreTx,
    conversation_id: i64,
    window_seconds: NonZeroU64,
) -> Result<i64, StoreError> {
    let cutoff = window_modifier(window_seconds);
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM conversation_blocks cb \
                 JOIN {CHAT_MESSAGE_TABLE} m ON m.block_id = cb.block_id \
                 JOIN blocks b ON b.id = m.block_id \
                 WHERE cb.conversation_id = ?1 AND {counted}",
                counted = COUNTED_DEBT_SQL.as_str(),
            ),
            (conversation_id, cutoff),
            |row| row.get(0),
        )?)
    })
    .await
}

/// The window as a `datetime` modifier: `-N seconds`, subtracted from the
/// count's own `datetime('now')` anchor. Whole seconds by the budget
/// type's own contract, so no finer value exists to lose here.
fn window_modifier(window_seconds: NonZeroU64) -> String {
    format!("-{window_seconds} seconds")
}

/// Every conversation this principal has a recorded message in — the
/// person-keyed read behind the first-answer disclosure (decision 0078):
/// the ledger is the memory of who was already introduced, and this names
/// where to look. Joined by name to the framework's junction table, the
/// deliberate coupling decision 0032 records; rides the same
/// (principal id, addressed) index as the principal budget count.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn conversations_of_principal(
    tx: &StoreTx,
    principal_id: i64,
) -> Result<Vec<i64>, StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let mut statement = conn.prepare(&format!(
            "SELECT DISTINCT cb.conversation_id FROM {CHAT_MESSAGE_TABLE} m \
             JOIN conversation_blocks cb ON cb.block_id = m.block_id \
             WHERE m.{COLUMN_PRINCIPAL_ID} = ?1"
        ))?;
        let rows = statement
            .query_map([principal_id], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        Ok(rows)
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
    /// The conversation's tool admission record (the tools module owns the
    /// kind; it composes here so one parse path reads every block).
    ToolPalette(crate::tools::palette::ToolPalette),
    /// A group's observed fact — its title or its rules (the note module
    /// owns the kind; it composes here so one parse path reads every
    /// block).
    ContextNote(crate::note::ContextNote),
    /// A filed report awaiting delivery (the report module owns the kind;
    /// it composes here so one parse path reads every block).
    Report(crate::tools::report::Report),
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
