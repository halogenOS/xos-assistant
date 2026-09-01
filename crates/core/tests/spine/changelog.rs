//! The harness-changelog unit at the core's edges (unit 47, AC1): the tool
//! registers unconditionally with the assembly, every new conversation's
//! recorded choice names it, and an ordinary member's question makes the scripted
//! model's call admit at member authority — the same shape unit 32 pinned
//! the runtime facts' registration with.
//!
//! The result is the embedded value verbatim, pinned end to end; in a
//! build that passes no changelog — the suites' ordinary state — that
//! value is the stated absence whole. The present-value
//! path is pinned over the resolve function in the tool's own module,
//! where the value can be injected — the compile-time environment itself
//! cannot vary under test.
//!
//! The blocks are found by kind, never by position: this unit claims
//! nothing about where a block sits in the ledger.

use agent_ledger::Store;
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::changelog;
use assistant_core::{ChannelKind, ProtectionConfig};

use crate::support::{
    self, ToolScript, field, inbound, only, tool_choice_names, tool_scripted_provider,
};

/// An ordinary member asks what changed in the assistant: the scripted
/// model calls the tool, the recorded choice resolves it and admission admits the
/// call at member authority, and the recorded result is the embedded value
/// whole — in this build, the stated absence byte for byte.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_reaches_the_tool_and_reads_the_embedded_value() {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: changelog::NAME.into(),
            input: "{}".into(),
            narration: None,
        },
        None,
    );
    let fixture = support::start_assistant_config(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        support::assembly_config(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-harness-changelog").await;

    // "A" is an ordinary member, not the configured operator: the tool is
    // reached at member authority, through the recorded choice, never by calling
    // the handler.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "what changed in you lately?",
        ),
    )
    .await;
    let blocks = support::viewed_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the harness-changelog turn",
        |blocks| {
            blocks.iter().any(|block| block.block_type == "tool_result")
                && blocks
                    .last()
                    .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;

    assert!(
        tool_choice_names(&blocks).contains(&changelog::NAME.to_owned()),
        "the creation choice names the tool, so the call can resolve"
    );
    assert_eq!(field(&only(&blocks, "tool_call"), "name"), changelog::NAME);
    let result = field(&only(&blocks, "tool_result"), "content");
    assert_eq!(
        result,
        changelog::CHANGELOG,
        "the result is the embedded value, verbatim and whole"
    );
    // A build that passed no changelog answers the stated absence — the
    // honest register pinned end to end, not a summary the model could
    // have written from memory. Guarded like the module's own resolve
    // pin: a developer build that exports the variable is answering with
    // its value, which the verbatim assertion above already proved.
    if option_env!("ASSISTANT_BUILD_CHANGELOG").is_none() {
        assert_eq!(changelog::CHANGELOG, changelog::ABSENT_RESULT);
    }
}

/// The teaching that routes the question rides every conversation's
/// recorded prompt: the composition is what the assembly records, and the
/// tool it names is in the same conversation's choice — an instruction
/// and a capability that cannot drift apart.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_recorded_prompt_teaches_the_tool_the_choice_carries() {
    let fixture = support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-changelog-teaching").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "A", "hello"),
    )
    .await;
    let blocks = support::consumer_view(
        &fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the ledger reads"),
    );
    assert!(
        field(&only(&blocks, "system_prompt"), "content")
            .contains(&format!("call the {} tool", changelog::NAME)),
        "the recorded prompt routes change questions to the tool"
    );
    assert!(
        field(&only(&blocks, "system_prompt"), "content").contains(
            "a question about a halogenOS release or about changes in halogenOS belongs \
             to the release lookup, never to it"
        ),
        "the recorded prompt keeps the assistant's changes apart from the OS's"
    );
    assert!(
        tool_choice_names(&blocks).contains(&changelog::NAME.to_owned()),
        "the same conversation's choice carries what the prompt teaches"
    );
}
