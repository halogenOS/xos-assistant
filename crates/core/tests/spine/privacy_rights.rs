//! The privacy self-service unit at the core's edges (AC1–AC5): the
//! suppression drop with its full no-write claim, the deterministically
//! interleaved suppression race, the frozen exempt commands, the
//! principal-keyed deletion with its spawned erasure — its confirms
//! bounded so the inline-deadlock shape fails by name — the privacy tool
//! over the scripted model, the per-person reply bound beside the
//! budgets, the stub-keeping erasure, and the appended-step upgrade.

use agent_ledger::Store;
use agent_ledger::store::domain_run;
use assistant_core::schema::{DOMAIN, store_config};
use assistant_core::tools::ToolSet;
use assistant_core::tools::rights as privacy_tool;
use assistant_core::{
    Assistant, ChannelKey, ChannelKind, DeliveryItem, ErasureOutcome, IngestOutcome,
    PRIVACY_REPLY_CAP, ProtectionConfig, privacy,
};
use serde_json::json;

use crate::support::{
    self, ToolScript, channel, field, inbound, inbound_unaddressed, recv_reply, settle_shape,
    tool_scripted_provider, with_command, with_username,
};

/// The tables the no-write claim reads raw, one by one: the identity
/// table, the message content table, the channel mapping, the framework's
/// conversation and block tables with their junction, and the palette
/// content table. A suppressed message may grow none of them.
const NO_WRITE_TABLES: [&str; 7] = [
    "principals",
    "block_chat_message",
    "channels",
    "conversations",
    "blocks",
    "conversation_blocks",
    "block_tool_palette",
];

/// One raw row count, read through the domain seam.
async fn table_count(store: &Store, table: &'static str) -> i64 {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        Ok(
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })?,
        )
    })
    .await
    .expect("the raw count reads")
}

/// Every no-write table's raw count, in declaration order.
async fn no_write_snapshot(store: &Store) -> Vec<i64> {
    let mut counts = Vec::with_capacity(NO_WRITE_TABLES.len());
    for table in NO_WRITE_TABLES {
        counts.push(table_count(store, table).await);
    }
    counts
}

