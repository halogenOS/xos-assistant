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

use std::future::Future;
use std::num::NonZeroU64;
use std::sync::LazyLock;

use agent_ledger::agency::{DateMarker, Quote};
use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{
    Agency, AgencyCtx, Awaiting, Block, BlockKind, Column, ColumnType, ContentDescriptor,
    ContentPart, FromBlock, GateDecision, LeafKind, Projection, Role, RuntimeEvent, StoreError,
};
use rusqlite::OptionalExtension;
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
/// Whether the message summoned the assistant, as the entry point resolved
/// it at the write (recast 2026-08-23, the helpful-mode unit): the adapter's
/// addressed fact — a mention, a reply to the assistant, its name, a direct
/// chat — or the helpful answering mode, under which every group message is
/// summoned for the model's own judgment. The column keeps its original
/// name; the recast is what lets the debt spine — the budget counts, the
/// unlatch emission, the co-summoner rule and the disclosure fold — read one
/// stored fact under both answering modes, stamped once instead of
/// re-derived against a configuration that can change between runs.
/// Structure, not personal data: erasure leaves it.
pub const COLUMN_ADDRESSED: &str = "addressed";
/// Whether the user LITERALLY addressed the assistant — the adapter's own
/// fact, before the answering mode folded into the summons (unit 16,
/// 2026-08-24): a mention, a reply to the assistant, its name, a direct
/// chat. Stored BESIDE the recast summons, which keeps its column and every
/// one of its readers — the budgets, the unlatch, the co-summoner rule, the
/// report scoping and the disclosure fold all stay on [`COLUMN_ADDRESSED`].
/// Exactly ONE consumer reads this column: the outbound edge's answer
/// threading (unit 26, 2026-08-24), which names the message an answer is
/// delivered as a reply to. It must be the literal fact and not the
/// recast summons — helpful mode summons the assistant for every message,
/// and quote-replying someone who never addressed it, in front of the
/// group, is not a courtesy. Structure, not personal data: erasure leaves
/// it. Added by the literal-addressed migration step, so pre-migration
/// rows read NULL — never a decision input, because an absent value folds
/// to unaddressed and the answer goes out plain.
pub const COLUMN_LITERAL_ADDRESSED: &str = "literal_addressed";
/// Whether the message owes the model a turn — the write-time stamp the
/// entry point decides once at insert: true when the message is summoned
/// and no budget refused it, or when the block behind it carries an
/// unanswered answer-due, so a message arriving on the heels of a
/// summoning one propagates the debt instead of cancelling it. Structure,
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
/// what reply threading names (decided 2026-08-23; the report tool's
/// reply-target resolution left this column on 2026-08-24, when the tool
/// gained its validated origin parameter). NULL for a non-reply, for a reply
/// without a usable id, and for a reply to one of the assistant's own
/// messages. Personal data of TWO people at once: the row's author chose
/// to store it, and its value is the replied-to person's own message
/// identifier — so erasure reaches it from both ends: the author-keyed
/// pass nulls it with the rest of the row, and the crate-private
/// target-keyed pass `erase_reply_targets_naming` nulls it when the
/// replied-to person is erased; the deletion mirror's own pass nulls it
/// across the conversation when the named message is erased (decision
/// 0085). The target-keyed reach is exactly as wide
/// as its join: a stored value matching none of the erased person's
/// recorded origins — a reply recorded after their erasure completed,
/// one recorded inside a failed erasure's retry window, or one naming a
/// message the assistant never recorded — keeps an unreachable copy.
/// Decision 0063's refinements record that residual and its decided
/// follow-up, a reach key resolved when the reply is recorded. Added by
/// the reply-target migration step, so pre-migration rows read NULL.
pub const COLUMN_REPLY_TARGET: &str = "reply_target";
/// Whether the message replies to one of the assistant's own messages —
/// a stored addressing fact (decided 2026-08-23; the report tool stopped
/// reading it on 2026-08-24, when its self-report refusal moved to the
/// named message's own stored voice). Structure, not personal data:
/// erasure leaves it. Added by the reply-target migration step, so
/// pre-migration rows read NULL.
pub const COLUMN_REPLY_TO_ASSISTANT: &str = "reply_to_assistant";
/// The platform's own id for the message this one is a new version of,
/// opaque — the revision reference (unit T3, 2026-08-31). NULL for an
/// ordinary message, which supersedes nothing, and for every pre-migration
/// row. The stored value names the message as first known, so every version
/// of one message carries one key: `WHERE {COLUMN_ORIGIN} = ?1 OR
/// {COLUMN_REVISES} = ?1` reaches a chain of any length in one match
/// whenever the id matched against is THAT key. On this platform every id
/// the platform can name for a message is that key, because an edit arrives
/// under the original's own message id — which is how the newest-version
/// read, the mirror's named erasure and the report's resolution all reach
/// every version without walking a chain. On a platform delivering an id
/// per revision the same match reaches the whole chain from the root id and
/// one row from any later version's id, which is why such an adapter owes a
/// root-resolution step before it reports a revision (decision 0171).
///
/// Personal data of its author, the same standing
/// [`COLUMN_REPLY_TARGET`] holds: the author-keyed pass nulls it with the
/// rest of the row, and the deletion mirror's named pass nulls it across the
/// conversation when the named message is erased. No target-keyed pass is
/// owed for it, and that is ENFORCED rather than assumed: the ingestion
/// compares the reviser's resolved principal against the author of the
/// newest recorded version and stores this column only when the two are the
/// same person — a mismatch records the message as an ordinary new one,
/// with no reference at all. A stored reference therefore always names a
/// message of its own row's author, and the author-keyed pass reaches both
/// ends of it. Added by the revision migration step.
pub const COLUMN_REVISES: &str = "revises";

/// What an erased message contributes to a projected request in place of its
/// nulled prose. Non-empty on purpose: the live vendors whose strict
/// alternation decision 0027 protects also reject a message whose content is
/// empty, and a run in which every message is erased projects exactly one
/// message built from these contributions alone. A fixed marker carries none
/// of the person's words, so the erasure's promise holds.
pub const ERASED_MARKER: &str = "[message erased]";

/// What a revision carries at the head of its text, so the model reads the
/// line as what the room sees: the same message, said differently (unit T3,
/// 2026-08-31). A fixed constant beside the erased marker, and prose like
/// the speaker prefix and the id mark — a member can type these characters
/// into their own message and nothing distinguishes the bytes.
///
/// The bound is that NOTHING mechanical reads it: no stamp, no tool, no
/// erasure pass and no admission consults the marker, so a forgery can
/// mislead the model's reading and reach nothing else, exactly as
/// [`projected_origin_mark`]'s own documentation bounds a forged id. The
/// stored fact the marker speaks for is [`COLUMN_REVISES`], and every
/// mechanism reads that column instead.
///
/// It sits after the speaker prefix and ahead of the text, never inside the
/// bracketed id: the id is the one token the model is taught to name a
/// message by, and folding a word into it would corrupt exactly that token.
pub const EDITED_MARKER: &str = "(edited)";

