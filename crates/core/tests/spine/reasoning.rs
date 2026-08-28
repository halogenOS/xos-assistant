//! The reasoning-effort pins (decision 0087): every conversation the
//! assembly creates is set to the configured level at its creation, direct
//! and group channels alike, and the level reaches the provider's edge —
//! the request the framework builds over the stored key carries it.

use agent_ledger::Store;
use assistant_core::schema::store_config;
use assistant_core::{ChannelKind, ProtectionConfig, ReasoningLevel};

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
        },
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_created_conversation_carries_the_configured_reasoning_level() {
    let fixture = start_assistant_reasoning(ReasoningLevel::Medium).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
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
