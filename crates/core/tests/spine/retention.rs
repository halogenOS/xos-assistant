//! The retention sweep at the core's edges (unit 53, 2026-09-02): a
//! conversation nobody has touched for the configured span is deleted whole,
//! and the assistant carries on around the hole it leaves.
//!
//! Both cases here run the sweep the way production runs it — a process
//! assembled with a span, whose first tick is at its spawn — so what is
//! proven is the shipped path and not a function called by hand. The store
//! is aged between the two processes, because a span is wall-clock days and
//! a test may not wait for them.

use std::time::Duration;

use agent_ledger::Store;
use assistant_core::commands::{COMPACT_COMMAND, COMPACT_DONE};
use assistant_core::mirror::DELETION_COMMAND;
use assistant_core::schema::store_config;
use assistant_core::{
    Authority, ChannelKey, ChannelKind, DeliveryItem, InboundMessage, IngestOutcome, ReplyKind,
    ReplyTarget, RetentionConfig,
};

use crate::support::{
    self, inbound, inbound_as, inbound_unaddressed, recv_reply, with_command, with_origin,
    with_reply,
};

/// The span every process here is assembled with.
const SPAN_DAYS: u32 = 90;

/// How far back the store is aged between the two processes: past the span,
/// in the seconds the ageing seam speaks.
const PAST_THE_SPAN: i64 = 91 * 24 * 60 * 60;

/// How long a case waits for a spawned sweep's first tick to finish its
/// work. Generous against a loaded runner, bounded so a sweep that never
/// runs fails the case instead of hanging it.
const SWEEP_BOUND: Duration = Duration::from_secs(10);

/// A process assembled over the given store WITH the retention span, which
/// is what starts the sweep: its first tick is at the spawn.
async fn process_with_retention(store: Store) -> support::Fixture {
    let (provider, script) = support::scripted_provider(None);
    let mut config = support::assembly_config();
    config.retention = RetentionConfig::of_days(SPAN_DAYS);
    support::start_assistant_config(
        store,
        provider,
        script,
        support::production_toolset(),
        config,
    )
    .await
}

/// The conversations the store still holds.
async fn held(store: &Store) -> Vec<i64> {
    store
        .list_conversations()
        .await
        .expect("the conversation list reads")
        .into_iter()
        .map(|conversation| conversation.id)
        .collect()
}

/// Wait until the sweep has taken the named conversation, failing the case
/// if it never does.
async fn await_swept(store: &Store, conversation_id: i64, what: &str) {
    let deadline = tokio::time::Instant::now() + SWEEP_BOUND;
    while tokio::time::Instant::now() < deadline {
        if !held(store).await.contains(&conversation_id) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{what}: conversation {conversation_id} was never swept");
}

/// The conversation a channel maps to right now, read raw.
async fn mapped_conversation(store: &Store, key: &ChannelKey) -> Option<i64> {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                "SELECT conversation_id FROM channels WHERE adapter = ?1 AND channel = ?2",
                rusqlite::params![adapter, channel],
                |row| row.get::<_, i64>(0),
            )
            .ok())
    })
    .await
    .expect("the mapping reads")
}

/// One answered question with its send recorded, answering the id of the
/// block the answer was stored as.
async fn answered_and_delivered(
    fixture: &support::Fixture,
    replies: &mut tokio::sync::mpsc::UnboundedReceiver<assistant_core::Outbound>,
    room: &ChannelKey,
    ask: &str,
    origin: &str,
    delivered: &str,
) -> i64 {
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(inbound(room, ChannelKind::Group, "42", ask), origin),
    )
    .await;
    let answer = recv_reply(replies).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    support::report_delivery(&fixture.assistant, answer.delivery, &[delivered]).await;
    fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .rev()
        .find(|block| block.role == Some(agent_ledger::Role::Assistant))
        .expect("the answer is stored")
        .id
}

