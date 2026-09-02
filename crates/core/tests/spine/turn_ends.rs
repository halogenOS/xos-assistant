//! The two turn-ending tools at the core's edges (unit 54): a bare call of
//! either ends the turn — the resolution carries the framework's
//! ends-turn stamp, no continuation round is dispatched, and the channel
//! receives nothing — the stored close is each module's own sentence, both
//! tools ride every created conversation's recorded choice while the
//! compaction fork's empty choice keeps them out of a summary turn, and a
//! park beside a sibling call silences only itself.
//!
//! Every turn scripted here is NARRATION-FREE: the call is made bare,
//! which is the shape the teaching prohibits writing prose ahead of. What
//! prose ahead of a call would do — deliver as its own message — is the
//! standing mechanism's, asserted by the search suite, and this module
//! neither repeats nor contradicts it.

use std::time::{Duration, Instant};

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::TemporaryFork;
use agent_ledger::{
    Block, CoreEvent, ProviderModule, Store, ToolContext, ToolHandler, ToolOutcome,
};
use assistant_core::schema::store_config;
use assistant_core::tools::{ToolSet, no_reply_needed, report, work_is_done};
use assistant_core::{ChannelKind, Outbound, ProtectionConfig};
use serde_json::json;

use crate::support::{
    self, CLOSING_ANSWER, ToolScript, channel, field, inbound, only, recv_reply, tool_choice_names,
    tool_scripted_provider,
};

/// The outbound edge a fixture's items arrive on.
type Outgoing = tokio::sync::mpsc::UnboundedReceiver<Outbound>;

/// The ledger shape of a turn ended by a bare park call: the message
/// summons the turn, the call names the tool, the resolution records the
/// close. No answer block, because no answer was written.
const PARKED_TURN: [&str; 5] = [
    "system_prompt",
    "tool_choice",
    "chat_message",
    "tool_call",
    "tool_result",
];

/// One assembled fixture over a provider scripted to call the named tool
/// with no narration ahead of it, plus the outbound edge — taken before
/// anything is ingested, so everything the channel receives afterwards is
/// this edge's business. The tools themselves are registered by the
/// assembly unconditionally, so nothing here admits them: that omission is
/// the registration's own proof.
async fn parking_fixture(tool: &str) -> (support::Fixture, Outgoing) {
    let (provider, script) = tool_scripted_provider(
        ToolScript {
            tool: tool.to_owned(),
            input: "{}".into(),
            narration: None,
        },
        None,
    );
    assemble(provider, script).await
}

/// The assembly preamble every fixture here shares: an in-memory store, an
/// empty embedder set, the default budgets.
async fn assemble(
    provider: Box<dyn ProviderModule>,
    script: support::ScriptHandle,
) -> (support::Fixture, Outgoing) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        provider,
        script,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let outgoing = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    (fixture, outgoing)
}

/// Whether one block is a resolution carrying the framework's ends-turn
/// stamp — read off the stored row, which is where the turn's end lives.
fn stamped(block: &Block) -> bool {
    block.block_type == "tool_result" && block.fields["ends_turn"] == json!(true)
}

// ─── AC2 and AC3: the bare call ends the turn, and stores its close ──────

