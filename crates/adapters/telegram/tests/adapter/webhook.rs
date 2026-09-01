//! The webhook intake's pins (unit 35), every one of them driven through a
//! real local HTTP round trip against the door the adapter binds, with the
//! scripted platform on the other side recording what was registered.
//!
//! What they hold: the mode is one predicate — a webhook configuration
//! serves and never polls, its absence polls and first deletes any
//! registered webhook; the start binds before it registers and refuses
//! loudly where it cannot serve, the unpersisted secret included; the door
//! authenticates and bounds every delivery and holds at most its stated
//! number of connections; the response code is the acknowledgement,
//! duplicates are met, a consumer that never answers has the door answering
//! at its deadline, and a step slower than that deadline converges through
//! the redelivery; the offset file is untouched; and the secret is generated
//! owner-only, persisted atomically, and kept.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use agent_ledger::StoreError;
use assistant_adapter_telegram::{AdapterError, TelegramAdapter, webhook_secret_path};
use reqwest::Method;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

use crate::server::BotApiServer;
use crate::support::{
    DEADLINE, TempStateFile, WEBHOOK_PATH, WEBHOOK_PUBLIC_URL, authorize_group,
    await_chat_messages, await_conversations, deliver, group_update, kept_secret, knock,
    pending_sleep, private_update, recording_sleep, sleep_answering_first, spawn_adapter,
    spawn_webhook_adapter, spawn_webhook_adapter_for_outcome, start_assistant, try_knock,
    webhook_adapter_config,
};

/// The update types both intakes pin, as the poll request already names them.
const CONSUMED_UPDATE_TYPES: [&str; 3] = ["message", "edited_message", "my_chat_member"];

/// The most connections the door serves at once, as the adapter states it:
/// twice the queue's depth, so a full queue is answered instead of left
/// unaccepted. The pin below is the contract that number keeps.
const CONNECTION_CAP: usize = 128;

/// The characters a generated secret may carry — the platform's own
/// permitted alphabet.
fn is_permitted(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

/// The mode bits of one file.
fn mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .expect("the file's metadata reads")
        .permissions()
        .mode()
        & 0o777
}

/// Set one file's permission bits.
fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .expect("the file's permissions set");
}

/// AC2 and AC7: with the section, the door serves and NOTHING polls — and
/// the offset file is neither read nor written. The file is pre-filled with
/// an offset far past the delivered update, so a run that consulted it would
/// have to skip the delivery, and a run that wrote it would have to change
/// the number; neither happens.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_webhook_configuration_serves_deliveries_and_never_polls() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-serves");
    std::fs::write(state.path(), "999").expect("the pre-filled offset writes");

    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());

    let status = deliver(
        address,
        &secret,
        &private_update(10, 3, "delivered, not polled"),
    )
    .await;
    assert_eq!(status, 200, "an acknowledged delivery answers 200");

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages[0].fields["text"], json!("delivered, not polled"));
    assert!(
        server.recorded("getUpdates").is_empty(),
        "no poll reaches the wire in webhook mode"
    );
    assert_eq!(
        std::fs::read_to_string(state.path()).expect("the state file reads"),
        "999",
        "the offset file is neither read nor written by the webhook intake"
    );
    let mut sidecar = state.path().to_path_buf().into_os_string();
    sidecar.push(".next");
    assert!(
        !std::path::Path::new(&sidecar).exists(),
        "no offset write was even attempted"
    );
}

