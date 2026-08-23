//! The shared windowed bounds (decided 2026-08-23, refined at each unit's
//! close): every structure here answers one question — "may this go out
//! now" — over a named window length, and nothing here is a feature's own
//! state. Two bounds live in this module:
//!
//! - [`LineWindow`] — at most one fixed line per channel per window: the
//!   privacy notice's answer and the report filing each keep an instance.
//!   The rules acknowledgment left this bound on 2026-08-23: pinning is an
//!   administrator-only right, so the pin-toggling spammer the window was
//!   built against cannot exist, and the window only silenced legitimate
//!   rules edits — the on-delta note comparison is the rules path's whole
//!   admission check now.
//! - [`ReplyWindow`] — up to a cap of replies per key per window, the
//!   privacy family's per-person bound, carrying the one writing of the
//!   grant-exactly-with-the-action protocol in [`ReplyWindow::grant_with`].
//!
//! The flood-amplifier discipline the protection unit recorded for notices
//! applies to the lines anyone can trigger. The bookkeeping is in-memory on
//! purpose: a restart forgetting the windows costs at most one window of
//! extra lines. Expired entries are swept on every access, so no map
//! outlives its own window. Domain state with a window of its own — the
//! deletion flow's pending memory — lives with its feature, not here.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// At most one fixed line per channel inside this window — the privacy
/// command's answer keeps its per-channel bookkeeping over this length; a
/// further trigger within it is recorded silence. The name predates the
/// rules acknowledgment's departure from the bound (2026-08-23): the
/// notice window it still measures answers a real vector — a flood of
/// failed or repeated triggers anyone in the chat can cause.
pub const ACKNOWLEDGMENT_WINDOW: Duration = Duration::from_mins(5);

/// At most one filed report per channel inside this window — the report
/// tool's own bound (decided 2026-08-23), a named constant of its own
/// instead of the acknowledgment length, because the two lines answer
/// different threats: an acknowledgment is a courtesy line, a report pings
/// every group administrator through the moderation bot. The window is
/// process memory, and the re-argument is this unit's own instead of the
/// courtesy-line rationale: for THIS bound a restart forgives at most one
/// extra report, which is one extra ping — accepted.
pub const REPORT_WINDOW: Duration = Duration::from_mins(5);

/// The rights replies' own per-person window (decided 2026-08-23), the same
/// length as the acknowledgment window: the four privacy commands and the
/// privacy tool's deterministic replies are bounded per PRINCIPAL inside
/// it, so one person's flood bounds that person alone and a neighbor's
/// commands starve nobody's right.
pub const PRIVACY_REPLY_WINDOW: Duration = Duration::from_mins(5);

/// How many rights replies one person draws inside one
/// [`PRIVACY_REPLY_WINDOW`]. The unit spec names the window and keys it by
/// principal; the count that makes it a bound is fixed here: eight covers a
/// person's whole flow with repeats — opt out, ask, confirm, undo, and the
/// already-so answers between — while a flood past it draws recorded
/// silence, and the state change a silenced command would make is withheld
/// with the reply, never applied silently.
pub const PRIVACY_REPLY_CAP: u32 = 8;

/// The at-most-once-per-window bookkeeping of one fixed line, keyed by
/// conversation, over the window length it is constructed with. The window
/// opens at the grant that actually happened, so a run of silent triggers
/// cannot postpone the next grant forever.
pub(crate) struct LineWindow {
    window: Duration,
    granted: Mutex<HashMap<i64, Instant>>,
}

impl LineWindow {
    pub(crate) fn new(window: Duration) -> Self {
        Self {
            window,
            granted: Mutex::new(HashMap::new()),
        }
    }

    /// Whether the line goes out now: `true` opens the channel's window,
    /// `false` is the recorded silence within it. Expired windows are swept
    /// here, so the map holds only channels inside a live window. The
    /// check and the spend are one atomic step under the lock — two racing
    /// callers cannot both be granted — which is what makes this the
    /// report bound's atomic grant.
    pub(crate) async fn grants(&self, channel: i64) -> bool {
        let mut granted = self.granted.lock().await;
        let now = Instant::now();
        granted.retain(|_, at| now.duration_since(*at) < self.window);
        if granted.contains_key(&channel) {
            return false;
        }
        granted.insert(channel, now);
        true
    }

