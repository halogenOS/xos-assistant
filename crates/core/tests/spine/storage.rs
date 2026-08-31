//! The composed kind and the store (AC2): the descriptor path opens,
//! validation passes, and a reopened file-backed store proves the durable
//! registry path.

use agent_ledger::{
    Agency, Awaiting, Block, BlockKind, ContentPart, FromBlock, Projection, Role, Store,
};
use assistant_core::Authority;
use assistant_core::kind::{
    AssistantKind, CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ERASED_MARKER, FrameworkKind,
};
use assistant_core::schema::store_config;
use assistant_core::{ChannelKind, IngestOutcome};
use serde_json::json;

use crate::support;
use crate::support::TempDb;

/// One stored chat-message block with the given content fields — the shared
/// shape of the runtime-free parse pins below.
fn chat_block(fields: serde_json::Map<String, serde_json::Value>) -> Block {
    Block {
        id: 7,
        role: Some(Role::User),
        block_type: CHAT_MESSAGE_KIND.into(),
        created_at: String::new(),
        dispatch_anchor: None,
        fields,
    }
}

/// The derive's composition, pinned without a runtime: the assistant's kind
/// parses through its own parse, a framework kind resolves through the
/// delegate, and the concatenated descriptor set is exactly the leaf's
/// declaration.
#[test]
fn the_composed_kind_parses_and_declares_one_descriptor() {
    let stored = chat_block({
        let mut fields = serde_json::Map::new();
        fields.insert("text".into(), json!("hello there"));
        fields.insert("principal_id".into(), json!(3));
        fields.insert("authority".into(), json!("moderator"));
        fields.insert("origin".into(), json!("m-1"));
        fields.insert("sent_at".into(), json!("2026-08-21T00:00:00+00:00"));
        fields.insert("addressed".into(), json!(true));
        fields.insert("answer_due".into(), json!(true));
        fields
    });
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
            assert_eq!(message.addressed, Some(true));
            assert_eq!(message.answer_due, Some(true));
            assert_eq!(message.awaiting(), Some(Awaiting::Model));
        }
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::Report(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_) => {
            panic!("the assistant's kind resolved through the delegate")
        }
    }

    let framework_kind = Block {
        block_type: "text".into(),
        ..stored
    };
    assert!(
        matches!(
            AssistantKind::from_block(&framework_kind),
            AssistantKind::Core(FrameworkKind(BlockKind::Text(_)))
        ),
        "a framework kind resolves through the delegate, untouched"
    );

    assert_eq!(AssistantKind::DESCRIPTORS.len(), 7);
    assert_eq!(AssistantKind::DESCRIPTORS[0].table, CHAT_MESSAGE_TABLE);
    assert_eq!(
        AssistantKind::DESCRIPTORS[1].table,
        assistant_core::tools::palette::TOOL_PALETTE_TABLE
    );
    assert_eq!(
        AssistantKind::DESCRIPTORS[2].table,
        assistant_core::note::CONTEXT_NOTE_TABLE
    );
    assert_eq!(
        AssistantKind::DESCRIPTORS[3].table,
        assistant_core::join::JOIN_NOTICE_TABLE
    );
    assert_eq!(
        AssistantKind::DESCRIPTORS[4].table,
        assistant_core::tools::report::REPORT_TABLE
    );
    assert_eq!(
        AssistantKind::DESCRIPTORS[5].table,
        assistant_core::delivery::DELIVERED_TABLE
    );
    assert_eq!(
        AssistantKind::DESCRIPTORS[6].table,
        assistant_core::tools::mark::MESSAGE_MARK_TABLE,
        "the newest kind's descriptor is last, as its migration step is"
    );
    agent_ledger::agency::check_descriptor_durability::<AssistantKind>(AssistantKind::DESCRIPTORS)
        .expect("durable() and the descriptor's ephemerality are one fact");
}