/// AC3: the start binds first and registers second — read at the exact
/// moment the port is bound — and what it registers is the address, the
/// shared update-type list, the explicit refusal to drop pending updates,
/// and the adapter's own secret.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_start_binds_before_it_registers_and_pins_what_it_registers() {
    let fixture = start_assistant().await;
    let server = Arc::new(BotApiServer::start().await);
    let state = TempStateFile::new("webhook-order");

    // Read inside the bind announcement, which runs after the bind and
    // before the registration call is made: a registration already recorded
    // there would be one made before anything served.
    let registered_at_bind = Arc::new(AtomicBool::new(false));
    let identity_at_bind = Arc::new(AtomicBool::new(false));
    let observer = {
        let server = Arc::clone(&server);
        let registered_at_bind = Arc::clone(&registered_at_bind);
        let identity_at_bind = Arc::clone(&identity_at_bind);
        Arc::new(move |_: SocketAddr| {
            registered_at_bind.store(!server.recorded("setWebhook").is_empty(), Ordering::SeqCst);
            identity_at_bind.store(!server.recorded("getMe").is_empty(), Ordering::SeqCst);
        })
    };

    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        Some(observer),
    )
    .await;

    let registrations = server.await_recorded("setWebhook", 1).await;
    assert!(
        !registered_at_bind.load(Ordering::SeqCst),
        "the address must not be registered before the listener is bound"
    );
    assert!(
        identity_at_bind.load(Ordering::SeqCst),
        "the identity is read before the listener binds — the shared step \
         cannot translate without it"
    );

    let registration = &registrations[0].body;
    assert_eq!(registration["url"], json!(WEBHOOK_PUBLIC_URL));
    assert_eq!(
        registration["allowed_updates"],
        json!(CONSUMED_UPDATE_TYPES),
        "the registration pins the same update types the poll does"
    );
    assert_eq!(
        registration["drop_pending_updates"],
        json!(false),
        "whatever queued through an outage is delivered, never discarded"
    );
    let secret = kept_secret(state.path());
    assert_eq!(
        registration["secret_token"],
        json!(secret),
        "the registered secret is the one the adapter kept"
    );
    assert_eq!(secret.chars().count(), 64);
    assert!(
        secret.chars().all(is_permitted),
        "the secret is drawn from the platform's permitted alphabet"
    );

    // And the port that was bound before all this genuinely serves.
    let status = deliver(
        address,
        &secret,
        &private_update(11, 3, "after the registration"),
    )
    .await;
    assert_eq!(status, 200);
}

/// AC3: each of the four startup steps refuses the start with its own named
/// error — a webhook deployment that cannot serve must never come up quiet.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_start_that_cannot_serve_refuses_loudly() {
    let fixture = start_assistant().await;

    // The identity read, which the shared step cannot translate without.
    let blind = BotApiServer::start_without_identity().await;
    let state = TempStateFile::new("webhook-refusal-identity");
    let (sleep, _) = pending_sleep();
    let refusal =
        TelegramAdapter::with_sleep(webhook_adapter_config(&blind, state.path(), 0), sleep)
            .run(Arc::clone(&fixture.assistant))
            .await
            .expect_err("a failed identity read refuses the webhook start");
    assert!(
        matches!(refusal, AdapterError::Identity(_)),
        "the refusal names the identity read; got {refusal}"
    );
    assert!(
        blind.recorded("setWebhook").is_empty(),
        "nothing is registered when the start already refused"
    );

    // The secret that cannot be kept: a directory sits where the sidecar the
    // persist writes through belongs, so the write fails and the start is
    // refused instead of running on a secret the next start would replace.
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-refusal-secret");
    let mut blocked = webhook_secret_path(state.path()).into_os_string();
    blocked.push(".next");
    std::fs::create_dir(&blocked).expect("the blocking directory is created");
    let (sleep, _) = pending_sleep();
    let refusal =
        TelegramAdapter::with_sleep(webhook_adapter_config(&server, state.path(), 0), sleep)
            .run(Arc::clone(&fixture.assistant))
            .await
            .expect_err("a secret that cannot be kept refuses the webhook start");
    assert!(
        matches!(refusal, AdapterError::Secret(_)),
        "the refusal names the secret; got {refusal}"
    );
    assert!(
        !webhook_secret_path(state.path()).exists(),
        "a persist that failed leaves no half-written secret behind"
    );
    assert!(
        server.recorded("setWebhook").is_empty(),
        "no secret is registered that the next start could not read back"
    );
    std::fs::remove_dir(&blocked).expect("the blocking directory is removed");

    // The bind, against a port this test holds itself.
    let server = BotApiServer::start().await;
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the test holds a loopback port");
    let port = occupied.local_addr().expect("the held port reads").port();
    let state = TempStateFile::new("webhook-refusal-bind");
    let (sleep, _) = pending_sleep();
    let refusal =
        TelegramAdapter::with_sleep(webhook_adapter_config(&server, state.path(), port), sleep)
            .run(Arc::clone(&fixture.assistant))
            .await
            .expect_err("an unbindable port refuses the webhook start");
    assert!(
        matches!(refusal, AdapterError::Listener { port: named, .. } if named == port),
        "the refusal names the port; got {refusal}"
    );
    assert!(
        server.recorded("setWebhook").is_empty(),
        "the platform is never pointed at a port nothing serves"
    );

    // The registration, refused by the platform — with the scripted refusal
    // quoting the secret it was handed, the way a platform description
    // quotes the parameter it refused.
    let server = BotApiServer::start().await;
    server.fail_registration();
    let state = TempStateFile::new("webhook-refusal-registration");
    let (sleep, _) = pending_sleep();
    let refusal =
        TelegramAdapter::with_sleep(webhook_adapter_config(&server, state.path(), 0), sleep)
            .run(Arc::clone(&fixture.assistant))
            .await
            .expect_err("a refused registration refuses the webhook start");
    assert!(
        matches!(refusal, AdapterError::Registration(_)),
        "the refusal names the registration; got {refusal}"
    );
    let rendered = refusal.to_string();
    assert!(
        !rendered.contains(&kept_secret(state.path())),
        "the refusal's text carries no secret: {rendered}"
    );
    assert!(
        rendered.contains("[redacted]"),
        "the platform's own quoting of the secret is scrubbed, not dropped: {rendered}"
    );
}

