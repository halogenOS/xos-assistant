//! The deletion mirror: the moderation bot's reply deletion command,
//! mirrored into the store (decided 2026-08-23).
//!
//! When a group administrator replies to a message with the moderation
//! bot's own deletion command, both bots receive that command
//! independently: the moderation bot deletes the message in the chat, and
//! the assistant erases its stored copy of the named message, every
//! recorded version of it — silently,
//! because the admin addressed the moderation bot, and a second bot
//! answering a command meant for the first is noise. The command message
//! itself is recorded like any command: the request is the lawful record.
//!
//! The trigger is deterministic from the message alone — the reported
//! invoked command is the deletion token, the channel is a group, the
//! reply names a stored message by its origin (whoever wrote it: nothing
//! checks the target's author, and an administrator deleting their own
//! message mirrors the same way; only the assistant's own messages fall
//! outside, their reply variant carrying no origin), and the sender's
//! resolved standing is in the administrator set. Store state never enters it: an
//! unknown or already-erased target leaves the erasure a no-op,
//! not the command unrecognized. This is a deterministic command, not a
//! model tool — the sender's resolved authority is the check, the
//! decision-0043 interrupt blocker stays untripped, and no model judgment
//! sits anywhere in the path: the administrator IS the human decision of
//! decision 0070.
//!
//! Recognising the command and acting on it are two readings (unit T3,
//! 2026-08-31), and both live here: [`recognized_deletion`] answers
//! "is this the other bot's deletion command", which the write's command
//! stamp reads, and [`mirrored_target`] answers "does the mirror erase",
//! which narrows it. The narrowing is that a message revising another one
//! mirrors nothing — nothing establishes that the moderation bot deletes
//! on an edited command, and a one-sided erasure of a message still
//! visible to the group is the divergence the mirror exists to prevent.

use crate::message::{Authority, ChannelKind, InboundMessage, ReplyTarget};

/// The moderation bot's own deletion command, exactly as its
/// administrators type it in a reply. The assistant piggybacks on this
/// token instead of owning a command: the admin asks the moderation bot,
/// and the assistant's part is bookkeeping. Recognition matches the
/// invoked command the adapter reports, never the stored text — the
/// command family's own rule.
pub const DELETION_COMMAND: &str = "/del";

/// The least standing the mirror accepts — the lower edge of the group's
/// administrator set. Decision 0015 translates the platform's member
/// statuses so the group's owner resolves to admin and its administrators
/// to moderator; the moderation bot obeys that whole set, so the mirror
/// does too: everything at or above this floor triggers it.
pub const ADMINISTRATOR_FLOOR: Authority = Authority::Moderator;

/// What the message NAMES as a deletion command's target: the replied-to
/// message's origin, exactly when the message is the moderation bot's
/// deletion command — the deletion token reported as the invoked command,
/// on a group channel, replying to a message with a stored origin (the
/// target's author is never checked: an administrator deleting their own
/// message mirrors the same way), from a sender at or above
/// [`ADMINISTRATOR_FLOOR`]. `None`
/// says the message is ordinary and names nothing: a member's deletion
/// command, one without a reply, one replying to the assistant's own
/// message — no origin rides that variant, and the assistant's own words
/// are no person's message row — and every direct-channel message alike.
///
/// This is RECOGNITION, and it is deliberately not the same question as
/// whether the mirror acts (unit T3, 2026-08-31). Recognition says "this
/// message is a command aimed at the other bot", which is what the write's
/// command stamp reads: such a message takes no debt, spends no budget
/// slot, opens no turn and draws no answer, whether or not anything is
/// erased for it. [`mirrored_target`] below decides the ACTION, and it is
/// narrower. Joining the two would make the stamp follow the erasure, so a
/// command the mirror declines to act on would become an ordinary summoned
/// message and, under helpful answering, spend a model turn on a command
/// meant for another bot.
///
/// The authority arrives as the entry point resolved it, not off the
/// message, so an unresolved standing was already refused before this is
/// asked.
#[must_use]
pub fn recognized_deletion(message: &InboundMessage, authority: Authority) -> Option<&str> {
    let command = message.command.as_ref()?;
    if message.channel_kind != ChannelKind::Group
        || authority < ADMINISTRATOR_FLOOR
        || command.name() != DELETION_COMMAND
    {
        return None;
    }
    match message.reply_target.as_ref()? {
        ReplyTarget::Message { origin } => Some(origin),
        // The assistant's own words are no person's row. Since unit 38 the
        // variant names which of her messages was replied to, and this
        // reading deliberately still mirrors nothing with it: the origin
        // is the quote's, and taking one of her own messages back is unit
        // T4's, whose build hooks this arm.
        ReplyTarget::AssistantMessage { origin: _ } => None,
    }
}

