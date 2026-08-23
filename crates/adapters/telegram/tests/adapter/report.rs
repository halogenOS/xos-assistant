//! AC4 over the adapter: a member replies to an offending message and asks
//! for a report — the tool files it, and the wire carries the fixed line as
//! a platform reply to the offending message's id, before the answer, with
//! send-without-reply tolerance stated on the request itself.

use std::sync::Arc;

use assistant_core::tools::ToolSet;
use assistant_core::tools::report;
use serde_json::{Value, json};

use crate::server::BotApiServer;
use crate::support::{
    self, BOT_USERNAME, MODERATION_HANDLE, TOOL_CLOSING_ANSWER, TempStateFile, ToolScript,
    authorize_group, await_conversations, date_of, message_id_of, recording_sleep, spawn_adapter,
    start_assistant_moderating,
};

/// A group message replying to ANOTHER member's message while mentioning
/// the bot — the report ask's wire shape: addressed by mention, replying
/// to the offending line.
fn reply_ask_update(
    update_id: i64,
    chat_id: i64,
    asker_id: i64,
    replied_message_id: i64,
    replied_author_id: i64,
) -> Value {
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": "group" },
            "from": { "id": asker_id, "first_name": format!("Person {asker_id}") },
            "text": format!("@{BOT_USERNAME} please report this"),
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

/// The whole report round trip over the wire: the spam line is recorded,
/// the member's reply ask files the report, and the wire shows the fixed
/// line sent as a platform reply — the current reply parameters, the
/// offending message's id, send-without-reply tolerance — BEFORE the
/// answer, which itself carries no reply parameters.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_ask_files_and_the_wire_threads_the_report_before_the_answer() {
    let chat = -500;
    let fixture = start_assistant_moderating(
        Some(ToolScript {
            tool: report::NAME.into(),
            input: "{}".into(),
            narration: None,
        }),
        ToolSet::new(),
        Some(MODERATION_HANDLE.into()),
    )
    .await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);

    // The offending line arrives first, unaddressed and recorded; the
    // member's reply ask follows.
    server.push_update(support::group_update(1, chat, 900, "an offending line"));
    server.push_update(reply_ask_update(2, chat, 7, message_id_of(1), 900));

    let state = TempStateFile::new("report-e2e");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Two sends: the threaded report line first, then the plain answer.
    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(format!("/report@{MODERATION_HANDLE}")),
        "the fixed line names the configured handle"
    );
    assert_eq!(
        sends[0].body["reply_parameters"]["message_id"],
        json!(message_id_of(1)),
        "the report threads onto the offending message"
    );
    assert_eq!(
        sends[0].body["reply_parameters"]["allow_sending_without_reply"],
        json!(true),
        "a deleted target degrades to a plain send"
    );
    assert_eq!(sends[1].body["text"], json!(TOOL_CLOSING_ANSWER));
    assert_eq!(
        sends[1].body.get("reply_parameters"),
        None,
        "the answer stays unthreaded"
    );

    // The ledger: the report block names the offending message's origin
    // and its sender's principal, and carries the fixed line.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let stored = blocks
        .iter()
        .find(|block| block.block_type == "report")
        .expect("the report block stands");
    assert_eq!(
        stored.fields["target_origin"],
        json!(message_id_of(1).to_string())
    );
    assert_eq!(
        stored.fields["line"],
        json!(format!("/report@{MODERATION_HANDLE}"))
    );
    let offense = blocks
        .iter()
        .find(|block| {
            block.block_type == "chat_message"
                && block.fields.get("text") == Some(&json!("an offending line"))
        })
        .expect("the offending row stands");
    assert_eq!(
        stored.fields["reported_principal_id"], offense.fields["principal_id"],
        "the block names the offending message's sender"
    );
}

/// An over-cap reply threads only its FIRST chunk (2026-08-23): a reply
/// longer than the platform's message cap goes out in chunks, and the
/// wire shows the reply parameters — target id, send-without-reply
/// tolerance — on the first chunk alone, with every continuation sent
/// plain. Driven end to end through a report line whose configured handle
/// pushes it past the cap; the fixture takes that liberty because the
/// report line is the one reply the core threads.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_over_cap_reply_threads_only_its_first_chunk() {
    // 4096 UTF-16 code units is the platform's cap on one message.
    let over_cap_handle = "m".repeat(5000);
    let line = format!("/report@{over_cap_handle}");
    let chat = -501;
    let fixture = start_assistant_moderating(
        Some(ToolScript {
            tool: report::NAME.into(),
            input: "{}".into(),
            narration: None,
        }),
        ToolSet::new(),
        Some(over_cap_handle),
    )
    .await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);

    server.push_update(support::group_update(1, chat, 900, "an offending line"));
    server.push_update(reply_ask_update(2, chat, 7, message_id_of(1), 900));

    let state = TempStateFile::new("report-chunking");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Three sends: the report line's two chunks, then the plain answer.
    let sends = server.await_recorded("sendMessage", 3).await;
    assert_eq!(
        sends[0].body["reply_parameters"],
        json!({ "message_id": message_id_of(1), "allow_sending_without_reply": true }),
        "the first chunk threads onto the offending message"
    );
    assert_eq!(
        sends[1].body.get("reply_parameters"),
        None,
        "the continuation chunk sends plain"
    );
    let first = sends[0].body["text"]
        .as_str()
        .expect("the first chunk is text");
    let second = sends[1].body["text"]
        .as_str()
        .expect("the second chunk is text");
    assert_eq!(
        first.encode_utf16().count(),
        4096,
        "the first chunk fills the platform cap exactly"
    );
    assert_eq!(
        format!("{first}{second}"),
        line,
        "the chunks reassemble into the whole report line"
    );
    assert_eq!(sends[2].body["text"], json!(TOOL_CLOSING_ANSWER));
    assert_eq!(
        sends[2].body.get("reply_parameters"),
        None,
        "the answer stays unthreaded"
    );
}