/// One principal's raw identity row on the suite's adapter: id, username,
/// and the suppression flag — `None` when no row stands.
async fn principal_row(store: &Store, external_id: &str) -> Option<(i64, Option<String>, i64)> {
    let external = external_id.to_owned();
    domain_run(&store.tx(), DOMAIN, move |conn| {
        let mut statement = conn.prepare(
            "SELECT id, username, opted_out FROM principals
             WHERE adapter = ?1 AND external_id = ?2",
        )?;
        let rows = statement
            .query_map([support::ADAPTER, external.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().next())
    })
    .await
    .expect("the principals table reads")
}

/// The stored texts of one principal's message rows, raw — what the
/// erasure pins read to prove the nulling.
async fn stored_texts(store: &Store, principal_id: i64) -> Vec<Option<String>> {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        let mut statement = conn.prepare(
            "SELECT text FROM block_chat_message WHERE principal_id = ?1 ORDER BY block_id",
        )?;
        let rows = statement
            .query_map([principal_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the message table reads")
}

/// The stored speaker handles of one principal's message rows, raw — what
/// the handle-freeze pins read: a suppressed sender's exempt command
/// records no speaker, so after a deletion no command re-materializes the
/// handle the erasure emptied.
async fn stored_speakers(store: &Store, principal_id: i64) -> Vec<Option<String>> {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        let mut statement = conn.prepare(
            "SELECT speaker FROM block_chat_message WHERE principal_id = ?1 ORDER BY block_id",
        )?;
        let rows = statement
            .query_map([principal_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the message table reads")
}

/// Poll the raw identity table until `accept` says the person's row has
/// the awaited shape — the erasure runs as a spawned task, so its outcome
/// is awaited, never assumed.
async fn await_principal_shape(
    store: &Store,
    external_id: &str,
    what: &str,
    accept: impl Fn(&Option<(i64, Option<String>, i64)>) -> bool,
) {
    let deadline = std::time::Instant::now() + support::DEADLINE;
    loop {
        let row = principal_row(store, external_id).await;
        if accept(&row) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {what}; the row reads {row:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Poll one principal's stored texts until every row reads nulled — the
/// positive observable a spawned erasure's nulling pass leaves, awaited
/// with the suite's bounded-poll shape so a delayed spawn is tolerated
/// while a never-running or wedged erasure fails by name.
async fn await_nulled_texts(store: &Store, principal_id: i64, what: &str) {
    let deadline = std::time::Instant::now() + support::DEADLINE;
    loop {
        let texts = stored_texts(store, principal_id).await;
        if !texts.is_empty() && texts.iter().all(Option::is_none) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting {what}; the texts read {texts:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Ingest one unaddressed group command and return its delivered answer
/// text, `None` for recorded silence. Panics on a refused outcome — the
/// family is recorded even under suppression.
async fn command_reply(
    assistant: &Assistant,
    key: &ChannelKey,
    sender: &str,
    token: &str,
) -> Option<String> {
    let outcome = assistant
        .ingest(with_command(
            inbound_unaddressed(key, ChannelKind::Group, sender, token),
            token,
        ))
        .await
        .expect("the command ingests");
    match outcome {
        IngestOutcome::Recorded { deliver, .. } => deliver.map(|item| item.text().to_owned()),
        refused => panic!("the command is recorded, never refused: {refused:?}"),
    }
}

/// One command's reply under a bounded await — the suite's timeout-helper
/// shape on the command path: the ingestion answering a command holds the
/// erasure fence for reading, while an erasure takes the same fence for
/// writing, so an erasure run inline from ingestion would deadlock right
/// here. The bound turns that shape into a named failure in seconds
/// instead of a hung suite. Multi-thread tests only: under paused time
/// the bound's timer would race the store's external thread.
async fn bounded_command_reply(
    assistant: &Assistant,
    key: &ChannelKey,
    sender: &str,
    token: &str,
) -> Option<String> {
    tokio::time::timeout(
        support::DEADLINE,
        command_reply(assistant, key, sender, token),
    )
    .await
    .expect(
        "the command's ingestion returns before the deadline — an erasure run \
             inline would deadlock on the fence this ingestion holds",
    )
}

/// Ingest one message that must be dropped by the standing flag.
async fn ingest_dropped(assistant: &Assistant, message: assistant_core::InboundMessage) {
    let outcome = assistant.ingest(message).await.expect("the drop is judged");
    assert_eq!(
        outcome,
        IngestOutcome::Disregarded,
        "the suppressed message is dropped without effect"
    );
}

/// Install a fault that aborts every UPDATE on the named table — the seam
/// behind the failed flag-write pin: the tool's identity update dies,
/// nothing is recorded, and the transient result answers. Never healed:
/// the one test that installs it holds its own store for its whole life.
async fn sabotage_updates(store: &Store, table: &'static str) {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        conn.execute_batch(&format!(
            "CREATE TRIGGER sabotage_update_{table} BEFORE UPDATE ON {table} \
             BEGIN SELECT RAISE(ABORT, 'injected update failure'); END;"
        ))?;
        Ok(())
    })
    .await
    .expect("the sabotage trigger installs");
}

/// Install a fault that aborts every DELETE on the named table — the seam
/// behind the failed-erasure pin, aimed at the erasure's CONCLUSION on
/// purpose: the earlier passes run and leave their positive observable
/// (the nulled texts the pin awaits), then the conclusion dies and the
/// identity row stands. A fault on the nulling pass itself would leave a
/// failed run indistinguishable from one that never started, and the pin
/// would be reduced to sleeping at a race. [`heal_deletes`] removes it.
async fn sabotage_deletes(store: &Store, table: &'static str) {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        conn.execute_batch(&format!(
            "CREATE TRIGGER sabotage_delete_{table} BEFORE DELETE ON {table} \
             BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END;"
        ))?;
        Ok(())
    })
    .await
    .expect("the sabotage trigger installs");
}

/// Remove the injected delete fault; the next erasure concludes again.
async fn heal_deletes(store: &Store, table: &'static str) {
    domain_run(&store.tx(), DOMAIN, move |conn| {
        conn.execute_batch(&format!("DROP TRIGGER sabotage_delete_{table};"))?;
        Ok(())
    })
    .await
    .expect("the sabotage trigger drops");
}

// ─── AC2: suppression, the full no-write claim ───────────────────────────

/// The drop, table by table: after `/privacyout`, the person's group
/// message — even one carrying a refreshed username — leaves no row in
/// any table, draws no answer and no turn, and a first message on a fresh
/// authorized channel creates nothing either; a bystander's message still
/// records and answers, so the pipeline provably stayed alive around the
/// drop.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_standing_flag_drops_the_persons_messages_with_no_write() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-suppression").await;
    // The fresh channel is authorized ahead of the snapshot: authorization
    // rows are the operator's writes, not the suppressed person's.
    let fresh = support::authorized_group(&fixture.assistant, "room-suppression-fresh").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "A", "a line before the opt-out"),
    )
    .await;
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        Some(privacy::OPT_OUT_DONE.to_owned()),
        "the opt-out answers its fixed line"
    );
    let (_, username, flag) = principal_row(&fixture.store, "A")
        .await
        .expect("the person's row stands");
    assert_eq!(flag, 1, "the flag stands from the answered opt-out on");

    let before = no_write_snapshot(&fixture.store).await;
    // The dropped message carries a changed username on purpose: a
    // refresh would be visible even where the counts stay equal.
    let mut dropped = inbound(
        &room,
        ChannelKind::Group,
        "A",
        "an addressed ask after opting out",
    );
    dropped.sender.username = Some("renamed".into());
    ingest_dropped(&fixture.assistant, dropped).await;
    // The fresh-channel first message: no mapping, no conversation, no
    // palette, nothing.
    ingest_dropped(
        &fixture.assistant,
        inbound(
            &fresh,
            ChannelKind::Group,
            "A",
            "first contact on a fresh channel",
        ),
    )
    .await;
    assert_eq!(
        no_write_snapshot(&fixture.store).await,
        before,
        "no table grew under the drop — the full no-write claim, read raw"
    );
    let (_, username_after, _) = principal_row(&fixture.store, "A")
        .await
        .expect("the row still stands");
    assert_eq!(
        username_after, username,
        "no identity refresh rode the drop"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "no turn fired for a dropped message"
    );

    // The pipeline around the drop: a bystander's addressed message still
    // records and answers — and this is the only answer the edge carries.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "B", "a bystander's question"),
    )
    .await;
    // The scripted answer derives from the projected run, so the reply
    // proves two facts at once: the bystander's line reached the model
    // newest, and neither dropped text ever entered the projection.
    let reply = recv_reply(&mut replies).await.text;
    assert!(
        reply.ends_with("a bystander's question"),
        "the bystander is answered: {reply}"
    );
    for dropped_text in [
        "an addressed ask after opting out",
        "first contact on a fresh channel",
    ] {
        assert!(
            !reply.contains(dropped_text),
            "the dropped text {dropped_text:?} never reached the model"
        );
    }
}

