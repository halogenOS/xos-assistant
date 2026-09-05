//! Which message one of the assistant's own messages threads onto (unit
//! 26, re-keyed by unit 55, 2026-09-02): the one the MODEL named, through
//! the reply tool. The derived threading is gone with the relay it served —
//! the edge no longer guesses which absorbed message an answer was for —
//! and a send the model aimed at nothing goes out plain.
//!
//! One rule survives the change untouched, and it is the reason this
//! module exists: a send whose own text carries a reply-acted command shape
//! goes out UNTHREADED, with its words byte-for-byte as the model wrote
//! them.

use agent_ledger::Store;
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::mirror::DELETION_COMMAND;
use assistant_core::schema::store_config;
use assistant_core::tools::report::REPORT_LINE_LEAD;
use assistant_core::{AnsweringMode, ChannelKind, ProtectionConfig, ReplyThread};

use crate::support::{
    self, answer_to, first_answer_to, inbound, inbound_unaddressed, recv_reply, with_origin,
};

/// The outbound edge a fixture's replies arrive on.
type Replies = tokio::sync::mpsc::UnboundedReceiver<assistant_core::Outbound>;

/// A running assistant under the given answering mode, with the outbound
/// edge open and a held stream, so a test can absorb messages into one
/// open turn.
async fn threading_fixture(
    answering: AnsweringMode,
    hold: Option<std::sync::Arc<support::TurnHold>>,
) -> (support::Fixture, Replies) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture =
        support::start_assistant_answering(store, hold, ProtectionConfig::default(), answering)
            .await;
    let replies = support::outbound(&fixture).await;
    (fixture, replies)
}

/// Await the conversation holding the given number of recorded messages —
/// what proves an absorbed message reached the ledger behind an open
/// stream, before the stream is released.
async fn await_messages(fixture: &support::Fixture, conversation_id: i64, count: usize) {
    support::await_ledger(
        &fixture.store,
        conversation_id,
        "the absorbed messages behind the open stream",
        move |blocks| {
            blocks
                .iter()
                .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
                .count()
                == count
        },
    )
    .await;
}

/// AC6 and AC7 (unit 55): a send threads onto the message the MODEL named,
/// and only onto that one.
///
/// The turn is crowded on purpose — an unaddressed frontier, an addressed
/// message absorbed behind it, a bystander behind that — which is exactly
/// the shape the old derived threading was built to read. It reads nothing
/// now: the target is the id the model handed the reply tool, and here that
/// is the FRONTIER's, the one line the deleted rule would never have
/// picked, since it picked the message that addressed the assistant.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_send_threads_onto_the_message_the_model_named() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) =
        threading_fixture(AnsweringMode::Helpful, Some(hold.clone())).await;
    let room = support::authorized_group(&fixture.assistant, "room-crowded").await;

    // The frontier: an unaddressed warm-up line, which helpful answering
    // summons a turn for.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "C",
                &format!("morning all {cue}", cue = support::REPLY_CUE),
            ),
            "origin-warm-up",
        ),
    )
    .await;
    hold.started().await;

    // Absorbed into the open turn: the one member who addresses the
    // assistant, then a bystander's line behind them. Neither is what the
    // send aims at.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", "which kernel does it ship?"),
            "origin-asker",
        ),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&room, ChannelKind::Group, "B", "same here"),
            "origin-bystander",
        ),
    )
    .await;
    await_messages(&fixture, receipt.conversation_id, 3).await;
    hold.release();

    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.reply_target,
        Some(ReplyThread::OntoOrPlainly("origin-warm-up".into())),
        "the send threads onto the id the model named — and not onto the \
         message that addressed the assistant, which the deleted derived \
         threading would have picked"
    );
}

/// A send the model aimed at nothing goes out plain, however many people
/// addressed the assistant in the turn. There is no guess left to make.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_send_the_model_aimed_at_nothing_goes_out_plain() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) =
        threading_fixture(AnsweringMode::Addressed, Some(hold.clone())).await;
    let room = support::authorized_group(&fixture.assistant, "room-crowd-asks").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", "does it support my device?"),
            "origin-first-asker",
        ),
    )
    .await;
    hold.started().await;
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "B", "and what about mine?"),
            "origin-second-asker",
        ),
    )
    .await;
    await_messages(&fixture, receipt.conversation_id, 2).await;
    hold.release();

    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.reply_target, None,
        "the model named no target, so the message quotes nobody"
    );
    assert_eq!(
        reply.text,
        // The dispatched request was assembled from the summoning message,
        // so the scripted answer echoes it; the second ask joined the turn
        // behind the request and is answered by the same turn.
        first_answer_to("does it support my device?"),
        "the message itself is delivered whole"
    );
}

