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
/// This is the input to principal resolution and nothing else: the entry point
/// resolves or creates the principal from it, and only the principal id enters
/// the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderIdentity {
    /// The sender's opaque external id on the adapter's platform.
    pub external_id: String,
    /// The name the sender displays.
    pub display_name: String,
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

/// One message on its way into the core.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Where the message was said.
    pub channel: ChannelKey,
    /// Whether the channel names a person or a group.
    pub channel_kind: ChannelKind,
    /// Who said it, as the adapter saw them.
    pub sender: SenderIdentity,
    /// The sender's standing in the channel at receipt.
    pub authority: Authority,
    /// What was said.
    pub text: String,
    /// The platform's own id for the message, opaque, kept for later reply
    /// threading.
    pub origin: Option<String>,
    /// When the platform says the message was sent. Recorded on the message
    /// block, so the ledger keeps both times: the platform's send time from
    /// this field, and the store's own insertion time on the block header.
    pub timestamp: DateTime<Utc>,
}

/// One reply on its way out of the core, bound to the channel it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundReply {
    /// The channel the reply belongs on.
    pub channel: ChannelKey,
    /// What the assistant says.
    pub text: String,
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