/// AC3's polling half: the poll start deletes the registration
/// unconditionally — one call, whether or not anything was registered, and
/// nothing asked first — and polls anyway when that delete fails.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_polling_start_deletes_the_registration_unconditionally() {
    // One registered: deleted, then polled.
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.set_registered_webhook(WEBHOOK_PUBLIC_URL);
    let state = TempStateFile::new("poll-clears-webhook");
    server.push_update(private_update(20, 4, "polled after the delete"));
    let (sleep, _) = recording_sleep();
    let adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    server.await_recorded("deleteWebhook", 1).await;
    assert!(
        server.registered_webhook().is_empty(),
        "the registration is gone before the poll runs"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;
    drop(adapter);

    // None registered: the same one delete, and nothing asked beforehand.
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("poll-nothing-to-clear");
    server.push_update(private_update(21, 4, "polled with no webhook set"));
    let (sleep, _) = recording_sleep();
    let adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(
        server.recorded("deleteWebhook").len(),
        1,
        "the delete is made once whether or not anything was registered"
    );
    assert!(
        server.recorded("getWebhookInfo").is_empty(),
        "nothing is asked first; the delete is idempotent at the platform"
    );
    drop(adapter);

    // The delete itself failing: the local deployment starts anyway.
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.fail_webhook_delete();
    let state = TempStateFile::new("poll-delete-fails");
    server.push_update(private_update(22, 4, "polled past the failed delete"));
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(
        messages[0].fields["text"],
        json!("polled past the failed delete")
    );
}

/// One knock the door must refuse, and the status it owes it.
struct RefusedKnock<'a> {
    status: u16,
    why: &'static str,
    method: Method,
    path: &'a str,
    offered: Option<&'a str>,
    body: Vec<u8>,
}

