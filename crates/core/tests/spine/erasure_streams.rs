//! Erasure ordered against open streams: the interrupt-settle-delete path
//! for a direct conversation caught mid-stream, the loud timeout that
//! deletes nothing, and the idle erasure that pays no wait.

use std::sync::Arc;

use agent_ledger::store::domain_run;
use agent_ledger::{CoreEvent, EventBus, ProviderModule, Store, StreamEvent, ToolRegistry};
use assistant_core::schema::store_config;
use assistant_core::{Assistant, ChannelKind, CoreError, ErasureOutcome, schema};
use serde_json::json;

use crate::support::{self, await_ledger, channel, inbound};

/// The stored identity rows' external ids — read raw, because the loud
/// timeout's promise is about what the tables still hold.
async fn stored_principals(store: &Store) -> Vec<String> {
    domain_run(&store.tx(), schema::DOMAIN, |conn| {
        let mut statement = conn.prepare("SELECT external_id FROM principals ORDER BY id")?;
        let rows = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the identity table reads")
}

/// Erasing mid-stream: the held stream is interrupted, the ledger settles —
/// streaming tail gone, the interrupt's status append in — and only then is
/// the direct conversation deleted, whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasing_during_a_held_stream_interrupts_settles_then_deletes() {
    let hold = support::TurnHold::new();
    let fixture = support::start_assistant(Some(hold.clone())).await;
    let key = channel("dm-held");

    let receipt = fixture
        .assistant
        .ingest(inbound(&key, ChannelKind::Direct, "A", "the held ask"))
        .await
        .expect("the message ingests");
    let conv = receipt.conversation_id;
    hold.started().await;
    // The stream is provably open once its tail is in stored state; waiting
    // for that is what makes the erasure's mid-stream path deterministic
    // instead of racing the reader.
    await_ledger(&fixture.store, conv, "the streaming tail", |blocks| {
        blocks.iter().any(|b| b.block_type == "streaming")
    })
    .await;

    let mut events = fixture.bus.subscribe();
    let outcome = fixture
        .assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("the mid-stream erasure settles and succeeds");
    assert_eq!(
        outcome,
        ErasureOutcome::Erased {
            deleted_conversations: vec![conv],
        }
    );

    // The interrupt went out before the deletion.
    let mut interrupted = false;
    while let Ok(event) = events.try_recv() {
        if matches!(event, CoreEvent::InterruptRequested { conversation_id } if conversation_id == conv)
        {
            interrupted = true;
        }
    }
    assert!(interrupted, "the open stream was interrupted");

    assert!(
        fixture
            .store
            .find_conversation(conv)
            .await
            .expect("the conversation table reads")
            .is_none(),
        "the direct conversation is removed entirely"
    );
    assert_eq!(
        stored_principals(&fixture.store).await,
        Vec::<String>::new()
    );
}

/// An idle erasure pays no wait: no stream is open, so no interrupt is
/// emitted and the call returns straight from the deletion steps.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_idle_erasure_pays_no_wait() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-idle");

    let receipt = fixture
        .assistant
        .ingest(inbound(&key, ChannelKind::Direct, "A", "the settled ask"))
        .await
        .expect("the message ingests");
    support::recv_reply(&mut replies).await;
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the settled turn",
        3,
    )
    .await;

    let mut events = fixture.bus.subscribe();
    let before = std::time::Instant::now();
    let outcome = fixture
        .assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("the idle erasure succeeds");
    assert_eq!(
        outcome,
        ErasureOutcome::Erased {
            deleted_conversations: vec![receipt.conversation_id],
        }
    );
    assert!(
        before.elapsed() < std::time::Duration::from_secs(2),
        "an idle erasure returns without a settle wait"
    );
    while let Ok(event) = events.try_recv() {
        assert!(
            !matches!(event, CoreEvent::InterruptRequested { .. }),
            "an idle erasure interrupts nothing"
        );
    }
}

/// A provider that opens a stream, writes a tail, and then holds the stream
/// open forever, deaf to the interrupt — the shape that forces the settle
/// timeout.
fn deaf_provider() -> Box<dyn ProviderModule> {
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
            // Hold the response half open forever, ignoring the interrupt:
            // the reader never sees the stream close.
            std::future::pending::<()>().await;
        });
        (request_tx, responses)
    })
}

