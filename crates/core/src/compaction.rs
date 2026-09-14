//! When a session is compacted, and what the harness asks the model for
//! when it is.
//!
//! The mechanism itself — the fork of the first half, the captured summary,
//! the thread the summary opens — lives in [`crate::session`] over the
//! framework's storage primitives. This module answers the two questions
//! that are policy, and holds the one text that is copy:
//!
//! - **Whether** a conversation needs compacting: two threshold arms over
//!   the context window, read from the last turn's reported usage.
//! - **When** to act on an armed threshold: at a quiet moment, but never
//!   knowingly into an expired prompt cache.
//! - **What** the harness asks for: the compaction instructions, byte-pinned.
//!
//! Both policy questions are pure functions over injected numbers and
//! injected clock readings, so what they decide is pinned without a clock,
//! a provider or a store anywhere near them. [`ContextWatch`] is the one
//! home of the readings they take.
//!
//! # The thresholds
//!
//! Two arms, and the second is not the first restated:
//!
//! - **Headroom.** Once the context window has no more than
//!   [`HEADROOM_TOKENS`] left, the conversation is close to the wall and
//!   compacting is overdue whatever else is true.
//! - **A cold cache over a half-full window.** Once the prompt cache has
//!   expired AND more than half the window is in use, the next turn pays
//!   full price for the whole prefix anyway — so paying it once, for a
//!   compaction that shrinks the prefix, is the cheap move.
//!
//! Both arms read the same two numbers, and with either unknown BOTH stay
//! silent: the trigger never fires blind. A turn whose provider reported no
//! usage leaves the last known number standing; with none ever known, and
//! with no configured window size, nothing arms. That absent-data behavior
//! is a stated choice, not an oversight — the other two doors into the
//! mechanism are unaffected by it.
//!
//! # The timing
//!
//! An armed threshold does not act immediately. The rule is ONE rule
//! whichever arm armed it, and it tracks the CURRENT cache state rather than
//! the state that armed the trigger:
//!
//! - A quiet moment — no inbound message for [`QUIET_WINDOW`] — is always
//!   the moment to go.
//! - While the cache is WARM and quiet has not come, the cache's edge is the
//!   deadline: at [`CACHE_EDGE_MARGIN`] before it the compaction goes
//!   anyway, so its own dispatch still lands on the warm prefix instead of
//!   paying for a cold one.
//! - While the cache is already expired and nothing has re-warmed it, there
//!   is no warm window left to protect, so waiting for quiet costs nothing
//!   and the rule waits.
//!
//! Every dispatch re-warms the cache and restarts the rule from the new
//! edge, so an armed trigger under continuing traffic never knowingly
//! dispatches full-price into an expired cache.
//!
//! # What is an estimate, and what is not
//!
//! [`KV_CACHE_TTL`] is an ESTIMATE of an external fact: how long a provider
//! holds a conversation's prefix in its prompt cache. No provider reports
//! it, and time since the last dispatch is the only locally measurable
//! reading of "the cache has expired". Everything else here is exact.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::Instant;

use crate::streams::StreamObserver;

/// What the harness asks the model for, and the whole of it.
///
/// Written from the design's own words — compact the first half of the
/// conversation, mentioning important things, conversational topics etc —
/// and byte-pinned by the test below, so a later edit is a deliberate act
/// with the pin moved in the same change.
///
/// The answer is the JSON object [`compaction_schema`] describes, one list
/// per unit of the summary, and the provider holds the model to that shape;
/// the instructions name the lists once so the model knows what each is
/// for, and the prose rules apply to every item.
///
/// It is model-facing harness text, never a line anyone in the chat reads:
/// the temporary conversation it is appended to is retired the moment its
/// answer is captured.
pub(crate) const COMPACTION_INSTRUCTIONS: &str = "\
You are compacting the conversation above so it can be carried forward in a \
shorter form. Everything above this message is the first half of a longer \
conversation; the rest of it continues after your summary.

