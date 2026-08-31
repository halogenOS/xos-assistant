//! The core's own message vocabulary: one inbound message, one outbound reply.
//!
//! Adapters translate their platform's types into this model at the boundary
//! and never past it. Nothing in here names a platform; an adapter is a name
//! the adapter chose for itself, and every identifier from the platform's side
//! is opaque here — compared for equality, never interpreted.

use chrono::{DateTime, Utc};

/// Where a message lives: one conversation surface on one adapter.
///
/// An opaque pair — the adapter's registered name plus the adapter's own
/// conversation identifier — compared only for equality. The core stores it in
/// exactly one place, the channel-to-conversation mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelKey {
    /// The adapter's registered name.
    pub adapter: String,
    /// The adapter's own identifier for the conversation surface.
    pub channel: String,
}

/// What a channel's key names: a person, or a group.
///
/// A direct channel's key identifies a person and is personal data; a group
/// channel's key names the group. Erasure treats the two differently, so the
/// mapping records the kind at creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    /// A one-on-one channel; its key identifies a person.
    Direct,
    /// A group channel; its key names the group.
    Group,
}

impl ChannelKind {
    /// Every variant, in stored-encoding order — what closes the vocabulary
    /// in the schema's CHECK constraint, so the constraint and this enum
    /// cannot drift apart.
    pub const ALL: [Self; 2] = [Self::Direct, Self::Group];

    /// The stored encoding, a closed vocabulary.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Group => "group",
        }
    }

    /// Parse the stored encoding back, `None` for anything outside the
    /// vocabulary.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        match stored {
            "direct" => Some(Self::Direct),
            "group" => Some(Self::Group),
            _ => None,
        }
    }
}

/// Who sent a message, as the adapter saw them.
///
/// The input to principal resolution, plus one recorded fact: the entry point
/// resolves or creates the principal from it, the principal id enters the
/// ledger — and since decision 0065 the username joins the message row as its
/// speaker, bounded by the kind's storable-speaker predicate. The external id
/// never reaches a block. The display name is not carried at all (decision
/// 0077): nothing consumed it, so the adapter stops translating it and the
/// core stores what it needs and nothing it does not.
///
/// Three facts cross the boundary, not two (2026-08-30, widening 0077): the
/// automation fact below joined the pair. It is the one field here that is
/// stored NOWHERE — read fresh off every update, consumed while the message
/// is decided, and never written to a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderIdentity {
    /// The sender's opaque external id on the adapter's platform.
    pub external_id: String,
    /// The sender's username, where the platform has one.
    pub username: Option<String>,
    /// Whether the account that sent the message is automated instead of a
    /// person — the platform's own fact about the account, translated by
    /// the adapter (2026-08-30). It is a property of the ACCOUNT, not of
    /// one message of it, so it rides the identity and not the message; a
    /// platform that marks nothing leaves it false, which is exactly what
    /// "no automated account is known here" means.
    ///
    /// On a message's sender, three readings consume it, all of them at
    /// the moment the message is decided: the adapter narrows an automated
    /// sender's addressing to the deliberate mention, the core's summons
    /// resolution never summons such a sender by mode, and the core's
    /// stamp write withholds the owing tail from an unsummoned automated
    /// sender's row so no bot carries somebody else's debt into a turn
    /// (decisions 0152, 0153, 0154).
    ///
    /// On a join notice's joiner the fact is carried and read by nothing:
    /// a joiner is not a sender, so no addressing and no summons is decided
    /// for them. It rides anyway, because this is the one identity a
    /// joiner crosses the boundary with and the honest value of an
    /// account's own flag is the platform's, never a per-site `false`
    /// invented to fill the field (decision 0151 fills all three building
    /// sites from their own account's flag).
    ///
    /// It reaches no column, no migration and no erasure pass — a stored
    /// copy would only drift from the account's current state.
    pub bot: bool,
}

/// The sender's standing in the channel, resolved live by the adapter at
/// receipt.
///
/// The variants are ordered: `Member` < `Moderator` < `Admin`. The stored
/// encoding is the fixed text vocabulary below; the ordering lives here, in
/// code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Authority {
    /// An ordinary member.
    Member,
    /// A moderator of the channel.
    Moderator,
    /// An administrator of the channel.
    Admin,
}

impl Authority {
    /// Every variant, in ascending order of standing — what closes the
    /// vocabulary in the schema's CHECK constraint, so the constraint and
    /// this enum cannot drift apart.
    pub const ALL: [Self; 3] = [Self::Member, Self::Moderator, Self::Admin];

