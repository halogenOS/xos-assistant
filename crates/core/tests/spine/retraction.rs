//! The message retraction (unit T4, AC1-AC5 and AC7): an administrator's
//! reply deletion command on one of the assistant's own messages records
//! one retraction, answers the chat's own directive, and forks the retracted
//! answer out of what the model reads.
//!
//! Every assertion about what the model reads runs the production fold —
//! the same function the runtime's request assembly calls — or reads the
//! request the scripted provider actually received. The ledger assertions
//! read the CHANNEL's conversation, never a remembered id: the fork moves
//! the channel, and a test that kept the old id would assert over a
//! conversation nobody is served from any more.

use agent_ledger::providers::{Message, MessageContent, MessageRole, blocks_to_messages};
use agent_ledger::store::domain_run;
use agent_ledger::{Block, FromBlock, LeafKind, Store};
use assistant_core::commands::{COMPACT_COMMAND, COMPACT_DONE};
use assistant_core::delivery::{RETRACTION_KIND, Retraction};
use assistant_core::kind::AssistantKind;
use assistant_core::mirror::DELETION_COMMAND;
use assistant_core::schema::DOMAIN;
use assistant_core::{
    Authority, ChannelKey, ChannelKind, DeliveryItem, InboundMessage, IngestOutcome, ReplyKind,
    ReplyTarget,
};

use crate::support::{
    self, inbound, inbound_as, recv_reply, with_command, with_origin, with_reply,
};

/// One reply carrying the moderation bot's deletion command, aimed at one of
/// the assistant's own messages — the shape this unit's whole capability
/// hangs on. Left addressed exactly as the adapter delivers it: every reply
/// to the assistant arrives addressed, and the silence is the command
/// stamp's doing.
fn retraction_command(
    room: &ChannelKey,
    sender: &str,
    standing: Authority,
    origin: &str,
    target: &str,
) -> InboundMessage {
    with_origin(
        with_command(
            with_reply(
                inbound_as(room, ChannelKind::Group, sender, standing, DELETION_COMMAND),
                ReplyTarget::AssistantMessage {
                    origin: Some(target.into()),
                },
            ),
            DELETION_COMMAND,
        ),
        origin,
    )
}