Write that summary. Mention the important things: what people asked and what \
was answered, decisions and conclusions reached, facts established about \
people, versions, settings and links, corrections made, and anything left \
open or unfinished. Mention the conversational topics that came up, in the \
order they came up, so a reader can tell what this conversation has already \
been about.

Answer with a JSON object holding one list per part of that summary: topics, \
in the order they came up; questions_answered, each with the question and the \
answer it got; decisions; facts; corrections; open_items. Leave a list empty \
when the conversation holds nothing for it. Write each item as plain prose \
for the assistant that continues this conversation. Do not greet anyone, do \
not address anyone, do not describe what you are about to do, and do not \
offer to help.";

/// The answer object's fields, one per logical unit of the summary, as the
/// schema names them.
const TOPICS: &str = "topics";
const QUESTIONS_ANSWERED: &str = "questions_answered";
const DECISIONS: &str = "decisions";
const FACTS: &str = "facts";
const CORRECTIONS: &str = "corrections";
const OPEN_ITEMS: &str = "open_items";

/// The two fields of one asked-and-answered item.
const QUESTION: &str = "question";
const ANSWER: &str = "answer";

/// The headings the units render under, in the order they render.
const TOPICS_HEADING: &str = "Topics, in the order they came up:";
const QUESTIONS_HEADING: &str = "Asked and answered:";
const DECISIONS_HEADING: &str = "Decisions and conclusions reached:";
const FACTS_HEADING: &str = "Facts established about people, versions, settings and links:";
const CORRECTIONS_HEADING: &str = "Corrections made:";
const OPEN_ITEMS_HEADING: &str = "Left open or unfinished:";

/// The shape the compaction's answer must take: one object with one list
/// per logical unit of the summary, every field present, nothing else. The
/// provider enforces it; this is the document it is handed. The reading of
/// what comes back is [`CompactionSummary`], whose fields are these names.
pub(crate) fn compaction_schema() -> Value {
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "properties": {
            TOPICS: strings,
            QUESTIONS_ANSWERED: {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        QUESTION: { "type": "string" },
                        ANSWER: { "type": "string" },
                    },
                    "required": [QUESTION, ANSWER],
                    "additionalProperties": false,
                },
            },
            DECISIONS: strings,
            FACTS: strings,
            CORRECTIONS: strings,
            OPEN_ITEMS: strings,
        },
        "required": [TOPICS, QUESTIONS_ANSWERED, DECISIONS, FACTS, CORRECTIONS, OPEN_ITEMS],
        "additionalProperties": false,
    })
}

/// The answer object as the schema describes it: one list per logical unit
/// of the summary. Deserializing is the one reading of what the provider
/// returned, and a field the schema does not name is refused there, at
/// either level; nothing here validates a document against the schema,
/// which the provider enforced.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompactionSummary {
    topics: Vec<String>,
    questions_answered: Vec<AnsweredQuestion>,
    decisions: Vec<String>,
    facts: Vec<String>,
    corrections: Vec<String>,
    open_items: Vec<String>,
}

/// One question and the answer it got.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsweredQuestion {
    question: String,
    answer: String,
}

impl CompactionSummary {
    /// The compaction message: each unit that holds anything, under its
    /// heading as a list, in the units' order; units with nothing omitted.
    /// An answer with nothing in any unit is no summary, and says so.
    ///
    /// The rendering is harness text the model reads at the head of the
    /// compacted thread, never a line anyone in the chat sees. The reason
    /// an answer is refused names no content of it: the log line it reaches
    /// is not where a conversation's facts belong.
    pub(crate) fn render(&self) -> Result<String, String> {
        let sections = [
            section(
                TOPICS_HEADING,
                self.topics.iter().map(String::as_str).map(item),
            ),
            section(
                QUESTIONS_HEADING,
                self.questions_answered.iter().map(AnsweredQuestion::item),
            ),
            section(
                DECISIONS_HEADING,
                self.decisions.iter().map(String::as_str).map(item),
            ),
            section(
                FACTS_HEADING,
                self.facts.iter().map(String::as_str).map(item),
            ),
            section(
                CORRECTIONS_HEADING,
                self.corrections.iter().map(String::as_str).map(item),
            ),
            section(
                OPEN_ITEMS_HEADING,
                self.open_items.iter().map(String::as_str).map(item),
            ),
        ];
        let rendered: Vec<String> = sections.into_iter().flatten().collect();
        if rendered.is_empty() {
            return Err("every unit of the summary is empty".to_owned());
        }
        Ok(rendered.join("\n\n"))
    }
}

