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

use assistant_core::{
    Authority, ChannelKey, ChannelKind, InvokedCommand, Observation, ObservedFact, ReplyTarget,
    SenderIdentity,
};

use crate::ADAPTER_NAME;
use crate::client::{BotIdentity, ChatInfo, Incoming, MemberState, MemberUpdate, Update};

/// What one update translates to.
pub(crate) enum Translation {
    /// The update is not recorded; the reason names the decision that says
    /// so. A skip is acknowledged past like a success.
    Skip(Skip),
    /// A message to record, pending authority resolution for groups.
    Record(Pending),
    /// A platform fact for the core's observation surface: a pin event's
    /// announcement text, or the assistant's own entry into a group.
    Observe(Observation),
}

/// Every reason an update is skipped, each one a recorded decision or a
/// well-formedness check, so a log line names the case instead of a shrug.
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
    /// A pin whose content the platform withholds — the inaccessible form,
    /// discriminated by its zero date. It yields no observation.
    InaccessiblePin,
    /// A pinned message with neither text nor caption; the rules contract
    /// reads text, so there is nothing to report.
    TextlessPin,
    /// A pin service note outside a group; group facts belong to groups.
    PinOutsideGroup,
    /// A membership change outside a group — the platform fires the same
    /// update for private blocks and unblocks, which are nobody's
    /// invitation.
    MembershipOutsideGroup,
    /// A membership change that is not the assistant entering the group —
    /// judged by membership, never by a literal status pair.
    MembershipNotAnEntry,
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
    /// What the message replies to, translated beside the addressed flag
    /// (2026-08-23): the replied-to message's id as the opaque origin, or
    /// the fact that the reply points at one of the bot's own messages. A
    /// non-reply, and a reply the platform carries no usable id for,
    /// translates to no target.
    pub reply_target: Option<ReplyTarget>,
    /// The command the message invokes, reported beside the text: the
    /// leading command token, a self-directed handle suffix normalized
    /// away. The text itself is never rewritten.
    pub command: Option<InvokedCommand>,
    /// What was said, verbatim — the ledger records what the person typed
    /// (refined 2026-08-23).
    pub text: String,
    /// The platform's message id, the origin reference.
    pub origin: String,
    /// The platform's send time.
    pub sent_at: DateTime<Utc>,
}

/// Translate one update per the recorded decisions, resolving addressing
/// against the bot's own identity.
pub(crate) fn translate(update: &Update, me: &BotIdentity) -> Translation {
    if let Some(member) = &update.my_chat_member {
        return translate_membership(member);
    }
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
    // The pin service note translates ahead of the on-behalf-of-chat skip:
    // an anonymous-admin pin arrives exactly there, and the pin is the
    // group's own fact, not a person's message.
    if let Some(pinned) = &message.pinned_message {
        if channel_kind != ChannelKind::Group {
            return Translation::Skip(Skip::PinOutsideGroup);
        }
        if pinned.date == 0 {
            return Translation::Skip(Skip::InaccessiblePin);
        }
        let text = pinned
            .text
            .as_deref()
            .or(pinned.caption.as_deref())
            .filter(|text| !text.is_empty());
        let Some(text) = text else {
            return Translation::Skip(Skip::TextlessPin);
        };
        return Translation::Observe(Observation {
            channel: channel_key(message.chat.id),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::PinnedAnnouncement(text.to_owned()),
        });
    }
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
        reply_target: reply_target_of(message, me),
        command: invoked_command(text, me),
        sender: SenderIdentity {
            external_id: from.id.to_string(),
            username: from.username.clone(),
        },
        sender_id: from.id,
        text: text.to_owned(),
        origin: message.message_id.to_string(),
        sent_at,
    })
}

/// Translate one membership update about the assistant itself: an entry
/// into a group — from outside the member set to inside it, in any member
/// shape the platform grants — becomes a membership observation carrying
/// the acting principal. Everything else is a named skip: the platform
/// fires the same update for private blocks and unblocks, and a departure
/// is not an invitation.
fn translate_membership(member: &MemberUpdate) -> Translation {
    match member.chat.kind.as_str() {
        "group" | "supergroup" => {}
        _ => return Translation::Skip(Skip::MembershipOutsideGroup),
    }
    let was_in = member.old_chat_member.as_ref().is_some_and(is_in_chat);
    let is_in = member.new_chat_member.as_ref().is_some_and(is_in_chat);
    if was_in || !is_in {
        return Translation::Skip(Skip::MembershipNotAnEntry);
    }
    Translation::Observe(Observation {
        channel: channel_key(member.chat.id),
        channel_kind: ChannelKind::Group,
        fact: ObservedFact::Added {
            by: member.from.as_ref().map(|from| SenderIdentity {
                external_id: from.id.to_string(),
                username: from.username.clone(),
            }),
        },
    })
}

