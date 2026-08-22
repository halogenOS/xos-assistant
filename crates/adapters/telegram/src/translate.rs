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
use crate::client::{BotIdentity, Incoming, Update};

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
    /// Whether the message addresses the assistant — platform knowledge,
    /// resolved here: a direct chat always does; a group message does when
    /// it mentions the bot's username or replies to one of the bot's own
    /// messages. The core receives only this neutral fact.
    pub addressed: bool,
    pub text: String,
    /// The platform's message id, the origin reference.
    pub origin: String,
    /// The platform's send time.
    pub sent_at: DateTime<Utc>,
}

/// Translate one update per the recorded decisions, resolving addressing
/// against the bot's own identity.
pub(crate) fn translate(update: &Update, me: &BotIdentity) -> Translation {
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
    let addressed = match channel_kind {
        ChannelKind::Direct => true,
        ChannelKind::Group => mentions_bot(text, me) || replies_to_bot(message, me),
    };
    Translation::Record(Pending {
        chat_id: message.chat.id,
        channel_kind,
        authority,
        addressed,
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

/// Whether the text mentions the bot by username. A mention is `@` followed
/// by exactly the bot's username as one whole handle token — the platform's
/// handle alphabet is ASCII letters, digits and the underscore — compared
/// case-insensitively, because the platform treats usernames so. A longer
/// handle that merely starts with the username is someone else's, and an
/// `@` inside a longer word — an address like `a@b.example` — starts no
/// handle at all, with one platform-defined exception: in a command aimed
/// at one bot, `/help@assistant_bot`, the run before the `@` is the
/// command's name and the handle after it is a mention.
fn mentions_bot(text: &str, me: &BotIdentity) -> bool {
    let Some(username) = me.username.as_deref() else {
        return false;
    };
    for (position, character) in text.char_indices() {
        if character != '@' || buried_in_word(text, position) {
            continue;
        }
        let handle_and_on = &text[position + 1..];
        let handle_end = handle_and_on
            .find(|c| !is_handle_char(c))
            .unwrap_or(handle_and_on.len());
        if handle_and_on[..handle_end].eq_ignore_ascii_case(username) {
            return true;
        }
    }
    false
}

/// The platform's handle alphabet: ASCII letters, digits and the underscore.
fn is_handle_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether the `@` at `position` sits inside a longer word — an address like
/// `a@b.example` — and so starts no handle. The run of handle characters
/// before the `@` is read whole: when a `/` opens that run at a word start,
/// the run is a command's name and the `@` does start a handle —
/// `/help@assistant_bot` is the platform's own way of aiming a command at
/// one bot, while `path/to@thing` stays a longer word.
fn buried_in_word(text: &str, position: usize) -> bool {
    let before = &text[..position];
    let Some(run_start) = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_handle_char(c))
        .last()
        .map(|(index, _)| index)
    else {
        return false;
    };
    let before_run = &before[..run_start];
    let command = before_run.ends_with('/')
        && before_run[..before_run.len() - 1]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace);
    !command
}

/// Whether the message replies to one of the bot's own messages.
fn replies_to_bot(message: &Incoming, me: &BotIdentity) -> bool {
    message
        .reply_to_message
        .as_ref()
        .and_then(|replied| replied.from.as_ref())
        .is_some_and(|author| author.id == me.id)
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
    use crate::client::{Chat, Incoming, RepliedTo, Update, User};

    /// The bot identity the tests resolve addressing against.
    fn bot() -> BotIdentity {
        BotIdentity {
            id: 4242,
            username: Some("helper_bot".into()),
        }
    }

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
                reply_to_message: None,
            }),
            edited_message: None,
        }
    }

    fn recorded(update: &Update) -> Pending {
        match translate(update, &bot()) {
            Translation::Record(pending) => pending,
            Translation::Skip(reason) => {
                panic!("expected a recorded message, got a skip: {reason}")
            }
        }
    }

    /// One recordable group update with the given text and replied-to
    /// author, for the addressing table below.
    fn group_update(text: &str, replied_author: Option<i64>) -> Update {
        Update {
            id: 2,
            message: Some(Incoming {
                message_id: 20,
                date: 1_700_000_000,
                chat: Chat {
                    id: -100,
                    kind: "supergroup".into(),
                },
                from: Some(User {
                    id: 7,
                    first_name: "Ada".into(),
                    last_name: None,
                    username: None,
                }),
                sender_chat: None,
                text: Some(text.into()),
                caption: None,
                reply_to_message: replied_author.map(|id| RepliedTo {
                    from: Some(User {
                        id,
                        first_name: "Bot".into(),
                        last_name: None,
                        username: None,
                    }),
                }),
            }),
            edited_message: None,
        }
    }

    /// A direct chat is always addressed: the whole conversation is with
    /// the assistant.
    #[test]
    fn a_private_message_is_addressed() {
        assert!(
            recorded(&update_with_sender(User {
                id: 5,
                first_name: "Ada".into(),
                last_name: None,
                username: None,
            }))
            .addressed
        );
    }

    /// The mention rule: the exact handle addresses, in any casing; a longer
    /// handle that merely starts with the username is someone else and does
    /// not; an unrelated group message does not.
    #[test]
    fn group_addressing_reads_the_mention_exactly() {
        assert!(recorded(&group_update("hey @helper_bot, ping?", None)).addressed);
        assert!(recorded(&group_update("hey @Helper_Bot!", None)).addressed);
        assert!(!recorded(&group_update("hey @helper_bot2, not you", None)).addressed);
        assert!(!recorded(&group_update("no handle at all", None)).addressed);
        assert!(!recorded(&group_update("mail me at a@helper_bot.example", None)).addressed);
    }

    /// The command form: `/command@handle` is the platform's way of aiming a
    /// command at one bot, so the handle after the command's name addresses
    /// — at the start, mid-message after whitespace, and in any casing —
    /// while another bot's command and a path-like word do not.
    #[test]
    fn group_addressing_reads_the_command_form() {
        assert!(recorded(&group_update("/help@helper_bot", None)).addressed);
        assert!(recorded(&group_update("try /start@Helper_Bot now", None)).addressed);
        assert!(!recorded(&group_update("/help@helper_bot2", None)).addressed);
        assert!(!recorded(&group_update("see path/to@helper_bot", None)).addressed);
    }

    /// The reply rule: replying to the bot's own message addresses it;
    /// replying to anyone else does not.
    #[test]
    fn group_addressing_reads_the_replied_author() {
        assert!(recorded(&group_update("thanks!", Some(4242))).addressed);
        assert!(!recorded(&group_update("thanks!", Some(9))).addressed);
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
