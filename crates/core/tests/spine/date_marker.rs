//! The framework's date record, as the consumer meets it.
//!
//! Every other module in this suite reads the ledger through
//! [`support::consumer_view`], which drops these rows so a test about the
//! assistant's own content is not counting the framework's calendar. That
//! filter would equally hide a marker written twice, or written in the wrong
//! place, so the fact it filters is pinned here — once, against the raw
//! ledger and the raw recorded request.

use agent_ledger::agency::{DateMarker, LeafKind};
use agent_ledger::providers::{Message, MessageRole};
use agent_ledger::{Block, Store};
use assistant_core::ChannelKind;
use assistant_core::kind::CHAT_MESSAGE_KIND;

use crate::support::{self, channel, inbound};

/// The lead of the line a date marker renders into a request.
///
/// The whole line — the weekday, the timezone clause, the writing minute —
/// is the framework's format decision, pinned by the framework's own render
/// golden. The consumer knows only that a dated line arrives and how it
/// starts, so this lead is the only shape of it recorded here: a widened
/// clause on the framework's side must not turn into a consumer test
/// failure, and recomputing the line would both duplicate that decision and
/// race midnight.
const DATED_LINE_LEAD: &str = "Current date: ";

/// The conversation's date markers, oldest first, read off the raw ledger.
fn markers(blocks: &[Block]) -> Vec<&Block> {
    blocks
        .iter()
        .filter(|block| DateMarker::KINDS.contains(&block.block_type.as_str()))
        .collect()
}

/// The position of the given block on the ledger, by its id.
fn position_of(blocks: &[Block], wanted: &Block) -> usize {
    blocks
        .iter()
        .position(|block| block.id == wanted.id)
        .expect("the block was read off this ledger")
}

/// The position of the conversation's first recorded chat message.
fn first_message_at(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .position(|block| block.block_type == CHAT_MESSAGE_KIND)
        .expect("the ingested message is on the ledger")
}

/// The raw ledger of one conversation — the view every other module filters.
async fn raw_ledger(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
}

/// The stored types of a ledger, for a failure message that says what the
/// assertion actually met.
fn shape_of(blocks: &[Block]) -> Vec<&str> {
    blocks
        .iter()
        .map(|block| block.block_type.as_str())
        .collect()
}

/// Every dated line of one request reaches the model as a system message of
/// its own, behind the prompt's, and the request carries `dated_lines` of
/// them.
///
/// The consumer's ledger head is prompt, choice, marker, message: the
/// choice projects no role, which splits the system run, so nothing joins
/// the dated line onto the prompt's message. Matched by its lead alone —
/// the rest of the line is the framework's format, recorded there. The
/// expected count is the caller's, because a request rendered after a run
/// stepped over local midnight carries one line per marked date.
fn assert_dated_line_stands_alone(request: &[Message], dated_lines: usize) {
    let prompt_at = request
        .iter()
        .position(|message| support::carries(message, support::SYSTEM_PROMPT))
        .expect("the request carries the recorded prompt");
    let dated: Vec<usize> = request
        .iter()
        .enumerate()
        .filter(|(_, message)| support::carries(message, DATED_LINE_LEAD))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        dated.len(),
        dated_lines,
        "one dated line per marked date in the request: {request:?}"
    );
    for dated_at in dated {
        assert_eq!(
            request[dated_at].role,
            MessageRole::System,
            "the dated line speaks in the system voice: {request:?}"
        );
        assert!(
            dated_at > prompt_at,
            "the dated line follows the prompt's message: {request:?}"
        );
    }
    assert!(
        !support::carries(&request[prompt_at], DATED_LINE_LEAD),
        "the dated line is a message of its own, never folded into the prompt's: {request:?}"
    );
}

/// The consumer's own ledger carries the framework's calendar exactly once
/// per recorded date, ahead of the message that tripped it, and the dated
/// line reaches the model as a system message of its own behind the
/// prompt's — in the first recorded request and in the newest one alike, so
/// the shape is pinned as the ledger grows and not only on a bare
/// conversation.
///
/// Cardinality is asserted here and nowhere else: every other module reads
/// through the consumer view, which would swallow a second marker silently.
/// Repetition is judged by the markers' STORED dates, never by a count
/// against the wall clock, so a run that steps over local midnight — the
/// second ingest landing on the next day, which legitimately writes a
/// second marker — reads as the two distinct dates it is instead of as a
/// failure. The newest request's line count is the marked-date count for
/// the same reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_framework_dates_the_ledger_once_per_day_ahead_of_the_message() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let key = channel("dm-dates");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the first ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    support::settle_shape(
        &fixture.store,
        conv,
        "the first answered turn",
        &["system_prompt", "tool_choice", "chat_message", "text"],
    )
    .await;
    support::recv_reply(&mut replies).await;

    let blocks = raw_ledger(&fixture.store, conv).await;
    let written = markers(&blocks);
    assert_eq!(
        written.len(),
        1,
        "the day's first user-voiced append writes exactly one date marker; the ledger is {:?}",
        shape_of(&blocks)
    );
    assert!(
        position_of(&blocks, written[0]) < first_message_at(&blocks),
        "the marker rides ahead of the message that tripped it; the ledger is {:?}",
        shape_of(&blocks)
    );

    // The second ask, same conversation: no fresh marker for a date already
    // recorded. A midnight crossing writes one for the NEW date, which is
    // the framework working, so the pin is on repetition, not on the count.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the second ask"),
    )
    .await;
    support::settle_shape(
        &fixture.store,
        conv,
        "the second answered turn",
        &[
            "system_prompt",
            "tool_choice",
            "chat_message",
            "text",
            "chat_message",
            "text",
        ],
    )
    .await;
    support::recv_reply(&mut replies).await;

    let blocks = raw_ledger(&fixture.store, conv).await;
    let mut dates: Vec<String> = markers(&blocks)
        .into_iter()
        .map(|block| DateMarker::parse(block).date)
        .collect();
    let recorded = dates.len();
    dates.sort();
    dates.dedup();
    assert_eq!(
        dates.len(),
        recorded,
        "a date the ledger already carries is never marked twice; the marked dates are {dates:?}"
    );

    let seen = fixture.script.seen.lock().expect("the requests read");
    assert_eq!(
        seen.len(),
        2,
        "two asks, two answered turns, each recording its own request"
    );
    assert_dated_line_stands_alone(
        seen.first().expect("the first turn recorded its request"),
        1,
    );
    // The newest request too: the marker is a stored row every later
    // projection folds back in, so a second turn must carry the same dated
    // line once, in the same standalone system voice — one line per date the
    // ledger has marked by now.
    assert_dated_line_stands_alone(
        seen.last().expect("the newest turn recorded its request"),
        dates.len(),
    );
}
