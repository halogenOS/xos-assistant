//! AC3: translation pinned per decision — chat kinds, authority mapping,
//! the caption fallback, and every named skip case — driven through the
//! public wire, asserted on the ledger and the persisted offset.
//!
//! Channel kinds are proven through the core's own contract: a channel
//! mapped under one kind refuses a message claiming the other, and the
//! refusal is deterministic — logged and acknowledged past — so the second
//! message's absence plus the advanced offset is the proof of what the
//! first message stored.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::server::BotApiServer;
use crate::support::{
    TempStateFile, await_chat_messages, await_conversations, await_state_file, group_update,
    message_id_of, message_update, private_update, recording_sleep, spawn_adapter, start_assistant,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_private_chat_maps_direct_and_its_sender_is_a_member() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 9, "hello from a direct chat"));

    let state = TempStateFile::new("private-direct");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["authority"], json!("member"));

    // The same chat id claimed as a group is refused deterministically and
    // acknowledged past: the stored kind was direct.
    server.push_update(group_update(2, 9, 9, "the same channel claimed as a group"));
    await_state_file(state.path(), 3).await;
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(
        messages.len(),
        1,
        "the mismatched claim is refused, not recorded"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn group_and_supergroup_both_map_group() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.set_admins(-70, &[]);
    server.set_admins(-71, &[]);
    server.push_update(message_update(1, "group", -70, 5, "in a group"));
    server.push_update(message_update(2, "supergroup", -71, 5, "in a supergroup"));

    let state = TempStateFile::new("group-kinds");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversations = await_conversations(&fixture.store, 2).await;
    for conversation in &conversations {
        await_chat_messages(&fixture.store, *conversation, 1).await;
    }

    // Both chat ids claimed as private chats are refused: both stored group.
    server.push_update(message_update(3, "private", -70, 5, "claimed direct"));
    server.push_update(message_update(4, "private", -71, 5, "claimed direct"));
    await_state_file(state.path(), 5).await;
    for conversation in &conversations {
        let messages = await_chat_messages(&fixture.store, *conversation, 1).await;
        assert_eq!(messages.len(), 1, "the mismatched claims are not recorded");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn member_statuses_translate_to_authorities() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -80;
    server.set_admins(chat, &[(1, "creator"), (2, "administrator")]);
    server.push_update(group_update(1, chat, 1, "from the creator"));
    server.push_update(group_update(2, chat, 2, "from an administrator"));
    server.push_update(group_update(3, chat, 3, "from a plain member"));

    let state = TempStateFile::new("authorities");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 3).await;
    let authority_of = |text: &str| -> Value {
        messages
            .iter()
            .find(|block| block.fields["text"] == json!(text))
            .unwrap_or_else(|| panic!("the message {text:?} is recorded"))
            .fields["authority"]
            .clone()
    };
    assert_eq!(authority_of("from the creator"), json!("admin"));
    assert_eq!(authority_of("from an administrator"), json!("moderator"));
    assert_eq!(authority_of("from a plain member"), json!("member"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_caption_is_the_fallback_text() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.push_update(json!({
        "update_id": 1,
        "message": {
            "message_id": message_id_of(1),
            "date": 1_700_000_001,
            "chat": { "id": 9, "type": "private" },
            "from": { "id": 9, "first_name": "Person 9" },
            "caption": "a captioned photo",
            "photo": [ { "file_id": "irrelevant" } ],
        },
    }));

    let state = TempStateFile::new("caption");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["text"], json!("a captioned photo"));
}

/// The chat every skip test's recordable message is said in.
const SKIP_TEST_CHAT: i64 = -90;

/// One skip case's shared pin: the update is acknowledged past — the offset
/// ends beyond it — and nothing of it reaches the ledger. A recordable group
/// message pushed behind it proves both at once: the offset reaching past
/// both updates shows the skip did not halt the batch, and the conversation
/// holding exactly that one message shows the skip recorded nothing.
async fn assert_skipped_and_acknowledged(name: &str, skipped: Value) {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.set_admins(SKIP_TEST_CHAT, &[]);
    server.push_update(skipped);
    server.push_update(group_update(2, SKIP_TEST_CHAT, 3, "the recordable message"));

    let state = TempStateFile::new(name);
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    await_state_file(state.path(), 3).await;
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["text"], json!("the recordable message"));
    assert_eq!(
        await_conversations(&fixture.store, 1).await.len(),
        1,
        "the skipped update minted no conversation"
    );
}

/// A channel broadcast, as the platform sends it: a `channel_post` update
/// with no `message` field. The client's update model carries no such field
/// on purpose, so the update decodes as a non-message update and is skipped;
/// the pin here is the wire outcome the loop contract promises — the
/// broadcast is acknowledged past and nothing of it is recorded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_channel_post_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-channel-post",
        json!({
            "update_id": 1,
            "channel_post": {
                "message_id": message_id_of(1),
                "date": 1_700_000_001,
                "chat": { "id": -500, "type": "channel" },
                "text": "a channel broadcast",
            },
        }),
    )
    .await;
}

/// A message on behalf of a chat — an anonymous administrator posting as
/// the group — is skipped per decision 0016.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_on_behalf_of_a_chat_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-sender-chat",
        json!({
            "update_id": 1,
            "message": {
                "message_id": message_id_of(1),
                "date": 1_700_000_001,
                "chat": { "id": SKIP_TEST_CHAT, "type": "group" },
                "from": { "id": 1_087_968_824, "first_name": "Group" },
                "sender_chat": { "id": SKIP_TEST_CHAT, "type": "group" },
                "text": "posted as the group",
            },
        }),
    )
    .await;
}

/// An edit to an existing message is skipped per decision 0017: the ledger
/// keeps the message as first seen.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edited_message_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-edit",
        json!({
            "update_id": 1,
            "edited_message": {
                "message_id": message_id_of(1),
                "date": 1_700_000_001,
                "chat": { "id": SKIP_TEST_CHAT, "type": "group" },
                "from": { "id": 3, "first_name": "Person 3" },
                "text": "an edited statement",
            },
        }),
    )
    .await;
}

/// A message with neither text nor caption is skipped per decision 0017.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_with_neither_text_nor_caption_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-no-text",
        json!({
            "update_id": 1,
            "message": {
                "message_id": message_id_of(1),
                "date": 1_700_000_001,
                "chat": { "id": SKIP_TEST_CHAT, "type": "group" },
                "from": { "id": 4, "first_name": "Person 4" },
                "sticker": { "file_id": "irrelevant" },
            },
        }),
    )
    .await;
}

/// A message whose chat type is the broadcast kind is skipped by the
/// chat-kind branch, even though the platform delivers broadcasts as
/// channel posts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_broadcast_shaped_message_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-broadcast-shaped",
        json!({
            "update_id": 1,
            "message": {
                "message_id": message_id_of(1),
                "date": 1_700_000_001,
                "chat": { "id": -501, "type": "channel" },
                "from": { "id": 5, "first_name": "Person 5" },
                "text": "a broadcast-shaped message",
            },
        }),
    )
    .await;
}

/// A non-message update — here a membership change — is skipped per
/// decision 0017.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_non_message_update_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-non-message",
        json!({
            "update_id": 1,
            "my_chat_member": { "chat": { "id": SKIP_TEST_CHAT, "type": "group" } },
        }),
    )
    .await;
}
