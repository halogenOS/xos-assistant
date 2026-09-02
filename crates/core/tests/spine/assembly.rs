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
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
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
            web_search: None,
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
            retention: assistant_core::RetentionConfig::disabled(),
            started_at: std::time::Instant::now(),
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
            web_search: None,
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
        .retire_stale_channels()
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
        .retire_stale_channels()
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

/// The walk retires a channel whose stored MODEL is stale, prompt unchanged —
/// the operator's 2026-08-29 finding: a conversation keeps the binding it was
/// created with, so a configured swap reached only new channels while old ones
/// kept talking, and billing, through the previous model. The successor must
/// carry the configured binding, which is also what the runtime-facts tool
/// truthfully reports.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_swapped_model_moves_a_served_group_onto_it() {
    let db = support::TempDb::new("stale-model");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");

    let first = support::start_assistant_on(store.clone(), None).await;
    let room = support::authorized_group(&first.assistant, "room-stale-model").await;
    let before = support::ingest_recorded(
        &first.assistant,
        support::inbound(&room, ChannelKind::Group, "member-1", "hello there"),
    )
    .await;

    // The same store, the same prompt, a different configured model: the
    // restart a model swap produces.
    let mut swapped = support::assembly_config();
    swapped.binding.model = "script-model-2".into();
    swapped.binding.model_display_name = "Script Model Two".into();
    let (provider, script) = support::scripted_provider(None);
    let restarted = support::start_assistant_config(
        store.clone(),
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        swapped,
    )
    .await;

    let retired = restarted
        .assistant
        .retire_stale_channels()
        .await
        .expect("the retirement reads the ledger");
    assert_eq!(retired, 1, "the one channel on the old model retires");

    let after = support::ingest_recorded(
        &restarted.assistant,
        support::inbound(&room, ChannelKind::Group, "member-1", "still here?"),
    )
    .await;
    assert_ne!(
        after.conversation_id, before.conversation_id,
        "the next message lands in the successor conversation"
    );
    let successor = store
        .find_conversation(after.conversation_id)
        .await
        .expect("the successor reads")
        .expect("the successor exists");
    assert_eq!(
        successor.model.external_id, "script-model-2",
        "the successor runs on the configured model, which is what the \
         dispatch sends and the runtime tool reports"
    );

    // Idempotent: nothing changed, nothing retires.
    let again = restarted
        .assistant
        .retire_stale_channels()
        .await
        .expect("the retirement reads the ledger");
    assert_eq!(again, 0, "an unchanged binding retires nothing");
}

/// Rules the group never changed are not announced again because the
/// assistant moved to a new conversation.
///
/// The acknowledgment fires on a delta — text that differs from the newest
/// note the assistant holds. Those notes live in the conversation, so a
/// channel that starts a fresh one would find every rule new and say so to
/// the whole group, on every prompt edit. The group did not change anything
/// and should not be told that it did.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_moved_channel_does_not_re_announce_rules_it_already_read() {
    use assistant_core::{DeliveryItem, Observation, ObserveOutcome, ObservedFact};

    /// One group-kind observation on a channel — the shape every group fact
    /// travels in.
    fn observed(key: &assistant_core::ChannelKey, fact: ObservedFact) -> Observation {
        Observation {
            channel: key.clone(),
            channel_kind: ChannelKind::Group,
            fact,
        }
    }

    let db = support::TempDb::new("moved-rules");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");

    let first = support::start_assistant_on(store.clone(), None).await;
    let room = support::authorized_group(&first.assistant, "room-moved-rules").await;
    let announced = first
        .assistant
        .observe(observed(
            &room,
            ObservedFact::PinnedAnnouncement("Rules:\n1. Be kind.".into()),
        ))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        support::observed_item(&announced),
        Some(&DeliveryItem::Acknowledgment(
            support::scripted_acknowledgment("1. Be kind.")
        )),
        "rules the assistant has not read before are acknowledged"
    );

    // The restart a prompt edit produces, which moves the channel.
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
    assert_eq!(
        restarted
            .assistant
            .retire_stale_channels()
            .await
            .expect("the retirement reads the ledger"),
        1,
        "the channel moves"
    );

    let again = restarted
        .assistant
        .observe(observed(
            &room,
            ObservedFact::PinnedAnnouncement("Rules:\n1. Be kind.".into()),
        ))
        .await
        .expect("the re-observed pin is judged");
    assert_eq!(
        again,
        ObserveOutcome::Observed { deliver: None },
        "the same rules in the new conversation say nothing: what the channel \
         had read travelled with it"
    );
}