/// One reply carrying the moderation bot's deletion command, aimed at one of
/// the assistant's own messages.
fn retraction_command(room: &ChannelKey, origin: &str, target: &str) -> InboundMessage {
    with_origin(
        with_command(
            with_reply(
                inbound_as(
                    room,
                    ChannelKind::Group,
                    "root-ext",
                    Authority::Admin,
                    DELETION_COMMAND,
                ),
                ReplyTarget::AssistantMessage {
                    origin: Some(target.into()),
                },
            ),
            DELETION_COMMAND,
        ),
        origin,
    )
}

/// Ingest one command and answer what it delivered.
async fn invoke(fixture: &support::Fixture, message: InboundMessage) -> Option<DeliveryItem> {
    match fixture
        .assistant
        .ingest(message)
        .await
        .expect("the command ingests")
    {
        IngestOutcome::Recorded { deliver, .. } => deliver,
        refused => panic!("the command was refused: {refused:?}"),
    }
}

/// AC7: a swept serving channel's next message opens a fresh session and is
/// answered. Deleting a mapped conversation unmaps its channel, and the
/// first-contact path the next message takes is the one that has always been
/// there — the group is served without being admitted again, because the
/// operator's admission is not the members' data and never went.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_swept_serving_channel_answers_the_next_message() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let first = support::start_assistant_on(store.clone(), None).await;
    let room = support::authorized_group(&first.assistant, "room-gone-quiet").await;
    let opened = support::ingest_recorded(
        &first.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            "where did the setting move?",
        ),
    )
    .await;
    support::viewed_ledger(&store, opened.conversation_id, "the first turn", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type == "text")
    })
    .await;
    // The first process stops before the second starts: two live assemblies
    // on one store would race for the same owed turn.
    first.shutdown().await;
    support::age_receipts(&store, PAST_THE_SPAN).await;

    let restarted = process_with_retention(store.clone()).await;
    let mut replies = restarted
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    await_swept(&store, opened.conversation_id, "the quiet channel").await;
    assert_eq!(
        mapped_conversation(&store, &room).await,
        None,
        "the swept conversation took its channel mapping with it"
    );

    let next = support::ingest_recorded(
        &restarted.assistant,
        inbound(&room, ChannelKind::Group, "42", "are you still here?"),
    )
    .await;
    let answer = recv_reply(&mut replies).await;
    assert_eq!(
        answer.kind,
        ReplyKind::Answer,
        "the fresh session answers, with no second admission asked for"
    );
    assert_eq!(
        mapped_conversation(&store, &room).await,
        Some(next.conversation_id),
        "the channel maps to the session that answered"
    );
    // The store reissues conversation ids AND block ids, so a fresh session
    // is proven by what its ledger says and never by any number: this one
    // opens with its own system prompt, the way first contact opens, and
    // holds only the message that opened it.
    let fresh = support::consumer_view(
        &store
            .list_blocks(next.conversation_id)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(
        fresh
            .first()
            .map(|block| block.block_type.as_str())
            .unwrap_or_default(),
        "system_prompt",
        "the fresh session opens the way first contact opens"
    );
    let asked: Vec<String> = fresh
        .iter()
        .filter(|block| block.block_type == assistant_core::kind::CHAT_MESSAGE_KIND)
        .map(|block| support::field(block, "text"))
        .collect();
    assert_eq!(
        asked,
        vec!["are you still here?".to_owned()],
        "and holds nothing the swept conversation held"
    );
}

