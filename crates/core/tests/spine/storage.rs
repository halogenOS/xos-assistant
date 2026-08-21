//! The composed kind and the store (AC2): the descriptor path opens,
//! validation passes, and a reopened file-backed store proves the durable
//! registry path.

use agent_ledger::{Agency, Awaiting, Block, BlockKind, FromBlock, Projection, Role, Store};
use assistant_core::Authority;
use assistant_core::kind::{AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE};
use assistant_core::schema::store_config;
use serde_json::json;

use crate::support;
use crate::support::TempDb;

/// The derive's composition, pinned without a runtime: the assistant's kind
/// parses through its own parse, a framework kind resolves through the
/// delegate, and the concatenated descriptor set is exactly the leaf's
/// declaration.
#[test]
fn the_composed_kind_parses_and_declares_one_descriptor() {
    let stored = Block {
        id: 7,
        role: Some(Role::User),
        block_type: CHAT_MESSAGE_KIND.into(),
        created_at: String::new(),
        fields: {
            let mut fields = serde_json::Map::new();
            fields.insert("text".into(), json!("hello there"));
            fields.insert("principal_id".into(), json!(3));
            fields.insert("authority".into(), json!("moderator"));
            fields.insert("origin".into(), json!("m-1"));
            fields.insert("sent_at".into(), json!("2026-08-21T00:00:00+00:00"));
            fields
        },
    };
    match AssistantKind::from_block(&stored) {
        AssistantKind::ChatMessage(message) => {
            assert_eq!(message.text.as_deref(), Some("hello there"));
            assert_eq!(message.principal_id, Some(3));
            assert_eq!(message.authority, Some(Authority::Moderator));
            assert_eq!(message.origin.as_deref(), Some("m-1"));
            assert_eq!(
                message.sent_at.as_deref(),
                Some("2026-08-21T00:00:00+00:00")
            );
            assert_eq!(message.awaiting(), Some(Awaiting::Model));
        }
        AssistantKind::Core(_) => panic!("the assistant's kind resolved through the delegate"),
    }

    // A block with no stored text is an erased message: it awaits nothing
    // and projects nothing, while its provenance fields still parse.
    let erased = Block {
        fields: {
            let mut fields = serde_json::Map::new();
            fields.insert("principal_id".into(), json!(3));
            fields.insert("authority".into(), json!("moderator"));
            fields.insert("sent_at".into(), json!("2026-08-21T00:00:00+00:00"));
            fields
        },
        ..stored.clone()
    };
    match AssistantKind::from_block(&erased) {
        AssistantKind::ChatMessage(message) => {
            assert_eq!(message.text, None);
            assert_eq!(
                message.awaiting(),
                None,
                "an erased message summons no turn"
            );
            assert_eq!(message.group_role(), None);
            assert_eq!(message.llm_parts(), None);
            assert_eq!(message.llm_text(), None);
        }
        AssistantKind::Core(_) => panic!("the erased row resolved through the delegate"),
    }

    let framework_kind = Block {
        block_type: "text".into(),
        ..stored
    };
    assert!(
        matches!(
            AssistantKind::from_block(&framework_kind),
            AssistantKind::Core(BlockKind::Text(_))
        ),
        "a framework kind resolves through the delegate, untouched"
    );

    assert_eq!(AssistantKind::DESCRIPTORS.len(), 1);
    assert_eq!(AssistantKind::DESCRIPTORS[0].table, CHAT_MESSAGE_TABLE);
    agent_ledger::agency::check_descriptor_durability::<AssistantKind>(AssistantKind::DESCRIPTORS)
        .expect("durable() and the descriptor's ephemerality are one fact");
}

/// AC2: the store opens with the descriptor and the domain migrations,
/// validation passes, and a REOPENED file-backed store loads the stored row
/// back through the descriptor path — the durable registry, proven on disk.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_file_backed_store_reopens_and_loads_the_stored_kind() {
    let db = TempDb::new("reopen");

    let appended;
    let conversation;
    {
        let store =
            Store::open_with(db.path(), store_config()).expect("the configured store opens");
        assert!(
            store.content_tables().contains(&CHAT_MESSAGE_TABLE),
            "the descriptor's table joins the one content-table list"
        );
        conversation = store
            .create_conversation(
                "scripted-1".into(),
                "script-model".into(),
                "Script Model".into(),
                support::VENDOR.into(),
            )
            .await
            .expect("a conversation row");
        let mut fields = serde_json::Map::new();
        fields.insert("text".into(), json!("a durable message"));
        fields.insert("principal_id".into(), json!(1));
        fields.insert("authority".into(), json!("member"));
        fields.insert("sent_at".into(), json!("2026-08-21T00:00:00+00:00"));
        appended = store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                fields,
                None,
            )
            .await
            .expect("the consumer write path appends");
        // The first store closes before the reopen, so the reopen reads what
        // the disk holds, not what a live connection still shares.
    }

    let reopened = Store::open_with(db.path(), store_config()).expect("the store reopens");
    let blocks = reopened
        .list_blocks(conversation)
        .await
        .expect("the reopened ledger reads");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id, appended);
    assert_eq!(blocks[0].block_type, CHAT_MESSAGE_KIND);
    assert_eq!(blocks[0].role, Some(Role::User));
    assert_eq!(blocks[0].fields["text"], json!("a durable message"));

    match AssistantKind::from_block(&blocks[0]) {
        AssistantKind::ChatMessage(message) => {
            assert_eq!(message.text.as_deref(), Some("a durable message"));
            assert_eq!(message.principal_id, Some(1));
            assert_eq!(message.authority, Some(Authority::Member));
            assert_eq!(message.origin, None);
            assert_eq!(
                message.sent_at.as_deref(),
                Some("2026-08-21T00:00:00+00:00")
            );
        }
        AssistantKind::Core(_) => panic!("the reopened row resolved through the delegate"),
    }
}