/// AC3's absent half and AC4: a helpful-mode answer to messages nobody
/// addressed the assistant with goes out plain. The rule yields no target
/// by construction — no message carries the literal addressed fact — so
/// no mode is consulted anywhere in the delivery. Answering an unaddressed
/// message is a courtesy; quote-replying someone who never asked, in front
/// of the group, is not. The answer still arrives.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_helpful_mode_answer_to_nobodys_question_delivers_plainly() {
    let (fixture, mut replies) = threading_fixture(AnsweringMode::Helpful, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-helpful-plain").await;

    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&room, ChannelKind::Group, "A", "the flash keeps failing"),
            "origin-unaddressed",
        ),
    )
    .await;

    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.reply_target, None,
        "nobody addressed the assistant, so the answer quotes nobody"
    );
    assert_eq!(
        reply.text,
        first_answer_to("the flash keeps failing"),
        "the courtesy answer is delivered, unthreaded"
    );
}

/// AC5, and AC7's threading half in unit 55: a message whose text carries
/// the moderation command shape is delivered plainly EVEN THOUGH the model
/// aimed it, with its text byte-for-byte as the model wrote it. The
/// moderation bot files a report from a reply carrying that shape, so a
/// threaded message repeating it would file a real report against the
/// message it threaded onto. Nothing is rewritten, stripped or withheld —
/// the routing changes, the words do not.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_send_carrying_the_moderation_command_shape_delivers_plainly() {
    let (fixture, mut replies) = threading_fixture(AnsweringMode::Addressed, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-command-prose").await;

    // The scripted answer echoes the ask, so an ask about the command puts
    // the command shape into the assistant's own prose.
    let ask = format!(
        "what does {REPORT_LINE_LEAD}moderator_bot actually do? {cue}",
        cue = support::REPLY_CUE
    );
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", &ask),
            "origin-command-asker",
        ),
    )
    .await;

    let reply = recv_reply(&mut replies).await;
    let expected = first_answer_to(&ask);
    assert!(
        expected.contains(REPORT_LINE_LEAD),
        "the premise: the answer's own prose carries the command shape"
    );
    assert_eq!(
        reply.reply_target, None,
        "an answer carrying the command shape is delivered as a plain \
         message, so it can file no report"
    );
    assert_eq!(
        reply.text, expected,
        "the text goes out exactly as written: no sanitation, no refusal"
    );

    // The routing is the whole difference: the same aim with the shape
    // absent keeps its thread.
    let ordinary = format!("and where are the rules? {cue}", cue = support::REPLY_CUE);
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", &ordinary),
            "origin-ordinary-asker",
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.reply_target,
        Some(ReplyThread::OntoOrPlainly("origin-ordinary-asker".into())),
        "the same aim, with no command shape in the words, keeps its thread"
    );
    assert_eq!(reply.text, answer_to(&ordinary));
}

/// The second reply-acted shape holds the same guard as the first.
/// `crates/core/src/mirror.rs` records [`DELETION_COMMAND`] as the
/// moderation bot's own command, which an administrator invokes by
/// replying with it — so a threaded answer repeating that shape would end
/// in a deletion of an innocent message rather than a report. Decision
/// 0108 was widened on 2026-08-27 to cover every reply-acted shape, and
/// the shapes come from the one list in `reply_commands.rs`, which
/// this pin holds to its word.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_send_carrying_the_deletion_command_shape_delivers_plainly() {
    let (fixture, mut replies) = threading_fixture(AnsweringMode::Addressed, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-deletion-prose").await;

    let ask = format!(
        "what does {DELETION_COMMAND} do in this group? {cue}",
        cue = support::REPLY_CUE
    );
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", &ask),
            "origin-deletion-asker",
        ),
    )
    .await;

    let reply = recv_reply(&mut replies).await;
    assert!(
        reply.text.contains(DELETION_COMMAND),
        "the premise: the answer's own prose carries the deletion shape"
    );
    assert_eq!(
        reply.reply_target, None,
        "an answer carrying the deletion command shape is delivered as a \
         plain message, so an administrator's bot can act on no reply of \
         the assistant's"
    );
}