    /// The stored encoding, a closed vocabulary.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Moderator => "moderator",
            Self::Admin => "admin",
        }
    }

    /// Parse the stored encoding back, `None` for anything outside the
    /// vocabulary.
    #[must_use]
    pub fn parse(stored: &str) -> Option<Self> {
        match stored {
            "member" => Some(Self::Member),
            "moderator" => Some(Self::Moderator),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

/// The command a message invokes, as the adapter translated it: the leading
/// command token of the text as sent, with a self-directed handle suffix
/// normalized away — platform knowledge, resolved at the boundary. Which
/// commands exist and what they answer stays the core's; the adapter only
/// reports the invocation, and the core matches the reported command, never
/// the stored text (refined 2026-08-23). A token aimed at a foreign handle
/// is no invocation and reports nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvokedCommand(String);

impl InvokedCommand {
    /// The reported invocation, by its bare command token.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The bare command token, the assistant's own handle already stripped.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// What a message replies to, as the adapter resolved it at the boundary
/// (decided 2026-08-23): the platform's own id for the replied-to message —
/// opaque, kept for reply threading and the report tool's target resolution
/// — or the fact that the reply points at one of the assistant's own
/// messages, which the report tool refuses with its own error. A reply the
/// platform carries no usable id for translates to no target at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyTarget {
    /// A reply to a message that is not the assistant's own — any person's,
    /// the sender's included — named by that message's origin.
    Message {
        /// The platform's own id for the replied-to message, opaque.
        origin: String,
    },
    /// A reply to one of the assistant's own messages, named by that
    /// message's origin where the platform carried one (unit 38,
    /// 2026-08-30). The origin is consumed during ingestion — it resolves
    /// which of her recorded deliveries the reply points at, so the reply
    /// can quote her stored words — and it is never stored on the chat
    /// message: [`crate::kind::ChatMessage::stored_fields`] keeps writing
    /// the reply-to-assistant flag alone, so the reply-target column stays
    /// the personal-data column its own documentation describes. `None`
    /// for a reply the platform carried no usable id for.
    AssistantMessage {
        /// The platform's own id for the replied-to message of hers,
        /// opaque.
        origin: Option<String>,
    },
}

/// The excerpt a reply quotes of the message it replies to, as the adapter
/// translated it (unit 31, 2026-08-28): the quoted words, and whether the
/// member selected them themselves.
///
/// Two fields and no more. A platform that carries an OFFSET beside the
/// excerpt carries it in its own text encoding, and converting one
/// encoding's offsets onto stored text is arithmetic this repository has
/// been burned by; the core decides where the excerpt sits by searching
/// the stored text for it instead, so the offset never crosses the
/// boundary and there is nothing here to convert. The excerpt is
/// meaningful only beside a [`ReplyTarget::Message`]: it says which part
/// of THAT message the reply points at, and without a resolvable target
/// there is nothing for it to narrow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotedExcerpt {
    /// The quoted words as the platform delivered them.
    pub text: String,
    /// Whether the member chose the excerpt by hand. A hand-chosen excerpt
    /// is the member narrowing what they answer; anything the platform
    /// composed on its own narrows nothing, because the member never
    /// pointed at a part.
    pub manual: bool,
}

/// One message on its way into the core.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Where the message was said.
    pub channel: ChannelKey,
    /// Whether the channel names a person or a group.
    pub channel_kind: ChannelKind,
    /// Who said it, as the adapter saw them.
    pub sender: SenderIdentity,
    /// The sender's standing in the channel at receipt. `None` says the
    /// adapter's authority source failed and the standing is unresolved —
    /// delivered anyway, because admission is judged before authority
    /// (refined 2026-08-23): an unadmitted group is refused with the
    /// withdraw directive without this field ever being read, and an
    /// admitted message with no authority is refused with the typed
    /// transient [`crate::CoreError::AuthorityUnresolved`], never recorded
    /// with a default.
    pub authority: Option<Authority>,
    /// Whether the message addresses the assistant. What "addressed" means on
    /// a platform — a direct chat, a mention of the assistant, a reply to one
    /// of its messages — is platform knowledge the adapter resolves; the core
    /// receives only the neutral fact and stores it on the message block.
    pub addressed: bool,
    /// What the message replies to, where the platform carries a usable
    /// reply (decided 2026-08-23) — translated beside the addressed flag
    /// and stored on the message block, under the same erasure null as the
    /// origin. `None` for a non-reply and for a reply without a usable id.
    pub reply_target: Option<ReplyTarget>,
    /// The part of the replied-to message this reply quotes, where the
    /// platform reports one (unit 31, 2026-08-28). Nothing is stored from
    /// it: it decides only WHICH span of the target the quote block
    /// references. `None` for a message quoting nothing in particular, in
    /// which case a reply quotes its target whole.
    pub quoted: Option<QuotedExcerpt>,
    /// The command the message invokes, as the adapter reports it beside
    /// the addressed flag — the core matches this, never the text.
    pub command: Option<InvokedCommand>,
    /// What was said, verbatim: the ledger records what the person typed,
    /// never a rewritten form (refined 2026-08-23).
    pub text: String,
    /// The platform's own id for the message, opaque, kept for later reply
    /// threading.
    pub origin: Option<String>,
    /// The opaque origin of the message this one supersedes — a member's
    /// edit reported as a new version of something they already said (unit
    /// T3, 2026-08-31). `None` for an ordinary message, which supersedes
    /// nothing.
    ///
    /// The value names the message as FIRST known, never the version
    /// immediately superseded: a third edit of one message reports the same
    /// identifier as the first, so every version of one message shares one
    /// key and a single match on THAT key reaches them all — which is what
    /// lets erasure and the report resolve a whole chain without walking
    /// it. On a platform where an edit arrives under the same identifier as
    /// the original, this equals [`Self::origin`] and every id the platform
    /// can name for that message is the shared key. On one where a revision
    /// carries an identifier of its own, the two differ: the readings that
    /// match either column — the newest-version read, the mirror's named
    /// erasure, the report's resolution — then reach the whole chain from
    /// the ORIGINAL's identifier and one row from a later version's, which
    /// is why such an adapter owes a root-resolution step before it reports
    /// a revision at all — the obligation decision 0171 states, and the one
    /// place it is stated. The reply quote's read is narrower still and
    /// matches [`Self::origin`] alone, so a quote resolves to the version
    /// stored under the id the reply named. WHICH update is a revision is
    /// the adapter's reading of the platform's own event type, never an
    /// inference from a field's presence, and the core never learns which
    /// platform reported it.
    pub revises: Option<String>,
    /// When the platform says the message was sent. Recorded on the message
    /// block, so the ledger keeps both times: the platform's send time from
    /// this field, and the store's own insertion time on the block header.
    pub timestamp: DateTime<Utc>,
}

