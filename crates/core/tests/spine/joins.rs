//! The join notice at the core's edges (unit 36, AC2–AC5): a join lands
//! through the observation seam as one marked block per joiner, wakes
//! nothing and buries nothing on either walk, reaches the report path
//! without anybody else feeling it, and is erased by person and by event
//! exactly as a message is — with a suppressed person's join never stored
//! at all.

use std::sync::Arc;

use agent_ledger::agency::ratchet;
use agent_ledger::store::domain_run;
use agent_ledger::{
    AgencyCtx, Awaiting, Block, CoreEvent, EventBus, LeafKind, Projection, Role, Store,
};
use assistant_core::join::{self, JoinNotice};
use assistant_core::schema::{DOMAIN, store_config};
use assistant_core::tools::ToolSet;
use assistant_core::tools::report;
use assistant_core::{
    AnsweringMode, Authority, ChannelKey, ChannelKind, ErasureOutcome, IngestOutcome, JoinedMember,
    Observation, ObserveOutcome, ObservedFact, ProtectionConfig, ReplyKind, ReplyTarget,
    ReplyThread, SenderIdentity, privacy,
};
use serde_json::json;

use crate::support::{
    self, ToolScript, channel, field, inbound, inbound_as, inbound_unaddressed, recv_reply,
    settle_shape, tool_scripted_provider, with_command, with_reply,
};

/// One joiner as the adapter translated them: the identity every sender
/// crosses the boundary with, plus the name the platform showed.
fn joiner(external_id: &str, handle: Option<&str>, name: &str) -> JoinedMember {
    JoinedMember {
        identity: SenderIdentity {
            external_id: external_id.into(),
            username: handle.map(Into::into),
            bot: false,
        },
        name: name.into(),
    }
}

/// One join observation: the given joiners, under one event origin.
fn joined(key: &ChannelKey, origin: &str, joiners: Vec<JoinedMember>) -> Observation {
    Observation {
        channel: key.clone(),
        channel_kind: ChannelKind::Group,
        fact: ObservedFact::MembersJoined {
            joiners,
            origin: origin.into(),
            timestamp: chrono::Utc::now(),
        },
    }
}

/// Report one join observation, asserting the surface observed it without
/// asking the adapter to do anything: a join delivers nothing.
async fn observe_join(
    assistant: &assistant_core::Assistant,
    key: &ChannelKey,
    origin: &str,
    joiners: Vec<JoinedMember>,
) {
    let outcome = assistant
        .observe(joined(key, origin, joiners))
        .await
        .expect("the join observation is judged");
    assert_eq!(
        outcome,
        ObserveOutcome::Observed { deliver: None },
        "a join is recorded and answered with nothing"
    );
}

/// The join-notice blocks of one conversation, oldest first.
async fn join_blocks(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == join::JOIN_NOTICE_KIND)
        .collect()
}

/// What one stored join block projects to the model — the kind's own
/// reading of the stored row, which is what the request carries.
fn projected(block: &Block) -> Option<String> {
    JoinNotice::parse(block).llm_text()
}

/// The same line with the envelope stripped away — what these cases pin.
///
/// The envelope's `date` is the platform's own live clock, so pinning it
/// here would pin a timestamp; what it renders is pinned byte for byte at
/// the kind, and the id it declares is asserted separately below.
fn projected_line(block: &Block) -> Option<String> {
    projected(block).map(|line| support::without_envelope(&line))
}

/// The tool names of every recorded choice one conversation holds, oldest
/// first — the supersession pins read the newest one back.
async fn stored_choices(store: &Store, conversation_id: i64) -> Vec<Vec<String>> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| block.block_type == "tool_choice")
        .map(support::choice_names)
        .collect()
}

