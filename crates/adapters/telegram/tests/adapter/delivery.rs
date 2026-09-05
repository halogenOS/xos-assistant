//! The delivery receipt across the adapter thread (unit 38, AC2 and AC5):
//! what the wire actually put in the chat becomes what the ledger records,
//! on both send paths, per chunk, and only for chunks that reached the
//! chat — and a member replying to any of those messages quotes her whole
//! stored answer.
//!
//! The pins run against the scripted Bot API server, whose `sendMessage`
//! answers a fresh platform id per delivered send, exactly as the platform
//! does. That is what makes "the record names the message the platform
//! took" an assertion instead of a coincidence.

use std::sync::Arc;

use agent_ledger::agency::Quote;
use agent_ledger::{Block, LeafKind};
use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    TempStateFile, answer_to, authorize_group, await_conversations, await_quiet, await_receipts,
    pin_update, private_update, receipts, recording_sleep, reply_to_bot_message, spawn_adapter,
    start_assistant,
};

/// The block she said her answer AS: the outgoing message a sending tool
/// filed, which is what the chat received. Her turn's own text is private
/// notes from unit 55 on, so a receipt names this block and never that.
async fn her_answer(store: &agent_ledger::Store, conversation_id: i64) -> Block {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .rev()
        .find(|block| block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND)
        .expect("her answer is stored")
}

/// AC2, the answer's own record: a delivered answer yields one `Delivered`
/// block naming the platform id the send was answered with, that same id as
/// the delivery key, and the answer's own stored block.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delivered_answer_records_its_platform_id_and_her_block() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 5, "where did the setting move?"));

    let state = TempStateFile::new("delivery-answer");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    server.await_recorded("sendMessage", 1).await;
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let rows = await_receipts(&fixture.store, conversation, 1).await;
    let answer = her_answer(&fixture.store, conversation).await;

    assert_eq!(rows.len(), 1, "one message in the chat, one receipt");
    assert_eq!(
        rows[0].origin.as_deref(),
        Some("1"),
        "the record names the id the platform answered the send with"
    );
    assert_eq!(
        rows[0].delivery.as_deref(),
        Some("1"),
        "a one-message send is its own delivery key"
    );
    assert_eq!(
        rows[0].answer_block,
        Some(answer.id),
        "the record names the block she said it as"
    );
}

/// AC2, the other send path: a deterministic item — the privacy command's
/// fixed answer — records its delivery like everything else, and its row
/// names no block of hers, because an item is the core's own prose and
/// never one of her blocks.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_deterministic_items_record_names_no_block() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 5, "/privacy"));

    let state = TempStateFile::new("delivery-item");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::PRIVACY_UNPUBLISHED),
        "non-vacuity: the send under test is the item's, not an answer's"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let rows = await_receipts(&fixture.store, conversation, 1).await;

    assert_eq!(rows.len(), 1, "the item's send records like any other");
    assert_eq!(rows[0].origin.as_deref(), Some("1"));
    assert_eq!(
        rows[0].answer_block, None,
        "an item carries no block of hers, so a reply to it lands quoteless"
    );
}

/// AC2, the observe path's own record: the rules acknowledgment an
/// observation returns is recorded like every other send, under the handle
/// that rode with the observed item (decision 0141) — and its row names no
/// block of hers, because the acknowledgment is the core's own prose.
///
/// The two send paths meet the ledger through different calls: the
/// ingest path's handle comes from its receipt, this one's from
/// `ObserveOutcome::Observed`. Both are pinned here, so a regression that
/// drops the handle on either side fails a test.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observed_acknowledgments_record_names_no_block() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -604;
    server.set_chat_info(chat, "The kernel room", None);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(pin_update(1, chat, 9, "Rules:\n1. Be kind."));

    let state = TempStateFile::new("delivery-observed-item");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(answer_to("1. Be kind.")),
        "non-vacuity: the send under test is the acknowledgment's"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let rows = await_receipts(&fixture.store, conversation, 1).await;

    assert_eq!(
        rows.len(),
        1,
        "the observation's own send records like any other"
    );
    assert_eq!(
        rows[0].origin.as_deref(),
        Some("1"),
        "the record names the id the platform answered the send with"
    );
    assert_eq!(
        rows[0].delivery.as_deref(),
        Some("1"),
        "a one-message send is its own delivery key"
    );
    assert_eq!(
        rows[0].answer_block, None,
        "the acknowledgment carries no block of hers, so a reply to it \
         lands quoteless"
    );
}

