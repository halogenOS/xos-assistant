//! Translation of one platform update into either a message the core can
//! record or a named skip — pure, synchronous, and decision-shaped: every
//! branch here restates a recorded decision, never invents one.
//!
//! Authority is the one part translation cannot settle alone: a group
//! sender's standing comes from the administrator list, which is a wire
//! concern. So translation yields a [`Pending`] message carrying the
//! authority where translation can settle it and leaving it open where the
//! administrator list must answer, and the driver finishes the job.
//!
//! The channel key is translation too: [`channel_key`] and [`chat_id_of`]
//! are the two directions of one platform-to-core naming rule, kept side by
//! side so neither direction can drift from the other.

use chrono::{DateTime, Utc};

use assistant_core::{Authority, ChannelKey, ChannelKind, SenderIdentity};

use crate::ADAPTER_NAME;
use crate::client::{Incoming, Update};

/// What one update translates to.
pub(crate) enum Translation {
    /// The update is not recorded; the reason names the decision that says
    /// so. A skip is acknowledged past like a success.
    Skip(Skip),
    /// A message to record, pending authority resolution for groups.
    Record(Pending),
}

/// Every reason an update is skipped, each one a recorded decision or a
/// well-formedness guard, so a log line names the case instead of a shrug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skip {
    /// The update carries no new message at all (decision 0017).
    NonMessage,
    /// An edit to an existing message; the ledger keeps the message as
    /// first seen (decision 0017).
    EditedMessage,
    /// A broadcast-channel post; no conversation this assistant serves
    /// (the loop contract).
    ChannelBroadcast,
    /// Sent on behalf of a chat — an anonymous administrator or a linked
    /// channel — with no resolvable person behind it (decision 0016).
    OnBehalfOfChat,
    /// A message with neither text nor caption (decision 0017).
    NoText,
    /// A message without a sending person, such as a service message.
    NoSender,
    /// A chat type outside the platform's documented vocabulary; nothing
    /// is known about what its key would name, so nothing is recorded.
    UnknownChatKind,
    /// A send time outside the representable range — a malformed update.
    BadTimestamp,
}

/// One message ready for the core, minus the authority a group still owes.
pub(crate) struct Pending {
    /// The chat the message was said in; [`channel_key`] names it for the
    /// core.
    pub chat_id: i64,
    pub channel_kind: ChannelKind,
    /// The sender's standing where translation alone settles it: a direct
    /// chat's sender is a member by decision 0015. `None` in a group, where
    /// the administrator list must answer.
    pub authority: Option<Authority>,
    pub sender: SenderIdentity,
    /// The sender's platform id, what the administrator list is matched on.
    pub sender_id: i64,
    pub text: String,
    /// The platform's message id, the origin reference.
    pub origin: String,
    /// The platform's send time.
    pub sent_at: DateTime<Utc>,
}

/// Translate one update per the recorded decisions.
pub(crate) fn translate(update: &Update) -> Translation {
    if update.edited_message.is_some() {
        return Translation::Skip(Skip::EditedMessage);
    }
    let Some(message) = &update.message else {
        return Translation::Skip(Skip::NonMessage);
    };
    let (channel_kind, authority) = match message.chat.kind.as_str() {
        "private" => (ChannelKind::Direct, Some(Authority::Member)),
        "group" | "supergroup" => (ChannelKind::Group, None),
        "channel" => return Translation::Skip(Skip::ChannelBroadcast),
        _ => return Translation::Skip(Skip::UnknownChatKind),
    };
    if message.sender_chat.is_some() {
        return Translation::Skip(Skip::OnBehalfOfChat);
    }
    let Some(from) = &message.from else {
        return Translation::Skip(Skip::NoSender);
    };
    let Some(text) = text_of(message) else {
        return Translation::Skip(Skip::NoText);
    };
    let Some(sent_at) = DateTime::from_timestamp(message.date, 0) else {
        return Translation::Skip(Skip::BadTimestamp);
    };
    Translation::Record(Pending {
        chat_id: message.chat.id,
        channel_kind,
        authority,
        sender: SenderIdentity {
            external_id: from.id.to_string(),
            display_name: display_name(&from.first_name, from.last_name.as_deref()),
            username: from.username.clone(),
        },
        sender_id: from.id,
        text: text.to_owned(),
        origin: message.message_id.to_string(),
        sent_at,
    })
}

