//! The helpful answering mode (unit 14): an unaddressed group message
//! summons a turn through the same debt spine an addressed one takes, the
//! model's abstention sentinel delivers nothing and spends no
//! answer-window slot, a rate-limited member's message opens no turn at
//! all, and a question absorbed into a running turn reaches the model with
//! a member's intervening answer beside it.

use agent_ledger::Store;
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::schema::store_config;
use assistant_core::tools::wiki;
use assistant_core::{ABSTENTION_SENTINEL, AnsweringMode, ChannelKind};
use serde_json::json;

use crate::support::{
    self, ABSTAIN_CUE, Fixture, ToolScript, carries, first_answer_to, inbound_unaddressed,
    recv_reply,
};

/// A running helpful-mode assistant over a fresh store, with the given
/// budgets.
async fn helpful_fixture(protection: assistant_core::ProtectionConfig) -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    support::start_assistant_answering(store, None, protection, AnsweringMode::Helpful).await
}

/// The message rows of one loaded ledger, in order.
fn message_rows(blocks: &[agent_ledger::Block]) -> Vec<&agent_ledger::Block> {
    blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect()
}

/// AC2: an unaddressed group question summons a turn — stamped as the
/// summons the debt spine reads — and the answer reaches the chat carrying
/// the first-interaction line for the new person.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unaddressed_group_question_is_answered_with_the_line() {
    let fixture = helpful_fixture(assistant_core::ProtectionConfig::default()).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-helpful").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            "what is the release cadence?",
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, room);
    assert_eq!(
        reply.text,
        first_answer_to("what is the release cadence?"),
        "the unaddressed question is answered, introduced for the new person"
    );

    let blocks = support::settle(&fixture.store, receipt.conversation_id, "the turn", 4).await;
    let rows = message_rows(&blocks);
    assert_eq!(
        rows[0].fields["addressed"],
        json!(true),
        "the stored summons fact"
    );
    assert_eq!(rows[0].fields["answer_due"], json!(true), "the opened debt");
}

/// AC2 and AC4: the abstention sentinel as the whole answer delivers
/// nothing, introduces nobody, stays out of the next turn's projection,
/// and spends no answer-window slot — while the spoken answer behind it
/// spends exactly one, so the limiter still limits.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_abstained_turn_speaks_nothing_and_spends_no_window_slot() {
    // One principal answer per window: the whole test rides on which turns
    // consume it.
    let fixture = helpful_fixture(support::budgets(Some((1, 600)), None)).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-abstain").await;

    // The first message draws the scripted abstention: the turn runs, the
    // sentinel commits as the stored answer, and nothing is delivered.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("members talking among themselves {ABSTAIN_CUE}"),
        ),
    )
    .await;
    let conversation = receipt.conversation_id;
    let blocks = support::settle(&fixture.store, conversation, "the abstained turn", 4).await;
    assert_eq!(
        blocks[3].fields["content"],
        json!(ABSTENTION_SENTINEL),
        "the stored answer is the raw sentinel: no delivery rewrote it"
    );

    // The second message is answered: the abstained turn spent no slot, so
    // the one-answer budget still admits this debt — and the answer carries
    // the line, because the abstention introduced nobody. Arriving first on
    // the edge, it also proves the abstention delivered nothing: the
    // unbounded reply channel preserves order.
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "a real question"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("a real question"),
        "the first delivered reply is the second turn's: the abstention \
         delivered nothing and introduced nobody"
    );
    support::settle(&fixture.store, conversation, "the answered turn", 6).await;

    // The answered turn's request skipped the recognized abstention: the
    // model never reads its own machinery token as prose.
    {
        let seen = fixture.script.seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "two turns ran");
        assert!(
            seen[1].iter().any(|message| carries(message, ABSTAIN_CUE)),
            "the first question still projects to the second turn"
        );
        // The system prompt's own teaching names the sentinel, so the pin
        // scopes to the spoken voices: no assistant or user message carries
        // it — the recognized abstention projects nothing at all.
        assert!(
            !seen[1]
                .iter()
                .filter(|message| message.role != agent_ledger::providers::MessageRole::System)
                .any(|message| carries(message, ABSTENTION_SENTINEL)),
            "the recognized abstention stays out of the projection"
        );
    }

    // The third message meets the spent window: the SPOKEN answer consumed
    // the one slot, so this debt is refused — no turn, no model call — the
    // free quiet of the existing limiter.
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "one question too many"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let rows = message_rows(&blocks);
    assert_eq!(rows.len(), 3, "the third message is recorded");
    assert_eq!(
        rows[2].fields["limited"],
        json!("principal"),
        "the spoken answer spent the slot; the abstained one did not"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        2,
        "the refused debt opened no third turn"
    );
}