impl AnsweredQuestion {
    /// The pair as one list item, or `None` when both sides are blank. A
    /// pair with one blank side still renders, the blank side as its label
    /// alone: a question the model recorded without its answer is a fact
    /// about the conversation, not a defect of the answer.
    fn item(&self) -> Option<String> {
        let (question, answer) = (contained(&self.question), contained(&self.answer));
        if question.is_empty() && answer.is_empty() {
            return None;
        }
        Some(
            format!("- Q: {question}\n  A: {answer}")
                .trim_end()
                .to_owned(),
        )
    }
}

/// One rendered section, or `None` when no item of it carries anything.
fn section(heading: &str, items: impl Iterator<Item = Option<String>>) -> Option<String> {
    let lines: Vec<String> = items.flatten().collect();
    (!lines.is_empty()).then(|| format!("{heading}\n{}", lines.join("\n")))
}

/// One list item, or `None` when it is blank.
fn item(text: &str) -> Option<String> {
    let text = contained(text);
    (!text.is_empty()).then(|| format!("- {text}"))
}

/// A text kept inside its list item: lines trimmed, blank lines dropped,
/// every line after the first indented under the item's marker. A line
/// break the model put inside an item therefore cannot open a section or
/// an item of its own, whatever the line says.
fn contained(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// The headroom arm's floor: once the context window has no more than this
/// many tokens left, the conversation is compacted. The design's own
/// number.
pub(crate) const HEADROOM_TOKENS: u32 = 50_000;

/// How long a provider's prompt cache is assumed to hold a conversation's
/// prefix after a dispatch.
///
/// An ESTIMATE of an external fact, and named as one: no provider reports
/// its cache lifetime, and time since the last dispatch is the only reading
/// of expiry this process can take. Five minutes is the shortest lifetime
/// the vendors this deployment reaches publish, so the estimate errs toward
/// treating a cache as cold — which costs a compaction that could have
/// waited, never a full-price dispatch nobody expected.
pub(crate) const KV_CACHE_TTL: Duration = Duration::from_mins(5);

/// How long a channel must go without an inbound message to count as quiet.
/// A compaction takes a model turn of its own, and a quiet channel is where
/// that turn interrupts nobody.
pub(crate) const QUIET_WINDOW: Duration = Duration::from_mins(3);

/// How far ahead of the cache's edge an armed compaction stops waiting for
/// quiet. The compaction's own dispatch has to reach the provider before
/// the prefix falls out of its cache, and the margin is what the round trip
/// gets.
pub(crate) const CACHE_EDGE_MARGIN: Duration = Duration::from_secs(30);

/// How often the driver re-reads the thresholds and the timing. Both are
/// time-based readings, so nothing on the bus announces the moment they
/// change: the sweep is what notices.
pub(crate) const CONTEXT_SWEEP: Duration = Duration::from_secs(30);

/// The most conversations the readings are held for. Past the cap the whole
/// map is cleared — the established memory-cap shape: losing a reading
/// costs one delayed compaction, which the conversation's next turn
/// restores, while an unbounded map would grow with every channel this
/// process ever served.
const READING_CAP: usize = 4096;

/// What the two threshold arms read about one conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContextReading {
    /// The model's context window, from the configured binding. Absent
    /// means the deployment did not state one.
    pub window_tokens: Option<NonZeroU32>,
    /// What the conversation's last reported turn occupied of it — the
    /// request's input plus the response's output. Absent means no turn has
    /// ever reported usage for this conversation.
    ///
    /// Reasoning tokens are deliberately NOT added on top: where a provider
    /// reports them separately they are part of the output count, and
    /// adding them would count the same spend twice.
    pub used_tokens: Option<u32>,
    /// How long since this conversation's last dispatch, or absent when it
    /// has never dispatched under this process.
    pub since_dispatch: Option<Duration>,
}