/// One platform-neutral observation on its way into the core: a fact about
/// a channel, read from the channel itself — never configuration. The
/// observation surface judges it against the stored facts and answers with
/// an [`ObserveOutcome`]; everything deterministic it decides rides that
/// return, never the event edge (decided 2026-08-23).
#[derive(Debug, Clone)]
pub struct Observation {
    /// The channel the fact is about.
    pub channel: ChannelKey,
    /// Whether the channel names a person or a group — checked against the
    /// stored mapping exactly as ingestion checks it.
    pub channel_kind: ChannelKind,
    /// The observed fact itself.
    pub fact: ObservedFact,
}

/// One person as a join notice showed them (unit 36, 2026-08-29): the
/// identity the joiner's principal resolves from — the same two fields a
/// sender crosses the boundary with — plus the name the platform displayed
/// beside the entry.
///
/// The shown name rides here and nowhere else. Decision 0077 is not
/// reopened: a message still carries no display name, because a message's
/// content is what was said. A join notice's content IS the shown name, so
/// it is recorded once, on the event that carried it, and erased with the
/// person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedMember {
    /// Who joined, as the adapter saw them — the input to principal
    /// resolution, exactly as a sender's is.
    pub identity: SenderIdentity,
    /// The name the platform displayed for the joiner, already composed by
    /// the adapter into the one string members saw. Empty when the platform
    /// showed no name at all: nothing is invented in its place, and the
    /// projected line falls back to the handle.
    pub name: String,
}

