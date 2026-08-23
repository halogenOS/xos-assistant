//! The group-context unit at the core's edges: the persisted fail-closed
//! authorization (AC5), the context notes with the rules contract and the
//! acknowledgment (AC2, AC3), the frontier transparency and the stamp's
//! walk-through (AC4), the privacy command (AC6), and the observation
//! path's locks and creation (AC7). The adapter-side halves of AC2 and AC5
//! live in the adapter suite; the pure rules-contract table lives beside
//! the contract in the note module.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_ledger::agency::ratchet;
use agent_ledger::providers::{Message, MessageContent, MessageRole};
use agent_ledger::{AgencyCtx, Awaiting, Block, CoreEvent, EventBus, Role, Store, StreamEvent};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::note::{CONTEXT_NOTE_KIND, ContextNote, NoteTopic};
use assistant_core::schema::store_config;
use assistant_core::{
    ACKNOWLEDGMENT_WINDOW, Assistant, Authority, ChannelKey, ChannelKind, CoreError, DeliveryItem,
    FailureKind, IngestOutcome, NOTE_TOPIC_APPEND_CAP, Observation, ObserveOutcome, ObservedFact,
    OperatorConfig, PRIVACY_UNPUBLISHED, RULES_ACKNOWLEDGMENT,
};
use serde_json::json;

use crate::support::{
    self, added_by, authorize, await_ledger, channel, inbound, inbound_unaddressed, recv_reply,
    with_command,
};

/// One observation on the given channel, group-kind — the shape every
/// group fact travels in.
fn observed(key: &ChannelKey, fact: ObservedFact) -> Observation {
    Observation {
        channel: key.clone(),
        channel_kind: ChannelKind::Group,
        fact,
    }
}

/// How many unlatch intents sit on the bus right now — the latch pin: an
/// ingestion emits its intent before returning, so a drain right after the
/// call is deterministic for that message.
fn drain_unlatches(events: &mut tokio::sync::broadcast::Receiver<CoreEvent>) -> usize {
    let mut unlatches = 0;
    while let Ok(event) = events.try_recv() {
        if matches!(event, CoreEvent::UnlatchRequested { .. }) {
            unlatches += 1;
        }
    }
    unlatches
}

/// The context-note blocks of one conversation, oldest first.
async fn notes(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == CONTEXT_NOTE_KIND)
        .collect()
}

/// The conversation a channel key maps to, read back through the store's
/// public conversation list plus the ledger — the suite asserts through
/// public surfaces only, so the mapping is resolved by the one conversation
/// that exists in these single-channel tests.
async fn only_conversation(store: &Store) -> i64 {
    let conversations = store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    assert_eq!(conversations.len(), 1, "one channel, one conversation");
    conversations[0].id
}

// ─── AC5: authorization, persistent and fail-closed ──────────────────────

/// The operator's add writes the row and stands across a restart: the
/// second process answers the group without any new membership observation,
/// because the authorization is a table row, not process memory.
#[test]
fn the_operators_add_admits_the_group_and_the_admission_survives_a_restart() {
    let db = support::TempDb::new("authorization-restart");
    let key = channel("group-restart");

    let first = support::process_runtime();
    first.block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        let fixture = support::start_assistant_on(store, None).await;
        authorize(&fixture.assistant, &key).await;
    });
    drop(first);

    let second = support::process_runtime();
    second.block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store reopens");
        let fixture = support::start_assistant_on(store, None).await;
        let outcome = fixture
            .assistant
            .ingest(inbound(&key, ChannelKind::Group, "42", "still admitted?"))
            .await
            .expect("the message ingests");
        assert!(
            matches!(outcome, IngestOutcome::Recorded { .. }),
            "the restarted process still holds the admission; got {outcome:?}"
        );
    });
}

/// Every inadmissible membership observation returns the withdraw directive
/// and records nothing: a foreign adder, a missing adder, and — on an
/// assembly with no operator configured — even the operator's own id. A
/// replayed inadmissible add re-returns the directive idempotently, and the
/// group stays refused afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_foreign_add_a_missing_adder_and_no_operator_each_draw_the_withdraw_directive() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-foreign");

    let foreign = fixture
        .assistant
        .observe(added_by(&key, "a-stranger"))
        .await
        .expect("the foreign add is judged");
    assert_eq!(foreign, ObserveOutcome::Withdraw);
    let replayed = fixture
        .assistant
        .observe(added_by(&key, "a-stranger"))
        .await
        .expect("the replayed add is judged");
    assert_eq!(replayed, ObserveOutcome::Withdraw, "idempotent re-return");

    let nameless = fixture
        .assistant
        .observe(observed(&key, ObservedFact::Added { by: None }))
        .await
        .expect("the nameless add is judged");
    assert_eq!(
        nameless,
        ObserveOutcome::Withdraw,
        "no adder, no invitation"
    );

    // Nothing was recorded: the group's next contact is still refused.
    let message = fixture
        .assistant
        .ingest(inbound(&key, ChannelKind::Group, "42", "am I admitted?"))
        .await
        .expect("the message is judged");
    assert_eq!(message, IngestOutcome::Withdraw);

    // With no operator configured at all, the operator's own id admits
    // nothing either.
    let bare = support::start_assistant_operators(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        fixture.script.clone(),
        support::production_toolset(),
        assistant_core::ProtectionConfig::default(),
        OperatorConfig::default(),
        None,
    )
    .await;
    let unconfigured = bare
        .assistant
        .observe(added_by(&key, support::OPERATOR))
        .await
        .expect("the add is judged");
    assert_eq!(unconfigured, ObserveOutcome::Withdraw);
}