/// The conversation a channel maps to right now, read raw — the fact the
/// fork changes, so nothing here infers it from a later ingestion.
async fn mapped_conversation(store: &Store, key: &ChannelKey) -> i64 {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    domain_run(&store.tx(), DOMAIN, move |conn| {
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
    .expect("the channel is mapped")
}

/// Every retraction the whole store holds, whichever conversation carries
/// it: the idempotence assertion is about the FACT, and the fork moves the
/// facts between conversations.
async fn stored_retractions(store: &Store) -> Vec<Option<String>> {
    domain_run(&store.tx(), DOMAIN, |conn| {
        let rows = conn
            .prepare(&format!(
                "SELECT {} FROM {} ORDER BY block_id",
                assistant_core::delivery::COLUMN_RETRACTED_DELIVERY,
                assistant_core::delivery::RETRACTION_TABLE
            ))?
            .query_map([], |row| row.get::<_, Option<String>>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the retraction table reads")
}

/// One projected message rendered to its whole text, in either content mode.
fn rendered(message: &Message) -> String {
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

/// Everything one conversation's projection puts in front of the model, as
/// one string — what a "reads nothing of it" assertion reads.
async fn projected_whole(store: &Store, conversation_id: i64) -> String {
    let blocks = store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads");
    blocks_to_messages::<AssistantKind>(&blocks)
        .iter()
        .map(rendered)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The whole of the newest request the provider was handed — every message
/// of it, which is the model's own view at that turn.
fn newest_request(script: &support::ScriptHandle) -> String {
    let requests = script.seen.lock().unwrap();
    requests
        .last()
        .expect("a turn's request was recorded")
        .iter()
        .map(rendered)
        .collect::<Vec<_>>()
        .join("\n")
}

/// One answered question with its delivery recorded: the shape every case
/// below retracts. Answers the answer's text and the origins the send is
/// recorded under.
async fn answered_and_delivered(
    fixture: &support::Fixture,
    replies: &mut tokio::sync::mpsc::UnboundedReceiver<assistant_core::Outbound>,
    room: &ChannelKey,
    ask: &str,
    origin: &str,
    delivered: &[&str],
) -> String {
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(inbound(room, ChannelKind::Group, "42", ask), origin),
    )
    .await;
    let answer = recv_reply(replies).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    support::report_delivery(&fixture.assistant, answer.delivery, delivered).await;
    answer.text
}

/// Ingest one deletion command and answer what it delivered.
async fn retract(fixture: &support::Fixture, message: InboundMessage) -> Option<DeliveryItem> {
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

/// AC1: an administrator's reply deletion command on one of the assistant's
/// own messages names every message of that delivery and appends exactly one
/// retraction fact — and a repeat asks for the same messages again while
/// appending no second fact.
///
/// The reply points at the SECOND chunk on purpose: the retraction is keyed
/// on the delivery, so any chunk of an answer names the whole answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_administrators_reply_retracts_the_whole_delivery_once() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-retract-once").await;
    answered_and_delivered(
        &fixture,
        &mut replies,
        &room,
        "where did the setting move?",
        "org-asked",
        &["31", "32"],
    )
    .await;

    let delivered = retract(
        &fixture,
        retraction_command(&room, "root-ext", Authority::Admin, "org-del-1", "32"),
    )
    .await;
    assert_eq!(
        delivered,
        Some(DeliveryItem::Retraction {
            origins: vec!["31".to_owned(), "32".to_owned()],
        }),
        "every message of the delivery is named, from a reply to any one of them"
    );
    assert_eq!(
        stored_retractions(&fixture.store).await,
        vec![Some("31".to_owned())],
        "one retraction, keyed on the send's own delivery key"
    );

    // The repeat, aimed at the FIRST chunk this time: the same ask, so the
    // same messages go out again and the recorded fact stays one.
    let repeated = retract(
        &fixture,
        retraction_command(&room, "root-ext", Authority::Admin, "org-del-2", "31"),
    )
    .await;
    assert_eq!(
        repeated,
        Some(DeliveryItem::Retraction {
            origins: vec!["31".to_owned(), "32".to_owned()],
        }),
        "the repeat re-issues the ask: the first attempt may have failed"
    );
    assert_eq!(
        stored_retractions(&fixture.store).await,
        vec![Some("31".to_owned())],
        "asking twice is one fact, so no second retraction is appended"
    );
}

/// AC2: a non-administrator's deletion command on one of the assistant's own
/// messages does nothing anywhere. Nothing is retracted, nothing is
/// recorded, the channel keeps its session — and, being a reply to the
/// assistant, it is still answered exactly as it was before this unit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_members_deletion_command_retracts_nothing_and_still_draws_an_answer() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-retract-member").await;
    let answer = answered_and_delivered(
        &fixture,
        &mut replies,
        &room,
        "where did the setting move?",
        "org-asked",
        &["31"],
    )
    .await;
    let served = mapped_conversation(&fixture.store, &room).await;

    let delivered = retract(
        &fixture,
        retraction_command(&room, "peer-ext", Authority::Member, "org-del-1", "31"),
    )
    .await;
    assert_eq!(delivered, None, "a member's command directs nothing");
    assert!(
        stored_retractions(&fixture.store).await.is_empty(),
        "a member's command records no retraction"
    );
    assert_eq!(
        mapped_conversation(&fixture.store, &room).await,
        served,
        "and nothing forked: the channel keeps the session it had"
    );
    assert!(
        projected_whole(&fixture.store, served).await.contains(
            answer
                .lines()
                .next_back()
                .expect("the answer carries a line")
        ),
        "the answer stands in the model's view, untouched"
    );

    let answered = recv_reply(&mut replies).await;
    assert_eq!(
        answered.kind,
        ReplyKind::Answer,
        "a member's reply to the assistant still summons a turn, as it always did"
    );
}

/// AC3: past the fork, the model reads neither the retracted answer nor any
/// quote of it — asserted through the production fold on the session the
/// channel is actually served from, and again on the request the provider is
/// handed for the very next turn.
///
/// The deletion command replies to the assistant, so the ingestion lands a
/// quote of the retracted answer ahead of the command row. That quote is the
/// one this unit's strip set exists for: left behind, it would resolve the
/// retracted words under an administrator's message.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn the_retracted_answer_leaves_the_model_view_with_every_quote_of_it() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-retract-view").await;
    let answer = answered_and_delivered(
        &fixture,
        &mut replies,
        &room,
        "where did the setting move?",
        "org-asked",
        &["31"],
    )
    .await;
    let words = answer
        .lines()
        .next_back()
        .expect("the answer carries a line")
        .to_owned();
    let source = mapped_conversation(&fixture.store, &room).await;
    assert!(
        projected_whole(&fixture.store, source)
            .await
            .contains(&words),
        "non-vacuity: the answer is in the model's view before the command"
    );

    // A member's own reply quotes her answer, which is the second half of
    // the strip set: a quote block stores a span into the block it quotes,
    // so left behind it would resolve the retracted words under this
    // member's message. The reply carries the fixture's quiet cue, so the
    // turn it summons writes no text of its own — an answer echoing her
    // earlier words would make the assertion below unreadable.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            with_reply(
                inbound(&room, ChannelKind::Group, "43", support::SILENT_CUE),
                ReplyTarget::AssistantMessage {
                    origin: Some("31".into()),
                },
            ),
            "org-quoting",
        ),
    )
    .await;
    let quote_kind = agent_ledger::agency::Quote::KINDS[0];
    support::await_ledger(
        &fixture.store,
        source,
        "the quoting reply's turn",
        |blocks| {
            blocks.iter().any(|block| block.block_type == quote_kind)
                && blocks
                    .last()
                    .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;
    assert!(
        projected_whole(&fixture.store, source)
            .await
            .matches(words.as_str())
            .count()
            >= 2,
        "non-vacuity: the model reads her words twice, once through the quote"
    );

    retract(
        &fixture,
        retraction_command(&room, "root-ext", Authority::Admin, "org-del-1", "31"),
    )
    .await;

    let served = mapped_conversation(&fixture.store, &room).await;
    assert_ne!(served, source, "the channel is served from the fork");
    let view = projected_whole(&fixture.store, served).await;
    assert!(
        !view.contains(&words),
        "the retracted answer is out of the model's view: {view}"
    );
    assert!(
        !fixture
            .store
            .list_blocks(served)
            .await
            .expect("the ledger reads")
            .iter()
            .any(|block| block.block_type == quote_kind),
        "and no quote of it rides forward either"
    );
    assert!(
        fixture
            .store
            .list_blocks(served)
            .await
            .expect("the ledger reads")
            .iter()
            .any(|block| block.block_type == RETRACTION_KIND),
        "the lawful record rides into the fork with the command row"
    );

    // The next turn's own request is the model's view, taken from the wire
    // side, not from the fold.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "42", "and on the tablet?"),
            "org-after",
        ),
    )
    .await;
    let next = recv_reply(&mut replies).await;
    assert_eq!(next.kind, ReplyKind::Answer);
    assert!(
        !newest_request(&fixture.script).contains(&words),
        "the model's next request carries no trace of the retracted answer"
    );
}

/// AC4: a deletion command arriving while a turn is in flight settles that
/// turn before the swap, exactly as a compaction does — the answer being
/// written is cut short — and the retracted target still forks away.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_retraction_settles_an_in_flight_turn_before_it_forks() {
    let hold = support::TurnHold::new();
    let fixture = support::start_assistant(Some(std::sync::Arc::clone(&hold))).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-retract-held").await;

    // The first turn is released, so its answer stands and records its
    // delivery; the second is left open, which is the turn the command
    // arrives on top of.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(
                &room,
                ChannelKind::Group,
                "42",
                "where did the setting move?",
            ),
            "org-asked",
        ),
    )
    .await;
    hold.started().await;
    hold.release();
    let answer = recv_reply(&mut replies).await;
    let words = answer
        .text
        .lines()
        .next_back()
        .expect("the answer carries a line")
        .to_owned();
    support::report_delivery(&fixture.assistant, answer.delivery, &["31"]).await;

    let source = mapped_conversation(&fixture.store, &room).await;
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "43", "and on the tablet?"),
            "org-second",
        ),
    )
    .await;
    hold.started().await;
    support::await_ledger(&fixture.store, source, "the streaming tail", |blocks| {
        blocks.iter().any(|block| block.block_type == "streaming")
    })
    .await;

    let mut events = fixture.bus.subscribe();
    let delivered = retract(
        &fixture,
        retraction_command(&room, "root-ext", Authority::Admin, "org-del-1", "31"),
    )
    .await;
    assert_eq!(
        delivered,
        Some(DeliveryItem::Retraction {
            origins: vec!["31".to_owned()],
        }),
        "the retraction completes over a settled source"
    );

    let mut interrupted = false;
    while let Ok(event) = events.try_recv() {
        if matches!(
            event,
            agent_ledger::CoreEvent::InterruptRequested { conversation_id }
                if conversation_id == source
        ) {
            interrupted = true;
        }
    }
    assert!(
        interrupted,
        "the in-flight turn was interrupted before the history was copied"
    );

    let served = mapped_conversation(&fixture.store, &room).await;
    assert_ne!(served, source, "the retracted target still forked away");
    assert!(
        !projected_whole(&fixture.store, served)
            .await
            .contains(&words),
        "and the retracted answer is out of the fork's view"
    );
    assert!(
        !fixture
            .store
            .list_blocks(served)
            .await
            .expect("the ledger reads")
            .iter()
            .any(|block| block.block_type.starts_with("streaming")),
        "the settled turn's streaming tail rides into nothing"
    );
    hold.release();
}