/// The projected id mark of one recorded message: the stored origin in
/// brackets, ahead of the speaker prefix and the text (unit 15,
/// 2026-08-24). The mark is what lets the model NAME a message — the
/// report tool takes the id it shows, validated against the turn's own
/// assessment set — so it rides every user-voiced live message that has a
/// stored origin, in every mode: the projection is a per-block reading
/// with no configuration access, and an id beside a message is honest
/// context wherever it appears. The erased placeholder never carries it
/// (erasure nulls the origin with the text), and the mark is prose like
/// the speaker prefix: a member who types a bracketed id into their
/// message forges bytes the model cannot tell apart, and the tool's
/// co-summoner validation is what bounds where such a forgery can aim.
#[must_use]
pub fn projected_origin_mark(origin: &str) -> String {
    format!("[{origin}]")
}

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

/// The two platform identifiers one write records: the message's own
/// opaque origin, and the origin of the message it supersedes where the
/// adapter reported one (unit T3, 2026-08-31). They enter the row together
/// and are carried as one value so the append's field map cannot take them
/// apart — the same discipline [`RecordedSender`] and [`Stamp`] travel
/// under.
///
/// Both are `None` for a message the platform gave no usable id for; the
/// revision reference alone is `None` for every ordinary message, which
/// supersedes nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecordedOrigin<'a> {
    /// This version's own identifier, stored under [`COLUMN_ORIGIN`].
    pub origin: Option<&'a str>,
    /// The identifier of the message this one is a new version of, stored
    /// under [`COLUMN_REVISES`].
    pub revises: Option<&'a str>,
}

/// The summons resolution's two facts, decided together at the ONE place
/// the answering mode enters the machinery (unit 16, 2026-08-24): the
/// summons — the adapter's addressed fact OR the helpful mode's
/// every-message evaluation, what the whole debt spine reads — and the
/// literal addressed fact as the adapter alone recorded it, what the
/// outbound answer threading reads. Carried as one value so the two booleans
/// can never swap places between the resolution and the stamp. The literal
/// fact is stored, never derived: helpful is mutable configuration, and an
/// addressed helpful-mode message would be indistinguishable after the
/// fact.
#[derive(Debug, Clone, Copy)]
pub struct Summons {
    /// Whether the message summoned the assistant, mode folded in — stored
    /// under [`COLUMN_ADDRESSED`].
    pub summoned: bool,
    /// Whether the user literally addressed the assistant — stored under
    /// [`COLUMN_LITERAL_ADDRESSED`]. Always false when `summoned` is
    /// false: a message that did not even summon was not addressed.
    pub literal_addressed: bool,
}

/// The write-time stamp: the facts the entry point decides once at the
/// insert, composed here so the composition rule and the minimum rule live
/// beside their readers ([`ChatMessage::owes_answer`],
/// [`ChatMessage::carried_debt_authority`]) as one pure value a test can
/// call directly.
#[derive(Debug, Clone, Copy)]
pub struct Stamp {
    /// Whether the message summoned the assistant — the entry point's
    /// resolution of the adapter's addressed fact and the answering mode,
    /// stored under [`COLUMN_ADDRESSED`].
    pub addressed: bool,
    /// Whether the user literally addressed the assistant, per
    /// [`Summons::literal_addressed`] — stored under
    /// [`COLUMN_LITERAL_ADDRESSED`], read only by the outbound answer
    /// threading.
    pub literal_addressed: bool,
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
    /// Compose the stamp from the write's inputs: the message's resolved
    /// [`Summons`] — the entry point resolves both of its facts before this
    /// call — its sender's authority, the first refusing budget (already
    /// `None` for unsummoned messages — budgets are consulted for summoned
    /// ones only), and the owing tail's debt if the conversation carries
    /// one.
    ///
    /// The composition rule: answer-due = (summoned and not limited) or
    /// tail-owes — a refused own debt never cancels a propagated one. The
    /// minimum rule (decision 0036): a carried debt takes the lowest
    /// authority that contributed to summoning the turn — the tail's debt
    /// authority against the incoming sender's, regardless of the incoming
    /// message's own summons fact — and a fresh taken debt opens at its
    /// sender's own authority. The literal fact passes through untouched:
    /// no rule here reads it, only the store carries it to the outbound
    /// answer threading.
    #[must_use]
    pub fn compose(
        summons: Summons,
        sender: Authority,
        limited: Option<LimitedBy>,
        owing_tail: Option<TailDebt>,
    ) -> Self {
        let own_debt_taken = summons.summoned && limited.is_none();
        Self {
            addressed: summons.summoned,
            literal_addressed: summons.literal_addressed,
            limited,
            answer_due: own_debt_taken || owing_tail.is_some(),
            debt_authority: match owing_tail {
                Some(tail) => Some(tail.authority.map_or(sender, |carried| carried.min(sender))),
                None => own_debt_taken.then_some(sender),
            },
        }
    }

