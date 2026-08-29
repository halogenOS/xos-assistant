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
    Authority, ChannelKey, ChannelKind, InvokedCommand, JoinedMember, Observation, ObservedFact,
    QuotedExcerpt, ReplyTarget, ReplyThread, SenderIdentity,
};

use crate::ADAPTER_NAME;
use crate::client::{
    BotIdentity, ChatInfo, Incoming, Joiner, MemberState, MemberUpdate, SendThread, Update,
};

/// What one update translates to.
pub(crate) enum Translation {
    /// The update is not recorded; the reason names the decision that says
    /// so. A skip is acknowledged past like a success.
    Skip(Skip),
    /// A message to record, pending authority resolution for groups.
    Record(Pending),
    /// A platform fact for the core's observation surface: a pin event's
    /// announcement text, the assistant's own entry into a group, or the
    /// people a join service note announces.
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
    /// A join service note outside a group; group facts belong to groups.
    JoinOutsideGroup,
    /// A join service note naming the assistant itself and nobody else:
    /// its own membership is the membership observation's territory, so
    /// the event holds nothing left to record.
    OwnEntryOnly,
    /// A membership service note that is not a join — a departure, the
    /// platform's one form for a member leaving and for a member removed,
    /// or a chat's creation. Named instead of dying at the generic no-text
    /// skip: they are not joins, and decision 0017 still governs them.
    MembershipServiceNote,
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
    /// it mentions the bot's username, replies to one of the bot's own
    /// messages, or names the assistant by its validated wake trigger
    /// (unit 14). The core receives only this neutral fact.
    pub addressed: bool,
    /// What the message replies to, translated beside the addressed flag
    /// (2026-08-23): the replied-to message's id as the opaque origin, or
    /// the fact that the reply points at one of the bot's own messages. A
    /// non-reply, and a reply the platform carries no usable id for,
    /// translates to no target.
    pub reply_target: Option<ReplyTarget>,
    /// The part of the replied-to message this reply quotes, where the
    /// platform reports one (unit 31, 2026-08-28): the excerpt's text and
    /// whether the sender selected it by hand. The excerpt's platform
    /// offset is not carried — the core searches its stored text for the
    /// excerpt instead of converting an offset between text encodings.
    pub quoted: Option<QuotedExcerpt>,
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
/// against the bot's own identity and the validated wake trigger, if one
/// stands ([`wake_trigger`]).
pub(crate) fn translate(update: &Update, me: &BotIdentity, wake: Option<&str>) -> Translation {
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
    // The join service note translates ahead of the on-behalf-of-chat and
    // no-sender skips, for the pin's reason: a join is the group's own
    // event, not a person's message, and it carries no sending person at
    // all. Every other membership service shape is named and skipped
    // beside it — they are not joins, and decision 0017 still governs them.
    if let Some(joined) = &message.new_chat_members {
        return translate_join(message, joined, channel_kind, me);
    }
    if message.left_chat_member.is_some()
        || message.group_chat_created
        || message.supergroup_chat_created
        || message.channel_chat_created
    {
        return Translation::Skip(Skip::MembershipServiceNote);
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
        ChannelKind::Group => {
            mentions_bot(text, me)
                || replies_to_bot(message, me)
                || wake.is_some_and(|trigger| names_bot(text, trigger))
        }
    };
    Translation::Record(Pending {
        chat_id: message.chat.id,
        channel_kind,
        authority,
        addressed,
        reply_target: reply_target_of(message, me),
        quoted: quoted_excerpt_of(message),
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

/// Translate one join service note (unit 36, 2026-08-29): the people it
/// named become the core's join fact, under the note's own message id as
/// the shared event origin and the note's send time.
///
/// The assistant's own entry drops here and nowhere else — its membership
/// is the membership observation's territory — and when one note names it
/// AND other people, only its own entry goes: the co-joiners translate and
/// the event stands. A join outside a group is a named skip, like the
/// pin's: group facts belong to groups.
fn translate_join(
    message: &Incoming,
    joined: &[Joiner],
    channel_kind: ChannelKind,
    me: &BotIdentity,
) -> Translation {
    if channel_kind != ChannelKind::Group {
        return Translation::Skip(Skip::JoinOutsideGroup);
    }
    let Some(sent_at) = DateTime::from_timestamp(message.date, 0) else {
        return Translation::Skip(Skip::BadTimestamp);
    };
    let joiners: Vec<JoinedMember> = joined
        .iter()
        .filter(|joiner| joiner.id != me.id)
        .map(joined_member)
        .collect();
    if joiners.is_empty() {
        return Translation::Skip(Skip::OwnEntryOnly);
    }
    Translation::Observe(Observation {
        channel: channel_key(message.chat.id),
        channel_kind: ChannelKind::Group,
        fact: ObservedFact::MembersJoined {
            joiners,
            origin: message.message_id.to_string(),
            timestamp: sent_at,
        },
    })
}

/// One joiner in the core's vocabulary: the identity every sender crosses
/// the boundary with, plus the name the platform displayed — the first
/// name, and the last name when one exists, space-joined, which is the
/// platform's own composition of what members saw beside the join. A
/// joiner the platform gave no first name for translates to the empty
/// name: nothing is invented, and the core's projection falls back to the
/// handle.
fn joined_member(joiner: &Joiner) -> JoinedMember {
    let name = match (joiner.first_name.as_deref(), joiner.last_name.as_deref()) {
        (Some(first), Some(last)) => format!("{first} {last}"),
        (Some(first), None) => first.to_owned(),
        (None, _) => String::new(),
    };
    JoinedMember {
        identity: SenderIdentity {
            external_id: joiner.id.to_string(),
            username: joiner.username.clone(),
        },
        name,
    }
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

/// The core's reply thread in the platform's terms: the origin decoded
/// through [`message_id_of`], carrying the recovery the core stated for a
/// refused threaded send. A reply the core threads nowhere, and an origin
/// this adapter did not mint, both send plain — the same outcome, since a
/// target the platform cannot be told about is no target.
pub(crate) fn send_thread(thread: Option<&ReplyThread>) -> SendThread {
    let Some(thread) = thread else {
        return SendThread::Plain;
    };
    let Some(message_id) = message_id_of(thread.target()) else {
        return SendThread::Plain;
    };
    if thread.plain_when_refused() {
        SendThread::OntoOrPlainly(message_id)
    } else {
        SendThread::OntoOnly(message_id)
    }
}

/// The configured name as a usable wake trigger, lowercased for the
/// case-insensitive match (unit 14): the trimmed name, accepted exactly
/// when every character is alphanumeric or an underscore — the shapes
/// whole-word matching can bound. A name outside that alphabet — spaces,
/// punctuation, an empty trim — yields no trigger; the caller logs the
/// fallback to mention-and-reply once at startup.
pub(crate) fn wake_trigger(name: &str) -> Option<String> {
    let trimmed = name.trim();
    let clean = !trimmed.is_empty() && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_');
    clean.then(|| trimmed.to_lowercase())
}

/// Whether the text names the assistant: the validated wake trigger as one
/// whole word, case-insensitively — a longer word merely containing the
/// name is someone or something else. The comparison runs over the
/// lowercased text against the already-lowercased trigger, so the word
/// boundaries are judged in one casing.
fn names_bot(text: &str, trigger: &str) -> bool {
    let lowered = text.to_lowercase();
    let mut from = 0;
    while let Some(position) = lowered[from..].find(trigger) {
        let start = from + position;
        let end = start + trigger.len();
        let bounded_left = lowered[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let bounded_right = lowered[end..]
            .chars()
            .next()
            .is_none_or(|c| !is_word_char(c));
        if bounded_left && bounded_right {
            return true;
        }
        from = end;
    }
    false
}

/// A word character for the name trigger's boundaries: alphanumeric in any
/// script, or the underscore — wider than the platform's handle alphabet
/// on purpose, because the name is the operator's word, not a handle.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
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

/// The quoted excerpt the message carries, in the core's vocabulary (unit
/// 31, 2026-08-28): the quoted text and the hand-selected flag, and
/// nothing else — the platform's UTF-16 offset beside them is not decoded
/// at all, because the core locates the excerpt by searching the text it
/// stored. A quoted part the platform delivered without text carries
/// nothing to search for, so it translates to no excerpt and the reply
/// quotes its target whole.
///
/// Translated for every reply, the assistant's own messages included: what
/// an excerpt is worth against a given target is the core's decision, and
/// the adapter's job is to report the platform's fact once.
fn quoted_excerpt_of(message: &Incoming) -> Option<QuotedExcerpt> {
    let quote = message.quote.as_ref()?;
    Some(QuotedExcerpt {
        text: quote.text.clone()?,
        manual: quote.is_manual,
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
            Self::JoinOutsideGroup => "a join service note outside a group",
            Self::OwnEntryOnly => "a join service note naming the assistant alone",
            Self::MembershipServiceNote => "a membership service note that is not a join",
        };
        f.write_str(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{Chat, Incoming, PinnedContent, QuotedPart, RepliedTo, Update, User};

    /// The bot identity the tests resolve addressing against.
    fn bot() -> BotIdentity {
        BotIdentity {
            id: 4242,
            username: Some("helper_bot".into()),
            first_name: Some("Fixture".into()),
        }
    }

    /// One incoming message with every field at its absent value — the
    /// base the fixtures below override, so a wire field added for a new
    /// service shape lands in one place instead of in every fixture.
    fn bare_incoming() -> Incoming {
        Incoming {
            message_id: 0,
            date: 1_700_000_000,
            chat: Chat {
                id: 0,
                kind: "supergroup".into(),
            },
            from: None,
            sender_chat: None,
            text: None,
            caption: None,
            reply_to_message: None,
            quote: None,
            pinned_message: None,
            new_chat_members: None,
            left_chat_member: None,
            group_chat_created: false,
            supergroup_chat_created: false,
            channel_chat_created: false,
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
                text: Some("a message".into()),
                ..bare_incoming()
            }),
            edited_message: None,
            my_chat_member: None,
        }
    }

    fn recorded(update: &Update) -> Pending {
        match translate(update, &bot(), None) {
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
                text: Some(text.into()),
                reply_to_message: replied_author.map(|id| RepliedTo {
                    from: Some(User { id, username: None }),
                    message_id: Some(19),
                }),
                ..bare_incoming()
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

    /// The recorded message under the given wake trigger, for the name
    /// rule's own table.
    fn recorded_waked(update: &Update, wake: Option<&str>) -> Pending {
        match translate(update, &bot(), wake) {
            Translation::Record(pending) => pending,
            Translation::Skip(reason) => {
                panic!("expected a recorded message, got a skip: {reason}")
            }
            Translation::Observe(_) => {
                panic!("expected a recorded message, got an observation")
            }
        }
    }

    /// The name rule (unit 14): the validated trigger addresses as one
    /// whole word in any casing, at any position and against punctuation
    /// boundaries; a longer word merely containing it does not, and
    /// without a trigger the same naming text rests on mention-and-reply.
    #[test]
    fn group_addressing_reads_the_name_as_a_whole_word() {
        let wake = wake_trigger("Xenia").expect("a clean name forms a trigger");
        let addressed =
            |text: &str| recorded_waked(&group_update(text, None), Some(&wake)).addressed;
        assert!(addressed("Xenia, which kernel does it run?"));
        assert!(addressed("does XENIA know this?"));
        assert!(addressed("ask xenia."));
        assert!(!addressed("the xenial release is old"));
        assert!(!addressed("praxenia is someone else"));
        assert!(!addressed("no name at all"));

        assert!(
            !recorded_waked(&group_update("Xenia, hello", None), None).addressed,
            "without a trigger the naming text rests"
        );
        assert!(
            recorded_waked(&group_update("hey @helper_bot", None), None).addressed,
            "the mention stands with or without a trigger"
        );
    }

    /// The trigger validation (unit 14): a trimmed alphanumeric-or-
    /// underscore name lowercases into the trigger, and a name outside
    /// that alphabet — spaces, punctuation, an empty trim — forms none:
    /// the fallback to mention-and-reply the driver logs.
    #[test]
    fn a_name_outside_the_trigger_alphabet_forms_no_trigger() {
        assert_eq!(wake_trigger("Xenia").as_deref(), Some("xenia"));
        assert_eq!(wake_trigger("  Xenia  ").as_deref(), Some("xenia"));
        assert_eq!(wake_trigger("helper_9").as_deref(), Some("helper_9"));
        assert_eq!(
            wake_trigger("XOS Assistant"),
            None,
            "a space bounds no one word"
        );
        assert_eq!(wake_trigger("X-9!"), None, "punctuation falls back");
        assert_eq!(wake_trigger("   "), None, "an empty trim falls back");
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

    /// The quoted excerpt's translation (unit 31, 2026-08-28): the quoted
    /// text and the hand-selected flag cross the boundary, the platform's
    /// offset does not exist to cross, a quoted part without text carries
    /// nothing to search for, and a message quoting nothing carries no
    /// excerpt at all. Translated whatever the reply points at — what an
    /// excerpt is worth against a given target is the core's decision.
    #[test]
    fn the_quoted_excerpt_translates_with_its_text_and_its_flag() {
        let quoting = |quote: Option<QuotedPart>| {
            let mut update = group_update("which one?", Some(9));
            update
                .message
                .as_mut()
                .expect("the fixture carries a message")
                .quote = quote;
            recorded(&update).quoted
        };

        assert_eq!(
            quoting(Some(QuotedPart {
                text: Some("the text font".into()),
                is_manual: true,
            })),
            Some(QuotedExcerpt {
                text: "the text font".into(),
                manual: true,
            }),
            "a hand-selected excerpt crosses with its words and its flag"
        );
        assert_eq!(
            quoting(Some(QuotedPart {
                text: Some("the text font".into()),
                is_manual: false,
            })),
            Some(QuotedExcerpt {
                text: "the text font".into(),
                manual: false,
            }),
            "a part the platform composed crosses as itself; the core \
             decides that it narrows nothing"
        );
        assert_eq!(
            quoting(Some(QuotedPart {
                text: None,
                is_manual: true,
            })),
            None,
            "a quoted part without text names no words to look for"
        );
        assert_eq!(quoting(None), None, "a plain reply quotes no part");
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
        match translate(update, &bot(), None) {
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
                    translate(&update, &bot(), None),
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
                        &bot(),
                        None,
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
                sender_chat: anonymous.then(|| Chat {
                    id: -300,
                    kind: chat_kind.into(),
                }),
                pinned_message: Some(pinned),
                ..bare_incoming()
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
                &bot(),
                None,
            ),
            Translation::Skip(Skip::InaccessiblePin)
        ));
        assert!(matches!(
            translate(
                &pin("group", pinned_text(None, None, 1_700_000_000), false),
                &bot(),
                None,
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
                &bot(),
                None,
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
    // ─── Join translation ────────────────────────────────────────────────

    /// One join service note in the given chat kind, naming the given
    /// joiners.
    fn join(chat_kind: &str, joiners: Vec<Joiner>) -> Update {
        Update {
            id: 5,
            message: Some(Incoming {
                message_id: 50,
                chat: Chat {
                    id: -400,
                    kind: chat_kind.into(),
                },
                new_chat_members: Some(joiners),
                ..bare_incoming()
            }),
            edited_message: None,
            my_chat_member: None,
        }
    }

    fn wire_joiner(
        id: i64,
        username: Option<&str>,
        first: Option<&str>,
        last: Option<&str>,
    ) -> Joiner {
        Joiner {
            id,
            username: username.map(Into::into),
            first_name: first.map(Into::into),
            last_name: last.map(Into::into),
        }
    }

    /// The joiners and the event a join note translates to: the name is the
    /// platform's own composition — first name, then last name where one
    /// exists — the identity is the id and the handle, and every joiner of
    /// the note carries the service message's own id as the shared origin.
    #[test]
    fn a_group_join_translates_to_the_joiners_the_platform_showed() {
        let fact = observed_fact(&join(
            "supergroup",
            vec![
                wire_joiner(11, Some("ada"), Some("Ada"), Some("Lovelace")),
                wire_joiner(12, None, Some("Grace"), None),
                wire_joiner(13, Some("bo"), None, None),
            ],
        ));
        let ObservedFact::MembersJoined {
            joiners,
            origin,
            timestamp,
        } = fact
        else {
            panic!("a join translates to the joined fact");
        };
        assert_eq!(origin, "50", "the service message's own id is the event");
        assert_eq!(timestamp.timestamp(), 1_700_000_000);
        assert_eq!(
            joiners,
            vec![
                JoinedMember {
                    identity: SenderIdentity {
                        external_id: "11".into(),
                        username: Some("ada".into()),
                    },
                    name: "Ada Lovelace".into(),
                },
                JoinedMember {
                    identity: SenderIdentity {
                        external_id: "12".into(),
                        username: None,
                    },
                    name: "Grace".into(),
                },
                JoinedMember {
                    identity: SenderIdentity {
                        external_id: "13".into(),
                        username: Some("bo".into()),
                    },
                    name: String::new(),
                },
            ],
            "a joiner without a first name carries the empty name, never an invented one"
        );
    }

    /// The assistant's own entry drops at translation and nowhere else: an
    /// event naming it alone is the named skip, and an event naming it
    /// beside other people keeps the co-joiners and the shared event.
    #[test]
    fn the_assistants_own_entry_drops_and_the_co_joiners_stand() {
        assert!(
            matches!(
                translate(
                    &join(
                        "supergroup",
                        vec![wire_joiner(4242, Some("helper_bot"), Some("Fixture"), None)]
                    ),
                    &bot(),
                    None,
                ),
                Translation::Skip(Skip::OwnEntryOnly)
            ),
            "her own entry alone is the membership observation's territory"
        );

        let fact = observed_fact(&join(
            "group",
            vec![
                wire_joiner(4242, Some("helper_bot"), Some("Fixture"), None),
                wire_joiner(14, Some("ada"), Some("Ada"), None),
            ],
        ));
        let ObservedFact::MembersJoined {
            joiners, origin, ..
        } = fact
        else {
            panic!("a mixed event translates to the joined fact");
        };
        assert_eq!(origin, "50", "the shared event stands");
        assert_eq!(
            joiners
                .iter()
                .map(|joiner| joiner.identity.external_id.as_str())
                .collect::<Vec<_>>(),
            vec!["14"],
            "only her own entry is dropped"
        );
    }

    /// A join outside a group observes nothing: group facts belong to
    /// groups.
    #[test]
    fn a_join_outside_a_group_is_a_named_skip() {
        assert!(matches!(
            translate(
                &join("private", vec![wire_joiner(15, None, Some("Ada"), None)]),
                &bot(),
                None,
            ),
            Translation::Skip(Skip::JoinOutsideGroup)
        ));
    }

    /// Every other membership service shape is one named skip — a
    /// departure (the platform's one form for a leave and a removal) and a
    /// chat's creation — instead of dying at the generic no-text arm: they
    /// are not joins, and decision 0017 still governs them.
    #[test]
    fn the_other_membership_service_shapes_are_one_named_skip() {
        let departure = Update {
            id: 6,
            message: Some(Incoming {
                message_id: 60,
                left_chat_member: Some(serde::de::IgnoredAny),
                ..bare_incoming()
            }),
            edited_message: None,
            my_chat_member: None,
        };
        assert!(matches!(
            translate(&departure, &bot(), None),
            Translation::Skip(Skip::MembershipServiceNote)
        ));

        for created in [
            Incoming {
                group_chat_created: true,
                ..bare_incoming()
            },
            Incoming {
                supergroup_chat_created: true,
                ..bare_incoming()
            },
            Incoming {
                channel_chat_created: true,
                ..bare_incoming()
            },
        ] {
            let update = Update {
                id: 7,
                message: Some(created),
                edited_message: None,
                my_chat_member: None,
            };
            assert!(matches!(
                translate(&update, &bot(), None),
                Translation::Skip(Skip::MembershipServiceNote)
            ));
        }
    }
}
