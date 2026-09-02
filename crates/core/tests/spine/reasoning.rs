//! The model's reasoning, on both sides of a turn.
//!
//! The reasoning-effort pins (decision 0087): every conversation the
//! assembly creates is set to the configured level at its creation, direct
//! and group channels alike, and the level reaches the provider's edge —
//! the request the framework builds over the stored key carries it.
//!
//! And the pin for a trace that comes back inside the answer (unit 43,
//! decision 0168): where the answer text carries exactly one closing
//! think-tag, the send delivers the prose behind it alone, while the ledger
//! keeps what the model wrote.

use agent_ledger::{Role, Store};
use assistant_core::schema::store_config;
use assistant_core::{ChannelKind, ProtectionConfig, ReasoningLevel};
use serde_json::json;

use crate::support;
use crate::support::{channel, inbound, recv_reply};

/// Assemble a running assistant configured to a distinctive level — medium,
/// not the deployment default, so the assertions prove the configured value
/// travels rather than a constant the code could have hardwired.
async fn start_assistant_reasoning(level: ReasoningLevel) -> support::Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let (provider, script) = support::scripted_provider(None);
    support::start_assistant_config(
        store,
        provider,
        script,
        support::production_toolset(),
        assistant_core::AssemblyConfig {
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
            binding: support::binding(),
            reasoning: level,
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_created_conversation_carries_the_configured_reasoning_level() {
    let fixture = start_assistant_reasoning(ReasoningLevel::Medium).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let dm = channel("reasoning-dm");
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&dm, ChannelKind::Direct, "sender-1", "a direct question"),
    )
    .await;
    recv_reply(&mut replies).await;

    let group = support::authorized_group(&fixture.assistant, "reasoning-group").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&group, ChannelKind::Group, "sender-2", "a group question"),
    )
    .await;
    recv_reply(&mut replies).await;

    // The stored rows: both conversations carry the configured level as the
    // framework's own key — the exact string its parser reads back.
    let conversations = fixture
        .store
        .list_conversations()
        .await
        .expect("the conversations list");
    assert_eq!(
        conversations.len(),
        2,
        "the direct and the group channel each created one conversation"
    );
    for conversation in &conversations {
        assert_eq!(
            conversation.reasoning.as_deref(),
            Some(ReasoningLevel::Medium.as_key()),
            "a created conversation carries the configured reasoning key"
        );
    }

    // The provider's edge: both answered turns' requests carried the level,
    // resolved by the framework from the stored key — the end-to-end proof
    // that the configuration reaches the wire, not only the row.
    let reasonings = fixture.script.reasonings.lock().unwrap().clone();
    assert_eq!(
        reasonings,
        vec![Some(ReasoningLevel::Medium), Some(ReasoningLevel::Medium)],
        "every turn's provider request carried the configured level"
    );
}

/// AC1 through the whole assistant (unit 43): a model whose reasoning
/// escapes into its answer — one closing think-tag, the trace in front of
/// the prose — is delivered as the prose alone, under the first-interaction
/// line, while the stored block keeps the model's full text. The script
/// derives its answer from the newest projected message, so a question
/// carrying the leak shape scripts an answer carrying it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_leaked_reasoning_trace_never_reaches_the_channel() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let dm = channel("dm-leaked-reasoning");

    let asked = "how do I install it?</think>Install it from the recovery.";
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&dm, ChannelKind::Direct, "42", asked),
    )
    .await;

    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, dm);
    assert_eq!(
        reply.text,
        support::disclosed("Install it from the recovery."),
        "the channel sees the prose behind the one closer, under the line"
    );

    let stored = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .rev()
        .find(|block| block.role == Some(Role::Assistant))
        .expect("the answer is stored");
    assert_eq!(
        stored.fields["content"],
        json!(support::disclosed(&support::answer_to(asked))),
        "the ledger keeps the model's full text under the same line"
    );
}

/// The incident's own shape (unit 43): the live leak arrived in a GROUP.
/// An addressed group question whose scripted answer carries the
/// one-closer shape is delivered to the group as the prose alone, under
/// the first-interaction line — the cut does not depend on the channel
/// kind, and this pin holds it where it actually happened.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_leaked_reasoning_trace_is_cut_in_a_group_too() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let group = support::authorized_group(&fixture.assistant, "leak-group").await;

    let asked = "what broke?</think>The updater; flash the latest build.";
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&group, ChannelKind::Group, "member-7", asked),
    )
    .await;

    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, group);
    assert_eq!(
        reply.text,
        support::disclosed("The updater; flash the latest build."),
        "the group sees the prose behind the one closer, under the line"
    );
}