/// The suppression race, deterministically interleaved through the
/// scripted-pause seam (2026-08-23): an ordinary message on the
/// fresh-channel long path reads a clean standing OUTSIDE the stamp lock
/// and pauses; the person's `/privacyout` then runs whole — its flag
/// write serialized under the stamp lock — before the racer resumes. The
/// under-lock re-read must drop the racer with no message row: the flag
/// suppresses from the moment it stands. Removing the re-read makes
/// exactly this test store the racing row.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_flag_landing_between_the_pre_lock_read_and_the_append_drops_the_racer() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let mut fixture = support::start_assistant(None).await;
    let (reached_tx, mut reached_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let resume = Arc::new(tokio::sync::Semaphore::new(0));
    let armed = Arc::new(AtomicBool::new(false));
    {
        let resume = Arc::clone(&resume);
        let armed = Arc::clone(&armed);
        fixture
            .assistant
            .pause_between_standing_read_and_append(Arc::new(move || {
                let reached_tx = reached_tx.clone();
                let resume = Arc::clone(&resume);
                let armed = Arc::clone(&armed);
                Box::pin(async move {
                    // Only the armed racer pauses; every other ingestion —
                    // the opt-out command above all — passes through.
                    if !armed.swap(false, Ordering::SeqCst) {
                        return;
                    }
                    let _ = reached_tx.send(());
                    tokio::time::timeout(support::DEADLINE, resume.acquire())
                        .await
                        .expect("the racer is resumed before the deadline")
                        .expect("the semaphore outlives the test")
                        .forget();
                })
            }));
    }
    let room = support::authorized_group(&fixture.assistant, "room-suppression-race").await;
    let assistant = Arc::new(fixture.assistant);

    armed.store(true, Ordering::SeqCst);
    let racer = {
        let assistant = Arc::clone(&assistant);
        let room = room.clone();
        tokio::spawn(async move {
            assistant
                .ingest(inbound(
                    &room,
                    ChannelKind::Group,
                    "A",
                    "the racing message",
                ))
                .await
                .expect("the racing ingestion is judged")
        })
    };
    tokio::time::timeout(support::DEADLINE, reached_rx.recv())
        .await
        .expect("the racer reaches the seam before the deadline")
        .expect("the seam outlives the test");
    // The peer ingestion runs whole while the racer sits between its
    // pre-lock read and its append: this is the probed interleaving.
    assert_eq!(
        command_reply(&assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        Some(privacy::OPT_OUT_DONE.to_owned()),
        "the flag lands while the racer is paused"
    );
    resume.add_permits(1);
    assert_eq!(
        racer.await.expect("the racer finishes"),
        IngestOutcome::Disregarded,
        "the under-lock re-read drops the racing message"
    );
    let (principal_id, ..) = principal_row(&fixture.store, "A")
        .await
        .expect("the person's row stands");
    assert_eq!(
        stored_texts(&fixture.store, principal_id).await,
        vec![Some(privacy::OPT_OUT_COMMAND.to_owned())],
        "the ledger holds the exempt command alone — the racing row was never written"
    );
}

/// The exempt commands of an opted-out person: `/privacy` and the repeats
/// keep answering, nothing refreshes the frozen username — even when the
/// command itself carries a new one — and `/unblockprivacy` reopens
/// collection, after which the next message records, answers, and
/// refreshes again.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_opted_out_persons_commands_answer_frozen_and_unblock_reopens() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-frozen").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "A", "hello"),
    )
    .await;
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        Some(privacy::OPT_OUT_DONE.to_owned())
    );
    let (principal_id, frozen_username, _) = principal_row(&fixture.store, "A")
        .await
        .expect("the row stands");

    // The person's own commands, each carrying a changed username: the
    // freeze holds across every one of them.
    for (token, expected) in [
        (
            privacy::PRIVACY_COMMAND,
            assistant_core::PRIVACY_UNPUBLISHED,
        ),
        (privacy::OPT_OUT_COMMAND, privacy::OPT_OUT_ALREADY),
    ] {
        let mut message = with_command(
            inbound_unaddressed(&room, ChannelKind::Group, "A", token),
            token,
        );
        message.sender.username = Some("renamed".into());
        let outcome = fixture
            .assistant
            .ingest(message)
            .await
            .expect("the exempt command ingests");
        let IngestOutcome::Recorded { deliver, .. } = outcome else {
            panic!("the exempt command is recorded");
        };
        assert_eq!(
            deliver,
            Some(DeliveryItem::CommandAnswer(expected.to_owned())),
            "{token} answers its fixed line under suppression"
        );
    }
    let (_, username, _) = principal_row(&fixture.store, "A")
        .await
        .expect("the row stands");
    assert_eq!(username, frozen_username, "the username stays frozen");
    // The freeze covers the delivered handle too: the suppressed person's
    // exempt command rows record no speaker, though each command carried
    // one.
    assert!(
        stored_speakers(&fixture.store, principal_id)
            .await
            .iter()
            .all(Option::is_none),
        "no suppressed command row records a speaker"
    );

    // The door reopens from inside, and only then does recording resume.
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_IN_COMMAND).await,
        Some(privacy::OPT_IN_DONE.to_owned())
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        0,
        "the flag is cleared"
    );
    let mut returned = inbound(&room, ChannelKind::Group, "A", "back again");
    returned.sender.username = Some("renamed".into());
    support::ingest_recorded(&fixture.assistant, returned).await;
    let reply = recv_reply(&mut replies).await.text;
    assert!(
        reply.ends_with("back again"),
        "the next message after the opt-in records and answers: {reply}"
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").1,
        Some("renamed".into()),
        "an unflagged person's message refreshes the username again"
    );
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_IN_COMMAND).await,
        Some(privacy::OPT_IN_ALREADY.to_owned()),
        "the repeat answers the already-so line"
    );
}

