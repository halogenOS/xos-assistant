//! The manual-quote fact across the adapter thread (unit 31, AC9): a reply
//! whose payload carries a hand-selected excerpt reaches the core with that
//! excerpt intact, and the core narrows the quote to it.
//!
//! The pin is the whole thread at once — decode, translate, the intake's
//! copy into the core's inbound message — because those three steps have no
//! observable seam between them and a fact dropped at any of them is a fact
//! the model never reads. The decoder's own half is pinned against raw
//! platform JSON in `client.rs`, per the convention stated there.

use std::sync::Arc;

use agent_ledger::LeafKind;
use serde_json::{Value, json};

use crate::server::BotApiServer;
use crate::support::{
    self, BOT_USERNAME, TempStateFile, authorize_group, await_conversations, date_of,
    first_answer_to, message_id_of, recording_sleep, spawn_adapter, start_assistant,
};

/// A group message replying to another member's message, addressed through
/// the mention rule, and carrying the platform's quoted part: the excerpt's
/// text, its UTF-16 offset — which nothing in this workspace reads — and
/// the hand-selected flag.
///
/// The replied-to message arrives as one value — its id, its author and its
/// text — because those three are one fact about one message.
fn quoted_reply_update(
    update_id: i64,
    chat_id: i64,
    user_id: i64,
    replied: (i64, i64, &str),
    excerpt: &str,
    text: &str,
) -> Value {
    let (replied_message_id, replied_author_id, replied_text) = replied;
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": "group" },
            "from": { "id": user_id, "first_name": format!("Person {user_id}") },
            "text": format!("@{BOT_USERNAME} {text}"),
            "reply_to_message": {
                "message_id": replied_message_id,
                "date": date_of(update_id) - 10,
                "chat": { "id": chat_id, "type": "group" },
                "from": {
                    "id": replied_author_id,
                    "first_name": format!("Person {replied_author_id}")
                },
                "text": replied_text,
            },
            "quote": {
                "text": excerpt,
                "position": 4,
                "is_manual": true,
            },
        },
    })
}

/// The hand-selected excerpt survives the whole thread: the reply's answer
/// is derived from a projection carrying exactly the quoted words,
/// `> `-prefixed above the reply — which is only possible if the excerpt
/// was decoded from the payload, translated, copied into the core's
/// message, and used to narrow the span.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hand_selected_excerpt_reaches_the_core_and_narrows_the_quote() {
    let chat = -610;
    let fixture = start_assistant().await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[]);

    server.push_update(support::group_update(
        1,
        chat,
        900,
        "die Größe — the text font tiring my eyes",
    ));
    server.push_update(quoted_reply_update(
        2,
        chat,
        7,
        (
            message_id_of(1),
            900,
            "die Größe — the text font tiring my eyes",
        ),
        "the text font",
        "which one?",
    ));

    let state = TempStateFile::new("quoted-reply");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(first_answer_to(&format!(
            "die Größe — the text font tiring my eyes\n\n\
             &gt; the text font\n\n\
             @{BOT_USERNAME} which one?"
        ))),
        "the excerpt reached the core intact and narrowed the quote to it — \
         located by searching the stored text, so the multibyte characters \
         ahead of it never became a byte offset. The quote marker arrives \
         escaped because the scripted answer quotes the projection back and \
         the send escapes the text it carries, exactly as it does for any \
         answer"
    );

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let quotes = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == agent_ledger::agency::Quote::KINDS[0])
        .count();
    assert_eq!(quotes, 1, "one reply, one quote block");
}