/// The whole flow for one tool, block by block: a member's message summons
/// a turn, the model calls the tool with nothing written ahead of the
/// call, the resolution carries the module's own close and the framework's
/// stamp, no continuation round is dispatched, and the channel receives
/// nothing at all.
///
/// The last claim needs an instrument that can tell the two outcomes
/// apart, so the test does not stop at an empty edge — an edge is empty
/// while a continuation is still being dispatched, too. A second message
/// follows, its answer is awaited, and the request count is read after it:
/// two requests total means the park round was never continued, while a
/// dispatched continuation would have made three and delivered its own
/// answer first.
async fn assert_a_bare_call_ends_the_turn(tool: &str, close: &str) {
    let (fixture, mut outgoing) = parking_fixture(tool).await;
    let key = channel(&format!("dm-{tool}"));
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &key,
            ChannelKind::Direct,
            "member-1",
            "a question for someone else",
        ),
    )
    .await;

    let blocks = support::viewed_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the turn ended by a bare call",
        |blocks| {
            blocks.len() == PARKED_TURN.len()
                && blocks
                    .iter()
                    .zip(PARKED_TURN)
                    .all(|(block, want)| block.block_type == want)
        },
    )
    .await;
    assert_eq!(field(&only(&blocks, "tool_call"), "name"), tool);
    let resolution = only(&blocks, "tool_result");
    assert_eq!(
        field(&resolution, "content"),
        close,
        "the resolution stores the module's own close, byte for byte"
    );
    assert!(
        stamped(&resolution),
        "the resolution row carries the framework's ends-turn stamp"
    );
    assert!(
        !blocks.iter().any(|block| block.block_type == "text"),
        "a parked turn writes no answer: {:?}",
        blocks
            .iter()
            .map(|block| block.block_type.as_str())
            .collect::<Vec<_>>()
    );
    let nothing = outgoing.try_recv();
    assert!(
        nothing.is_err(),
        "the channel receives nothing from a parked turn: {nothing:?}"
    );

    // The second message proves the machinery is live and that the count
    // above is the whole traffic: its turn reads the resolved call and
    // closes with prose, which arrives as the FIRST thing this edge ever
    // carries.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "member-1", "and now one for you"),
    )
    .await;
    assert_eq!(
        recv_reply(&mut outgoing).await.text,
        support::disclosed(CLOSING_ANSWER),
        "the answer of the SECOND turn is the first thing delivered"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        2,
        "two model requests: the parked turn and the one the second message summoned \
         — the parked round summoned no continuation"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bare_no_reply_needed_call_ends_the_turn_and_delivers_nothing() {
    assert_a_bare_call_ends_the_turn(no_reply_needed::NAME, no_reply_needed::CLOSE).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bare_work_is_done_call_ends_the_turn_and_delivers_nothing() {
    assert_a_bare_call_ends_the_turn(work_is_done::NAME, work_is_done::CLOSE).await;
}

/// AC3 at the constants themselves: the two stored closes are the
/// sentences this unit fixed, whole, and they are not each other's. The
/// end-to-end halves above read the same constants off the stored rows.
#[test]
fn the_two_stored_closes_are_the_units_sentences() {
    assert_eq!(no_reply_needed::CLOSE, "Turn ended: no reply was needed.");
    assert_eq!(
        work_is_done::CLOSE,
        "Turn ended: the actions taken are the whole answer."
    );
}

// ─── AC4: the registered set, and the summary turn that has neither ──────

/// Both tools ride the registered set: a created conversation's recorded
/// choice names them, in a deployment carrying no moderation handle and
/// therefore no report tool, which is what "unconditional" buys — and the
/// compaction fork's own choice, written EMPTY by the framework's door,
/// names neither, so the turn that summarizes a conversation is offered no
/// way to end itself early.
///
/// This unit's criterion 6 places the two tools "beside the react and
/// report tools". No home holds those two together unconditionally: the
/// react tool joins with the assembled tools, which exist to carry
/// injected state, and the report tool joins there only where a moderation
/// handle is configured. A tool that needs nothing handed to it joins the
/// assembly's unconditional home, whose other residents are the runtime
/// facts and the harness changelog, and that is where these two are
/// registered. The criterion's naming of the neighbours is the part that
/// disagrees with the tree; every claim it makes about behaviour holds.
///
/// The fork is opened here instead of running a whole compaction,
/// because a compaction retires its temporary conversation as soon as the
/// summary is read: the ledger this claim is about exists only inside that
/// operation. The door opened here is the one the compaction calls.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn both_tools_ride_the_choice_and_the_summary_fork_names_neither() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-turn-ends-choice"),
            ChannelKind::Direct,
            "42",
            "hello",
        ),
    )
    .await;
    recv_reply(&mut replies).await;

    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let names = tool_choice_names(&blocks);
    for tool in [no_reply_needed::NAME, work_is_done::NAME] {
        assert!(
            names.contains(&tool.to_owned()),
            "the created conversation's choice names {tool}: {names:?}"
        );
    }
    assert!(
        !names.contains(&report::NAME.to_owned()),
        "this deployment has no report tool, and the two ride its choice anyway: {names:?}"
    );

    let summary_turn = fixture
        .store
        .fork_temporary(
            receipt.conversation_id,
            blocks.last().expect("the source ledger is not empty").id,
            TemporaryFork {
                records: Vec::new(),
                instructions: "Summarize the conversation above.".into(),
            },
        )
        .await
        .expect("the compaction's fork door opens");
    let forked = fixture
        .store
        .list_blocks(summary_turn.conversation_id)
        .await
        .expect("the forked ledger reads");
    assert_eq!(
        tool_choice_names(&forked),
        Vec::<String>::new(),
        "the fork records the empty choice, so a summary turn has no tools at all"
    );
    fixture
        .store
        .delete_conversation(summary_turn.conversation_id)
        .await
        .expect("the temporary conversation retires");
}