/// What one observation reports.
#[derive(Debug, Clone)]
pub enum ObservedFact {
    /// The channel's title, as the platform shows it.
    Title(String),
    /// The text of the channel's pinned announcement — the one the
    /// platform's lookup exposes, or the one a pin event names. Whether it
    /// is the group's rules is the core's contract, never the adapter's.
    PinnedAnnouncement(String),
    /// The assistant itself entered the channel's member set.
    Added {
        /// The acting principal — who admitted the assistant, as the
        /// adapter saw them. Absence fails closed: an add nobody is named
        /// for is nobody's invitation.
        by: Option<SenderIdentity>,
    },
    /// People entered the channel's member set, as one platform service
    /// message reported them (unit 36, 2026-08-29). One event, a list of
    /// joiners: the observation surface stores one block per joiner behind
    /// its existing authorization gate, all of them under this event's one
    /// origin. The assistant's own entry is never in the list — its own
    /// membership is [`ObservedFact::Added`]'s territory — and a joiner
    /// whose suppression flag stands is stored not at all.
    MembersJoined {
        /// The joiners the event named, in the platform's own order.
        joiners: Vec<JoinedMember>,
        /// The service message's own id, opaque — what every joiner's
        /// block records, so a report can name the event and the human
        /// side can act on it.
        origin: String,
        /// When the platform says the service message was sent.
        timestamp: DateTime<Utc>,
    },
}

/// One deterministic item a call returns for the adapter to deliver on the
/// channel — typed by what it is, so an adapter can present the kinds
/// differently without reading the text. The core still supplies the exact
/// wording for both, because wording is behavior and behavior stays out of
/// adapters.
///
/// This is what a deterministic call returns SYNCHRONOUSLY on its receipt.
/// What the asynchronous replies channel yields from a model turn is
/// [`Outbound`], a separate type: merging the two would put a return value
/// and a channel element in one enum whose arms are unreachable from half
/// its call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryItem {
    /// The rules acknowledgment: a rules note was appended — every real
    /// delta carries one (the operator decided, 2026-08-23).
    Acknowledgment(String),
    /// A command's fixed answer — the privacy command's, in this unit.
    CommandAnswer(String),
}

impl DeliveryItem {
    /// The delivered text, whichever kind carries it.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Acknowledgment(text) | Self::CommandAnswer(text) => text,
        }
    }
}

/// Where one of the assistant's own sends is recorded (unit 38,
/// 2026-08-30): the conversation the sent message belongs to, and the
/// stored block a reply to that message quotes, where the send carried one
/// of her blocks at all.
///
/// Opaque to adapters. An adapter receives a handle beside the text it is
/// asked to send and hands the same handle back to
/// [`crate::Assistant::report_delivery`] once the platform has taken the
/// message; it reads nothing out of it and decides nothing from it. The
/// two values inside are the core's own: which ledger the delivery record
/// is appended to, and which block that record points a later quote at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryHandle {
    /// The conversation the sent message belongs to.
    pub(crate) conversation_id: i64,
    /// The stored block a reply to this send quotes. `None` where the
    /// send carries no quotable block of the assistant's own: a report's
    /// line, whose block declares no quotable column, and every
    /// deterministic item, which is fixed prose and stored nowhere.
    pub(crate) quotable_block: Option<i64>,
}

impl DeliveryHandle {
    /// The handle of a send that carries no quotable block of the
    /// assistant's own.
    pub(crate) fn in_conversation(conversation_id: i64) -> Self {
        Self {
            conversation_id,
            quotable_block: None,
        }
    }

    /// The same handle, naming the stored block a reply to this send
    /// quotes.
    pub(crate) fn quoting(self, quotable_block: Option<i64>) -> Self {
        Self {
            quotable_block,
            ..self
        }
    }

    /// The conversation the delivery record is appended to.
    pub(crate) fn conversation_id(self) -> i64 {
        self.conversation_id
    }

    /// The block a reply to this send quotes, where there is one.
    pub(crate) fn quotable_block(self) -> Option<i64> {
        self.quotable_block
    }
}

/// One deterministic item an observation returns, with the handle its send
/// is recorded under (unit 38, 2026-08-30).
///
/// The two ride together because a delivered item and the place its
/// delivery is recorded are one fact: an item with nowhere to record its
/// send cannot exist, and pairing them keeps the adapter from ever having
/// to decide what to do with one half. The ingestion side needs no pairing
/// — its [`IngestReceipt`] always names the conversation — and answers the
/// same handle through [`IngestReceipt::delivery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDelivery {
    /// Where the send is recorded, handed back to the core afterwards.
    pub delivery: DeliveryHandle,
    /// What the adapter delivers on the channel.
    pub item: DeliveryItem,
}

/// What one observation call comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveOutcome {
    /// The observation was judged against the stored facts; a delta
    /// appended its note, an unchanged fact appended nothing.
    Observed {
        /// The item the adapter delivers on the channel — the rules
        /// acknowledgment, when a rules note was appended — with the
        /// handle its send is recorded under. `None` says nothing.
        deliver: Option<ObservedDelivery>,
    },
    /// Refused fail-closed: the channel is a group the operator never
    /// admitted, or the membership observation named no admissible adder.
    /// Nothing touched the ledger; the adapter performs the withdrawal.
    Withdraw,
}