/// The two non-awaiting shapes, pinned without a runtime: a resting message
/// projects but summons no turn, and an erased message summons nothing and
/// projects only the fixed marker while keeping its stored voice for the
/// grouping pass.
#[test]
fn resting_and_erased_messages_summon_no_turn() {
    // A recorded message whose stamp owes no answer rests: it awaits
    // nothing, while still projecting its text into the context.
    let resting = chat_block({
        let mut fields = serde_json::Map::new();
        fields.insert("text".into(), json!("a resting group message"));
        fields.insert("principal_id".into(), json!(3));
        fields.insert("authority".into(), json!("member"));
        fields.insert("sent_at".into(), json!("2026-08-21T00:00:00+00:00"));
        fields.insert("addressed".into(), json!(false));
        fields.insert("answer_due".into(), json!(false));
        fields
    });
    match AssistantKind::from_block(&resting) {
        AssistantKind::ChatMessage(message) => {
            assert_eq!(
                message.awaiting(),
                None,
                "a resting message summons no turn"
            );
            assert!(
                message.llm_text().is_some(),
                "a resting message still projects"
            );
        }
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::Report(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_) => {
            panic!("the resting row resolved through the delegate")
        }
    }

    // A block with no stored text is an erased message: it awaits nothing
    // and projects only the fixed marker — none of the prose — while its
    // provenance fields still parse.
    let erased = chat_block({
        let mut fields = serde_json::Map::new();
        fields.insert("principal_id".into(), json!(3));
        fields.insert("authority".into(), json!("moderator"));
        fields.insert("addressed".into(), json!(true));
        fields.insert("answer_due".into(), json!(true));
        fields
    });
    match AssistantKind::from_block(&erased) {
        AssistantKind::ChatMessage(message) => {
            assert_eq!(message.text, None);
            assert_eq!(
                message.awaiting(),
                None,
                "an erased message summons no turn"
            );
            assert_eq!(
                message.group_role(),
                Some(Role::User),
                "an erased message keeps its stored voice in the grouping pass"
            );
            assert_eq!(
                message.llm_text().as_deref(),
                Some(ERASED_MARKER),
                "an erased message projects the fixed marker, never prose"
            );
            match message.llm_parts().as_deref() {
                Some([ContentPart::Text { text }]) => assert_eq!(text, ERASED_MARKER),
                other => panic!("the erased parts carry one marker part, got {other:?}"),
            }
        }
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::Report(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_) => {
            panic!("the erased row resolved through the delegate")
        }
    }
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
        fields.insert("addressed".into(), json!(true));
        fields.insert("answer_due".into(), json!(true));
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
    let blocks = support::consumer_view(
        &reopened
            .list_blocks(conversation)
            .await
            .expect("the reopened ledger reads"),
    );
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
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::Report(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_) => {
            panic!("the reopened row resolved through the delegate")
        }
    }
}

/// Decision 0077's upgrade pin: a store the username-projection unit's
/// binary wrote — the principals table still carrying its display-name
/// column with stored values, the domain's version at eleven — upgrades
/// through the appended steps past eleven. The retirement step drops the
/// column with its values, the version advances to the newest, the
/// surviving identity fields read back intact, and the write path resolves
/// principals over the upgraded table.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_version_eleven_store_upgrades_through_the_display_name_drop() {
    let db = TempDb::new("v11-upgrade");
    {
        let store =
            Store::open_with(db.path(), store_config()).expect("the configured store opens");
        // The rewind: undo exactly what the steps past version eleven do —
        // restore the column the retirement step drops, with a stored
        // value, and drop the suppression flag and the literal-addressed
        // column the later steps add — and set the version back, leaving
        // that unit's disk shape. The written value proves the drop
        // deletes data, not just an empty surface.
        agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
            conn.execute_batch(&format!(
                "ALTER TABLE principals ADD COLUMN display_name TEXT NOT NULL DEFAULT '';
                 ALTER TABLE principals DROP COLUMN opted_out;
                 DROP INDEX {revises_index};
                 ALTER TABLE block_chat_message DROP COLUMN revises;
                 ALTER TABLE block_chat_message DROP COLUMN literal_addressed;
                 DROP TABLE block_join_notice;
                 DROP TABLE block_delivered;
                 DROP TABLE block_message_mark;
                 INSERT INTO principals (adapter, external_id, display_name, username)
                     VALUES ('test-adapter', '42', 'Ada Lovelace', 'ada');",
                revises_index = assistant_core::schema::MESSAGE_REVISES_INDEX.as_str(),
            ))?;
            Ok(())
        })
        .await
        .expect("the store rewinds to the previous unit's shape");
        support::rewind_domain_migration_version(&store, 11).await;
        // The first store closes before the reopen, so the upgrade reads
        // the disk, not a live connection.
    }

    let reopened = Store::open_with(db.path(), store_config())
        .expect("the version-eleven store reopens under the shipped configuration");
    assert_eq!(
        support::domain_migration_version(&reopened).await,
        19,
        "the appended steps advanced the domain's version"
    );
    let (columns, row): (Vec<String>, (String, String, Option<String>)) =
        agent_ledger::store::domain_run(&reopened.tx(), assistant_core::schema::DOMAIN, |conn| {
            let mut statement = conn.prepare("SELECT name FROM pragma_table_info('principals')")?;
            let columns = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            let row = conn.query_row(
                "SELECT adapter, external_id, username FROM principals",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            Ok((columns, row))
        })
        .await
        .expect("the upgraded table reads");
    assert!(
        !columns.iter().any(|column| column == "display_name"),
        "the retirement step dropped the column and its stored values"
    );
    assert_eq!(
        row,
        ("test-adapter".into(), "42".into(), Some("ada".into())),
        "the surviving identity fields are intact"
    );

    // The upgraded store serves the write path: the pre-existing principal
    // resolves — not a duplicate — for the first post-upgrade message.
    let fixture = support::start_assistant_on(reopened, None).await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        support::inbound(
            &support::channel("dm-post-upgrade"),
            assistant_core::ChannelKind::Direct,
            "42",
            "the first post-upgrade ask",
        ),
    )
    .await;
    assert_eq!(
        receipt.principal_id, 1,
        "the stored principal resolved over the upgraded table"
    );
}