/// The filed targets of every report block in the store, oldest first:
/// `None` for a filing whose target an erasure pass nulled — the shape
/// that makes the report undeliverable, which is what erasure owes a
/// person whose record it named.
async fn stored_report_targets(store: &Store) -> Vec<Option<String>> {
    domain_run(&store.tx(), DOMAIN, |conn| {
        let mut statement = conn.prepare(&format!(
            "SELECT {target} FROM {table} ORDER BY block_id",
            target = report::COLUMN_TARGET_ORIGIN,
            table = report::REPORT_TABLE,
        ))?;
        let rows = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the report table reads")
}

/// One assembled assistant over the silent provider — every stream request
/// accepted and never answered — under the given answering mode and
/// budgets: the owed message stays the tail, so a stamp is observable
/// without racing an answer.
async fn silent_fixture(
    answering: AnsweringMode,
    protection: ProtectionConfig,
) -> support::Fixture {
    support::start_assistant_config(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            answering,
            protection,
            ..support::assembly_config()
        },
    )
    .await
}

/// The one conversation of a single-channel test.
async fn only_conversation(store: &Store) -> i64 {
    let conversations = store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    assert_eq!(conversations.len(), 1, "one channel, one conversation");
    conversations[0].id
}

/// The raw join rows of one principal: the shown name, the handle, the
/// event origin and the send time, as the erasure pins read them.
async fn stored_join_rows(
    store: &Store,
    principal_id: i64,
) -> Vec<(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        let mut statement = conn.prepare(&format!(
            "SELECT {name}, {handle}, {origin}, {joined_at} FROM {table} \
             WHERE {principal} = ?1 ORDER BY block_id",
            name = join::COLUMN_NAME,
            handle = join::COLUMN_HANDLE,
            origin = join::COLUMN_ORIGIN,
            joined_at = join::COLUMN_JOINED_AT,
            table = join::JOIN_NOTICE_TABLE,
            principal = join::COLUMN_PRINCIPAL_ID,
        ))?;
        let rows = statement
            .query_map([principal_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the join table reads")
}

// ─── AC2: the join lands, marked, through the observation seam ───────────

/// One service message naming four joiners lands four blocks: the name,
/// the handle, the resolved principal, the shared event origin and the
/// platform send time on each — and one projected system line apiece,
/// opening with the event's bracketed id, in every naming shape the
/// platform delivers: a name with a handle, a handle with no name, a name
/// with no handle, and neither.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_join_event_lands_one_marked_block_per_joiner() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-joins").await;

    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-1",
        vec![
            joiner("j-1", Some("ada"), "Ada Lovelace"),
            joiner("j-2", Some("bo"), ""),
            joiner("j-3", None, "Grace Hopper"),
            joiner("j-4", None, ""),
        ],
    )
    .await;

    let conversation = only_conversation(&fixture.store).await;
    let joins = join_blocks(&fixture.store, conversation).await;
    assert_eq!(joins.len(), 4, "one block per joiner, never one per event");
    for block in &joins {
        assert_eq!(
            field(block, join::COLUMN_ORIGIN),
            "origin-join-1",
            "every joiner of one service message shares its origin"
        );
        assert!(
            !field(block, join::COLUMN_JOINED_AT).is_empty(),
            "the platform's send time is recorded beside the store's own"
        );
    }
    let principals: Vec<i64> = joins
        .iter()
        .map(|block| {
            block.fields[join::COLUMN_PRINCIPAL_ID]
                .as_i64()
                .expect("each joiner resolves a principal")
        })
        .collect();
    let mut distinct = principals.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 4, "four joiners, four principals");

    assert_eq!(field(&joins[0], join::COLUMN_NAME), "Ada Lovelace");
    assert_eq!(field(&joins[0], join::COLUMN_HANDLE), "ada");
    let lines: Vec<Option<String>> = joins.iter().map(projected_line).collect();
    assert_eq!(
        lines,
        vec![
            Some("A member joined the group: Ada Lovelace (@ada)".to_owned()),
            Some("A member joined the group: @bo".to_owned()),
            Some("A member joined the group: Grace Hopper".to_owned()),
            Some("A member joined the group.".to_owned()),
        ],
        "each joiner projects one platform-fact line"
    );
    for line in joins.iter().filter_map(projected) {
        assert!(
            line.contains("msgid: origin-join-1"),
            "and each declares the event's own id, which a report names it \
             by: {line}"
        );
    }
    assert_eq!(
        JoinNotice::parse(&joins[0]).group_role(),
        Some(Role::System),
        "a join is stated in the ledger's own system voice"
    );
}

/// A join in a group the operator never admitted stores nothing and draws
/// the withdraw directive — the observation surface's existing gate, with
/// a join as one more thing behind it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_join_in_an_unadmitted_group_stores_nothing() {
    let fixture = support::start_assistant(None).await;
    let key = channel("room-join-stranger");

    let outcome = fixture
        .assistant
        .observe(joined(
            &key,
            "origin-join-stranger",
            vec![joiner("j-9", Some("nobody"), "A Stranger")],
        ))
        .await
        .expect("the join observation is judged");
    assert_eq!(outcome, ObserveOutcome::Withdraw);
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "an unadmitted group's join creates no conversation and no row"
    );
}