/// A group message for a channel with no authorization row is refused
/// without touching anything: no conversation, no mapping, no identity row,
/// no block — and the refusal carries the withdraw directive.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_group_message_without_authorization_is_refused_touching_nothing() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-stranger");

    let outcome = fixture
        .assistant
        .ingest(inbound(
            &key,
            ChannelKind::Group,
            "42",
            "hello from a stranger group",
        ))
        .await
        .expect("the message is judged");
    assert_eq!(outcome, IngestOutcome::Withdraw);

    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "no conversation was mapped for the refused group"
    );
    let principals: i64 = agent_ledger::store::domain_run(
        &fixture.store.tx(),
        assistant_core::schema::DOMAIN,
        |conn| Ok(conn.query_row("SELECT COUNT(*) FROM principals", [], |row| row.get(0))?),
    )
    .await
    .expect("the identity table reads");
    assert_eq!(principals, 0, "no identity row for a refused message");
}

/// An observation for an unadmitted group is refused the same way, and a
/// direct-channel observation observes nothing at all — the direct path is
/// untouched by this unit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unauthorized_observation_withdraws_and_a_direct_observation_observes_nothing() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-unadmitted");

    let outcome = fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::Title("A stranger group".into()),
        ))
        .await
        .expect("the observation is judged");
    assert_eq!(outcome, ObserveOutcome::Withdraw);

    let direct = fixture
        .assistant
        .observe(Observation {
            channel: channel("dm-observed"),
            channel_kind: ChannelKind::Direct,
            fact: ObservedFact::Title("nobody's group".into()),
        })
        .await
        .expect("the direct observation is judged");
    assert_eq!(direct, ObserveOutcome::Observed { deliver: None });
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "neither refused nor direct observations create conversations"
    );
}

/// An observation claiming the wrong kind for a mapped channel is refused
/// with the same terminal mismatch ingestion answers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observation_disagreeing_with_the_mapped_channel_kind_is_refused() {
    let fixture = support::start_assistant(None).await;
    let key = channel("dm-then-claimed-group");
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "a direct message"),
    )
    .await;
    recv_reply(
        &mut fixture
            .assistant
            .replies(support::ADAPTER)
            .await
            .expect("the outbound edge opens"),
    )
    .await;

    let refusal = fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::Title("claimed as a group".into()),
        ))
        .await
        .expect_err("the mis-claimed observation must be refused");
    assert!(
        matches!(refusal, CoreError::ChannelKindMismatch { .. }),
        "the refusal names the mismatch; got {refusal}"
    );
    assert_eq!(refusal.failure_kind(), FailureKind::Terminal);
}

/// The exact table shape a version-five binary left on disk, written out
/// verbatim on purpose: the widening step must be exercised against the
/// TRUE pre-upgrade table — the two-kind limited constraint above all — so
/// any drift in the step's rebuild DDL fails this pin instead of passing
/// against a table the current code already created widened.
const V5_CHAT_MESSAGE_DDL: &str = "
    CREATE TABLE block_chat_message_v5 (
        block_id             INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
        role                 TEXT,
        text                 TEXT,
        principal_id         INTEGER NOT NULL,
        authority            TEXT NOT NULL CHECK (authority IN ('member','moderator','admin')),
        origin               TEXT,
        sent_at              TEXT,
        addressed            INTEGER NOT NULL CHECK (addressed IN (0, 1)),
        answer_due           INTEGER NOT NULL CHECK (answer_due IN (0, 1)),
        limited              TEXT CHECK (limited IN ('principal','channel')),
        debt_authority       TEXT CHECK (debt_authority IN ('member','moderator','admin'))
    );
    INSERT INTO block_chat_message_v5
        SELECT block_id, role, text, principal_id, authority, origin, sent_at,
               addressed, answer_due, limited, debt_authority
        FROM block_chat_message;
    DROP TABLE block_chat_message;
    ALTER TABLE block_chat_message_v5 RENAME TO block_chat_message;
    CREATE INDEX idx_block_chat_message_principal_addressed
        ON block_chat_message(principal_id, addressed);";

/// The appended migration steps on a version-five store: the note and
/// authorization tables arrive, the existing group mapping is backfilled as
/// authorized — a stranger's group message is admitted without any
/// membership observation — and the widening step rebuilds the GENUINE
/// version-five table, proven two-kind before the reopen, into the widened
/// shape that accepts the command stamp.
#[test]
fn a_version_five_store_upgrades_with_the_backfill_and_the_widened_stamp() {
    let db = support::TempDb::new("v5-upgrade");
    let key = channel("group-backfilled");

    let first = support::process_runtime();
    first.block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        let fixture = support::start_assistant_on(store.clone(), None).await;
        // The old-regime shape: a group mapping that exists at migration
        // time. It is created through the current entry points, then the
        // unit's additions are dropped, the message table is rebuilt to
        // the explicit version-five DDL, and the version rewound — leaving
        // exactly what the previous unit's binary wrote.
        authorize(&fixture.assistant, &key).await;
        support::ingest_recorded(
            &fixture.assistant,
            inbound(
                &key,
                ChannelKind::Group,
                "42",
                "mapped under the old regime",
            ),
        )
        .await;
        agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
            conn.execute_batch(&format!(
                "DROP TABLE {note};
                 DROP TABLE group_authorizations;
                 DROP TABLE {report};
                 ALTER TABLE principals DROP COLUMN opted_out;
                 {V5_CHAT_MESSAGE_DDL}",
                note = assistant_core::note::CONTEXT_NOTE_TABLE,
                report = assistant_core::tools::report::REPORT_TABLE,
            ))?;
            // Non-vacuity: the rebuilt table really is the two-kind
            // version-five shape — the command stamp the widening step
            // exists for is refused by its CHECK.
            let refused = conn.execute(
                "INSERT INTO block_chat_message
                     (block_id, principal_id, authority, addressed, answer_due, limited)
                 VALUES (999999, 1, 'member', 1, 0, 'command')",
                [],
            );
            assert!(
                refused.is_err(),
                "the genuine version-five constraint refuses the command stamp"
            );
            Ok(())
        })
        .await
        .expect("the store rewinds to the previous unit's shape");
        support::rewind_domain_migration_version(&store, 5).await;
    });
    drop(first);

    let second = support::process_runtime();
    second.block_on(async {
        let store = Store::open_with(db.path(), store_config())
            .expect("the version-five store reopens under the shipped configuration");
        assert_eq!(
            support::domain_migration_version(&store).await,
            12,
            "the appended steps advanced the domain's version"
        );
        let fixture = support::start_assistant_on(store.clone(), None).await;

        // The backfill admitted the mapped group: a stranger's message is
        // recorded, not withdrawn.
        let outcome = fixture
            .assistant
            .ingest(inbound(
                &key,
                ChannelKind::Group,
                "7",
                "under the new regime",
            ))
            .await
            .expect("the message ingests");
        assert!(
            matches!(outcome, IngestOutcome::Recorded { .. }),
            "the backfilled mapping is authorized; got {outcome:?}"
        );

        // The widened constraint accepts the command stamp on the upgraded
        // table: the privacy command records with the command kind.
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            with_command(
                inbound_unaddressed(&key, ChannelKind::Group, "7", "/privacy"),
                "/privacy",
            ),
        )
        .await;
        let command = await_ledger(
            &fixture.store,
            receipt.conversation_id,
            "the recorded command",
            |blocks| {
                blocks
                    .iter()
                    .any(|block| block.fields.get("limited") == Some(&json!("command")))
            },
        )
        .await;
        assert!(
            command
                .iter()
                .any(|block| block.fields.get("text") == Some(&json!("/privacy"))),
            "the command row itself is stored on the upgraded table"
        );
    });
}