/// AC1 of the grounded-answer unit (unit 16): a store the previous unit's
/// binary wrote — version thirteen, its message table without the literal
/// column — upgrades cleanly through the appended literal-addressed step.
/// The column arrives with its safe default: every historical row reads
/// NULL, which the one reader folds to the silent outcome, and the recast
/// `addressed` column is untouched — a pre-upgrade row's stored summons
/// reads back exactly as it was written, so no summons reader changes
/// meaning. The pinned version is the shipped step count; a unit that
/// appends a step updates it deliberately, beside its own upgrade test.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_version_thirteen_store_upgrades_through_the_literal_addressed_step() {
    let db = TempDb::new("v13-upgrade");
    let conversation;
    {
        let store =
            Store::open_with(db.path(), store_config()).expect("the configured store opens");
        assert_eq!(
            support::domain_migration_version(&store).await,
            19,
            "the domain's recorded version is the shipped step count"
        );
        // One recorded summoned message, then the rewind: drop exactly the
        // column the literal-addressed step adds and set the version back,
        // leaving the previous unit's disk shape with a stored row in it.
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
        fields.insert("text".into(), json!("a pre-upgrade summoned ask"));
        fields.insert("principal_id".into(), json!(1));
        fields.insert("authority".into(), json!("member"));
        fields.insert("addressed".into(), json!(true));
        fields.insert("answer_due".into(), json!(true));
        store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                fields,
                None,
            )
            .await
            .expect("the pre-upgrade row appends");
        agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
            conn.execute_batch(&format!(
                "DROP INDEX {revises_index};
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN revises;
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN literal_addressed;
                 DROP TABLE {join};
                 DROP TABLE {delivered};
                 DROP TABLE {marks};",
                revises_index = assistant_core::schema::MESSAGE_REVISES_INDEX.as_str(),
                join = assistant_core::join::JOIN_NOTICE_TABLE,
                delivered = assistant_core::delivery::DELIVERED_TABLE,
                marks = assistant_core::tools::mark::MESSAGE_MARK_TABLE,
            ))?;
            Ok(())
        })
        .await
        .expect("the store rewinds to the previous unit's shape");
        support::rewind_domain_migration_version(&store, 13).await;
        // The first store closes before the reopen, so the upgrade reads
        // the disk, not a live connection.
    }

    let reopened = Store::open_with(db.path(), store_config())
        .expect("the version-thirteen store reopens under the shipped configuration");
    assert_eq!(
        support::domain_migration_version(&reopened).await,
        19,
        "the appended step advanced the domain's version"
    );
    let blocks = support::consumer_view(
        &reopened
            .list_blocks(conversation)
            .await
            .expect("the upgraded ledger reads"),
    );
    match AssistantKind::from_block(&blocks[0]) {
        AssistantKind::ChatMessage(message) => {
            assert_eq!(
                message.addressed,
                Some(true),
                "the recast summons column is untouched by the upgrade"
            );
            assert_eq!(
                message.literal_addressed, None,
                "the historical row reads the safe default: no literal value"
            );
        }
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::Report(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_) => {
            panic!("the upgraded row resolved through the delegate")
        }
    }
}