/// What the timing rule reads about one conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimingReading {
    /// How long since the last inbound message on this conversation's
    /// channel, or absent when none has arrived under this process — which
    /// reads as quiet, because nothing is going on.
    pub since_inbound: Option<Duration>,
    /// How long since the last dispatch, or absent when there has been
    /// none — which reads as a cache that was never warmed.
    pub since_dispatch: Option<Duration>,
}

/// Whether either threshold arm holds.
///
/// The arms are stated separately and read the same two numbers; with
/// either number unknown, neither fires.
pub(crate) fn threshold_armed(reading: ContextReading, cache_ttl: Duration) -> bool {
    let (Some(window), Some(used)) = (reading.window_tokens, reading.used_tokens) else {
        return false;
    };
    let window = u64::from(window.get());
    let used = u64::from(used);
    // "once only 50k context is left" — saturating, because a turn that
    // overran the configured window leaves nothing, not a negative amount.
    if window.saturating_sub(used) <= u64::from(HEADROOM_TOKENS) {
        return true;
    }
    // "the KV cache has expired and more than 50% of the context window is
    // used" — strictly more than half, and doubling instead of halving so
    // an odd window has no rounding to argue about.
    let expired = reading
        .since_dispatch
        .is_some_and(|since| since >= cache_ttl);
    expired && used * 2 > window
}

/// Whether an armed compaction goes now or waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moment {
    /// Go: the channel is quiet, or the warm cache's edge is close enough
    /// that waiting for quiet would cost the prefix.
    Now,
    /// Wait: the sweep asks again.
    Wait,
}

/// When an armed compaction acts. The rule is the module's own; it is one
/// rule whichever arm armed the trigger, and it reads the CURRENT cache
/// state rather than the state that armed it.
pub(crate) fn moment(reading: TimingReading, cache_ttl: Duration) -> Moment {
    let quiet = reading
        .since_inbound
        .is_none_or(|since| since >= QUIET_WINDOW);
    if quiet {
        return Moment::Now;
    }
    match reading.since_dispatch {
        // The cache is warm and the channel is busy: the edge is the
        // deadline, and the margin is what the dispatch's own round trip
        // needs to reach the provider while the prefix is still cached.
        Some(since) if since < cache_ttl => {
            if since >= cache_ttl.saturating_sub(CACHE_EDGE_MARGIN) {
                Moment::Now
            } else {
                Moment::Wait
            }
        }
        // Expired already, or never warmed: there is no warm window left to
        // protect, so the next quiet moment suffices and the wait is free.
        _ => Moment::Wait,
    }
}

/// The one home of the readings the two policy questions take: the stream
/// facts the observer holds, the inbound activity the ingestion records,
/// and the configured window size.
///
/// It is a reader, not a decider — [`threshold_armed`] and [`moment`] decide
/// — and it is the only place those two are called from, so no second site
/// can assemble a reading of its own.
pub(crate) struct ContextWatch {
    /// The stream facts: what the last turn used, and when the last
    /// dispatch was. Shared with the erasure ordering, which reads its own
    /// half.
    streams: Arc<StreamObserver>,
    /// When each conversation last took an inbound message. In-memory: the
    /// quiet window is about live traffic, and a restart that forgets it
    /// costs at most one early compaction.
    inbound: Mutex<HashMap<i64, Instant>>,
    /// The model's context window, as the deployment configured it. Absent
    /// keeps both threshold arms silent.
    window_tokens: Option<NonZeroU32>,
}

impl ContextWatch {
    /// Whether the thresholds are readable at all: with no configured
    /// window size BOTH arms are permanently silent, so nothing sweeps for
    /// them and no periodic timer exists in a process that stated none.
    pub(crate) fn sweeps(&self) -> bool {
        self.window_tokens.is_some()
    }