/// AC5: a deletion command naming an answer that sits below a compaction
/// boundary runs the shipped lineage scrub. The answer survives there only
/// as digest prose, so the whole chain is rebuilt from a clone of the
/// ancestor without it, and the block itself goes to the collector.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn a_retraction_below_a_compaction_boundary_scrubs_the_digest() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-retract-compacted").await;
    answered_and_delivered(
        &fixture,
        &mut replies,
        &room,
        "where did the setting move?",
        "org-asked",
        &["31"],
    )
    .await;
    let source = mapped_conversation(&fixture.store, &room).await;
    let answer_block = fixture
        .store
        .list_blocks(source)
        .await
        .expect("the ledger reads")
        .into_iter()
        .rev()
        .find(|block| block.role == Some(agent_ledger::Role::Assistant))
        .expect("the answer is stored")
        .id;

    // Enough traffic behind the answer that the ledger splits with the
    // answer below the cut.
    for index in 0..12 {
        support::ingest_recorded(
            &fixture.assistant,
            with_origin(
                support::inbound_unaddressed(&room, ChannelKind::Group, "43", "chatter"),
                &format!("org-filler-{index}"),
            ),
        )
        .await;
    }
    let compacted = retract(
        &fixture,
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
    let thread = mapped_conversation(&fixture.store, &room).await;
    assert_ne!(thread, source, "the channel is on the compacted thread");
    assert!(
        !fixture
            .store
            .list_blocks(thread)
            .await
            .expect("the ledger reads")
            .iter()
            .any(|block| block.id == answer_block),
        "non-vacuity: the answer is below the boundary and the thread never inherited it"
    );

    let delivered = retract(
        &fixture,
        retraction_command(&room, "root-ext", Authority::Admin, "org-del-1", "31"),
    )
    .await;
    assert_eq!(
        delivered,
        Some(DeliveryItem::Retraction {
            origins: vec!["31".to_owned()],
        }),
        "the delivery resolves across the channel's whole lineage, not one thread of it"
    );

    let scrubbed = mapped_conversation(&fixture.store, &room).await;
    assert_ne!(
        scrubbed, thread,
        "the scrub handed the channel a rebuilt thread"
    );
    assert!(
        fixture
            .store
            .find_block(answer_block)
            .await
            .expect("the block reads")
            .is_none(),
        "the retracted answer went to the collector with the lineage that held it"
    );
    assert!(
        fixture
            .store
            .list_blocks(thread)
            .await
            .expect("the retired thread reads")
            .is_empty(),
        "the lineage the scrub replaced is retired, digest and all"
    );
}

