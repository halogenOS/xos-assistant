//! The shared acknowledgment-window mechanism (decided 2026-08-23, refined
//! at the unit's close): every fixed line a non-operator can trigger goes
//! out at most once per channel per window, and note appends of one topic
//! are capped within the same window. One window length binds both — the
//! flood-amplifier discipline the protection unit recorded for notices,
//! applied to chat lines and ledger growth alike.
//!
//! The bookkeeping is in-memory on purpose: the bounded lines are courtesy
//! and legal-pointer lines with at-most-once intent per window, and a
//! restart forgetting the windows costs at most one extra line and one
//! burst of capped appends. Expired entries are swept on every access, so
//! neither map outlives its own window.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// At most one fixed line per channel inside this window — the rules
/// acknowledgment and the privacy command's answer each keep their own
/// per-channel bookkeeping over this one length; a further trigger within
/// it is recorded silence. Note appends share the length as their cap
/// window.
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

/// How many notes of one topic may append per conversation inside one
/// window. A pin toggler's burst appends at most this many system-voiced
/// notes; a capped delta is not queued — the next observation after the
/// window re-reads the stored newest note and appends the still-standing
/// delta then.
pub const NOTE_TOPIC_APPEND_CAP: u32 = 3;

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

/// The per-key append budget of one window: the first recorded append opens
/// the key's window, further appends count against [`NOTE_TOPIC_APPEND_CAP`]
/// inside it, and the window's expiry resets the count whole.
///
/// Admission and spend are two calls on purpose: [`Self::admits`] is a pure
/// read gating the append, and [`Self::record`] counts it only once it
/// stands — a transiently failed append spends nothing, so its redelivery is
/// not capped by its own failures. The check-then-record pair is not atomic
/// here; the one caller runs it under the stamp lock, which serializes every
/// note append anyway.
pub(crate) struct AppendWindow<K> {
    opened: Mutex<HashMap<K, (Instant, u32)>>,
}

impl<K: Eq + Hash> AppendWindow<K> {
    pub(crate) fn new() -> Self {
        Self {
            opened: Mutex::new(HashMap::new()),
        }
    }

    /// Whether one more append is admitted now — a read, spending nothing.
    /// Expired windows are swept here, so the map holds only keys inside a
    /// live window.
    pub(crate) async fn admits(&self, key: K) -> bool {
        let mut opened = self.opened.lock().await;
        let now = Instant::now();
        opened.retain(|_, (start, _)| now.duration_since(*start) < ACKNOWLEDGMENT_WINDOW);
        opened
            .get(&key)
            .is_none_or(|(_, count)| *count < NOTE_TOPIC_APPEND_CAP)
    }

    /// One admitted append stood; count it. The first recorded append opens
    /// the key's window.
    pub(crate) async fn record(&self, key: K) {
        let mut opened = self.opened.lock().await;
        let now = Instant::now();
        opened.retain(|_, (start, _)| now.duration_since(*start) < ACKNOWLEDGMENT_WINDOW);
        opened
            .entry(key)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((now, 1));
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
    /// this instance into the report tool — and because a paused full
    /// assembly is unsound: the store's actor is an external thread, and
    /// the paused clock auto-advances past every deadline while a task
    /// awaits it.
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

    #[tokio::test(start_paused = true)]
    async fn an_append_window_admits_the_cap_then_resets_past_the_window() {
        let window: AppendWindow<i64> = AppendWindow::new();
        for _ in 0..NOTE_TOPIC_APPEND_CAP {
            assert!(
                window.admits(1).await,
                "appends within the cap are admitted"
            );
            window.record(1).await;
        }
        assert!(!window.admits(1).await, "the capped key admits no more");
        assert!(window.admits(2).await, "keys are capped independently");
        tokio::time::advance(ACKNOWLEDGMENT_WINDOW + Duration::from_secs(1)).await;
        assert!(
            window.admits(1).await,
            "the expired window resets the count"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_unrecorded_admission_spends_nothing() {
        let window: AppendWindow<i64> = AppendWindow::new();
        for _ in 0..(NOTE_TOPIC_APPEND_CAP * 2) {
            assert!(
                window.admits(1).await,
                "an admission without a recorded append is a free read"
            );
        }
        window.record(1).await;
        assert!(
            window.admits(1).await,
            "only recorded appends count against the cap"
        );
    }
}
