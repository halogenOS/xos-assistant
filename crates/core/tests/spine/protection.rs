//! The protection unit at the core's edges: the limited stamp, the two
//! budgets, the composition with propagated debt, the stamp serialization
//! under a race, the debt authority's minimum rule, and the ledger-derived
//! budget state released by aging receipt times.
//!
//! Budgets in these tests are small on purpose, and the receipt times move
//! through the test seam — backdating the stored header times — so no test
//! sleeps a real window away.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use agent_ledger::providers::{ProviderRequest, Usage};
use agent_ledger::{
    Block, CoreEvent, EventBus, ProviderModule, ProviderResponse, Role, StopReason, Store,
    StreamEvent,
};
use assistant_core::kind::{
    CHAT_MESSAGE_KIND, CHAT_MESSAGE_TABLE, ChatMessage, RecordedSender, Stamp,
};
use assistant_core::schema::{PRINCIPAL_ADDRESSED_INDEX, store_config};
use assistant_core::{Assistant, Authority, ChannelKind, ReplyKind};
use serde_json::json;
use tokio::sync::{Semaphore, mpsc};

use crate::support::{
    self, age_receipts, await_ledger, budgets, channel, domain_migration_version, inbound,
    inbound_as, inbound_unaddressed, recv_reply, reencode_receipts_at_utc_minus_five,
    rewind_domain_migration_version,
};

/// The chat-message blocks of a conversation, oldest first.
async fn messages(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect()
}

/// Await the conversation's `count`th recorded chat message and return it.
async fn await_message(store: &Store, conversation_id: i64, count: usize) -> Block {
    let blocks = await_ledger(store, conversation_id, "the recorded message", |blocks| {
        blocks
            .iter()
            .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
            .count()
            >= count
    })
    .await;
    blocks
        .into_iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .nth(count - 1)
        .expect("the awaited message exists")
}

/// A silent-provider assistant over a fresh in-memory store with the given
/// budgets — the shape most stamp pins here share.
async fn silent_assistant(
    protection: assistant_core::ProtectionConfig,
) -> (Assistant, Store, Arc<EventBus<CoreEvent>>) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection,
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
    .expect("the assembly starts");
    (assistant, store, bus)
}

/// The migration's observable schema shape, read through the domain seam:
/// the content table's column names and whether the principal count's
/// index exists.
async fn stamp_schema(store: &Store) -> (Vec<String>, i64) {
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
        let mut names = Vec::new();
        let mut statement = conn.prepare(&format!("PRAGMA table_info({CHAT_MESSAGE_TABLE})"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            names.push(row.get::<_, String>(1)?);
        }
        drop(rows);
        drop(statement);
        let indexes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [PRINCIPAL_ADDRESSED_INDEX.as_str()],
            |row| row.get(0),
        )?;
        Ok((names, indexes))
    })
    .await
    .expect("the schema reads")
}

/// The protection migration's observable schema on a fresh store, where all
/// steps run at open: both stamp columns exist on the content table and the
/// principal count's index was created.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_appended_migration_adds_the_columns_and_the_index() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let (columns, index_count) = stamp_schema(&store).await;
    assert!(columns.iter().any(|name| name == "limited"));
    assert!(columns.iter().any(|name| name == "debt_authority"));
    assert_eq!(index_count, 1, "the principal count's index exists");
}

