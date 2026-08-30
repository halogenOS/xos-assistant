//! Which message an answer is delivered as a reply to (unit 26): the one
//! message the turn absorbed that addressed the assistant, when exactly
//! one did. Nobody addressed it, or several did, and the answer goes out
//! plain — never silent, never a guess. An answer whose own prose carries
//! the moderation command shape goes out plain too, with its text
//! untouched.

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
    let replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
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

/// AC2: the answer is delivered as a reply to the member who addressed the
/// assistant, and to nobody else. Three members speak into one turn: the
/// warm-up line summons it under helpful answering and becomes the
/// dispatch frontier, the asker's mention is absorbed into the open
/// stream, and a bystander's chatter is absorbed behind it. The addressed
/// message is therefore neither the frontier nor the newest — the two
/// messages an anchor-threaded or newest-threaded delivery would quote —
/// and it is the one the answer names.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_one_addressed_message_in_a_crowded_turn_is_the_answers_target() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) =
        threading_fixture(AnsweringMode::Helpful, Some(hold.clone())).await;
    let room = support::authorized_group(&fixture.assistant, "room-crowded").await;

    // The frontier: an unaddressed warm-up line, which helpful answering
    // summons a turn for.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&room, ChannelKind::Group, "C", "morning all"),
            "origin-warm-up",
        ),
    )
    .await;
    hold.started().await;

    // Absorbed into the open turn: the one member who addresses the
    // assistant, then a bystander's line behind them.
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
        Some(ReplyThread::OntoOrPlainly("origin-asker".into())),
        "the answer is delivered as a reply to the member who addressed \
         the assistant — not to the frontier, not to the newest line — and \
         plainly if the platform refuses that thread"
    );
}

/// AC3, the ambiguous half: two members address the assistant in one turn
/// and the answer names neither. Picking one would tell the other they
/// were ignored, and picking the newest is the same guess decision 0018
/// refused. The answer still arrives — ambiguity costs the thread, never
/// the answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_addressed_messages_in_one_turn_deliver_the_answer_plainly() {
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
        "two askers name no single target, and neither is quoted"
    );
    assert_eq!(
        reply.text,
        // The dispatched request was assembled from the summoning message,
        // so the scripted answer echoes it; the second ask joined the turn
        // behind the request and is answered by the same turn.
        first_answer_to("does it support my device?"),
        "the answer itself is delivered whole"
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

/// AC5: an answer whose prose carries the moderation command shape is
/// delivered plainly, with its text byte-for-byte as the model wrote it.
/// The moderation bot files a report from a reply carrying that shape, so
/// a threaded answer repeating it would file a real report against the
/// message it threaded onto. Nothing is rewritten, stripped or withheld —
/// the routing changes, the words do not.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_answer_carrying_the_moderation_command_shape_delivers_plainly() {
    let (fixture, mut replies) = threading_fixture(AnsweringMode::Addressed, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-command-prose").await;

    // The scripted answer echoes the ask, so an ask about the command puts
    // the command shape into the assistant's own prose.
    let ask = format!("what does {REPORT_LINE_LEAD}moderator_bot actually do?");
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

    // The routing is the whole difference: an answer with the same shape
    // absent threads onto the asker as usual.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", "and where are the rules?"),
            "origin-ordinary-asker",
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.reply_target,
        Some(ReplyThread::OntoOrPlainly("origin-ordinary-asker".into())),
        "the same asker's ordinary question is answered as a reply to them"
    );
    assert_eq!(reply.text, answer_to("and where are the rules?"));
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
async fn an_answer_carrying_the_deletion_command_shape_delivers_plainly() {
    let (fixture, mut replies) = threading_fixture(AnsweringMode::Addressed, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-deletion-prose").await;

    let ask = format!("what does {DELETION_COMMAND} do in this group?");
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