/// AC5: an expired ancestor of a living thread is swept, the thread serves
/// on, and both walkers behave exactly as they already do over a
/// conversation that no longer reads.
///
/// The thread keeps every block that rode across the cut, because those rows
/// are SHARED with the ancestor and its junction still holds them — so a
/// retraction aimed at an answer above the cut resolves as it did before the
/// ancestor went. Below the cut there is nothing left to resolve: the
/// lineage walk ends at the deleted hop, logging that the ancestry ends
/// there, and the command records itself and retracts nothing, which is what
/// it already does for a message the channel recorded no delivery for.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn an_expired_ancestor_is_swept_and_its_thread_serves_on() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let first = support::start_assistant_on(store.clone(), None).await;
    let mut replies = first
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&first.assistant, "room-compacted-quiet").await;

    // The answer that will sit BELOW the cut, then enough traffic that the
    // ledger splits with room on both sides, then the answer that will ride
    // across it.
    let below_the_cut = answered_and_delivered(
        &first,
        &mut replies,
        &room,
        "an early ask",
        "org-early",
        "11",
    )
    .await;
    for index in 0..12 {
        support::ingest_recorded(
            &first.assistant,
            with_origin(
                inbound_unaddressed(&room, ChannelKind::Group, "43", "chatter"),
                &format!("org-filler-{index}"),
            ),
        )
        .await;
    }
    let above_the_cut =
        answered_and_delivered(&first, &mut replies, &room, "a later ask", "org-late", "22").await;

    let source = mapped_conversation(&store, &room)
        .await
        .expect("the channel is mapped");
    let compacted = invoke(
        &first,
        with_origin(
            with_command(
                inbound_as(
                    &room,
                    ChannelKind::Group,
                    "root-ext",
                    Authority::Moderator,
                    COMPACT_COMMAND,
                ),
                COMPACT_COMMAND,
            ),
            "org-compact",
        ),
    )
    .await;
    assert_eq!(
        compacted,
        Some(DeliveryItem::CommandAnswer(COMPACT_DONE.to_owned())),
        "the compaction ran"
    );
    let thread = mapped_conversation(&store, &room)
        .await
        .expect("the channel is mapped");
    assert_ne!(thread, source, "the channel is on the compacted thread");
    let inherited: Vec<i64> = store
        .list_blocks(thread)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.id)
        .collect();
    assert!(
        !inherited.contains(&below_the_cut),
        "the premise: the early answer stayed in the ancestor"
    );
    assert!(
        inherited.contains(&above_the_cut),
        "the premise: the later answer rode across, shared by id"
    );

    first.shutdown().await;
    // Only the ancestor stops being touched. The thread's own opening was
    // written at the compaction and stays inside the span, which is what
    // makes this the ancestor's expiry and not the channel's.
    support::age_conversation_days(&store, source, 91).await;

    let restarted = process_with_retention(store.clone()).await;
    let mut replies = restarted
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    await_swept(&store, source, "the compacted ancestor").await;

    assert!(
        held(&store).await.contains(&thread),
        "the living thread survives its ancestor"
    );
    assert_eq!(
        mapped_conversation(&store, &room).await,
        Some(thread),
        "and still serves the channel"
    );
    assert!(
        store
            .find_block(below_the_cut)
            .await
            .expect("the block reads")
            .is_none(),
        "the ancestor's own half went to the collector with it"
    );
    assert!(
        store
            .find_block(above_the_cut)
            .await
            .expect("the block reads")
            .is_some(),
        "the shared half stands, because the thread's junction still holds it"
    );

    assert_eq!(
        invoke(&restarted, retraction_command(&room, "org-del-early", "11")).await,
        None,
        "the walk ends at the deleted hop, so the pre-cut delivery resolves to nothing"
    );
    assert_eq!(
        invoke(&restarted, retraction_command(&room, "org-del-late", "22")).await,
        Some(DeliveryItem::Retraction {
            origins: vec!["22".to_owned()],
        }),
        "and a delivery that rode across the cut retracts as it always did"
    );

    let next = support::ingest_recorded(
        &restarted.assistant,
        inbound(&room, ChannelKind::Group, "42", "anything new?"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.kind,
        ReplyKind::Answer,
        "and the channel is answered from the thread that survived"
    );
    assert!(
        held(&store).await.contains(&next.conversation_id),
        "the served conversation stands"
    );
}