/// Every knock the door must refuse, in one list beside the pin that walks
/// them, so what the door is reads as one statement.
fn refused_knocks<'a>(
    secret: &'a str,
    wrong_secret: &'a str,
    query_path: &'a str,
    update: &str,
) -> Vec<RefusedKnock<'a>> {
    vec![
        RefusedKnock {
            status: 403,
            why: "a wrong secret is discarded",
            method: Method::POST,
            path: WEBHOOK_PATH,
            offered: Some(wrong_secret),
            body: update.as_bytes().to_vec(),
        },
        RefusedKnock {
            status: 403,
            why: "a missing secret is discarded",
            method: Method::POST,
            path: WEBHOOK_PATH,
            offered: None,
            body: update.as_bytes().to_vec(),
        },
        RefusedKnock {
            status: 400,
            why: "a body that does not parse is refused",
            method: Method::POST,
            path: WEBHOOK_PATH,
            offered: Some(secret),
            body: b"{ not an update".to_vec(),
        },
        RefusedKnock {
            status: 413,
            why: "a body past the bound is refused instead of read",
            method: Method::POST,
            path: WEBHOOK_PATH,
            offered: Some(secret),
            body: vec![b'x'; 1024 * 1024 + 1],
        },
        RefusedKnock {
            status: 404,
            why: "another path is not this door",
            method: Method::POST,
            path: "/somewhere-else",
            offered: Some(secret),
            body: update.as_bytes().to_vec(),
        },
        RefusedKnock {
            status: 404,
            why: "the path is matched exactly, with no query string",
            method: Method::POST,
            path: query_path,
            offered: Some(secret),
            body: update.as_bytes().to_vec(),
        },
        RefusedKnock {
            status: 405,
            why: "the door accepts exactly one method",
            method: Method::GET,
            path: WEBHOOK_PATH,
            offered: Some(secret),
            body: Vec::new(),
        },
    ]
}

/// AC4: the door authenticates and bounds, on the address's own path. Every
/// refusal reads nothing into the pipeline — the one delivery that passes is
/// the only thing the ledger ever holds — and none of them panics.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_door_authenticates_and_bounds_every_delivery() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-door");
    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());
    let update = private_update(30, 5, "the only one that gets in").to_string();
    let wrong_secret: String = secret.chars().rev().collect();
    let query_path = format!("{WEBHOOK_PATH}?token=guessed");

    for knock_at_the_door in refused_knocks(&secret, &wrong_secret, &query_path, &update) {
        let RefusedKnock {
            status,
            why,
            method,
            path,
            offered,
            body,
        } = knock_at_the_door;
        assert_eq!(
            knock(address, method, path, offered, body).await,
            status,
            "{why}"
        );
    }

    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "no refused delivery reached the pipeline"
    );

    assert_eq!(
        knock(
            address,
            Method::POST,
            WEBHOOK_PATH,
            Some(&secret),
            update.into_bytes(),
        )
        .await,
        200,
        "the delivery that carries the right secret is served"
    );
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(
        messages[0].fields["text"],
        json!("the only one that gets in")
    );
}

/// AC4's backpressure: with the consumer wedged inside one delivery's own
/// platform call, the queue fills and the door answers 503 — honest
/// backpressure the platform's retry absorbs, with nothing ingested.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_full_queue_is_refused_for_redelivery() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -350;
    // The wedge: the step parks inside the administrator fetch of this
    // chat's first message, so everything behind it queues.
    server.hang_admins(chat);
    authorize_group(&fixture.assistant, chat).await;
    let state = TempStateFile::new("webhook-queue");
    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());

    // One held in the consumer, the queue's sixty-four behind it, and more
    // than enough past that to be refused.
    let refused = Arc::new(AtomicBool::new(false));
    let mut knocking = Vec::new();
    for id in 0..80 {
        let body = group_update(400 + id, chat, 6, "queued behind the wedge")
            .to_string()
            .into_bytes();
        let secret = secret.clone();
        let refused = Arc::clone(&refused);
        knocking.push(tokio::spawn(async move {
            if try_knock(address, Method::POST, WEBHOOK_PATH, Some(&secret), body).await
                == Some(503)
            {
                refused.store(true, Ordering::SeqCst);
            }
        }));
    }

    let deadline = std::time::Instant::now() + DEADLINE;
    while !refused.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the full queue's refusal"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    for held in knocking {
        held.abort();
    }
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "a refused delivery reads nothing into the pipeline, and the wedged \
         one records nothing either"
    );
}