// ─── AC2: notes on the ledger, the acknowledgment, the projection ────────

/// The whole rules flow at the core edge: a rules-prefixed announcement
/// appends one note and returns exactly one acknowledgment; the same text
/// re-observed appends and acknowledges nothing; a changed text appends
/// again, silently inside the acknowledgment window — and the next turn
/// projects the notes to the model in the system voice, newest wording
/// authoritative.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rules_change_appends_on_delta_acknowledges_once_and_projects_in_the_system_voice() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("group-rules");
    authorize(&fixture.assistant, &key).await;

    let first = fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\n1. Be kind.".into()),
        ))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        first,
        ObserveOutcome::Observed {
            deliver: Some(DeliveryItem::Acknowledgment(
                RULES_ACKNOWLEDGMENT.to_owned()
            ))
        },
        "a fresh rules note carries the fixed acknowledgment, typed"
    );

    let unchanged = fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\n1. Be kind.".into()),
        ))
        .await
        .expect("the re-observed pin is judged");
    assert_eq!(
        unchanged,
        ObserveOutcome::Observed { deliver: None },
        "the same text appends and acknowledges nothing"
    );

    let changed = fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\n1. Be kind.\n2. Stay on topic.".into()),
        ))
        .await
        .expect("the changed pin is judged");
    assert_eq!(
        changed,
        ObserveOutcome::Observed { deliver: None },
        "a further delta inside the acknowledgment window appends silently"
    );

    let conversation = only_conversation(&fixture.store).await;
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(stored.len(), 2, "one note per delta, none for the repeat");
    assert_eq!(stored[0].fields["topic"], json!("rules"));
    assert_eq!(stored[0].fields["text"], json!("1. Be kind."));
    assert_eq!(
        stored[1].fields["text"],
        json!("1. Be kind.\n2. Stay on topic.")
    );

    // The next turn projects the notes in the system voice.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "what are the rules?"),
    )
    .await;
    recv_reply(&mut replies).await;
    let seen = fixture.script.seen.lock().expect("the request log locks");
    let request = seen.last().expect("the turn's request was recorded");
    let system_lines: Vec<String> = request
        .iter()
        .filter(|message| message.role == MessageRole::System)
        .map(message_text)
        .collect();
    assert!(
        system_lines
            .iter()
            .any(|line| line.contains("The group's rules are now:\n1. Be kind.\n2. Stay on topic.")),
        "the newest rules note reaches the model in the system voice: {system_lines:?}"
    );
    assert!(
        system_lines
            .iter()
            .any(|line| line.contains(support::SYSTEM_PROMPT)),
        "the system prompt survives beside the notes: {system_lines:?}"
    );
}

/// One projected message's whole text, in either content mode.
fn message_text(message: &Message) -> String {
    match &message.content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                agent_ledger::providers::ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// A title change appends its note on-delta and is never acknowledged.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_title_change_appends_its_note_and_is_never_acknowledged() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-title");
    authorize(&fixture.assistant, &key).await;

    for _ in 0..2 {
        let outcome = fixture
            .assistant
            .observe(observed(
                &key,
                ObservedFact::Title("The kernel room".into()),
            ))
            .await
            .expect("the title observation is judged");
        assert_eq!(outcome, ObserveOutcome::Observed { deliver: None });
    }
    let renamed = fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::Title("The halogen room".into()),
        ))
        .await
        .expect("the renamed title is judged");
    assert_eq!(
        renamed,
        ObserveOutcome::Observed { deliver: None },
        "title changes are not acknowledged"
    );

    let conversation = only_conversation(&fixture.store).await;
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(stored.len(), 2, "one note per title, none for the repeat");
    assert_eq!(stored[0].fields["text"], json!("The kernel room"));
    assert_eq!(stored[1].fields["text"], json!("The halogen room"));
}