/// Both transports deliver at least once, so the same join service message
/// arrives twice whenever an acknowledgment is lost: the redelivery stores
/// nothing at all — not a second block, not a second row for either joiner
/// — while a genuinely different event behind it still lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_redelivered_join_event_stores_nothing_new() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-redelivered").await;
    let event = || {
        vec![
            joiner("j-r1", Some("ada"), "Ada Lovelace"),
            joiner("j-r2", Some("bo"), "Grace Hopper"),
        ]
    };

    observe_join(&fixture.assistant, &key, "origin-join-12", event()).await;
    let conversation = only_conversation(&fixture.store).await;
    let first = join_blocks(&fixture.store, conversation).await;
    assert_eq!(first.len(), 2, "the event's two joiners land once each");

    observe_join(&fixture.assistant, &key, "origin-join-12", event()).await;
    let after = join_blocks(&fixture.store, conversation).await;
    assert_eq!(
        after.len(),
        2,
        "the redelivered service message records nothing new"
    );
    assert_eq!(
        after.iter().map(|block| block.id).collect::<Vec<i64>>(),
        first.iter().map(|block| block.id).collect::<Vec<i64>>(),
        "the stored blocks are the first delivery's own"
    );

    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-13",
        vec![joiner("j-r3", None, "Alan Turing")],
    )
    .await;
    assert_eq!(
        join_blocks(&fixture.store, conversation).await.len(),
        3,
        "the check is per event: a different service message still records"
    );
}

/// The tool-choice supersession fires on an observed join like every other
/// first activity (decided 2026-08-23): a conversation carrying an older
/// process's choice gains the current one when someone walking in is what
/// this process sees first — the delta append landing ahead of the join
/// blocks, exactly as it lands ahead of a message.
#[test]
fn a_join_supersedes_a_stale_choice_like_any_other_first_activity() {
    let db = support::TempDb::new("join-choice");
    let key = channel("room-join-choice");

    // Process one, a moderating deployment: the group's choice names the
    // report tool among the rest.
    let conversation = support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the first store opens");
        let fixture = support::start_assistant_reporting(
            store,
            support::silent_provider(),
            support::ScriptHandle::fresh(),
            support::production_toolset(),
            ProtectionConfig::default(),
        )
        .await;
        support::authorize(&fixture.assistant, &key).await;
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            inbound_unaddressed(&key, ChannelKind::Group, "42", "recorded under the handle"),
        )
        .await;
        assert!(
            stored_choices(&fixture.store, receipt.conversation_id)
                .await
                .last()
                .expect("the creation choice stands")
                .contains(&report::NAME.to_owned()),
            "the moderating process wrote the report tool into the choice"
        );
        receipt.conversation_id
    });

    // Process two, no moderation handle, and a JOIN is the first thing it
    // sees in this group.
    support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store reopens");
        let fixture = support::start_assistant_full(
            store,
            support::silent_provider(),
            support::ScriptHandle::fresh(),
            support::production_toolset(),
            ProtectionConfig::default(),
        )
        .await;
        observe_join(
            &fixture.assistant,
            &key,
            "origin-join-14",
            vec![joiner("j-p1", Some("ada"), "Ada Lovelace")],
        )
        .await;
        let choices = stored_choices(&fixture.store, conversation).await;
        assert_eq!(choices.len(), 2, "the join's activity superseded it");
        assert!(
            !choices[1].contains(&report::NAME.to_owned()),
            "the delta carries this process's registered set: {:?}",
            choices[1]
        );
        let blocks = fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let choice_at = blocks
            .iter()
            .rposition(|block| block.block_type == "tool_choice")
            .expect("the delta choice stands");
        let join_at = blocks
            .iter()
            .position(|block| block.block_type == join::JOIN_NOTICE_KIND)
            .expect("the join stands");
        assert!(
            choice_at < join_at,
            "the delta lands ahead of the join it was triggered by"
        );
    });
}

/// A join inside the window reaches the model: the turn a later message
/// summons carries the join's projected line in its request, under the
/// same bracketed id the report tool takes as its parameter.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_windowed_join_reaches_the_models_request() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-context").await;
    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-2",
        vec![joiner("j-5", Some("ada"), "Ada Lovelace")],
    )
    .await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "who just joined?"),
    )
    .await;
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the answered turn behind a join",
        5,
    )
    .await;

    let requests = fixture.script.seen.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|messages| messages.iter().any(|message| {
                support::carries(message, "msgid: origin-join-2")
                    && support::carries(message, "A member joined the group: Ada Lovelace (@ada)")
            })),
        "the join's marked line rides the request the turn composed"
    );
}