/// AC5: the response code is honest. A failed ingest is refused, the same
/// update redelivered afterwards ingests exactly once, and a duplicate of an
/// acknowledged update answers 200 without a second ingest.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_delivery_redelivers_once_and_a_duplicate_is_met() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-acknowledgement");
    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());
    let update = private_update(50, 7, "said exactly once");

    // Hide the identity table: the ingest fails inside the core with a
    // storage error, the transient class.
    fixture
        .store
        .run(|conn| {
            conn.execute("ALTER TABLE principals RENAME TO principals_hidden", [])?;
            Ok(())
        })
        .await
        .expect("the identity table hides");
    assert_eq!(
        deliver(address, &secret, &update).await,
        500,
        "a failed ingest is refused so the platform redelivers"
    );
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "nothing was recorded by the refused delivery"
    );

    // Storage answers again: the redelivery ingests, once.
    fixture
        .store
        .run(|conn| {
            conn.execute("ALTER TABLE principals_hidden RENAME TO principals", [])?;
            Ok(())
        })
        .await
        .expect("the identity table returns");
    assert_eq!(deliver(address, &secret, &update).await, 200);
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 1).await;

    // The duplicate meets the acknowledged set: answered, not re-ingested.
    assert_eq!(
        deliver(address, &secret, &update).await,
        200,
        "a duplicate of an acknowledged update is answered"
    );
    let messages = await_chat_messages(&fixture.store, conversation, 1).await;
    assert_eq!(messages.len(), 1, "the duplicate was not ingested again");
    assert_eq!(messages[0].fields["text"], json!("said exactly once"));
}

/// AC5's third answer, beside the acknowledgement and the refusal: a core
/// that can serve nothing more ends the run for the supervisor to replace
/// (2026-09-01). The delivery it was working on is never acknowledged — it
/// is refused, or the ending run cuts the connection before the refusal is
/// written, which the platform reads the same way — so the update is still
/// there for the replacement process.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_core_that_cannot_serve_refuses_the_delivery_and_ends_the_run() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-core-cannot-serve");
    let (sleep, _) = pending_sleep();
    let (run, address) = spawn_webhook_adapter_for_outcome(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());

    // Kill the one writer the way a bug would: a panic on its own thread.
    // Every store call from here answers `ActorStopped`, which the core
    // states as the fatal class.
    let killed = fixture
        .store
        .run(|_conn| -> Result<(), StoreError> { panic!("the scripted writer death") })
        .await;
    assert!(
        matches!(killed, Err(StoreError::ActorStopped)),
        "killing the writer answers ActorStopped; answered {killed:?}"
    );

    let update = private_update(60, 9, "meets the departed writer");
    let answered = try_knock(
        address,
        Method::POST,
        WEBHOOK_PATH,
        Some(&secret),
        update.to_string().into_bytes(),
    )
    .await;
    assert_ne!(
        answered,
        Some(200),
        "the update is never acknowledged: it is refused, or the ending run \
         cuts the connection"
    );
    let outcome = tokio::time::timeout(DEADLINE, run)
        .await
        .expect("the run ends before the deadline")
        .expect("the run's task is not aborted");
    assert!(
        matches!(outcome, Err(AdapterError::CoreCannotServe)),
        "the run ends stating the core cannot serve; ended {outcome:?}"
    );
}

/// AC5's deadline: with the consumer wedged, the door answers at its own
/// bound instead of holding the connection — and the bound it waited is the
/// stated one, taken through the sleep seam.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_door_answers_at_its_deadline_when_the_consumer_does_not() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -360;
    server.hang_admins(chat);
    authorize_group(&fixture.assistant, chat).await;
    let state = TempStateFile::new("webhook-deadline");
    // The recording sleep answers every wait at once, so the deadline is
    // driven without any real time passing.
    let (sleep, waits) = recording_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());

    let status = deliver(
        address,
        &secret,
        &group_update(60, chat, 6, "the wedged delivery"),
    )
    .await;
    assert_eq!(
        status, 500,
        "the door answers at its deadline instead of holding the connection"
    );
    assert!(
        waits
            .lock()
            .expect("the wait log locks")
            .contains(&Duration::from_secs(25)),
        "the wait the door took is the stated deadline: {:?}",
        waits.lock().expect("the wait log locks")
    );
}