/// The appended steps on an existing store — the deployed upgrade path,
/// which the fresh-store pin above cannot see: there, all steps run at one
/// open, so folding the stamp columns into the CREATE TABLE and deleting
/// the appended steps would keep that test green while stranding every
/// store the earlier binary wrote. Here a file store is rewound to the
/// shape that binary left behind — an owing chat row on disk, neither
/// stamp column, no index, no palette table, the domain's version at three
/// — and reopened with the shipped configuration. The appended steps must
/// run alone (a rerun creating step would fail the open on the existing
/// tables), add the columns, the index and the palette table, advance the
/// version, and leave the pre-existing row reading the typed absence in
/// both stamp columns.
// The length is the upgrade story itself: write, rewind, reopen, pin every
// appended step's artifact.
#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_version_three_store_upgrades_through_the_appended_steps_alone() {
    let db = support::TempDb::new("v3-upgrade");
    let conversation;
    {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        conversation = store
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
                Some(Role::User),
                CHAT_MESSAGE_KIND,
                ChatMessage::stored_fields(
                    "a message the earlier binary recorded",
                    RecordedSender {
                        principal_id: 1,
                        authority: Authority::Member,
                        speaker: None,
                    },
                    Some("scripted:41"),
                    None,
                    "2026-08-21T00:00:00+00:00",
                    Stamp {
                        addressed: true,
                        literal_addressed: false,
                        limited: None,
                        answer_due: true,
                        debt_authority: None,
                    },
                ),
                None,
            )
            .await
            .expect("the pre-upgrade row appends");
        // The rewind: drop exactly what the appended steps add, restore
        // what the retirement step drops (a version-three principals table
        // still carried its display-name column), and set the domain's
        // version back, so the disk holds what the pre-protection binary's
        // store held — the group-context unit's tables included. The
        // dropped columns take their column-level CHECK constraints with
        // them. The version rewind goes through the support seam that owns
        // the suite's framework-schema knowledge.
        agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
            conn.execute_batch(&format!(
                "DROP INDEX {index};
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN limited;
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN debt_authority;
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN reply_target;
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN reply_to_assistant;
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN speaker;
                 ALTER TABLE {CHAT_MESSAGE_TABLE} DROP COLUMN literal_addressed;
                 ALTER TABLE principals DROP COLUMN opted_out;
                 DROP TABLE {palette};
                 DROP TABLE {note};
                 DROP TABLE {report};
                 DROP TABLE {join};
                 DROP TABLE {delivered};
                 DROP TABLE {marks};
                 DROP TABLE group_authorizations;
                 ALTER TABLE principals ADD COLUMN display_name TEXT NOT NULL DEFAULT '';",
                index = PRINCIPAL_ADDRESSED_INDEX.as_str(),
                palette = assistant_core::tools::palette::TOOL_PALETTE_TABLE,
                note = assistant_core::note::CONTEXT_NOTE_TABLE,
                report = assistant_core::tools::report::REPORT_TABLE,
                join = assistant_core::join::JOIN_NOTICE_TABLE,
                delivered = assistant_core::delivery::DELIVERED_TABLE,
                marks = assistant_core::tools::mark::MESSAGE_MARK_TABLE,
            ))?;
            Ok(())
        })
        .await
        .expect("the store rewinds to the pre-protection shape");
        rewind_domain_migration_version(&store, 3).await;
        // The first store closes before the reopen, so the upgrade reads
        // the disk, not a live connection.
    }

    let reopened = Store::open_with(db.path(), store_config())
        .expect("the version-three store reopens under the shipped configuration");
    let (columns, index_count) = stamp_schema(&reopened).await;
    assert!(columns.iter().any(|name| name == "limited"));
    assert!(columns.iter().any(|name| name == "debt_authority"));
    assert!(
        columns.iter().any(|name| name == "reply_target"),
        "the reply-target step added its origin column"
    );
    assert!(
        columns.iter().any(|name| name == "reply_to_assistant"),
        "the reply-target step added its assistant-reply column"
    );
    assert!(
        columns.iter().any(|name| name == "speaker"),
        "the speaker step added its column"
    );
    assert_eq!(index_count, 1, "the appended step created the index");
    let report_tables: i64 =
        agent_ledger::store::domain_run(&reopened.tx(), assistant_core::schema::DOMAIN, |conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [assistant_core::tools::report::REPORT_TABLE],
                |row| row.get(0),
            )?)
        })
        .await
        .expect("the table listing reads");
    assert_eq!(report_tables, 1, "the report step created its table");
    assert_eq!(
        domain_migration_version(&reopened).await,
        18,
        "the appended steps advanced the domain's version"
    );

    let rows = messages(&reopened, conversation).await;
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].fields.get("limited").is_none(),
        "the pre-existing row reads null in the limited fact"
    );
    assert!(
        rows[0].fields.get("debt_authority").is_none(),
        "the pre-existing row reads null in the debt authority"
    );
    assert!(
        rows[0].fields.get("reply_target").is_none()
            && rows[0].fields.get("reply_to_assistant").is_none(),
        "the pre-existing row reads null in both reply facts"
    );
    assert!(
        rows[0].fields.get("speaker").is_none(),
        "the pre-existing row reads null in the speaker"
    );
    assert_eq!(
        rows[0].fields["answer_due"],
        json!(true),
        "the upgrade left the stored stamp itself untouched"
    );
    // The command-stamp step recreates the message table and copies every
    // row, so each nullable content column is pinned by exact value: one
    // omitted from the copy list would read back as silently erased
    // history — NULL text is the erasure encoding, and a NULL role
    // mis-voices the row and cancels its owed answer — while the NOT NULL
    // columns would fail the copy loudly on their own.
    assert_eq!(
        rows[0].fields["text"],
        json!("a message the earlier binary recorded"),
        "the recreate-and-copy carried the stored text"
    );
    assert_eq!(
        rows[0].fields["origin"],
        json!("scripted:41"),
        "the recreate-and-copy carried the stored origin"
    );
    assert_eq!(
        rows[0].fields["sent_at"],
        json!("2026-08-21T00:00:00+00:00"),
        "the recreate-and-copy carried the stored sending time"
    );
    assert_eq!(
        rows[0].role,
        Some(Role::User),
        "the recreate-and-copy carried the stored voice"
    );
}