    /// Hand a grant back: the granted action failed transiently before it
    /// stood, so the channel's window closes again as if never opened. This
    /// is how a spend-only-after-the-append ordering rides an atomic grant:
    /// the grant is taken first — so a concurrent second ask is declined —
    /// and revoked when the append does not stand, so a redelivered ask is
    /// not silenced by a failure that delivered nothing.
    pub(crate) async fn revoke(&self, channel: i64) {
        self.granted.lock().await.remove(&channel);
    }
}

/// The per-key reply bound of one window: up to `cap` grants per key inside
/// the window it is constructed with, the first grant opening the key's
/// window and its expiry resetting the count whole. The check and the spend
/// are one atomic step under the lock, like [`LineWindow::grants`], and
/// [`Self::revoke`] hands one grant back when the granted action failed
/// before it stood. Keys are the privacy family's principal ids; the
/// bookkeeping is in-memory on purpose, under the same reasoning as the
/// module's other windows — a restart forgives at most one window of
/// extra replies.
pub(crate) struct ReplyWindow {
    window: Duration,
    cap: u32,
    spent: Mutex<HashMap<i64, (Instant, u32)>>,
}

impl ReplyWindow {
    pub(crate) fn new(window: Duration, cap: u32) -> Self {
        Self {
            window,
            cap,
            spent: Mutex::new(HashMap::new()),
        }
    }

    /// Whether one more reply goes out now for this key: `true` counts the
    /// grant, `false` is the recorded silence of an exhausted window.
    /// Expired windows are swept here, so the map holds only keys inside a
    /// live window.
    pub(crate) async fn grants(&self, key: i64) -> bool {
        let mut spent = self.spent.lock().await;
        let now = Instant::now();
        spent.retain(|_, (start, _)| now.duration_since(*start) < self.window);
        match spent.get_mut(&key) {
            Some((_, count)) if *count >= self.cap => false,
            Some((_, count)) => {
                *count += 1;
                true
            }
            None => {
                spent.insert(key, (now, 1));
                true
            }
        }
    }

    /// Hand one grant back: the granted action failed transiently before it
    /// stood, so the spent slot reopens and a redelivered ask is not
    /// silenced by a failure that delivered nothing.
    pub(crate) async fn revoke(&self, key: i64) {
        let mut spent = self.spent.lock().await;
        if let Some((_, count)) = spent.get_mut(&key) {
            *count = count.saturating_sub(1);
        }
    }