/// Whether one member state is inside the chat — membership, never a
/// literal status pair: member, administrator and creator are in; the
/// restricted form is in exactly when its own member flag says so; left
/// and kicked — and any status outside the vocabulary — are out.
fn is_in_chat(state: &MemberState) -> bool {
    match state.status.as_str() {
        "member" | "administrator" | "creator" => true,
        "restricted" => state.is_member == Some(true),
        _ => false,
    }
}

/// What one first-contact lookup reports. A pin event outranks the
/// lookup's pin (refined 2026-08-23): when the lookup runs because a pin
/// event arrived, the event carries the authoritative text, and the
/// lookup's by-sending-date pin would append stale rules and spend the
/// acknowledgment on them — so that lookup reports the title only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LookupScope {
    /// The title and the exposed pinned announcement.
    Whole,
    /// The title only; the caller's pin event carries the pinned text.
    TitleOnly,
}

/// The observations one first-contact lookup yields: the chat's title, and
/// — within [`LookupScope::Whole`] — the exposed pinned announcement where
/// it is accessible and carries text. Pure translation of the lookup's
/// answer; what either fact means is the core's contract.
pub(crate) fn lookup_observations(
    chat_id: i64,
    info: &ChatInfo,
    scope: LookupScope,
) -> Vec<Observation> {
    let mut observations = Vec::new();
    if let Some(title) = info.title.as_deref().filter(|title| !title.is_empty()) {
        observations.push(Observation {
            channel: channel_key(chat_id),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::Title(title.to_owned()),
        });
    }
    if scope == LookupScope::Whole
        && let Some(pinned) = &info.pinned_message
        && pinned.date != 0
        && let Some(text) = pinned
            .text
            .as_deref()
            .or(pinned.caption.as_deref())
            .filter(|text| !text.is_empty())
    {
        observations.push(Observation {
            channel: channel_key(chat_id),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::PinnedAnnouncement(text.to_owned()),
        });
    }
    observations
}