/// The contract's refusals at the observation edge (AC3's stored half): a
/// non-prefixed announcement, an empty remainder and an over-bound text
/// each append nothing — the earlier rules note stands untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_or_non_rules_pin_appends_nothing() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-refusals");
    authorize(&fixture.assistant, &key).await;
    fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\nBe kind.".into()),
        ))
        .await
        .expect("the standing rules pin is judged");

    for pinned in [
        "This week's release schedule".to_owned(),
        "Rules:\n".to_owned(),
        format!(
            "Rules:\n{}",
            "r".repeat(assistant_core::note::RULES_TEXT_MAX_BYTES + 1)
        ),
    ] {
        let outcome = fixture
            .assistant
            .observe(observed(&key, ObservedFact::PinnedAnnouncement(pinned)))
            .await
            .expect("the pin is judged");
        assert_eq!(outcome, ObserveOutcome::Observed { deliver: None });
    }

    let conversation = only_conversation(&fixture.store).await;
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(stored.len(), 1, "only the standing rules note exists");
    assert_eq!(stored[0].fields["text"], json!("Be kind."));
}

// ─── AC4: the frontier reads through a note ──────────────────────────────

/// A note appended over an unanswered message buries nothing, pinned
/// against the framework walk itself: with the note as the stored tail, the
/// drive still reports the turn owed and awaiting the model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_note_over_an_unanswered_message_leaves_the_turn_owed() {
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
            CHAT_MESSAGE_KIND,
            assistant_core::kind::ChatMessage::stored_fields(
                "the owed ask",
                assistant_core::kind::RecordedSender {
                    principal_id: 1,
                    authority: Authority::Member,
                    speaker: None,
                },
                None,
                None,
                "2026-08-23T00:00:00+00:00",
                assistant_core::kind::Stamp {
                    addressed: true,
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
            CONTEXT_NOTE_KIND,
            ContextNote::stored_fields(NoteTopic::Rules, "Be kind."),
            None,
        )
        .await
        .expect("the note appends on top");

    // Non-vacuity: the note really is the stored tail the frontier reads.
    let tail = store
        .latest_block(conversation)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(tail.block_type, CONTEXT_NOTE_KIND);

    let ctx: AgencyCtx<CoreEvent> = AgencyCtx {
        conversation_id: conversation,
        store,
        bus: Arc::new(EventBus::new()),
    };
    let outcome = ratchet::drive::<assistant_core::kind::AssistantKind, CoreEvent>(&ctx)
        .await
        .expect("the drive runs");
    assert!(
        outcome.owes_turn,
        "the turn is still owed through the transparent note"
    );
    assert_eq!(
        outcome.awaiting,
        Some(Awaiting::Model),
        "the frontier's ask is the buried message's own"
    );
}

/// Debt propagation reads through a note at the stamp: an unaddressed
/// message arriving with a note on top of the owed tail still carries the
/// debt forward, at the carried authority's minimum.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn debt_propagation_reads_through_a_note_at_the_stamp() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    .expect("the assembly starts");
    let key = channel("group-propagation");
    authorize(&assistant, &key).await;

    let receipt = support::ingest_recorded(
        &assistant,
        support::inbound_as(
            &key,
            ChannelKind::Group,
            "A",
            Authority::Admin,
            "the owed ask",
        ),
    )
    .await;
    assistant
        .observe(observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\nBe kind.".into()),
        ))
        .await
        .expect("the note lands on top of the owed message");

    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "B", "an aside behind the note"),
    )
    .await;
    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks
        .iter()
        .find(|block| block.fields.get("text") == Some(&json!("an aside behind the note")))
        .expect("the aside is recorded");
    assert_eq!(
        aside.fields["answer_due"],
        json!(true),
        "the debt propagates through the note"
    );
    assert_eq!(
        aside.fields["debt_authority"],
        json!("member"),
        "the minimum rule folds the aside's member standing into the carried debt"
    );
}

// ─── AC6: the privacy command ────────────────────────────────────────────

/// The configured address answers with the fixed line, deterministically:
/// no turn fires, the boot latch stays untouched, and the next addressed
/// message is answered normally.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_privacy_command_answers_the_configured_address_without_a_turn() {
    let (provider, script) = support::scripted_provider(None);
    let fixture = support::start_assistant_operators(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        support::production_toolset(),
        assistant_core::ProtectionConfig::default(),
        support::operator_config(),
        Some("https://example.org/privacy".into()),
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let mut events = fixture.bus.subscribe();
    let key = channel("dm-privacy");

    let outcome = fixture
        .assistant
        .ingest(with_command(
            inbound(&key, ChannelKind::Direct, "42", "/privacy"),
            "/privacy",
        ))
        .await
        .expect("the command ingests");
    let IngestOutcome::Recorded { receipt, deliver } = outcome else {
        panic!("the command is recorded, not refused");
    };
    assert_eq!(
        deliver,
        Some(DeliveryItem::CommandAnswer(
            "Privacy policy: https://example.org/privacy".into()
        )),
        "the fixed line carries the configured address, typed as the command's answer"
    );

    // Recorded with the command stamp; no turn, no unlatch.
    let blocks = await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the recorded command",
        |blocks| blocks.iter().any(|b| b.block_type == CHAT_MESSAGE_KIND),
    )
    .await;
    let command = blocks
        .iter()
        .find(|b| b.block_type == CHAT_MESSAGE_KIND)
        .expect("the command block exists");
    assert_eq!(command.fields["limited"], json!("command"));
    assert_eq!(command.fields["answer_due"], json!(false));
    assert_eq!(
        command.fields["addressed"],
        json!(true),
        "the stored addressed column keeps the adapter's resolution"
    );
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        0,
        "a legal pointer costs no model turn"
    );
    // The latch untouched, pinned on the bus: the command's ingestion has
    // returned, so any unlatch intent it emitted would already be here.
    assert_eq!(
        drain_unlatches(&mut events),
        0,
        "a command-stamped message emits no unlatch intent"
    );

    // The conversation behaves normally afterwards: the next addressed
    // message unlatches and is answered.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "a real question"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert!(reply.text.contains("a real question"));
    assert_eq!(fixture.script.turns.load(Ordering::SeqCst), 1);
    assert_eq!(
        drain_unlatches(&mut events),
        1,
        "the addressed follow-up is the one re-engagement"
    );
}

