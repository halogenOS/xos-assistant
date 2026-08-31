//! The command catalogue: one list of the commands this assistant answers,
//! one recognition over it, and one reading of who is offered each command
//! in which kind of channel.
//!
//! Recognition was hand-matched per feature before this module existed, and
//! a fourth hand-written list was what the commands-menu design was written
//! to end. The catalogue is that design's core half, adopted here with the
//! session-reset commands (2026-08-30): [`Command`] with its [`Command::ALL`]
//! order, [`Command::invocation`], [`Command::offered`] and [`recognized`].
//! What a person reads in a published menu — the per-command summary and the
//! publication itself — stays with the commands-menu unit; nothing here
//! speaks about a menu.
//!
//! Two rules hold across the catalogue:
//!
//! - **Recognition folds ASCII case.** A hand-typed `/Privacy` is the same
//!   invocation as `/privacy`: every spelling here is lowercase ASCII, so no
//!   two of them can collide under the fold, and a person exercising a data
//!   right with an autocapitalising keyboard is answered. The fold lives in
//!   the core because which spellings the core accepts is the core's
//!   decision; the adapter's report keeps the typed form intact.
//! - **[`Command::offered`] decides the answer, not only who is told about
//!   the command.** A recognized command always takes the command stamp, so
//!   it opens no turn and takes no debt; whether it is ANSWERED is this one
//!   reading. An invocation where the command is not offered answers
//!   silence: no refusal line, because a refusal line advertises a surface
//!   the person cannot use.
//!
//! The moderation bot's own deletion command stays outside the catalogue
//! (see [`crate::mirror`]): it is not this assistant's command, and listing
//! it would present the assistant as the actor that deletes messages.
//!
//! # What the report carries
//!
//! Recognition matches the invocation token ALONE: the marker and the name,
//! with nothing after them. Normalizing an addressed form — a token
//! carrying the assistant's own handle after it — and dropping whatever a
//! person typed past the token are the adapter's work, done at its
//! boundary, where the platform's own spelling of both is known; a token
//! aimed at another recipient is reported as no command at all. So `/wipe`,
//! the same command addressed to the assistant, and `/wipe` followed by
//! anything at all all arrive here as one token, which is why this module
//! folds case and matches nothing else. A report carrying a suffix or an
//! argument would not be recognized, and that is the boundary holding
//! rather than a gap: the adapters' own suites pin the shape they deliver.

use crate::message::{Authority, ChannelKind, InvokedCommand};
use crate::privacy::{
    CONFIRM_COMMAND, DELETE_COMMAND, OPT_IN_COMMAND, OPT_OUT_COMMAND, PRIVACY_COMMAND,
};

/// The session wipe: the group's channel leaves its conversation behind and
/// starts a new one, exactly as a newly admitted group does.
pub const WIPE_COMMAND: &str = "/wipe";

/// The session compact: the first half of the group's conversation is
/// summarized and the rest of it comes along verbatim.
pub const COMPACT_COMMAND: &str = "/compact";

/// The least standing the two session resets accept. The deletion mirror
/// already trusts this same edge of the group's administrator set with
/// removing a member's stored message (decision 0015 resolves the group's
/// owner to admin and its administrators to moderator), and resetting a
/// context is the lighter act of the two.
pub const RESET_FLOOR: Authority = Authority::Moderator;

// ─── The fixed lines, exact copy per the unit spec (2026-08-30) ──────────

/// What `/wipe` answers when the reset stands. It states both halves of
/// what happened: the group speaks into an empty session from here, and the
/// old conversation is not gone anywhere.
pub const WIPE_DONE: &str = "Done. This group starts a fresh session; the old one stays on record.";

/// What `/compact` answers when the compaction stands. The operator chose
/// this copy (2026-08-31): the act is confirmed and nothing more — what
/// compaction does to the history is the operator contract's job to
/// explain, not this line's.
pub const COMPACT_DONE: &str = "Compaction finished";

/// What `/compact` answers when the session does not split: a ledger too
/// short to have two halves, or one whose whole history is a single message
/// group. Nothing is summarized and nothing moves.
pub const COMPACT_ALREADY: &str = "This session is already compact. Nothing changed.";