/// AC1 of the join-notice unit (unit 36): a store the previous unit's
/// binary wrote — version fourteen, its report table still carrying the
/// NOT NULL reported column, a filed report standing in it — upgrades
/// through the two appended steps. The table-recreating step is the one
/// migration in this domain that can LOSE data, so the pin is about the
/// data: the populated row survives column for column, the block still
/// resolves as a report through the loaded ledger, and the relaxed
/// constraint then accepts the filing a plural join event produces, which
/// the old shape would have refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_version_fourteen_store_upgrades_through_the_reported_nullable_step() {
    let db = TempDb::new("v14-upgrade");
    let conversation = a_populated_version_fourteen_store(&db).await;

    let reopened = Store::open_with(db.path(), store_config())
        .expect("the version-fourteen store reopens under the shipped configuration");
    assert_eq!(
        support::domain_migration_version(&reopened).await,
        19,
        "the appended steps advanced the domain's version"
    );

    // The recreated table's own shape, read back from the store: every
    // column of the shipped table in its shipped order, the reported column
    // relaxed and nothing else — the target stays nullable, the line stays
    // NOT NULL, the primary key and its cascade to the block header stand.
    let (columns, parent) = table_shape(&reopened, assistant_core::tools::report::REPORT_TABLE)
        .await
        .expect("the upgraded report table reads");
    assert_eq!(
        columns,
        vec![
            ("block_id".to_owned(), "INTEGER".to_owned(), 0, 1),
            (
                assistant_core::tools::report::COLUMN_TARGET_ORIGIN.to_owned(),
                "TEXT".to_owned(),
                0,
                0
            ),
            (
                assistant_core::tools::report::COLUMN_REPORTED_PRINCIPAL_ID.to_owned(),
                "INTEGER".to_owned(),
                0,
                0
            ),
            (
                assistant_core::tools::report::COLUMN_LINE.to_owned(),
                "TEXT".to_owned(),
                1,
                0
            ),
        ],
        "the recreated table keeps every column, its order and its types; \
         only the reported column's NOT NULL is gone"
    );
    assert_eq!(
        parent, "blocks",
        "the header cascade survives the recreation"
    );

    // The data: the pre-migration row still stands, whole.
    let blocks = support::consumer_view(
        &reopened
            .list_blocks(conversation)
            .await
            .expect("the upgraded ledger reads"),
    );
    match AssistantKind::from_block(&blocks[0]) {
        AssistantKind::Report(report) => {
            assert_eq!(
                report.target_origin.as_deref(),
                Some("origin-pre-upgrade"),
                "the stored target survives the table recreation"
            );
            assert_eq!(
                report.reported_principal_id,
                Some(77),
                "the stored reported principal survives it"
            );
            assert_eq!(
                report.line.as_deref(),
                Some("/report@moderation_bot"),
                "the stored line survives it"
            );
        }
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::ChatMessage(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_) => {
            panic!("the upgraded report row resolved as another kind")
        }
    }

    // The point of the step: a filing that names no single person — the
    // plural join event's — now stores, where version fourteen refused it.
    reopened
        .append_consumer_block(
            conversation,
            None,
            assistant_core::tools::report::REPORT_KIND,
            assistant_core::tools::report::Report::stored_fields(
                "origin-plural-event",
                None,
                "/report@moderation_bot",
            ),
            None,
        )
        .await
        .expect("a report naming no single person appends over the relaxed column");

    // The join table's own step brought both of its indexes with it.
    let indexes = table_indexes(&reopened, assistant_core::join::JOIN_NOTICE_TABLE)
        .await
        .expect("the join table's indexes read");
    assert!(
        indexes.contains(&*assistant_core::schema::JOIN_NOTICE_PRINCIPAL_INDEX)
            && indexes.contains(&*assistant_core::schema::JOIN_NOTICE_ORIGIN_INDEX),
        "both keyed access paths of the join table are indexed: {indexes:?}"
    );
}