/// Unconfigured, the command answers the not-yet-published line — and a
/// foreign-suffix form is not the command at all: recorded like any
/// message, with nothing to deliver.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unconfigured_address_answers_the_not_published_line_and_a_foreign_suffix_does_not() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-privacy");
    authorize(&fixture.assistant, &key).await;

    let outcome = fixture
        .assistant
        .ingest(with_command(
            inbound_unaddressed(&key, ChannelKind::Group, "42", "/privacy"),
            "/privacy",
        ))
        .await
        .expect("the command ingests");
    let IngestOutcome::Recorded { deliver, .. } = outcome else {
        panic!("the command is recorded");
    };
    assert_eq!(
        deliver,
        Some(DeliveryItem::CommandAnswer(PRIVACY_UNPUBLISHED.to_owned())),
        "the unaddressed group form answers, from the fixed constant"
    );

    // The foreign-suffix form: the adapter reports no command for it, the
    // text lands verbatim, and the core matches the report — never the
    // text.
    let foreign = fixture
        .assistant
        .ingest(inbound_unaddressed(
            &key,
            ChannelKind::Group,
            "42",
            "/privacy@another_bot",
        ))
        .await
        .expect("the foreign-suffix message ingests");
    let IngestOutcome::Recorded { receipt, deliver } = foreign else {
        panic!("the foreign-suffix message is recorded");
    };
    assert_eq!(
        deliver, None,
        "a command aimed at someone else is not answered"
    );
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let stored = blocks
        .iter()
        .find(|b| b.fields.get("text") == Some(&json!("/privacy@another_bot")))
        .expect("the foreign form is recorded as an ordinary message");
    assert!(
        stored.fields.get("limited").is_none(),
        "the foreign form takes no command stamp"
    );
}

/// A pending tail debt is preserved past the command: the command's stamp
/// propagates the owed answer instead of cancelling it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pending_tail_debt_is_preserved_past_the_privacy_command() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    .expect("the assembly starts");
    let key = channel("group-debt-past-command");
    authorize(&assistant, &key).await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Group, "A", "the owed ask"),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        with_command(
            inbound_unaddressed(&key, ChannelKind::Group, "B", "/privacy"),
            "/privacy",
        ),
    )
    .await;

    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let command = blocks
        .iter()
        .find(|b| b.fields.get("text") == Some(&json!("/privacy")))
        .expect("the command is recorded");
    assert_eq!(command.fields["limited"], json!("command"));
    assert_eq!(
        command.fields["answer_due"],
        json!(true),
        "the pending tail debt propagates past the command"
    );
}

/// An exhausted answer window yields recorded silence: the command is
/// stored with its stamp and nothing is delivered — the notice discipline,
/// not a protection bypass.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_exhausted_answer_window_records_the_command_and_answers_with_silence() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture =
        support::start_assistant_configured(store, None, support::budgets(None, Some((1, 600))))
            .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("group-exhausted");
    authorize(&fixture.assistant, &key).await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "A", "the window's one answer"),
    )
    .await;
    recv_reply(&mut replies).await;

    let outcome = fixture
        .assistant
        .ingest(with_command(
            inbound_unaddressed(&key, ChannelKind::Group, "B", "/privacy"),
            "/privacy",
        ))
        .await
        .expect("the command ingests");
    let IngestOutcome::Recorded { deliver, .. } = outcome else {
        panic!("the command is recorded even in silence");
    };
    assert_eq!(deliver, None, "the exhausted window answers with silence");

    let blocks = await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the silently recorded command",
        |blocks| {
            blocks
                .iter()
                .any(|b| b.fields.get("text") == Some(&json!("/privacy")))
        },
    )
    .await;
    let command = blocks
        .iter()
        .find(|b| b.fields.get("text") == Some(&json!("/privacy")))
        .expect("the command block exists");
    assert_eq!(command.fields["limited"], json!("command"));
}

/// The command's reply is bounded even in a quiet channel (refined
/// 2026-08-23): the command stamp keeps `/privacy` out of both budget
/// counts, so the exhausted-window silence rule alone never bounded it —
/// the deterministic reply shares the acknowledgment-window mechanism
/// instead, at most one answer per channel per window, every further
/// repeat recorded in silence.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_privacy_commands_in_a_quiet_channel_are_bounded() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-command-repeats");
    authorize(&fixture.assistant, &key).await;

    let mut answered = 0;
    for _ in 0..5 {
        let outcome = fixture
            .assistant
            .ingest(with_command(
                inbound_unaddressed(&key, ChannelKind::Group, "B", "/privacy"),
                "/privacy",
            ))
            .await
            .expect("the command ingests");
        if matches!(
            outcome,
            IngestOutcome::Recorded {
                deliver: Some(_),
                ..
            }
        ) {
            answered += 1;
        }
    }
    assert_eq!(
        answered, 1,
        "one answer per channel per window; the repeats are recorded silence"
    );
}

