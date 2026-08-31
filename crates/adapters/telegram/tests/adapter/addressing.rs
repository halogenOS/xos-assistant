//! Addressing end to end over the public wire: the identity-first contract,
//! the three addressed shapes each answered, the unaddressed message that
//! rests into the next context, the debt that survives a trailing
//! unaddressed message, and the failed turn's silence with the addressed
//! re-engagement.

use std::sync::Arc;

use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, BOT_USERNAME, TempStateFile, answer_to, authorize_group, await_chat_messages,
    await_conversations, await_receipts, first_answer_to, group_update, mention_update,
    private_update, recording_sleep, reply_to_bot_update, spawn_adapter, start_assistant,
};

/// The identity comes before the first poll: while `getMe` fails, the
/// adapter retries it on the poll backoff and never asks for updates; once
/// the identity answers, polling begins and the queued message is answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_poll_happens_before_the_identity_answers() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start_without_identity().await;
    server.push_update(private_update(1, 5, "queued behind the identity"));

    let state = TempStateFile::new("identity-first");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The identity fetch is retried; no update poll goes out meanwhile.
    server.await_recorded("getMe", 2).await;
    assert!(
        server.recorded("getUpdates").is_empty(),
        "no message is translated before the identity is known"
    );

    server.set_me(crate::support::BOT_ID, BOT_USERNAME);
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to("queued behind the identity"))
    );
}

/// The three addressed shapes, each answered: a direct message, a group
/// mention, and a group reply to one of the assistant's own messages.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_mention_and_reply_to_assistant_are_each_answered() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let group = -400;
    server.set_admins(group, &[]);
    authorize_group(&fixture.assistant, group).await;
    server.push_update(private_update(1, 5, "the direct ask"));
    server.push_update(mention_update(2, group, 6, "the mentioned ask"));

    let state = TempStateFile::new("addressed-shapes");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 2).await;
    let texts: Vec<String> = sends
        .iter()
        .map(|send| send.body["text"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(texts.contains(&first_answer_to("the direct ask")));
    let mentioned = format!("@{BOT_USERNAME} the mentioned ask");
    assert!(texts.contains(&first_answer_to(&mentioned)));

    // The reply-to-assistant shape, on the settled group: the reply opens a
    // fresh user group behind the stored answer, so the projected tail — and
    // with it the scripted answer — is the reply's own text.
    server.push_update(reply_to_bot_update(3, group, 6, "the replied ask"));
    let sends = server.await_recorded("sendMessage", 3).await;
    // The same person's second answer arrives bare: the introduction
    // already rode the mentioned ask's answer.
    assert_eq!(sends[2].body["text"], json!(answer_to("the replied ask")));
    assert_eq!(sends[2].body["chat_id"], json!(group));
}

/// An unaddressed group message is recorded and rests — no send — and then
/// appears in the next addressed turn's context: the group's memory is the
/// product.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unaddressed_group_message_rests_and_joins_the_next_context() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let group = -401;
    server.set_admins(group, &[]);
    authorize_group(&fixture.assistant, group).await;
    server.push_update(group_update(1, group, 7, "a resting remark"));

    let state = TempStateFile::new("resting-group");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Recorded, resting: the ledger holds it, nothing was sent.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["addressed"], json!(false));
    assert_eq!(messages[0].fields["answer_due"], json!(false));
    assert!(
        server.recorded("sendMessage").is_empty(),
        "an unaddressed message is not answered"
    );

    // The next addressed message is answered out of a context carrying the
    // resting remark.
    server.push_update(mention_update(2, group, 7, "now a question"));
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to(&format!(
            "a resting remark\n\n@{BOT_USERNAME} now a question"
        )))
    );
}