/// What the mirror ERASES: the recognized target, unless the message
/// revising another one carried it (unit T3, 2026-08-31). A revision
/// mirrors nothing.
///
/// The mirror's whole premise is that the moderation bot receives the same
/// command and deletes the message in the chat (decision 0082). Nothing
/// establishes that it acts on an edited command, and an assistant that
/// erased its stored copy of a message still visible to everyone would
/// produce precisely the divergence the mirror exists to prevent. The
/// recognition rides in from the caller instead of being asked again, so
/// the stamp and this gate read ONE recognition per write.
///
/// The privacy self-service commands are the opposite case and stay
/// reachable through an edit: only a message's own author can edit it, so
/// an edited rights command is that person's own ask about their own data.
/// That exemption lives at the ingestion's drops, not here — this module
/// decides one thing.
#[must_use]
pub fn mirrored_target<'a>(
    message: &InboundMessage,
    recognized: Option<&'a str>,
) -> Option<&'a str> {
    recognized.filter(|_| message.revises.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{ChannelKey, InvokedCommand, SenderIdentity};

    /// One group reply carrying the deletion command — the triggering
    /// shape the cases below vary away from.
    fn deletion_reply() -> InboundMessage {
        InboundMessage {
            channel: ChannelKey {
                adapter: "test".into(),
                channel: "room".into(),
            },
            channel_kind: ChannelKind::Group,
            sender: SenderIdentity {
                external_id: "admin-1".into(),
                username: None,
                bot: false,
            },
            authority: Some(Authority::Admin),
            addressed: false,
            reply_target: Some(ReplyTarget::Message {
                origin: "origin-7".into(),
            }),
            quoted: None,
            command: Some(InvokedCommand::new(DELETION_COMMAND)),
            text: DELETION_COMMAND.into(),
            origin: Some("del-1".into()),
            revises: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// The recognition and the action, as the ingestion reads them: one
    /// recognition, and the mirror's target derived from it.
    fn acted(message: &InboundMessage, authority: Authority) -> Option<&str> {
        mirrored_target(message, recognized_deletion(message, authority))
    }

    /// The trigger admits the whole administrator set of decision 0015 —
    /// moderator and admin — and names the replied-to origin.
    #[test]
    fn the_administrator_sets_reply_deletion_names_its_target() {
        let message = deletion_reply();
        for standing in [Authority::Moderator, Authority::Admin] {
            assert_eq!(
                acted(&message, standing),
                Some("origin-7"),
                "{standing:?} is in the administrator set"
            );
        }
    }

    /// A deletion command arriving as a revision is still RECOGNIZED — the
    /// stamp reads that and keeps the message silent and debt-free — while
    /// the mirror acts on nothing: the moderation bot's matching deletion
    /// is not established for an edited command, and a one-sided erasure
    /// would leave the store disagreeing with the chat.
    #[test]
    fn an_edited_deletion_command_is_recognized_and_mirrors_nothing() {
        let mut revised = deletion_reply();
        revised.revises = Some("del-1".into());
        assert_eq!(
            recognized_deletion(&revised, Authority::Admin),
            Some("origin-7"),
            "the recognition is unchanged: this is the other bot's command"
        );
        assert_eq!(
            acted(&revised, Authority::Admin),
            None,
            "a message revising another one mirrors nothing"
        );
    }

    /// Everything the mirror ignores, case by case: a member sender, a
    /// missing reply, a reply to the assistant's own message, a foreign
    /// command, no command at all, and a direct channel.
    #[test]
    fn every_non_triggering_shape_mirrors_nothing() {
        let message = deletion_reply();
        assert_eq!(
            acted(&message, Authority::Member),
            None,
            "a member's deletion command is ordinary"
        );

        let mut no_reply = deletion_reply();
        no_reply.reply_target = None;
        assert_eq!(
            acted(&no_reply, Authority::Admin),
            None,
            "a deletion command without a reply names nothing"
        );

        let mut to_assistant = deletion_reply();
        to_assistant.reply_target = Some(ReplyTarget::AssistantMessage {
            origin: Some("19".into()),
        });
        assert_eq!(
            acted(&to_assistant, Authority::Admin),
            None,
            "the assistant's own message is no person's row, named origin or not"
        );

        let mut other_command = deletion_reply();
        other_command.command = Some(InvokedCommand::new("/help"));
        assert_eq!(acted(&other_command, Authority::Admin), None);

        let mut no_command = deletion_reply();
        no_command.command = None;
        no_command.text = "just prose".into();
        assert_eq!(
            acted(&no_command, Authority::Admin),
            None,
            "the stored text is never matched, only the reported command"
        );

        let mut direct = deletion_reply();
        direct.channel_kind = ChannelKind::Direct;
        assert_eq!(
            acted(&direct, Authority::Admin),
            None,
            "the mirror belongs to groups, where the moderation bot is"
        );
    }

    /// The deletion token, pinned to the moderation bot's own spelling.
    #[test]
    fn the_deletion_token_is_the_moderation_bots_own() {
        assert_eq!(DELETION_COMMAND, "/del");
    }
}