    /// Whether this message's own debt was taken — summoned and no budget
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
    /// The origin of the message this one is a new version of, per
    /// [`COLUMN_REVISES`]. `None` for an ordinary message, for every
    /// pre-migration row, and for an erased row, whose author-keyed pass
    /// nulled it.
    pub revises: Option<String>,
    /// The platform's send time, RFC 3339. The store's insertion time lives
    /// on the block header. `None` only for a block the store did not
    /// produce.
    pub sent_at: Option<String>,
    /// Whether the message summoned the assistant, per
    /// [`COLUMN_ADDRESSED`]'s recast. `None` only for a block
    /// the store did not produce (the schema stores it NOT NULL).
    pub addressed: Option<bool>,
    /// Whether the user literally addressed the assistant, per
    /// [`COLUMN_LITERAL_ADDRESSED`]. `None` on every pre-migration row and
    /// on a block the store did not produce; the one reader — the outbound
    /// answer threading — folds that absence to unaddressed, and the
    /// answer goes out plain.
    pub literal_addressed: Option<bool>,
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
    /// as the [`RecordedSender`]; the two platform identifiers travel
    /// together as the [`RecordedOrigin`]; the four decided facts travel
    /// together as the composed [`Stamp`]; the reply fact travels as the
    /// translated [`ReplyTarget`](crate::message::ReplyTarget), encoded into
    /// its two columns here.
    #[must_use]
    pub fn stored_fields(
        text: &str,
        sender: RecordedSender<'_>,
        identifiers: RecordedOrigin<'_>,
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
        if let Some(origin) = identifiers.origin {
            fields.insert(COLUMN_ORIGIN.into(), json!(origin));
        }
        if let Some(revises) = identifiers.revises {
            fields.insert(COLUMN_REVISES.into(), json!(revises));
        }
        match reply_target {
            Some(crate::message::ReplyTarget::Message { origin }) => {
                fields.insert(COLUMN_REPLY_TARGET.into(), json!(origin));
            }
            // The origin the variant carries since unit 38 is deliberately
            // not stored: it is consumed during ingestion, where it
            // resolves which of the assistant's own recorded deliveries
            // the reply quotes, and it never reaches a column.
            // [`COLUMN_REPLY_TARGET`]'s documentation states that the
            // column is NULL for a reply to one of the assistant's own
            // messages and classifies its values as two people's personal
            // data; a column that sometimes held the assistant's own id
            // would make both statements false, and erasure's target-keyed
            // pass reads it as member-message references.
            Some(crate::message::ReplyTarget::AssistantMessage { origin: _ }) => {
                fields.insert(COLUMN_REPLY_TO_ASSISTANT.into(), json!(true));
            }
            None => {}
        }
        fields.insert(COLUMN_SENT_AT.into(), json!(sent_at));
        fields.insert(COLUMN_ADDRESSED.into(), json!(stamp.addressed));
        fields.insert(
            COLUMN_LITERAL_ADDRESSED.into(),
            json!(stamp.literal_addressed),
        );
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

    /// Whether erasure emptied this message: the text is the one column
    /// whose absence only erasure produces — the adapter never records an
    /// empty message — so its null speaks for the whole row. The owing-tail
    /// walk reads through rows this affirms (decision 0086); the row's own
    /// cancelled debt stays cancelled through [`Self::owes_answer`].
    #[must_use]
    pub fn erased(&self) -> bool {
        self.text.is_none()
    }

    /// Whether the owing-tail walk reads THROUGH this row instead of
    /// settling on it — the row-shape half of the walk's one decision. The
    /// second half is the query this module runs to skip a whole
    /// transparent run
    /// (`newest_block_id_past_transparent`), whose SQL spells exactly
    /// this disjunction over the stored columns; the two are written
    /// beside each other so neither can drift.
    ///
    /// A third home reads this same predicate and does not spell it again:
    /// the anchor gate's debt-chain walk
    /// (`crate::tools::provenance`, decision 0043), which extends across a
    /// row this predicate affirms instead of ending its chain there
    /// (2026-08-30). The two walks ask different questions — one hands a
    /// debt to the next write, the other reads a debt's origins for tool
    /// admission — but a row that answered nothing answers nothing for
    /// either, and a gate that stopped on an unsummoned bot's row would
    /// fold the debt origin's own turn to the floor.
    ///
    /// Two shapes are transparent, disjunctively. An erased row
    /// (decision 0086): someone's deletion empties one ask, never the
    /// standing question behind it. A row whose stamp is false
    /// (2026-08-30): it owes nothing itself, and reading on past it loses
    /// nothing — for two different reasons, one per class of row a false
    /// stamp is written on. This is where that argument is made; the three
    /// reading sites carry the widening, not a second telling of why.
    ///
    /// A row whose stamp was composed against a READ owing tail — every
    /// production append but the one below, a command's limited row
    /// included — took that stamp under the entry point's lock with the
    /// tail read in the same critical section, so anything older owed would
    /// have made the stamp true. Such a row's falseness CERTIFIES a settled
    /// frontier behind it, and reading through reaches the same frontier
    /// stopping at it already named.
    ///
    /// The one row composed against no tail at all is an unsummoned bot's
    /// message, which must trigger nothing: `Assistant::owed_tail`
    /// withholds the tail there and the row is stamped false by rule,
    /// without any read (decision 0154). That row certifies NOTHING — it is
    /// deliberately written false above a live debt — and this widened walk
    /// is the whole of its safety: reading through it is exactly what
    /// leaves the debt owed for the next message entitled to carry it.
    ///
    /// The disjunction is essential: a true-stamped erased row is
    /// transparent by the erased half alone, and narrowing either half to
    /// the other's condition would bury a live debt behind it.
    #[must_use]
    pub fn transparent_to_the_walk(&self) -> bool {
        self.erased() || self.answer_due == Some(false)
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
    /// the opened-debt predicate: summoned, and no budget refused. One
    /// predicate, three spellings that must agree: [`Stamp::own_debt_taken`]
    /// at the write, the budget counts' SQL fragment over the stored rows,
    /// and this reading of one loaded row — which is also decision 0043's
    /// co-summoner rule, since exactly the messages this predicate affirms
    /// join a turn's provenance when absorbed into its span; the summons
    /// recast is what makes an unaddressed message under helpful answering
    /// a co-summoner without this reading ever consulting the mode. A row
    /// the store did not produce reads `None` for the summons and answers
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
    /// model may address the person by — and a user-voiced message with a
    /// stored origin opens with [`projected_origin_mark`] ahead of that
    /// (unit 15, 2026-08-24): the id the model names when it reports.
    /// Everything else projects bare: a handleless sender's message (no
    /// substitute identifier leaves the machine, per decision 0056), any
    /// non-user voice, and the erased marker — the erasure pass nulls the
    /// speaker and the origin with the text, and even a row it
    /// half-reached keeps the placeholder exactly as it is, unmarked.
    ///
    /// A revision — a row carrying [`COLUMN_REVISES`] — differs in two
    /// places (unit T3, 2026-08-31). It projects under the REVISED
    /// message's id, so every version of one message shows the model one
    /// token to name it by and the report resolves that token through
    /// either column. And [`EDITED_MARKER`] opens its text, after the
    /// speaker prefix, so the model reads which version it is looking at.
    /// The superseded version keeps projecting its own words untouched:
    /// this is a per-block reading with no ledger access, so folding
    /// history is not something it can do, and rewriting a stored row is
    /// not something an append-only ledger does.
    fn projected_text(&self) -> String {
        let Some(text) = &self.text else {
            return ERASED_MARKER.to_owned();
        };
        let said = match &self.revises {
            Some(_) => format!("{EDITED_MARKER} {text}"),
            None => text.clone(),
        };
        let line = match &self.speaker {
            Some(speaker) if self.role == Some(Role::User) => format!("{speaker}: {said}"),
            _ => said,
        };
        match self.revises.as_deref().or(self.origin.as_deref()) {
            Some(named) if self.role == Some(Role::User) => {
                format!("{} {line}", projected_origin_mark(named))
            }
            _ => line,
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
            Column::new(COLUMN_LITERAL_ADDRESSED, ColumnType::Boolean),
            Column::new(COLUMN_ANSWER_DUE, ColumnType::Boolean),
            Column::new(COLUMN_LIMITED, ColumnType::Text),
            Column::new(COLUMN_DEBT_AUTHORITY, ColumnType::Text),
            Column::new(COLUMN_REPLY_TARGET, ColumnType::Text),
            Column::new(COLUMN_REPLY_TO_ASSISTANT, ColumnType::Boolean),
            Column::new(COLUMN_REVISES, ColumnType::Text),
        ],
        reference_columns: &[],
        // What a quote of one of these messages resolves to (unit 31,
        // 2026-08-28). The framework reads this column raw into a quoted
        // span, and validates the declaration when the store opens: the
        // named column must be declared here, must not be the role column,
        // must be `ColumnType::Text` by variant, and the kind must not be
        // ephemeral — all four hold for the message's own text. An erased
        // row's NULL text resolves to nothing, so erasure needs no second
        // pass for the quotes pointing at it.
        quoted_text_column: Some(COLUMN_TEXT),
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
            literal_addressed: block
                .fields
                .get(COLUMN_LITERAL_ADDRESSED)
                .and_then(Value::as_bool),
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
            revises: string_field(block, COLUMN_REVISES),
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
/// the reply-target reference and the speaker (both extended 2026-08-23),
/// and the revision reference (unit T3, 2026-08-31) — of every message a
/// principal wrote, in every conversation: the first of
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
                 {COLUMN_REPLY_TARGET} = NULL, {COLUMN_SPEAKER} = NULL, \
                 {COLUMN_REVISES} = NULL \
                 WHERE {COLUMN_PRINCIPAL_ID} = ?1"
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

/// Null EVERY recorded version of the message a deletion mirror names
/// (2026-08-23, the deletion mirror; widened to versions by unit T3,
/// 2026-08-31): text, origin, send time, reply reference, speaker and the
/// revision reference — the six nulls [`erase_principal_content`] applies
/// to a person's own rows, scoped within the conversation to the rows whose
/// stored origin OR stored revision reference matches the named id. Returns
/// how many rows were nulled, which is what tells the composition whether
/// the named target was ever here: the unknown-target command stays a full
/// no-op.
///
/// The match is a disjunction because a message can be recorded more than
/// once: every version of one message stores the original's id under
/// [`COLUMN_REVISES`], so one statement reaches a chain of any length from
/// that id. On this platform that is every id a deletion can name — an edit
/// arrives under the original's own message id, so the reply an
/// administrator deletes carries it whichever version they were looking at.
/// On a platform delivering an id per revision the same statement reaches
/// the whole chain from the root id and the one row an id naming a later
/// version alone identifies; an adapter there owes a root-resolution step
/// before it reports a revision (decision 0171), and nothing here changes
/// for it. Deleting a message deletes what the group saw, and what the
/// group saw is every version of it. The count is therefore the number of
/// VERSIONS emptied, not a claim that one row was.
///
/// The references pointing AT the target are nulled beside this, at the
/// composition site — the reply references by
/// [`erase_reply_references_naming`], the filed report targets by the
/// report kind's own pass — because the named origin can belong to another
/// kind's record too, and the composition is where the whole mirror is
/// decided. Every one of those passes keys on the origin the caller hands
/// in, not on the target row's column, so their order carries no join
/// hazard. The command row requesting the deletion
/// appends after the mirror runs and keeps its own reply reference, the
/// request's lawful record (decision 0085).
///
/// Platform message ids are opaque and unique only per channel, so the
/// match runs through the conversation junction and never reaches a
/// stranger conversation's row; the framework-table name it joins carries
/// the deliberate coupling decision 0032 records. The block header keeps
/// its place, the placeholder projects the erased marker, and nothing else
/// moves. Idempotent: nulling already-null columns is a no-op, and an
/// already-erased row's origin is NULL and matches nothing.
pub(crate) async fn erase_message_named(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<usize, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn.execute(
            &format!(
                "UPDATE {CHAT_MESSAGE_TABLE} SET {COLUMN_TEXT} = NULL, \
                 {COLUMN_ORIGIN} = NULL, {COLUMN_SENT_AT} = NULL, \
                 {COLUMN_REPLY_TARGET} = NULL, {COLUMN_SPEAKER} = NULL, \
                 {COLUMN_REVISES} = NULL \
                 WHERE ({COLUMN_ORIGIN} = ?1 OR {COLUMN_REVISES} = ?1) AND EXISTS (\
                   SELECT 1 FROM conversation_blocks cb \
                   WHERE cb.block_id = {CHAT_MESSAGE_TABLE}.block_id \
                   AND cb.conversation_id = ?2\
                 )"
            ),
            (&origin, conversation_id),
        )?)
    })
    .await
}

/// Null the reply reference of every row in one conversation that named
/// the deleted origin (decision 0085). Without it each replier would keep
/// a verbatim copy of the deleted record's identifier that no later
/// erasure could reach, because [`erase_reply_targets_naming`] joins on
/// the very origins the erasing passes null. The reference is this kind's
/// own column, whoever wrote the deleted record — a message or a join
/// event — which is why the composition decides WHEN this runs and the
/// kind decides only how.
///
/// Returns how many references were nulled. Idempotent: nulling
/// already-null columns is a no-op, and a second run finds nothing left
/// naming the origin.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_reply_references_naming(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<usize, StoreError> {
    crate::erasure::null_references_to(tx, REPLY_TARGET_SITE, conversation_id, origin).await
}