/// The name trigger over the wire (unit 14): a group message naming the
/// assistant — its wake name as one whole word, in any casing — summons
/// exactly one answer, while a longer word merely containing the name
/// rests like any unaddressed remark.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_group_message_naming_the_assistant_summons_one_answer() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let group = -405;
    server.set_admins(group, &[]);
    authorize_group(&fixture.assistant, group).await;
    // The longer word rests; the lowercased whole-word name summons.
    server.push_update(group_update(1, group, 7, "the fixtures are elsewhere"));
    server.push_update(group_update(
        2,
        group,
        7,
        "fixture, which kernel does it run?",
    ));

    let state = TempStateFile::new("name-trigger");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to(
            "the fixtures are elsewhere\n\nfixture, which kernel does it run?"
        )),
        "the naming message summons the turn; the longer word only rested \
         into its context"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(
        messages[0].fields["addressed"],
        json!(false),
        "a longer word containing the name does not address"
    );
    assert_eq!(messages[1].fields["addressed"], json!(true));
}

/// The punctuated-name fallback over the wire (unit 14): a configured name
/// that cannot form a clean trigger word wakes nothing — the naming text
/// rests — while the mention keeps addressing exactly as before.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_punctuated_name_falls_back_to_mention_and_reply() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let group = -406;
    server.set_admins(group, &[]);
    authorize_group(&fixture.assistant, group).await;
    server.push_update(group_update(1, group, 7, "Fix ture! are you around?"));

    let state = TempStateFile::new("punctuated-name");
    let (sleep, _) = recording_sleep();
    let _adapter = crate::support::spawn_adapter_named(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        Some("Fix ture!"),
    );

    // Recorded, resting: no trigger formed, nothing was sent.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["addressed"], json!(false));
    assert!(
        server.recorded("sendMessage").is_empty(),
        "a punctuated name wakes nothing"
    );

    // The mention still addresses: the fallback is mention-and-reply.
    server.push_update(mention_update(2, group, 7, "now a question"));
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to(&format!(
            "Fix ture! are you around?\n\n@{BOT_USERNAME} now a question"
        )))
    );
}

/// An unaddressed message arriving after an addressed one does not cancel
/// the owed answer: both land in one poll batch, and the addressed
/// message's answer still arrives.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_trailing_unaddressed_message_does_not_cancel_the_owed_answer() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let group = -402;
    server.set_admins(group, &[]);
    authorize_group(&fixture.assistant, group).await;
    server.push_update(mention_update(1, group, 7, "the owed ask"));
    server.push_update(group_update(2, group, 8, "an aside right behind it"));

    let state = TempStateFile::new("no-cancel");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    let answered = sends[0].body["text"].as_str().unwrap_or_default();
    assert!(
        answered.contains("the owed ask"),
        "the owed answer arrived despite the trailing aside: {answered}"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 2).await;
}

/// A failed turn reaches the chat with nothing at all, and the next
/// addressed message re-engages the conversation (unit 49).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_turn_sends_nothing_and_the_chat_recovers() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let mut events = fixture.bus.subscribe();
    fixture
        .failures
        .store(1, std::sync::atomic::Ordering::SeqCst);
    server.push_update(private_update(1, 5, "the failing ask"));

    let state = TempStateFile::new("silent-failure");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The failed turn's own settling, awaited on the latch it closes. The
    // latch orders nothing about the outbound edge's arm; the empty wire is
    // decided by the arm having no send left in it and by this turn having
    // stored nothing to deliver. The wait itself keeps the second update
    // from overtaking the first turn.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    support::await_failure_latch(&mut events, conversation).await;
    assert!(
        server.recorded("sendMessage").is_empty(),
        "the failed turn put nothing on the wire"
    );

    // The next message from the same chat is addressed — a direct chat
    // always is — so it unlatches and gets answered.
    server.push_update(private_update(2, 5, "asking again"));
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(5));
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to("the failing ask\n\nasking again")),
        "the answer is the chat's first message: the dead turn recorded no \
         block of its own between the two asks, so they stand as one user \
         run and the answer covers both"
    );

    // The delivery of that answer is recorded like any other send.
    await_receipts(&fixture.store, conversation, 1).await;
}
