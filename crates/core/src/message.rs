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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderIdentity {
    /// The sender's opaque external id on the adapter's platform.
    pub external_id: String,
    /// The sender's username, where the platform has one.
    pub username: Option<String>,
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
    /// A reply to another person's message, named by that message's origin.
    Message {
        /// The platform's own id for the replied-to message, opaque.
        origin: String,
    },
    /// A reply to one of the assistant's own messages. No origin rides
    /// here: the assistant never reports itself, so nothing downstream
    /// threads onto it.
    AssistantMessage,
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
    /// The command the message invokes, as the adapter reports it beside
    /// the addressed flag — the core matches this, never the text.
    pub command: Option<InvokedCommand>,
    /// What was said, verbatim: the ledger records what the person typed,
    /// never a rewritten form (refined 2026-08-23).
    pub text: String,
    /// The platform's own id for the message, opaque, kept for later reply
    /// threading.
    pub origin: Option<String>,
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
}

/// One deterministic item a call returns for the adapter to deliver on the
/// channel — typed by what it is, so an adapter can present the kinds
/// differently without reading the text. The core still supplies the exact
/// wording for both, because wording is behavior and behavior stays out of
/// adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryItem {
    /// The rules acknowledgment: a rules note was appended outside the
    /// acknowledgment window.
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

/// What one observation call comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveOutcome {
    /// The observation was judged against the stored facts; a delta
    /// appended its note, an unchanged fact appended nothing.
    Observed {
        /// The item the adapter delivers on the channel — the rules
        /// acknowledgment, when a rules note was appended outside the
        /// acknowledgment window. `None` says nothing.
        deliver: Option<DeliveryItem>,
    },
    /// Refused fail-closed: the channel is a group the operator never
    /// admitted, or the membership observation named no admissible adder.
    /// Nothing touched the ledger; the adapter performs the withdrawal.
    Withdraw,
}

/// What one ingestion call comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome {
    /// The message was recorded on the ledger.
    Recorded {
        /// The ids the write resolved on the way in.
        receipt: IngestReceipt,
        /// The item the adapter delivers on the channel — the privacy
        /// command's answer. `None` says nothing.
        deliver: Option<DeliveryItem>,
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

/// What one outbound item is: the assistant's own prose, or the core's
/// notice that a turn failed. The marker exists so an adapter can present
/// the two differently without reading the text; the core still supplies the
/// text for both, because the wording is behavior and behavior stays out of
/// adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyKind {
    /// A finalized answer from the model.
    Answer,
    /// The failure notice: the turn did not produce an answer, said once,
    /// with no model prose.
    Notice,
    /// A filed report's fixed line (decided 2026-08-23): the core's own
    /// machinery text, delivered threaded onto the reported message.
    Report,
}

/// Whether the assistant is composing an answer on a channel right now.
///
/// A live presence cue, not a delivery: it exists only while the process
/// runs, is never stored, and owes nothing across a restart. The core
/// derives it from the turn lifecycle — composing from the moment a turn is
/// owed and being worked, stopped on the turn's completion or failure — so
/// a deterministic reply, which takes no turn, never composes.
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

/// One reply on its way out of the core, bound to the channel it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundReply {
    /// The channel the reply belongs on.
    pub channel: ChannelKey,
    /// What the assistant says.
    pub text: String,
    /// Whether this is the assistant's answer, the core's failure notice,
    /// or a filed report's line.
    pub kind: ReplyKind,
    /// The platform origin of the message this reply answers, where the
    /// reply threads (decided 2026-08-23, the deferral of decision 0018
    /// falling due). The adapter translates it into the platform's reply
    /// parameters with send-without-reply tolerance — a deleted target
    /// degrades to a plain send — and threads only the first chunk. The
    /// model's answers stay unthreaded in this unit: only the report's
    /// delivery sets a target, and the field exists for whatever
    /// answer-threading decision comes later.
    pub reply_target: Option<String>,
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