// ─── AC2 and AC5: a suppressed person's join is not stored at all ────────

/// A mixed event: one joiner's suppression flag stands, the other's does
/// not. Only the unsuppressed joiner's block exists — no name, no handle,
/// no principal refresh for the flagged person — and the event itself
/// stands, with no departure and no reaction of any kind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_suppressed_joiners_block_never_exists_and_the_event_stands() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-suppressed").await;

    // The flag is raised the only way a person can raise it: their own
    // rights command.
    let outcome = fixture
        .assistant
        .ingest(with_command(
            inbound_unaddressed(&key, ChannelKind::Group, "j-out", privacy::OPT_OUT_COMMAND),
            privacy::OPT_OUT_COMMAND,
        ))
        .await
        .expect("the opt-out ingests");
    assert!(
        matches!(outcome, IngestOutcome::Recorded { .. }),
        "the rights command is recorded and answered"
    );

    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-3",
        vec![
            joiner("j-out", Some("quiet"), "The Opted Out"),
            joiner("j-in", Some("ada"), "Ada Lovelace"),
        ],
    )
    .await;

    let conversation = only_conversation(&fixture.store).await;
    let joins = join_blocks(&fixture.store, conversation).await;
    assert_eq!(joins.len(), 1, "the flagged joiner's block never exists");
    assert_eq!(field(&joins[0], join::COLUMN_NAME), "Ada Lovelace");
    assert_eq!(
        field(&joins[0], join::COLUMN_ORIGIN),
        "origin-join-3",
        "the co-joiner's block keeps the shared event"
    );

    let stored: Vec<String> = domain_run(&fixture.store.tx(), DOMAIN, |conn| {
        let mut statement = conn.prepare(&format!(
            "SELECT {name} FROM {table}",
            name = join::COLUMN_NAME,
            table = join::JOIN_NOTICE_TABLE,
        ))?;
        let rows = statement
            .query_map([], |row| row.get::<_, Option<String>>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().flatten().collect())
    })
    .await
    .expect("the join table reads");
    assert_eq!(
        stored,
        vec!["Ada Lovelace".to_owned()],
        "the flagged person's shown name is nowhere in the store"
    );
}

// ─── AC3: a join wakes nothing and buries nothing ────────────────────────

/// A join arriving alone summons no turn, in either answering mode — the
/// helpful mode's every-message evaluation is about messages, and a join
/// simply never carries a summons.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_join_alone_summons_no_turn_in_either_mode() {
    for answering in [AnsweringMode::Addressed, AnsweringMode::Helpful] {
        let fixture = support::start_assistant_answering(
            Store::in_memory_with(store_config()).expect("an in-memory store opens"),
            None,
            ProtectionConfig::default(),
            answering,
        )
        .await;
        let key = support::authorized_group(&fixture.assistant, "room-join-quiet").await;
        observe_join(
            &fixture.assistant,
            &key,
            "origin-join-4",
            vec![joiner("j-6", Some("ada"), "Ada Lovelace")],
        )
        .await;

        let conversation = only_conversation(&fixture.store).await;
        let blocks = support::viewed_ledger(
            &fixture.store,
            conversation,
            "the join's own three blocks",
            |blocks| blocks.len() == 3,
        )
        .await;
        let kinds: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["system_prompt", "tool_choice", join::JOIN_NOTICE_KIND],
            "{answering:?}: the join is the whole of what a join writes"
        );
        assert_eq!(
            fixture
                .script
                .turns
                .load(std::sync::atomic::Ordering::SeqCst),
            0,
            "{answering:?}: a join owes no answer and draws no turn"
        );
    }
}

