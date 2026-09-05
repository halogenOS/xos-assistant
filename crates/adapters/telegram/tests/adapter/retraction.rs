//! The retraction over the wire (unit T4, criteria 4, 5 and 10): an
//! administrator replies to one of the assistant's own messages with the
//! moderation bot's deletion command, and the adapter takes every message of
//! that recorded delivery back off the chat — through the plural deletion
//! method, in batches of at most a hundred identifiers, whatever the size.
//!
//! The tests assert against the scripted Bot API server, whose `sendMessage`
//! answers a fresh platform id per delivered send. That is what makes "the
//! request names the messages the platform took" an assertion instead of a
//! coincidence.

use std::sync::Arc;

use agent_ledger::agency::{LeafKind, Streaming};
use assistant_core::delivery::{DELIVERED_KIND, Delivered, RETRACTION_KIND, Retraction};
use assistant_core::kind::{ChatMessage, LimitedBy};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::TryRecvError;

use crate::server::{BotApiServer, Recorded};
use crate::support::{
    self, TempStateFile, authorize_group, await_conversations, await_ledger, await_receipts,
    date_of, first_answer_to, message_id_of, private_update, recording_sleep, spawn_adapter,
    start_assistant,
};

/// A group message replying to one of the bot's own messages and carrying
/// the moderation bot's deletion command, from the named sender.
fn del_reply_to_bot(update_id: i64, chat_id: i64, admin_id: i64, her_message_id: i64) -> Value {
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": "group" },
            "from": { "id": admin_id, "first_name": format!("Person {admin_id}") },
            "text": "/del",
            "reply_to_message": {
                "message_id": her_message_id,
                "date": date_of(update_id) - 10,
                "chat": { "id": chat_id, "type": "group" },
                "from": {
                    "id": support::BOT_ID,
                    "is_bot": true,
                    "first_name": "Fixture",
                    "username": support::BOT_USERNAME,
                },
                "text": "an earlier answer",
            },
        },
    })
}

/// The identifiers one recorded deletion request carried, in its own order.
fn deleted_ids(request: &Recorded) -> Vec<i64> {
    request.body["message_ids"]
        .as_array()
        .expect("the request carries a message id list")
        .iter()
        .map(|id| id.as_i64().expect("a message id is a number"))
        .collect()
}

/// Criterion 4, the one-message case: the administrator's reply
/// retracts the answer through ONE plural-deletion request carrying the one
/// id — never the single-message method, whose refusal for an id it cannot
/// find would make the commonest case a logged failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admins_reply_retracts_her_answer_through_one_plural_request() {
    let chat = -700;
    let fixture = start_assistant().await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[(5, "administrator")]);
    server.push_update(support::mention_update(1, chat, 7, "where did it move?"));

    let state = TempStateFile::new("retraction-one");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to(&format!(
            "@{} where did it move?",
            support::BOT_USERNAME
        ))),
        "non-vacuity: the send under test is her answer"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_receipts(&fixture.store, conversation, 1).await;

    server.push_update(del_reply_to_bot(2, chat, 5, 1));

    let deletions = server.await_recorded("deleteMessages", 1).await;
    assert_eq!(deletions.len(), 1, "one delivery, one request");
    assert_eq!(deletions[0].body["chat_id"], json!(chat));
    assert_eq!(
        deleted_ids(&deletions[0]),
        vec![1],
        "the request names the message the platform answered her send with"
    );
    assert_eq!(
        server.recorded("sendMessage").len(),
        1,
        "the retraction is silent: nothing goes out for it"
    );
}

/// Criterion 4, the chunked case: an answer the platform took as two
/// messages is retracted whole from a reply to either of them, in one
/// request naming both — taking back only the replied-to chunk would leave
/// the group reading the remainder of a retracted answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_chunked_answer_is_retracted_whole_from_a_reply_to_its_second_chunk() {
    let chat = -701;
    let fixture = start_assistant().await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[(5, "administrator")]);
    // The scripted answer echoes the ask, so an ask this long pushes the
    // answer past one message's cap and the platform takes it as two.
    server.push_update(support::mention_update(1, chat, 7, &"x".repeat(5000)));

    let state = TempStateFile::new("retraction-chunked");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(sends.len(), 2, "non-vacuity: the answer went out as two");
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_receipts(&fixture.store, conversation, 2).await;

    // The reply points at the SECOND chunk: the retraction is keyed on the
    // delivery, so any chunk of an answer names the whole answer.
    server.push_update(del_reply_to_bot(2, chat, 5, 2));

    let deletions = server.await_recorded("deleteMessages", 1).await;
    assert_eq!(
        deleted_ids(&deletions[0]),
        vec![1, 2],
        "both messages of the send go, in send order"
    );
}

/// Criterion 5, past the platform's own range: a delivery of a hundred
/// and one messages goes out as successive requests of at most a hundred
/// identifiers, and no larger list is ever assembled.
///
/// The ledger state is built by this test on purpose. Reaching a hundred and
/// one chunks through the send path would mean pushing more than four
/// hundred thousand characters through it, which asserts the batching no
/// better and runs the suite far slower.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delivery_past_the_platforms_range_goes_out_in_batches_of_a_hundred() {
    let chat = -702;
    let fixture = start_assistant().await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[(5, "administrator")]);
    server.push_update(support::group_update(1, chat, 7, "chatter"));

    let state = TempStateFile::new("retraction-batched");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    let conversation = await_conversations(&fixture.store, 1).await[0];

    // A hundred and one delivered messages under one delivery key, written
    // straight onto the ledger.
    let key = "9000";
    for offset in 0..101 {
        let origin = (9000 + offset).to_string();
        fixture
            .store
            .append_consumer_block(
                conversation,
                None,
                DELIVERED_KIND,
                Delivered::stored_fields(&origin, key, None),
                None,
            )
            .await
            .expect("the constructed receipt appends");
    }

    server.push_update(del_reply_to_bot(2, chat, 5, 9050));

    let deletions = server.await_recorded("deleteMessages", 2).await;
    assert_eq!(deletions.len(), 2, "a hundred and one ids, two requests");
    assert_eq!(
        deleted_ids(&deletions[0]).len(),
        100,
        "the first request carries the platform's own maximum"
    );
    assert_eq!(
        deleted_ids(&deletions[1]),
        vec![9100],
        "the remainder follows in its own request"
    );
    assert_eq!(
        deleted_ids(&deletions[0]).first().copied(),
        Some(9000),
        "the batches walk the recorded origins in order"
    );
}

