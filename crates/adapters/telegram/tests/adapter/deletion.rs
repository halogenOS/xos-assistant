//! The deletion mirror over the wire (unit 13, AC2): a group
//! administrator replies to a recorded message with the moderation bot's
//! own deletion command — the adapter reports the bare command with the
//! reply beside the verbatim text, the core nulls the stored row, and
//! nothing is sent for it: the only send on the wire is a later canary's
//! answer.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::server::BotApiServer;
use crate::support::{
    self, BOT_USERNAME, TempStateFile, authorize_group, await_chat_messages, await_conversations,
    date_of, first_answer_to, message_id_of, recording_sleep, spawn_adapter, start_assistant,
};

/// A group message replying to ANOTHER member's message and carrying the
/// moderation bot's deletion command — unaddressed on purpose: the admin
/// is talking to the moderation bot, not to the assistant.
fn del_reply_update(
    update_id: i64,
    chat_id: i64,
    admin_id: i64,
    replied_message_id: i64,
    replied_author_id: i64,
) -> Value {
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": "group" },
            "from": { "id": admin_id, "first_name": format!("Person {admin_id}") },
            "text": "/del",
            "reply_to_message": {
                "message_id": replied_message_id,
                "date": date_of(update_id) - 10,
                "chat": { "id": chat_id, "type": "group" },
                "from": {
                    "id": replied_author_id,
                    "first_name": format!("Person {replied_author_id}")
                },
                "text": "an offending line",
            },
        },
    })
}

/// The mirror end to end: the administrator's reply `/del` nulls the
/// stored row — the platform's `administrator` status resolves inside the
/// administrator set per decision 0015 — the command row is recorded with
/// the command stamp, and the wire carries exactly one send: the canary's
/// answer, whose projection shows the erased marker in place of the
/// deleted prose.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admins_reply_deletion_erases_the_row_and_sends_nothing() {
    let chat = -600;
    let fixture = start_assistant().await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[(5, "administrator")]);

    server.push_update(support::group_update(1, chat, 900, "an offending line"));
    server.push_update(del_reply_update(2, chat, 5, message_id_of(1), 900));
    server.push_update(support::mention_update(3, chat, 7, "is that gone now?"));

    let state = TempStateFile::new("deletion-mirror");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // One send only: the canary's answer. The updates are polled in order,
    // so anything the mirror sent would have been recorded ahead of it.
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to(&format!(
            "[message erased]\n\n/del\n\n@{BOT_USERNAME} is that gone now?"
        ))),
        "the canary's answer projects the erased marker and the verbatim command"
    );
    assert_eq!(
        server.recorded("sendMessage").len(),
        1,
        "the mirror itself sent nothing"
    );

    // The store: the offending row is nulled, the command row stands.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 3).await;
    assert_eq!(
        messages[0].fields.get("text"),
        None,
        "the offending row's prose is nulled"
    );
    assert_eq!(
        messages[0].fields.get("origin"),
        None,
        "the offending row's origin reference is nulled"
    );
    assert_eq!(
        messages[0].fields.get("sent_at"),
        None,
        "the offending row's platform send time is nulled"
    );
    assert_eq!(
        messages[1].fields["text"],
        json!("/del"),
        "the command row records the request verbatim"
    );
    assert_eq!(
        messages[1].fields["limited"],
        json!("command"),
        "the command stamp keeps the mirror out of the answer machinery"
    );
    assert_eq!(
        messages[1].fields["reply_target"],
        json!(message_id_of(1).to_string()),
        "the command row names the deleted message — the lawful record"
    );
}