/// The dispatch frontier reads through a join: with the join as the stored
/// tail over an unanswered message, the framework's own drive still
/// reports the turn owed and awaiting the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_join_over_an_unanswered_message_leaves_the_turn_owed() {
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
    store
        .append_consumer_block(
            conversation,
            Some(Role::User),
            assistant_core::kind::CHAT_MESSAGE_KIND,
            assistant_core::kind::ChatMessage::stored_fields(
                "the owed ask",
                assistant_core::kind::RecordedSender {
                    principal_id: 1,
                    authority: Authority::Member,
                    speaker: None,
                },
                assistant_core::kind::RecordedOrigin::default(),
                None,
                "2026-08-29T00:00:00+00:00",
                assistant_core::kind::Stamp {
                    addressed: true,
                    literal_addressed: false,
                    limited: None,
                    answer_due: true,
                    debt_authority: Some(Authority::Member),
                },
            ),
            None,
        )
        .await
        .expect("the owed message appends");
    store
        .append_consumer_block(
            conversation,
            None,
            join::JOIN_NOTICE_KIND,
            JoinNotice::stored_fields(
                join::RecordedJoiner {
                    principal_id: 2,
                    name: "Ada Lovelace",
                    handle: Some("ada"),
                },
                "origin-join-5",
                "2026-08-29T00:00:00+00:00",
            ),
            None,
        )
        .await
        .expect("the join appends on top");

    let tail = store
        .latest_block(conversation)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(
        tail.block_type,
        join::JOIN_NOTICE_KIND,
        "non-vacuity: the join really is the tail the frontier reads"
    );

    let ctx: AgencyCtx<CoreEvent> = AgencyCtx {
        conversation_id: conversation,
        store,
        bus: Arc::new(EventBus::new()),
    };
    let outcome = ratchet::drive::<assistant_core::kind::AssistantKind, CoreEvent>(&ctx)
        .await
        .expect("the drive runs")
        .outcome()
        .expect("the conversation still exists");
    assert!(
        outcome.owes_turn,
        "the turn is still owed through the transparent join"
    );
    assert_eq!(outcome.awaiting, Some(Awaiting::Model));
}

/// The ingestion's owing-tail walk reads through a join, in both answering
/// modes. Addressed mode: the next message summons nothing of its own, so
/// its answer-due can only be the propagated debt. Helpful mode: the
/// sender's budget refuses the next message's OWN debt, so again only the
/// propagated debt can stamp it — without the join in the read-through
/// list, both would rest and the owed question would die behind someone
/// walking in.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_debt_behind_a_join_still_propagates_in_either_mode() {
    for (answering, protection) in [
        (AnsweringMode::Addressed, ProtectionConfig::default()),
        (
            AnsweringMode::Helpful,
            support::budgets(Some((1, 600)), None),
        ),
    ] {
        // The silent provider never answers, so the owed message stays
        // owed and no answer races the stamp under test.
        let fixture = silent_fixture(answering, protection).await;
        let key = support::authorized_group(&fixture.assistant, "room-join-debt").await;
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            inbound(&key, ChannelKind::Group, "asker", "the owed ask"),
        )
        .await;
        observe_join(
            &fixture.assistant,
            &key,
            "origin-join-6",
            vec![joiner("j-7", Some("ada"), "Ada Lovelace")],
        )
        .await;
        let follow_up = match answering {
            AnsweringMode::Addressed => inbound_unaddressed(
                &key,
                ChannelKind::Group,
                "asker",
                "an aside behind the join",
            ),
            AnsweringMode::Helpful => inbound(
                &key,
                ChannelKind::Group,
                "asker",
                "an aside behind the join",
            ),
        };
        support::ingest_recorded(&fixture.assistant, follow_up).await;

        let blocks = fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the ledger reads");
        let aside = blocks
            .iter()
            .find(|block| block.fields.get("text") == Some(&json!("an aside behind the join")))
            .expect("the aside is recorded");
        assert_eq!(
            aside.fields["answer_due"],
            json!(true),
            "{answering:?}: the debt behind the join reaches the next message's stamp"
        );
    }
}

// ─── AC4: the report path reaches a join, and nobody else feels it ───────

/// A turn whose window carried a single-joiner event files the existing
/// report against that event: the block names the event origin and the one
/// joiner it resolves, and the edge delivers the fixed line threaded onto
/// the join itself — the message the human side acts on.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_windowed_join_is_reported_against_its_event() {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: report::NAME.into(),
            input: r#"{"message_id":"origin-join-7"}"#.into(),
            narration: None,
            announce: None,
        },
        None,
    );
    let fixture = support::start_assistant_reporting(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let mut replies = support::outbound(&fixture).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-report").await;

    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-7",
        vec![joiner("spam-1", Some("free_crypto"), "FREE CRYPTO SIGNALS")],
    )
    .await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "member-1", "an ordinary line"),
    )
    .await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the assessed turn over a join",
        &[
            "system_prompt",
            "tool_choice",
            join::JOIN_NOTICE_KIND,
            "chat_message",
            "tool_call",
            report::REPORT_KIND,
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[5], report::COLUMN_TARGET_ORIGIN),
        "origin-join-7"
    );
    assert_eq!(
        blocks[5].fields[report::COLUMN_REPORTED_PRINCIPAL_ID],
        blocks[2].fields[join::COLUMN_PRINCIPAL_ID],
        "a single-joiner event names that joiner, so erasure reaches the report"
    );
    assert_eq!(
        field(&blocks[5], report::COLUMN_LINE),
        report::report_line(support::MODERATION_HANDLE)
    );
    assert_eq!(field(&blocks[6], "content"), report::FILED_RESULT);

    let filed = recv_reply(&mut replies).await;
    assert_eq!(filed.kind, ReplyKind::Report);
    assert_eq!(
        filed.reply_target,
        Some(ReplyThread::OntoOnly("origin-join-7".into())),
        "the report goes out threaded onto the join the group saw"
    );
}

