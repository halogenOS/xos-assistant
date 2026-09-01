//! The runtime-facts tool (unit 32, 2026-08-28): what this process is
//! actually running, answered from values the process holds.
//!
//! Asked which model she runs on, a model answers from its training data
//! and from whatever the chat said earlier — both wrong the moment the
//! deployment's configured model changes. The facts here come from the
//! running system instead: the model this conversation's turns are
//! dispatched on, the version compiled into the binary, the revision the
//! build passed in, the time elapsed since the anchor the binary captured
//! at startup, the date and time the machine's own clock says (unit 34,
//! 2026-08-29), and — since unit 37, 2026-08-30 — the distribution the
//! host states, the architecture the binary was built for, and the public
//! homes of the software.
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
//! The distribution is read from the host's `os-release` file at execute
//! time, so a host rebuilt onto a new release answers as it is now instead
//! of as it was when the process started. Every way that read can fail —
//! an absent file, an unreadable one, a text naming neither key —
//! collapses to nothing, and nothing renders the literal `unknown`: a
//! distribution worked out from anything else on the host would be the
//! invention this tool exists to end. The architecture is the binary's
//! own, resolved by the compiler; a 32-bit build on a 64-bit host is a
//! 32-bit assistant, and stating the host's width there would answer a
//! question nobody asked. The two homes are fixed literals of the
//! software — they vary on no build — and they share one row, because
//! where the software lives is one fact.
//!
//! No parameters, and no input is read: there is nothing to select among
//! these lines, so extra arguments change nothing. Beyond the read of the
//! conversation's own record, the read of the clock and the read of one
//! named host file — the `os-release` file, and no other — the execute
//! path reads process-held values only: no network call, no subprocess,
//! nothing that can hang or leak, which is why the result cannot be stale
//! in the way a remembered answer is.
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

/// What a fact reads when the value behind it is not there: a stated
/// `unknown` beats a value invented from anything else. The revision
/// reads it when the build passed none, and the distribution reads it
/// when the host file answered nothing. One spelling, so the two cannot
/// drift into two different admissions of the same thing.
pub const UNKNOWN_FACT: &str = "unknown";

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

/// The architecture this binary was built for, resolved by the compiler.
/// The software's own architecture is the one honest answer a process can
/// give about itself: a 32-bit build served by a 64-bit host is a 32-bit
/// assistant, whatever the host would answer for itself.
pub const ARCH: &str = std::env::consts::ARCH;

/// The assistant's public repository — where this software lives.
pub const ASSISTANT_HOME: &str = "https://github.com/halogenOS/xos-assistant";

/// The framework's public repository — where the ledger this assistant is
/// built on lives.
pub const FRAMEWORK_HOME: &str = "https://github.com/xdevs23/ronna-core";

/// The file a host states its distribution in. The one file the execute
/// path reads; every other fact it states is a process-held value.
const OS_RELEASE_PATH: &str = "/etc/os-release";

/// The one place an absent build revision becomes the literal `unknown`.
const fn resolve_revision(passed: Option<&'static str>) -> &'static str {
    match passed {
        Some(revision) => revision,
        None => UNKNOWN_FACT,
    }
}

/// The distribution the host states, or nothing.
///
/// The read and the parse are one surface, so no caller can hold half of
/// the fact: the named file is read here, at the call, and its text goes
/// straight into the parse below. Execute-time, because a host rebuilt
/// onto a new release must be answered for as it is now.
///
/// Every failure collapses to nothing — no file, no permission, no key
/// the parse recognises — and the row renders [`UNKNOWN_FACT`] for it.
/// There is no half of this fact to state and nothing else on the host
/// honest enough to answer from.
#[must_use]
pub fn host_distribution() -> Option<String> {
    distribution_stated_in(&std::fs::read_to_string(OS_RELEASE_PATH).ok()?)
}

/// The distribution named by os-release text: `PRETTY_NAME` when it
/// states one, else `NAME`. Public so the parse can be exercised over
/// injected shapes without a host file, while [`host_distribution`] stays
/// the one surface the execute path calls.
///
/// A line is split at its FIRST `=`; the value is trimmed of ASCII
/// whitespace, then one matching pair of surrounding double or single
/// quotes is stripped. Escape sequences pass through as stored — no
/// distribution puts them in these two keys, and a decoder here would be
/// machinery for bytes that never arrive. A key stated twice answers from
/// its first line. A key whose value is empty answers nothing and yields:
/// an empty `PRETTY_NAME` falls through to a usable `NAME`, and both
/// empty is nothing at all.
#[must_use]
pub fn distribution_stated_in(os_release: &str) -> Option<String> {
    ["PRETTY_NAME", "NAME"]
        .into_iter()
        .find_map(|key| stated_value(os_release, key))
}

/// The value the text states for one key, read from that key's first line
/// and answered only when something is left of it.
fn stated_value(os_release: &str, key: &str) -> Option<String> {
    let stated = os_release
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(named, value)| (named == key).then_some(value))?;
    let value = unquoted(stated.trim_ascii());
    (!value.is_empty()).then(|| value.to_owned())
}