/// AC2, end to end over the scripted provider: addressed messages up to the
/// principal budget are answered; the next is recorded addressed but
/// limited with the `principal` fact, draws no answer and no notice; and
/// once the receipt times age past the window through the test seam, the
/// same principal is answered again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_principal_budget_refuses_the_next_debt_and_the_window_releases_it() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture =
        support::start_assistant_configured(store, None, budgets(Some((2, 600)), None)).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-principal-budget");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "A", "the first ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Answer);
    support::settle(&fixture.store, conv, "the first answer", 4).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "A", "the second ask"),
    )
    .await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Answer);
    support::settle(&fixture.store, conv, "the second answer", 6).await;

    // The third ask crosses the budget: recorded, addressed, limited by the
    // principal budget, owing nothing — and silent, no answer and no notice.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "A", "the refused ask"),
    )
    .await;
    let refused = await_message(&fixture.store, conv, 3).await;
    assert_eq!(refused.fields["addressed"], json!(true));
    assert_eq!(refused.fields["limited"], json!("principal"));
    assert_eq!(refused.fields["answer_due"], json!(false));
    assert!(
        refused.fields.get("debt_authority").is_none(),
        "a refused debt is no debt, so no authority is stamped"
    );
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        2,
        "the refused ask draws no turn"
    );
    assert!(
        matches!(replies.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
        "over-limit is silent in the chat"
    );

    // Aging the receipt times past the window releases the budget: the same
    // principal is answered again.
    age_receipts(&fixture.store, 601).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "A", "the later ask"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.kind, ReplyKind::Answer);
    assert_eq!(
        reply.text,
        support::answer_to("the refused ask\n\nthe later ask")
    );
}

/// AC3: two principals exhaust the channel budget together; the over-limit
/// message carries the `channel` fact; another channel is unaffected; and
/// the limited principal's direct chat still answers under its own
/// principal budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_channel_budget_spares_other_channels_and_the_direct_chat() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture =
        support::start_assistant_configured(store, None, budgets(Some((100, 600)), Some((2, 600))))
            .await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-channel-budget").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "A", "the first ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Answer);
    support::settle(&fixture.store, conv, "the first answer", 4).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "B", "the second ask"),
    )
    .await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Answer);
    support::settle(&fixture.store, conv, "the second answer", 6).await;

    // The two principals exhausted the channel together: B's next ask is
    // The two principals exhausted the channel together: B's next ask is
    // limited with the channel fact.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "B", "the refused ask"),
    )
    .await;
    let refused = await_message(&fixture.store, conv, 3).await;
    assert_eq!(refused.fields["limited"], json!("channel"));
    assert_eq!(refused.fields["answer_due"], json!(false));

    // Another channel is unaffected.
    let untouched = support::authorized_group(&fixture.assistant, "room-untouched").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&untouched, ChannelKind::Group, "B", "the other room's ask"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, support::answer_to("the other room's ask"));

    // The limited principal's direct chat answers under its own budgets.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&channel("dm-b"), ChannelKind::Direct, "B", "the direct ask"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, support::answer_to("the direct ask"));
}

/// A provider held before its first stream event: each turn waits for a
/// permit before anything reaches the wire, so the asked message stays the
/// conversation's tail — provably unanswered, with no streaming block —
/// while the test writes behind it.
fn held_provider(release: Arc<Semaphore>) -> Box<dyn ProviderModule> {
    support::provider_stub("Held", "answers when released", move || {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let ProviderRequest::Stream { .. } = request else {
                    continue;
                };
                release
                    .acquire()
                    .await
                    .expect("the release outlives the test")
                    .forget();
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                    text: "the released answer".into(),
                }));
                let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                    usage: Usage::default(),
                    stop_reason: StopReason::EndTurn,
                }));
            }
        });
        (request_tx, responses)
    })
}

