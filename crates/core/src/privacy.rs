//! The privacy command: a deterministic answer to the one message form the
//! published terms require to be easy to reach (decided 2026-08-23). The
//! command is recognized here, answered from the configured address, and
//! stamped by the entry point as taking no debt — no turn, no answer-window
//! count, no unlatch — through the command kind of the limited
//! classification.

use crate::message::InvokedCommand;
use crate::outbound::{PRIVACY_ANSWER_LEAD, PRIVACY_UNPUBLISHED};

/// The command's exact spelling. Recognition matches the invoked command
/// the adapter reports beside the message — never the stored text, which
/// lands verbatim (refined 2026-08-23). The adapter's translation already
/// normalized a self-directed handle suffix away and reported nothing for a
/// foreign-handle one — that command was aimed at someone else.
pub const PRIVACY_COMMAND: &str = "/privacy";

/// Whether this message invokes the privacy command: the reported command
/// is exactly [`PRIVACY_COMMAND`]. Invoking a command is addressing by
/// form, so the answer does not consult the stored addressed fact.
#[must_use]
pub fn is_privacy_command(command: Option<&InvokedCommand>) -> bool {
    command.is_some_and(|command| command.name() == PRIVACY_COMMAND)
}

/// The command's fixed answer: the configured address behind the fixed
/// opening, or the not-yet-published line when none is configured — a legal
/// pointer must be exact and free, which is why no model turn is anywhere
/// near this.
#[must_use]
pub fn privacy_answer(address: Option<&str>) -> String {
    match address {
        Some(address) => format!("{PRIVACY_ANSWER_LEAD}{address}"),
        None => PRIVACY_UNPUBLISHED.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reported_command_decides_and_nothing_else_matches() {
        assert!(is_privacy_command(Some(&InvokedCommand::new("/privacy"))));
        assert!(
            !is_privacy_command(Some(&InvokedCommand::new("/Privacy"))),
            "the spelling is exact"
        );
        assert!(!is_privacy_command(Some(&InvokedCommand::new("/help"))));
        assert!(
            !is_privacy_command(None),
            "a message reporting no command invokes none"
        );
    }

    #[test]
    fn the_answer_is_the_address_or_the_unpublished_line() {
        assert_eq!(
            privacy_answer(Some("https://example.org/privacy")),
            "Privacy policy: https://example.org/privacy"
        );
        assert_eq!(
            privacy_answer(None),
            "The privacy policy is not published yet."
        );
    }
}