// ─── AC3: deletion, confirmed programmatically, erased outside the fence ─

/// The deletion flow across chats: the ask in one group answers the
/// confirm instruction, the confirm in ANOTHER group runs — the pending is
/// keyed by principal, not by room — the spawned erasure completes after
/// the ingestion returned, the unflagged person's row is deleted and their
/// texts nulled, and a second confirm answers the nothing-pending line. A
/// confirm with nothing ever pending answers the same line. The bounded
/// confirm is the no-deadlock pin: an erasure run inline would deadlock
/// on the fence the confirm's own ingestion holds, and the bound names
/// that shape in seconds instead of hanging the suite.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cross_chat_confirm_runs_the_spawned_erasure() {
    let fixture = support::start_assistant(None).await;
    let one = support::authorized_group(&fixture.assistant, "room-delete-1").await;
    let two = support::authorized_group(&fixture.assistant, "room-delete-2").await;

    assert_eq!(
        bounded_command_reply(&fixture.assistant, &one, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::NOTHING_PENDING.to_owned()),
        "a confirm with nothing pending answers the fixed line"
    );
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&one, ChannelKind::Group, "A", "some words to erase"),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&two, ChannelKind::Group, "A", "words in the other room"),
    )
    .await;

    assert_eq!(
        bounded_command_reply(&fixture.assistant, &one, "A", privacy::DELETE_COMMAND).await,
        Some(privacy::CONFIRM_INSTRUCTION.to_owned()),
        "the ask answers the confirm instruction"
    );
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &two, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::DELETION_STARTED.to_owned()),
        "the confirm in the other chat consumes the same pending"
    );
    await_principal_shape(
        &fixture.store,
        "A",
        "the spawned erasure's deletion",
        Option::is_none,
    )
    .await;
    assert!(
        stored_texts(&fixture.store, receipt.principal_id)
            .await
            .iter()
            .all(Option::is_none),
        "every stored text of the person is nulled"
    );

    // The person's next appearance is a fresh principal; nothing pends for
    // it, and the line says so.
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &one, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::NOTHING_PENDING.to_owned()),
        "a second confirm after the completed run answers nothing-pending"
    );
}