/// AC4, the composition rule: an over-limit addressed message arriving
/// behind an unanswered answer-due tail is recorded with BOTH facts — the
/// limited stamp naming the refusing budget and a true answer-due carrying
/// the earlier sender's debt forward — and that earlier answer still
/// arrives. A flooder can be refused their own answer but can never cancel
/// someone else's. The exhausted channel still records unaddressed messages
/// with a null limited fact: budgets are consulted for addressed messages
/// only.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_over_limit_message_propagates_the_debt_and_the_earlier_answer_arrives() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let release = Arc::new(Semaphore::new(0));
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        support::registry_of(held_provider(Arc::clone(&release))),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: budgets(None, Some((1, 600))),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
    .expect("the assembly starts");
    let mut replies = assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&assistant, "room-propagated").await;

    // The innocent sender's ask takes the channel's one slot; its turn is
    // held before any stream event, so the ask stays the unanswered tail.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "A", "the innocent ask"),
    )
    .await;
    let conv = receipt.conversation_id;

    // The flooder's ask is over-limit AND behind the owed answer: limited
    // set, answer-due still true — the propagated debt, not a contradiction.
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "B", "the flooding ask"),
    )
    .await;
    let flooded = await_message(&store, conv, 2).await;
    assert_eq!(flooded.fields["addressed"], json!(true));
    assert_eq!(flooded.fields["limited"], json!("channel"));
    assert_eq!(
        flooded.fields["answer_due"],
        json!(true),
        "the limited stamp refuses the message's own debt, never the \
         debt it propagates"
    );

    // The exhausted channel consults no budget for an unaddressed message:
    // The exhausted channel consults no budget for an unaddressed message:
    // recorded with a null limited fact.
    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "C", "an aside"),
    )
    .await;
    let aside = await_message(&store, conv, 3).await;
    assert_eq!(aside.fields["addressed"], json!(false));
    assert!(
        aside.fields.get("limited").is_none(),
        "an unaddressed message is never limited — no budget was consulted"
    );

    // Released, the earlier sender's answer arrives.
    release.add_permits(1);
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.kind, ReplyKind::Answer);
    assert_eq!(reply.channel, room);
}

/// AC5, the race for the last slot: two messages ingested concurrently
/// against a one-answer channel budget yield exactly one taken debt and one
/// limited stamp, because the counts and the append share the stamp
/// serialization. The interleaving is probabilistic — nothing forces the
/// two ingestions to collide on any single run — so each round starts the
/// two racers from a barrier into an already-mapped room, and the loop
/// repeats the collision enough times that a broken serialization,
/// double-granting the slot on some round, fails in practice: with the
/// serialization removed, this test fails within the first few rounds.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_racing_messages_cannot_both_take_the_last_slot() {
    let (assistant, store, _bus) = silent_assistant(budgets(None, Some((1, 600)))).await;
    let assistant = Arc::new(assistant);
    for round in 0..30 {
        let room = support::authorized_group(&assistant, &format!("room-race-{round}")).await;
        // The room is mapped and both racers' principals resolved before
        // the race, by an unaddressed opener from each — openers consume no
        // budget — so the racers reach the stamp in lockstep instead of
        // skewing apart on first-message conversation creation or on a
        // first-sender identity insert.
        for sender in ["A", "B"] {
            support::ingest_recorded(
                &assistant,
                inbound_unaddressed(&room, ChannelKind::Group, sender, "the opener"),
            )
            .await;
        }
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let racer = |sender: &'static str, text: &'static str| {
            let assistant = Arc::clone(&assistant);
            let room = room.clone();
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                barrier.wait().await;
                support::ingest_recorded(
                    &assistant,
                    inbound(&room, ChannelKind::Group, sender, text),
                )
                .await
            })
        };
        let (first, second) = tokio::join!(racer("A", "one racer"), racer("B", "the other racer"));
        let conv = first.expect("the first racer's task joins").conversation_id;
        assert_eq!(
            conv,
            second
                .expect("the second racer's task joins")
                .conversation_id,
            "both racers were recorded in the room's one conversation"
        );

        let recorded = messages(&store, conv).await;
        assert_eq!(recorded.len(), 4, "recording is never limited");
        let limited: Vec<bool> = recorded
            .iter()
            .skip(2)
            .map(|block| block.fields.get("limited").is_some())
            .collect();
        assert_eq!(
            limited.iter().filter(|stamped| **stamped).count(),
            1,
            "round {round}: exactly one racer is limited, one takes the \
             slot; stamps were {limited:?}"
        );
    }
}

