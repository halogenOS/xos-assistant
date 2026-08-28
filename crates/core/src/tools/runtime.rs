//! The runtime-facts tool (unit 32, 2026-08-28): what this process is
//! actually running, answered from values the process holds.
//!
//! Asked which model she runs on, a model answers from its training data
//! and from whatever the chat said earlier — both wrong the moment the
//! deployment's configured model changes. The facts here come from the
//! running system instead: the model this conversation's turns are
//! dispatched on, the version compiled into the binary, the revision the
//! build passed in, the time elapsed since the anchor the binary captured
//! at startup, and — since unit 34, 2026-08-29 — the date and time the
//! machine's own clock says.
//!
//! The model is read from the answering conversation's own record, which
//! is the value the framework dispatches its turns on. Configuration is
//! not that value: a conversation's model is written once, when the
//! conversation is created, a fork inherits it, and a later configuration
//! change moves no conversation already open. Stating the configured id
//! would name a model the wire does not carry — the exact claim this tool
//! exists to end — so the read happens per call, at the conversation in
//! hand, and nothing about it is remembered here.
//!
//! The date and time are the framework's one clock reading, taken at
//! execute time. The ledger's date markers state the date, which stays
//! true all day, and deliberately never the minute they were written at;
//! a model asked the time answered from a marker is right once and stale
//! for the rest of the day. So the tool reads the clock per call, through
//! the same source the markers are stamped from — the reading is the only
//! clock this crate has, and re-deriving local time here would write the
//! clock decision a second time and let the two drift apart. Whatever the
//! reading says about the zone is stated; whatever it leaves absent stays
//! absent, because a zone worked out from the other parts is a guess.
//!
//! No parameters, and no input is read: there is nothing to select among
//! these lines, so extra arguments change nothing. Beyond the read of the
//! conversation's own record and the read of the clock, the execute path
//! reads process-held values only — no network call, no subprocess —
//! which is why the result cannot be stale in the way a remembered answer
//! is.
//!
//! The revision arrives as a compile-time environment value the build
//! passes in (`ASSISTANT_BUILD_REVISION`), because the deployment builds
//! from a source tree carrying no version-control metadata: nothing at
//! compile time and nothing at run time could read a revision there. A
//! build that passes none answers the literal `unknown`, which is the
//! honest form of not knowing.
//!
//! Uptime is rendered in days, hours and minutes. Seconds would suggest a
//! freshness the model, version and revision do not have — they are as old
//! as the process — and nobody asking how long the assistant has been up
//! needs them. The clock's own line stops at the minute too, which is
//! where the reading itself stops.

use std::time::{Duration, Instant};

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::ClockReading;
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

/// The decline when the conversation's own record cannot be read, so the
/// model the turn runs on is unknown. The facts are withheld whole: a
/// partial list missing its first line would be read as an answer, and
/// substituting any other model id is the invention this tool exists to
/// end.
pub const UNREADABLE_RESULT: &str = "The runtime facts could not be read just now. Say \
     so plainly, and state nothing about which model, version or uptime is running.";

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
///
/// The last two rows render `clock`: the date with the weekday it falls
/// on, and the time with the zone the reading answers for. Nothing here
/// parses or recomputes any part of the reading — the parts are stated as
/// they were read.
#[must_use]
pub fn fact_lines(
    model: &str,
    version: &str,
    revision: &str,
    uptime: Duration,
    clock: &ClockReading,
) -> String {
    format!(
        "model: {model}\n\
         version: {version}\n\
         revision: {revision}\n\
         uptime: {uptime}\n\
         date: {date} ({weekday})\n\
         time: {time}{zone}",
        uptime = coarse_uptime(uptime),
        date = clock.date,
        weekday = clock.weekday,
        time = clock.time,
        zone = zone_clause(clock)
    )
}