/// A plural event attaches no single principal: the filing names the
/// event, the model's own words name which joiner offends, and no record
/// puts the wrong person's name in a moderation trail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_plural_join_event_files_once_and_names_no_single_person() {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: report::NAME.into(),
            input: r#"{"message_id":"origin-join-8"}"#.into(),
            narration: None,
            announce: None,
        },
        None,
    );
    let fixture = support::start_assistant_reporting(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-join-plural").await;

    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-8",
        vec![
            joiner("spam-2", Some("free_crypto_2"), "FREE CRYPTO SIGNALS"),
            joiner("guest-2", Some("ada"), "Ada Lovelace"),
        ],
    )
    .await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "member-2", "an ordinary line"),
    )
    .await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the assessed turn over a plural join",
        &[
            "system_prompt",
            "tool_choice",
            join::JOIN_NOTICE_KIND,
            join::JOIN_NOTICE_KIND,
            "chat_message",
            "tool_call",
            report::REPORT_KIND,
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[6], report::COLUMN_TARGET_ORIGIN),
        "origin-join-8"
    );
    assert_eq!(
        blocks[6].fields.get(report::COLUMN_REPORTED_PRINCIPAL_ID),
        None,
        "a plural event attaches no single principal"
    );
    let reports = blocks
        .iter()
        .filter(|block| block.block_type == report::REPORT_KIND)
        .count();
    assert_eq!(reports, 1, "one filing per event");
}

/// The spot-pin behind AC4's by-construction claim: a turn whose window
/// carries a join discloses and threads exactly as one without it. The
/// join is not a person this turn counted — it changed neither the
/// first-contact line nor the message the answer replies to.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_windowed_join_changes_neither_the_disclosure_nor_the_threading() {
    let fixture = support::start_assistant(None).await;
    let mut replies = support::outbound(&fixture).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-threading").await;
    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-9",
        vec![joiner("j-8", Some("ada"), "Ada Lovelace")],
    )
    .await;

    support::ingest_recorded(
        &fixture.assistant,
        support::with_origin(
            inbound(
                &key,
                ChannelKind::Group,
                "asker",
                &format!("the first question {cue}", cue = support::REPLY_CUE),
            ),
            "origin-asker-1",
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text,
        support::first_answer_to(&format!(
            "the first question {cue}",
            cue = support::REPLY_CUE
        )),
        "the first message to this person still opens with the disclosure line, once"
    );
    assert_eq!(
        reply.reply_target,
        Some(ReplyThread::OntoOrPlainly("origin-asker-1".into())),
        "the send threads onto the message the model named"
    );
}

// ─── AC5: erasure reaches a join, by person and by event ─────────────────