/// The command a message invokes, read from a leading command token: the
/// token as the invocation, with exactly the assistant's own handle suffix
/// normalized away — case-insensitively on the handle, because the
/// platform treats usernames so. A foreign handle reports nothing: that
/// command was aimed at someone else. Only the first token is read, because
/// the platform's command-at-a-bot form lives there. The text itself is
/// NEVER rewritten (refined 2026-08-23): the ledger records what the
/// person typed, and the core matches this report, not the text.
fn invoked_command(text: &str, me: &BotIdentity) -> Option<InvokedCommand> {
    if !text.starts_with('/') {
        return None;
    }
    let token_end = text.find(char::is_whitespace).unwrap_or(text.len());
    let token = &text[..token_end];
    let Some(at) = token.find('@') else {
        return Some(InvokedCommand::new(token));
    };
    let username = me.username.as_deref()?;
    let handle = &token[at + 1..];
    handle
        .eq_ignore_ascii_case(username)
        .then(|| InvokedCommand::new(&token[..at]))
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

/// The message a stored reply target names — the inverse of the decimal
/// spelling [`reply_target_of`] stored the id under, so both directions of
/// the naming rule live beside each other. `None` names a target this
/// adapter did not mint; the send goes out unthreaded.
pub(crate) fn message_id_of(origin: &str) -> Option<i64> {
    origin.parse().ok()
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

/// The reply target the message carries, in the core's vocabulary
/// (2026-08-23): a reply to one of the bot's own messages is the
/// assistant-message fact — no origin rides it — and every other reply
/// with a usable id contributes that id as the opaque origin, in the same
/// decimal spelling [`Pending::origin`] uses for the message's own id. A
/// reply without a usable id stores no target.
fn reply_target_of(message: &Incoming, me: &BotIdentity) -> Option<ReplyTarget> {
    let replied = message.reply_to_message.as_ref()?;
    if replies_to_bot(message, me) {
        return Some(ReplyTarget::AssistantMessage);
    }
    replied.message_id.map(|id| ReplyTarget::Message {
        origin: id.to_string(),
    })
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
            Self::InaccessiblePin => "a pin whose content the platform withholds",
            Self::TextlessPin => "a pinned message with neither text nor caption",
            Self::PinOutsideGroup => "a pin service note outside a group",
            Self::MembershipOutsideGroup => "a membership change outside a group",
            Self::MembershipNotAnEntry => {
                "a membership change that is not the assistant entering the group"
            }
        };
        f.write_str(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Chat, Incoming, PinnedContent, RepliedTo, Update, User};

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
                pinned_message: None,
            }),
            edited_message: None,
            my_chat_member: None,
        }
    }

    fn recorded(update: &Update) -> Pending {
        match translate(update, &bot()) {
            Translation::Record(pending) => pending,
            Translation::Skip(reason) => {
                panic!("expected a recorded message, got a skip: {reason}")
            }
            Translation::Observe(_) => {
                panic!("expected a recorded message, got an observation")
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
                    username: None,
                }),
                sender_chat: None,
                text: Some(text.into()),
                caption: None,
                reply_to_message: replied_author.map(|id| RepliedTo {
                    from: Some(User { id, username: None }),
                    message_id: Some(19),
                }),
                pinned_message: None,
            }),
            edited_message: None,
            my_chat_member: None,
        }
    }

    /// A direct chat is always addressed: the whole conversation is with
    /// the assistant.
    #[test]
    fn a_private_message_is_addressed() {
        assert!(
            recorded(&update_with_sender(User {
                id: 5,
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

    /// The reply-target translation (2026-08-23): a reply to another
    /// person's message carries that message's id as the opaque origin, a
    /// reply to the bot's own message carries the assistant-message fact,
    /// a reply without a usable id stores no target, and a non-reply
    /// stores none either.
    #[test]
    fn the_reply_target_translates_beside_the_addressed_flag() {
        assert_eq!(
            recorded(&group_update("report this", Some(9))).reply_target,
            Some(ReplyTarget::Message {
                origin: "19".into()
            }),
            "a reply to a member carries the replied-to id as the origin"
        );
        assert_eq!(
            recorded(&group_update("thanks!", Some(4242))).reply_target,
            Some(ReplyTarget::AssistantMessage),
            "a reply to the bot's own message is the assistant-message fact"
        );
        assert_eq!(
            recorded(&group_update("no reply here", None)).reply_target,
            None,
            "a non-reply stores no target"
        );

        let mut idless = group_update("report this", Some(9));
        idless
            .message
            .as_mut()
            .expect("the fixture carries a message")
            .reply_to_message
            .as_mut()
            .expect("the fixture carries a reply")
            .message_id = None;
        assert_eq!(
            recorded(&idless).reply_target,
            None,
            "a reply without a usable id stores no target"
        );
    }

    /// The identity crossing the boundary is the external id and the
    /// username, nothing more (decision 0077): the platform's name fields
    /// are not translated at all, so a display name cannot even reach the
    /// core to be discarded there.
    #[test]
    fn a_sender_translates_to_the_external_id_and_the_username_alone() {
        let pending = recorded(&update_with_sender(User {
            id: 5,
            username: Some("ada".into()),
        }));
        assert_eq!(
            pending.sender,
            SenderIdentity {
                external_id: "5".into(),
                username: Some("ada".into()),
            },
            "the whole identity is the id and the handle — no name fields"
        );
    }

    /// An absent username stays absent instead of becoming an empty one.
    #[test]
    fn a_sender_without_a_username_translates_bare() {
        let pending = recorded(&update_with_sender(User {
            id: 6,
            username: None,
        }));
        assert_eq!(pending.sender.username, None);
        assert_eq!(pending.sender.external_id, "6");
    }

    // ─── Membership translation ──────────────────────────────────────────

    /// One membership update: the assistant moved between the given member
    /// states in a chat of the given kind, by actor 77.
    fn membership(chat_kind: &str, old: Option<MemberState>, new: Option<MemberState>) -> Update {
        Update {
            id: 3,
            message: None,
            edited_message: None,
            my_chat_member: Some(MemberUpdate {
                chat: Chat {
                    id: -200,
                    kind: chat_kind.into(),
                },
                from: Some(User {
                    id: 77,
                    username: None,
                }),
                old_chat_member: old,
                new_chat_member: new,
            }),
        }
    }

    fn state(status: &str, is_member: Option<bool>) -> MemberState {
        MemberState {
            status: status.into(),
            is_member,
        }
    }

    fn observed_fact(update: &Update) -> ObservedFact {
        match translate(update, &bot()) {
            Translation::Observe(observation) => {
                assert_eq!(observation.channel_kind, ChannelKind::Group);
                observation.fact
            }
            Translation::Skip(reason) => panic!("expected an observation, got a skip: {reason}"),
            Translation::Record(_) => panic!("expected an observation, got a recorded message"),
        }
    }

    /// An entry is judged by membership, in every member shape the platform
    /// grants: member, administrator, and restricted-but-in.
    #[test]
    fn every_member_shape_of_an_entry_translates_to_the_added_observation() {
        for new in [
            state("member", None),
            state("administrator", None),
            state("restricted", Some(true)),
        ] {
            let fact = observed_fact(&membership(
                "supergroup",
                Some(state("left", None)),
                Some(new),
            ));
            let ObservedFact::Added { by } = fact else {
                panic!("an entry translates to the membership fact");
            };
            assert_eq!(
                by.expect("the acting principal is carried").external_id,
                "77"
            );
        }
    }

    /// Transitions that are not the assistant entering — staying in,
    /// leaving, or arriving restricted-but-out — are named skips, never a
    /// status-pair match.
    #[test]
    fn a_non_entry_membership_transition_is_skipped() {
        let cases = [
            membership(
                "group",
                Some(state("member", None)),
                Some(state("administrator", None)),
            ),
            membership(
                "group",
                Some(state("member", None)),
                Some(state("left", None)),
            ),
            membership(
                "group",
                Some(state("kicked", None)),
                Some(state("restricted", Some(false))),
            ),
            membership("group", None, None),
        ];
        for update in cases {
            assert!(
                matches!(
                    translate(&update, &bot()),
                    Translation::Skip(Skip::MembershipNotAnEntry)
                ),
                "a non-entry transition is the named skip"
            );
        }
    }

    /// A private-chat membership shape — the platform's block and unblock —
    /// produces no observation, and neither does a broadcast channel's.
    #[test]
    fn a_private_membership_shape_produces_no_observation() {
        for kind in ["private", "channel"] {
            assert!(
                matches!(
                    translate(
                        &membership(kind, Some(state("left", None)), Some(state("member", None))),
                        &bot()
                    ),
                    Translation::Skip(Skip::MembershipOutsideGroup)
                ),
                "a {kind} membership change observes nothing"
            );
        }
    }

    // ─── Pin translation ─────────────────────────────────────────────────

    /// One pin service message in the given chat kind, with the given
    /// pinned payload; sent on behalf of the chat when `anonymous`.
    fn pin(chat_kind: &str, pinned: PinnedContent, anonymous: bool) -> Update {
        Update {
            id: 4,
            message: Some(Incoming {
                message_id: 40,
                date: 1_700_000_000,
                chat: Chat {
                    id: -300,
                    kind: chat_kind.into(),
                },
                from: None,
                sender_chat: anonymous.then(|| Chat {
                    id: -300,
                    kind: chat_kind.into(),
                }),
                text: None,
                caption: None,
                reply_to_message: None,
                pinned_message: Some(pinned),
            }),
            edited_message: None,
            my_chat_member: None,
        }
    }

    fn pinned_text(text: Option<&str>, caption: Option<&str>, date: i64) -> PinnedContent {
        PinnedContent {
            date,
            text: text.map(Into::into),
            caption: caption.map(Into::into),
        }
    }

    /// A group pin translates to the announcement observation — and an
    /// anonymous-admin pin does too, because pin handling precedes the
    /// on-behalf-of-chat skip, which is exactly where such a pin arrives.
    #[test]
    fn a_pin_translates_ahead_of_the_on_behalf_of_chat_skip() {
        for anonymous in [false, true] {
            let fact = observed_fact(&pin(
                "group",
                pinned_text(Some("Rules:\nBe kind."), None, 1_700_000_000),
                anonymous,
            ));
            let ObservedFact::PinnedAnnouncement(text) = fact else {
                panic!("a pin translates to the announcement fact");
            };
            assert_eq!(text, "Rules:\nBe kind.");
        }
    }

    /// The withheld and empty pin forms are named skips: the inaccessible
    /// form's zero-date discriminator, a pin with neither text nor caption,
    /// and a pin outside a group.
    #[test]
    fn the_inaccessible_textless_and_direct_pin_forms_yield_no_observation() {
        assert!(matches!(
            translate(
                &pin("group", pinned_text(Some("hidden"), None, 0), false),
                &bot()
            ),
            Translation::Skip(Skip::InaccessiblePin)
        ));
        assert!(matches!(
            translate(
                &pin("group", pinned_text(None, None, 1_700_000_000), false),
                &bot()
            ),
            Translation::Skip(Skip::TextlessPin)
        ));
        assert!(matches!(
            translate(
                &pin(
                    "private",
                    pinned_text(Some("a note"), None, 1_700_000_000),
                    false
                ),
                &bot()
            ),
            Translation::Skip(Skip::PinOutsideGroup)
        ));
    }

    /// A pinned media message's caption is the fallback text, mirroring the
    /// message rule.
    #[test]
    fn a_pinned_caption_is_the_fallback_text() {
        let fact = observed_fact(&pin(
            "group",
            pinned_text(None, Some("Rules:\nFrom a caption."), 1_700_000_000),
            false,
        ));
        assert!(
            matches!(fact, ObservedFact::PinnedAnnouncement(text) if text == "Rules:\nFrom a caption.")
        );
    }

    // ─── The invoked-command report ──────────────────────────────────────

    /// One recordable group update carrying exactly this text.
    fn group_text_update(text: &str) -> Update {
        group_update(text, None)
    }

    /// The text lands verbatim in every form, and the invocation travels
    /// beside it: a bare leading command reports itself, the assistant's
    /// own handle suffix is normalized out of the report case-insensitively,
    /// and a foreign handle, a mid-text command or a non-command text
    /// report nothing.
    #[test]
    fn the_text_stays_verbatim_and_the_invoked_command_is_reported_beside_it() {
        let own = recorded(&group_text_update("/privacy@helper_bot"));
        assert_eq!(
            own.text, "/privacy@helper_bot",
            "the text is never rewritten"
        );
        assert_eq!(own.command, Some(InvokedCommand::new("/privacy")));

        let cased = recorded(&group_text_update("/privacy@Helper_Bot please"));
        assert_eq!(cased.text, "/privacy@Helper_Bot please");
        assert_eq!(cased.command, Some(InvokedCommand::new("/privacy")));

        let bare = recorded(&group_text_update("/privacy"));
        assert_eq!(bare.command, Some(InvokedCommand::new("/privacy")));

        assert_eq!(
            recorded(&group_text_update("/privacy@helper_bot2")).command,
            None,
            "a foreign handle reports no command"
        );
        assert_eq!(
            recorded(&group_text_update("see /privacy@helper_bot")).command,
            None,
            "only a leading command token is read"
        );
        assert_eq!(
            recorded(&group_text_update("mail a@helper_bot.example")).command,
            None,
            "a non-command text reports nothing"
        );
    }

    /// The own-handle command form stays addressed: addressing is resolved
    /// on the text as sent, where that form is a mention.
    #[test]
    fn the_own_handle_command_form_keeps_its_addressing() {
        let pending = recorded(&group_text_update("/privacy@helper_bot"));
        assert!(pending.addressed, "the command form addressed the bot");
        assert_eq!(pending.command, Some(InvokedCommand::new("/privacy")));
    }

    // ─── The first-contact lookup's observations ─────────────────────────

    /// The lookup yields the title and the accessible pinned text, and
    /// withholds the inaccessible or textless pin while keeping the title.
    #[test]
    fn the_lookup_yields_title_and_accessible_pin_observations() {
        let full = lookup_observations(
            -300,
            &ChatInfo {
                title: Some("The kernel room".into()),
                pinned_message: Some(pinned_text(Some("Rules:\nBe kind."), None, 1_700_000_000)),
            },
            LookupScope::Whole,
        );
        assert_eq!(full.len(), 2);
        assert!(matches!(&full[0].fact, ObservedFact::Title(t) if t == "The kernel room"));
        assert!(
            matches!(&full[1].fact, ObservedFact::PinnedAnnouncement(t) if t == "Rules:\nBe kind.")
        );
        assert_eq!(full[0].channel.channel, "-300");

        let withheld = lookup_observations(
            -300,
            &ChatInfo {
                title: Some("The kernel room".into()),
                pinned_message: Some(pinned_text(Some("hidden"), None, 0)),
            },
            LookupScope::Whole,
        );
        assert_eq!(withheld.len(), 1, "the inaccessible pin yields nothing");
        assert!(matches!(&withheld[0].fact, ObservedFact::Title(_)));
    }

    /// The title-only scope withholds even an accessible pinned text: the
    /// pin event that triggered the lookup carries the authoritative pin.
    #[test]
    fn the_title_only_scope_reports_the_title_and_never_the_lookups_pin() {
        let title_only = lookup_observations(
            -300,
            &ChatInfo {
                title: Some("The kernel room".into()),
                pinned_message: Some(pinned_text(Some("Rules:\nThe stale rules."), None, 100)),
            },
            LookupScope::TitleOnly,
        );
        assert_eq!(title_only.len(), 1);
        assert!(matches!(&title_only[0].fact, ObservedFact::Title(_)));
    }
}
