//! The join notice over the public wire (unit 36, AC2): a join service
//! message decoded from the scripted platform lands one marked block per
//! joiner, the assistant's own entry among them is dropped while the
//! co-joiners stand, and a join in a group the operator never admitted
//! stores nothing and draws the standing leave call.

use std::sync::Arc;

use agent_ledger::{Block, Store};
use serde_json::{Value, json};

use crate::server::BotApiServer;
use crate::support::{
    self, BOT_ID, TempStateFile, authorize_group, await_conversations, date_of, message_id_of,
    recording_sleep, spawn_adapter, start_assistant,
};

/// One person a join note names, as the wire carries them: the account id,
/// the handle, the first name and the last name, each absent where the
/// platform sends none.
struct WireJoiner<'a> {
    id: i64,
    handle: Option<&'a str>,
    first: Option<&'a str>,
    last: Option<&'a str>,
}

/// One join service message: the given chat's join note naming the given
/// people — the wire shape the platform sends when someone walks in.
fn join_update(update_id: i64, chat_id: i64, joiners: &[WireJoiner<'_>]) -> Value {
    let members: Vec<Value> = joiners
        .iter()
        .map(|joiner| {
            let mut member = json!({ "id": joiner.id, "is_bot": false });
            if let Some(handle) = joiner.handle {
                member["username"] = json!(handle);
            }
            if let Some(first) = joiner.first {
                member["first_name"] = json!(first);
            }
            if let Some(last) = joiner.last {
                member["last_name"] = json!(last);
            }
            member
        })
        .collect();
    json!({
        "update_id": update_id,
        "message": {
            "message_id": message_id_of(update_id),
            "date": date_of(update_id),
            "chat": { "id": chat_id, "type": "supergroup" },
            "new_chat_members": members,
        },
    })
}

/// The join blocks of one conversation, awaited so the poll's own timing
/// is never assumed.
async fn await_joins(store: &Store, conversation_id: i64, count: usize) -> Vec<Block> {
    let deadline = std::time::Instant::now() + support::DEADLINE;
    loop {
        let joins: Vec<Block> = store
            .list_blocks(conversation_id)
            .await
            .expect("the ledger reads")
            .into_iter()
            .filter(|block| block.block_type == assistant_core::join::JOIN_NOTICE_KIND)
            .collect();
        if joins.len() >= count {
            assert_eq!(joins.len(), count, "more join blocks than the test expects");
            return joins;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {count} join blocks; have {}",
            joins.len()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// A join over the wire, in an admitted group: two people walk in beside
/// the assistant's own entry, and the ledger holds one block per PERSON —
/// her own dropped, the co-joiners' names and handles decoded, the service
/// message's id shared by both.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_join_over_the_wire_records_one_block_per_person() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -650;
    server.set_admins(chat, &[]);
    server.set_chat_info(chat, "The kernel room", None);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(join_update(
        1,
        chat,
        &[
            WireJoiner {
                id: BOT_ID,
                handle: Some(support::BOT_USERNAME),
                first: Some("Fixture"),
                last: None,
            },
            WireJoiner {
                id: 4001,
                handle: Some("ada"),
                first: Some("Ada"),
                last: Some("Lovelace"),
            },
            WireJoiner {
                id: 4002,
                handle: None,
                first: Some("Grace"),
                last: None,
            },
        ],
    ));

    let state = TempStateFile::new("wire-join");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let joins = await_joins(&fixture.store, conversation, 2).await;
    // What each PERSON stored, read by name instead of by position: the
    // platform's order is the platform's business, and a pin that reads it
    // as a contract fails the day the wire reorders.
    let stored: Vec<(String, Option<String>)> = joins
        .iter()
        .map(|join| {
            (
                join.fields["name"].as_str().expect("a shown name").into(),
                join.fields
                    .get("handle")
                    .map(|handle| handle.as_str().expect("a stored handle").to_owned()),
            )
        })
        .collect();
    assert!(
        stored.contains(&("Ada Lovelace".to_owned(), Some("ada".to_owned()))),
        "the platform's own name composition reaches the ledger: {stored:?}"
    );
    assert!(
        stored.contains(&("Grace".to_owned(), None)),
        "a joiner the platform gave no handle stores none: {stored:?}"
    );
    for join in &joins {
        assert_eq!(
            join.fields["origin"],
            json!(message_id_of(1).to_string()),
            "both blocks name the one service message"
        );
        assert!(
            join.fields["principal_id"].as_i64().is_some(),
            "a joiner resolves a principal like any member"
        );
    }
    assert!(
        server.recorded("leaveChat").is_empty(),
        "a join in an admitted group is no reason to leave"
    );
}

/// A join in a group the operator never admitted stores nothing and draws
/// the standing leave call — the authorization gate's existing full
/// answer, with a join as one more trigger of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_join_in_an_unadmitted_group_stores_nothing_over_the_wire() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -651;
    server.set_chat_info(chat, "A stranger group", None);
    server.push_update(join_update(
        1,
        chat,
        &[WireJoiner {
            id: 4003,
            handle: Some("ada"),
            first: Some("Ada"),
            last: None,
        }],
    ));

    let state = TempStateFile::new("wire-join-stranger");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let leaves = server.await_recorded("leaveChat", 1).await;
    assert_eq!(leaves[0].body["chat_id"], json!(chat));
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "the refused join touched no ledger"
    );
}
