//! AC2: the whole round trip — a scripted group-message update, through the
//! real core, out to the scripted server's `sendMessage` — asserted on the
//! server's recorded requests and on the ledger.

use std::sync::Arc;

use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, TempStateFile, answer_to, authorize_group, await_chat_messages, await_conversations,
    date_of, first_answer_to, group_update, message_id_of, recording_sleep, spawn_adapter,
    start_assistant,
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

    // The reply reaches the scripted server, bound to the chat it answers —
    // the asker's first answer ever, so it opens with the disclosure line
    // (unit 12, AC2's wire half).
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(sends[0].body["text"], json!(first_answer_to(&asked)));

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

    // The sent message stands in the ledger too — the outgoing block a
    // sending tool filed, carrying the words the chat received, disclosure
    // line and all. Her turn's own text is private notes from unit 55 on,
    // so what went out lives here and nowhere else.
    let deadline = std::time::Instant::now() + support::DEADLINE;
    loop {
        let blocks = fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        if blocks.iter().any(|block| {
            block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND
                && block.fields[assistant_core::outgoing::COLUMN_TEXT]
                    == json!(first_answer_to(&asked))
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the sent message's block"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // The authority came from the wire, not from a default.
    assert!(
        !server.recorded("getChatAdministrators").is_empty(),
        "a group message resolves authority through the administrator list"
    );

    // The same person's second ask over the wire: the answer arrives bare —
    // the introduction rode the first answer and never repeats.
    let again = format!("@{} And the second question?", support::BOT_USERNAME);
    support::await_quiet(&fixture.store).await;
    server.push_update(group_update(2, chat, 7, &again));
    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(sends[1].body["chat_id"], json!(chat));
    assert_eq!(sends[1].body["text"], json!(answer_to(&again)));

    // Titles are off (decision 0077): the whole answered round trip
    // dispatched no derivation. The window one would fire in is held open
    // before the count is read.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        fixture
            .title_requests
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a full conversation flow dispatches zero title requests"
    );
}
