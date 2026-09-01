//! The delivery receipt at the core's edges (unit 38, AC2's core half,
//! AC8 and AC9): what one send records, what the model is shown of it
//! (nothing), what a receipt at the tail does to a standing debt (nothing),
//! and what the widened reply-target variant stores (nothing).
//!
//! Every receipt here is reported through the public entry point with the
//! handle the outbound edge handed out, which is the seam the adapter uses.

use std::sync::Arc;

use agent_ledger::agency::ratchet;
use agent_ledger::providers::{MessageRole, blocks_to_messages};
use agent_ledger::store::domain_run;
use agent_ledger::{AgencyCtx, Awaiting, Block, CoreEvent, EventBus, LeafKind, Role, Store};
use assistant_core::delivery::{
    COLUMN_ANSWER_BLOCK, COLUMN_DELIVERY, COLUMN_ORIGIN, DELIVERED_KIND, Delivered,
};
use assistant_core::kind::{AssistantKind, CHAT_MESSAGE_KIND};
use assistant_core::schema::{DOMAIN, store_config};
use assistant_core::{Authority, ChannelKind, DeliveryItem, IngestOutcome, ReplyKind, ReplyTarget};
use serde_json::json;

use crate::support::{
    self, channel, inbound, inbound_unaddressed, recv_reply, with_command, with_reply,
};

/// The receipts one conversation holds, oldest first.
async fn receipts(store: &Store, conversation_id: i64) -> Vec<Delivered> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| block.block_type == DELIVERED_KIND)
        .map(Delivered::parse)
        .collect()
}

/// One conversation's projection, as the model reads it.
async fn projected(store: &Store, conversation_id: i64) -> Vec<(MessageRole, String)> {
    let blocks = store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads");
    blocks_to_messages::<AssistantKind>(&blocks)
        .iter()
        .map(|message| {
            let text = match &message.content {
                agent_ledger::providers::MessageContent::Text(text) => text.clone(),
                agent_ledger::providers::MessageContent::Parts(parts) => parts
                    .iter()
                    .filter_map(|part| match part {
                        agent_ledger::providers::ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            (message.role, text)
        })
        .collect()
}

/// AC2's core half and AC8's projection half: an answer's send records one
/// receipt per platform message, all under the send's first id as the
/// delivery key and all naming the answer's own block — and the model is
/// shown exactly nothing of any of it.
///
/// Two origins for one send is the chunked case as the core meets it: the
/// transport decided the chunks, the core records what reached the chat,
/// and every row names the one block she said it as.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_send_records_a_receipt_per_message_and_shows_the_model_nothing() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-delivery-record");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &key,
            ChannelKind::Direct,
            "42",
            "where did the setting move?",
        ),
    )
    .await;
    let answer = recv_reply(&mut replies).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    let before = projected(&fixture.store, asked.conversation_id).await;

    support::report_delivery(&fixture.assistant, answer.delivery, &["31", "32"]).await;

    let rows = receipts(&fixture.store, asked.conversation_id).await;
    assert_eq!(rows.len(), 2, "one receipt per delivered platform message");
    assert_eq!(
        rows.iter()
            .map(|row| row.origin.clone().expect("a recorded origin"))
            .collect::<Vec<_>>(),
        vec!["31".to_owned(), "32".to_owned()],
        "the receipts land in send order, each naming its own message"
    );
    assert!(
        rows.iter().all(|row| row.delivery.as_deref() == Some("31")),
        "every message of one send carries the send's first id as the key"
    );
    let stored_answer = fixture
        .store
        .list_blocks(asked.conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .rev()
        .find(|block| block.role == Some(Role::Assistant))
        .expect("the answer is stored");
    assert!(
        rows.iter()
            .all(|row| row.answer_block == Some(stored_answer.id)),
        "a reply to any chunk resolves the one block she said it as"
    );

    assert_eq!(
        projected(&fixture.store, asked.conversation_id).await,
        before,
        "the receipts add nothing the model reads: bookkeeping it never meets"
    );
}

/// AC8's crucial case, live: a send whose receipt names no block of
/// hers records its delivery AT THE TAIL, over a question nobody answered
/// — and the owing walk reads straight through it, so the next resting
/// message still carries the debt standing behind it.
///
/// This is the shape the read-through membership exists for. An opaque
/// receipt here would answer the walk a settled tail and bury the standing
/// question behind it.
///
/// The standing question is a failed turn's, which since unit 49 puts
/// nothing on the channel at all, and the receipt is the privacy command's
/// deterministic answer — fixed prose the ledger never stores, so its
/// receipt carries no answer block, exactly the row this case needs. The
/// command draws no turn and does not re-engage the latched conversation,
/// so the debt it carries forward is the dead turn's own.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_receipt_at_the_tail_buries_no_standing_debt() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let mut events = fixture.bus.subscribe();
    let key = support::authorized_group(&fixture.assistant, "room-delivery-debt").await;

    fixture.script.fail_next_turns(1);
    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "the failing ask"),
    )
    .await;
    support::await_failure_latch(&mut events, asked.conversation_id).await;
    assert!(
        matches!(
            replies.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ),
        "the failed turn says nothing, so the debt stands unanswered and \
         unmentioned"
    );

    let outcome = fixture
        .assistant
        .ingest(with_command(
            inbound_unaddressed(&key, ChannelKind::Group, "42", "/privacy"),
            "/privacy",
        ))
        .await
        .expect("the command ingests");
    let IngestOutcome::Recorded {
        receipt,
        deliver: Some(DeliveryItem::CommandAnswer(_)),
        ..
    } = outcome
    else {
        panic!("non-vacuity: the command answers deterministically: {outcome:?}");
    };
    support::report_delivery(&fixture.assistant, receipt.delivery(), &["77"]).await;
    let tail = fixture
        .store
        .latest_block(asked.conversation_id)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(
        tail.block_type, DELIVERED_KIND,
        "non-vacuity: the command answer's receipt really is the tail"
    );
    let rows = receipts(&fixture.store, asked.conversation_id).await;
    assert_eq!(
        rows.iter().map(|row| row.answer_block).collect::<Vec<_>>(),
        vec![None],
        "the command answer is not stored, so its receipt names no block \
         of hers"
    );

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "43", "still wondering"),
    )
    .await;
    let recorded = fixture
        .store
        .list_blocks(asked.conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .rfind(|block| block.block_type == CHAT_MESSAGE_KIND)
        .expect("the resting message is recorded");
    assert_eq!(
        recorded.fields["addressed"],
        json!(false),
        "the resting message summoned nothing of its own"
    );
    assert_eq!(
        recorded.fields["answer_due"],
        json!(true),
        "the debt reaches it THROUGH the receipt: the walk reads past it"
    );
}