/// What the time line says about the zone it is stated in: the
/// abbreviation with the name when the reading carries both, whichever one
/// it carries alone, and nothing at all when it carries neither.
///
/// An absent part is written nowhere rather than filled in: the reading
/// leaves a source's answer NULL exactly when that source answered
/// nothing, and the honest rendering of nothing is silence — a zone
/// inferred from the date or from the other part would be this tool's own
/// invention, which is what it exists to end.
fn zone_clause(clock: &ClockReading) -> String {
    match (clock.tz_abbrev.as_deref(), clock.tz_name.as_deref()) {
        (Some(abbrev), Some(name)) => format!(" {abbrev} ({name})"),
        (Some(zone), None) | (None, Some(zone)) => format!(" {zone}"),
        (None, None) => String::new(),
    }
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
/// start instant the binary captured — the one fact the tool could not
/// reach for itself. The model is not held here: it belongs to the
/// conversation being answered, not to the process, and a copy kept here
/// would be the stale answer again in another place.
pub(crate) struct RuntimeFacts {
    /// The monotonic anchor uptime is measured from, captured once at
    /// startup: every call reads the same anchor, so the answer measures
    /// the process and not the call.
    started_at: Instant,
}

impl RuntimeFacts {
    pub(crate) fn new(started_at: Instant) -> Self {
        Self { started_at }
    }

    /// The facts as of now, over the model the calling conversation runs
    /// on and the clock reading its call took.
    ///
    /// The reading is an argument rather than a field: a reading kept here
    /// would age with the process exactly like the stale answer this tool
    /// exists to end, so the call brings its own.
    fn facts(&self, model: &str, clock: &ClockReading) -> String {
        fact_lines(model, VERSION, REVISION, self.started_at.elapsed(), clock)
    }
}

impl ToolHandler<CoreEvent> for RuntimeFacts {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "State what this assistant is running right now: the model this \
                 conversation runs on, its software version and build revision, how long \
                 the process has been up, and what today's date and the current time are. \
                 Call it whenever someone asks what you run on, which model or version you \
                 are, how long you have been running, or what the date, the day of the week \
                 or the time is — the answer comes from the running process and its clock, \
                 never from what you remember or from what the conversation said earlier. \
                 It takes no arguments."
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
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            // The conversation record carries the model its turns are
            // dispatched on. Read at the call, over the conversation the
            // call belongs to: the answer must be the binding this turn
            // ran under, and no other conversation's.
            let conversation_id = ctx.agency.conversation_id;
            let running = match ctx.agency.store.find_conversation(conversation_id).await {
                Ok(Some(conversation)) => conversation.model.external_id,
                Ok(None) => {
                    tracing::warn!(
                        conversation_id,
                        "the runtime-facts tool found no conversation to read the model from"
                    );
                    return ToolOutcome::Error(UNREADABLE_RESULT.to_owned());
                }
                Err(error) => {
                    tracing::warn!(
                        conversation_id,
                        %error,
                        "the runtime-facts tool's conversation read failed"
                    );
                    return ToolOutcome::Error(UNREADABLE_RESULT.to_owned());
                }
            };
            // The clock, read here and nowhere else: one reading per
            // call, kept only long enough to render this result.
            ToolOutcome::Done(self.facts(&running, &ClockReading::now_local()))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_ledger::{AgencyCtx, EventBus, Store};

    use super::*;
    use crate::schema::store_config;

    /// A reading with both zone parts answered — what a machine whose
    /// platform names its abbreviation reads.
    fn zoned_reading() -> ClockReading {
        ClockReading {
            date: "2026-08-29".into(),
            weekday: "Saturday".into(),
            tz_abbrev: Some("CEST".into()),
            tz_name: Some("Europe/Berlin".into()),
            time: "14:05".into(),
        }
    }

    /// A reading whose zone sources both answered nothing — the shape
    /// every reading this deployment takes has today, since the
    /// abbreviation has no reachable source and the name resolver can
    /// answer nothing.
    fn zoneless_reading() -> ClockReading {
        ClockReading {
            tz_abbrev: None,
            tz_name: None,
            ..zoned_reading()
        }
    }

    /// The rendering, character for character, over a known tuple —
    /// including the `revision: unknown` form a build that passed none
    /// answers with, a duration whose seconds are dropped, and the two
    /// clock lines with the weekday beside the date and the zone beside
    /// the time.
    #[test]
    fn the_fact_lines_are_byte_exact() {
        assert_eq!(
            fact_lines(
                "vendor/model-9",
                "0.1.0",
                UNKNOWN_REVISION,
                Duration::from_secs(2 * 86_400 + 3 * 3_600 + 4 * 60 + 59),
                &zoned_reading(),
            ),
            "model: vendor/model-9\nversion: 0.1.0\nrevision: unknown\nuptime: 2d 3h 4m\n\
             date: 2026-08-29 (Saturday)\ntime: 14:05 CEST (Europe/Berlin)"
        );
        assert_eq!(
            fact_lines("m", "1.2.3", "abc1234", Duration::ZERO, &zoneless_reading()),
            "model: m\nversion: 1.2.3\nrevision: abc1234\nuptime: 0d 0h 0m\n\
             date: 2026-08-29 (Saturday)\ntime: 14:05"
        );
    }

    /// Every zone shape a reading can carry, rendered: both parts, either
    /// part alone, and neither — the last one leaving the time line ending
    /// at the minute, with no empty bracket, no comma and no zone worked
    /// out from the date.
    #[test]
    fn absent_zone_parts_render_absent() {
        let both = zoned_reading();
        assert_eq!(zone_clause(&both), " CEST (Europe/Berlin)");
        assert_eq!(
            zone_clause(&ClockReading {
                tz_name: None,
                ..both.clone()
            }),
            " CEST"
        );
        assert_eq!(
            zone_clause(&ClockReading {
                tz_abbrev: None,
                ..both
            }),
            " Europe/Berlin"
        );
        assert_eq!(zone_clause(&zoneless_reading()), "");

        let rendered = fact_lines("m", "1.2.3", "abc1234", Duration::ZERO, &zoneless_reading());
        assert!(
            rendered.ends_with("time: 14:05"),
            "a NULL zone leaves the time line whole and bare: {rendered}"
        );
        assert!(
            !rendered.contains("()") && !rendered.contains("None"),
            "nothing is written where a source answered nothing: {rendered}"
        );
    }

    /// The clock lines follow the reading they were handed, part by part:
    /// two readings differing in one part render differently, so no part
    /// of them is fixed at build time or carried over from another call.
    #[test]
    fn the_clock_lines_are_the_readings_own_parts() {
        let facts =
            |clock: &ClockReading| fact_lines("m", "1.2.3", "abc1234", Duration::ZERO, clock);
        let noon = zoned_reading();
        for other in [
            ClockReading {
                time: "14:06".into(),
                ..noon.clone()
            },
            ClockReading {
                date: "2026-08-30".into(),
                weekday: "Sunday".into(),
                ..noon.clone()
            },
            ClockReading {
                tz_name: Some("Europe/Lisbon".into()),
                ..noon.clone()
            },
        ] {
            assert_ne!(
                facts(&noon),
                facts(&other),
                "a reading's parts reach the lines: {other:?}"
            );
        }
    }

    /// The description says which questions the tool answers, the date and
    /// the time among them: a model that reads it knows to call the tool
    /// when someone asks the time instead of answering from a day marker
    /// or from memory.
    #[test]
    fn the_definition_names_the_clock_among_the_facts() {
        let definition = RuntimeFacts::new(Instant::now()).definition();
        assert_eq!(definition.name, NAME);
        for fact in [
            "the model this conversation runs on",
            "software version and build revision",
            "how long the process has been up",
            "what today's date and the current time are",
            "what the date, the day of the week or the time is",
            "never from what you remember",
        ] {
            assert!(
                definition.description.contains(fact),
                "the description carries: {fact}"
            );
        }
        assert_eq!(definition.parameters["properties"], json!({}));
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
        let tool = RuntimeFacts::new(started_at);
        let clock = zoned_reading();
        assert!(
            tool.facts("vendor/model-9", &clock)
                .contains("\nuptime: 1d 2h 5m\n"),
            "the stored anchor is what uptime measures: {}",
            tool.facts("vendor/model-9", &clock)
        );
        assert_eq!(
            tool.facts("vendor/model-9", &clock),
            tool.facts("vendor/model-9", &clock),
            "two calls, one anchor"
        );
    }

    /// The tool holds no clock: its whole state is the monotonic anchor,
    /// so there is nowhere for a reading to be cached between calls, and
    /// the reading each call renders is the one that call took.
    ///
    /// A cached reading is pinned by the mechanism rather than by waiting
    /// for a minute to tick: a test that slept a minute to watch the
    /// answer move would prove the same thing a minute slower, and one
    /// that slept less would prove nothing at all. What is asserted here
    /// is that nothing survives a call to be reused by the next — so two
    /// calls in one process answer whatever the clock says at each of
    /// them, differing exactly when the clock moved between them.
    #[test]
    fn the_tool_caches_no_reading_between_calls() {
        assert_eq!(
            std::mem::size_of::<RuntimeFacts>(),
            std::mem::size_of::<Instant>(),
            "the tool's state is the anchor and nothing else"
        );
    }

    /// One call of the tool over a given conversation id, with a block id
    /// naming nothing: the conversation record is the only thing the
    /// execute path reads.
    async fn call(tool: &RuntimeFacts, agency: &AgencyCtx<CoreEvent>, input: &str) -> ToolOutcome {
        tool.execute(
            input,
            ToolContext {
                agency,
                tool_call_id: "call-0",
                block_id: 12345,
            },
        )
        .await
    }

    /// An agency over a store holding one conversation created on the
    /// given model — the record the execute path reads the model from.
    async fn agency_over(model: &str) -> AgencyCtx<CoreEvent> {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let conversation_id = store
            .create_conversation(
                "provider-1".into(),
                model.into(),
                "Model Nine".into(),
                "vendor".into(),
            )
            .await
            .expect("a conversation row");
        AgencyCtx {
            conversation_id,
            store,
            bus: Arc::new(EventBus::new()),
        }
    }

    /// Every input — an empty one, an object with strange fields,
    /// unparsable text — answers the same facts, over the model the
    /// calling conversation was created on. Nothing is selected, and the
    /// ledger's blocks are never read.
    #[tokio::test]
    async fn every_input_answers_the_calling_conversations_model() {
        let agency = agency_over("vendor/model-9").await;
        let tool = RuntimeFacts::new(Instant::now());
        for input in ["", "{}", r#"{"fact":"model"}"#, "not json"] {
            match call(&tool, &agency, input).await {
                ToolOutcome::Done(facts) => assert!(
                    facts.starts_with("model: vendor/model-9\n"),
                    "the input {input:?} answers the conversation's own facts: {facts}"
                ),
                ToolOutcome::Error(_) | ToolOutcome::Pending => {
                    panic!("the input {input:?} answers the facts")
                }
            }
        }
    }

    /// Every call renders the clock as of that call: the result of each of
    /// two calls in one process is the rendering of a reading taken
    /// between the readings bracketing that call, so nothing older than
    /// the call reaches the lines.
    #[tokio::test]
    async fn each_call_states_the_clock_as_of_that_call() {
        let agency = agency_over("vendor/model-9").await;
        let tool = RuntimeFacts::new(Instant::now());
        let rendered = |clock: &ClockReading| {
            fact_lines("vendor/model-9", VERSION, REVISION, Duration::ZERO, clock)
        };
        for _ in 0..2 {
            let before = ClockReading::now_local();
            let answered = call(&tool, &agency, "{}").await;
            let after = ClockReading::now_local();
            let ToolOutcome::Done(facts) = answered else {
                panic!("the call states the facts")
            };
            assert!(
                facts == rendered(&before) || facts == rendered(&after),
                "the clock lines belong to this call, bracketed by {before:?} and {after:?}: {facts}"
            );
        }
    }

    /// A call over a conversation the store does not hold answers the
    /// decline, whole: no fact list missing its model line, and no other
    /// model id put in its place.
    #[tokio::test]
    async fn an_unreadable_conversation_answers_the_decline() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let agency: AgencyCtx<CoreEvent> = AgencyCtx {
            conversation_id: 404,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let tool = RuntimeFacts::new(Instant::now());
        match call(&tool, &agency, "{}").await {
            ToolOutcome::Error(decline) => assert_eq!(decline, UNREADABLE_RESULT),
            ToolOutcome::Done(facts) => {
                panic!("a conversation nobody holds states no facts: {facts}")
            }
            ToolOutcome::Pending => panic!("the tool resolves its own call"),
        }
    }
}
