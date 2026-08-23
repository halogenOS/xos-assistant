//! The privacy command family: the deterministic answers the published
//! terms require to be easy to reach. The notice command was decided
//! 2026-08-23; the self-service family — opt-out, deletion with its
//! programmatic confirm, and opt-in — joined the same day with the
//! privacy-self-service unit. Every command is recognized here from the
//! invoked-command report, answered with a fixed line owned here, and
//! stamped by the entry point as taking no debt — no turn, no answer-window
//! count, no unlatch — through the command kind of the limited
//! classification.
//!
//! The family is exempt from suppression: an opted-out person's
//! `/unblockprivacy` must work, or the door never reopens from inside, and
//! their `/privacy` keeps answering. The entry point records an exempted
//! command through the read-only identity path, so the freeze the
//! suppression stub promises holds even across the person's own commands.
//!
//! The family splits in two on the type: the notice, and the self-service
//! rights commands. They are opposites in every rule the unit states —
//! channel-keyed vs principal-keyed reply window, budget-consulted vs
//! budget-exempt, no state change vs state change — so the split lives in
//! [`PrivacyCommand`] itself and every reader matches it totally instead
//! of guarding an invariant the compiler cannot check.
//!
//! The deletion flow's pending memory lives here too, beside the commands
//! that file and consume it: the crate-internal `PendingDeletions`, keyed
//! by principal over [`CONFIRM_WINDOW`]. It is the rights mechanism's own
//! state, not a bound on how often a line goes out — the bounds live in
//! the window module.

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::message::InvokedCommand;
use crate::outbound::{PRIVACY_ANSWER_LEAD, PRIVACY_UNPUBLISHED};

/// The notice command's exact spelling. Recognition matches the invoked
/// command the adapter reports beside the message — never the stored text,
/// which lands verbatim (refined 2026-08-23). The adapter's translation
/// already normalized a self-directed handle suffix away and reported
/// nothing for a foreign-handle one — that command was aimed at someone
/// else.
pub const PRIVACY_COMMAND: &str = "/privacy";

/// The opt-out command: from its answer on, the sender's inbound messages
/// on this adapter are dropped at ingestion.
pub const OPT_OUT_COMMAND: &str = "/privacyout";

/// The deletion ask: files the sender's pending confirmation and answers
/// the confirm instruction.
pub const DELETE_COMMAND: &str = "/privacydelete";

/// The deletion confirm: consumes a live pending and starts the erasure.
pub const CONFIRM_COMMAND: &str = "/confirmdelete";

/// The opt-in command, under the operator's chosen name: clears the
/// standing opt-out flag.
pub const OPT_IN_COMMAND: &str = "/unblockprivacy";

/// One recognized member of the privacy command family: the notice, or one
/// of the self-service rights commands. The two kinds answer under opposite
/// rules, so the split is on the type and every reader's match is total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyCommand {
    /// `/privacy` — the policy pointer. A notice, not a state change: it
    /// keeps its channel-keyed answer window and its budget consultation.
    Notice,
    /// One of the four rights commands: principal-keyed reply window,
    /// budget-exempt, its state change applied exactly with the granted
    /// reply.
    SelfService(RightsCommand),
}

/// One self-service rights command — the state-changing members of the
/// family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightsCommand {
    /// `/privacyout` — set the suppression flag.
    OptOut,
    /// `/privacydelete` — file the pending confirmation.
    Delete,
    /// `/confirmdelete` — consume a live pending and start the erasure.
    Confirm,
    /// `/unblockprivacy` — clear the suppression flag.
    OptIn,
}

/// The family member a message invokes, if any: the reported command
/// matched against the five exact spellings. Invoking a command is
/// addressing by form, so the answers never consult the stored addressed
/// fact, and every member works unaddressed in groups.
#[must_use]
pub fn family_command(command: Option<&InvokedCommand>) -> Option<PrivacyCommand> {
    match command?.name() {
        PRIVACY_COMMAND => Some(PrivacyCommand::Notice),
        OPT_OUT_COMMAND => Some(PrivacyCommand::SelfService(RightsCommand::OptOut)),
        DELETE_COMMAND => Some(PrivacyCommand::SelfService(RightsCommand::Delete)),
        CONFIRM_COMMAND => Some(PrivacyCommand::SelfService(RightsCommand::Confirm)),
        OPT_IN_COMMAND => Some(PrivacyCommand::SelfService(RightsCommand::OptIn)),
        _ => None,
    }
}

// ─── The fixed lines, exact copy per the unit spec (2026-08-23) ──────────