/// AC3's post-window line through the command path, under paused time: the
/// filed pending lapses past `CONFIRM_WINDOW`, the late `/confirmdelete`
/// answers the nothing-pending line exactly, and no erasure runs — the row
/// and every stored text stand untouched.
#[tokio::test(start_paused = true)]
async fn a_confirm_past_the_window_answers_nothing_pending_and_erases_nothing() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-lapsed").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "A", "words the lapse protects"),
    )
    .await;
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::DELETE_COMMAND).await,
        Some(privacy::CONFIRM_INSTRUCTION.to_owned()),
        "the ask files the pending"
    );
    tokio::time::advance(privacy::CONFIRM_WINDOW + std::time::Duration::from_secs(1)).await;
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::NOTHING_PENDING.to_owned()),
        "past the window the confirm answers the nothing-pending line"
    );
    assert!(
        principal_row(&fixture.store, "A").await.is_some(),
        "no erasure ran — the identity row stands"
    );
    assert!(
        stored_texts(&fixture.store, receipt.principal_id)
            .await
            .iter()
            .all(Option::is_some),
        "the stored texts stand untouched"
    );
}

/// A restart forgets the pending state: the memory is process-held on
/// purpose, so a confirm in the next process answers nothing-pending and
/// the data stands untouched.
#[test]
fn a_restart_forgets_the_pending_state() {
    let db = support::TempDb::new("pending-restart");

    let first = support::process_runtime();
    first.block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        let fixture = support::start_assistant_on(store, None).await;
        let room = support::authorized_group(&fixture.assistant, "room-restart").await;
        support::ingest_recorded(
            &fixture.assistant,
            inbound_unaddressed(&room, ChannelKind::Group, "A", "words that stay"),
        )
        .await;
        assert_eq!(
            command_reply(&fixture.assistant, &room, "A", privacy::DELETE_COMMAND).await,
            Some(privacy::CONFIRM_INSTRUCTION.to_owned())
        );
    });
    drop(first);

    let second = support::process_runtime();
    second.block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store reopens");
        let fixture = support::start_assistant_on(store, None).await;
        let room = channel("room-restart");
        assert_eq!(
            command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
            Some(privacy::NOTHING_PENDING.to_owned()),
            "the restarted process holds no pending — deletion errs safe"
        );
        assert!(
            principal_row(&fixture.store, "A").await.is_some(),
            "nothing was deleted across the restart"
        );
    });
}

/// A failed erasure: the confirm consumes the pending and spawns the run,
/// the injected fault kills the erasure's conclusion, and the identity
/// row stands — with the run's completion AWAITED, never slept at: the
/// fault sits on the conclusion so the nulling pass leaves its positive
/// observable first, the awaited nulled texts prove the spawned run
/// really ran and died at the fault, and a never-running or wedged
/// erasure times the await out by name. A fresh ask-and-confirm after the
/// fault heals runs to completion, which is the promised re-ask.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_erasure_leaves_the_identity_standing_and_a_re_ask_works() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-erasure-fault").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "A", "words the fault protects"),
    )
    .await;

    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::DELETE_COMMAND).await,
        Some(privacy::CONFIRM_INSTRUCTION.to_owned())
    );
    sabotage_deletes(&fixture.store, "principals").await;
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::DELETION_STARTED.to_owned()),
        "the confirm consumes the pending before the spawned run can fail"
    );
    // The spawned erasure takes the fence after this ingestion released
    // it. Its nulling pass runs first and is the awaited proof the run
    // happened; the conclusion behind it dies on the injected fault.
    await_nulled_texts(
        &fixture.store,
        receipt.principal_id,
        "the failed run's completed nulling pass",
    )
    .await;
    assert!(
        principal_row(&fixture.store, "A").await.is_some(),
        "the fault killed the conclusion — the identity row stands"
    );
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::NOTHING_PENDING.to_owned()),
        "the failed run's pending is consumed; a bare re-confirm re-runs nothing"
    );

    heal_deletes(&fixture.store, "principals").await;
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::DELETE_COMMAND).await,
        Some(privacy::CONFIRM_INSTRUCTION.to_owned()),
        "re-asking works"
    );
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::DELETION_STARTED.to_owned())
    );
    await_principal_shape(
        &fixture.store,
        "A",
        "the healed erasure's deletion",
        Option::is_none,
    )
    .await;
}

