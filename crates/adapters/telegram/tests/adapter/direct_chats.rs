//! The direct-chat switch over the wire (decision 0069): with the core
//! configured off, a private-chat update is acknowledged past — the offset
//! advances and the loop stays healthy — while nothing is sent, nothing is
//! left, and the store holds no conversation for it.

use std::sync::Arc;

use crate::server::BotApiServer;
use crate::support::{
    await_state_file, private_update, recording_sleep, spawn_adapter, start_assistant_direct_off,
};

/// A direct message under off, end to end through the adapter: the update
/// is confirmed — the persisted offset moves past it, twice, so the first
/// disregard provably did not halt the batch — no send goes out, no leave
/// call fires (there is no group to leave), and the store stays empty of
/// conversations. The offset advancing while the store stays empty is the
/// wire-level shape of "refused before anything is written".
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_message_under_off_advances_the_offset_and_touches_nothing() {
    let fixture = start_assistant_direct_off().await;
    let server = BotApiServer::start().await;
    let state = crate::support::TempStateFile::new("direct-off");
    server.push_update(private_update(30, 5, "a first quiet ask"));

    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    await_state_file(state.path(), 31).await;

    // The loop is not wedged behind the disregarded update: the next one
    // is confirmed the same way.
    server.push_update(private_update(31, 5, "a second quiet ask"));
    await_state_file(state.path(), 32).await;

    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "no conversation exists for the disregarded direct messages"
    );
    assert!(
        server.recorded("sendMessage").is_empty(),
        "nothing was sent to the chat"
    );
    assert!(
        server.recorded("leaveChat").is_empty(),
        "the disregard carries no directive — no leave call fires"
    );
}