/// AC2's failure half, both ways. A send whose FIRST chunk fails put
/// nothing in the chat and records nothing. A send cut short after some
/// chunks records exactly the chunks that reached it — no more, and not
/// nothing either.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_send_records_nothing_and_a_cut_short_one_records_what_reached_the_chat() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_send_failure_after(0);
    server.push_update(private_update(1, 5, "the dropped answer's cause"));

    let state = TempStateFile::new("delivery-failure");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    server.await_recorded("sendMessage", 1).await;
    let first = await_conversations(&fixture.store, 1).await[0];

    // The cut-short send, in its own chat: three chunks scripted, the
    // second one failing, so exactly one reaches the chat.
    let long_ask = "x".repeat(9000);
    server.script_send_failure_after(1);
    server.push_update(private_update(2, 6, &long_ask));
    server.await_recorded("sendMessage", 3).await;
    let conversations = await_conversations(&fixture.store, 2).await;
    let second = conversations
        .iter()
        .copied()
        .find(|id| *id != first)
        .expect("the second chat has its own conversation");
    let rows = await_receipts(&fixture.store, second, 1).await;

    assert_eq!(
        rows.len(),
        1,
        "exactly the chunk that reached the chat is recorded"
    );
    assert_eq!(
        rows[0].origin.as_deref(),
        Some("1"),
        "the recorded id is the delivered chunk's — the platform mints one \
         only for a message it took, so the refused send minted none"
    );
    assert!(
        receipts(&fixture.store, first).await.is_empty(),
        "a send that put nothing in the chat records nothing at all"
    );
}

/// AC5: a reply to a LATER chunk of a multi-chunk answer quotes her whole
/// stored answer. Every chunk's record names the same block, because the
/// chunks are the transport's and her message is the block.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_to_a_later_chunk_quotes_her_whole_answer() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    // The scripted answer echoes the ask, so an ask this long pushes the
    // answer past one message's cap and into two chunks.
    let long_ask = "x".repeat(5000);
    server.push_update(private_update(1, 5, &long_ask));

    let state = TempStateFile::new("delivery-chunk-reply");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    server.await_recorded("sendMessage", 2).await;
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let rows = await_receipts(&fixture.store, conversation, 2).await;
    let answer = her_answer(&fixture.store, conversation).await;
    assert_eq!(
        rows.iter().map(|row| row.answer_block).collect::<Vec<_>>(),
        vec![Some(answer.id), Some(answer.id)],
        "both chunks name the one block she said it as"
    );
    assert!(
        rows.iter().all(|row| row.delivery.as_deref() == Some("1")),
        "both chunks carry the send's first id as the delivery key"
    );

    // The reply names the SECOND chunk, which the platform gave id two. It
    // waits for the answering turn to finish first: a message pushed while
    // that turn still runs is absorbed by it instead of summoning its own.
    await_quiet(&fixture.store).await;
    server.push_update(reply_to_bot_message(
        2,
        "private",
        5,
        5,
        2,
        "and on the tablet?",
    ));
    server.await_recorded("sendMessage", 3).await;

    let quotes: Vec<Block> = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == Quote::KINDS[0])
        .collect();
    assert_eq!(quotes.len(), 1, "one reply to her, one quote block");
    let whole = i64::try_from(
        answer.fields[assistant_core::outgoing::COLUMN_TEXT]
            .as_str()
            .expect("her stored answer carries its text")
            .chars()
            .count(),
    )
    .expect("her answer's length fits an offset");
    assert_eq!(
        (
            quotes[0].fields["start_block_id"].as_i64(),
            quotes[0].fields["start_pos"].as_i64(),
            quotes[0].fields["end_block_id"].as_i64(),
            quotes[0].fields["end_pos"].as_i64(),
        ),
        (Some(answer.id), Some(0), Some(answer.id), Some(whole)),
        "the reply to the second chunk spans her WHOLE stored answer"
    );
}