/// One command this assistant answers.
///
/// Adding a command means adding a variant; the compiler then asks for its
/// spelling and its audience, which is the whole point of the catalogue
/// being an enum instead of a list of strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `/privacy` — the policy pointer.
    Privacy,
    /// `/privacyout` — stop collecting and answering this person's
    /// messages.
    PrivacyOut,
    /// `/privacydelete` — ask for deletion, answered with the confirm
    /// instruction.
    PrivacyDelete,
    /// `/confirmdelete` — confirm the deletion just asked for.
    ConfirmDelete,
    /// `/unblockprivacy` — start collecting this person's messages again.
    PrivacyIn,
    /// `/wipe` — the group's session starts over, empty.
    Wipe,
    /// `/compact` — the group's session keeps its recent messages and
    /// carries a summary of the rest.
    Compact,
}

impl Command {
    /// Every command, in the order a person meets them: the policy pointer,
    /// then the rights in the order they are exercised, then the two
    /// session resets. Pinned by a test, so a reordering is a deliberate
    /// act.
    pub const ALL: [Self; 7] = [
        Self::Privacy,
        Self::PrivacyOut,
        Self::PrivacyDelete,
        Self::ConfirmDelete,
        Self::PrivacyIn,
        Self::Wipe,
        Self::Compact,
    ];

    /// The token that invokes this command, leading marker included —
    /// exactly the shape the adapter's invoked-command report carries.
    #[must_use]
    pub fn invocation(self) -> &'static str {
        match self {
            Self::Privacy => PRIVACY_COMMAND,
            Self::PrivacyOut => OPT_OUT_COMMAND,
            Self::PrivacyDelete => DELETE_COMMAND,
            Self::ConfirmDelete => CONFIRM_COMMAND,
            Self::PrivacyIn => OPT_IN_COMMAND,
            Self::Wipe => WIPE_COMMAND,
            Self::Compact => COMPACT_COMMAND,
        }
    }

    /// Whether a person of at least this standing is offered this command
    /// in this kind of channel.
    ///
    /// The authority is a FLOOR, not a point: the caller passes the lowest
    /// standing in the audience it is asking about, and the answer is
    /// monotone — a higher standing is offered everything a lower one is.
    ///
    /// The two rows are stated, not derived from one another:
    ///
    /// - The five privacy commands are offered to every member in both
    ///   kinds of channel. That is what they have always done, and a data
    ///   right must not depend on which room a person is standing in.
    /// - `/wipe` and `/compact` are offered in a group, at
    ///   [`RESET_FLOOR`] and above, and nowhere else. The direct-chat fence
    ///   lives in these two commands' arm and nowhere else — there is no
    ///   catalogue-wide fencing machinery, and the row above shows what
    ///   that means: a data right crosses into a direct chat untouched.
    ///   The fence is here because this deployment's direct chats are not a
    ///   place where a group's session can be reset, and there is no other
    ///   session to reset there.
    #[must_use]
    pub fn offered(self, channel: ChannelKind, authority: Authority) -> bool {
        match self {
            Self::Privacy
            | Self::PrivacyOut
            | Self::PrivacyDelete
            | Self::ConfirmDelete
            | Self::PrivacyIn => authority >= Authority::Member,
            Self::Wipe | Self::Compact => channel == ChannelKind::Group && authority >= RESET_FLOOR,
        }
    }
}