/// A store on disk in the shape the previous unit's binary left: one filed
/// report standing in a report table that still carries its NOT NULL
/// reported column, no join table, the domain's version at fourteen.
/// Returns the conversation the report sits in.
///
/// The rewind undoes exactly what the two steps past fourteen do, and the
/// store closes before the caller reopens, so the upgrade reads the disk
/// and not a live connection.
async fn a_populated_version_fourteen_store(db: &TempDb) -> i64 {
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");
    let conversation = store
        .create_conversation(
            "scripted-1".into(),
            "script-model".into(),
            "Script Model".into(),
            support::VENDOR.into(),
        )
        .await
        .expect("a conversation row");
    store
        .append_consumer_block(
            conversation,
            None,
            assistant_core::tools::report::REPORT_KIND,
            assistant_core::tools::report::Report::stored_fields(
                "origin-pre-upgrade",
                Some(77),
                "/report@moderation_bot",
            ),
            None,
        )
        .await
        .expect("the pre-upgrade report appends");
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
        conn.execute_batch(&format!(
            "DROP INDEX {revises_index};
             ALTER TABLE block_chat_message DROP COLUMN revises;
             DROP TABLE {join};
             DROP TABLE {delivered};
             DROP TABLE {marks};
             CREATE TABLE {report}_v14 (
                 block_id   INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
                 {target}   TEXT,
                 {reported} INTEGER NOT NULL,
                 {line}     TEXT NOT NULL
             );
             INSERT INTO {report}_v14 (block_id, {target}, {reported}, {line})
                 SELECT block_id, {target}, {reported}, {line} FROM {report};
             DROP TABLE {report};
             ALTER TABLE {report}_v14 RENAME TO {report};",
            revises_index = assistant_core::schema::MESSAGE_REVISES_INDEX.as_str(),
            join = assistant_core::join::JOIN_NOTICE_TABLE,
            delivered = assistant_core::delivery::DELIVERED_TABLE,
            marks = assistant_core::tools::mark::MESSAGE_MARK_TABLE,
            report = assistant_core::tools::report::REPORT_TABLE,
            target = assistant_core::tools::report::COLUMN_TARGET_ORIGIN,
            reported = assistant_core::tools::report::COLUMN_REPORTED_PRINCIPAL_ID,
            line = assistant_core::tools::report::COLUMN_LINE,
        ))?;
        Ok(())
    })
    .await
    .expect("the store rewinds to the previous unit's shape");
    support::rewind_domain_migration_version(&store, 14).await;
    conversation
}

/// AC1 of the her-replies-quote unit (unit 38): a store the previous
/// unit's binary wrote — version sixteen, no delivery table at all —
/// upgrades cleanly through the one appended step, which creates the table
/// and both of its keyed access paths, and a receipt then stores where the
/// older shape had nowhere to put one.
///
/// The step is additive: nothing existing is recreated, so a pre-upgrade
/// conversation reads back exactly as it was written.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_version_sixteen_store_upgrades_through_the_delivery_step() {
    let db = TempDb::new("v16-upgrade");
    let conversation;
    {
        let store =
            Store::open_with(db.path(), store_config()).expect("the configured store opens");
        conversation = store
            .create_conversation(
                "scripted-1".into(),
                "script-model".into(),
                "Script Model".into(),
                support::VENDOR.into(),
            )
            .await
            .expect("a conversation row");
        agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
            conn.execute_batch(&format!(
                "DROP INDEX {revises_index};
                 ALTER TABLE block_chat_message DROP COLUMN revises;
                 DROP TABLE {table};
                 DROP TABLE {marks};",
                revises_index = assistant_core::schema::MESSAGE_REVISES_INDEX.as_str(),
                table = assistant_core::delivery::DELIVERED_TABLE,
                marks = assistant_core::tools::mark::MESSAGE_MARK_TABLE,
            ))?;
            Ok(())
        })
        .await
        .expect("the store rewinds to the previous unit's shape");
        support::rewind_domain_migration_version(&store, 16).await;
        // The first store closes before the reopen, so the upgrade reads
        // the disk, not a live connection.
    }

    let reopened = Store::open_with(db.path(), store_config())
        .expect("the version-sixteen store reopens under the shipped configuration");
    assert_eq!(
        support::domain_migration_version(&reopened).await,
        19,
        "the appended step advanced the domain's version"
    );

    let indexes = table_indexes(&reopened, assistant_core::delivery::DELIVERED_TABLE)
        .await
        .expect("the delivery table's indexes read");
    assert!(
        indexes.contains(&*assistant_core::schema::DELIVERY_ORIGIN_INDEX)
            && indexes.contains(&*assistant_core::schema::DELIVERY_KEY_INDEX),
        "both keyed access paths of the delivery table are indexed: {indexes:?}"
    );

    reopened
        .append_consumer_block(
            conversation,
            None,
            assistant_core::delivery::DELIVERED_KIND,
            assistant_core::delivery::Delivered::stored_fields("31", "31", None),
            None,
        )
        .await
        .expect("a receipt appends over the created table");
}

