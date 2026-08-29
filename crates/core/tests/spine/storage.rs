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
        | AssistantKind::Report(_) => {
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

    assert_eq!(AssistantKind::DESCRIPTORS.len(), 5);
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
        | AssistantKind::Report(_) => {
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
        | AssistantKind::Report(_) => {
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
        | AssistantKind::Report(_) => {
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
            conn.execute_batch(
                "ALTER TABLE principals ADD COLUMN display_name TEXT NOT NULL DEFAULT '';
                 ALTER TABLE principals DROP COLUMN opted_out;
                 ALTER TABLE block_chat_message DROP COLUMN literal_addressed;
                 DROP TABLE block_join_notice;
                 INSERT INTO principals (adapter, external_id, display_name, username)
                     VALUES ('test-adapter', '42', 'Ada Lovelace', 'ada');",
            )?;
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
        16,
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
            16,
            "the domain's recorded version is the grounded-answer unit's"
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
                "ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN literal_addressed;
                 DROP TABLE {join};",
                join = assistant_core::join::JOIN_NOTICE_TABLE,
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
        16,
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
        | AssistantKind::Report(_) => {
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
        16,
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
        | AssistantKind::ChatMessage(_) => {
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
            "DROP TABLE {join};
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
            join = assistant_core::join::JOIN_NOTICE_TABLE,
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