    /// The watch over one observer and one configured window.
    pub(crate) fn new(streams: Arc<StreamObserver>, window_tokens: Option<NonZeroU32>) -> Self {
        Self {
            streams,
            inbound: Mutex::new(HashMap::new()),
            window_tokens,
        }
    }

    /// Record that a message just arrived on this conversation — the quiet
    /// window's one writer.
    pub(crate) fn record_inbound(&self, conversation_id: i64) {
        let mut inbound = self.lock();
        if inbound.len() >= READING_CAP {
            tracing::debug!("the inbound activity memory reached its cap and was cleared");
            inbound.clear();
        }
        inbound.insert(conversation_id, Instant::now());
    }

    /// The stream observation itself, for the one caller that needs more
    /// than a reading: a session replacement settles a conversation's open
    /// stream before it copies that conversation's history onto a successor
    /// and unmaps it. The watch already holds the observer, so handing it on
    /// here keeps the single home the module doc claims rather than passing a
    /// second handle around beside it.
    pub(crate) fn streams(&self) -> &Arc<StreamObserver> {
        &self.streams
    }

    /// Drop a conversation's readings. The store reissues conversation ids,
    /// so a retired or deleted conversation's entries must not shadow the
    /// id's next holder.
    pub(crate) fn forget(&self, conversation_id: i64) {
        self.lock().remove(&conversation_id);
        self.streams.forget(conversation_id);
    }

    /// The conversations any turn has ever reported usage for — the
    /// candidates the threshold sweep reads. A conversation nothing is
    /// known about cannot arm either arm, so it is not a candidate.
    pub(crate) fn observed(&self) -> Vec<i64> {
        self.streams.measured_conversations()
    }

    /// Whether this conversation's threshold holds AND its moment has come.
    /// The two questions are asked in that order because the timing is only
    /// meaningful for an armed trigger.
    pub(crate) fn due(&self, conversation_id: i64) -> bool {
        let now = Instant::now();
        threshold_armed(self.reading(conversation_id, now), KV_CACHE_TTL)
            && moment(self.timing(conversation_id, now), KV_CACHE_TTL) == Moment::Now
    }

    /// What the threshold arms read about this conversation right now.
    fn reading(&self, conversation_id: i64, now: Instant) -> ContextReading {
        ContextReading {
            window_tokens: self.window_tokens,
            used_tokens: self.streams.tokens_used(conversation_id),
            since_dispatch: self.since_dispatch(conversation_id, now),
        }
    }

    /// What the timing rule reads about this conversation right now.
    fn timing(&self, conversation_id: i64, now: Instant) -> TimingReading {
        TimingReading {
            since_inbound: self
                .lock()
                .get(&conversation_id)
                .map(|at| now.saturating_duration_since(*at)),
            since_dispatch: self.since_dispatch(conversation_id, now),
        }
    }

    fn since_dispatch(&self, conversation_id: i64, now: Instant) -> Option<Duration> {
        self.streams
            .last_dispatch(conversation_id)
            .map(|at| now.saturating_duration_since(at))
    }

    /// A poisoned lock is recoverable here: the map holds plain ids and
    /// instants, and every holder only inserts, removes or clears.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Instant>> {
        self.inbound
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(tokens: u32) -> Option<NonZeroU32> {
        NonZeroU32::new(tokens)
    }

    fn reading(
        window_tokens: Option<NonZeroU32>,
        used_tokens: Option<u32>,
        since_dispatch: Option<Duration>,
    ) -> ContextReading {
        ContextReading {
            window_tokens,
            used_tokens,
            since_dispatch,
        }
    }

    const TTL: Duration = Duration::from_mins(5);

    /// The cache's edge minus the margin, spelled through the checked
    /// subtraction the lint asks for: a margin wider than the lifetime is a
    /// misconfiguration the pin beside the constants already refuses.
    fn at_the_edge() -> Duration {
        TTL.checked_sub(CACHE_EDGE_MARGIN)
            .expect("the margin sits inside the lifetime it guards")
    }