/// One table's declared shape, read through the domain seam: each column's
/// name, type, NOT NULL flag and primary-key position, in the table's own
/// order, plus the table its foreign key points at. What a recreating
/// migration must reproduce exactly.
async fn table_shape(
    store: &Store,
    table: &str,
) -> Result<(Vec<(String, String, i64, i64)>, String), agent_ledger::StoreError> {
    let table = table.to_owned();
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        let mut statement = conn.prepare(&format!(
            "SELECT name, type, \"notnull\", pk FROM pragma_table_info('{table}')"
        ))?;
        let columns = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let parent: String = conn.query_row(
            &format!("SELECT \"table\" FROM pragma_foreign_key_list('{table}')"),
            [],
            |row| row.get(0),
        )?;
        Ok((columns, parent))
    })
    .await
}

/// The index names one table carries, alphabetically — what a migration
/// step's own `CREATE INDEX` lines are read back through.
async fn table_indexes(
    store: &Store,
    table: &str,
) -> Result<Vec<String>, agent_ledger::StoreError> {
    let table = table.to_owned();
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        let mut statement = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = ?1 ORDER BY name",
        )?;
        let names = statement
            .query_map([table], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(names)
    })
    .await
}

/// AC-D's schema half (unit 39): the mark table's own CHECK is the twin of
/// the tool's byte bound, and it bounds BYTES rather than characters — the
/// distinction that matters, since every emoji worth storing costs several
/// bytes per character. The bound is pinned on both sides: an emoji at
/// thirty-two bytes stores, one past it is refused by the store itself,
/// and an empty one is refused too.
///
/// The store, not the tool, is what this proves. The tool refuses the same
/// shapes ahead of the append with a taught error; this is the row that
/// could never exist even if some later writer forgot to ask.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_mark_tables_check_bounds_the_stored_emoji_in_bytes() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let conversation = store
        .create_conversation(
            "scripted-1".into(),
            "script-model".into(),
            "Script Model".into(),
            support::VENDOR.into(),
        )
        .await
        .expect("a conversation row");

    let append = |emoji: String| {
        store.append_consumer_block(
            conversation,
            None,
            assistant_core::tools::mark::MESSAGE_MARK_KIND,
            assistant_core::tools::mark::MessageMark::stored_fields("origin-1", 7, &emoji),
            None,
        )
    };

    let limit = assistant_core::tools::mark::EMOJI_BYTE_LIMIT;
    // A four-byte emoji repeated to exactly the bound: eight characters,
    // thirty-two bytes. A character-counting CHECK would admit four times
    // as many, so this shape is what tells the two readings apart.
    let at_the_bound = "\u{1F389}".repeat(limit / 4);
    assert_eq!(at_the_bound.len(), limit);
    append(at_the_bound)
        .await
        .expect("an emoji at the bound stores");

    for refused in [String::new(), "a".repeat(limit + 1)] {
        let outcome = append(refused.clone()).await;
        assert!(
            outcome.is_err(),
            "the stored CHECK refuses an emoji of {} bytes",
            refused.len()
        );
    }
    // The character-versus-byte reading, stated as its own claim: nine of
    // the same four-byte emoji is thirty-six bytes and nine characters,
    // so a character-counting CHECK would take it and this one must not.
    let past_in_bytes_only = "\u{1F389}".repeat(limit / 4 + 1);
    assert!(
        past_in_bytes_only.chars().count() < limit && past_in_bytes_only.len() > limit,
        "the probe is past the bound in bytes and inside it in characters"
    );
    assert!(
        append(past_in_bytes_only).await.is_err(),
        "the CHECK counts bytes, not characters"
    );
}

// ─── The editing unit's pins (unit T3, 2026-08-31) ───────────────────────

/// The recorded chat messages of one conversation, in ledger order.
async fn recorded_messages(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect()
}