/// Both windows have an expiry side, pinned under paused time: past the
/// window a fresh rules delta is acknowledged again, and the privacy
/// command is answered again — the bound is a window, not a one-time
/// grant.
#[tokio::test(start_paused = true)]
async fn the_acknowledgment_and_the_command_answer_return_past_the_window() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-window-expiry");
    authorize(&fixture.assistant, &key).await;

    let rules = |text: &str| observed(&key, ObservedFact::PinnedAnnouncement(text.to_owned()));
    let first = fixture
        .assistant
        .observe(rules("Rules:\nThe first wording."))
        .await
        .expect("the first rules pin is judged");
    assert!(
        matches!(
            first,
            ObserveOutcome::Observed {
                deliver: Some(DeliveryItem::Acknowledgment(_))
            }
        ),
        "the fresh window acknowledges"
    );
    let within = fixture
        .assistant
        .observe(rules("Rules:\nThe second wording."))
        .await
        .expect("the second rules pin is judged");
    assert_eq!(
        within,
        ObserveOutcome::Observed { deliver: None },
        "a delta within the window appends silently"
    );
    tokio::time::advance(ACKNOWLEDGMENT_WINDOW + std::time::Duration::from_secs(1)).await;
    let past = fixture
        .assistant
        .observe(rules("Rules:\nThe third wording."))
        .await
        .expect("the post-window rules pin is judged");
    assert!(
        matches!(
            past,
            ObserveOutcome::Observed {
                deliver: Some(DeliveryItem::Acknowledgment(_))
            }
        ),
        "the expired window acknowledges again"
    );

    let command = || {
        with_command(
            inbound_unaddressed(&key, ChannelKind::Group, "B", "/privacy"),
            "/privacy",
        )
    };
    let answered = |outcome: &IngestOutcome| {
        matches!(
            outcome,
            IngestOutcome::Recorded {
                deliver: Some(DeliveryItem::CommandAnswer(_)),
                ..
            }
        )
    };
    let first = fixture
        .assistant
        .ingest(command())
        .await
        .expect("the first command ingests");
    assert!(answered(&first), "the fresh window answers the command");
    let within = fixture
        .assistant
        .ingest(command())
        .await
        .expect("the repeated command ingests");
    assert!(
        !answered(&within),
        "the repeat within the window is silence"
    );
    tokio::time::advance(ACKNOWLEDGMENT_WINDOW + std::time::Duration::from_secs(1)).await;
    let past = fixture
        .assistant
        .ingest(command())
        .await
        .expect("the post-window command ingests");
    assert!(answered(&past), "the expired window answers again");
}

/// A transient append failure spends no command-answer window: the window
/// is consulted only after the append stands, so the redelivered command
/// is answered instead of silenced — with the grant spent before the
/// append, the failed attempt would eat it and the redelivery would draw
/// recorded silence. Budgets are disabled so the injected append fault is
/// the first store write on the command's answer path.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_transient_append_failure_does_not_spend_the_command_answer_window() {
    let (provider, script) = support::scripted_provider(None);
    let fixture = support::start_assistant_operators(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        support::production_toolset(),
        support::budgets(None, None),
        support::operator_config(),
        Some("https://example.org/privacy".into()),
    )
    .await;
    let key = channel("dm-command-redelivery");
    let command = || {
        with_command(
            inbound(&key, ChannelKind::Direct, "42", "/privacy"),
            "/privacy",
        )
    };

    support::sabotage_appends(&fixture.store, assistant_core::kind::CHAT_MESSAGE_TABLE).await;
    let failed = fixture
        .assistant
        .ingest(command())
        .await
        .expect_err("the sabotaged append fails the ingest");
    assert_eq!(
        failed.failure_kind(),
        FailureKind::Transient,
        "the failed append is the typed transient refusal the driver redelivers on"
    );

    support::heal_appends(&fixture.store, assistant_core::kind::CHAT_MESSAGE_TABLE).await;
    let outcome = fixture
        .assistant
        .ingest(command())
        .await
        .expect("the redelivered command ingests");
    let IngestOutcome::Recorded { deliver, .. } = outcome else {
        panic!("the redelivered command is recorded");
    };
    assert_eq!(
        deliver,
        Some(DeliveryItem::CommandAnswer(
            "Privacy policy: https://example.org/privacy".into()
        )),
        "the failed attempt spent no window; the redelivery is answered, not silenced"
    );
}

/// A transient note-append failure spends neither the topic's cap slot nor
/// the acknowledgment: both are recorded only after the append stands.
/// With the slot spent before the append, a whole cap's worth of failed
/// attempts would cap the topic, and the healed redelivery would land
/// nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_transient_note_append_failure_spends_neither_cap_nor_acknowledgment() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "group-note-redelivery").await;
    let pin = || {
        observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\nBe kind.".into()),
        )
    };

    support::sabotage_appends(&fixture.store, assistant_core::note::CONTEXT_NOTE_TABLE).await;
    for _ in 0..NOTE_TOPIC_APPEND_CAP {
        let failed = fixture
            .assistant
            .observe(pin())
            .await
            .expect_err("the sabotaged append fails the observation");
        assert_eq!(
            failed.failure_kind(),
            FailureKind::Transient,
            "the failed append is the typed transient refusal the driver redelivers on"
        );
    }

    support::heal_appends(&fixture.store, assistant_core::note::CONTEXT_NOTE_TABLE).await;
    let outcome = fixture
        .assistant
        .observe(pin())
        .await
        .expect("the redelivered observation is judged");
    assert_eq!(
        outcome,
        ObserveOutcome::Observed {
            deliver: Some(DeliveryItem::Acknowledgment(
                RULES_ACKNOWLEDGMENT.to_owned()
            ))
        },
        "the failed attempts spent no cap slot and no acknowledgment"
    );
    let conversation = only_conversation(&fixture.store).await;
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(stored.len(), 1, "exactly the redelivered note landed");
    assert_eq!(stored[0].fields["text"], json!("Be kind."));
}

