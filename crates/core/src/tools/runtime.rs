//! The runtime-facts tool (unit 32, 2026-08-28): what this process is
//! actually running, answered from values the process holds.
//!
//! Asked which model she runs on, a model answers from its training data
//! and from whatever the chat said earlier — both wrong the moment the
//! deployment's configured model changes. The four facts here come from
//! the process instead: the model id the assembly was configured with, the
//! version compiled into the binary, the revision the build passed in, and
//! the time elapsed since the anchor the binary captured at startup.
//!
//! No parameters, and no input is read: there is nothing to select among
//! four lines, so extra arguments change nothing. The execute path reads
//! process-held values only — no network call, no subprocess, no ledger
//! read — which is why the result cannot be stale in the way a remembered
//! answer is.
//!
//! The revision arrives as a compile-time environment value the build
//! passes in (`ASSISTANT_BUILD_REVISION`), because the deployment builds
//! from a source tree carrying no version-control metadata: nothing at
//! compile time and nothing at run time could read a revision there. A
//! build that passes none answers the literal `unknown`, which is the
//! honest form of not knowing.
//!
//! Uptime is rendered in days, hours and minutes. Seconds would suggest a
//! freshness the other three facts do not have — they are as old as the
//! process — and nobody asking how long the assistant has been up needs
//! them.

use std::time::{Duration, Instant};

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::json;

use crate::message::Authority;

/// The registered name the model calls the tool by.
pub const NAME: &str = "runtime_facts";

/// The authority this tool requires — member: which model answers and how
/// long the process has been up are facts anyone in the group may ask
/// about, and the question comes from ordinary members.
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// What the revision reads when the build passed none. A stated `unknown`
/// beats a value invented from anything else.
pub const UNKNOWN_REVISION: &str = "unknown";

/// The version compiled into this binary. Every crate in the workspace
/// carries the workspace version, so the core's own is the binary's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The build revision, resolved once at compile time.
pub const REVISION: &str = resolve_revision(option_env!("ASSISTANT_BUILD_REVISION"));