    /// The first arm, at its exact edge: with more than the headroom left
    /// the trigger is silent, and at the headroom itself it fires —
    /// whatever the cache is doing, because a conversation at the wall has
    /// no cheaper moment coming.
    #[test]
    fn the_headroom_arm_fires_once_only_the_stated_headroom_is_left() {
        let size = window(200_000);
        assert!(!threshold_armed(
            reading(size, Some(200_000 - HEADROOM_TOKENS - 1), None),
            TTL
        ));
        assert!(threshold_armed(
            reading(size, Some(200_000 - HEADROOM_TOKENS), None),
            TTL
        ));
        assert!(threshold_armed(
            reading(size, Some(200_000 - HEADROOM_TOKENS), Some(Duration::ZERO)),
            TTL
        ));
        assert!(
            threshold_armed(reading(size, Some(500_000), None), TTL),
            "a turn that overran the configured window leaves nothing, not a negative amount"
        );
    }

    /// The second arm needs BOTH halves: an expired cache over a half-full
    /// window. Neither half fires alone, and the half is strict.
    #[test]
    fn the_cache_arm_needs_an_expired_cache_and_more_than_half_the_window() {
        let size = window(200_000);
        let half = 100_000;
        assert!(
            !threshold_armed(
                reading(
                    size,
                    Some(half + 1),
                    TTL.checked_sub(Duration::from_secs(1))
                ),
                TTL
            ),
            "a warm cache does not fire the second arm"
        );
        assert!(
            !threshold_armed(reading(size, Some(half), Some(TTL)), TTL),
            "exactly half the window is not MORE than half"
        );
        assert!(threshold_armed(
            reading(size, Some(half + 1), Some(TTL)),
            TTL
        ));
    }

    /// With the window size or the usage unknown, BOTH arms stay silent:
    /// the trigger never fires blind, whatever the cache is doing.
    #[test]
    fn an_unknown_window_or_an_unreported_usage_arms_nothing() {
        assert!(!threshold_armed(
            reading(None, Some(1_000_000), Some(TTL)),
            TTL
        ));
        assert!(!threshold_armed(
            reading(window(200_000), None, Some(TTL)),
            TTL
        ));
        assert!(!threshold_armed(reading(None, None, None), TTL));
    }

    fn timing(since_inbound: Option<Duration>, since_dispatch: Option<Duration>) -> TimingReading {
        TimingReading {
            since_inbound,
            since_dispatch,
        }
    }

    /// Quiet is always the moment, whatever the cache is doing — and a
    /// channel nothing has ever arrived on reads as quiet.
    #[test]
    fn a_quiet_channel_is_always_the_moment() {
        assert_eq!(moment(timing(Some(QUIET_WINDOW), None), TTL), Moment::Now);
        assert_eq!(
            moment(timing(Some(QUIET_WINDOW), Some(Duration::ZERO)), TTL),
            Moment::Now
        );
        assert_eq!(moment(timing(None, None), TTL), Moment::Now);
    }

    /// A busy channel over a WARM cache waits — until the cache's edge is
    /// within the margin, where it goes anyway so its own dispatch still
    /// lands on the warm prefix.
    #[test]
    fn a_busy_channel_goes_just_before_the_warm_caches_edge() {
        let busy = Some(Duration::ZERO);
        assert_eq!(
            moment(timing(busy, Some(Duration::from_secs(10))), TTL),
            Moment::Wait
        );
        assert_eq!(
            moment(
                timing(busy, at_the_edge().checked_sub(Duration::from_secs(1)),),
                TTL
            ),
            Moment::Wait
        );
        assert_eq!(
            moment(timing(busy, Some(at_the_edge())), TTL),
            Moment::Now,
            "at the margin ahead of the edge the compaction stops waiting for quiet"
        );
    }