    /// Spend one grant on a fallible change, as one operation — the one
    /// writing of the grant-exactly-with-the-reply protocol the command
    /// path and the privacy tool both ride (decided 2026-08-23). The grant
    /// is taken first, so a concurrent ask is bounded; the change runs only
    /// when granted, so a state change never lands into recorded silence;
    /// and a failed change hands the grant back before it is reported, so a
    /// redelivered ask is not silenced by a failure that delivered nothing.
    ///
    /// `None` is the exhausted window — the change never ran. `Some(Ok(_))`
    /// is the change standing with its grant. `Some(Err(_))` is the failed
    /// change after the revoke; the caller supplies its own wording for
    /// each.
    pub(crate) async fn grant_with<T, E>(
        &self,
        key: i64,
        change: impl Future<Output = Result<T, E>>,
    ) -> Option<Result<T, E>> {
        if !self.grants(key).await {
            return None;
        }
        match change.await {
            Ok(value) => Some(Ok(value)),
            Err(error) => {
                self.revoke(key).await;
                Some(Err(error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_line_window_grants_once_then_again_past_the_window() {
        let window = LineWindow::new(ACKNOWLEDGMENT_WINDOW);
        assert!(window.grants(1).await, "the first trigger is granted");
        assert!(
            !window.grants(1).await,
            "recorded silence within the window"
        );
        assert!(window.grants(2).await, "channels bound independently");
        tokio::time::advance(ACKNOWLEDGMENT_WINDOW + Duration::from_secs(1)).await;
        assert!(window.grants(1).await, "the expired window grants again");
        assert!(
            window.granted.lock().await.len() < 3,
            "expired entries are swept on access"
        );
    }

    /// The report window's own reopening, under paused time: one grant per
    /// channel inside [`REPORT_WINDOW`], and the expired window grants
    /// again. Pinned on the primitive because the assembly injects exactly
    /// this instance into the report tool — and because the window
    /// arithmetic stays exact here: the store's actor is an external
    /// thread, so under a paused full assembly the auto-advancing clock
    /// can jump while a store roundtrip is in flight and expire a window
    /// mid-operation. A paused full assembly itself works — the confirm
    /// window's spine pin runs one — but its clock cannot be trusted to
    /// sit still between two store reads.
    #[tokio::test(start_paused = true)]
    async fn the_report_window_declines_within_and_reopens_past_its_length() {
        let window = LineWindow::new(REPORT_WINDOW);
        assert!(window.grants(1).await, "the first filing takes the grant");
        assert!(
            !window.grants(1).await,
            "a second ask inside the window is declined"
        );
        tokio::time::advance(
            REPORT_WINDOW
                .checked_sub(Duration::from_secs(1))
                .expect("the window is longer than a second"),
        )
        .await;
        assert!(
            !window.grants(1).await,
            "still inside the window, still declined"
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(
            window.grants(1).await,
            "past the window, the channel files again"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_revoked_grant_spends_nothing() {
        let window = LineWindow::new(REPORT_WINDOW);
        assert!(window.grants(1).await, "the first ask takes the grant");
        window.revoke(1).await;
        assert!(
            window.grants(1).await,
            "a revoked grant closes the window again, as if never opened"
        );
        assert!(
            !window.grants(1).await,
            "the re-taken grant is spent normally"
        );
        window.revoke(2).await;
        assert!(
            window.grants(2).await,
            "revoking an unopened channel is a no-op"
        );
    }

    /// The one-operation protocol over the reply bound: an exhausted window
    /// never runs the change, a granted change spends its slot, and a
    /// failed change hands the grant back — so a redelivery is not silenced
    /// by a failure that delivered nothing.
    #[tokio::test(start_paused = true)]
    async fn a_granted_change_spends_and_a_failed_one_hands_the_grant_back() {
        let window = ReplyWindow::new(PRIVACY_REPLY_WINDOW, 1);
        let failed: Option<Result<&str, &str>> = window.grant_with(1, async { Err("down") }).await;
        assert_eq!(
            failed,
            Some(Err("down")),
            "the failed change is reported after the revoke"
        );
        let granted: Option<Result<&str, &str>> = window.grant_with(1, async { Ok("stood") }).await;
        assert_eq!(
            granted,
            Some(Ok("stood")),
            "the handed-back grant is spendable again"
        );
        let exhausted: Option<Result<&str, &str>> = window
            .grant_with(1, async {
                panic!("an exhausted window never runs the change")
            })
            .await;
        assert_eq!(exhausted, None, "the exhausted window is recorded silence");
    }

    /// The per-person reply bound under paused time (AC5): one key's cap
    /// exhausts without touching another key, a revoked grant reopens its
    /// slot, and the expired window resets the count whole.
    #[tokio::test(start_paused = true)]
    async fn the_reply_window_caps_per_key_and_resets_past_the_window() {
        let window = ReplyWindow::new(PRIVACY_REPLY_WINDOW, PRIVACY_REPLY_CAP);
        for _ in 0..PRIVACY_REPLY_CAP {
            assert!(window.grants(1).await, "grants inside the cap");
        }
        assert!(!window.grants(1).await, "the exhausted key is silence");
        assert!(
            window.grants(2).await,
            "another person's replies are bounded independently"
        );
        window.revoke(1).await;
        assert!(
            window.grants(1).await,
            "a revoked grant reopens the slot it spent"
        );
        assert!(!window.grants(1).await, "the re-spent cap holds again");
        tokio::time::advance(PRIVACY_REPLY_WINDOW + Duration::from_secs(1)).await;
        assert!(
            window.grants(1).await,
            "the expired window resets the count whole"
        );
    }
}