/// The person-keyed pass: one joiner's block is nulled — name, handle,
/// event origin and send time — while a co-joiner of the SAME event keeps
/// theirs whole, the erased join projects nothing at all, and a member's
/// welcome reply loses its stored copy of the erased joiner's event id.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasing_a_joiner_nulls_their_block_their_event_reference_and_nothing_else() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-erasure").await;
    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-10",
        vec![
            joiner("erased-1", Some("ada"), "Ada Lovelace"),
            joiner("kept-1", Some("bo"), "Grace Hopper"),
        ],
    )
    .await;
    let conversation = only_conversation(&fixture.store).await;
    let joins = join_blocks(&fixture.store, conversation).await;
    let erased_principal = joins[0].fields[join::COLUMN_PRINCIPAL_ID]
        .as_i64()
        .expect("the joiner resolved a principal");
    let kept_principal = joins[1].fields[join::COLUMN_PRINCIPAL_ID]
        .as_i64()
        .expect("the co-joiner resolved a principal");

    // A welcome reply: ordinary, and it stores the event's own id.
    support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            inbound_unaddressed(&key, ChannelKind::Group, "greeter", "welcome!"),
            ReplyTarget::Message {
                origin: "origin-join-10".into(),
            },
        ),
    )
    .await;

    // The co-joiner's whole row as it stood BEFORE the pass. The send time
    // is the platform's own clock, so the only value the pin can compare it
    // against is the one read before erasure ran.
    let kept_before = stored_join_rows(&fixture.store, kept_principal).await;
    assert_eq!(kept_before.len(), 1, "the co-joiner stored exactly one row");
    assert_eq!(
        (
            kept_before[0].0.as_deref(),
            kept_before[0].1.as_deref(),
            kept_before[0].2.as_deref(),
        ),
        (Some("Grace Hopper"), Some("bo"), Some("origin-join-10")),
        "the co-joiner's row stands whole before the erasure"
    );
    assert!(
        kept_before[0].3.is_some(),
        "the co-joiner's send time is stored before the erasure"
    );

    let outcome = fixture
        .assistant
        .erase_principal(erased_principal)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));

    assert_eq!(
        stored_join_rows(&fixture.store, erased_principal).await,
        vec![(None, None, None, None)],
        "the erased joiner's block keeps its place and names nobody"
    );
    assert_eq!(
        stored_join_rows(&fixture.store, kept_principal).await,
        kept_before,
        "the co-joiner of the same event is untouched, the send time included"
    );

    let joins = join_blocks(&fixture.store, conversation).await;
    assert_eq!(
        projected(&joins[0]),
        None,
        "an erased join projects nothing at all"
    );
    assert_eq!(
        projected_line(&joins[1]),
        Some("A member joined the group: Grace Hopper (@bo)".to_owned())
    );
    assert!(
        projected(&joins[1]).is_some_and(|line| line.contains("msgid: origin-join-10")),
        "the surviving row keeps the event's id"
    );

    let reply_targets: Vec<Option<String>> = domain_run(&fixture.store.tx(), DOMAIN, |conn| {
        let mut statement = conn.prepare(
            "SELECT reply_target FROM block_chat_message WHERE reply_target IS NOT NULL",
        )?;
        let rows = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the message table reads");
    assert!(
        reply_targets.is_empty(),
        "the greeter's stored copy of the erased joiner's event id is nulled with them"
    );
}

/// The deletion mirror's origin pass: an administrator's reply deletion
/// command naming a join service message nulls the WHOLE event — deleting
/// the message removes the event, not one joiner's part of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_deletion_mirror_nulls_a_whole_join_event() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-join-mirror").await;
    observe_join(
        &fixture.assistant,
        &key,
        "origin-join-11",
        vec![
            joiner("gone-1", Some("ada"), "Ada Lovelace"),
            joiner("gone-2", Some("bo"), "Grace Hopper"),
        ],
    )
    .await;
    let conversation = only_conversation(&fixture.store).await;
    let joins = join_blocks(&fixture.store, conversation).await;
    let principals: Vec<i64> = joins
        .iter()
        .map(|block| block.fields[join::COLUMN_PRINCIPAL_ID].as_i64().unwrap())
        .collect();

    support::ingest_recorded(
        &fixture.assistant,
        with_command(
            with_reply(
                inbound_as(
                    &key,
                    ChannelKind::Group,
                    "admin-1",
                    Authority::Admin,
                    assistant_core::mirror::DELETION_COMMAND,
                ),
                ReplyTarget::Message {
                    origin: "origin-join-11".into(),
                },
            ),
            assistant_core::mirror::DELETION_COMMAND,
        ),
    )
    .await;

    for principal in principals {
        assert_eq!(
            stored_join_rows(&fixture.store, principal).await,
            vec![(None, None, None, None)],
            "the deleted service message takes the whole event with it"
        );
    }
    let joins = join_blocks(&fixture.store, conversation).await;
    assert!(
        joins.iter().all(|block| projected(block).is_none()),
        "an erased event projects nothing"
    );
}