/// AC2: a rate-limited member's message opens no turn at all — zero model
/// calls — through the limited stamp the ingestion writes before any turn
/// could exist.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rate_limited_members_message_opens_no_turn() {
    let fixture = helpful_fixture(support::budgets(Some((1, 600)), None)).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-limited").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "the first question"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the first question")
    );
    support::settle(&fixture.store, receipt.conversation_id, "the first turn", 4).await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "the over-budget question"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let rows = message_rows(&blocks);
    assert_eq!(rows[1].fields["limited"], json!("principal"));
    assert_eq!(
        rows[1].fields["answer_due"],
        json!(false),
        "the refused debt owes nothing"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the rate-limited message cost zero model calls"
    );
}

/// AC4: an ordinary answer that carries the sentinel's words as prose is
/// delivered untouched — the sentinel is the whole answer or nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_answer_quoting_the_sentinel_as_prose_is_not_swallowed() {
    let fixture = helpful_fixture(assistant_core::ProtectionConfig::default()).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-prose").await;

    let text = format!("what does {ABSTENTION_SENTINEL} mean?");
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", &text),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text,
        first_answer_to(&text),
        "the sentinel inside prose is prose; the answer delivers whole"
    );
    assert!(
        reply.text.contains(ABSTENTION_SENTINEL),
        "the premise holds: the delivered answer quotes the sentinel's words"
    );
}

/// AC6: a question absorbed into a running turn's tool window reaches the
/// model together with a member's intervening answer — the continuation
/// request carries both, so the model can defer to the member.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_absorbed_question_and_the_intervening_answer_reach_the_model() {
    let hold = support::TurnHold::new();
    let (provider, script) = support::tool_scripted_provider(
        ToolScript {
            tool: wiki::NAME.into(),
            input: r#"{"page":"Home"}"#.into(),
            narration: None,
        },
        Some(hold.clone()),
    );
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_config(
        store,
        provider,
        script,
        support::production_toolset(),
        assistant_core::AssemblyConfig {
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: AnsweringMode::Helpful,
            name: support::NAME.into(),
            disclosure: None,
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-absorbed-helpful").await;

    // The opening question summons the tool turn, held open before its
    // call events.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "A", "where is the wiki page?"),
    )
    .await;
    hold.started().await;

    // Absorbed mid-turn: a second member's question, and a third member's
    // answer to it — both unaddressed, both recorded into the open turn's
    // window.
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "B", "which kernel does it run?"),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "C", "kernel 6.6, see the wiki"),
    )
    .await;
    hold.release();

    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(support::CLOSING_ANSWER),
        "the turn closes over the continuation round"
    );
    support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the closing answer",
        |blocks| {
            blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;

    let seen = fixture.script.seen.lock().unwrap();
    let closing = seen.last().expect("the continuation request was seen");
    assert!(
        closing
            .iter()
            .any(|message| carries(message, "which kernel does it run?")),
        "the absorbed question reaches the model"
    );
    assert!(
        closing
            .iter()
            .any(|message| carries(message, "kernel 6.6, see the wiki")),
        "the member's intervening answer is visible beside it"
    );
}
