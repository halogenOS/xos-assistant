//! AC2: the whole round trip — a scripted group-message update, through the
//! real core, out to the scripted server's `sendMessage` — asserted on the
//! server's recorded requests and on the ledger.

use std::sync::Arc;

use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, TempStateFile, answer_to, authorize_group, await_chat_messages, await_conversations,
    date_of, group_update, message_id_of, recording_sleep, spawn_adapter, start_assistant,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_group_message_round_trips_to_a_send() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -100_200_300;
    // The mention is what addresses the assistant in a group; the recorded
    // text keeps the mention, so the scripted answer derives from it too.
    let asked = format!("@{} What is the release cadence?", support::BOT_USERNAME);
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(group_update(1, chat, 7, &asked));

    let state = TempStateFile::new("e2e");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The reply reaches the scripted server, bound to the chat it answers.
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(sends[0].body["text"], json!(answer_to(&asked)));

    // The ledger holds the recorded message with the translated fields.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["text"], json!(asked.clone()));
    assert_eq!(messages[0].fields["authority"], json!("member"));
    assert_eq!(
        messages[0].fields["origin"],
        json!(message_id_of(1).to_string())
    );
    // The platform's send date becomes the message timestamp: date_of(1)
    // is unix second 1_700_000_001, and the core records it in RFC 3339 —
    // this exact string, not merely some present value.
    assert_eq!(
        date_of(1),
        1_700_000_001,
        "the asserted instant names the builder's date"
    );
    assert_eq!(
        messages[0].fields["sent_at"],
        json!("2023-11-14T22:13:21+00:00")
    );

    // The finalized answer stands in the ledger too.
    let deadline = std::time::Instant::now() + support::DEADLINE;
    loop {
        let blocks = fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        if blocks.iter().any(|block| {
            block.block_type == "text" && block.fields["content"] == json!(answer_to(&asked))
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the finalized answer block"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // The authority came from the wire, not from a default.
    assert!(
        !server.recorded("getChatAdministrators").is_empty(),
        "a group message resolves authority through the administrator list"
    );
}