/// A filed report against a plural join event — the shape whose report row
/// holds NO reported person, because naming one of several joiners would
/// record the wrong one. Two joiners under one event origin, a turn that
/// assessed them, the model's one filing; the caller gets the fixture, the
/// group and the conversation the filing stands in.
///
/// It is the setup for the two reachability pins below: with no reported
/// principal stored, the person-keyed report pass cannot reach this row,
/// and the filed target is a verbatim copy of the joiners' own event id.
async fn a_filed_plural_event(room: &str, origin: &str) -> (support::Fixture, ChannelKey, i64) {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: report::NAME.into(),
            input: format!(r#"{{"message_id":"{origin}"}}"#),
            narration: None,
            announce: None,
        },
        None,
    );
    let fixture = support::start_assistant_reporting(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, room).await;
    observe_join(
        &fixture.assistant,
        &key,
        origin,
        vec![
            joiner("spam-3", Some("free_crypto_3"), "FREE CRYPTO SIGNALS"),
            joiner("guest-3", Some("ada"), "Ada Lovelace"),
        ],
    )
    .await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "member-3", "an ordinary line"),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the filed report over a plural join",
        &[
            "system_prompt",
            "tool_choice",
            join::JOIN_NOTICE_KIND,
            join::JOIN_NOTICE_KIND,
            "chat_message",
            "tool_call",
            report::REPORT_KIND,
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(
        blocks[6].fields.get(report::COLUMN_REPORTED_PRINCIPAL_ID),
        None,
        "the premise: a plural event's filing names no single person"
    );
    assert_eq!(
        stored_report_targets(&fixture.store).await,
        vec![Some(origin.to_owned())],
        "the premise: the filing holds the event's own id"
    );
    (fixture, key, receipt.conversation_id)
}

/// The person-keyed pass over a PLURAL event, the whole reach: erasing
/// one of two joiners nulls their own row and leaves the co-joiner's
/// standing — the event stands, so the co-joiner's line still projects —
/// and nulls the filed report's target, which no principal-keyed pass
/// could ever have reached, because the plural filing named nobody. Left
/// standing, that target would be a verbatim copy of the erased person's
/// own event id in a moderation record.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasing_one_joiner_of_a_reported_event_nulls_the_filing_and_spares_the_co_joiner() {
    let (fixture, _key, conversation) =
        a_filed_plural_event("room-join-report-erasure", "origin-join-15").await;
    let joins = join_blocks(&fixture.store, conversation).await;
    let erased_principal = joins[0].fields[join::COLUMN_PRINCIPAL_ID]
        .as_i64()
        .expect("the reported joiner resolved a principal");
    let kept_principal = joins[1].fields[join::COLUMN_PRINCIPAL_ID]
        .as_i64()
        .expect("the co-joiner resolved a principal");
    let kept_before = stored_join_rows(&fixture.store, kept_principal).await;

    let outcome = fixture
        .assistant
        .erase_principal(erased_principal)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));

    assert_eq!(
        stored_join_rows(&fixture.store, erased_principal).await,
        vec![(None, None, None, None)],
        "the erased joiner's row names nobody"
    );
    assert_eq!(
        stored_join_rows(&fixture.store, kept_principal).await,
        kept_before,
        "the co-joiner of the same event is untouched"
    );
    assert_eq!(
        stored_report_targets(&fixture.store).await,
        vec![None],
        "the plural event's filing loses its target with the person it named"
    );
    let joins = join_blocks(&fixture.store, conversation).await;
    assert_eq!(projected(&joins[0]), None);
    assert_eq!(
        projected_line(&joins[1]),
        Some("A member joined the group: Ada Lovelace (@ada)".to_owned()),
        "the event stands for the joiner who did not ask to be forgotten"
    );
    assert!(
        projected(&joins[1]).is_some_and(|line| line.contains("msgid: origin-join-15")),
        "and it keeps the event's id"
    );
}

/// The origin-keyed mirror over a reported event: an administrator's
/// deletion command naming the join service message nulls the whole event
/// AND the filed report's target — the report threads onto the very
/// record that just went away, and a copy of a deleted id left in a report
/// row is exactly the residual decision 0085 closed for every other holder.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deleting_a_reported_join_event_nulls_the_filings_target_too() {
    let (fixture, key, conversation) =
        a_filed_plural_event("room-join-report-mirror", "origin-join-16").await;

    support::ingest_recorded(
        &fixture.assistant,
        with_command(
            with_reply(
                inbound_as(
                    &key,
                    ChannelKind::Group,
                    "admin-2",
                    Authority::Admin,
                    assistant_core::mirror::DELETION_COMMAND,
                ),
                ReplyTarget::Message {
                    origin: "origin-join-16".into(),
                },
            ),
            assistant_core::mirror::DELETION_COMMAND,
        ),
    )
    .await;

    assert_eq!(
        stored_report_targets(&fixture.store).await,
        vec![None],
        "the filing against the deleted event goes undeliverable"
    );
    let joins = join_blocks(&fixture.store, conversation).await;
    assert!(
        joins.iter().all(|block| projected(block).is_none()),
        "the deleted service message took the whole event with it"
    );
}
