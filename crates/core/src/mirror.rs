//! The deletion mirror: the moderation bot's reply deletion command,
//! mirrored into the store (decided 2026-08-23).
//!
//! When a group administrator replies to a message with the moderation
//! bot's own deletion command, both bots receive that command
//! independently: the moderation bot deletes the message in the chat, and
//! the assistant erases its stored copy of the named row — silently,
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
//! unknown or already-erased target leaves the one-row erasure a no-op,
//! not the command unrecognized. This is a deterministic command, not a
//! model tool — the sender's resolved authority is the check, the
//! decision-0043 interrupt blocker stays untripped, and no model judgment
//! sits anywhere in the path: the administrator IS the human decision of
//! decision 0070.

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

/// The mirror's target: the replied-to message's origin, exactly when the
/// message triggers the mirror — the deletion token reported as the
/// invoked command, on a group channel, replying to a message with a
/// stored origin (the target's author is never checked: an administrator
/// deleting their own message mirrors the same way), from a sender at or
/// above [`ADMINISTRATOR_FLOOR`]. `None`
/// says the message is ordinary and mirrors nothing: a member's deletion
/// command, one without a reply, one replying to the assistant's own
/// message — no origin rides that variant, and the assistant's own words
/// are no person's message row — and every direct-channel message alike.
///
/// The authority arrives as the entry point resolved it, not off the
/// message, so an unresolved standing was already refused before this is
/// asked.
#[must_use]
pub fn mirrored_target(message: &InboundMessage, authority: Authority) -> Option<&str> {
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
            timestamp: chrono::Utc::now(),
        }
    }

    /// The trigger admits the whole administrator set of decision 0015 —
    /// moderator and admin — and names the replied-to origin.
    #[test]
    fn the_administrator_sets_reply_deletion_names_its_target() {
        let message = deletion_reply();
        for standing in [Authority::Moderator, Authority::Admin] {
            assert_eq!(
                mirrored_target(&message, standing),
                Some("origin-7"),
                "{standing:?} is in the administrator set"
            );
        }
    }

    /// Everything the mirror ignores, case by case: a member sender, a
    /// missing reply, a reply to the assistant's own message, a foreign
    /// command, no command at all, and a direct channel.
    #[test]
    fn every_non_triggering_shape_mirrors_nothing() {
        let message = deletion_reply();
        assert_eq!(
            mirrored_target(&message, Authority::Member),
            None,
            "a member's deletion command is ordinary"
        );

        let mut no_reply = deletion_reply();
        no_reply.reply_target = None;
        assert_eq!(
            mirrored_target(&no_reply, Authority::Admin),
            None,
            "a deletion command without a reply names nothing"
        );

        let mut to_assistant = deletion_reply();
        to_assistant.reply_target = Some(ReplyTarget::AssistantMessage {
            origin: Some("19".into()),
        });
        assert_eq!(
            mirrored_target(&to_assistant, Authority::Admin),
            None,
            "the assistant's own message is no person's row, named origin or not"
        );

        let mut other_command = deletion_reply();
        other_command.command = Some(InvokedCommand::new("/help"));
        assert_eq!(mirrored_target(&other_command, Authority::Admin), None);

        let mut no_command = deletion_reply();
        no_command.command = None;
        no_command.text = "just prose".into();
        assert_eq!(
            mirrored_target(&no_command, Authority::Admin),
            None,
            "the stored text is never matched, only the reported command"
        );

        let mut direct = deletion_reply();
        direct.channel_kind = ChannelKind::Direct;
        assert_eq!(
            mirrored_target(&direct, Authority::Admin),
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
