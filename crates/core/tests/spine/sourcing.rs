//! The grounded-answer discipline (unit 16): the literal addressed fact is
//! stored beside the untouched summons, the model's miss sentinel is
//! routed by the mechanism — silence for an unaddressed asker, the fixed
//! don't-know line for a literally addressed one — and a turn whose
//! lookup did not answer closes with the miss sentinel instead of prose
//! from the lookup's unhelpful result.

use agent_ledger::Store;
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::schema::store_config;
use assistant_core::tools::wiki;
use assistant_core::{AnsweringMode, ChannelKind, DONT_KNOW_ANSWER, MISS_SENTINEL};
use serde_json::json;

use crate::support::{
    self, Fixture, MISS_CUE, ToolScript, carries, carries_tool_result, first_answer_to, inbound,
    inbound_unaddressed, recv_reply,
};

/// A running helpful-mode assistant over a fresh store, under the default
/// budgets.
async fn helpful_fixture() -> Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    support::start_assistant_answering(
        store,
        None,
        assistant_core::ProtectionConfig::default(),
        AnsweringMode::Helpful,
    )
    .await
}

/// The message rows of one loaded ledger, in order.
fn message_rows(blocks: &[agent_ledger::Block]) -> Vec<&agent_ledger::Block> {
    blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect()
}

/// AC2: the literal fact is stored beside the summons without disturbing
/// it — an unaddressed helpful-mode message records summons=true with
/// literal=false, an addressed one records both true, through the
/// production ingest path.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_ingest_path_stores_the_literal_fact_beside_the_summons() {
    let fixture = helpful_fixture().await;
    let room = support::authorized_group(&fixture.assistant, "room-literal").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "an unaddressed remark"),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "42", "an addressed ask"),
    )
    .await;

    let blocks = support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "both messages",
        |blocks| message_rows(blocks).len() == 2,
    )
    .await;
    let rows = message_rows(&blocks);
    assert_eq!(
        rows[0].fields["addressed"],
        json!(true),
        "helpful answering summons the unaddressed message"
    );
    assert_eq!(
        rows[0].fields["literal_addressed"],
        json!(false),
        "the literal fact stays the adapter's own"
    );
    assert_eq!(rows[1].fields["addressed"], json!(true));
    assert_eq!(
        rows[1].fields["literal_addressed"],
        json!(true),
        "an addressed message records both facts true"
    );
}

/// AC3: a turn summoned by an UNADDRESSED message whose model answer is
/// the miss sentinel delivers nothing, prepends no disclosure and
/// introduces nobody — the next spoken answer carries the
/// first-interaction line, and it is the first thing the channel hears.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unaddressed_miss_delivers_nothing_and_introduces_nobody() {
    let fixture = helpful_fixture().await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-miss-silent").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "42",
            &format!("does anyone know this? {MISS_CUE}"),
        ),
    )
    .await;
    let conversation = receipt.conversation_id;
    let blocks = support::settle(&fixture.store, conversation, "the missed turn", 4).await;
    assert_eq!(
        blocks[3].fields["content"],
        json!(MISS_SENTINEL),
        "the stored answer is the raw sentinel: no delivery rewrote it"
    );

    // The follow-up question's answer arrives first on the edge and still
    // carries the line: the miss delivered nothing and introduced nobody.
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "42", "an answerable question"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("an answerable question"),
        "the first delivered reply is the second turn's: the unaddressed \
         miss stayed silent"
    );
}

/// AC4: the mirrored case over the same miss sentinel — a turn summoned by
/// an ADDRESSED message delivers exactly the fixed don't-know line,
/// carrying the first asker's disclosure line and no trained-knowledge
/// tail, and the stored answer block holds the delivered text.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_addressed_miss_delivers_the_fixed_dont_know_line() {
    let fixture = helpful_fixture().await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-miss-spoken").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            &format!("what about this? {MISS_CUE}"),
        ),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, room);
    assert_eq!(
        reply.text,
        support::disclosed(DONT_KNOW_ANSWER),
        "the addressed miss delivers the fixed line, introduced for the \
         new person and nothing else"
    );

    let blocks = support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the rewritten answer",
        |blocks| {
            blocks.last().is_some_and(|block| {
                block.block_type == "text"
                    && block.fields["content"] == json!(support::disclosed(DONT_KNOW_ANSWER))
            })
        },
    )
    .await;
    assert_eq!(blocks.len(), 4, "one turn: prompt, palette, ask, answer");
}

/// AC4's addressed-mode half: under addressed answering every summoning
/// message is literally addressed, so the miss always delivers the line.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn in_addressed_mode_the_miss_always_delivers_the_line() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_answering(
        store,
        None,
        assistant_core::ProtectionConfig::default(),
        AnsweringMode::Addressed,
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-miss-addressed").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            &format!("assistant, is this supported? {MISS_CUE}"),
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(DONT_KNOW_ANSWER),
        "the addressed-mode miss speaks the fixed line"
    );
}

/// AC6's sufficiency half: a turn whose in-turn lookup did not answer the
/// question — the scripted wiki call runs against an unreachable endpoint,
/// so its recorded result contains no claim at all — closes with the miss
/// sentinel and delivers the fixed don't-know line, never prose from the
/// unhelpful result. The lookup provably precedes the answer: the closing
/// request carries the tool result. The answered half of AC6 — a lookup
/// that does answer is answered from it — is the tools suite's standing
/// pins.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lookup_that_does_not_answer_closes_as_a_miss_not_as_prose() {
    let (provider, script) = support::tool_scripted_provider(
        ToolScript {
            tool: wiki::NAME.into(),
            input: r#"{"page":"Home"}"#.into(),
            narration: None,
        },
        None,
    );
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_config(
        store,
        provider,
        script,
        support::production_toolset(),
        assistant_core::AssemblyConfig {
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
        },
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-miss-lookup").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            &format!("how do I use the sandboxed feature? {MISS_CUE}"),
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(DONT_KNOW_ANSWER),
        "the unanswering lookup ends in the fixed line, not in prose \
         assembled from the result or from memory"
    );

    let seen = fixture.script.seen.lock().unwrap();
    assert!(seen.len() >= 2, "the turn ran its lookup round");
    let closing = seen.last().expect("the closing request was seen");
    assert!(
        closing.iter().any(carries_tool_result),
        "the lookup preceded the answer: the closing request carries its result"
    );
    assert!(
        closing.iter().any(|message| carries(message, MISS_CUE)),
        "the closing request still projects the question the lookup failed"
    );
}
