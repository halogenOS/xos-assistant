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
    TempStateFile, authorize_group, await_chat_messages, await_conversations, await_state_file,
    date_of, group_update, message_id_of, message_update, private_update, recording_sleep,
    spawn_adapter, start_assistant,
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
    authorize_group(&fixture.assistant, -70).await;
    authorize_group(&fixture.assistant, -71).await;
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
    authorize_group(&fixture.assistant, chat).await;
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
    authorize_group(&fixture.assistant, SKIP_TEST_CHAT).await;
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

/// AC2 over the wire: an edit of a recorded message records a SECOND row
/// naming the message it revises, carrying its own origin, its sender, the
/// reply target and the edit time as the stored send time — and the first
/// row stands untouched beside it. The decision-0017 deferral falls due
/// here: the update was skipped, and now it records.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_records_a_second_row_naming_what_it_revises() {
    let chat = -91;
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    // The original, replying to an earlier message so the revision's own
    // reply fact is provably carried across too.
    server.push_update(json!({
        "update_id": 1,
        "message": {
            "message_id": message_id_of(1),
            "date": date_of(1),
            "chat": { "id": chat, "type": "group" },
            "from": { "id": 7, "first_name": "Person 7", "username": "casey" },
            "reply_to_message": { "message_id": 4242, "from": { "id": 8 } },
            "text": "what is the reelase cadence?",
        },
    }));

    let state = TempStateFile::new("edit-records");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let first = await_chat_messages(&fixture.store, conversation, 1).await;
    let original = first[0].fields.clone();

    // The edit: the same message id, the corrected text, and the platform's
    // own edit time beside the original send time.
    server.push_update(json!({
        "update_id": 2,
        "edited_message": {
            "message_id": message_id_of(1),
            "date": date_of(1),
            "edit_date": date_of(50),
            "chat": { "id": chat, "type": "group" },
            "from": { "id": 7, "first_name": "Person 7", "username": "casey" },
            "reply_to_message": { "message_id": 4242, "from": { "id": 8 } },
            "text": "what is the release cadence?",
        },
    }));

    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(
        messages[0].fields, original,
        "the earlier version's row is untouched in every column"
    );
    let revision = &messages[1].fields;
    assert_eq!(revision["text"], json!("what is the release cadence?"));
    assert_eq!(
        revision["revises"],
        json!(
            messages[0].fields["origin"]
                .as_str()
                .expect("the original stored an origin")
        ),
        "the revision names the message it supersedes"
    );
    assert_eq!(
        revision["origin"], messages[0].fields["origin"],
        "an edit arrives under the original's own id on this platform"
    );
    assert_eq!(revision["speaker"], json!("casey"));
    assert_eq!(revision["authority"], json!("member"));
    assert_eq!(
        revision["reply_target"], messages[0].fields["reply_target"],
        "the revision reports the reply fact like any message"
    );
    assert_eq!(
        revision["sent_at"],
        json!(
            chrono::DateTime::from_timestamp(date_of(50), 0)
                .expect("a representable edit time")
                .to_rfc3339()
        ),
        "the edit time is the version's send time"
    );
}

/// The edit time's fallback, pinned over the wire: an edit update that
/// decodes without an edit time records under the ORIGINAL send time
/// instead of being refused a timestamp. The platform documents the edit
/// time on the message an edit update carries — the fallback is this
/// decoder's leniency about an optional field, never a platform case, and
/// the row it writes still names what it revises.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_without_an_edit_time_records_the_original_send_time() {
    let chat = -92;
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(json!({
        "update_id": 1,
        "message": {
            "message_id": message_id_of(1),
            "date": date_of(1),
            "chat": { "id": chat, "type": "group" },
            "from": { "id": 7, "first_name": "Person 7" },
            "text": "the first wording",
        },
    }));

    let state = TempStateFile::new("edit-without-edit-date");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;

    server.push_update(json!({
        "update_id": 2,
        "edited_message": {
            "message_id": message_id_of(1),
            "date": date_of(1),
            "chat": { "id": chat, "type": "group" },
            "from": { "id": 7, "first_name": "Person 7" },
            "text": "the corrected wording",
        },
    }));

    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    let revision = &messages[1].fields;
    assert_eq!(revision["text"], json!("the corrected wording"));
    assert_eq!(
        revision["revises"], messages[0].fields["origin"],
        "the revision still names the message it supersedes"
    );
    assert_eq!(
        revision["sent_at"], messages[0].fields["sent_at"],
        "with no edit time reported, the original send time stands"
    );
}

/// AC8 over the wire: an edit that leaves a message with neither text nor
/// caption — a member deleting a photo's caption — records nothing and is
/// acknowledged, and the earlier version's row stands untouched beside the
/// nothing it wrote.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_that_leaves_no_text_records_nothing_and_is_acknowledged() {
    let chat = -93;
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(json!({
        "update_id": 1,
        "message": {
            "message_id": message_id_of(1),
            "date": date_of(1),
            "chat": { "id": chat, "type": "group" },
            "from": { "id": 3, "first_name": "Person 3" },
            "caption": "the caption a member then deletes",
            "photo": [ { "file_id": "irrelevant" } ],
        },
    }));

    let state = TempStateFile::new("skip-textless-edit");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let before = await_chat_messages(&fixture.store, conversation, 1).await;
    let original = before[0].fields.clone();

    server.push_update(json!({
        "update_id": 2,
        "edited_message": {
            "message_id": message_id_of(1),
            "date": date_of(1),
            "edit_date": date_of(50),
            "chat": { "id": chat, "type": "group" },
            "from": { "id": 3, "first_name": "Person 3" },
            "photo": [ { "file_id": "irrelevant" } ],
        },
    }));

    // The acknowledgment is the offset advancing past the skipped update.
    await_state_file(state.path(), 3).await;
    let after = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(after.len(), 1, "the textless edit recorded nothing");
    assert_eq!(
        after[0].fields, original,
        "the earlier version's row stands untouched in every column"
    );
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

/// An update carrying nothing this adapter consumes — here a poll — is
/// skipped per decision 0017 as a non-message update.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_non_message_update_is_skipped_and_acknowledged() {
    assert_skipped_and_acknowledged(
        "skip-non-message",
        json!({
            "update_id": 1,
            "poll": { "id": "irrelevant" },
        }),
    )
    .await;
}