/// Where this kind holds a copy of ANOTHER record's platform id: the reply
/// target, stored whoever wrote the record it names — a message or a join
/// event. The column is the kind's own; which records name a person is a
/// fact about the whole consumer, so both passes over this site are driven
/// by the erasure composition.
pub(crate) const REPLY_TARGET_SITE: crate::erasure::ReferenceSite = crate::erasure::ReferenceSite {
    table: CHAT_MESSAGE_TABLE,
    column: COLUMN_REPLY_TARGET,
};

/// This kind's own recorded platform ids, for the target-keyed reply pass:
/// a reply stores the replied-to MESSAGE's origin, and this names where
/// that origin is recorded against its author.
pub(crate) const ORIGIN_SOURCE: crate::erasure::OriginSource = crate::erasure::OriginSource {
    table: CHAT_MESSAGE_TABLE,
    origin_column: COLUMN_ORIGIN,
    principal_column: COLUMN_PRINCIPAL_ID,
};

/// Null the reply-target reference of every message that replies to one of
/// this principal's own records — the target-keyed half of the reply-target
/// erasure (2026-08-23; widened to the join notice's events by unit 36,
/// 2026-08-29): the stored
/// value is the replied-to person's own identifier, so the
/// author-keyed pass alone would null it on the person's rows while
/// leaving a verbatim copy on every row that replied to them. The match
/// runs through each source's origin column within the same conversation —
/// platform message ids are opaque and unique only per channel, so a bare
/// id match across conversations would null a stranger's reference — which
/// is why erasure runs this pass BEFORE the passes that null the origins it
/// joins on. Nulling already-null columns is a no-op, so the
/// step is idempotent; the framework-table names it joins carry the
/// deliberate coupling decision 0032 records.
///
/// The sources arrive from the caller, which is the erasure composition:
/// the reference column belongs to this kind, and WHICH records name a
/// person is a fact about the whole consumer, so a new recording kind
/// joins the reach by entering that list. The join itself is the
/// composition's own, spelled once for every kind that holds such a copy.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_reply_targets_naming(
    tx: &StoreTx,
    principal_id: i64,
    sources: &'static [crate::erasure::OriginSource],
) -> Result<(), StoreError> {
    crate::erasure::null_references_naming(tx, REPLY_TARGET_SITE, principal_id, sources).await
}

/// The framework records that are never an answerable block (2026-08-28;
/// the quote joined 2026-08-29).
///
/// The date marker is the ledger's own calendar entry, which the framework
/// writes inside the same transaction as the user-voiced append that
/// tripped it and orders immediately before it. It carries no voice, no ask
/// and no answer, so it can answer for nothing that reads the ledger back:
/// a reader that stops on one has read a record of the day in place of the
/// message the day was recorded for — a member's standing question
/// swallowed by the calendar.
///
/// The quote is the context a member attached to their reply (unit 31): it
/// precedes their own message and, between the two appends, stands at the
/// tail alone — the exact state a crash leaves, and the state the retry's
/// tail-skip then preserves. A reader that settled a debt on it would let
/// a quote of someone's words answer for the standing question behind it,
/// which no member ever asked it to do.
///
/// Both facts belong to their kinds, hold for every reader there will ever
/// be, and are independent of any caller's kind list. This is the whole of
/// that decision, recorded once and read by exactly two sites, so the two
/// can never drift apart: [`newest_block_id_past_transparent`] below, which
/// excludes these rows in SQL for every caller, and the tail condition in
/// `Assistant::owing_tail_debt`, which treats such a tail as transparent
/// and reads behind it. It is deliberately NOT folded into
/// [`crate::assembly::DEBT_READ_THROUGH`] — that list is the consumer's own
/// policy about its own kinds, chosen per caller, while this holds whoever
/// asks. Each kind is named through the framework leaf's own `KINDS`
/// declaration, never a literal here, which is why the list is composed at
/// first read instead of spelled as a const: the leaves own their strings.
pub(crate) static NEVER_ANSWERABLE: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    DateMarker::KINDS
        .iter()
        .chain(Quote::KINDS)
        .copied()
        .collect()
});

