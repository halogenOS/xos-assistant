//! The send path's platform contracts. AC5: the rate-limit reply is honored
//! through the injectable sleep — the stated wait, the three-attempt bound,
//! and the drop past it — with no real waiting anywhere; the same contract
//! on the poll side; and the message cap, under which a long reply is
//! delivered as chunks instead of refused whole.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    TempStateFile, answer_to, await_chat_messages, await_conversations, private_update,
    recording_sleep, spawn_adapter, start_assistant,
};

/// Two rate-limited attempts, then success: each refusal hands its stated
/// wait to the injectable sleep, and the third attempt delivers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rate_limited_send_waits_the_stated_time_and_retries() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_sends(7, 2);
    server.push_update(private_update(1, 5, "kept through the retries"));

    let state = TempStateFile::new("rate-limit-retry");
    let (sleep, waits) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 3).await;
    assert_eq!(sends.len(), 3, "two refusals, then the delivering attempt");
    for send in &sends {
        assert_eq!(
            send.body["text"],
            json!(answer_to("kept through the retries"))
        );
    }
    assert_eq!(
        *waits.lock().expect("the wait log locks"),
        vec![Duration::from_secs(7), Duration::from_secs(7)],
        "each refusal's stated wait went to the sleep, and nothing else waited"
    );
}

/// Every attempt rate-limited: the bound is spent, the reply is dropped, and
/// the consumer moves on — the next chat's reply still sends.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn past_the_bound_the_reply_is_dropped_and_the_consumer_moves_on() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_sends(7, 3);
    server.push_update(private_update(1, 5, "the dropped answer's cause"));

    let state = TempStateFile::new("rate-limit-drop");
    let (sleep, waits) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Three attempts, all refused; the drop takes no further wait.
    let sends = server.await_recorded("sendMessage", 3).await;
    assert_eq!(sends.len(), 3, "the bound is three attempts");
    assert_eq!(
        *waits.lock().expect("the wait log locks"),
        vec![Duration::from_secs(7), Duration::from_secs(7)],
        "two waits between three attempts; the drop does not wait again"
    );

    // A second chat's message: its reply sends, so the dropped reply did not
    // wedge the consumer.
    server.push_update(private_update(2, 6, "after the drop"));
    let sends = server.await_recorded("sendMessage", 4).await;
    assert_eq!(sends[3].body["text"], json!(answer_to("after the drop")));
    assert_eq!(sends[3].body["chat_id"], json!(6));
}

/// A stated wait past the honored ceiling is not waited out: the send fails
/// on the spot — the wait is never handed to the sleep — and the consumer
/// moves on, instead of parking every later reply behind one flooded chat.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stated_wait_past_the_ceiling_drops_the_reply_without_waiting() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_sends(3600, 1);
    server.push_update(private_update(1, 5, "the flooded reply's cause"));

    let state = TempStateFile::new("over-ceiling-wait");
    let (sleep, waits) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The consumer moves on past the refused wait: the next chat's reply
    // sends, and honoring the hour-long wait would have retried the first
    // reply here instead.
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(5));
    server.push_update(private_update(2, 6, "after the refused wait"));
    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(
        sends[1].body["text"],
        json!(answer_to("after the refused wait"))
    );
    assert_eq!(sends[1].body["chat_id"], json!(6));
    assert!(
        !waits
            .lock()
            .expect("the wait log locks")
            .contains(&Duration::from_hours(1)),
        "the over-ceiling wait was never handed to the sleep"
    );
}

/// The rate-limit contract binds every endpoint, not only the send: a
/// rate-limited poll hands the stated wait to the injectable sleep and
/// retries, instead of falling back to the generic backoff while the
/// limiter is asking for longer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rate_limited_poll_waits_the_stated_time_and_retries() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_polls(5, 1);
    server.push_update(private_update(1, 5, "past the limited poll"));

    let state = TempStateFile::new("rate-limit-poll");
    let (sleep, waits) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The message arrives only through the retried poll.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;
    assert!(
        waits
            .lock()
            .expect("the wait log locks")
            .contains(&Duration::from_secs(5)),
        "the limited poll's stated wait went to the sleep"
    );
}