/// AC6, the minimum rule, block by block over a silent provider: a fresh
/// debt opens at its sender's authority; a carried debt — propagated by an
/// unaddressed message or ridden by an addressed one — takes the minimum of
/// the tail's debt authority and the incoming sender's, so a member's
/// question riding an admin's debt never gains admin standing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_debt_authority_is_stamped_by_the_minimum_rule() {
    let (assistant, store, _bus) = silent_assistant(budgets(None, None)).await;

    // An admin's addressed message opens an admin debt; a member's
    // unaddressed message propagating it stamps member.
    let room = support::authorized_group(&assistant, "room-authority-propagated").await;
    let receipt = support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "the admin ask",
        ),
    )
    .await;
    let opened = await_message(&store, receipt.conversation_id, 1).await;
    assert_eq!(opened.fields["debt_authority"], json!("admin"));
    let mut aside = inbound_as(
        &room,
        ChannelKind::Group,
        "m",
        Authority::Member,
        "a member aside",
    );
    aside.addressed = false;
    support::ingest_recorded(&assistant, aside).await;
    let carried = await_message(&store, receipt.conversation_id, 2).await;
    assert_eq!(carried.fields["answer_due"], json!(true));
    assert_eq!(
        carried.fields["debt_authority"],
        json!("member"),
        "the carried debt takes the minimum of tail and sender"
    );

    // A member's ADDRESSED message behind the same admin-debt shape also
    // stamps member: the minimum rule applies whenever the tail owes,
    // regardless of the incoming message's own addressed fact.
    let room = support::authorized_group(&assistant, "room-authority-addressed").await;
    let receipt = support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "the admin ask",
        ),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "m",
            Authority::Member,
            "a member question behind it",
        ),
    )
    .await;
    let riding = await_message(&store, receipt.conversation_id, 2).await;
    assert_eq!(riding.fields["debt_authority"], json!("member"));

    // A fresh member debt stays member.
    let fresh_room = support::authorized_group(&assistant, "room-authority-fresh").await;
    let receipt = support::ingest_recorded(
        &assistant,
        inbound_as(
            &fresh_room,
            ChannelKind::Group,
            "m",
            Authority::Member,
            "a fresh member ask",
        ),
    )
    .await;
    let fresh = await_message(&store, receipt.conversation_id, 1).await;
    assert_eq!(fresh.fields["debt_authority"], json!("member"));

    // A resting unaddressed message with no owing tail carries no debt.
    let resting_room = support::authorized_group(&assistant, "room-authority-resting").await;
    let receipt = support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&resting_room, ChannelKind::Group, "m", "a resting remark"),
    )
    .await;
    let resting = await_message(&store, receipt.conversation_id, 1).await;
    assert!(resting.fields.get("debt_authority").is_none());
}

/// AC6, the minimum rule's other direction — the escalation prevention the
/// rule exists for: an admin's ADDRESSED message behind a member's debt
/// carries the member authority forward. The higher-standing sender cannot
/// lift the summoned turn above the lowest authority that contributed to
/// it, which is exactly the escalation decision 0036 forbids.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_higher_authority_sender_behind_a_lower_debt_carries_the_lower() {
    let (assistant, store, _bus) = silent_assistant(budgets(None, None)).await;
    let room = support::authorized_group(&assistant, "room-authority-escalation").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "m",
            Authority::Member,
            "the member ask",
        ),
    )
    .await;
    let opened = await_message(&store, receipt.conversation_id, 1).await;
    assert_eq!(opened.fields["debt_authority"], json!("member"));
    support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "the admin follow-up",
        ),
    )
    .await;
    let held = await_message(&store, receipt.conversation_id, 2).await;
    assert_eq!(held.fields["answer_due"], json!(true));
    assert_eq!(
        held.fields["debt_authority"],
        json!("member"),
        "a higher-authority sender behind a lower-authority debt carries \
         the lower authority forward"
    );
}