/// AC2: an edit of a recorded message appends a SECOND message block in the
/// same conversation, column by column against the descriptor's own list —
/// the revised message's origin in the new column, this version's own
/// origin, the sender, the speaker, the reply target, the edit time as the
/// send time and the stamp the message earns. The earlier row is untouched
/// in every column: the ledger is append-only, and the earlier version was
/// already read and possibly already answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_appends_a_second_block_and_leaves_the_first_untouched() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-revision").await;

    let original = support::with_reply(
        support::with_origin(
            support::with_username(
                support::inbound_unaddressed(
                    &room,
                    ChannelKind::Group,
                    "casey-ext",
                    "what is the reelase cadence?",
                ),
                "casey",
            ),
            "msg-1",
        ),
        assistant_core::ReplyTarget::Message {
            origin: "earlier-9".into(),
        },
    );
    let receipt = support::ingest_recorded(&fixture.assistant, original.clone()).await;
    let conversation = receipt.conversation_id;
    let before = recorded_messages(&fixture.store, conversation).await;
    assert_eq!(before.len(), 1);

    let mut edit = original;
    edit.text = "what is the release cadence?".into();
    edit.timestamp = "2026-08-31T12:00:00+00:00"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .expect("a representable edit time");
    support::ingest_recorded(&fixture.assistant, support::revising(edit, "msg-1")).await;

    let after = recorded_messages(&fixture.store, conversation).await;
    assert_eq!(
        after.len(),
        2,
        "the revision is a second row, not a rewrite"
    );
    assert_eq!(
        (after[0].id, after[0].role, &after[0].fields),
        (before[0].id, before[0].role, &before[0].fields),
        "the earlier version's row is untouched in every column"
    );

    let revision = &after[1];
    assert_eq!(revision.role, Some(Role::User));
    assert_eq!(
        revision.fields["revises"],
        json!("msg-1"),
        "the new column names the message this one supersedes"
    );
    assert_eq!(revision.fields["origin"], json!("msg-1"));
    assert_eq!(
        revision.fields["text"],
        json!("what is the release cadence?")
    );
    assert_eq!(revision.fields["speaker"], json!("casey"));
    assert_eq!(revision.fields["authority"], json!("member"));
    assert_eq!(
        revision.fields["principal_id"],
        before[0].fields["principal_id"]
    );
    assert_eq!(revision.fields["reply_target"], json!("earlier-9"));
    assert_eq!(
        revision.fields["sent_at"],
        json!("2026-08-31T12:00:00+00:00"),
        "the edit time is the version's send time"
    );
    assert_eq!(
        revision.fields["addressed"],
        json!(false),
        "the revision is stamped like the unaddressed message it is"
    );
    assert_eq!(revision.fields["answer_due"], json!(false));
    // Every column the descriptor declares is either written above or
    // absent for a stated reason, so a column added without a home here
    // fails this pin instead of shipping unwritten.
    let written: Vec<&str> = revision.fields.keys().map(String::as_str).collect();
    assert_eq!(
        written,
        vec![
            "addressed",
            "answer_due",
            "authority",
            "literal_addressed",
            "origin",
            "principal_id",
            "reply_target",
            "revises",
            "sent_at",
            "speaker",
            "text",
        ],
        "the revision's stored columns, exactly"
    );
}

/// AC3: a revision whose text equals the newest recorded version of that
/// message records nothing and delivers nothing, and the update is
/// acknowledged — the platform fires edit updates for changes nobody asked
/// for. A genuinely different edit records. So does one that returns to an
/// earlier wording: the comparison is against the NEWEST version, never
/// against the history. And the same update redelivered after the first
/// records once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_repeating_the_newest_version_records_nothing() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-identical").await;

    let first = support::with_origin(
        support::inbound_unaddressed(&room, ChannelKind::Group, "casey-ext", "the first wording"),
        "msg-2",
    );
    let conversation = support::ingest_recorded(&fixture.assistant, first.clone())
        .await
        .conversation_id;

    let revision_of = |text: &str| {
        let mut message = first.clone();
        message.text = text.into();
        support::revising(message, "msg-2")
    };

    // The platform's own repeat: the same text under the same message.
    let outcome = fixture
        .assistant
        .ingest(revision_of("the first wording"))
        .await
        .expect("the redelivered update is acknowledged");
    assert!(
        matches!(outcome, IngestOutcome::Disregarded),
        "an unchanged edit touches nothing: {outcome:?}"
    );
    assert_eq!(
        recorded_messages(&fixture.store, conversation).await.len(),
        1
    );

    // A genuine edit records.
    support::ingest_recorded(&fixture.assistant, revision_of("the second wording")).await;
    assert_eq!(
        recorded_messages(&fixture.store, conversation).await.len(),
        2
    );

    // Redelivery of that same update after a halted batch records nothing.
    let redelivered = fixture
        .assistant
        .ingest(revision_of("the second wording"))
        .await
        .expect("the redelivered update is acknowledged");
    assert!(matches!(redelivered, IngestOutcome::Disregarded));
    assert_eq!(
        recorded_messages(&fixture.store, conversation).await.len(),
        2
    );

    // Returning to the first wording is a change against the NEWEST
    // version, so it records: the comparison reads one row, not a history.
    support::ingest_recorded(&fixture.assistant, revision_of("the first wording")).await;
    let messages = recorded_messages(&fixture.store, conversation).await;
    assert_eq!(messages.len(), 3);
    assert_eq!(
        messages
            .iter()
            .map(|block| block.fields["text"].as_str().expect("stored text"))
            .collect::<Vec<_>>(),
        vec![
            "the first wording",
            "the second wording",
            "the first wording"
        ],
        "every version a person wrote stands in the ledger, in order"
    );
}