/// What the write did to the channel's session, and therefore what the
/// adapter must forget about the channel (unit 45, 2026-08-30).
///
/// The core decides; the adapter translates mechanically, exactly as it does
/// with the withdraw directive. Anything an adapter derived from an earlier
/// contact with this channel — a once-per-process enrichment above all — was
/// derived for a session that no longer exists, so the channel's next
/// contact starts over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelReset {
    /// The channel keeps the session it had; there is nothing to forget.
    #[default]
    Kept,
    /// The channel's session was replaced with an empty one. Whatever the
    /// adapter remembers about this channel from an earlier contact no
    /// longer describes the conversation it now speaks into.
    Replaced,
}

/// What one ingestion call comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome {
    /// The message was recorded on the ledger.
    Recorded {
        /// The ids the write resolved on the way in.
        receipt: IngestReceipt,
        /// The item the adapter delivers on the channel — a recognized
        /// command's answer. `None` says nothing.
        deliver: Option<DeliveryItem>,
        /// What the write did to the channel's session. Anything but
        /// [`ChannelReset::Kept`] is a directive the adapter carries out.
        reset: ChannelReset,
    },
    /// Refused fail-closed: the channel is a group the operator never
    /// admitted. Nothing touched the ledger; the adapter performs the
    /// withdrawal.
    Withdraw,
    /// Refused without effect at the person's own ask or the operator's
    /// switch (widened 2026-08-23): the channel is direct and the
    /// assembly's configuration serves no direct chats, or the sender's
    /// standing suppression flag drops the message at ingestion. Nothing
    /// touched the ledger or the identity tables, and nothing is delivered
    /// — there is no directive to perform, so the adapter simply
    /// acknowledges the message and moves on.
    Disregarded,
}

/// What one accepted ingestion reports back: the ids the core resolved on
/// the way in. The principal id is the handle a later
/// [`crate::Assistant::erase_principal`] call needs, so an operator surface
/// built on the adapter has a lawful path to it without reading the core's
/// tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestReceipt {
    /// The principal the sender resolved or created.
    pub principal_id: i64,
    /// The conversation the message was recorded in.
    pub conversation_id: i64,
}

impl IngestReceipt {
    /// Where a deterministic item returned beside this receipt has its
    /// send recorded (unit 38, 2026-08-30): this ingestion's own
    /// conversation, carrying no quotable block — an item is the core's
    /// fixed prose and no block of the assistant's own.
    #[must_use]
    pub fn delivery(&self) -> DeliveryHandle {
        DeliveryHandle::in_conversation(self.conversation_id)
    }
}

/// What one outbound item is: the assistant's own prose, or a filed
/// report's machinery line. The marker exists so an adapter can present the
/// two differently without reading the text; the core still supplies the
/// text for both, because the wording is behavior and behavior stays out of
/// adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyKind {
    /// A finalized answer from the model.
    Answer,
    /// A filed report's fixed line (decided 2026-08-23): the core's own
    /// machinery text, delivered threaded onto the reported message.
    Report,
}

/// Whether the assistant is composing an answer on a channel right now.
///
/// A live presence cue, not a delivery: it exists only while the process
/// runs, is never stored, and owes nothing across a restart. The core
/// derives it from the turn lifecycle; the authoritative statement of when
/// the cue is on lives on the composing edge (`crate::composing`) — in
/// short, on while the model is composing (its thinking and its streaming),
/// and off during a tool call and a human wait, so a deterministic reply,
/// which takes no turn, never composes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposingState {
    /// The assistant is composing an answer on the channel.
    Composing,
    /// The composing ended: the turn completed or failed.
    Stopped,
}

/// One change of the composing signal, bound to the channel it is about.
/// The edge yields transitions only — one `Composing` when a channel's turn
/// begins, one `Stopped` when it ends — so an adapter holds its platform's
/// indicator between the two without deduplicating anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposingUpdate {
    /// The channel the signal is about.
    pub channel: ChannelKey,
    /// The new state.
    pub state: ComposingState,
}