/// AC8's frontier half: with a receipt as the stored tail over an
/// unanswered message, the framework's own drive still reports the turn
/// owed and awaiting the model — the join notice's frontier pin, mirrored
/// for the receipt because it is transparent for the same reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_receipt_over_an_unanswered_message_leaves_the_turn_owed() {
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
                assistant_core::kind::RecordedOrigin::default(),
                None,
                "2026-08-30T00:00:00+00:00",
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
            DELIVERED_KIND,
            Delivered::stored_fields("91", "91", None),
            None,
        )
        .await
        .expect("the receipt appends on top");

    let tail = store
        .latest_block(conversation)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(
        tail.block_type, DELIVERED_KIND,
        "non-vacuity: the receipt really is the tail the frontier reads"
    );

    let ctx: AgencyCtx<CoreEvent> = AgencyCtx {
        conversation_id: conversation,
        store,
        bus: Arc::new(EventBus::new()),
    };
    let outcome = ratchet::drive::<AssistantKind, CoreEvent>(&ctx)
        .await
        .expect("the drive runs")
        .outcome()
        .expect("the conversation still exists");
    assert!(
        outcome.owes_turn,
        "the turn is still owed through the transparent receipt"
    );
    assert_eq!(outcome.awaiting, Some(Awaiting::Model));
}

/// AC9: the widened reply-target variant changes no storage. A reply to
/// one of the assistant's own messages names her message in the core's
/// vocabulary and stores exactly what it stored before: the
/// reply-to-assistant flag, and a NULL reply target — the column erasure's
/// naming pass reads as member-message references stays what its own
/// documentation says it is.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_to_her_stores_the_flag_and_a_null_target() {
    let fixture = support::start_assistant(None).await;
    let key = channel("dm-delivery-column");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            inbound(&key, ChannelKind::Direct, "42", "and on the tablet?"),
            ReplyTarget::AssistantMessage {
                origin: Some("31".into()),
            },
        ),
    )
    .await;

    let stored: Vec<(Option<String>, Option<bool>)> =
        domain_run(&fixture.store.tx(), DOMAIN, |conn| {
            let rows = conn
                .prepare(&format!(
                    "SELECT reply_target, reply_to_assistant \
                     FROM {} ORDER BY block_id",
                    assistant_core::kind::CHAT_MESSAGE_TABLE
                ))?
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("the chat table reads");
    assert_eq!(
        stored,
        vec![(None, Some(true))],
        "the flag stands and the reply-target column holds nothing: her \
         origin is consumed during ingestion and never stored"
    );
    assert!(
        receipt.conversation_id > 0,
        "non-vacuity: the reply really was recorded"
    );
}

/// The receipt is invisible to every reader that walks the ledger by kind:
/// a Delivered block among an ordinary exchange contributes no projected
/// message at all, whatever else the fold shows.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_receipt_contributes_no_projected_message() {
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
            None,
            DELIVERED_KIND,
            Delivered::stored_fields("12", "11", Some(7)),
            None,
        )
        .await
        .expect("the receipt appends");

    let blocks: Vec<Block> = store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    assert!(
        blocks
            .iter()
            .any(|block| block.block_type == DELIVERED_KIND),
        "non-vacuity: the receipt is on the ledger"
    );
    let stored = blocks
        .iter()
        .find(|block| block.block_type == DELIVERED_KIND)
        .expect("the receipt is on the ledger");
    assert_eq!(stored.fields[COLUMN_ORIGIN], json!("12"));
    assert_eq!(stored.fields[COLUMN_DELIVERY], json!("11"));
    assert_eq!(stored.fields[COLUMN_ANSWER_BLOCK], json!(7));
    assert!(
        blocks_to_messages::<AssistantKind>(&blocks).is_empty(),
        "a ledger of receipts alone projects no message at all"
    );
}