/// The opt-out's answer when the flag was just set. It states the plain
/// reach openly: opting out stops collection going forward on this
/// platform, what was stored before stands until deletion, and undoing is
/// one command away.
pub const OPT_OUT_DONE: &str = "Understood. From now on your messages here are not collected \
     and not answered on this platform. What was stored before stays until you ask for \
     deletion with /privacydelete. Undo with /unblockprivacy.";

/// The opt-out's answer when the flag already stood.
pub const OPT_OUT_ALREADY: &str = "You are already opted out. Undo with /unblockprivacy, or delete stored data with \
     /privacydelete.";

/// The opt-in's answer when the flag was just cleared.
pub const OPT_IN_DONE: &str =
    "Collection is on again for you. Nothing that was deleted comes back.";

/// The opt-in's answer when no flag stood.
pub const OPT_IN_ALREADY: &str = "You were not opted out. Nothing changed.";

/// The deletion ask's answer: the confirm instruction, naming the confirm
/// command and the window's length in the person's own terms.
pub const CONFIRM_INSTRUCTION: &str = "To delete your stored data, reply /confirmdelete \
     within five minutes. This removes your messages and identity data and cannot be undone.";

/// The confirm's answer when a live pending was consumed. It promises what
/// the mechanism delivers — the deletion is underway, not instantaneously
/// done: the erasure runs as its own spawned task after the ingestion
/// returns, a failure is logged and leaves the data standing, and re-asking
/// works.
pub const DELETION_STARTED: &str =
    "Deletion is underway. Your messages and identity data are being removed.";

/// The confirm's answer when nothing pending stood — never filed, lapsed,
/// or already consumed alike: a lapsed pending IS nothing pending, and a
/// second confirm after a completed run answers the same line.
pub const NOTHING_PENDING: &str =
    "There is no deletion waiting for confirmation. Start one with /privacydelete.";

// ─── The deletion flow's pending memory ──────────────────────────────────

/// How long a filed deletion waits for its `/confirmdelete` (decided
/// 2026-08-23): the pending state lapses past this, and a lapsed pending IS
/// nothing pending — the confirm instruction's own "within five minutes" is
/// this constant spoken to the person.
pub const CONFIRM_WINDOW: Duration = Duration::from_mins(5);

/// The most principals the pending-deletion memory holds. Past the cap the
/// memory is cleared whole — the established memory-cap shape, and deletion
/// is the flow where forgetting errs safe: a cleared pending answers the
/// nothing-pending line and the person re-asks with `/privacydelete`.
const PENDING_DELETION_CAP: usize = 4096;

/// The pending deletion confirmations, keyed by PRINCIPAL (decided
/// 2026-08-23): a deletion asked in one chat confirms in any, since the
/// person is the subject, not the room. Process memory on purpose —
/// forgotten on restart, because deletion is the flow where forgetting errs
/// safe: a lost pending answers the nothing-pending line and the person
/// re-asks. Entries lapse past [`CONFIRM_WINDOW`] and are swept on every
/// access; the map is bounded by [`PENDING_DELETION_CAP`] and cleared whole
/// at the cap, the established memory-cap shape.
pub(crate) struct PendingDeletions {
    filed: Mutex<HashMap<i64, Instant>>,
}

impl PendingDeletions {
    pub(crate) fn new() -> Self {
        Self {
            filed: Mutex::new(HashMap::new()),
        }
    }

    /// File one principal's pending confirmation, opening — or refreshing —
    /// its window: `/privacydelete` is idempotent, and a re-ask starts the
    /// five minutes over.
    pub(crate) async fn file(&self, principal_id: i64) {
        let mut filed = self.filed.lock().await;
        let now = Instant::now();
        filed.retain(|_, at| now.duration_since(*at) < CONFIRM_WINDOW);
        if filed.len() >= PENDING_DELETION_CAP {
            tracing::warn!("the pending-deletion memory reached its cap and was cleared");
            filed.clear();
        }
        filed.insert(principal_id, now);
    }

    /// Consume one principal's pending confirmation: `true` exactly when a
    /// live pending stood — the confirm that starts the erasure. A lapsed
    /// pending IS nothing pending and answers `false`, exactly like one
    /// never filed or one already consumed.
    pub(crate) async fn take(&self, principal_id: i64) -> bool {
        let mut filed = self.filed.lock().await;
        let now = Instant::now();
        filed.retain(|_, at| now.duration_since(*at) < CONFIRM_WINDOW);
        filed.remove(&principal_id).is_some()
    }
}

