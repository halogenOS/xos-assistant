//! The session reset over the wire (unit 45, AC2): a moderator's `/wipe`
//! is answered with the core's own fixed line, and the reset directive
//! riding the ingestion voids the chat's once-per-process lookup memory —
//! so the chat's next contact looks it up again and the fresh session gets
//! the group's title and rules back.

use std::sync::Arc;

use assistant_core::commands::WIPE_COMMAND;
use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, TempStateFile, authorize_group, recording_sleep, spawn_adapter, start_assistant,
};

/// The chat's first contact is looked up once; the wipe voids that memory,
/// and the next message looks the chat up afresh.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_moderators_wipe_answers_its_line_and_re_enriches_the_chat() {
    let chat = -700;
    let fixture = start_assistant().await;
    authorize_group(&fixture.assistant, chat).await;
    let server = BotApiServer::start().await;
    server.set_chat_info(chat, "The kernel room", None);
    server.set_admins(chat, &[(5, "administrator")]);

    server.push_update(support::group_update(1, chat, 900, "hello"));
    server.push_update(support::group_update(2, chat, 5, WIPE_COMMAND));
    server.push_update(support::group_update(3, chat, 900, "starting over"));

    let state = TempStateFile::new("session-wipe");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::commands::WIPE_DONE),
        "the core's own fixed line reaches the chat"
    );

    // Two lookups: the chat's first contact, then the one the reset
    // directive forced by voiding the memory. Nothing else in this run
    // clears it.
    let lookups = server.await_recorded("getChat", 2).await;
    assert_eq!(lookups.len(), 2);
    assert!(
        lookups
            .iter()
            .all(|request| request.body["chat_id"] == json!(chat)),
        "both lookups name the wiped chat"
    );
}