/// The send's flood-wait ceiling does not bind the poll: a poll whose
/// stated wait is far past the ceiling still hands that wait to the sleep
/// and retries — re-asking a limiter early would amplify the load being
/// limited, and the poll parks no queue of replies behind it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_poll_honors_a_stated_wait_past_the_send_ceiling() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_polls(3600, 1);
    server.push_update(private_update(1, 5, "past the flood-limited poll"));

    let state = TempStateFile::new("flood-limit-poll");
    let (sleep, waits) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;
    assert!(
        waits
            .lock()
            .expect("the wait log locks")
            .contains(&Duration::from_hours(1)),
        "the flood wait went to the sleep in full; the ceiling is the send's alone"
    );
}

/// A reply past the platform's message cap is delivered whole, as
/// consecutive chunks each within the cap — not refused by the platform and
/// dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_past_the_message_cap_is_sent_in_chunks() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    // The scripted answer echoes the ask, so an ask this long pushes the
    // answer past one message's 4096 UTF-16 code units.
    let long_ask = "x".repeat(5000);
    server.push_update(private_update(1, 5, &long_ask));

    let state = TempStateFile::new("long-reply");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(sends.len(), 2, "one over-cap reply, two chunks");
    let mut delivered = String::new();
    for send in &sends {
        let chunk = send.body["text"].as_str().expect("the chunk is text");
        assert!(
            chunk.encode_utf16().count() <= 4096,
            "every chunk stays within the platform's cap"
        );
        assert_eq!(send.body["chat_id"], json!(5));
        delivered.push_str(chunk);
    }
    assert_eq!(
        delivered,
        answer_to(&long_ask),
        "the chunks concatenate to the whole reply, in order"
    );
}

/// A failure on a later chunk ends the reply at the last delivered chunk,
/// per decision 0019: the chunks before the failing one are in the chat, and
/// the tail past a lost middle is never sent — a spliced statement must not
/// reach the chat. The failure leaves one chunk delivered, which is the
/// cut-short outcome the consumer logs (asserted in the token-scan binary,
/// which owns the process-wide capture); here the wire pins the sends. The
/// follow-up reply proves the tail's absence: the next send on the wire is
/// the new reply, not the abandoned third chunk a keep-sending mutation
/// would emit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_later_chunk_ends_the_reply_and_the_tail_is_never_sent() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    // Three chunks: the echoed answer runs just past two 4096-unit caps.
    let long_ask = "x".repeat(9000);
    server.script_send_failure_after(1);
    server.push_update(private_update(1, 5, &long_ask));

    let state = TempStateFile::new("chunk-failure");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The first chunk delivered; the second was attempted and refused.
    let sends = server.await_recorded("sendMessage", 2).await;
    let whole = answer_to(&long_ask);
    let head: String = sends
        .iter()
        .take(2)
        .map(|send| send.body["text"].as_str().expect("the chunk is text"))
        .collect();
    assert!(
        whole.starts_with(&head),
        "the first two sends are the reply's chunks, in order"
    );
    assert!(
        head.len() < whole.len(),
        "a tail past the failing chunk exists, so its absence below means something"
    );

    // The next send is the follow-up's reply: the abandoned tail never went
    // out after the failure.
    server.push_update(private_update(2, 5, "after the cut"));
    let sends = server.await_recorded("sendMessage", 3).await;
    assert_eq!(
        sends[2].body["text"],
        json!(answer_to("after the cut")),
        "the send after the failure is the next reply, not the dropped tail"
    );
}

/// The cap is counted in UTF-16 code units — the platform's measure — not in
/// characters. Pinned with surrogate-pair characters, each one `char` but two
/// code units: this answer is about 2500 characters yet about 5000 units, so
/// an accounting that counted characters would send it as a single message
/// the platform refuses, and this test would see one over-cap send instead
/// of two bounded ones.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_chunk_cap_counts_utf16_units_not_characters() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let long_ask = "\u{1F642}".repeat(2500);
    server.push_update(private_update(1, 5, &long_ask));

    let state = TempStateFile::new("surrogate-chunks");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(sends.len(), 2, "about 5000 units splits into two chunks");
    let mut delivered = String::new();
    for send in &sends {
        let chunk = send.body["text"].as_str().expect("the chunk is text");
        assert!(
            chunk.encode_utf16().count() <= 4096,
            "every chunk stays within the cap in the platform's own measure"
        );
        assert_eq!(send.body["chat_id"], json!(5));
        delivered.push_str(chunk);
    }
    assert_eq!(
        delivered,
        answer_to(&long_ask),
        "the chunks concatenate to the whole reply with no character torn apart"
    );
}
