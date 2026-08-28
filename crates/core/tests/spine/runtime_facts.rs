//! The runtime-facts unit at the core's edges (unit 32, AC3 and AC5): an
//! ordinary member's question makes the scripted model call the tool, the
//! recorded result carries the four fact lines exactly as the rendering
//! writes them, and the model id in them is the one the answering
//! conversation runs on — never the fixture's default, never anything the
//! model remembers, and never a configured id the conversation was not
//! created under.
//!
//! The byte-exact rendering, the coarse uptime, the anchor read once and
//! the ignored input are pinned beside the rendering itself, in the tool's
//! own module; what only the assembled core can prove is here: the
//! registration, the palette entry, and the conversation's own model
//! reaching the result across a configuration change.
//!
//! The blocks are found by kind, never by position: this unit claims
//! nothing about where a block sits in the ledger, and the framework may
//! append transparent kinds of its own between them.

use std::time::Duration;

use agent_ledger::Block;
use agent_ledger::Store;
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::runtime;
use assistant_core::{ChannelKind, ProtectionConfig};

use crate::support::{self, ToolScript, field, inbound, tool_scripted_provider};

/// The model id this suite assembles with: distinct from the fixture
/// default, so a result restating the default would fail here.
const CONFIGURED_MODEL: &str = "vendor/model-under-test-9";

/// The model a channel's conversation was created under before the
/// configuration changed — what the framework still dispatches that
/// conversation's turns on.
const MODEL_BEFORE_THE_SWAP: &str = "vendor/model-before-the-swap";

/// The stored palette names of the conversation's newest palette block.
fn palette_names(blocks: &[Block]) -> Vec<String> {
    let block = blocks
        .iter()
        .rev()
        .find(|block| block.block_type == "tool_palette")
        .expect("the conversation records a palette");
    serde_json::from_str(&field(block, "tools")).expect("the stored list parses")
}

/// The conversation's one block of the given kind.
fn only(blocks: &[Block], kind: &str) -> Block {
    let mut found = blocks.iter().filter(|block| block.block_type == kind);
    let block = found.next().unwrap_or_else(|| panic!("one {kind} block"));
    assert!(found.next().is_none(), "exactly one {kind} block");
    block.clone()
}

/// An ordinary member asks what the assistant runs on: the scripted model
/// calls the tool, admission through the recorded palette admits the call
/// at member authority, and the recorded result is the four fact lines
/// over the model this conversation was created on — which, for a
/// conversation opened under the running configuration, is that
/// configured id.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_reaches_the_tool_and_reads_the_model_the_turn_runs_on() {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: runtime::NAME.into(),
            input: "{}".into(),
            narration: None,
        },
        None,
    );
    let mut config = support::assembly_config();
    config.binding.model = CONFIGURED_MODEL.into();
    let fixture = support::start_assistant_config(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        config,
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-runtime-facts").await;

    // "A" is an ordinary member, not the configured operator: the tool is
    // reached at member authority, through the palette, never by calling
    // the handler.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "which model are you running on?",
        ),
    )
    .await;
    let blocks = support::viewed_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the runtime-facts turn",
        |blocks| {
            blocks.iter().any(|block| block.block_type == "tool_result")
                && blocks
                    .last()
                    .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;

    assert!(
        palette_names(&blocks).contains(&runtime::NAME.to_owned()),
        "the creation palette names the tool, so admission can admit it"
    );
    assert_eq!(field(&only(&blocks, "tool_call"), "name"), runtime::NAME);
    // A process seconds old renders a zero uptime, which makes the whole
    // result byte-exact end to end — the model id from the conversation's
    // own record, the version and revision compiled in.
    let result = field(&only(&blocks, "tool_result"), "content");
    assert_eq!(
        result,
        runtime::fact_lines(
            CONFIGURED_MODEL,
            runtime::VERSION,
            runtime::REVISION,
            Duration::ZERO
        ),
        "the result states the model this conversation runs on, not the fixture default"
    );
    assert!(
        !result.contains("script-model"),
        "the fixture's default model id reaches no result"
    );
}

/// The defect this unit's fix pass closed, pinned: the model the tool
/// states is the model the answering conversation runs on, not the
/// configured one.
///
/// A conversation's model row is written once, at creation, from the id
/// configured then, a fork inherits it, and nothing in this core or in the
/// framework ever updates it — while the dispatch reads exactly that
/// stored row. So a configuration change moves no conversation already
/// open: the channel served before it keeps its model, and a tool stating
/// the configured id would name a model the wire does not carry, with the
/// authority of a process fact. That is the incident the whole unit was
/// written to end, so the fact is read per call from the conversation the
/// call belongs to.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_stated_model_is_the_one_the_conversation_runs_on() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    // Before the swap: the channel's conversation is created under the id
    // configured then, and every later turn of it dispatches on that id.
    let mut before = support::assembly_config();
    before.binding.model = MODEL_BEFORE_THE_SWAP.into();
    let (silent, quiet) = support::scripted_provider(None);
    let first =
        support::start_assistant_config(store.clone(), silent, quiet, ToolSet::new(), before).await;
    let room = support::authorized_group(&first.assistant, "room-model-swap").await;
    let opened = support::ingest_recorded(
        &first.assistant,
        inbound(&room, ChannelKind::Group, "A", "hello"),
    )
    .await;
    support::viewed_ledger(
        &first.store,
        opened.conversation_id,
        "the turn before the swap",
        |blocks| {
            blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;

    // The swap: another model configured, the process restarted on the same
    // store. The prompt is untouched, so the channel keeps its conversation
    // — and with it the model the wire carries.
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: runtime::NAME.into(),
            input: "{}".into(),
            narration: None,
        },
        None,
    );
    let mut after = support::assembly_config();
    after.binding.model = CONFIGURED_MODEL.into();
    let restarted =
        support::start_assistant_config(store.clone(), provider, script, ToolSet::new(), after)
            .await;
    let receipt = support::ingest_recorded(
        &restarted.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "which model are you running on?",
        ),
    )
    .await;
    assert_eq!(
        receipt.conversation_id, opened.conversation_id,
        "the restart keeps the channel's conversation, which is the premise"
    );
    let blocks = support::viewed_ledger(
        &restarted.store,
        receipt.conversation_id,
        "the runtime-facts turn",
        |blocks| blocks.iter().any(|block| block.block_type == "tool_result"),
    )
    .await;

    let running = restarted
        .store
        .find_conversation(receipt.conversation_id)
        .await
        .expect("the conversation reads")
        .expect("the conversation exists")
        .model
        .external_id;
    assert_eq!(
        running, MODEL_BEFORE_THE_SWAP,
        "the stored model is what the framework dispatches on"
    );
    let result = field(&only(&blocks, "tool_result"), "content");
    assert!(
        result.starts_with(&format!("model: {running}\n")),
        "the tool states the model the turn actually ran on: {result}"
    );
}

/// The teaching that routes the question rides every conversation's
/// recorded prompt: the composition is what the assembly records, and the
/// tool it names is in the same conversation's palette — an instruction
/// and a capability that cannot drift apart.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_recorded_prompt_teaches_the_tool_the_palette_carries() {
    let fixture = support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-runtime-teaching").await;
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
            .contains(&format!("call the {} tool", runtime::NAME)),
        "the recorded prompt routes identity questions to the tool"
    );
    assert!(
        palette_names(&blocks).contains(&runtime::NAME.to_owned()),
        "the same conversation's palette carries what the prompt teaches"
    );
}