/// The core-side key of one chat: the adapter's pinned name plus the chat
/// id in decimal. The decimal form is a durable contract — it keys the
/// channel mappings — so it lives here, beside its inverse.
pub(crate) fn channel_key(chat_id: i64) -> ChannelKey {
    ChannelKey {
        adapter: ADAPTER_NAME.into(),
        channel: chat_id.to_string(),
    }
}

/// The chat a channel key names — the inverse of [`channel_key`]. `None`
/// names a corrupted mapping, not an expected path: the adapter minted
/// every key it subscribes to.
pub(crate) fn chat_id_of(key: &ChannelKey) -> Option<i64> {
    key.channel.parse().ok()
}

/// The message's text, or its caption when the message is media with a
/// caption (decision 0017).
fn text_of(message: &Incoming) -> Option<&str> {
    message
        .text
        .as_deref()
        .or(message.caption.as_deref())
        .filter(|text| !text.is_empty())
}

/// The name a person displays: the first name, with the last name appended
/// where the platform carries one.
fn display_name(first: &str, last: Option<&str>) -> String {
    match last {
        Some(last) => format!("{first} {last}"),
        None => first.to_owned(),
    }
}

impl std::fmt::Display for Skip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::NonMessage => "a non-message update",
            Self::EditedMessage => "an edit to an existing message",
            Self::ChannelBroadcast => "a broadcast-channel post",
            Self::OnBehalfOfChat => "a message sent on behalf of a chat",
            Self::NoText => "a message with neither text nor caption",
            Self::NoSender => "a message without a sending person",
            Self::UnknownChatKind => "a chat type outside the vocabulary",
            Self::BadTimestamp => "a send time outside the representable range",
        };
        f.write_str(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Chat, Incoming, Update, User};

    /// One recordable private-chat update carrying the given sender.
    fn update_with_sender(user: User) -> Update {
        Update {
            id: 1,
            message: Some(Incoming {
                message_id: 10,
                date: 1_700_000_000,
                chat: Chat {
                    id: 5,
                    kind: "private".into(),
                },
                from: Some(user),
                sender_chat: None,
                text: Some("a message".into()),
                caption: None,
            }),
            edited_message: None,
        }
    }

    fn recorded(update: &Update) -> Pending {
        match translate(update) {
            Translation::Record(pending) => pending,
            Translation::Skip(reason) => {
                panic!("expected a recorded message, got a skip: {reason}")
            }
        }
    }

    /// The display name composes the first and last names, and a username
    /// the platform carries is kept on the identity.
    #[test]
    fn a_sender_with_a_last_name_and_a_username_translates_whole() {
        let pending = recorded(&update_with_sender(User {
            id: 5,
            first_name: "Ada".into(),
            last_name: Some("Lovelace".into()),
            username: Some("ada".into()),
        }));
        assert_eq!(pending.sender.display_name, "Ada Lovelace");
        assert_eq!(pending.sender.username.as_deref(), Some("ada"));
        assert_eq!(pending.sender.external_id, "5");
    }

    /// A bare first name stands alone — no separator artifact — and an
    /// absent username stays absent instead of becoming an empty one.
    #[test]
    fn a_sender_with_only_a_first_name_translates_bare() {
        let pending = recorded(&update_with_sender(User {
            id: 6,
            first_name: "Ada".into(),
            last_name: None,
            username: None,
        }));
        assert_eq!(pending.sender.display_name, "Ada");
        assert_eq!(pending.sender.username, None);
    }
}
