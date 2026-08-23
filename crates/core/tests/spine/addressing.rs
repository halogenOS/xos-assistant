//! The addressing seam at the core's edge: the write-time answer-due stamp,
//! the resting unaddressed message, the debt that propagates instead of
//! cancelling, the recorded system prompt, and the failure notice with the
//! addressed re-engagement.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use agent_ledger::{CoreEvent, EventBus, Store};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::schema::store_config;
use assistant_core::{Assistant, ChannelKind, FAILURE_NOTICE, ReplyKind};
use serde_json::json;

use crate::support::{
    self, answer_to, await_ledger, carries, channel, inbound, inbound_unaddressed, recv_reply,
};

/// An unaddressed group message is recorded with both addressing columns
/// false, draws no turn, and rests — then joins the next turn's projected
/// context when an addressed message arrives.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unaddressed_message_rests_and_joins_the_next_context() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = support::authorized_group(&fixture.assistant, "room-rest").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "42", "a resting remark"),
    )
    .await;
    let conv = receipt.conversation_id;

    // Recorded with the stamped facts, resting.
    let blocks = await_ledger(
        &fixture.store,
        conv,
        "the recorded resting message",
        |blocks| blocks.iter().any(|b| b.block_type == CHAT_MESSAGE_KIND),
    )
    .await;
    let message = blocks
        .iter()
        .find(|b| b.block_type == CHAT_MESSAGE_KIND)
        .expect("the message block exists");
    assert_eq!(message.fields["addressed"], json!(false));
    assert_eq!(message.fields["answer_due"], json!(false));
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        0,
        "an unaddressed message draws no turn"
    );

    // The addressed message that follows is answered, with the resting
    // The addressed message that follows is answered, with the resting
    // remark in its projected context.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "43", "the question"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, answer_to("a resting remark\n\nthe question"));
    assert_eq!(reply.kind, ReplyKind::Answer);
    assert_eq!(fixture.script.turns.load(Ordering::SeqCst), 1);
    let requests = fixture.script.seen.lock().unwrap();
    assert!(
        requests[0].iter().any(|m| carries(m, "a resting remark")),
        "the resting message joined the projected context: {requests:?}"
    );
}

/// The system prompt seam: recorded at conversation creation through the
/// framework's system-prompt kind, and present in the first turn's projected
/// request.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_system_prompt_is_recorded_and_projected() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-prompt");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "hello"),
    )
    .await;
    recv_reply(&mut replies).await;

    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    assert_eq!(
        blocks[0].block_type, "system_prompt",
        "the prompt is the conversation's first block"
    );
    assert_eq!(
        blocks[0].fields["content"],
        json!(support::SYSTEM_PROMPT),
        "the recorded prompt is the assembly's own"
    );

    let requests = fixture.script.seen.lock().unwrap();
    let first = &requests[0][0];
    assert_eq!(
        first.role,
        agent_ledger::providers::MessageRole::System,
        "the projected request opens with the system voice"
    );
    assert!(
        carries(first, support::SYSTEM_PROMPT),
        "the projected system message carries the prompt text"
    );
}

/// The failure notice and the addressed re-engagement, end to end on the
/// core's edges: a scripted stream error latches the conversation and yields
/// exactly one notice, marked as a notice; the next addressed message
/// unlatches, and the conversation answers again.
///
/// The notice is at most once by construction — it derives from a lossy bus
/// event — so this test pins the one-notice case, not a redelivery.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_turn_yields_one_notice_and_the_next_addressed_message_reengages() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-failure");

    fixture.script.fail_next_turns(1);
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the failing ask"),
    )
    .await;

    let notice = recv_reply(&mut replies).await;
    assert_eq!(notice.kind, ReplyKind::Notice, "the notice is marked");
    assert_eq!(notice.text, FAILURE_NOTICE);
    assert_eq!(notice.channel, key);
    assert!(
        matches!(
            replies.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ),
        "one failed turn, one notice"
    );

    // The next addressed message re-engages the latched conversation.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "asking again"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.kind, ReplyKind::Answer);
    assert_eq!(reply.text, answer_to("the failing ask\n\nasking again"));
}

/// The write-time stamp, pinned deterministically: with the answer withheld,
/// an unaddressed message written onto the heels of an addressed one reads
/// the tail's unanswered debt and carries it forward — recorded unaddressed,
/// stamped answer-due — instead of cancelling it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_stamp_propagates_an_unanswered_debt_at_the_write() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    .expect("the assembly starts");
    let key = support::authorized_group(&assistant, "room-debt").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Group, "42", "the addressed ask"),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "43", "an aside on its heels"),
    )
    .await;

    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let messages: Vec<_> = blocks
        .iter()
        .filter(|b| b.block_type == CHAT_MESSAGE_KIND)
        .collect();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].fields["addressed"], json!(true));
    assert_eq!(messages[0].fields["answer_due"], json!(true));
    assert_eq!(messages[1].fields["addressed"], json!(false));
    assert_eq!(
        messages[1].fields["answer_due"],
        json!(true),
        "the unanswered debt propagates onto the newest block instead of \
         being cancelled by it"
    );
}

/// The same shape with the debtor erased: erasure cancels an unanswered debt
/// for both readers of the stamp — the awaiting hook already refuses to
/// summon a turn for an erased message, so the tail read must not propagate
/// that dead debt onto an unaddressed message either, or the debt the hook
/// cancelled would resurface durably on a block that never asked anything.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_erased_tail_propagates_no_debt() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::new()),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    .expect("the assembly starts");
    let key = support::authorized_group(&assistant, "room-erased-debt").await;

    // The answer stays withheld, so the addressed ask remains the tail,
    // stamped answer-due — then its sender is erased. The group conversation
    // survives the erasure; only the personal columns are nulled.
    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&key, ChannelKind::Group, "42", "the never-answered ask"),
    )
    .await;
    let outcome = assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("the erasure succeeds");
    assert_eq!(
        outcome,
        assistant_core::ErasureOutcome::Erased {
            deleted_conversations: vec![],
        },
        "a group conversation is kept, its sender's prose nulled"
    );
    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "43", "an aside after the erasure"),
    )
    .await;
    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks.last().expect("the aside is the newest block");
    assert_eq!(aside.fields["addressed"], json!(false));
    assert_eq!(
        aside.fields["answer_due"],
        json!(false),
        "an erased tail's cancelled debt does not resurface on the aside"
    );
    assert!(
        aside.fields.get("debt_authority").is_none(),
        "an erased tail owes nothing, so no debt authority is carried either"
    );
}

/// The same shape with the debt already paid: an unaddressed message behind
/// an ANSWERED conversation reads the assistant's answer as the tail and
/// carries no debt — the propagation is of unanswered debt only.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_paid_debt_does_not_propagate() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = support::authorized_group(&fixture.assistant, "room-paid").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "the answered ask"),
    )
    .await;
    recv_reply(&mut replies).await;
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the answered turn",
        4,
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "43", "an aside after the answer"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks.last().expect("the aside is the newest block");
    assert_eq!(aside.fields["answer_due"], json!(false));
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        1,
        "the aside draws no second turn"
    );
}

/// The addressed unlatch is unconditional and idempotent: every addressed
/// ingestion emits the unlatch intent on the bus, an unaddressed one never
/// does.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_addressed_messages_emit_the_unlatch_intent() {
    let fixture = support::start_assistant(None).await;
    let mut events = fixture.bus.subscribe();
    let key = support::authorized_group(&fixture.assistant, "room-unlatch").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "42", "resting"),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "addressed"),
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
        "one addressed message, one unlatch intent; the unaddressed one \
         emitted none"
    );
}