/// Criterion 10: a deletion the platform refuses — the 48-hour window, or a message
/// somebody else already deleted — is logged and dropped. The retraction
/// stays on the ledger, the answer is still out of the assistant's own
/// reading, and the update batch carries on with the next message.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_deletion_leaves_the_retraction_standing_and_the_batch_running() {
    let chat = -703;
    let fixture = start_assistant().await;
    fixture.hold_closing_turns();
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[(5, "administrator")]);
    server.fail_deletions();
    server.push_update(private_update(1, 5, "unused"));
    server.push_update(support::mention_update(2, chat, 7, "where did it move?"));

    let state = TempStateFile::new("retraction-refused");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    server.await_recorded("sendMessage", 2).await;
    let group = *await_conversations(&fixture.store, 2)
        .await
        .last()
        .expect("the group conversation exists");
    let receipts = await_receipts(&fixture.store, group, 1).await;
    let retracted_answer = receipts[0]
        .answer_block
        .expect("the receipt names its answer");
    // A delivered answer can still have unfinished closing notes. The hold
    // prevents their completion until the retraction interrupts the turn.
    await_ledger(&fixture.store, group, "the held closing notes", |blocks| {
        blocks
            .iter()
            .any(|block| block.block_type == Streaming::KINDS[0])
    })
    .await;
    let mut events = fixture.bus.subscribe();
    server.push_update(del_reply_to_bot(3, chat, 5, 2));
    let deletions = server.await_recorded("deleteMessages", 1).await;
    assert_eq!(deletions.len(), 1);
    assert_eq!(deletions[0].body["chat_id"], json!(chat));
    assert_eq!(
        deleted_ids(&deletions[0]),
        vec![2],
        "the whole delivery is requested"
    );
    let mut interrupted = false;
    loop {
        match events.try_recv() {
            Ok(agent_ledger::CoreEvent::InterruptRequested { conversation_id })
                if conversation_id == group =>
            {
                interrupted = true;
            }
            Ok(_) => {}
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Lagged(count)) => {
                panic!("interrupt evidence lost: event history lagged by {count} events");
            }
            Err(TryRecvError::Closed) => {
                panic!("interrupt evidence unavailable: event history closed");
            }
        }
    }
    assert!(
        interrupted,
        "the held source turn was interrupted for the fork"
    );

    // The successor has no introduction left, and the following update owes
    // its own answer. Command retention is checked on its stored row below.
    server.push_update(support::mention_update(4, chat, 7, "still there?"));
    let sends = server.await_recorded("sendMessage", 3).await;
    assert_eq!(
        sends[2].body["text"],
        json!(first_answer_to(&format!(
            "@{} still there?",
            support::BOT_USERNAME
        ))),
        "the update behind the refused deletion is answered as usual"
    );

    // The retraction stands, and the fork ran: the channel is served from a
    // conversation the retracted answer is not in, and the one it replaced
    // is retired.
    let forked = mapped_conversation(&fixture.store, chat).await;
    assert_ne!(forked, group, "the fork moved the channel");
    let blocks = fixture
        .store
        .list_blocks(forked)
        .await
        .expect("the ledger reads");
    let retractions: Vec<_> = blocks
        .iter()
        .filter(|block| block.block_type == RETRACTION_KIND)
        .map(Retraction::parse)
        .collect();
    assert_eq!(
        retractions.len(),
        1,
        "refusal leaves the retraction standing"
    );
    assert_eq!(retractions[0].delivery.as_deref(), Some("2"));
    assert!(
        blocks.iter().all(|block| block.id != retracted_answer),
        "the serving fork excludes the retracted answer"
    );
    let command_origin = message_id_of(3).to_string();
    let commands: Vec<_> = blocks
        .iter()
        .filter(|block| block.block_type == ChatMessage::KINDS[0])
        .map(ChatMessage::parse)
        .filter(|message| message.origin.as_deref() == Some(command_origin.as_str()))
        .collect();
    assert_eq!(
        commands.len(),
        1,
        "the serving successor retains the command row"
    );
    assert_eq!(commands[0].text.as_deref(), Some("/del"));
    assert_eq!(commands[0].limited, Some(LimitedBy::Command));
    assert_eq!(commands[0].answer_due, Some(false));
    assert_eq!(sends[2].body["chat_id"], json!(chat));
}

/// The conversation one chat maps to right now, read raw — the fact the fork
/// changes, so nothing here infers it from a conversation count.
async fn mapped_conversation(store: &agent_ledger::Store, chat_id: i64) -> i64 {
    let channel = chat_id.to_string();
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                "SELECT conversation_id FROM channels WHERE channel = ?1",
                [channel],
                |row| row.get::<_, i64>(0),
            )
            .ok())
    })
    .await
    .expect("the mapping reads")
    .expect("the chat is mapped")
}