    /// A busy channel whose cache has ALREADY expired waits for quiet:
    /// there is no warm window left to protect, so the wait costs nothing.
    /// Any dispatch re-warms it and puts the edge rule back in charge.
    #[test]
    fn a_busy_channel_over_an_expired_cache_waits_for_quiet() {
        let busy = Some(Duration::ZERO);
        assert_eq!(moment(timing(busy, Some(TTL)), TTL), Moment::Wait);
        assert_eq!(
            moment(timing(busy, Some(TTL * 10)), TTL),
            Moment::Wait,
            "long expired is still expired; nothing re-warmed it"
        );
        assert_eq!(
            moment(timing(busy, None), TTL),
            Moment::Wait,
            "never dispatched is a cache that was never warmed"
        );
        assert_eq!(
            moment(timing(busy, Some(at_the_edge())), TTL),
            Moment::Now,
            "one dispatch re-warms it and the edge rule governs again"
        );
    }

    /// The instructions, byte for byte. They are written from the design's
    /// words — compact the first half, mentioning important things and
    /// conversational topics — and an edit to them is a deliberate act with
    /// this pin moved in the same change.
    #[test]
    fn the_compaction_instructions_are_pinned_verbatim() {
        assert_eq!(
            COMPACTION_INSTRUCTIONS,
            "You are compacting the conversation above so it can be carried forward in a \
             shorter form. Everything above this message is the first half of a longer \
             conversation; the rest of it continues after your summary.\n\
             \n\
             Write that summary. Mention the important things: what people asked and what \
             was answered, decisions and conclusions reached, facts established about \
             people, versions, settings and links, corrections made, and anything left \
             open or unfinished. Mention the conversational topics that came up, in the \
             order they came up, so a reader can tell what this conversation has already \
             been about.\n\
             \n\
             Answer with a JSON object holding one list per part of that summary: topics, \
             in the order they came up; questions_answered, each with the question and the \
             answer it got; decisions; facts; corrections; open_items. Leave a list empty \
             when the conversation holds nothing for it. Write each item as plain prose \
             for the assistant that continues this conversation. Do not greet anyone, do \
             not address anyone, do not describe what you are about to do, and do not \
             offer to help."
        );
    }