/// The notice command's fixed answer: the configured address behind the
/// fixed opening, or the not-yet-published line when none is configured — a
/// legal pointer must be exact and free, which is why no model turn is
/// anywhere near this.
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
    fn the_family_recognizes_its_five_exact_spellings_and_nothing_else() {
        for (token, member) in [
            (PRIVACY_COMMAND, PrivacyCommand::Notice),
            (
                OPT_OUT_COMMAND,
                PrivacyCommand::SelfService(RightsCommand::OptOut),
            ),
            (
                DELETE_COMMAND,
                PrivacyCommand::SelfService(RightsCommand::Delete),
            ),
            (
                CONFIRM_COMMAND,
                PrivacyCommand::SelfService(RightsCommand::Confirm),
            ),
            (
                OPT_IN_COMMAND,
                PrivacyCommand::SelfService(RightsCommand::OptIn),
            ),
        ] {
            assert_eq!(
                family_command(Some(&InvokedCommand::new(token))),
                Some(member),
                "{token} recognizes its member"
            );
        }
        assert_eq!(
            family_command(Some(&InvokedCommand::new("/PrivacyOut"))),
            None,
            "the spelling is exact"
        );
        assert_eq!(family_command(Some(&InvokedCommand::new("/help"))), None);
        assert_eq!(
            family_command(None),
            None,
            "a message reporting no command invokes none"
        );
    }

    /// The command tokens, pinned to the spec's exact spellings — the
    /// opt-in under the operator's chosen name.
    #[test]
    fn the_command_tokens_are_the_spec_spellings() {
        assert_eq!(OPT_OUT_COMMAND, "/privacyout");
        assert_eq!(DELETE_COMMAND, "/privacydelete");
        assert_eq!(CONFIRM_COMMAND, "/confirmdelete");
        assert_eq!(OPT_IN_COMMAND, "/unblockprivacy");
    }

    /// Every fixed line of the family, pinned verbatim against the unit
    /// spec's copy.
    #[test]
    fn the_fixed_lines_match_the_spec_copy_verbatim() {
        assert_eq!(
            OPT_OUT_DONE,
            "Understood. From now on your messages here are not collected and not answered \
             on this platform. What was stored before stays until you ask for deletion with \
             /privacydelete. Undo with /unblockprivacy."
        );
        assert_eq!(
            OPT_OUT_ALREADY,
            "You are already opted out. Undo with /unblockprivacy, or delete stored data \
             with /privacydelete."
        );
        assert_eq!(
            OPT_IN_DONE,
            "Collection is on again for you. Nothing that was deleted comes back."
        );
        assert_eq!(OPT_IN_ALREADY, "You were not opted out. Nothing changed.");
        assert_eq!(
            CONFIRM_INSTRUCTION,
            "To delete your stored data, reply /confirmdelete within five minutes. This \
             removes your messages and identity data and cannot be undone."
        );
        assert_eq!(
            DELETION_STARTED,
            "Deletion is underway. Your messages and identity data are being removed."
        );
        assert_eq!(
            NOTHING_PENDING,
            "There is no deletion waiting for confirmation. Start one with /privacydelete."
        );
        assert!(
            CONFIRM_INSTRUCTION.contains(CONFIRM_COMMAND),
            "the instruction names the literal confirm token"
        );
    }

    /// The confirm window under paused time (AC3): a filed pending confirms
    /// inside five minutes, a lapsed one answers as nothing pending, a
    /// consumed one does not confirm twice, and a re-filed pending starts
    /// its window over. The same lapse is pinned through the command path in
    /// the spine suite; this pins the primitive's edges exactly.
    #[tokio::test(start_paused = true)]
    async fn a_pending_deletion_confirms_inside_the_window_and_lapses_past_it() {
        let pending = PendingDeletions::new();
        assert!(!pending.take(1).await, "nothing filed is nothing pending");
        pending.file(1).await;
        tokio::time::advance(
            CONFIRM_WINDOW
                .checked_sub(Duration::from_secs(1))
                .expect("the window is longer than a second"),
        )
        .await;
        assert!(pending.take(1).await, "the live pending confirms");
        assert!(
            !pending.take(1).await,
            "a consumed pending is nothing pending — a second confirm answers so"
        );
        pending.file(2).await;
        tokio::time::advance(CONFIRM_WINDOW + Duration::from_secs(1)).await;
        assert!(
            !pending.take(2).await,
            "a lapsed pending IS nothing pending"
        );
        pending.file(3).await;
        tokio::time::advance(
            CONFIRM_WINDOW
                .checked_sub(Duration::from_secs(30))
                .expect("the window is longer than half a minute"),
        )
        .await;
        pending.file(3).await;
        tokio::time::advance(Duration::from_mins(1)).await;
        assert!(
            pending.take(3).await,
            "a re-filed pending starts its five minutes over"
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