/// Where one outbound reply threads, and what the core wants done when the
/// platform refuses that threaded send (2026-08-24).
///
/// The two travel together because they are one decision about one reply,
/// and it is the core's: whether the words still mean something once the
/// thread is gone is a fact about the words. An adapter that read
/// [`ReplyKind`] to work the recovery out for itself would be deciding,
/// and adapters decide nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyThread {
    /// Threaded onto this origin, and where the platform refuses that
    /// send, the same text goes out once more without a target
    /// (decision 0109). The thread is a courtesy, and an answer must never
    /// be lost to a courtesy.
    OntoOrPlainly(String),
    /// Threaded onto this origin or not delivered at all. The report's
    /// line is the moderation bot's own command shape, which means what it
    /// means only as a reply: sent plainly it files nothing and leaves a
    /// bare command line standing in the group, so a refused send stays
    /// the send's own failure, logged and dropped exactly as before
    /// threading gained a recovery.
    OntoOnly(String),
}

impl ReplyThread {
    /// The platform origin the reply threads onto.
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::OntoOrPlainly(origin) | Self::OntoOnly(origin) => origin,
        }
    }

    /// Whether the same text goes out once more without the target when
    /// the platform refuses the threaded send.
    #[must_use]
    pub fn plain_when_refused(&self) -> bool {
        matches!(self, Self::OntoOrPlainly(_))
    }
}

/// One item on the assistant's outbound edge (unit 39, 2026-08-30). The
/// edge carried prose alone until a reaction joined it, and the two are
/// separate arms rather than a reply with an empty text and a glyph
/// smuggled beside it: every consumer of [`OutboundReply`] would otherwise
/// have to remember that one kind means "do not send this text".
///
/// Not to be confused with [`DeliveryItem`], which stays its own type: a
/// delivery item is what a deterministic call returns SYNCHRONOUSLY on its
/// receipt, while this is what the asynchronous replies channel yields
/// from a model turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outbound {
    /// Words to send: an answer, or a filed report's line.
    Reply(OutboundReply),
    /// One emoji to put on a message. It carries no delivery handle: a
    /// reaction is the cheap act, and a cheap act earns no bookkeeping row
    /// (unit 39). Nothing completes that symmetry.
    Mark(OutboundMark),
}

/// One reaction on its way out of the core, bound to the channel it
/// belongs on and the message it sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundMark {
    /// The channel the marked message is on.
    pub channel: ChannelKey,
    /// The emoji the model chose, exactly as it was stored. What the
    /// platform can actually place is the adapter's fact: a pick outside
    /// its set is dropped there, with a log line and no report back.
    pub emoji: String,
    /// The message the reaction goes on. A plain `String` because an item
    /// on the edge always names its target: a mark whose stored origin an
    /// erasure or the deletion mirror nulled is skipped at the edge and
    /// never reaches an adapter, so no adapter has to decide what an item
    /// with no target means.
    pub target_origin: String,
}

/// One reply on its way out of the core, bound to the channel it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundReply {
    /// The channel the reply belongs on.
    pub channel: ChannelKey,
    /// What the assistant says.
    pub text: String,
    /// Whether this is the assistant's answer or a filed report's line.
    pub kind: ReplyKind,
    /// The message this reply threads onto and how a refused thread is
    /// recovered, where it threads at all (decided 2026-08-23, the
    /// deferral of decision 0018 falling due). The adapter translates the
    /// origin into the platform's reply parameters with send-without-reply
    /// tolerance — a deleted target degrades to a plain send — and threads
    /// only the first chunk. A report's delivery names the reported
    /// message's origin; an answer names the origin of the one message
    /// that addressed the assistant this turn, and `None` says the reply
    /// goes out plain: an answer nobody or several addressed, and an
    /// answer whose prose carries the moderation command shape
    /// (2026-08-24).
    pub reply_target: Option<ReplyThread>,
    /// Where this reply's send is recorded (unit 38, 2026-08-30): the
    /// conversation it was read from, and — for the assistant's own
    /// answer — the stored block a member replying to it quotes. The
    /// adapter hands it back to [`crate::Assistant::report_delivery`]
    /// after the send and reads nothing out of it.
    pub delivery: DeliveryHandle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_orders_member_below_moderator_below_admin() {
        assert!(Authority::Member < Authority::Moderator);
        assert!(Authority::Moderator < Authority::Admin);
    }

    #[test]
    fn stored_vocabularies_round_trip_and_reject_strangers() {
        for authority in Authority::ALL {
            assert_eq!(Authority::parse(authority.as_str()), Some(authority));
        }
        assert_eq!(Authority::parse("owner"), None);

        for kind in ChannelKind::ALL {
            assert_eq!(ChannelKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ChannelKind::parse("broadcast"), None);
    }
}
