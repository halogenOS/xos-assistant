//! The standing lookup at the core's edges (unit 29, AC7): an ordinary
//! member's question makes the scripted model call the tool, admission
//! through the conversation's recorded palette admits it at member
//! standing, and the recorded result is the pinned answer about the person
//! who spoke — while the same call in a conversation that is not a group
//! records the pinned group-only refusal instead.
//!
//! What only the assembled core can prove is here: the unconditional
//! registration, the palette entry, the reach at member standing and the
//! non-group refusal through the whole path. The bound, the vocabulary
//! mapping, the freshness, the erasure outcome and every fixed string are
//! pinned beside the resolution itself, in the tool's own module.

use agent_ledger::{Block, Store};
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::standing;
use assistant_core::{Authority, ChannelKind, InboundMessage, ProtectionConfig};
use serde_json::json;

use crate::support::{
    self, ScriptHandle, ToolScript, channel, field, inbound, inbound_as, settle_shape,
    tool_scripted_provider, with_username,
};

/// The stored palette names of the conversation's newest palette block.
fn palette_names(blocks: &[Block]) -> Vec<String> {
    let block = blocks
        .iter()
        .rev()
        .find(|block| block.block_type == "tool_palette")
        .expect("the conversation records a palette");
    serde_json::from_str(&field(block, "tools")).expect("the stored list parses")
}

/// One assembled fixture whose scripted model calls the standing lookup
/// once, for the given handle, and then closes with its answer.
async fn looking_up(handle: &str) -> support::Fixture {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: standing::NAME.into(),
            input: json!({ standing::PARAMETER_HANDLE: handle }).to_string(),
            narration: None,
        },
        None,
    );
    support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        provider,
        script,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await
}

/// One resting group message from a person at the given standing, under the
/// given public handle — the message this lookup answers from.
fn spoke(
    channel: &assistant_core::ChannelKey,
    sender: &str,
    handle: &str,
    standing: Authority,
) -> InboundMessage {
    let mut message = with_username(
        inbound_as(
            channel,
            ChannelKind::Group,
            sender,
            standing,
            "a resting line",
        ),
        handle,
    );
    message.addressed = false;
    message
}

/// An ordinary member asks about someone who spoke as an administrator: the
/// scripted model calls the tool, the recorded palette admits it at member
/// standing, and the result is the pinned affirmative answer naming the
/// stored handle — from a call that asked in a different case, with an at
/// sign the stored form does not carry.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_reaches_the_lookup_in_a_group_and_reads_the_stored_standing() {
    let fixture = looking_up("@ADA").await;
    let room = support::authorized_group(&fixture.assistant, "room-standing").await;
    support::ingest_recorded(
        &fixture.assistant,
        spoke(&room, "ada-1", "Ada", Authority::Admin),
    )
    .await;
    // "B" is an ordinary member: the turn's provenance reads member, which
    // is what the tool is admitted at.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "B", "is @Ada an administrator?"),
    )
    .await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the standing turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert!(
        palette_names(&blocks).contains(&standing::NAME.to_owned()),
        "the creation palette names the tool, so admission can admit it"
    );
    assert_eq!(field(&blocks[4], "name"), standing::NAME);
    assert_eq!(
        field(&blocks[5], "content"),
        standing::administrator_answer("Ada"),
        "the result is the pinned answer, naming the handle as it is stored"
    );
}

/// The same question about an ordinary member answers the false form, which
/// names nobody — the pin that the affirmative answer is not what the tool
/// simply always says.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_who_holds_no_standing_answers_the_false_form() {
    let fixture = looking_up("bee").await;
    let room = support::authorized_group(&fixture.assistant, "room-standing-member").await;
    support::ingest_recorded(
        &fixture.assistant,
        spoke(&room, "bee-1", "bee", Authority::Member),
    )
    .await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "B", "is @bee an administrator?"),
    )
    .await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the standing turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "tool_call",
            "tool_result",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[5], "content"),
        standing::NOT_AN_ADMINISTRATOR_ANSWER
    );
}

/// Outside a group the lookup declines: a direct conversation's sender is
/// recorded at member standing whoever they are, so the tool refuses with
/// its pinned copy instead of stating something false about the person.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_conversation_outside_a_group_records_the_group_only_refusal() {
    let fixture = looking_up("ada").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-standing"),
            ChannelKind::Direct,
            "42",
            "are you talking to an administrator?",
        ),
    )
    .await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the declined lookup",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[4], "error"),
        standing::group_only_refusal(),
        "the direct conversation records the pinned group-only refusal"
    );
}

/// The registration is unconditional: a conversation created by an
/// assembly configured with no moderation handle, no search key and an
/// empty embedder tool set still records the lookup in its palette, which
/// is what lets the shipped conduct prose teach it without a predicate.
/// The prose itself is pinned in the documentation suite, where the shipped
/// prompt files are read.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_conversation_records_the_lookup_in_its_palette() {
    let fixture = support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        ScriptHandle::fresh(),
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-standing-palette").await;
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
        palette_names(&blocks).contains(&standing::NAME.to_owned()),
        "the palette of a conversation created under no capability carries the lookup"
    );
}