// ─── AC4: the tool, end to end over the scripted model ───────────────────

/// One assembled privacy-tool fixture: the tool alone in the palette
/// besides itself — no lookups, so the ledger shapes stay minimal — over
/// the tool-scripted provider calling it with the given action.
async fn privacy_tool_fixture(
    action: &str,
    narration: Option<String>,
    hold: Option<std::sync::Arc<support::TurnHold>>,
) -> support::Fixture {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: privacy_tool::NAME.into(),
            input: json!({ "action": action }).to_string(),
            narration,
        },
        hold,
    );
    support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await
}

/// The plain-language opt-out: the explicit ask makes the scripted model
/// call the tool, the flag stands through the system's own write, the tool
/// result relays the fixed line, and the person's next message is dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_plain_language_opt_out_calls_the_tool_and_the_system_enforces_it() {
    let fixture = privacy_tool_fixture(privacy_tool::ACTION_OPT_OUT, None, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-tool-opt-out").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "please stop collecting my messages",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the tool turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[4], "content"),
        privacy_tool::opt_out_result(),
        "the tool result relays the fixed opt-out line"
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        1,
        "the flag stands through the tool's write"
    );
    ingest_dropped(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "one more message after the ask",
        ),
    )
    .await;
}

/// The plain-language deletion ask: the tool files the pending and its
/// result carries the literal confirm token; the person's own
/// `/confirmdelete` then consumes exactly that pending and the erasure
/// runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_plain_language_deletion_ask_relays_the_confirm_token_and_confirms() {
    let fixture = privacy_tool_fixture(privacy_tool::ACTION_REQUEST_DELETION, None, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-tool-delete").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "please delete my stored data",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the tool turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    let result = field(&blocks[4], "content");
    assert_eq!(result, privacy_tool::request_deletion_result());
    assert!(
        result.contains(privacy::CONFIRM_COMMAND),
        "the pinned fact: the literal confirm token rides in the tool result"
    );
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::DELETION_STARTED.to_owned()),
        "the command consumes the pending the tool filed"
    );
    await_principal_shape(&fixture.store, "A", "the confirmed erasure", |row| {
        row.is_none()
    })
    .await;
}

/// The absorbed co-summoner shape declines: a second person's addressed
/// message lands mid-turn, the origin set holds two distinct principals,
/// and the tool answers the fixed ambiguity result acting on nobody.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_absorbed_co_summoner_declines_the_tool_with_the_ambiguity_result() {
    let hold = support::TurnHold::new();
    let fixture = privacy_tool_fixture(
        privacy_tool::ACTION_OPT_OUT,
        Some("One moment.".into()),
        Some(hold.clone()),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-tool-ambiguous").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "stop collecting my messages",
        ),
    )
    .await;
    let conv = receipt.conversation_id;
    hold.started().await;
    support::await_ledger(&fixture.store, conv, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "B", "and answer me something"),
    )
    .await;
    hold.release();

    let blocks = support::await_ledger(&fixture.store, conv, "the declined turn", |blocks| {
        blocks.iter().any(|block| block.block_type == "tool_error")
            && blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
    })
    .await;
    let declined = blocks
        .iter()
        .find(|block| block.block_type == "tool_error")
        .expect("the recorded decline exists");
    assert_eq!(
        field(declined, "error"),
        privacy_tool::AMBIGUOUS_RESULT,
        "several people spoke, so the tool acts on nobody"
    );
    for person in ["A", "B"] {
        assert_eq!(
            principal_row(&fixture.store, person)
                .await
                .expect("the row stands")
                .2,
            0,
            "{person} was not flagged by the declined call"
        );
    }
}

/// The erased-row shape declines: the asker is erased mid-turn, their
/// stored principal id resolves to nobody, and the tool answers the same
/// ambiguity result instead of raising a flag no lookup would ever find.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_erased_askers_turn_declines_the_tool() {
    let hold = support::TurnHold::new();
    let fixture = privacy_tool_fixture(
        privacy_tool::ACTION_OPT_OUT,
        Some("One moment.".into()),
        Some(hold.clone()),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-tool-erased").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "stop collecting my messages",
        ),
    )
    .await;
    hold.started().await;
    assert_eq!(
        fixture
            .assistant
            .erase_principal(receipt.principal_id)
            .await
            .expect("the mid-turn erasure runs"),
        ErasureOutcome::Erased {
            deleted_conversations: vec![]
        },
        "the asker is erased while the turn holds"
    );
    hold.release();

    let blocks = support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the declined turn",
        |blocks| {
            blocks.iter().any(|block| block.block_type == "tool_error")
                && blocks
                    .last()
                    .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;
    let declined = blocks
        .iter()
        .find(|block| block.block_type == "tool_error")
        .expect("the recorded decline exists");
    assert_eq!(field(declined, "error"), privacy_tool::AMBIGUOUS_RESULT);
}

