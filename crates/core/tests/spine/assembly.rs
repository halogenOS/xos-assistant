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
            binding,
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
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
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            protection: assistant_core::ProtectionConfig::default(),
            operators: support::operator_config(),
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