/// A pin-toggle burst appends at most the cap (refined 2026-08-23): one
/// window admits [`NOTE_TOPIC_APPEND_CAP`] notes of one topic, the rest of
/// the burst appends nothing — and the capped delta is not lost, it lands
/// on the next observation after the window through the on-delta rule.
#[tokio::test(start_paused = true)]
async fn a_pin_toggle_burst_appends_at_most_the_cap_and_the_delta_lands_after_the_window() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-pin-toggle");
    authorize(&fixture.assistant, &key).await;

    for toggle in 0..6 {
        let text = if toggle % 2 == 0 {
            "Rules:\nThe first pin."
        } else {
            "Rules:\nThe second pin."
        };
        fixture
            .assistant
            .observe(observed(
                &key,
                ObservedFact::PinnedAnnouncement(text.into()),
            ))
            .await
            .expect("the toggled pin is judged");
    }
    let conversation = only_conversation(&fixture.store).await;
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(
        stored.len(),
        NOTE_TOPIC_APPEND_CAP as usize,
        "the burst appended exactly the cap"
    );

    tokio::time::advance(ACKNOWLEDGMENT_WINDOW + std::time::Duration::from_secs(1)).await;
    fixture
        .assistant
        .observe(observed(
            &key,
            ObservedFact::PinnedAnnouncement("Rules:\nThe standing pin.".into()),
        ))
        .await
        .expect("the post-window pin is judged");
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(
        stored.len(),
        NOTE_TOPIC_APPEND_CAP as usize + 1,
        "the still-standing delta lands on the next observation after the window"
    );
    assert_eq!(
        stored.last().expect("a newest note").fields["text"],
        json!("The standing pin.")
    );
}

/// The failed-turn tail keeps its pre-unit meaning (refined 2026-08-23):
/// the consumer's transparent walk reads through context notes exactly, so
/// the turn-closure marker a failed turn writes over the owed message —
/// frontier-transparent to the FRAMEWORK's walk, which governs turn
/// liveness — stays a settled tail at the consumer stamp, and an
/// unaddressed message behind it propagates no debt. The marker is staged
/// exactly as the framework's error close edge writes it, like the AC4
/// staging above.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_turns_closure_marker_does_not_widen_debt_propagation() {
    // A silent provider: no answer ever lands, so the staged marker stays
    // the provable tail.
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    .expect("the assembly starts");
    let key = channel("dm-failed-turn");
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Direct, "42", "the failing ask"),
    )
    .await;
    store
        .insert_status_block(receipt.conversation_id, "turn_ended:errored".into(), None)
        .await
        .expect("the turn-closure marker appends");

    // Non-vacuity: the marker really is the stored tail the stamp reads
    // behind.
    let tail = store
        .latest_block(receipt.conversation_id)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(tail.block_type, "status");
    assert_eq!(tail.fields["status"], json!("turn_ended:errored"));

    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(
            &key,
            ChannelKind::Direct,
            "42",
            "an aside behind the failed turn",
        ),
    )
    .await;
    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks
        .iter()
        .find(|block| block.fields.get("text") == Some(&json!("an aside behind the failed turn")))
        .expect("the aside is recorded");
    assert_eq!(
        aside.fields["answer_due"],
        json!(false),
        "no debt propagates through a closed turn's marker"
    );
}

/// The other half of the failed-turn pin: with a NOTE on top of the
/// closure marker, the owing-tail walk past the note must stop at the
/// marker — a settled tail — never read on through it. This pins the
/// walk's exclusion scope exactly: widened to also read through the
/// framework's turn-closure kind, the walk would find the owed message
/// behind the failed turn and stamp the aside answer-due — the propagation
/// past failed turns the refinement scoped out.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_note_over_a_failed_turns_marker_stays_a_settled_tail() {
    // A silent provider: no answer ever lands, so the staged shape stays
    // the provable tail.
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Assistant::start(
        store.clone(),
        Arc::clone(&bus),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    .expect("the assembly starts");
    let key = channel("dm-noted-failed-turn");
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Direct, "42", "the failing ask"),
    )
    .await;
    store
        .insert_status_block(receipt.conversation_id, "turn_ended:errored".into(), None)
        .await
        .expect("the turn-closure marker appends");
    store
        .append_consumer_block(
            receipt.conversation_id,
            None,
            CONTEXT_NOTE_KIND,
            ContextNote::stored_fields(NoteTopic::Rules, "Be kind."),
            None,
        )
        .await
        .expect("the note appends on top of the marker");

    // Non-vacuity: the note is the tail and the closure marker stands
    // directly behind it — exactly the shape whose walk is under test.
    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let tail = blocks.last().expect("the ledger is non-empty");
    assert_eq!(tail.block_type, CONTEXT_NOTE_KIND);
    let behind = &blocks[blocks.len() - 2];
    assert_eq!(behind.block_type, "status");
    assert_eq!(behind.fields["status"], json!("turn_ended:errored"));

    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(
            &key,
            ChannelKind::Direct,
            "42",
            "an aside behind the noted turn",
        ),
    )
    .await;
    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks
        .iter()
        .find(|block| block.fields.get("text") == Some(&json!("an aside behind the noted turn")))
        .expect("the aside is recorded");
    assert_eq!(
        aside.fields["answer_due"],
        json!(false),
        "the walk past the note stops at the closed turn's marker; no debt reads through it"
    );
}

// ─── AC7: the observation path's locks and creation ──────────────────────