// ─── AC7: a park beside a sibling silences only itself ───────────────────

/// The sibling call of the same round: it answers a fixed line, but
/// only once the park call's stamped resolution is already in the ledger.
///
/// The wait is what makes the claim readable. The runner executes a
/// round's calls in parallel tasks, so without it the two resolutions
/// could commit in either order — and the framework's rule differs by
/// order: a sibling outcome at the tail owes a continuation now, while a
/// stamped outcome at the tail rests the frontier and leaves the sibling's
/// inheritance to the next summons. This test is about the first ordering,
/// so it makes that ordering happen instead of hoping for it.
struct LateSibling;

/// The line the sibling answers with, once it has seen the park.
const SIBLING_RESULT: &str = "the sibling ran after the park";

/// The registered name the sibling answers to.
const SIBLING: &str = "sibling_probe";

impl ToolHandler<CoreEvent> for LateSibling {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: SIBLING.into(),
            description: "a probe that answers after the park resolution is stored".into(),
            parameters: json!({ "type": "object" }),
        }
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            let deadline = Instant::now() + support::DEADLINE;
            while Instant::now() < deadline {
                let stored = ctx
                    .agency
                    .store
                    .list_blocks(ctx.agency.conversation_id)
                    .await
                    .expect("the ledger reads");
                if stored.iter().any(stamped) {
                    return ToolOutcome::Done(SIBLING_RESULT.to_owned());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            ToolOutcome::Error("the park resolution never arrived".into())
        })
    }
}

/// One round calls `no_reply_needed` and a sibling tool, and the sibling
/// resolves last. The framework's rule holds, all three claims: the
/// stamped outcome does not hold the turn open, the sibling's own
/// resolution still lands, and the sibling's continuation round is still
/// summoned — which is what delivers the closing answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_park_beside_a_sibling_call_silences_only_itself() {
    let (provider, script) = support::same_round_calls_provider(vec![
        support::RoundCall {
            tool: no_reply_needed::NAME.into(),
            input: "{}".into(),
        },
        support::RoundCall {
            tool: SIBLING.into(),
            input: "{}".into(),
        },
    ]);
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let mut tools = ToolSet::new();
    tools.admit(LateSibling);
    let fixture =
        support::start_assistant_full(store, provider, script, tools, ProtectionConfig::default())
            .await;
    let mut outgoing = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-park-with-sibling"),
            ChannelKind::Direct,
            "member-2",
            "one round, two calls",
        ),
    )
    .await;

    let blocks = support::viewed_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the round holding a park and its sibling",
        |blocks| {
            blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;

    let resolutions: Vec<&Block> = blocks
        .iter()
        .filter(|block| block.block_type == "tool_result")
        .collect();
    assert_eq!(resolutions.len(), 2, "both calls of the round resolved");
    let park = resolutions
        .iter()
        .find(|block| field(block, "content") == no_reply_needed::CLOSE)
        .expect("the park resolution stands");
    assert!(stamped(park), "the park resolution carries the stamp");
    let sibling = resolutions
        .iter()
        .find(|block| field(block, "content") == SIBLING_RESULT)
        .expect("the sibling's own resolution was stored behind the stamped one");
    assert!(
        !stamped(sibling),
        "the stamp silences its own outcome and nothing else"
    );

    assert_eq!(
        recv_reply(&mut outgoing).await.text,
        support::disclosed(CLOSING_ANSWER),
        "the sibling's continuation round was summoned and answered"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        2,
        "the calling round and the one continuation the sibling summoned"
    );
}
