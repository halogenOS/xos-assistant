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

/// How many notes of one topic may append per conversation inside one
/// window. A pin toggler's burst appends at most this many system-voiced
/// notes; a capped delta is not queued — the next observation after the
/// window re-reads the stored newest note and appends the still-standing
/// delta then.
pub const NOTE_TOPIC_APPEND_CAP: u32 = 3;

/// The at-most-once-per-window bookkeeping of one fixed line, keyed by
/// conversation. The window opens at the grant that actually happened, so
/// a run of silent triggers cannot postpone the next grant forever.
pub(crate) struct LineWindow {
    granted: Mutex<HashMap<i64, Instant>>,
}

impl LineWindow {
    pub(crate) fn new() -> Self {
        Self {
            granted: Mutex::new(HashMap::new()),
        }
    }

    /// Whether the line goes out now: `true` opens the channel's window,
    /// `false` is the recorded silence within it. Expired windows are swept
    /// here, so the map holds only channels inside a live window.
    pub(crate) async fn grants(&self, channel: i64) -> bool {
        let mut granted = self.granted.lock().await;
        let now = Instant::now();
        granted.retain(|_, at| now.duration_since(*at) < ACKNOWLEDGMENT_WINDOW);
        if granted.contains_key(&channel) {
            return false;
        }
        granted.insert(channel, now);
        true
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
        let window = LineWindow::new();
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
