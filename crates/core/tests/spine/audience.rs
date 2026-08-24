//! The audience discipline's delivery mechanics (unit 21): a clarifying
//! question is ordinary answer text — delivered through the same edge as
//! any answer and never swallowed by the empty-answer check — a clear
//! question is answered directly, the disambiguation is two sequential
//! turns with the prior question in the second turn's projected context,
//! and the first-interaction disclosure composes onto a clarifying
//! question exactly as onto any first answer.

use agent_ledger::Store;
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::schema::store_config;
use assistant_core::{AnsweringMode, ChannelKind};
use serde_json::json;

use crate::support::{
    self, CLARIFY_CUE, CLARIFYING_QUESTION, Fixture, answer_to, carries, first_answer_to,
    inbound_unaddressed, recv_reply,
};

/// A running helpful-mode assistant over a fresh store, under the default
/// budgets — the mode where every group message opens a turn, so a plain
/// disambiguating follow-up is seen.
async fn helpful_fixture() -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    support::start_assistant_answering(
        store,
        None,
        assistant_core::ProtectionConfig::default(),
        AnsweringMode::Helpful,
    )
    .await
}

/// The message rows of one loaded ledger, in order.
fn message_rows(blocks: &[agent_ledger::Block]) -> Vec<&agent_ledger::Block> {
    blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect()
}

/// AC3: an ambiguous question draws the clarifying question as an ordinary
/// delivered answer — not swallowed by the empty-answer check, which
/// matches only an answer whose whole trimmed text is empty, and a
/// clarifying question is real text, so it falls through to ordinary
/// delivery; the asker is introduced first, so the delivered text is the
/// bare question and nothing else.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ambiguous_question_draws_the_clarifying_question_delivered_whole() {
    assert!(
        !CLARIFYING_QUESTION.trim().is_empty(),
        "the premise of the check: the question is real text"
    );

    let fixture = helpful_fixture().await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-ambiguous").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "which kernel ships today?"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("which kernel ships today?"),
        "the introducing answer, so the clarifying question below is bare"
    );

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("how do I get the sandboxed feature? {CLARIFY_CUE}"),
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, room);
    assert_eq!(
        reply.text, CLARIFYING_QUESTION,
        "the clarifying question reaches the chat as an ordinary answer"
    );

    let blocks = support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the clarifying turn",
        6,
    )
    .await;
    assert_eq!(
        blocks[5].fields["content"],
        json!(CLARIFYING_QUESTION),
        "the stored answer is the question itself: nothing rewrote or \
         swallowed it"
    );
}

/// AC4: a clear question is answered directly, not interrogated — the
/// scripted turn on an unambiguous question delivers the answer and no
/// clarifying question.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_clear_question_is_answered_directly_not_interrogated() {
    let fixture = helpful_fixture().await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-clear").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            "how do I flash the newest release on my phone?",
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text,
        first_answer_to("how do I flash the newest release on my phone?"),
        "the clear question is answered in one turn"
    );
    assert!(
        !reply.text.contains(CLARIFYING_QUESTION),
        "no clarifying question rides a clear question's answer"
    );
}

/// AC5: the disambiguation is a normal two-turn exchange. The clarifying
/// turn settles first; the member's disambiguating reply arrives as an
/// ordinary later message, opens its own turn, and that turn's projected
/// context carries the prior clarifying question — two sequential turns,
/// not one absorbed one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_disambiguation_is_a_normal_two_turn_exchange() {
    let fixture = helpful_fixture().await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-two-turns").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("how do I set up the sandboxed feature? {CLARIFY_CUE}"),
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLARIFYING_QUESTION),
        "the first turn closes on the clarifying question"
    );
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the clarifying turn",
        4,
    )
    .await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            "on my device, not building it myself",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        answer_to("on my device, not building it myself"),
        "the disambiguated question is answered in its own turn"
    );
    let blocks = support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the answering turn",
        6,
    )
    .await;
    assert_eq!(
        message_rows(&blocks).len(),
        2,
        "one recorded message per turn: nothing was absorbed mid-turn"
    );

    let seen = fixture.script.seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "two sequential model turns ran");
    assert!(
        seen[1]
            .iter()
            .any(|message| carries(message, CLARIFYING_QUESTION)),
        "the prior clarifying question is visible in the second turn's \
         projected context"
    );
    assert!(
        seen[1]
            .iter()
            .any(|message| carries(message, "on my device, not building it myself")),
        "the disambiguating reply is the second turn's newest question"
    );
}

/// AC7's clarifying half (unit 22): a clarifying question is real text,
/// so its turn raises the typing cue — one begin once the question's text
/// flows, one stop at the turn's end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_clarifying_question_raises_the_typing_cue() {
    let fixture = helpful_fixture().await;
    let mut composing = fixture.assistant.composing(support::ADAPTER);
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-clarify-cue").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("how do I get the sandboxed feature? {CLARIFY_CUE}"),
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLARIFYING_QUESTION),
        "the clarifying question reaches the chat"
    );

    let begun = tokio::time::timeout(support::DEADLINE, composing.recv())
        .await
        .expect("the cue arrives before the deadline")
        .expect("the composing edge outlives the test");
    assert_eq!(begun.channel, room);
    assert_eq!(begun.state, assistant_core::ComposingState::Composing);
    let stopped = tokio::time::timeout(support::DEADLINE, composing.recv())
        .await
        .expect("the stop arrives before the deadline")
        .expect("the composing edge outlives the test");
    assert_eq!(stopped.channel, room);
    assert_eq!(stopped.state, assistant_core::ComposingState::Stopped);
}

/// AC6: a clarifying question, as a new person's first delivered answer,
/// carries the once-per-person disclosure line — delivery is
/// content-agnostic past the empty-answer check, so the fold composes onto
/// a question exactly as onto any first answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_first_clarifying_question_carries_the_disclosure_line() {
    let fixture = helpful_fixture().await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-first-question").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "fresh-asker",
            &format!("how does the sandboxed feature reach me? {CLARIFY_CUE}"),
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text,
        support::disclosed(CLARIFYING_QUESTION),
        "the new person's first answer is the introduced clarifying question"
    );
    assert!(
        reply
            .text
            .starts_with(&format!("{}\n\n", support::fixture_disclosure().line())),
        "the line comes first, separated by a blank line"
    );
}