/// AC7: a deletion command that arrives as an EDIT retracts nothing. The
/// reasoning is decision 0180's, unchanged: nothing establishes that an
/// edited command is the standing ask, and a retraction acts on the chat and
/// on the assistant's own reading on the strength of it. The command is
/// still RECOGNIZED, so the row stays silent and takes no turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edited_deletion_command_retracts_nothing() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-retract-edited").await;
    let answer = answered_and_delivered(
        &fixture,
        &mut replies,
        &room,
        "where did the setting move?",
        "org-asked",
        &["31"],
    )
    .await;
    let words = answer
        .lines()
        .next_back()
        .expect("the answer carries a line")
        .to_owned();

    // The original command is an ordinary line the administrator then edits
    // into the deletion token: the store holds the version being revised, so
    // the edit records as a further version of it.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_as(
                &room,
                ChannelKind::Group,
                "root-ext",
                Authority::Admin,
                "hm, second thoughts",
            ),
            "org-edited",
        ),
    )
    .await;
    let served = mapped_conversation(&fixture.store, &room).await;
    let mut edited = retraction_command(&room, "root-ext", Authority::Admin, "org-edit-2", "31");
    edited.revises = Some("org-edited".into());

    let delivered = retract(&fixture, edited).await;
    assert_eq!(delivered, None, "an edited command directs nothing");
    assert!(
        stored_retractions(&fixture.store).await.is_empty(),
        "and it records no retraction"
    );
    assert_eq!(
        mapped_conversation(&fixture.store, &room).await,
        served,
        "nothing forked: the channel keeps the session it had"
    );
    assert!(
        projected_whole(&fixture.store, served)
            .await
            .contains(&words),
        "the answer stands in the model's view"
    );
    let stamped = recorded_stamp(&fixture.store, served).await;
    assert_eq!(
        stamped,
        Some(assistant_core::kind::LimitedBy::Command),
        "the edited command is still the recognized command: silent, and no turn"
    );
}

/// The limiting fact stored on the conversation's newest chat message.
async fn recorded_stamp(
    store: &Store,
    conversation_id: i64,
) -> Option<assistant_core::kind::LimitedBy> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .rev()
        .find_map(|block: &Block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => Some(message.limited),
            _ => None,
        })
        .expect("a chat message is recorded")
}

/// The retraction kind's own shape, read back through its parse: one stored
/// value, and the model shown nothing of it.
#[test]
fn a_retraction_records_its_delivery_and_shows_the_model_nothing() {
    let block = Block {
        id: 1,
        role: None,
        block_type: RETRACTION_KIND.into(),
        created_at: String::new(),
        dispatch_anchor: None,
        fields: Retraction::stored_fields("31"),
    };
    let parsed = Retraction::parse(&block);
    assert_eq!(parsed.delivery.as_deref(), Some("31"));
    assert!(
        blocks_to_messages::<AssistantKind>(std::slice::from_ref(&block)).is_empty(),
        "a ledger of retractions alone projects no message at all"
    );
    assert_eq!(
        blocks_to_messages::<AssistantKind>(&[block])
            .first()
            .map(|message| message.role),
        None::<MessageRole>
    );
}
