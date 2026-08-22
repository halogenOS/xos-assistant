//! AC6: the token appears in no log line and no error string. The failure
//! paths that format errors are forced — a transport failure whose raw error
//! carries the token-bearing URL, a send dropped past the rate-limit bound,
//! and a malformed state file — and every captured line is scanned.
//!
//! This test owns its own binary on purpose: the capture subscriber is
//! installed as the process-wide default, so every task on every thread logs
//! into it — and a process-wide default can only be owned by a test that
//! shares its process with nothing else. (A thread-scoped subscriber is not
//! enough here: whichever concurrently running test first executes a log
//! statement decides its callsite's cached interest, so a shared process
//! makes the capture racy.)
//!
//! The error strings travel through the same lines: the adapter logs every
//! failure with the error's display form, so a token in any error string
//! would surface in the scan.

// The shared fixtures are compiled into this target too; each test target is
// its own crate, so the parts of the suite's support this scan does not use
// are dead code here by construction.
#[allow(dead_code)]
#[path = "adapter/server.rs"]
mod server;
#[allow(dead_code)]
#[path = "adapter/support.rs"]
mod support;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::span;

use crate::server::BotApiServer;
use crate::support::{
    DEADLINE, TOKEN, TempStateFile, private_update, recording_sleep, spawn_adapter, start_assistant,
};

/// A capture subscriber: every event's fields, formatted into one line.
struct Capture {
    lines: Arc<Mutex<Vec<String>>>,
    next_id: AtomicU64,
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(self.next_id.fetch_add(1, Ordering::SeqCst))
    }
    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut line = String::new();
        event.record(&mut Collect(&mut line));
        self.lines.lock().expect("the line log locks").push(line);
    }
    fn enter(&self, _: &span::Id) {}
    fn exit(&self, _: &span::Id) {}
}

struct Collect<'a>(&'a mut String);

impl tracing::field::Visit for Collect<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        let _ = write!(self.0, "{}={value:?} ", field.name());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_token_reaches_no_log_line_and_no_error_string() {
    let lines = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(Capture {
        lines: Arc::clone(&lines),
        // Span ids must be nonzero; the first handed out is 1.
        next_id: AtomicU64::new(1),
    })
    .expect("this test binary owns the process-wide subscriber");

    force_a_transport_failure().await;
    force_a_dropped_send_and_a_malformed_state_read(&lines).await;
    force_a_cut_short_reply(&lines).await;

    let lines = lines.lock().expect("the line log locks");
    assert!(
        lines
            .iter()
            .any(|line| line.contains("identity fetch failed")),
        "the transport-failure path logged"
    );
    assert!(
        lines.iter().any(|line| line.contains("send failed")),
        "the dropped-send path logged"
    );
    assert!(
        lines.iter().any(|line| line.contains("malformed")),
        "the malformed-state path logged"
    );
    assert!(
        lines.iter().any(|line| line.contains("cut short")),
        "the cut-short path logged"
    );
    let leaks: Vec<&String> = lines.iter().filter(|line| line.contains(TOKEN)).collect();
    assert!(leaks.is_empty(), "the token leaked into output: {leaks:?}");
}

/// Run against a root nothing listens on: the connection is refused on
/// loopback, the raw transport error is born carrying the token-bearing URL,
/// and the logged form must not. The first request the loop makes is the
/// identity fetch, so that is the line the failure surfaces on; the sleep
/// parks forever, so exactly one failure is forced before the timeout ends
/// the run. The state file is never written here — nothing gets that far —
/// but it still comes from the suite's helper, so no run names a path
/// outside the temp directory.
async fn force_a_transport_failure() {
    let fixture = start_assistant().await;
    let state = TempStateFile::new("token-scan-transport");
    let mut config = assistant_adapter_telegram::Config::new(TOKEN, state.path());
    config.api_root = "http://127.0.0.1:1".into();
    let adapter = assistant_adapter_telegram::TelegramAdapter::with_sleep(
        config,
        Arc::new(|_| Box::pin(std::future::pending())),
    );
    let outcome = tokio::time::timeout(
        Duration::from_millis(300),
        adapter.run(Arc::clone(&fixture.assistant)),
    )
    .await;
    assert!(outcome.is_err(), "the run parks on the backoff sleep");
}

/// A send rate-limited past the bound is logged and dropped, and a state
/// file holding garbage is logged as malformed — both under the capture.
async fn force_a_dropped_send_and_a_malformed_state_read(lines: &Arc<Mutex<Vec<String>>>) {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_sends(7, 3);
    server.push_update(private_update(1, 5, "the dropped answer's cause"));
    let state = TempStateFile::new("token-scan");
    std::fs::write(state.path(), "not an offset").expect("the malformed file writes");

    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    server.await_recorded("sendMessage", 3).await;

    // The drop's log arrives just after the third refusal is read.
    await_line(lines, "send failed").await;
}

/// A multi-chunk reply losing a later chunk is logged as cut short — the
/// delivered-count outcome, distinct from the whole-reply drop — under the
/// capture, so the scan covers this path's error string too.
async fn force_a_cut_short_reply(lines: &Arc<Mutex<Vec<String>>>) {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_send_failure_after(1);
    let long_ask = "x".repeat(5000);
    server.push_update(private_update(1, 5, &long_ask));
    let state = TempStateFile::new("token-scan-cut-short");

    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    server.await_recorded("sendMessage", 2).await;

    await_line(lines, "cut short").await;
}

/// Await a captured line carrying the needle, or name the stall.
async fn await_line(lines: &Arc<Mutex<Vec<String>>>, needle: &str) {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        if lines
            .lock()
            .expect("the line log locks")
            .iter()
            .any(|line| line.contains(needle))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting a log line carrying {needle:?}; captured: {:?}",
            lines.lock().expect("the line log locks")
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