/// AC5's convergence, which is what the deadline actually costs: a step
/// SLOWER than the deadline that then succeeds loses nothing. The door has
/// answered 500 while the platform call was still running, the step then
/// completes and records the update, and the platform's redelivery meets
/// that record and is answered 200 — one ingest, no loss, one extra
/// delivery paid.
///
/// The second delivery sequences it: the consumer is strictly serial, so
/// the second update is taken only after the first step finished AND its
/// id was recorded, which makes the ledger holding both messages the proof
/// that the redelivery below meets the memory instead of racing it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_step_slower_than_the_deadline_converges_on_the_redelivery() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -370;
    // Slow, not failed: the step parks inside the administrator fetch past
    // the door's deadline, and the fetch answers afterwards.
    server.hang_admins(chat);
    authorize_group(&fixture.assistant, chat).await;
    let state = TempStateFile::new("webhook-convergence");
    // The two refused deliveries take their deadline at once; every wait
    // past them parks, so the redelivery below is answered by the consumer
    // and never by a deadline that fired underneath it.
    let (sleep, _) = sleep_answering_first(2);
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let secret = kept_secret(state.path());
    let slow = group_update(90, chat, 6, "slow, not lost");

    assert_eq!(
        deliver(address, &secret, &slow).await,
        500,
        "the door answers at its deadline while the step is still running"
    );
    assert_eq!(
        deliver(
            address,
            &secret,
            &group_update(91, chat, 6, "queued behind it")
        )
        .await,
        500,
        "the delivery queued behind the slow step is refused at the deadline too"
    );

    // The slow call answers: both steps run to completion, in order.
    server.set_admins(chat, &[]);
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(messages[0].fields["text"], json!("slow, not lost"));
    assert_eq!(messages[1].fields["text"], json!("queued behind it"));

    // The platform redelivers what it was refused, and meets the record the
    // slow step left behind.
    assert_eq!(
        deliver(address, &secret, &slow).await,
        200,
        "the redelivery of a slow-but-successful step is acknowledged"
    );
    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(
        messages.len(),
        2,
        "the redelivery was not ingested a second time"
    );
}

/// One connection opened at the door, answered, and left open: the request
/// goes out whole and its answer comes back, which is the proof that this
/// connection was accepted and holds one of the door's places. The answer
/// is the 405 the door owes a method it does not take, and it carries the
/// method it does — a 405 without `Allow` describes nothing.
async fn held_connection(address: SocketAddr) -> tokio::net::TcpStream {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("the door accepts a connection");
    stream
        .write_all(format!("GET {WEBHOOK_PATH} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
        .await
        .expect("the held connection's request writes");
    let answer = read_answer(&mut stream).await;
    assert!(
        answer.starts_with("HTTP/1.1 405"),
        "the held connection is answered: {answer:?}"
    );
    assert!(
        answer.to_ascii_lowercase().contains("allow: post"),
        "the refusal names the one method the door takes: {answer:?}"
    );
    stream
}

/// One answer read off a connection, as text.
async fn read_answer(stream: &mut tokio::net::TcpStream) -> String {
    use tokio::io::AsyncReadExt;

    let mut buffer = [0_u8; 512];
    let read = stream
        .read(&mut buffer)
        .await
        .expect("the door's answer reads");
    String::from_utf8_lossy(&buffer[..read]).into_owned()
}

/// AC4's connection bound: the door holds at most [`CONNECTION_CAP`]
/// connections and accepts nothing past that, so a peer that opens sockets
/// on a port the reverse proxy forwards the public internet to cannot make
/// the process spawn without end. With the cap's worth of connections open
/// and idle, a further knock is not answered at all; releasing them frees
/// the places and the same knock is served.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_door_holds_no_more_connections_than_its_cap() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-connections");
    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;

    // Each one is answered before the next is opened, so all of them are
    // provably accepted and none is still waiting in the kernel's backlog.
    let mut held = Vec::new();
    for _ in 0..CONNECTION_CAP {
        held.push(held_connection(address).await);
    }

    let probe = tokio::time::timeout(
        Duration::from_millis(500),
        try_knock(address, Method::GET, WEBHOOK_PATH, None, Vec::new()),
    )
    .await;
    assert!(
        probe.is_err(),
        "at the cap the door accepts nothing more: {probe:?}"
    );

    drop(held);
    let answered = tokio::time::timeout(
        DEADLINE,
        try_knock(address, Method::GET, WEBHOOK_PATH, None, Vec::new()),
    )
    .await
    .expect("a freed place is taken within the deadline");
    assert_eq!(
        answered,
        Some(405),
        "the door serves again once its connections are released"
    );
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "nothing about the connection bound reaches the pipeline"
    );
}

