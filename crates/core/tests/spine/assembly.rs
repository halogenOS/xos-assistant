//! The assembly's own contract: the wiring facts `Assistant::start` refuses
//! to run without, and the ingestion refusals that keep the mapping honest.

use std::sync::Arc;

use agent_ledger::{CoreEvent, EventBus, Store};
use assistant_core::schema::store_config;
use assistant_core::{Assistant, ChannelKind, CoreError};

use crate::support;

/// A binding whose vendor names no registered module is refused at start.
/// The vendor is what resolves a conversation to its provider module, so a
/// mismatch accepted here would strand every conversation with no error
/// anywhere later.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_binding_with_an_unregistered_vendor_is_refused_at_start() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let (provider, _script) = support::scripted_provider(None);
    let mut binding = support::binding();
    binding.vendor = "someone-else".into();

    let Err(refused) = Assistant::start(
        store,
        bus,
        support::registry_of(provider),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            reasoning: assistant_core::ReasoningLevel::Low,
            binding,
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    else {
        panic!("the unresolvable binding must be refused at start");
    };
    match refused {
        CoreError::UnknownVendor { vendor } => assert_eq!(vendor, "someone-else"),
        other => panic!("the refusal names the unresolvable vendor; got {other:?}"),
    }
}

/// A store opened without the assistant's configuration is refused at start:
/// its content-table list lacks the message kind's table, and every append
/// would fail later and further from the cause.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_opened_without_the_configuration_is_refused_at_start() {
    let store = Store::in_memory().expect("a bare in-memory store opens");
    let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
    let (provider, _script) = support::scripted_provider(None);

    let Err(refused) = Assistant::start(
        store,
        bus,
        support::registry_of(provider),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await
    else {
        panic!("the unconfigured store must be refused at start");
    };
    assert!(
        matches!(refused, CoreError::MissingContentTable { .. }),
        "the refusal names the missing table; got {refused:?}"
    );
}

/// A message claiming a different channel kind than the mapping recorded is
/// refused: the kind decides what erasure does with the channel key, so a
/// silent disagreement would corrupt the privacy contract.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_disagreeing_with_the_mapped_channel_kind_is_refused() {
    let fixture = support::start_assistant(None).await;
    let key = support::authorized_group(&fixture.assistant, "room-kind").await;
    support::ingest_recorded(
        &fixture.assistant,
        support::inbound(&key, ChannelKind::Group, "42", "the mapping message"),
    )
    .await;

    let refused = fixture
        .assistant
        .ingest(support::inbound(
            &key,
            ChannelKind::Direct,
            "42",
            "the disagreeing message",
        ))
        .await;
    match refused {
        Err(CoreError::ChannelKindMismatch { stored, claimed }) => {
            assert_eq!(stored, ChannelKind::Group);
            assert_eq!(claimed, ChannelKind::Direct);
        }
        other => panic!("the disagreeing message must be refused; got {other:?}"),
    }
}

/// A channel whose conversation recorded an older system prompt starts a new
/// conversation, so an edited prompt reaches a group already being served.
///
/// The shape a deployment actually meets: the assistant serves a group, the
/// operator edits the prose, the process restarts on the same store. Before
/// this, the conversation kept the wording it was created with and the edit
/// reached only groups that appeared afterwards — the deployment changed and
/// the assistant did not.
///
/// Asserted through the ledger rather than the mapping table, because what
/// matters is where the next message lands: a different conversation under the
/// new prompt, and the group still admitted. A retirement that also lost the
/// operator's invitation would read as a group the assistant was never added
/// to, and it would withdraw from it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_edited_prompt_moves_a_served_group_to_a_new_conversation() {
    let db = support::TempDb::new("stale-prompt");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");

    let first = support::start_assistant_on(store.clone(), None).await;
    let room = support::authorized_group(&first.assistant, "room-stale-prompt").await;
    let before = support::ingest_recorded(
        &first.assistant,
        support::inbound(&room, ChannelKind::Group, "member-1", "hello there"),
    )
    .await;

    // The same store, a different prompt: the restart a prompt edit produces.
    let mut edited = support::assembly_config();
    edited.system_prompt = "a different system prompt entirely".into();
    let (provider, script) = support::scripted_provider(None);
    let restarted = support::start_assistant_config(
        store.clone(),
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        edited,
    )
    .await;

    let retired = restarted
        .assistant
        .retire_stale_prompts()
        .await
        .expect("the retirement reads the ledger");
    assert_eq!(retired, 1, "the one channel serving the old prompt retires");

    let after = support::ingest_recorded(
        &restarted.assistant,
        support::inbound(&room, ChannelKind::Group, "member-1", "still here?"),
    )
    .await;
    assert_ne!(
        after.conversation_id, before.conversation_id,
        "the next message lands in a new conversation, which is where the \
         current prompt was recorded"
    );

    // Idempotent: the restart that changes nothing retires nothing, and the
    // group stays where it was put.
    let again = restarted
        .assistant
        .retire_stale_prompts()
        .await
        .expect("the retirement reads the ledger");
    assert_eq!(again, 0, "an unchanged prompt retires nothing");
    let settled = support::ingest_recorded(
        &restarted.assistant,
        support::inbound(&room, ChannelKind::Group, "member-1", "and again"),
    )
    .await;
    assert_eq!(
        settled.conversation_id, after.conversation_id,
        "a second message stays in the conversation the first one opened"
    );
}