/// AC5: a failed store read in the identical-text path refuses the
/// ingestion as a store error and records nothing. The refusal is
/// TRANSIENT, which is the whole of what the adapter's batch discipline
/// reads: the update goes unacknowledged and the batch redelivers it.
/// Fail-closed is the standing choice for every admission read (decisions
/// 0041, 0052) — recording anyway would duplicate a row and, under helpful
/// answering, spend a model turn on it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_read_in_the_revision_path_refuses_the_ingestion() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-read-fails").await;

    let original = support::with_origin(
        support::inbound_unaddressed(&room, ChannelKind::Group, "casey-ext", "a first wording"),
        "msg-3",
    );
    let conversation = support::ingest_recorded(&fixture.assistant, original.clone())
        .await
        .conversation_id;

    // The failing store: the kind's own content table is gone, so the
    // revision's read cannot answer.
    agent_ledger::store::domain_run(
        &fixture.store.tx(),
        assistant_core::schema::DOMAIN,
        |conn| {
            conn.execute_batch(&format!("ALTER TABLE {CHAT_MESSAGE_TABLE} RENAME TO away;"))?;
            Ok(())
        },
    )
    .await
    .expect("the table is renamed away");

    let mut edit = original;
    edit.text = "a second wording".into();
    let refused = fixture
        .assistant
        .ingest(support::revising(edit, "msg-3"))
        .await
        .expect_err("the failed read refuses the ingestion");
    assert!(
        matches!(refused, assistant_core::CoreError::Store(_)),
        "the read fails closed as a store error: {refused:?}"
    );
    assert_eq!(
        refused.failure_kind(),
        assistant_core::FailureKind::Transient,
        "the adapter's batch discipline redelivers it like any transient refusal"
    );

    agent_ledger::store::domain_run(
        &fixture.store.tx(),
        assistant_core::schema::DOMAIN,
        |conn| {
            conn.execute_batch(&format!("ALTER TABLE away RENAME TO {CHAT_MESSAGE_TABLE};"))?;
            Ok(())
        },
    )
    .await
    .expect("the table is restored");
    assert_eq!(
        recorded_messages(&fixture.store, conversation).await.len(),
        1,
        "the refused ingestion recorded nothing"
    );
}

/// The author invariant the revision column states, ENFORCED at the
/// ingestion instead of assumed of a platform: a revision whose reviser is
/// not the author of the version the store holds records as an ordinary new
/// message, carrying no reference at all. Recording is never refused — a
/// person's words are not dropped because a platform reported an
/// implausible relation — and what falls away is the link, which erasure
/// and the report both read as one person's own data. On this platform the
/// case cannot arise; the enforcement is what makes the column's
/// documentation true of every stored row.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_revision_by_another_author_records_without_the_link() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-other-author").await;

    let original = support::with_origin(
        support::inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "casey-ext",
            "the author's own wording",
        ),
        "msg-4",
    );
    let conversation = support::ingest_recorded(&fixture.assistant, original)
        .await
        .conversation_id;

    let stranger = support::inbound_unaddressed(
        &room,
        ChannelKind::Group,
        "stranger-ext",
        "somebody else's rewording",
    );
    support::ingest_recorded(&fixture.assistant, support::revising(stranger, "msg-4")).await;

    let messages = recorded_messages(&fixture.store, conversation).await;
    assert_eq!(
        messages.len(),
        2,
        "the words record; only the link falls away"
    );
    assert_eq!(
        messages[1].fields["text"],
        json!("somebody else's rewording")
    );
    assert!(
        messages[1].fields.get("revises").is_none(),
        "no reference is stored to a message the reviser did not write"
    );
    assert_ne!(
        messages[1].fields["principal_id"], messages[0].fields["principal_id"],
        "the two versions are two people, which is what the check reads"
    );
}