/// The newest block of one conversation that can settle the owing-tail
/// walk: outside the caller's read-through kinds, and past every chat row
/// [`ChatMessage::transparent_to_the_walk`] affirms — the erased ones
/// (2026-08-23, the deletion mirror; decision 0086) and the false-stamped
/// ones (2026-08-30), disjunctively, exactly as that predicate reads them.
/// Erasure nulls a chat row's text but leaves its kind and its place, and
/// a false stamp leaves no mark on the kind at all, so a kind list alone
/// can skip neither — and a live debt a third party's row still owes
/// behind such a run must reach the next message's stamp instead of dying
/// with someone else's deletion or under a bot's passing remark. One
/// bounded query answers the whole run; a row-by-row walk would stretch
/// with the run's length into a conversation hydration on ingestion's hot
/// path. The query lives on the kind because only the kind knows those
/// shapes; the framework-table names it joins carry the deliberate
/// coupling decision 0032 records, and the placeholder list is built from
/// the slice, so a widened kind set is a data change at the caller. An
/// empty slice reads past transparent chat rows and date markers.
///
/// The caller's list is not the whole exclusion: every read also skips the
/// framework's date records, per [`NEVER_ANSWERABLE`] above — the one
/// recording of that rule, whose second reader is the walk's own tail
/// condition. It is not a caller's choice because a calendar entry
/// interposed above an owing message would make the walk judge the day
/// instead of the question, whoever asked for the read. The exclusion set
/// therefore always holds at least that kind, whatever the caller passed.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn newest_block_id_past_transparent(
    tx: &StoreTx,
    conversation_id: i64,
    read_through: &'static [&'static str],
) -> Result<Option<i64>, StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let excluded: Vec<&'static str> = read_through
            .iter()
            .chain(NEVER_ANSWERABLE.iter())
            .copied()
            .collect();
        let placeholders: Vec<String> = (0..excluded.len())
            .map(|index| format!("?{}", index + 2))
            .collect();
        let exclusion = format!("AND b.block_type NOT IN ({}) ", placeholders.join(", "));
        let mut parameters: Vec<&dyn rusqlite::ToSql> = vec![&conversation_id];
        parameters.extend(excluded.iter().map(|kind| kind as &dyn rusqlite::ToSql));
        Ok(conn
            .query_row(
                &format!(
                    "SELECT cb.block_id FROM conversation_blocks cb \
                     JOIN blocks b ON b.id = cb.block_id \
                     WHERE cb.conversation_id = ?1 \
                     {exclusion}\
                     AND NOT EXISTS (\
                       SELECT 1 FROM {CHAT_MESSAGE_TABLE} m \
                       WHERE m.block_id = cb.block_id \
                       AND (m.{COLUMN_TEXT} IS NULL \
                            OR m.{COLUMN_ANSWER_DUE} = 0)\
                     ) \
                     ORDER BY cb.id DESC LIMIT 1"
                ),
                parameters.as_slice(),
                |row| row.get(0),
            )
            .optional()?)
    })
    .await
}

/// One recorded message a quote can reference: the block a span points at,
/// and the stored text a span is measured against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuotableMessage {
    /// The block the span's endpoints name.
    pub block_id: i64,
    /// The message's stored text, as the quote will resolve it.
    pub text: String,
}

/// The message one origin names in one conversation, for the reply quote of
/// unit 31 (2026-08-28) — the NEWEST such row, and none of the erased ones.
///
/// Newest, because delivery is at-least-once and no origin dedupe exists in
/// the ingest: a redelivered update records its message a second time under
/// the same origin, and the latest stored version of that origin is the one
/// the member was looking at when they replied. Erased rows are excluded by
/// their nulled text — the erasing passes null the origin beside it, so a
/// fully erased row matches nothing here anyway, and the text condition
/// covers a row a half-reached pass left behind. A conversation with no
/// matching row answers `None`, and the reply lands quoteless: nothing is
/// invented in place of a message the ledger does not hold.
///
/// This is the first read in the consumer that maps an origin to a BLOCK
/// ID. The erasure passes match the same column but only ever null by it,
/// so there was no existing lookup to reuse. Scoped through the
/// conversation junction like every other origin match: platform message
/// ids are opaque and unique only per channel, and a bare id match would
/// reach a stranger conversation's row. The framework-table name it joins
/// carries the deliberate coupling decision 0032 records.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn newest_message_of_origin(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<Option<QuotableMessage>, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                &format!(
                    "SELECT m.block_id, m.{COLUMN_TEXT} \
                     FROM {CHAT_MESSAGE_TABLE} m \
                     JOIN conversation_blocks cb ON cb.block_id = m.block_id \
                     WHERE cb.conversation_id = ?1 \
                     AND m.{COLUMN_ORIGIN} = ?2 \
                     AND m.{COLUMN_TEXT} IS NOT NULL \
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

/// The newest recorded version of one named message in one conversation —
/// the read every one of a revision's readings is decided from (unit T3,
/// 2026-08-31).
///
/// `None` says the store holds NO version of that message: never recorded,
/// or emptied by erasure, which nulls a row's origin and its revision
/// reference along with its text and so leaves nothing to match. Both
/// readings are one answer on purpose — the caller records nothing either
/// way — and the erasure half is why: an edit update the platform fired on
/// its own would otherwise write a person's erased words back into the
/// ledger with no human act anywhere in the path.
///
/// `Some` carries the newest version's stored text, which the caller
/// compares byte for byte against the incoming one, and the principal that
/// wrote it, which the caller compares against the reviser: the two facts
/// travel together because one row answers both questions and a second read
/// would be a second chance for them to disagree.
///
/// The match is `origin OR revises`, because every version of one message
/// stores the original's id: on a platform where a revision carries an
/// origin of its own, matching the origin alone would find nothing after
/// the first edit and silently record every later one twice. Newest is by
/// the junction's own append order, never by a stored send time — that is a
/// clock a platform supplied, and two edits within one second would be
/// unordered under it. Erased rows drop out by their nulled text, exactly
/// as in [`newest_message_of_origin`] above.
///
/// One bounded statement, not a conversation load: this runs on every edit
/// update the platform delivers, and hydrating a ledger per link preview
/// would be a different cost class.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped. The
/// caller fails closed on it: recording anyway would duplicate a row and,
/// under helpful answering, spend a model turn on it.
pub(crate) async fn newest_recorded_version(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<Option<RecordedVersion>, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                &format!(
                    "SELECT m.{COLUMN_TEXT}, m.{COLUMN_PRINCIPAL_ID} \
                     FROM {CHAT_MESSAGE_TABLE} m \
                     JOIN conversation_blocks cb ON cb.block_id = m.block_id \
                     WHERE cb.conversation_id = ?1 \
                     AND (m.{COLUMN_ORIGIN} = ?2 OR m.{COLUMN_REVISES} = ?2) \
                     AND m.{COLUMN_TEXT} IS NOT NULL \
                     ORDER BY cb.id DESC LIMIT 1"
                ),
                (conversation_id, &origin),
                |row| {
                    Ok(RecordedVersion {
                        text: row.get(0)?,
                        principal_id: row.get(1)?,
                    })
                },
            )
            .optional()?)
    })
    .await
}