/// AC6: the secret is generated owner-only, persisted through a sidecar and
/// a rename like the offset beside it, kept across starts, and carried by no
/// configuration — the adapter's own, which no human handles.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_secret_is_generated_owner_only_and_kept() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-secret");
    let (sleep, _) = pending_sleep();
    let (adapter, _address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    // Each start is let finish its registration before its adapter is
    // dropped: the bind is announced first, so an adapter dropped straight
    // after the announcement could cut the registration off mid-flight.
    let registrations = server.await_recorded("setWebhook", 1).await;
    let first = kept_secret(state.path());
    let mut sidecar = webhook_secret_path(state.path()).into_os_string();
    sidecar.push(".next");
    assert!(
        !std::path::Path::new(&sidecar).exists(),
        "the secret was written through a sidecar and renamed, leaving none behind"
    );
    assert_eq!(
        registrations[0].body["secret_token"],
        json!(first),
        "the generated secret is what the first start registered"
    );
    assert_eq!(
        mode_of(&webhook_secret_path(state.path())),
        0o600,
        "the kept secret is readable by its owner and nobody else"
    );
    let rendered = format!("{:?}", webhook_adapter_config(&server, state.path(), 8085));
    assert!(
        !rendered.contains(&first),
        "no configuration carries the secret: {rendered}"
    );
    drop(adapter);

    // A second start on the same state file registers the same secret.
    let (sleep, _) = pending_sleep();
    let (adapter, _address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let registrations = server.await_recorded("setWebhook", 2).await;
    assert_eq!(
        registrations[1].body["secret_token"],
        json!(first),
        "the kept secret is reused, not regenerated"
    );
    assert_eq!(kept_secret(state.path()), first);
    drop(adapter);

    // A usable secret found at wider permissions: kept, and narrowed.
    set_mode(&webhook_secret_path(state.path()), 0o644);
    let (sleep, _) = pending_sleep();
    let (_adapter, _address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    server.await_recorded("setWebhook", 3).await;
    assert_eq!(
        kept_secret(state.path()),
        first,
        "a usable secret is kept whatever its permissions were"
    );
    assert_eq!(
        mode_of(&webhook_secret_path(state.path())),
        0o600,
        "permissions past the owner are corrected on read"
    );
}

/// AC6's convergence: a kept secret nobody can use is replaced with a
/// structural warning instead of refusing the start — and the replacement is
/// what that start registers, so the door and the platform agree again while
/// the old secret opens nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unusable_kept_secret_is_replaced_and_registered() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("webhook-secret-replaced");
    std::fs::write(webhook_secret_path(state.path()), "not a usable token")
        .expect("the unusable secret writes");
    set_mode(&webhook_secret_path(state.path()), 0o644);

    let (sleep, _) = pending_sleep();
    let (_adapter, address) = spawn_webhook_adapter(
        &server,
        state.path(),
        Arc::clone(&fixture.assistant),
        sleep,
        None,
    )
    .await;
    let replaced = kept_secret(state.path());
    assert_ne!(
        replaced, "not a usable token",
        "the kept secret is replaced"
    );
    assert_eq!(replaced.chars().count(), 64);
    assert!(replaced.chars().all(is_permitted));
    assert_eq!(
        mode_of(&webhook_secret_path(state.path())),
        0o600,
        "the replacement is written owner-only"
    );
    let registrations = server.await_recorded("setWebhook", 1).await;
    assert_eq!(
        registrations[0].body["secret_token"],
        json!(replaced),
        "the replacement is what this start registered"
    );
    assert_eq!(
        deliver(
            address,
            &replaced,
            &private_update(70, 9, "after the replacement")
        )
        .await,
        200
    );
    assert_eq!(
        knock(
            address,
            Method::POST,
            WEBHOOK_PATH,
            Some("not a usable token"),
            Value::Null.to_string().into_bytes(),
        )
        .await,
        403,
        "the replaced secret opens nothing"
    );
}
