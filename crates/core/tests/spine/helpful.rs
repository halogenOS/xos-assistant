//! The helpful answering mode (unit 14, silence re-keyed by unit 22): an
//! unaddressed group message summons a turn through the same debt spine an
//! addressed one takes, a turn the model ends with no text delivers
//! nothing and spends no answer-window slot, a rate-limited member's
//! message opens no turn at all, and a question absorbed into a running
//! turn reaches the model with a member's intervening answer beside it.

use agent_ledger::Store;
use agent_ledger::providers::{MessageContent, MessageRole};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::schema::store_config;
use assistant_core::tools::wiki;
use assistant_core::{AnsweringMode, ChannelKind};
use serde_json::json;

use crate::support::{
    self, Fixture, SILENT_CUE, ToolScript, carries, first_answer_to, inbound_unaddressed,
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

/// AC2, AC4 and AC5 (unit 22): a turn the model ends with no text commits
/// a real empty answer block, delivers nothing, introduces nobody,
/// projects to the next turn as the model's own empty message, and spends
/// no answer-window slot — while the spoken answer behind it spends
/// exactly one, so the limiter still limits.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_silent_turn_speaks_nothing_and_spends_no_window_slot() {
    // One principal answer per window: the whole test rides on which turns
    // consume it.
    let fixture = helpful_fixture(support::budgets(Some((1, 600)), None)).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-silent").await;

    // The first message draws the scripted silence: the turn runs, ends
    // with no text, the framework commits the empty answer block, and
    // nothing is delivered.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("members talking among themselves {SILENT_CUE}"),
        ),
    )
    .await;
    let conversation = receipt.conversation_id;
    let blocks = support::settle(&fixture.store, conversation, "the silent turn", 4).await;
    assert_eq!(
        blocks[3].fields["content"],
        json!(""),
        "the framework's committed record of the silent turn is the empty \
         answer block"
    );

    // The second message is answered: the silent turn spent no slot, so
    // the one-answer budget still admits this debt — and the answer carries
    // the line, because the silent turn introduced nobody. Arriving first
    // on the edge, it also proves the silent turn delivered nothing: the
    // unbounded reply channel preserves order.
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "a real question"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("a real question"),
        "the first delivered reply is the second turn's: the silent turn \
         delivered nothing and introduced nobody"
    );
    support::settle(&fixture.store, conversation, "the answered turn", 6).await;

    // AC5: the answered turn's request carries the empty turn as the
    // model's own empty assistant message — the projection is a pure
    // delegate, so the model reads its own silence back.
    {
        let seen = fixture.script.seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "two turns ran");
        assert!(
            seen[1].iter().any(|message| carries(message, SILENT_CUE)),
            "the first question still projects to the second turn"
        );
        assert!(
            seen[1].iter().any(|message| {
                message.role == MessageRole::Assistant
                    && matches!(&message.content, MessageContent::Text(text) if text.is_empty())
            }),
            "the empty turn projects as the model's own empty message"
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
        "the spoken answer spent the slot; the silent one did not"
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

/// AC7 at the spine: the typing cue lights only once real text flows. A
/// turn that says nothing raises no cue at all, and the spoken turn
/// behind it raises exactly one begin/stop pair — proven by the ordered
/// composing channel: its first update is the spoken turn's begin.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_cue_stays_dark_for_a_silent_turn_and_lights_for_a_spoken_one() {
    let fixture = helpful_fixture(assistant_core::ProtectionConfig::default()).await;
    let mut composing = fixture.assistant.composing(support::ADAPTER);
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-cue").await;

    // The silent turn runs to its committed empty answer first, so every
    // cue it could have raised would sit ahead of the spoken turn's.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("nothing to add here {SILENT_CUE}"),
        ),
    )
    .await;
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the silent turn",
        4,
    )
    .await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "a spoken question"),
    )
    .await;
    recv_reply(&mut replies).await;

    let begun = tokio::time::timeout(support::DEADLINE, composing.recv())
        .await
        .expect("the spoken turn's cue arrives before the deadline")
        .expect("the composing edge outlives the test");
    assert_eq!(
        begun.channel, room,
        "the first composing update is the spoken turn's: the silent turn \
         raised no cue"
    );
    assert_eq!(begun.state, assistant_core::ComposingState::Composing);
    let stopped = tokio::time::timeout(support::DEADLINE, composing.recv())
        .await
        .expect("the stop arrives before the deadline")
        .expect("the composing edge outlives the test");
    assert_eq!(stopped.channel, room);
    assert_eq!(stopped.state, assistant_core::ComposingState::Stopped);
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

/// AC3's boundary: only a wholly empty answer is swallowed — an ordinary
/// answer about silence itself is real text and delivers whole, so the
/// empty-answer check cannot widen into swallowing prose.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_answer_with_real_text_is_never_swallowed() {
    let fixture = helpful_fixture(assistant_core::ProtectionConfig::default()).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-prose").await;

    let text = "when do you stay silent instead of answering?";
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", text),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text,
        first_answer_to(text),
        "real prose delivers whole; only the empty answer yields nothing"
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
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
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
            web_search: None,
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