/// A failed flag write answers the transient result and changes nothing:
/// the injected fault aborts the identity update, the tool error carries
/// the fixed line, and the flag still reads clear.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_flag_write_answers_the_transient_result_and_changes_nothing() {
    let fixture = privacy_tool_fixture(privacy_tool::ACTION_OPT_OUT, None, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-tool-transient").await;
    sabotage_updates(&fixture.store, "principals").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "stop collecting my messages",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the transient turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[4], "error"),
        privacy_tool::TRANSIENT_RESULT,
        "the failed write answers the fixed transient result"
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        0,
        "nothing was recorded"
    );
}

/// An invalid tool action end to end (the closing verification's P8 shape,
/// kept as a permanent pin): the scripted model calls the tool with an
/// action outside the closed vocabulary, the full path — ingestion, turn,
/// call, runner — lands the fixed invalid-action result as the recorded
/// tool error on the ledger, and nothing changes for the asker.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_invalid_tool_action_reaches_the_recorded_result_end_to_end() {
    let fixture = privacy_tool_fixture("delete_everything", None, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-tool-invalid").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "please do something about my data",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the invalid-action turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[4], "error"),
        privacy_tool::INVALID_ACTION_RESULT,
        "the invalid action's fixed result is recorded on the ledger"
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        0,
        "nothing was changed by the invalid call"
    );
}

// ─── AC5: the bounds, and the stub under erasure ─────────────────────────

/// The per-person reply window bounds one person alone: the flooder's
/// replies stop at the cap, the state change past the cap is withheld with
/// the reply — never applied silently — and another member's confirm
/// instruction still answers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reply_window_bounds_one_person_and_withholds_the_silent_state_change() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-reply-bound").await;

    // The flood: alternating opt-out and opt-in, every one answered until
    // the cap. The alternation ends on an opt-in, so the flag reads clear
    // when the cap closes.
    let cap = usize::try_from(PRIVACY_REPLY_CAP).expect("the cap fits");
    for step in 0..cap {
        let token = if step % 2 == 0 {
            privacy::OPT_OUT_COMMAND
        } else {
            privacy::OPT_IN_COMMAND
        };
        assert!(
            command_reply(&fixture.assistant, &room, "A", token)
                .await
                .is_some(),
            "reply {step} inside the cap is granted"
        );
    }
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        None,
        "past the cap the reply is recorded silence"
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        0,
        "the withheld reply withheld its state change — no silent flag"
    );
    assert_eq!(
        command_reply(&fixture.assistant, &room, "B", privacy::DELETE_COMMAND).await,
        Some(privacy::CONFIRM_INSTRUCTION.to_owned()),
        "another person's rights reply is bounded independently"
    );
}

/// A budget-silenced sender still draws the rights answer: the principal
/// budget refuses their addressed messages, and the opt-out answers and
/// applies regardless — the budgets never gate the family.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_budget_silenced_sender_still_draws_the_rights_answer() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture =
        support::start_assistant_configured(store, None, support::budgets(Some((1, 600)), None))
            .await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-budget-rights").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "the first ask spends the budget",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::first_answer_to("the first ask spends the budget")
    );
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "A", "the second ask is refused"),
    )
    .await;
    support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the limited message",
        |blocks| {
            blocks
                .iter()
                .any(|block| block.fields.get("limited") == Some(&json!("principal")))
        },
    )
    .await;

    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        Some(privacy::OPT_OUT_DONE.to_owned()),
        "the silenced sender's rights command still answers"
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        1,
        "and the state change arrived with the granted reply"
    );
}