/// The one place an absent build revision becomes the literal `unknown`.
const fn resolve_revision(passed: Option<&'static str>) -> &'static str {
    match passed {
        Some(revision) => revision,
        None => UNKNOWN_REVISION,
    }
}

/// The fact list the model reads, one fact per line. A new fact joins as
/// another row here — the shape takes one without restructuring.
#[must_use]
pub fn fact_lines(model: &str, version: &str, revision: &str, uptime: Duration) -> String {
    format!(
        "model: {model}\n\
         version: {version}\n\
         revision: {revision}\n\
         uptime: {uptime}",
        uptime = coarse_uptime(uptime)
    )
}

/// A duration in whole days, hours and minutes, every part always written:
/// a total rendering with no case to choose between, and no seconds.
#[must_use]
pub fn coarse_uptime(uptime: Duration) -> String {
    let minutes = uptime.as_secs() / 60;
    format!(
        "{days}d {hours}h {minutes}m",
        days = minutes / (24 * 60),
        hours = (minutes / 60) % 24,
        minutes = minutes % 60
    )
}

/// The runtime-facts tool. Constructed by the assembly, which holds the
/// configured model id and the start instant the binary captured — the
/// tool reaches for neither itself.
pub(crate) struct RuntimeFacts {
    /// The provider's model identifier every conversation is created
    /// under, as configured.
    model: String,
    /// The monotonic anchor uptime is measured from, captured once at
    /// startup: every call reads the same anchor, so the answer measures
    /// the process and not the call.
    started_at: Instant,
}

impl RuntimeFacts {
    pub(crate) fn new(model: String, started_at: Instant) -> Self {
        Self { model, started_at }
    }

    /// The four facts as of now.
    fn facts(&self) -> String {
        fact_lines(&self.model, VERSION, REVISION, self.started_at.elapsed())
    }
}

impl ToolHandler<CoreEvent> for RuntimeFacts {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "State what this assistant is running right now: the model it is \
                 configured with, its software version and build revision, and how long \
                 the process has been up. Call it whenever someone asks what you run on, \
                 which model or version you are, or how long you have been running — the \
                 answer comes from the running process, never from what you remember or \
                 from what the conversation said earlier. It takes no arguments."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        _ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move { ToolOutcome::Done(self.facts()) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{AgencyCtx, EventBus, Store};

    use super::*;
    use crate::schema::store_config;

    /// The rendering, character for character, over a known tuple —
    /// including the `revision: unknown` form a build that passed none
    /// answers with, and a duration whose seconds are dropped.
    #[test]
    fn the_fact_lines_are_byte_exact() {
        assert_eq!(
            fact_lines(
                "vendor/model-9",
                "0.1.0",
                UNKNOWN_REVISION,
                Duration::from_secs(2 * 86_400 + 3 * 3_600 + 4 * 60 + 59),
            ),
            "model: vendor/model-9\nversion: 0.1.0\nrevision: unknown\nuptime: 2d 3h 4m"
        );
        assert_eq!(
            fact_lines("m", "1.2.3", "abc1234", Duration::ZERO),
            "model: m\nversion: 1.2.3\nrevision: abc1234\nuptime: 0d 0h 0m"
        );
    }

    /// The revision resolution: a passed value stands, an absent one is
    /// the literal fallback, and the compiled-in constant is one of the
    /// two.
    #[test]
    fn an_absent_build_revision_reads_unknown() {
        assert_eq!(resolve_revision(None), "unknown");
        assert_eq!(resolve_revision(Some("0123456789ab")), "0123456789ab");
        assert!(!REVISION.is_empty(), "the compiled revision is never empty");
    }

    /// Uptime is coarse: two durations inside one minute render the same,
    /// and the rendering carries no seconds field at all.
    #[test]
    fn the_uptime_rendering_carries_no_seconds() {
        assert_eq!(coarse_uptime(Duration::from_secs(0)), "0d 0h 0m");
        assert_eq!(coarse_uptime(Duration::from_secs(59)), "0d 0h 0m");
        assert_eq!(
            coarse_uptime(Duration::from_secs(3_600 + 59)),
            coarse_uptime(Duration::from_hours(1))
        );
        let rendered = coarse_uptime(Duration::from_secs(90_061));
        assert_eq!(rendered, "1d 1h 1m");
        assert!(
            !rendered.contains('s'),
            "no seconds field: {rendered} would suggest a freshness the facts lack"
        );
    }

    /// The anchor is the one captured at construction, read at every call:
    /// a tool built with a back-dated instant answers that elapsed time,
    /// which a per-call capture could never do, and two calls in a row
    /// answer alike.
    #[test]
    fn the_uptime_measures_the_anchor_captured_once() {
        let started_at = Instant::now()
            .checked_sub(Duration::from_mins(26 * 60 + 5))
            .expect("the monotonic clock takes the back-dated anchor");
        let tool = RuntimeFacts::new("vendor/model-9".into(), started_at);
        assert!(
            tool.facts().ends_with("uptime: 1d 2h 5m"),
            "the stored anchor is what uptime measures: {}",
            tool.facts()
        );
        assert_eq!(tool.facts(), tool.facts(), "two calls, one anchor");
    }

    /// The execute path reads process-held values only: over a store
    /// holding nothing and a block id naming nothing, every input — an
    /// empty one, an object with strange fields, unparsable text — answers
    /// the same four facts. Nothing is selected and nothing is read.
    #[tokio::test]
    async fn every_input_answers_the_same_facts_without_reading_the_ledger() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let agency: AgencyCtx<CoreEvent> = AgencyCtx {
            conversation_id: 404,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let tool = RuntimeFacts::new("vendor/model-9".into(), Instant::now());
        for input in ["", "{}", r#"{"fact":"model"}"#, "not json"] {
            let outcome = tool
                .execute(
                    input,
                    ToolContext {
                        agency: &agency,
                        tool_call_id: "call-0",
                        block_id: 12345,
                    },
                )
                .await;
            match outcome {
                ToolOutcome::Done(facts) => assert!(
                    facts.starts_with("model: vendor/model-9\n"),
                    "the input {input:?} answers the configured model's facts: {facts}"
                ),
                ToolOutcome::Error(_) | ToolOutcome::Pending => {
                    panic!("the input {input:?} answers the facts")
                }
            }
        }
    }
}