/// The command a message invokes, if any: the adapter's reported token
/// matched against the catalogue with ASCII case folded.
///
/// Invoking a command is addressing by form, so nothing here consults the
/// stored addressed fact and every command works unaddressed in a group.
/// A token the catalogue does not hold — another bot's, or one the platform
/// delivered that never existed anywhere — is no invocation here and the
/// message stays ordinary.
#[must_use]
pub fn recognized(command: Option<&InvokedCommand>) -> Option<Command> {
    let token = command?.name();
    Command::ALL
        .into_iter()
        .find(|command| token.eq_ignore_ascii_case(command.invocation()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant recognizes its own spelling, and recognizes it with
    /// ASCII case folded — the mixed-case form a phone's keyboard produces
    /// included.
    #[test]
    fn every_command_recognizes_its_own_spelling_with_case_folded() {
        for command in Command::ALL {
            let token = command.invocation();
            assert_eq!(
                recognized(Some(&InvokedCommand::new(token))),
                Some(command),
                "{token} recognizes itself"
            );
            assert_eq!(
                recognized(Some(&InvokedCommand::new(token.to_uppercase()))),
                Some(command),
                "{token} recognizes its uppercase spelling"
            );
            let mixed: String = token
                .char_indices()
                .map(|(index, character)| {
                    if index % 2 == 0 {
                        character
                    } else {
                        character.to_ascii_uppercase()
                    }
                })
                .collect();
            assert_eq!(
                recognized(Some(&InvokedCommand::new(mixed.clone()))),
                Some(command),
                "{mixed} recognizes its command"
            );
        }
    }

    /// What the catalogue refuses: the moderation bot's own token, a
    /// command nobody declared, and a message reporting no command at all.
    #[test]
    fn a_foreign_token_and_no_token_recognize_nothing() {
        for token in [crate::mirror::DELETION_COMMAND, "/Del", "/help", "/foo"] {
            assert_eq!(
                recognized(Some(&InvokedCommand::new(token))),
                None,
                "{token} is outside the catalogue"
            );
        }
        assert_eq!(recognized(None), None);
    }

    /// The spellings are what the catalogue promises: a leading marker and
    /// lowercase ASCII letters after it. Nothing here can collide under the
    /// case fold, which is what makes the fold safe.
    #[test]
    fn every_invocation_is_a_lowercase_ascii_token() {
        for command in Command::ALL {
            let token = command.invocation();
            let rest = token
                .strip_prefix('/')
                .unwrap_or_else(|| panic!("{token} opens with the command marker"));
            assert!(!rest.is_empty(), "{token} carries a name");
            assert!(
                rest.chars().all(|character| character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '_'),
                "{token} spells its name in lowercase ASCII"
            );
        }
        let mut folded: Vec<String> = Command::ALL
            .iter()
            .map(|command| command.invocation().to_ascii_lowercase())
            .collect();
        folded.sort();
        let unique = folded.len();
        folded.dedup();
        assert_eq!(unique, folded.len(), "no two spellings collide when folded");
    }

    /// The catalogue's order, pinned literally.
    #[test]
    fn the_catalogue_order_is_the_spec_order() {
        assert_eq!(
            Command::ALL,
            [
                Command::Privacy,
                Command::PrivacyOut,
                Command::PrivacyDelete,
                Command::ConfirmDelete,
                Command::PrivacyIn,
                Command::Wipe,
                Command::Compact,
            ]
        );
    }

    /// The audience rows, stated: the five privacy commands everywhere for
    /// everyone, the two resets in a group from the floor up.
    #[test]
    fn the_audience_rows_are_the_spec_rows() {
        for authority in Authority::ALL {
            for channel in ChannelKind::ALL {
                for command in [
                    Command::Privacy,
                    Command::PrivacyOut,
                    Command::PrivacyDelete,
                    Command::ConfirmDelete,
                    Command::PrivacyIn,
                ] {
                    assert!(
                        command.offered(channel, authority),
                        "{command:?} is offered to {authority:?} in {channel:?}"
                    );
                }
                for command in [Command::Wipe, Command::Compact] {
                    assert_eq!(
                        command.offered(channel, authority),
                        channel == ChannelKind::Group && authority >= RESET_FLOOR,
                        "{command:?} is offered in a group from the floor up and nowhere else"
                    );
                }
            }
        }
    }

    /// The floor reading is monotone in authority: whatever a lower
    /// standing is offered, every higher standing is offered too, in both
    /// kinds of channel. The catalogue's callers pass the lowest standing
    /// of the audience they ask about, so a non-monotone row would offer a
    /// command to a moderator that an administrator is refused.
    #[test]
    fn the_offered_set_grows_with_standing() {
        for channel in ChannelKind::ALL {
            let offered = |authority: Authority| -> Vec<Command> {
                Command::ALL
                    .into_iter()
                    .filter(|command| command.offered(channel, authority))
                    .collect()
            };
            for (lower, higher) in [
                (Authority::Member, Authority::Moderator),
                (Authority::Moderator, Authority::Admin),
            ] {
                let below = offered(lower);
                let above = offered(higher);
                for command in below {
                    assert!(
                        above.contains(&command),
                        "{channel:?}: {command:?} offered at {lower:?} is offered at {higher:?}"
                    );
                }
            }
        }
    }

    /// The three fixed lines, pinned verbatim against the unit spec's copy.
    #[test]
    fn the_fixed_lines_match_the_spec_copy_verbatim() {
        assert_eq!(
            WIPE_DONE,
            "Done. This group starts a fresh session; the old one stays on record."
        );
        assert_eq!(COMPACT_DONE, "Compaction finished");
        assert_eq!(
            COMPACT_ALREADY,
            "This session is already compact. Nothing changed."
        );
    }

    /// The two reset tokens and the floor, pinned to the spec's values.
    #[test]
    fn the_reset_tokens_and_the_floor_are_the_spec_values() {
        assert_eq!(WIPE_COMMAND, "/wipe");
        assert_eq!(COMPACT_COMMAND, "/compact");
        assert_eq!(RESET_FLOOR, Authority::Moderator);
    }
}