/// AC6, the pre-migration fold: an owing tail written before the protection
/// migration carries no debt-authority stamp, and the fold reads it as the
/// tail's own stored sender authority. The incoming sender is an admin on
/// purpose — against an admin tail the minimum stays admin, so a fold that
/// invented any lower standing would be caught here.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pre_migration_owing_tail_folds_to_its_stored_sender_authority() {
    let (assistant, store, _bus) = silent_assistant(budgets(None, None)).await;
    let room = support::authorized_group(&assistant, "room-pre-migration").await;

    // The conversation exists through the ordinary edge; the pre-migration
    // shape is then appended directly — the exact field set the old binary
    // wrote, with neither protection stamp.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "m", "an opening remark"),
    )
    .await;
    let conv = receipt.conversation_id;
    store
        .append_consumer_block(
            conv,
            Some(Role::User),
            CHAT_MESSAGE_KIND,
            ChatMessage::stored_fields(
                "the pre-migration admin ask",
                RecordedSender {
                    principal_id: receipt.principal_id,
                    authority: Authority::Admin,
                    speaker: None,
                },
                None,
                None,
                "2026-08-21T00:00:00+00:00",
                // The old binary's stamp: addressed and answer-due, with
                // neither protection fact — written literally, not through
                // Stamp::compose, because the pre-migration shape is the
                // point.
                Stamp {
                    addressed: true,
                    literal_addressed: false,
                    limited: None,
                    answer_due: true,
                    debt_authority: None,
                },
            ),
            None,
        )
        .await
        .expect("the pre-migration-shaped row appends");
    support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "boss",
            Authority::Admin,
            "the follow-up",
        ),
    )
    .await;
    let carried = await_message(&store, conv, 3).await;
    assert_eq!(carried.fields["answer_due"], json!(true));
    assert_eq!(
        carried.fields["debt_authority"],
        json!("admin"),
        "the null stamp folds to the tail's stored sender authority, \
         not to any invented default"
    );
}

/// The unlatch rule: only a taken debt re-engages. The first addressed
/// message emits the unlatch intent; an over-limit addressed message — the
/// budget just refused its debt — emits none, so a limited flood cannot
/// wake an error-latched conversation. Unaddressed messages were already
/// pinned silent by the addressing suite.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_a_taken_debt_emits_the_unlatch_intent() {
    let (assistant, _store, bus) = silent_assistant(budgets(None, Some((1, 600)))).await;
    let mut events = bus.subscribe();
    let room = support::authorized_group(&assistant, "room-unlatch-limited").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "A", "the taken ask"),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "B", "the refused ask"),
    )
    .await;

    let mut unlatches = 0;
    while let Ok(event) = events.try_recv() {
        if let CoreEvent::UnlatchRequested { conversation_id } = event {
            assert_eq!(conversation_id, receipt.conversation_id);
            unlatches += 1;
        }
    }
    assert_eq!(
        unlatches, 1,
        "the taken debt unlatched; the refused one did not"
    );
}

/// AC7, derived and never stored: the budget's whole state is the ledger's
/// recent history. A fresh assembly over the same store still refuses — an
/// in-memory tally would have reset with the restart — the outcome stays
/// refused while the history stands, and it changes exactly when the
/// history changes: aging the receipt times through the test seam releases
/// the budget.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_budget_state_is_the_ledger_and_ages_with_it() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let room = channel("dm-derived");
    let protection = budgets(Some((1, 600)), None);

    // The first assembly takes the one debt, then is dropped — as a stopped
    // process drops its memory.
    {
        let assistant = Assistant::start(
            store.clone(),
            Arc::new(EventBus::new()),
            support::registry_of(support::silent_provider()),
            assistant_core::tools::ToolSet::new(),
            assistant_core::AssemblyConfig {
                started_at: std::time::Instant::now(),
                reasoning: assistant_core::ReasoningLevel::Low,
                binding: support::binding(),
                system_prompt: support::SYSTEM_PROMPT.into(),
                answering: support::FIXTURE_ANSWERING,
                name: support::NAME.into(),
                disclosure: None,
                protection: protection.clone(),
                operators: support::operator_config(),
                direct_chats: assistant_core::DirectChats::default(),
                privacy_policy_address: None,
                moderation_handle: None,
                web_search: None,
            },
        )
        .await
        .expect("the first assembly starts");
        let receipt = support::ingest_recorded(
            &assistant,
            inbound(&room, ChannelKind::Direct, "A", "the taken ask"),
        )
        .await;
        let taken = await_message(&store, receipt.conversation_id, 1).await;
        assert!(taken.fields.get("limited").is_none());
    }

    // The second assembly derives the same refusal from the stored history.
    let (assistant, store, _bus) = {
        let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
        let assistant = Assistant::start(
            store.clone(),
            Arc::clone(&bus),
            support::registry_of(support::silent_provider()),
            assistant_core::tools::ToolSet::new(),
            assistant_core::AssemblyConfig {
                started_at: std::time::Instant::now(),
                reasoning: assistant_core::ReasoningLevel::Low,
                binding: support::binding(),
                system_prompt: support::SYSTEM_PROMPT.into(),
                answering: support::FIXTURE_ANSWERING,
                name: support::NAME.into(),
                disclosure: None,
                protection,
                operators: support::operator_config(),
                direct_chats: assistant_core::DirectChats::default(),
                privacy_policy_address: None,
                moderation_handle: None,
                web_search: None,
            },
        )
        .await
        .expect("the second assembly starts");
        (assistant, store, bus)
    };
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the refused ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    let refused = await_message(&store, conv, 2).await;
    assert_eq!(refused.fields["limited"], json!("principal"));

    // Unchanged history, unchanged outcome: the taken debt still stands
    // inside the window. (That a refused debt consumes no budget is pinned
    // by the staggered-aging test below — under this one-answer budget the
    // count here is at the limit either way.)
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "still refused"),
    )
    .await;
    let still = await_message(&store, conv, 3).await;
    assert_eq!(still.fields["limited"], json!("principal"));

    // The one history change a budget answers to: the receipt times age
    // past the window, and the budget releases.
    age_receipts(&store, 601).await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the released ask"),
    )
    .await;
    let released = await_message(&store, conv, 4).await;
    assert!(
        released.fields.get("limited").is_none(),
        "the aged history no longer refuses"
    );
    assert_eq!(released.fields["answer_due"], json!(true));
}