/// Past the settle bound the erasure fails loudly and deletes nothing: the
/// identity rows, the conversation and its blocks all stay. A retry then
/// completes from stored state, because the failed call dropped its
/// timed-out observation and the interrupt's teardown left no tail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stream_that_never_settles_fails_the_erasure_loudly_deleting_nothing() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        support::registry_of(deaf_provider()),
        Arc::new(ToolRegistry::new()),
        support::binding(),
        support::SYSTEM_PROMPT.into(),
        assistant_core::ProtectionConfig::default(),
    )
    .await
    .expect("the assembly starts");
    let key = channel("dm-deaf");

    let receipt = assistant
        .ingest(inbound(&key, ChannelKind::Direct, "A", "the unsettled ask"))
        .await
        .expect("the message ingests");
    let conv = receipt.conversation_id;
    await_ledger(&store, conv, "the deaf streaming tail", |blocks| {
        blocks.iter().any(|b| b.block_type == "streaming")
    })
    .await;

    let failure = assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect_err("the unsettled stream must fail the erasure loudly");
    assert!(
        matches!(failure, CoreError::ErasureUnsettled { conversation_id } if conversation_id == conv),
        "the failure names the unsettled conversation; got {failure:?}"
    );

    // Nothing was deleted, nothing was nulled.
    assert!(
        store
            .find_conversation(conv)
            .await
            .expect("the conversation table reads")
            .is_some(),
        "the conversation stays"
    );
    assert_eq!(stored_principals(&store).await, vec!["A"]);
    let blocks = store.list_blocks(conv).await.expect("the ledger reads");
    assert!(
        blocks
            .iter()
            .any(|b| b.fields.get("text") == Some(&json!("the unsettled ask"))),
        "the message text stays stored until an erasure can complete"
    );

    // The retry contract of the loud failure: the interrupt above tore the
    // stream's binding down and swept its tail, and the timed-out
    // observation was dropped with the failure — so a second call finds no
    // observed stream and no stored tail, decides from stored state, and
    // completes without burning the bound again.
    let before = std::time::Instant::now();
    let outcome = assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("the retried erasure completes from stored state");
    assert_eq!(
        outcome,
        ErasureOutcome::Erased {
            deleted_conversations: vec![conv],
        }
    );
    assert!(
        before.elapsed() < std::time::Duration::from_secs(2),
        "the retry pays no second settle wait"
    );
    assert!(
        store
            .find_conversation(conv)
            .await
            .expect("the conversation table reads")
            .is_none(),
        "the retried erasure removes the direct conversation entirely"
    );
    assert_eq!(stored_principals(&store).await, Vec::<String>::new());
}

/// A streaming tail left by a gone runtime — a crash's residue: no
/// stream-closed signal can ever arrive for it, so the settle decides from
/// stored state — the interrupt's sweep and status append — and the erasure
/// completes promptly instead of burning the bound and failing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crash_left_streaming_tail_settles_from_stored_state() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let key = channel("dm-leftover");

    // The first runtime writes a tail it never closes, then is abandoned —
    // the state a crash leaves in a durable store.
    let receipt = {
        let first = Assistant::start(
            store.clone(),
            Arc::new(EventBus::new()),
            support::registry_of(deaf_provider()),
            Arc::new(ToolRegistry::new()),
            support::binding(),
            support::SYSTEM_PROMPT.into(),
            assistant_core::ProtectionConfig::default(),
        )
        .await
        .expect("the first assembly starts");
        let receipt = first
            .ingest(inbound(&key, ChannelKind::Direct, "A", "the abandoned ask"))
            .await
            .expect("the message ingests");
        await_ledger(
            &store,
            receipt.conversation_id,
            "the abandoned tail",
            |blocks| blocks.iter().any(|b| b.block_type == "streaming"),
        )
        .await;
        receipt
    };
    let conv = receipt.conversation_id;

    // The restarted process: a fresh assembly and bus over the same store.
    // Its observer never saw the stream open, and nothing will close it.
    let second = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        support::registry_of(deaf_provider()),
        Arc::new(ToolRegistry::new()),
        support::binding(),
        support::SYSTEM_PROMPT.into(),
        assistant_core::ProtectionConfig::default(),
    )
    .await
    .expect("the second assembly starts");

    let before = std::time::Instant::now();
    let outcome = second
        .erase_principal(receipt.principal_id)
        .await
        .expect("the leftover tail settles from stored state");
    assert_eq!(
        outcome,
        ErasureOutcome::Erased {
            deleted_conversations: vec![conv],
        }
    );
    assert!(
        before.elapsed() < std::time::Duration::from_secs(2),
        "the settle needed no close signal and stayed well under the bound"
    );
    assert!(
        store
            .find_conversation(conv)
            .await
            .expect("the conversation table reads")
            .is_none(),
        "the direct conversation is removed entirely"
    );
    assert_eq!(stored_principals(&store).await, Vec::<String>::new());
}