/// What one matching pair of surrounding quotes encloses, or the text as
/// it stands when it carries no such pair.
fn unquoted(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// The fact list the model reads, one fact per line. A new fact joins as
/// another row here — the shape takes one without restructuring.
///
/// Two rows render `clock`: the date with the weekday it falls on, and
/// the time with the zone the reading answers for. Nothing here parses or
/// recomputes any part of the reading — the parts are stated as they were
/// read.
///
/// The last three rows state where the software runs and where it comes
/// from: the distribution as the host stated it, or [`UNKNOWN_FACT`] when
/// the read answered nothing; the architecture the binary was built for;
/// and the software's two public homes on one row, the assistant's first.
/// Whatever varies from build to build or from call to call arrives as an
/// argument, so nothing here can go stale behind a caller's back; the two
/// homes vary on no build, so they are written here beside the row labels
/// that never vary either.
#[must_use]
pub fn fact_lines(
    model: &str,
    version: &str,
    revision: &str,
    uptime: Duration,
    clock: &ClockReading,
    distribution: Option<&str>,
    arch: &str,
) -> String {
    format!(
        "model: {model}\n\
         version: {version}\n\
         revision: {revision}\n\
         uptime: {uptime}\n\
         date: {date} ({weekday})\n\
         time: {time}{zone}\n\
         os: {os}\n\
         arch: {arch}\n\
         source: {ASSISTANT_HOME}, {FRAMEWORK_HOME}",
        uptime = coarse_uptime(uptime),
        date = clock.date,
        weekday = clock.weekday,
        time = clock.time,
        zone = zone_clause(clock),
        os = distribution.unwrap_or(UNKNOWN_FACT)
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
    /// The reading is an argument and not a field: a reading kept here
    /// would age with the process exactly like the stale answer this tool
    /// exists to end, so the call brings its own. The distribution is
    /// read here, at the call, for the same reason.
    fn facts(&self, model: &str, clock: &ClockReading) -> String {
        fact_lines(
            model,
            VERSION,
            REVISION,
            self.started_at.elapsed(),
            clock,
            host_distribution().as_deref(),
            ARCH,
        )
    }
}

impl ToolHandler<CoreEvent> for RuntimeFacts {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "State what this assistant is running right now: the model this \
                 conversation runs on, its software version and build revision, how long \
                 the process has been up, what today's date and the current time are, the \
                 operating system and processor architecture it runs on, and the public \
                 repositories its own code and the framework under it live in. \
                 Call it whenever someone asks what you run on, which model or version you \
                 are, how long you have been running, what the date, the day of the week \
                 or the time is, which operating system or architecture you run on, what \
                 you are built on, or where your source code can be read — the answer \
                 comes from the running process, its clock and its host, \
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

    crate::tools::admission::admits_at_required_authority!(NAME, REQUIRED_AUTHORITY);

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
    /// answers with, a duration whose seconds are dropped, the two clock
    /// lines with the weekday beside the date and the zone beside the
    /// time, and the three rows stating where the software runs and where
    /// it comes from. The homes appear here as the literal text the model
    /// reads, so an edited constant fails this pin too.
    #[test]
    fn the_fact_lines_are_byte_exact() {
        assert_eq!(
            fact_lines(
                "vendor/model-9",
                "0.1.0",
                UNKNOWN_FACT,
                Duration::from_secs(2 * 86_400 + 3 * 3_600 + 4 * 60 + 59),
                &zoned_reading(),
                Some("Distro Linux 42 (Fortytwo)"),
                "x86_64",
            ),
            "model: vendor/model-9\nversion: 0.1.0\nrevision: unknown\nuptime: 2d 3h 4m\n\
             date: 2026-08-29 (Saturday)\ntime: 14:05 CEST (Europe/Berlin)\n\
             os: Distro Linux 42 (Fortytwo)\narch: x86_64\n\
             source: https://github.com/halogenOS/xos-assistant, \
             https://github.com/xdevs23/ronna-core"
        );
        assert_eq!(
            fact_lines(
                "m",
                "1.2.3",
                "abc1234",
                Duration::ZERO,
                &zoneless_reading(),
                Some("Distro Linux 42 (Fortytwo)"),
                "aarch64",
            ),
            "model: m\nversion: 1.2.3\nrevision: abc1234\nuptime: 0d 0h 0m\n\
             date: 2026-08-29 (Saturday)\ntime: 14:05\n\
             os: Distro Linux 42 (Fortytwo)\narch: aarch64\n\
             source: https://github.com/halogenOS/xos-assistant, \
             https://github.com/xdevs23/ronna-core"
        );
    }

    /// The homes the source row states, pinned where they are defined: an
    /// accidental edit fails loudly instead of quietly sending a member to
    /// a repository that is not this software's.
    #[test]
    fn the_homes_name_the_real_repositories() {
        assert_eq!(ASSISTANT_HOME, "https://github.com/halogenOS/xos-assistant");
        assert_eq!(FRAMEWORK_HOME, "https://github.com/xdevs23/ronna-core");
        assert_eq!(ARCH, std::env::consts::ARCH);
    }

    /// A distribution the reader could not answer for renders the literal
    /// `unknown`, in one row and with the rows after it untouched: no
    /// blank value, no invented name, and no row silently dropped, which
    /// would leave the list one shorter than the model was taught to
    /// expect.
    #[test]
    fn an_unanswered_distribution_renders_unknown() {
        let rendered = fact_lines(
            "m",
            "1.2.3",
            "abc1234",
            Duration::ZERO,
            &zoneless_reading(),
            None,
            "x86_64",
        );
        assert!(
            rendered.contains("\nos: unknown\narch: x86_64\n"),
            "an unread distribution states the honest literal: {rendered}"
        );
        assert!(
            rendered.ends_with(
                "source: https://github.com/halogenOS/xos-assistant, \
                 https://github.com/xdevs23/ronna-core"
            ),
            "the rows after it are whole: {rendered}"
        );
    }

    /// The parse, over the shapes an os-release text can take: the pretty
    /// name preferred, the plain name taken when the pretty one is missing
    /// or empty, quotes of either kind stripped once, a value split at its
    /// first `=` and kept whole after it, escapes passed through as
    /// stored, the first line of a repeated key winning, and every text
    /// naming neither key answering nothing.
    #[test]
    fn the_parse_takes_the_pretty_name_and_strips_one_quote_pair() {
        let cases = [
            (
                "NAME=Basic\nPRETTY_NAME=\"Basic Linux 9\"\n",
                Some("Basic Linux 9"),
            ),
            ("NAME='Quoted'\n", Some("Quoted")),
            ("NAME=Bare\n", Some("Bare")),
            ("PRETTY_NAME=\"\"\nNAME=\"Fallback\"\n", Some("Fallback")),
            ("PRETTY_NAME=\nNAME=Fallback\n", Some("Fallback")),
            ("PRETTY_NAME=\"  \"\nNAME=Fallback\n", Some("  ")),
            ("PRETTY_NAME=\"\"\nNAME=\"\"\n", None),
            ("NAME=First\nNAME=Second\n", Some("First")),
            ("VERSION_ID=42\nID=basic\n", None),
            ("", None),
            ("PRETTY_NAME=A=B\n", Some("A=B")),
            ("PRETTY_NAME=  spaced  \n", Some("spaced")),
            (
                "PRETTY_NAME=\"Escaped \\\"quote\\\"\"\n",
                Some("Escaped \\\"quote\\\""),
            ),
            ("PRETTY_NAME=\"'Nested'\"\n", Some("'Nested'")),
            ("PRETTY_NAME=\"\n", Some("\"")),
            ("#PRETTY_NAME=Commented\nNAME=Real\n", Some("Real")),
        ];
        for (text, expected) in cases {
            assert_eq!(
                distribution_stated_in(text).as_deref(),
                expected,
                "the text {text:?} states {expected:?}"
            );
        }
    }

    /// The reader answers a stated distribution or nothing, on whatever
    /// host runs this suite: a host carrying the file answers something
    /// non-empty, and a host without one answers nothing, which is the
    /// missing-file collapse itself. Neither outcome panics, because a
    /// fact list must not be lost to a file that is not there.
    #[test]
    fn the_reader_answers_a_stated_distribution_or_nothing() {
        if let Some(distribution) = host_distribution() {
            assert!(
                !distribution.is_empty(),
                "an answered distribution is never empty: {distribution:?}"
            );
        }
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

        let rendered = fact_lines(
            "m",
            "1.2.3",
            "abc1234",
            Duration::ZERO,
            &zoneless_reading(),
            Some("Distro Linux 42 (Fortytwo)"),
            "x86_64",
        );
        assert!(
            rendered.contains("time: 14:05\n"),
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
        let facts = |clock: &ClockReading| {
            fact_lines(
                "m",
                "1.2.3",
                "abc1234",
                Duration::ZERO,
                clock,
                Some("Distro Linux 42 (Fortytwo)"),
                "x86_64",
            )
        };
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

    /// The description says which questions the tool answers — the date
    /// and the time among them, and since unit 37 the operating system,
    /// the architecture and where the source can be read: a model that
    /// reads it knows to call the tool instead of answering from a day
    /// marker or from memory.
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
            "the operating system and processor architecture it runs on",
            "the public repositories its own code and the framework under it live in",
            "which operating system or architecture you run on",
            "what you are built on",
            "where your source code can be read",
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
                ToolOutcome::Error(_) | ToolOutcome::Pending | ToolOutcome::Refused(_) => {
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
        // The distribution comes from the same production reader the
        // execute path calls, so this expectation is self-consistent on
        // any host — one whose file names a release and one whose file
        // is not there at all.
        let distribution = host_distribution();
        let rendered = |clock: &ClockReading| {
            fact_lines(
                "vendor/model-9",
                VERSION,
                REVISION,
                Duration::ZERO,
                clock,
                distribution.as_deref(),
                ARCH,
            )
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
            ToolOutcome::Pending | ToolOutcome::Refused(_) => {
                panic!("the tool resolves its own call")
            }
        }
    }
}