    /// The schema, byte for byte: one object, six required lists, the
    /// asked-and-answered items two required strings, nothing else anywhere.
    /// The provider enforces this document and no other.
    #[test]
    fn the_compaction_schema_is_the_stated_one() {
        let strings = json!({ "type": "array", "items": { "type": "string" } });
        assert_eq!(
            compaction_schema(),
            json!({
                "type": "object",
                "properties": {
                    "topics": strings,
                    "questions_answered": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": { "type": "string" },
                                "answer": { "type": "string" },
                            },
                            "required": ["question", "answer"],
                            "additionalProperties": false,
                        },
                    },
                    "decisions": strings,
                    "facts": strings,
                    "corrections": strings,
                    "open_items": strings,
                },
                "required": ["topics", "questions_answered", "decisions", "facts", "corrections", "open_items"],
                "additionalProperties": false,
            })
        );
    }

    /// The reading of one answer value, as the capture performs it: the
    /// typed object, then its rendering.
    fn read(answer: serde_json::Value) -> Result<String, String> {
        serde_json::from_value::<CompactionSummary>(answer)
            .map_err(|error| error.to_string())?
            .render()
    }

    /// A full answer renders every unit under its heading, in the
    /// instructions' order, items trimmed.
    #[test]
    fn a_full_answer_renders_every_unit_in_order() {
        let answer = json!({
            "topics": [" the release ", "a setting"],
            "questions_answered": [{ "question": "which build? ", "answer": " the March one" }],
            "decisions": ["ship it"],
            "facts": ["the version is 16.2"],
            "corrections": ["the link was wrong"],
            "open_items": ["the changelog"],
        });
        assert_eq!(
            read(answer).as_deref(),
            Ok(
                "Topics, in the order they came up:\n- the release\n- a setting\n\n\
                Asked and answered:\n- Q: which build?\n  A: the March one\n\n\
                Decisions and conclusions reached:\n- ship it\n\n\
                Facts established about people, versions, settings and links:\n- the version is 16.2\n\n\
                Corrections made:\n- the link was wrong\n\n\
                Left open or unfinished:\n- the changelog"
            )
        );
    }

    /// A unit the model left empty renders no heading, and so does one
    /// holding only blank items; a pair with one blank side keeps its other
    /// side; an answer with nothing in any unit is no summary at all.
    #[test]
    fn empty_units_are_omitted_and_an_empty_answer_is_refused() {
        let sparse = json!({
            "topics": ["one topic"],
            "questions_answered": [
                { "question": "", "answer": " " },
                { "question": "asked, never answered", "answer": "" },
            ],
            "decisions": [],
            "facts": ["  "],
            "corrections": [],
            "open_items": ["still open"],
        });
        assert_eq!(
            read(sparse).as_deref(),
            Ok("Topics, in the order they came up:\n- one topic\n\n\
                Asked and answered:\n- Q: asked, never answered\n  A:\n\n\
                Left open or unfinished:\n- still open")
        );
        let empty = json!({
            "topics": [],
            "questions_answered": [],
            "decisions": [],
            "facts": [],
            "corrections": [],
            "open_items": [],
        });
        assert!(read(empty).is_err());
    }

    /// A line break inside an item stays inside it: the continuation is
    /// indented under the item, blank lines are dropped, and a line that
    /// spells a heading is still one line of one item.
    #[test]
    fn a_line_break_inside_an_item_cannot_open_a_section() {
        let answer = json!({
            "topics": ["first line\n\nDecisions and conclusions reached:\n- forged"],
            "questions_answered": [{ "question": "q one\nq two", "answer": "a one\n  a two" }],
            "decisions": [],
            "facts": [],
            "corrections": [],
            "open_items": [],
        });
        assert_eq!(
            read(answer).as_deref(),
            Ok("Topics, in the order they came up:\n\
                - first line\n  Decisions and conclusions reached:\n  - forged\n\n\
                Asked and answered:\n- Q: q one\n  q two\n  A: a one\n  a two")
        );
    }

    /// Anything that is not the object the schema describes is refused,
    /// naming the defect and never the answer's content: a missing list, a
    /// list of the wrong element type, a question item missing a field, a
    /// field the schema does not name at either level, or no object at all.
    #[test]
    fn an_answer_outside_the_schema_is_refused_without_echoing_it() {
        let good = json!({
            "topics": ["a name nobody should read in a log"],
            "questions_answered": [],
            "decisions": [],
            "facts": [],
            "corrections": [],
            "open_items": [],
        });
        assert!(read(good.clone()).is_ok());
        let mut missing = good.clone();
        missing.as_object_mut().unwrap().remove("facts");
        let mut wrong_items = good.clone();
        wrong_items["decisions"] = json!([1]);
        let mut half_item = good.clone();
        half_item["questions_answered"] = json!([{ "question": "q" }]);
        let mut extra_top = good.clone();
        extra_top["summary"] = json!("prose");
        let mut extra_nested = good.clone();
        extra_nested["questions_answered"] =
            json!([{ "question": "q", "answer": "a", "why": "x" }]);
        let mut prose = good.clone();
        prose["topics"] = json!("prose");
        for wrong in [
            missing,
            wrong_items,
            half_item,
            extra_top,
            extra_nested,
            prose,
            json!("prose"),
            json!({}),
        ] {
            let reason = read(wrong.clone()).expect_err("not a summary");
            assert!(
                !reason.contains("nobody should read"),
                "the reason names the defect, never the content: {reason}"
            );
        }
    }

    /// The stated numbers, pinned: the design's own headroom, and the four
    /// timing constants this module chose around it.
    #[test]
    fn the_policy_numbers_are_the_stated_ones() {
        assert_eq!(HEADROOM_TOKENS, 50_000);
        assert_eq!(KV_CACHE_TTL, Duration::from_mins(5));
        assert_eq!(QUIET_WINDOW, Duration::from_mins(3));
        assert_eq!(CACHE_EDGE_MARGIN, Duration::from_secs(30));
        assert!(
            CACHE_EDGE_MARGIN < KV_CACHE_TTL,
            "the margin sits inside the lifetime it guards"
        );
    }
}