/// The newest recorded version of one message, as
/// [`newest_recorded_version`] answers it: the text a revision is compared
/// against, and the principal that wrote that version.
///
/// The author rides along because the revision reference is only stored
/// when the reviser IS that author — the invariant [`COLUMN_REVISES`]
/// states, enforced at the ingestion rather than assumed of a platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedVersion {
    /// The stored text of that version, compared byte for byte.
    pub text: String,
    /// The principal that wrote it, compared against the reviser's.
    pub principal_id: i64,
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
// The counted predicate is opened debts: summoned, not limited. A refused
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
//
// A debt answered by an empty answer is excluded (unit 14, 2026-08-23;
// re-keyed by unit 22, 2026-08-24): the window bounds what the assistant
// SAYS, and a silent turn said nothing — the framework commits it as a
// real empty assistant text block — so the count subtracts every debt
// whose stored answer trims to nothing, matched through the answer's
// dispatch anchor, the id of the summoning frontier every block a turn
// writes carries. The reach is exactly the anchor's: a co-summoner
// absorbed into a silent turn keeps its own row's slot spent, because the
// anchor names the frontier alone — accepted, recorded with the decision.
// The `blocks` and `block_text` names are the framework's, the deliberate
// coupling decisions 0032 and 0079 record. The SQL trims the ASCII
// whitespace the wire realistically wraps an answer in; the edge's own
// check trims the full whitespace class, and the one divergence — an
// answer of nothing but exotic whitespace — errs toward counting, the
// limiting direction.

/// The counted-debt predicate both budget counts share, over the message
/// alias `m` and the block-header alias `b`: an opened debt — summoned,
/// not limited — younger than the window, whose modifier arrives as the
/// query's second parameter, and not answered by an empty answer — the
/// framework's committed record of a turn that said nothing. One fragment
/// on purpose: what consumes budget is one definition, and two spellings
/// of it could drift apart.
static COUNTED_DEBT_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "m.{COLUMN_ADDRESSED} = 1 AND m.{COLUMN_LIMITED} IS NULL \
         AND datetime(b.created_at) > datetime('now', ?2) \
         AND NOT EXISTS (\
           SELECT 1 FROM blocks ab \
           JOIN block_text at ON at.block_id = ab.id \
           WHERE ab.dispatch_anchor = m.block_id \
           AND at.role = 'assistant' \
           AND trim(at.content, ' ' || char(9) || char(10) || char(13)) = ''\
         )"
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

/// The framework's kinds as this consumer composes them: a transparent
/// delegation on every hook, projection included (unit 22, 2026-08-24). A
/// turn the model ended without writing any text is committed by the
/// framework as a real empty assistant text block, and it projects to the
/// model as its own empty message — the framework's intent: the model
/// reads its own silence back as the honest record of that turn.
///
/// Delegation is spelled per hook because the field is a wrapper, not the
/// derive's delegate directly; a framework hook added later lands here as
/// a compile-time absence only if it has no default, so the frontier
/// transparency pin in the provenance tests stands watch over the one
/// defaulted hook whose silent loss would change behavior.
pub struct FrameworkKind(pub BlockKind);

impl FromBlock for FrameworkKind {
    const DESCRIPTORS: &'static [ContentDescriptor] = BlockKind::DESCRIPTORS;
    const CLAIMED_KINDS: &'static [&'static str] = BlockKind::CLAIMED_KINDS;

    fn from_block(block: &Block) -> Self {
        Self(BlockKind::from_block(block))
    }
}

impl Agency for FrameworkKind {
    /// Transparent delegation with ONE consumer policy over it (unit 31,
    /// 2026-08-29): a quote asks for nothing here.
    ///
    /// The framework serves a model turn for any user-voiced frontier, and
    /// its quote is user-voiced — a person selecting a span in a composer
    /// IS an ask there. In this consumer nobody composes: a quote is
    /// context the ingest attaches ahead of the member's own message, and
    /// the turn duty lives on that message's answer-due stamp alone. Left
    /// delegating, a quote sitting bare — its message refused on retry, or
    /// the process restarted between the two appends — would draw a turn
    /// answering a quotation nobody asked about.
    ///
    /// It lives here, at the delegation, because this impl already IS the
    /// consumer's recorded policy over the framework's kinds; the
    /// alternative shapes were rejected with the decision — a leaf claiming
    /// the quote string overlaps the delegate's claim, which the derive's
    /// coherence assertion rightly refuses. Storage, projection and
    /// resolution are untouched: the quote still renders its `> `-prefixed
    /// lines above the message it precedes.
    fn awaiting(&self) -> Option<Awaiting> {
        match &self.0 {
            BlockKind::Quote(_) => None,
            kind => kind.awaiting(),
        }
    }

    fn durable(&self) -> bool {
        self.0.durable()
    }

    fn frontier_transparent(&self) -> bool {
        self.0.frontier_transparent()
    }

    fn gate<E: RuntimeEvent>(
        &self,
        ctx: &AgencyCtx<E>,
    ) -> impl Future<Output = GateDecision> + Send {
        self.0.gate(ctx)
    }

    fn run<E: RuntimeEvent>(
        &self,
        ctx: &AgencyCtx<E>,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send {
        self.0.run(ctx)
    }

    fn post_gate_id(&self, ledger: &[Block]) -> Option<i64> {
        self.0.post_gate_id(ledger)
    }

    fn run_post_gate<E: RuntimeEvent>(
        &self,
        ctx: &AgencyCtx<E>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        self.0.run_post_gate(ctx)
    }
}

impl Projection for FrameworkKind {
    fn group_role(&self) -> Option<Role> {
        self.0.group_role()
    }

    fn llm_parts(&self) -> Option<Vec<ContentPart>> {
        self.0.llm_parts()
    }

    fn llm_text(&self) -> Option<String> {
        self.0.llm_text()
    }

    fn forces_parts(&self) -> bool {
        self.0.forces_parts()
    }
}

/// The composed kind set the runtime is instantiated over: the framework's
/// kinds through the delegate, the assistant's beside them.
#[derive(Agency)]
pub enum AssistantKind {
    /// The framework's own kinds, resolved through the wrapping delegate.
    #[agency(delegate)]
    Core(FrameworkKind),
    /// The assistant's recorded channel message.
    ChatMessage(ChatMessage),
    /// The conversation's tool admission record (the tools module owns the
    /// kind; it composes here so one parse path reads every block).
    ToolPalette(crate::tools::palette::ToolPalette),
    /// A group's observed fact — its title or its rules (the note module
    /// owns the kind; it composes here so one parse path reads every
    /// block).
    ContextNote(crate::note::ContextNote),
    /// One person's recorded entry into a group (the join module owns the
    /// kind; it composes here so one parse path reads every block).
    JoinNotice(crate::join::JoinNotice),
    /// A filed report awaiting delivery (the report module owns the kind;
    /// it composes here so one parse path reads every block).
    Report(crate::tools::report::Report),
    /// One message the assistant successfully sent, as the platform took
    /// it (the delivery module owns the kind; it composes here so one
    /// parse path reads every block).
    Delivered(crate::delivery::Delivered),
    /// One emoji the assistant put on a message (the mark module owns the
    /// kind; it composes here so one parse path reads every block).
    MessageMark(crate::tools::mark::MessageMark),
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

#[cfg(test)]
mod tests {
    use agent_ledger::Store;