/// The counted predicate's other half: a refused debt consumed no spend,
/// so it consumes no budget either. Staggered aging is what makes this
/// discriminating — uniform aging with a full-window shift leaves a count
/// that also tallied limited rows green. Here the taken debt is aged just
/// past the window while the refused one is still inside it, and the next
/// ask must be admitted: a count that included limited rows would still
/// refuse, and a flooder could lock themselves out past the window forever.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_debt_consumes_no_budget() {
    let (assistant, store, _bus) = silent_assistant(budgets(Some((1, 600)), None)).await;
    let room = channel("dm-refused-consumes-nothing");

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the taken ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    let taken = await_message(&store, conv, 1).await;
    assert!(taken.fields.get("limited").is_none());

    // Half a window on, the taken debt still refuses the next ask.
    age_receipts(&store, 300).await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the refused ask"),
    )
    .await;
    let refused = await_message(&store, conv, 2).await;
    assert_eq!(refused.fields["limited"], json!("principal"));

    // Another shift: the taken debt leaves the window at roughly 700
    // seconds while the refused one sits inside it at roughly 400 — and
    // the ask is admitted, because only the taken debt ever counted.
    age_receipts(&store, 400).await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the admitted ask"),
    )
    .await;
    let admitted = await_message(&store, conv, 3).await;
    assert!(
        admitted.fields.get("limited").is_none(),
        "the refused debt consumed no budget"
    );
}

/// The channel count's same clause: a limited row never counts toward the
/// channel budget either. First the unchanged-history half — the channel
/// exhausted, a refused ask, then with nothing aged another ask refused by
/// the same budget. Then the staggered release that makes the clause
/// observable: the taken debt is aged past the window while the refused
/// rows stay inside it, and the next ask must be admitted. A channel count
/// that tallied limited rows would still refuse here, and each further
/// refusal would re-arm the window — a channel locked forever by its own
/// refusals.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_debt_consumes_no_channel_budget() {
    let (assistant, store, _bus) = silent_assistant(budgets(None, Some((1, 600)))).await;
    let room = support::authorized_group(&assistant, "room-refused-consumes-nothing").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "A", "the taken ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    let taken = await_message(&store, conv, 1).await;
    assert!(taken.fields.get("limited").is_none());

    // Half a window on, the channel's one slot still refuses the next ask.
    age_receipts(&store, 300).await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "B", "the refused ask"),
    )
    .await;
    let refused = await_message(&store, conv, 2).await;
    assert_eq!(refused.fields["limited"], json!("channel"));

    // Nothing aged between them: the next ask is refused by the same
    // budget — unchanged history, unchanged outcome.
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "B", "still refused"),
    )
    .await;
    let still = await_message(&store, conv, 3).await;
    assert_eq!(still.fields["limited"], json!("channel"));

    // Another shift: the taken debt leaves the window at roughly 700
    // seconds while the refused rows sit inside it at roughly 400 — and
    // the ask is admitted, because only the taken debt ever counted.
    age_receipts(&store, 400).await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "B", "the admitted ask"),
    )
    .await;
    let admitted = await_message(&store, conv, 4).await;
    assert!(
        admitted.fields.get("limited").is_none(),
        "a refused debt consumes no channel budget"
    );
}

