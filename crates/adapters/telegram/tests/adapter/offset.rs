//! AC4: offset persistence — the restart that re-ingests nothing, the
//! crash-window redelivery whose duplicate is the accepted outcome, the
//! batch that fails midway and persists up to the last success, and the
//! malformed state file that reads as absent.

use std::sync::Arc;

use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    TempStateFile, await_chat_messages, await_conversations, await_state_file, group_update,
    private_update, recording_sleep, spawn_adapter, start_assistant,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restarted_adapter_does_not_reingest_acknowledged_updates() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("restart");
    server.push_update(private_update(10, 3, "the first run's message"));

    let (sleep, _) = recording_sleep();
    let adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;
    await_state_file(state.path(), 11).await;
    drop(adapter);

    // The restarted adapter, same state file: the acknowledged update stays
    // acknowledged, and only the new one is ingested.
    server.push_update(private_update(11, 3, "the second run's message"));
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(messages[0].fields["text"], json!("the first run's message"));
    assert_eq!(
        messages[1].fields["text"],
        json!("the second run's message")
    );
    await_state_file(state.path(), 12).await;
}

/// The crash window, simulated exactly as a crash leaves it: the message is
/// ingested, the offset write never happened — the state file still names
/// the old offset and the platform still holds the update. The redelivered
/// update is ingested again; the duplicate is the accepted outcome, per
/// decision 0014.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crash_between_ingest_and_offset_write_redelivers_the_accepted_duplicate() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("crash-window");
    let redelivered = private_update(20, 4, "said exactly once");
    server.push_update(redelivered.clone());

    let (sleep, _) = recording_sleep();
    let adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;
    await_state_file(state.path(), 21).await;
    drop(adapter);

    // Reconstruct the crash's aftermath on a fresh scripted server: the
    // state file still names the old offset and the platform still holds
    // the update. Fresh on purpose — the earlier run's last poll may still
    // be on the old server's wire after the abort, and at arrival it would
    // confirm away the update this restarted run must receive.
    drop(server);
    std::fs::write(state.path(), "20").expect("the pre-crash state file writes");
    let server = BotApiServer::start().await;
    server.push_update(redelivered);
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(
        messages[0].fields["origin"], messages[1].fields["origin"],
        "the redelivered update is recorded twice — the accepted duplicate"
    );
    await_state_file(state.path(), 21).await;
}

/// A batch that fails midway persists the offset up to the last success, and
/// the failed update redelivers once the cause clears. The failure here is a
/// failed administrator-list fetch, which by the spec fails that message's
/// ingest transiently — authority is never silently defaulted.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_midway_failure_persists_up_to_the_last_success_and_redelivers() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("midway");
    let chat = -110;
    server.fail_admins(chat);
    server.push_update(private_update(30, 6, "recorded before the failure"));
    server.push_update(group_update(31, chat, 6, "held back by the failure"));

    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The batch stopped at the failure: the offset names the last success.
    await_state_file(state.path(), 31).await;
    let direct_conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, direct_conversation, 1).await;
    assert_eq!(
        messages[0].fields["text"],
        json!("recorded before the failure")
    );
    assert!(
        !server.recorded("getChatAdministrators").is_empty(),
        "the failure was the administrator fetch, exercised on the wire"
    );

    // The cause clears; the held-back update redelivers and is recorded.
    server.set_admins(chat, &[]);
    await_state_file(state.path(), 32).await;
    let conversations = await_conversations(&fixture.store, 2).await;
    let group_conversation = conversations[1];
    let messages = await_chat_messages(&fixture.store, group_conversation, 1).await;
    assert_eq!(
        messages[0].fields["text"],
        json!("held back by the failure")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_malformed_state_file_reads_as_absent() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("malformed");
    std::fs::write(state.path(), "not an offset").expect("the malformed file writes");
    server.push_update(private_update(40, 8, "after the malformed read"));

    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(
        messages[0].fields["text"],
        json!("after the malformed read")
    );
    await_state_file(state.path(), 41).await;
}