/// Erasure with the flag standing keeps the stub: the row survives emptied
/// with the flag up, the person's texts are nulled, and a repeat erasure
/// reports completion over the emptiness instead of not-found.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasure_with_the_flag_standing_keeps_the_stub_and_a_repeat_reports_completion() {
    let fixture = support::start_assistant(None).await;
    let room = support::authorized_group(&fixture.assistant, "room-stub").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "A", "words behind the stub"),
    )
    .await;
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        Some(privacy::OPT_OUT_DONE.to_owned())
    );
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::DELETE_COMMAND).await,
        Some(privacy::CONFIRM_INSTRUCTION.to_owned()),
        "the deletion commands stay exempt from suppression"
    );
    assert_eq!(
        bounded_command_reply(&fixture.assistant, &room, "A", privacy::CONFIRM_COMMAND).await,
        Some(privacy::DELETION_STARTED.to_owned())
    );
    // The nulled texts are the awaited observable: the identity row's own
    // fields cannot tell the erasure apart, since a sender without a
    // username already reads the typed absence before the emptying runs.
    await_nulled_texts(
        &fixture.store,
        receipt.principal_id,
        "the erasure's nulling pass",
    )
    .await;
    await_principal_shape(&fixture.store, "A", "the emptied stub", |row| {
        matches!(row, Some((_, None, 1)))
    })
    .await;
    // Still suppressed: the flag survived its own person's deletion.
    ingest_dropped(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "a message after the deletion",
        ),
    )
    .await;
    // An exempt command after the deletion, carrying a delivered handle:
    // the fresh row attaches to the stub with no speaker — no command
    // re-materializes the field the erasure emptied.
    let outcome = fixture
        .assistant
        .ingest(with_username(
            with_command(
                inbound_unaddressed(&room, ChannelKind::Group, "A", privacy::PRIVACY_COMMAND),
                privacy::PRIVACY_COMMAND,
            ),
            "revived-handle",
        ))
        .await
        .expect("the post-erasure exempt command ingests");
    assert!(
        matches!(outcome, IngestOutcome::Recorded { .. }),
        "the exempt command is recorded on the stub: {outcome:?}"
    );
    assert_eq!(
        stored_speakers(&fixture.store, receipt.principal_id)
            .await
            .last()
            .expect("the command row stands"),
        &None,
        "the post-erasure exempt command row holds no speaker"
    );
    assert_eq!(
        fixture
            .assistant
            .erase_principal(receipt.principal_id)
            .await
            .expect("the repeat erasure runs"),
        ErasureOutcome::Erased {
            deleted_conversations: vec![]
        },
        "the repeat re-runs over emptiness and reports completion"
    );
}

// ─── AC1: the appended step's upgrade ────────────────────────────────────

/// AC1's upgrade pin: a store decision 0077's binary wrote — no
/// suppression flag, the domain's version at twelve — upgrades through the
/// appended step alone. The step adds the column, the version advances,
/// the pre-existing row reads unflagged, and the write path raises the
/// flag for the first post-upgrade opt-out.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_version_twelve_store_upgrades_through_the_suppression_step_alone() {
    let db = support::TempDb::new("v12-upgrade");
    {
        let store = Store::open_with(db.path(), store_config()).expect("the store opens");
        domain_run(&store.tx(), DOMAIN, |conn| {
            conn.execute(
                "INSERT INTO principals (adapter, external_id, username)
                 VALUES ('test-adapter', 'A', NULL)",
                [],
            )?;
            // The rewind: drop exactly what the steps past version twelve
            // add — the suppression flag, and the literal-addressed column
            // the later grounded-answer step appends — and set the version
            // back, leaving the previous unit's disk shape. The
            // non-vacuity check proves the drop was real.
            conn.execute_batch(&format!(
                "ALTER TABLE principals DROP COLUMN opted_out;
                     DROP INDEX {revises_index};
                     ALTER TABLE block_chat_message DROP COLUMN revises;
                     ALTER TABLE block_chat_message DROP COLUMN literal_addressed;
                     DROP TABLE block_join_notice;
                     DROP TABLE block_delivered;
                     DROP TABLE block_message_mark;",
                revises_index = assistant_core::schema::MESSAGE_REVISES_INDEX.as_str(),
            ))?;
            let refused = conn.execute("UPDATE principals SET opted_out = 1", []);
            assert!(
                refused.is_err(),
                "the genuine version-twelve table has no suppression flag"
            );
            Ok(())
        })
        .await
        .expect("the store rewinds to the previous unit's shape");
        support::rewind_domain_migration_version(&store, 12).await;
        // The first store closes before the reopen, so the upgrade reads
        // the disk, not a live connection.
    }

    let reopened = Store::open_with(db.path(), store_config())
        .expect("the version-twelve store reopens under the shipped configuration");
    assert_eq!(
        support::domain_migration_version(&reopened).await,
        19,
        "the appended steps advanced the domain's version"
    );
    assert_eq!(
        principal_row(&reopened, "A").await.expect("the row").2,
        0,
        "the pre-existing row reads unflagged through the default"
    );

    let fixture = support::start_assistant_on(reopened, None).await;
    let room = support::authorized_group(&fixture.assistant, "room-upgraded").await;
    assert_eq!(
        command_reply(&fixture.assistant, &room, "A", privacy::OPT_OUT_COMMAND).await,
        Some(privacy::OPT_OUT_DONE.to_owned())
    );
    assert_eq!(
        principal_row(&fixture.store, "A").await.expect("the row").2,
        1,
        "the write path raises the upgraded column"
    );
    ingest_dropped(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "dropped over the upgraded store",
        ),
    )
    .await;
}