/// The stamp order's tie: a message both budgets would refuse carries the
/// `principal` fact, because the principal budget is consulted first and
/// the first refusing budget names the limited fact.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_budgets_exhausted_at_once_name_the_principal_fact() {
    let (assistant, store, _bus) = silent_assistant(budgets(Some((1, 600)), Some((1, 600)))).await;
    let room = support::authorized_group(&assistant, "room-both-exhausted").await;

    // One taken debt exhausts both one-answer budgets together.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "A", "the taken ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    let taken = await_message(&store, conv, 1).await;
    assert!(taken.fields.get("limited").is_none());
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "A", "the refused ask"),
    )
    .await;
    let refused = await_message(&store, conv, 2).await;
    assert_eq!(
        refused.fields["limited"],
        json!("principal"),
        "with both budgets exhausted, the first consulted budget names \
         the refusal"
    );
}

/// The principal count is global across conversations (decision 0033):
/// spend is global, so a group ask and the same principal's direct chat
/// draw on one budget. With a one-answer principal budget, a debt taken in
/// the group refuses the same principal's next ask in their direct chat —
/// a per-conversation count would have admitted it, handing a flooder a
/// fresh budget per room — while another principal stays untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_principal_budget_spans_conversations() {
    let (assistant, store, _bus) = silent_assistant(budgets(Some((1, 600)), None)).await;
    let room = support::authorized_group(&assistant, "room-global-budget").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "A", "the group ask"),
    )
    .await;
    let taken = await_message(&store, receipt.conversation_id, 1).await;
    assert!(taken.fields.get("limited").is_none());

    // The same principal in a different conversation: refused, because the
    // count crosses conversations.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(
            &channel("dm-global-budget-a"),
            ChannelKind::Direct,
            "A",
            "the direct ask",
        ),
    )
    .await;
    let refused = await_message(&store, receipt.conversation_id, 1).await;
    assert_eq!(
        refused.fields["limited"],
        json!("principal"),
        "the principal budget spans conversations"
    );

    // Another principal's spend is their own: admitted.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(
            &channel("dm-global-budget-b"),
            ChannelKind::Direct,
            "B",
            "the other ask",
        ),
    )
    .await;
    let admitted = await_message(&store, receipt.conversation_id, 1).await;
    assert!(admitted.fields.get("limited").is_none());
}

/// The window is a time comparison, not a text one. The store writes the
/// receipt time as RFC 3339 with whatever offset the deployment's clock
/// carries, while the count's cutoff is the database's own UTC form —
/// encodings that ordered as raw text would turn the window into a
/// calendar-date test. Pinned by re-encoding a fresh taken debt to the
/// same instant at UTC-05:00: the count must still see it (refusing the
/// next ask), and aging past the window under that offset must still
/// release.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_window_counts_receipt_times_across_offset_encodings() {
    let (assistant, store, _bus) = silent_assistant(budgets(Some((1, 600)), None)).await;
    let room = channel("dm-offset-encoding");

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the taken ask"),
    )
    .await;
    let conv = receipt.conversation_id;
    let taken = await_message(&store, conv, 1).await;
    assert!(taken.fields.get("limited").is_none());

    // The same instants, re-expressed at UTC-05:00 through the support
    // seam that owns the header's time encoding.
    reencode_receipts_at_utc_minus_five(&store).await;

    // Still inside the window, so still refused — a raw text comparison
    // against the UTC cutoff would have excluded the re-encoded row.
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the refused ask"),
    )
    .await;
    let refused = await_message(&store, conv, 2).await;
    assert_eq!(refused.fields["limited"], json!("principal"));

    // Aging keeps the UTC-05:00 encoding and the window still releases.
    age_receipts(&store, 601).await;
    support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Direct, "A", "the released ask"),
    )
    .await;
    let released = await_message(&store, conv, 3).await;
    assert!(
        released.fields.get("limited").is_none(),
        "the offset encoding does not stall the window"
    );
}