/// Two racing equal observations append one note: the stamp lock holds the
/// on-delta read-then-append. The race is made observable through the
/// scripted pause inside the read-then-append window: both racers try to
/// cross a two-party barrier there, which only succeeds when both sit
/// inside the window AT ONCE — exactly what the stamp lock forbids. Under
/// the lock the second racer is still waiting on it, the first times out
/// and appends, and the second reads the appended note and appends
/// nothing; with the lock removed, both would cross the barrier, both
/// would append, and the count below would fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_racing_equal_observations_append_one_note() {
    let mut fixture = support::start_assistant(None).await;
    let key = channel("group-race");
    let both_inside_the_window = Arc::new(tokio::sync::Barrier::new(2));
    let barrier = Arc::clone(&both_inside_the_window);
    fixture
        .assistant
        .pause_between_note_read_and_append(Arc::new(move || {
            let barrier = Arc::clone(&barrier);
            Box::pin(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_millis(250), barrier.wait())
                    .await;
            })
        }));
    authorize(&fixture.assistant, &key).await;

    let assistant = Arc::new(fixture.assistant);
    let racers: Vec<_> = (0..2)
        .map(|_| {
            let assistant = Arc::clone(&assistant);
            let key = key.clone();
            tokio::spawn(async move {
                assistant
                    .observe(observed(&key, ObservedFact::Title("The same title".into())))
                    .await
                    .expect("the racing observation is judged")
            })
        })
        .collect();
    for racer in racers {
        let outcome = racer.await.expect("the racer finishes");
        assert_eq!(outcome, ObserveOutcome::Observed { deliver: None });
    }

    let conversation = only_conversation(&fixture.store).await;
    let stored = notes(&fixture.store, conversation).await;
    assert_eq!(
        stored.len(),
        1,
        "the equal racers appended exactly one note"
    );
}

/// An observation-created conversation carries the system prompt and the
/// palette: a group's facts exist on the ledger before anyone speaks, on
/// the same winner-only creation path a first message takes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observation_created_conversation_carries_the_prompt_and_the_palette() {
    let fixture = support::start_assistant(None).await;
    let key = channel("group-created-by-observation");
    authorize(&fixture.assistant, &key).await;
    fixture
        .assistant
        .observe(observed(&key, ObservedFact::Title("A titled group".into())))
        .await
        .expect("the title observation is judged");

    let conversation = only_conversation(&fixture.store).await;
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let shape: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
    assert_eq!(
        shape,
        vec!["system_prompt", "tool_palette", CONTEXT_NOTE_KIND],
        "prompt and palette first, the note behind them"
    );
    assert_eq!(blocks[0].fields["content"], json!(support::SYSTEM_PROMPT));
}

/// An observation racing an erasure respects the fence: while the erasure
/// holds it — provably, between its interrupt going out and its loud settle
/// failure — the observation stays pending, and it completes only after the
/// erasure returned.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_observation_racing_an_erasure_respects_the_fence() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let assistant = Arc::new(
        Assistant::start(
            store.clone(),
            Arc::clone(&bus),
            support::registry_of(deaf_provider()),
            assistant_core::tools::ToolSet::new(),
            assistant_core::AssemblyConfig {
                binding: support::binding(),
                system_prompt: support::SYSTEM_PROMPT.into(),
                protection: assistant_core::ProtectionConfig::default(),
                operators: support::operator_config(),
                direct_chats: assistant_core::DirectChats::default(),
                privacy_policy_address: None,
                moderation_handle: None,
            },
        )
        .await
        .expect("the assembly starts"),
    );
    let group = channel("group-during-erasure");
    authorize(&assistant, &group).await;

    // A direct stream held open by a provider deaf to the interrupt: the
    // erasure will hold the fence for its whole settle bound.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(
            &channel("dm-erased"),
            ChannelKind::Direct,
            "A",
            "the unsettled ask",
        ),
    )
    .await;
    await_ledger(
        &store,
        receipt.conversation_id,
        "the streaming tail",
        |blocks| blocks.iter().any(|b| b.block_type.starts_with("streaming")),
    )
    .await;

    let mut events = bus.subscribe();
    let erasure_done = Arc::new(AtomicBool::new(false));
    let erasure = tokio::spawn({
        let assistant = Arc::clone(&assistant);
        let erasure_done = Arc::clone(&erasure_done);
        async move {
            let failure = assistant
                .erase_principal(receipt.principal_id)
                .await
                .expect_err("the deaf stream fails the erasure at the bound");
            erasure_done.store(true, Ordering::SeqCst);
            failure
        }
    });

    // The interrupt on the bus proves the erasure holds the fence now.
    loop {
        if let CoreEvent::InterruptRequested { .. } =
            tokio::time::timeout(support::DEADLINE, events.recv())
                .await
                .expect("the interrupt arrives before the deadline")
                .expect("the bus outlives the test")
        {
            break;
        }
    }

    let mut observation = Box::pin(assistant.observe(observed(
        &group,
        ObservedFact::Title("observed mid-erasure".into()),
    )));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(200), &mut observation)
            .await
            .is_err(),
        "the observation waits on the fence while the erasure holds it"
    );

    let failure = erasure.await.expect("the erasure task finishes");
    assert!(
        matches!(failure, CoreError::ErasureUnsettled { .. }),
        "the erasure failed loudly at the bound; got {failure:?}"
    );
    let outcome = observation.await.expect("the observation completes");
    assert!(
        erasure_done.load(Ordering::SeqCst),
        "the observation finished only after the erasure released the fence"
    );
    assert_eq!(outcome, ObserveOutcome::Observed { deliver: None });
}

/// A provider that opens a stream, writes a tail, and then holds the stream
/// open forever, deaf to the interrupt — what keeps the erasure on the
/// fence for its whole settle bound.
fn deaf_provider() -> Box<dyn agent_ledger::ProviderModule> {
    support::provider_stub("Deaf", "opens a stream and never lets go", || {
        let (request_tx, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let (response_tx, responses) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let Some(_first) = requests.recv().await else {
                return;
            };
            let _ = response_tx.send(agent_ledger::ProviderResponse::Event(
                StreamEvent::Connected,
            ));
            let _ = response_tx.send(agent_ledger::ProviderResponse::Event(
                StreamEvent::TextBlockStart,
            ));
            let _ = response_tx.send(agent_ledger::ProviderResponse::Event(
                StreamEvent::TextDelta {
                    text: "a tail that never ends".into(),
                },
            ));
            std::future::pending::<()>().await;
        });
        (request_tx, responses)
    })
}