    use super::*;
    use crate::note::{CONTEXT_NOTE_KIND, ContextNote, NoteTopic};

    /// One summoned member message appended through the consumer write
    /// path, answering its block id.
    async fn summoned_message(store: &Store, conversation: i64, text: &str) -> i64 {
        store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                ChatMessage::stored_fields(
                    text,
                    RecordedSender {
                        principal_id: 7,
                        authority: Authority::Member,
                        speaker: None,
                    },
                    RecordedOrigin::default(),
                    None,
                    "2026-08-23T00:00:00Z",
                    Stamp::compose(
                        Summons {
                            summoned: true,
                            literal_addressed: true,
                        },
                        Authority::Member,
                        None,
                        None,
                    ),
                ),
                None,
            )
            .await
            .expect("the message appends")
    }

    /// One finalized assistant answer, anchored on the given summons the
    /// way the framework's dispatch writes it — the anchor set through the
    /// domain seam, since the anchored destination is the framework's own.
    async fn anchored_answer(store: &Store, conversation: i64, anchor: i64, content: &str) {
        let answer = store
            .insert_final_text_block(conversation, Role::Assistant, content.into(), None)
            .await
            .expect("the answer inserts");
        domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
            conn.execute(
                "UPDATE blocks SET dispatch_anchor = ?2 WHERE id = ?1",
                [answer, anchor],
            )?;
            Ok(())
        })
        .await
        .expect("the anchor writes");
    }

    /// The window's silence refund on its new key (unit 22, AC4): a debt
    /// whose anchored answer trims to nothing — the framework's committed
    /// empty block, surrounding whitespace tolerated — stops counting in
    /// both budget counts, while a debt answered with real text keeps its
    /// slot spent, a spoken "I don't know" included. The window bounds
    /// what the assistant SAYS.
    #[tokio::test]
    async fn a_debt_answered_by_an_empty_answer_stops_counting() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let tx = store.tx();
        let window = NonZeroU64::new(600).expect("a nonzero window");

        let first = summoned_message(&store, conversation, "the silent ask").await;
        assert_eq!(
            opened_debts_by_principal(&tx, 7, window)
                .await
                .expect("the count runs"),
            1,
            "an unanswered debt counts"
        );

        anchored_answer(&store, conversation, first, "  \n").await;
        assert_eq!(
            opened_debts_by_principal(&tx, 7, window)
                .await
                .expect("the count runs"),
            0,
            "the silent debt spends no slot"
        );
        assert_eq!(
            opened_debts_in_conversation(&tx, conversation, window)
                .await
                .expect("the count runs"),
            0,
            "the channel count excludes it the same way"
        );

        let second = summoned_message(&store, conversation, "the answered ask").await;
        anchored_answer(
            &store,
            conversation,
            second,
            "I don't know — I could not confirm this with a lookup.",
        )
        .await;
        assert_eq!(
            opened_debts_by_principal(&tx, 7, window)
                .await
                .expect("the count runs"),
            1,
            "a spoken answer keeps its slot spent, the model's own don't-know \
             included: real text is what the window bounds"
        );
    }

    /// The projection is a pure delegate (unit 22, AC5): the framework's
    /// committed empty assistant block projects to the model as its own
    /// empty message — an assistant boundary with an empty text
    /// contribution — exactly as the framework states it, with no
    /// suppression in between.
    #[test]
    fn an_empty_answer_projects_as_the_models_own_empty_message() {
        let text_block = |role: Role, content: &str| {
            let mut fields = serde_json::Map::new();
            fields.insert("role".into(), json!(role.as_str()));
            fields.insert("content".into(), json!(content));
            Block {
                id: 1,
                role: Some(role),
                block_type: "text".into(),
                created_at: String::new(),
                dispatch_anchor: None,
                fields,
            }
        };

        let silent = AssistantKind::from_block(&text_block(Role::Assistant, ""));
        assert_eq!(
            silent.group_role(),
            Some(Role::Assistant),
            "the empty turn opens its own assistant boundary"
        );
        assert_eq!(
            silent.llm_text(),
            Some(String::new()),
            "the contribution is the framework's own empty text"
        );

        let spoken = AssistantKind::from_block(&text_block(Role::Assistant, "a spoken answer"));
        assert_eq!(spoken.group_role(), Some(Role::Assistant));
        assert_eq!(
            spoken.llm_text(),
            Some("a spoken answer".to_owned()),
            "an ordinary answer projects exactly as the framework states it"
        );
    }

    /// The projected id mark (unit 15), every branch at the kind: a
    /// user-voiced message with a stored origin opens with the bracketed
    /// id — ahead of the speaker prefix where one stands, ahead of the
    /// bare text where none does — while a non-user voice, an origin-less
    /// row and the erased placeholder all project unmarked. The mark is
    /// what the model names a message by when it reports, so its exact
    /// composition is pinned here beside the rule.
    #[test]
    fn the_projection_marks_exactly_the_user_voiced_messages_with_an_origin() {
        assert_eq!(projected_origin_mark("id-9"), "[id-9]");

        let row = |role: Option<Role>,
                   text: Option<&str>,
                   speaker: Option<&str>,
                   origin: Option<&str>| {
            let mut fields = serde_json::Map::new();
            if let Some(text) = text {
                fields.insert(COLUMN_TEXT.into(), json!(text));
            }
            if let Some(speaker) = speaker {
                fields.insert(COLUMN_SPEAKER.into(), json!(speaker));
            }
            if let Some(origin) = origin {
                fields.insert(COLUMN_ORIGIN.into(), json!(origin));
            }
            <ChatMessage as agent_ledger::LeafKind>::parse(&Block {
                id: 1,
                role,
                block_type: CHAT_MESSAGE_KIND.into(),
                created_at: String::new(),
                dispatch_anchor: None,
                fields,
            })
        };

        assert_eq!(
            row(Some(Role::User), Some("the ask"), Some("ada"), Some("id-9")).projected_text(),
            "[id-9] ada: the ask",
            "the mark leads, then the speaker prefix, then the text"
        );
        assert_eq!(
            row(Some(Role::User), Some("the ask"), None, Some("id-9")).projected_text(),
            "[id-9] the ask",
            "a handleless message still shows its id"
        );
        assert_eq!(
            row(Some(Role::User), Some("the ask"), None, None).projected_text(),
            "the ask",
            "no stored origin, no mark"
        );
        assert_eq!(
            row(
                Some(Role::Assistant),
                Some("the answer"),
                None,
                Some("id-9")
            )
            .projected_text(),
            "the answer",
            "only the user's voice carries the mark"
        );
        assert_eq!(
            row(Some(Role::User), None, None, Some("id-9")).projected_text(),
            ERASED_MARKER,
            "the erased placeholder projects unmarked, even on a synthetic \
             row whose origin outlived the text"
        );
    }

    /// AC2 at the stamp (unit 16): the literal addressed fact is stored
    /// beside the summons without disturbing it. An unaddressed
    /// helpful-mode message composes summons=true — its debt opens, it
    /// counts, it co-summons — with literal=false; an addressed one
    /// composes both true; and the stored fields carry both columns
    /// exactly as composed, so the debt spine and the answer threading
    /// read two facts from one write.
    #[test]
    fn the_literal_fact_is_stored_beside_the_undisturbed_summons() {
        let unaddressed_helpful = Stamp::compose(
            Summons {
                summoned: true,
                literal_addressed: false,
            },
            Authority::Member,
            None,
            None,
        );
        assert!(unaddressed_helpful.addressed, "the summons stands");
        assert!(
            !unaddressed_helpful.literal_addressed,
            "the literal fact stays the adapter's own"
        );
        assert!(
            unaddressed_helpful.own_debt_taken(),
            "the opened-debt predicate reads the summons, never the literal fact"
        );
        assert!(unaddressed_helpful.answer_due, "the debt opens");

        let addressed = Stamp::compose(
            Summons {
                summoned: true,
                literal_addressed: true,
            },
            Authority::Member,
            None,
            None,
        );
        assert!(addressed.addressed && addressed.literal_addressed);

        let fields = ChatMessage::stored_fields(
            "the ask",
            RecordedSender {
                principal_id: 7,
                authority: Authority::Member,
                speaker: None,
            },
            RecordedOrigin::default(),
            None,
            "2026-08-24T00:00:00Z",
            unaddressed_helpful,
        );
        assert_eq!(fields[COLUMN_ADDRESSED], json!(true));
        assert_eq!(fields[COLUMN_LITERAL_ADDRESSED], json!(false));

        let parsed = <ChatMessage as agent_ledger::LeafKind>::parse(&Block {
            id: 1,
            role: Some(Role::User),
            block_type: CHAT_MESSAGE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        });
        assert_eq!(parsed.addressed, Some(true));
        assert_eq!(parsed.literal_addressed, Some(false));
        assert!(
            parsed.own_debt_taken(),
            "the row-side co-summoner reading is untouched by the literal fact"
        );
    }

    /// One chat row at the given text, recorded origin pair and summons —
    /// every other fact a fixed well-formed value, since the reads below
    /// judge the text, the stamp and the two identifiers alone. The pair
    /// travels whole, the way the kind's own field map takes it, so a
    /// revision row is one call and not a second helper.
    async fn append_chat_row(
        store: &Store,
        conversation: i64,
        text: &str,
        origin: RecordedOrigin<'_>,
        summoned: bool,
    ) {
        store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                ChatMessage::stored_fields(
                    text,
                    RecordedSender {
                        principal_id: 7,
                        authority: Authority::Member,
                        speaker: None,
                    },
                    origin,
                    None,
                    "2026-08-23T00:00:00Z",
                    Stamp::compose(
                        Summons {
                            summoned,
                            literal_addressed: summoned,
                        },
                        Authority::Member,
                        None,
                        None,
                    ),
                ),
                None,
            )
            .await
            .expect("the chat row appends");
    }

    /// A tail that is a run of read-through kinds, false-stamped chat rows
    /// and erased chat rows answers the block behind the whole run in one
    /// query; an empty kind list still reads past transparent rows and
    /// date markers, and an empty conversation answers nothing.
    ///
    /// The two transparency shapes are pinned one at a time and in
    /// isolation, because the SQL spells them disjunctively and either
    /// condition alone would satisfy a mixed run: the live false-stamped
    /// row carries its text, and the erased row above it carries a TRUE
    /// stamp, so neither row is transparent by the other's half.
    #[tokio::test]
    async fn the_read_answers_past_kind_runs_false_stamps_and_erased_rows_alike() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let tx = store.tx();
        assert_eq!(
            newest_block_id_past_transparent(&tx, conversation, &[CONTEXT_NOTE_KIND])
                .await
                .expect("the empty read runs"),
            None,
            "an empty conversation holds nothing to answer"
        );

        let behind = store
            .insert_text_block(conversation, Role::User, "the block behind".into())
            .await
            .expect("the text block appends");
        append_chat_row(
            &store,
            conversation,
            "an unsummoned line, its text intact",
            RecordedOrigin {
                origin: Some("live-false"),
                revises: None,
            },
            false,
        )
        .await;
        assert_eq!(
            newest_block_id_past_transparent(&tx, conversation, &[])
                .await
                .expect("the false-stamped read runs"),
            Some(behind),
            "a live row whose stamp is false is transparent on its own"
        );

        append_chat_row(
            &store,
            conversation,
            "soon deleted",
            RecordedOrigin {
                origin: Some("gone-1"),
                revises: None,
            },
            true,
        )
        .await;
        let target_rows = erase_message_named(&tx, conversation, "gone-1")
            .await
            .expect("the mirror pass runs");
        assert_eq!(target_rows, 1, "the named row is erased");
        assert_eq!(
            newest_block_id_past_transparent(&tx, conversation, &[])
                .await
                .expect("the erased read runs"),
            Some(behind),
            "an erased row is transparent though its own stamp reads true (decision 0086)"
        );
        store
            .append_consumer_block(
                conversation,
                None,
                CONTEXT_NOTE_KIND,
                ContextNote::stored_fields(NoteTopic::Rules, "the newest note"),
                None,
            )
            .await
            .expect("the note appends");

        assert_eq!(
            newest_block_id_past_transparent(&tx, conversation, &[CONTEXT_NOTE_KIND])
                .await
                .expect("the read-through runs"),
            Some(behind),
            "the note, the erased row and the false-stamped row are one transparent run"
        );
        let newest = newest_block_id_past_transparent(&tx, conversation, &[])
            .await
            .expect("the plain read runs")
            .expect("the conversation has an answerable block");
        assert!(
            newest > behind,
            "an empty kind list reads past transparent rows and date markers: the note answers"
        );
    }

    /// The named erasure's returned count is the number of VERSIONS it
    /// emptied, not a claim that one row was (unit T3, 2026-08-31). The
    /// chain is three: the original, a second version stored under the
    /// original's own id as this platform delivers it, and a third under an
    /// origin of its own as a platform delivering an edit as its own event
    /// would. A stranger row in the same conversation is there so the count
    /// cannot pass by counting everything, and narrowing the disjunction
    /// back to either column alone fails this.
    #[tokio::test]
    async fn the_named_erasure_counts_every_version_it_emptied() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let tx = store.tx();

        for origin in [
            RecordedOrigin {
                origin: Some("chain-1"),
                revises: None,
            },
            RecordedOrigin {
                origin: Some("chain-1"),
                revises: Some("chain-1"),
            },
            RecordedOrigin {
                origin: Some("chain-1-v3"),
                revises: Some("chain-1"),
            },
        ] {
            append_chat_row(&store, conversation, "one version of it", origin, true).await;
        }
        append_chat_row(
            &store,
            conversation,
            "someone else entirely",
            RecordedOrigin {
                origin: Some("stranger"),
                revises: None,
            },
            true,
        )
        .await;

        assert_eq!(
            erase_message_named(&tx, conversation, "chain-1")
                .await
                .expect("the mirror pass runs"),
            3,
            "the count reports the three versions emptied, and no row beyond them"
        );
        assert_eq!(
            erase_message_named(&tx, conversation, "chain-1")
                .await
                .expect("the second mirror pass runs"),
            0,
            "a second pass finds nothing: every emptied row's two identifiers are NULL"
        );
    }
}
