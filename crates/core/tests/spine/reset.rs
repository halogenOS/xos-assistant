//! The session-reset commands at the core's edges (unit 45): `/wipe`
//! starting a group over on an empty conversation, `/compact` forking one
//! down to its recent tail, the outbound edge's seam that keeps a fork's
//! inherited answers from going out twice, the floor and the direct-chat
//! fence, the unattended compaction the framework's forced turn end
//! triggers, and the promise that runs through all of it: nothing
//! established is deleted.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_ledger::agency::{
    LeafKind, Quote, Status, SystemPrompt, Text, ToolCall, ToolError, ToolResult,
};
use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::{BlockDestination, domain_run};
use agent_ledger::{Block, CoreEvent, Store, ToolContext, ToolHandler, ToolOutcome};
use assistant_core::commands::{
    COMPACT_ALREADY, COMPACT_COMMAND, COMPACT_DONE, COMPACT_KEPT_MESSAGES, WIPE_COMMAND, WIPE_DONE,
};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::note::CONTEXT_NOTE_KIND;
use assistant_core::schema::{DOMAIN, store_config};
use assistant_core::tools::ToolSet;
use assistant_core::tools::palette::TOOL_PALETTE_KIND;
use assistant_core::{
    Assistant, Authority, ChannelKey, ChannelKind, ChannelReset, InboundMessage, IngestOutcome,
    Observation, ObservedFact, PRIVACY_REPLY_CAP, PRIVACY_UNPUBLISHED, ProtectionConfig,
    RESET_REPLY_CAP, privacy,
};
use serde_json::json;

use crate::support::{
    self, CLOSING_ANSWER, ToolScript, channel, inbound, inbound_as, inbound_unaddressed,
    recv_reply, settle_shape, tool_scripted_provider, with_command, with_origin,
};

/// The scripted tool the reset fixtures call: it needs no wire, so a
/// conversation carrying real tool traffic costs no server.
const PROBE: &str = "probe";

/// A tool that answers a fixed line, so a turn writes a real call and a
/// real result into the ledger.
struct ProbeTool(Arc<AtomicBool>);

impl ToolHandler<CoreEvent> for ProbeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: PROBE.into(),
            description: "a probe that answers a fixed line".into(),
            parameters: json!({ "type": "object" }),
        }
    }

    fn execute<'a>(
        &'a self,
        _input: &'a str,
        _ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async {
            self.0.store(true, Ordering::SeqCst);
            ToolOutcome::Done("the probe ran".into())
        })
    }
}

/// The outbound edge a fixture's replies arrive on.
type Replies = tokio::sync::mpsc::UnboundedReceiver<assistant_core::Outbound>;

/// Whether the stored kind is one a compact counts as a chat row, read
/// through the same declarations the core reads: a recorded channel
/// message, one of the assistant's own text blocks, or a stored quote.
fn is_chat_row(kind: &str) -> bool {
    kind == CHAT_MESSAGE_KIND || Text::KINDS.contains(&kind) || Quote::KINDS.contains(&kind)
}

/// Whether the stored kind is tool traffic — a call, its result, its error
/// — named through the framework's own declarations.
fn is_tool_traffic(kind: &str) -> bool {
    ToolCall::KINDS.contains(&kind)
        || ToolResult::KINDS.contains(&kind)
        || ToolError::KINDS.contains(&kind)
}

/// One assembled fixture over the tool-scripted provider with the probe
/// registered, plus the outbound edge taken before anything is ingested —
/// which is what makes everything stored afterwards this edge's business.
async fn reset_fixture() -> (support::Fixture, Replies) {
    reset_fixture_configured(ProtectionConfig::default()).await
}

/// The reset fixture with the answering budgets spelled out, for the pin
/// that a spent budget never silences a moderator's reset.
async fn reset_fixture_configured(protection: ProtectionConfig) -> (support::Fixture, Replies) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let mut tools = ToolSet::new();
    tools.admit(
        Authority::Member,
        ProbeTool(Arc::new(AtomicBool::new(false))),
    );
    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: PROBE.into(),
            input: "{}".into(),
            narration: None,
        },
        None,
    );
    let fixture = support::start_assistant_full(store, provider, handle, tools, protection).await;
    let replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    (fixture, replies)
}

/// The conversation a channel currently maps to, read raw — the fact a
/// reset changes, so no test infers it from a later ingestion.
async fn mapped_conversation(store: &Store, key: &ChannelKey) -> Option<i64> {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    domain_run(&store.tx(), DOMAIN, move |conn| {
        Ok(conn
            .query_row(
                "SELECT conversation_id FROM channels WHERE adapter = ?1 AND channel = ?2",
                rusqlite::params![adapter, channel],
                |row| row.get::<_, i64>(0),
            )
            .ok())
    })
    .await
    .expect("the mapping reads")
}

/// Point a channel's mapping at the given conversation, written raw — the
/// concurrent racer a reset can lose its claim to, with no second assistant
/// to assemble.
async fn claim_channel_for(store: &Store, key: &ChannelKey, conversation_id: i64) {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    domain_run(&store.tx(), DOMAIN, move |conn| {
        conn.execute(
            "INSERT INTO channels (adapter, channel, kind, conversation_id)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                adapter,
                channel,
                ChannelKind::Group.as_str(),
                conversation_id
            ],
        )?;
        Ok(())
    })
    .await
    .expect("the racer claims the channel");
}

/// One group command message: unaddressed, because a command is addressing
/// by form and a moderator does not mention the assistant to reset it.
fn command_message(
    key: &ChannelKey,
    sender: &str,
    authority: Authority,
    token: &str,
    origin: &str,
) -> InboundMessage {
    let mut message = inbound_as(key, ChannelKind::Group, sender, authority, token);
    message.addressed = false;
    with_origin(with_command(message, token), origin)
}

/// Ingest one command and answer with what it delivered and what it did to
/// the channel's session.
async fn invoke(assistant: &Assistant, message: InboundMessage) -> (Option<String>, ChannelReset) {
    match assistant
        .ingest(message)
        .await
        .expect("the command ingests")
    {
        IngestOutcome::Recorded { deliver, reset, .. } => {
            (deliver.map(|item| item.text().to_owned()), reset)
        }
        other => panic!("the command is recorded, not refused: {other:?}"),
    }
}

/// The block kinds of one conversation's whole ledger.
async fn kinds(store: &Store, conversation_id: i64) -> Vec<String> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.block_type.clone())
        .collect()
}

/// The shape a compacted fork has, whichever trigger produced it: no tool
/// traffic, no forced-end marker, exactly the kept bound of chat rows, one
/// palette, the newest note of the topic, and the current prompt alone.
/// Both triggers are held to this one reading, which is what "one
/// operation, two triggers" has to mean observably.
fn assert_compacted_shape(blocks: &[Block]) {
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|block| block.block_type.as_str())
        .collect();
    assert!(
        !blocks
            .iter()
            .any(|block| is_tool_traffic(block.block_type.as_str())),
        "no tool traffic survives: {kinds:?}"
    );
    assert!(
        !blocks
            .iter()
            .any(|block| Status::KINDS.contains(&block.block_type.as_str())),
        "no forced-end marker crosses, so a fork cannot re-fire: {kinds:?}"
    );
    assert_eq!(
        chat_rows(blocks),
        COMPACT_KEPT_MESSAGES,
        "exactly the kept bound of chat rows crosses: {kinds:?}"
    );
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == TOOL_PALETTE_KIND)
            .count(),
        1,
        "the palette crosses, so a fork woken by a turn still admits its tools"
    );
    let notes: Vec<String> = blocks
        .iter()
        .filter(|block| block.block_type == CONTEXT_NOTE_KIND)
        .map(|block| support::field(block, "text"))
        .collect();
    assert_eq!(
        notes,
        vec!["The newest title".to_owned()],
        "the newest note of the topic crosses, and only it"
    );
    let prompts: Vec<String> = blocks
        .iter()
        .filter(|block| SystemPrompt::KINDS.contains(&block.block_type.as_str()))
        .map(|block| support::block_text(block, "content"))
        .collect();
    assert_eq!(
        prompts,
        vec![support::composed_prompt()],
        "the fork records the current prompt and none of the inherited ones"
    );
}

/// Nothing established is deleted: every block the conversation held is
/// still there, in order, with whatever was recorded afterwards behind it.
async fn assert_kept_whole(store: &Store, conversation_id: i64, held: &[i64], what: &str) {
    let now: Vec<i64> = store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.id)
        .collect();
    assert!(
        now.starts_with(held),
        "{what} keeps every block it had: {held:?} against {now:?}"
    );
}

/// How many of the conversation's blocks are chat rows.
fn chat_rows(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .filter(|block| is_chat_row(block.block_type.as_str()))
        .count()
}

/// A group whose conversation holds real tool traffic, two title notes and
/// more chat rows than the kept bound — the shape `/compact` exists for.
/// Answers the channel, the conversation, and its block ids.
async fn flooded_group(
    fixture: &support::Fixture,
    replies: &mut Replies,
    id: &str,
) -> (ChannelKey, i64, Vec<i64>) {
    let key = support::authorized_group(&fixture.assistant, id).await;
    for title in ["The first title", "The newest title"] {
        fixture
            .assistant
            .observe(Observation {
                channel: key.clone(),
                channel_kind: ChannelKind::Group,
                fact: ObservedFact::Title(title.into()),
            })
            .await
            .expect("the title is observed");
    }
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "what changed?"),
    )
    .await;
    settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the tool turn",
        &[
            SystemPrompt::KINDS[0],
            TOOL_PALETTE_KIND,
            CONTEXT_NOTE_KIND,
            CONTEXT_NOTE_KIND,
            CHAT_MESSAGE_KIND,
            ToolCall::KINDS[0],
            ToolResult::KINDS[0],
            Text::KINDS[0],
        ],
    )
    .await;
    // The turn's own answer goes out before anything is compacted, so a
    // later item on this edge is a re-send and nothing else.
    let first = recv_reply(replies).await;
    assert_eq!(first.text, support::disclosed(CLOSING_ANSWER));

    for index in 0..COMPACT_KEPT_MESSAGES {
        support::ingest_recorded(
            &fixture.assistant,
            with_origin(
                inbound_unaddressed(&key, ChannelKind::Group, "43", "chatter"),
                &format!("filler-{index}"),
            ),
        )
        .await;
    }
    let ids = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.id)
        .collect();
    (key, receipt.conversation_id, ids)
}

/// AC2: a moderator's `/wipe` maps the channel to a new empty
/// conversation — the current prompt and palette, no inherited block —
/// answers its exact line, carries the reset directive, and leaves the old
/// conversation whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_moderators_wipe_starts_the_group_over_on_an_empty_session() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, source, source_ids) = flooded_group(&fixture, &mut replies, "wipe-room").await;

    let (answer, reset) = invoke(
        &fixture.assistant,
        command_message(&key, "5", Authority::Moderator, WIPE_COMMAND, "wipe-1"),
    )
    .await;
    assert_eq!(answer.as_deref(), Some(WIPE_DONE));
    assert_eq!(
        reset,
        ChannelReset::Replaced,
        "the adapter is told the session was replaced"
    );

    let fresh = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    assert_ne!(fresh, source, "the channel points at a new conversation");
    assert_eq!(
        kinds(&fixture.store, fresh).await,
        vec![SystemPrompt::KINDS[0], TOOL_PALETTE_KIND],
        "the fresh session is exactly what a newly admitted group gets"
    );
    let blocks = fixture
        .store
        .list_blocks(fresh)
        .await
        .expect("the ledger reads");
    assert_eq!(
        support::block_text(&blocks[0], "content"),
        support::composed_prompt(),
        "the fresh session records the current prompt"
    );

    assert_kept_whole(
        &fixture.store,
        source,
        &source_ids,
        "the wiped conversation",
    )
    .await;

    // The next message speaks into the empty session.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "44", "anybody here?"),
    )
    .await;
    assert_eq!(receipt.conversation_id, fresh);
}

/// AC3: `/compact` maps the channel to a fork whose readable history is
/// the kept set — no tool traffic, at most the kept bound of chat rows,
/// the palette, the newest note per topic — the source keeps every block,
/// and a second `/compact` answers the nothing-to-cut line.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_moderators_compact_keeps_the_recent_tail_and_cuts_the_flood() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, source, source_ids) = flooded_group(&fixture, &mut replies, "compact-room").await;

    let (answer, reset) = invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            COMPACT_COMMAND,
            "compact-1",
        ),
    )
    .await;
    assert_eq!(answer.as_deref(), Some(COMPACT_DONE));
    assert_eq!(
        reset,
        ChannelReset::Kept,
        "a compact carries its context notes across, so the adapter forgets nothing"
    );

    let fork = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    assert_ne!(fork, source, "the channel points at the fork");
    let blocks = fixture
        .store
        .list_blocks(fork)
        .await
        .expect("the ledger reads");
    assert_compacted_shape(&blocks);

    // Nothing was deleted: detaching removes a junction row from the FORK,
    // so the source still reads exactly as it did, with the command row
    // that asked for the compact behind it.
    assert_kept_whole(&fixture.store, source, &source_ids, "the compacted source").await;

    let (again, _) = invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            COMPACT_COMMAND,
            "compact-2",
        ),
    )
    .await;
    assert_eq!(
        again.as_deref(),
        Some(COMPACT_ALREADY),
        "a second compact finds nothing to cut"
    );
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(fork),
        "the nothing-to-cut answer forks nothing"
    );
}

/// The fork is born delivered. No answer the edge already sent goes
/// out again after a compact, no disclosure line is written a second time
/// into a block the source still holds, and the fork's own next answer
/// delivers normally.
///
/// The last of the three is what rules out the durable ratchet cursor as
/// the seed: that cursor stands past the answer the wake is about by the
/// time the edge reads it, so seeding from it would swallow the very
/// answer this test receives. The inherited boundary is the seed, and it
/// leaves no re-send residual to accept.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_compacted_fork_delivers_its_own_answers_and_never_the_inherited_ones() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, source, _) = flooded_group(&fixture, &mut replies, "seam-room").await;
    let delivered_answer: Vec<String> = fixture
        .store
        .list_blocks(source)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| Text::KINDS.contains(&block.block_type.as_str()))
        .map(|block| support::block_text(block, "content"))
        .collect();
    assert_eq!(delivered_answer, vec![support::disclosed(CLOSING_ANSWER)]);

    invoke(
        &fixture.assistant,
        command_message(&key, "5", Authority::Moderator, COMPACT_COMMAND, "seam-1"),
    )
    .await;
    let fork = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");

    // The fork's own turn: its answer is the one item on the edge.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "and now?"),
    )
    .await;
    assert_eq!(receipt.conversation_id, fork);
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text, CLOSING_ANSWER,
        "the fork's own answer goes out, and the kept history carries the \
         introduction receipt across, so nobody is introduced twice"
    );
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "no inherited answer is re-sent after a compact; got {extra:?}"
    );

    // The inherited answer block is untouched: one disclosure line, not two.
    let inherited: Vec<String> = fixture
        .store
        .list_blocks(source)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| Text::KINDS.contains(&block.block_type.as_str()))
        .map(|block| support::block_text(block, "content"))
        .collect();
    assert_eq!(
        inherited, delivered_answer,
        "no disclosure write reached a block the source still holds"
    );
}

/// The other half of the same seam: a wiped channel's fresh
/// conversation has inherited nothing, so it delivers its first answer
/// normally.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wiped_channels_fresh_session_delivers_its_first_answer() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, _, _) = flooded_group(&fixture, &mut replies, "wipe-seam").await;
    invoke(
        &fixture.assistant,
        command_message(&key, "5", Authority::Moderator, WIPE_COMMAND, "wipe-seam-1"),
    )
    .await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "77", "starting over"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text,
        support::disclosed(CLOSING_ANSWER),
        "a person never introduced to reads the disclosure line on their first answer"
    );
}

/// AC4: the floor and the fence. A member's `/wipe` in a group and a
/// moderator's `/compact` in a direct chat are recognized, stamped with the
/// command kind — no debt, no turn — and answered with silence, while a
/// direct-chat `/privacy` still answers: the fence belongs to the two
/// resets, not to the catalogue.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn below_the_floor_and_outside_the_group_the_resets_answer_silence() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, source, _) = flooded_group(&fixture, &mut replies, "floor-room").await;

    for token in [WIPE_COMMAND, COMPACT_COMMAND] {
        let (answer, reset) = invoke(
            &fixture.assistant,
            command_message(
                &key,
                "88",
                Authority::Member,
                token,
                &format!("member-{token}"),
            ),
        )
        .await;
        assert_eq!(answer, None, "{token} below the floor answers silence");
        assert_eq!(reset, ChannelReset::Kept);
    }
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(source),
        "a member's reset changes nothing"
    );

    // The command stamp: the member's rows opened no turn and took no debt.
    let blocks = fixture
        .store
        .list_blocks(source)
        .await
        .expect("the ledger reads");
    let member_rows: Vec<&Block> = blocks
        .iter()
        .filter(|block| {
            block.block_type == CHAT_MESSAGE_KIND
                && support::field(block, "text").starts_with('/')
                && support::field(block, "text") != "chatter"
        })
        .collect();
    assert_eq!(member_rows.len(), 2, "both commands were recorded");
    for row in member_rows {
        assert_eq!(
            support::field(row, "limited"),
            "command",
            "an unoffered command still takes the command stamp"
        );
        assert_eq!(
            row.fields["answer_due"],
            json!(false),
            "an unoffered command opens no turn"
        );
    }

    // The direct-chat fence, and the privacy family crossing it.
    let direct = channel("floor-dm");
    for token in [WIPE_COMMAND, COMPACT_COMMAND] {
        let mut message = inbound_as(
            &direct,
            ChannelKind::Direct,
            "5",
            Authority::Moderator,
            token,
        );
        message.addressed = false;
        let (answer, reset) = invoke(
            &fixture.assistant,
            with_origin(with_command(message, token), &format!("direct-{token}")),
        )
        .await;
        assert_eq!(answer, None, "{token} in a direct chat answers silence");
        assert_eq!(reset, ChannelReset::Kept);
    }
    let (privacy, _) = invoke(
        &fixture.assistant,
        with_origin(
            with_command(
                inbound(&direct, ChannelKind::Direct, "5", privacy::PRIVACY_COMMAND),
                privacy::PRIVACY_COMMAND,
            ),
            "direct-privacy",
        ),
    )
    .await;
    assert_eq!(
        privacy.as_deref(),
        Some(PRIVACY_UNPUBLISHED),
        "the direct-chat fence is the resets' own; a data right crosses it"
    );
}

/// AC5: the framework's forced turn end triggers the same compaction
/// unattended — nothing is answered in chat, the fork carries no marker,
/// and the swept source is unmapped so no later change re-fires it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_exhausted_turn_compacts_the_session_unattended() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, source, _) = flooded_group(&fixture, &mut replies, "exhausted-room").await;

    fixture
        .store
        .insert_status_block(
            BlockDestination::from(source),
            Status::TOOL_CALLS_EXHAUSTED.into(),
            None,
        )
        .await
        .expect("the forced turn end records its marker");

    let deadline = std::time::Instant::now() + support::DEADLINE;
    let fork = loop {
        // The re-point drops the source's mapping row and then claims the
        // channel for the fork, so a poll landing between the two reads no
        // mapping at all. That gap is the watcher mid-flight, not a
        // failure: only a mapping that has MOVED ends the wait.
        if let Some(mapped) = mapped_conversation(&fixture.store, &key).await
            && mapped != source
        {
            break mapped;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the unattended compaction"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };

    let blocks = fixture
        .store
        .list_blocks(fork)
        .await
        .expect("the ledger reads");
    assert_compacted_shape(&blocks);
    assert!(
        replies.try_recv().is_err(),
        "the unattended compaction answers nothing in chat"
    );

    // The swept source is unmapped, so however many late changes wake its
    // fold it is never compacted again.
    let before = kinds(&fixture.store, source).await.len();
    fixture
        .store
        .insert_status_block(
            BlockDestination::from(source),
            Status::TOOL_CALLS_EXHAUSTED.into(),
            None,
        )
        .await
        .expect("a late marker appends");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(fork),
        "an unmapped source is never auto-compacted"
    );
    assert_eq!(
        kinds(&fixture.store, source).await.len(),
        before + 1,
        "the late marker is the only thing that changed"
    );
}

/// A reset whose mapping claim is lost to a concurrent racer made nothing,
/// and says nothing: no line, no reset directive, and the winner's session
/// governs the channel. What the losing fork loses is junction rows alone —
/// every block it held is still in the source conversation, which is the
/// whole reason a fork may be dropped at all.
///
/// The racer lands in the one window that exists for it, between the reset's
/// mapping delete and its own claim, through the seam the assembly installs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reset_that_loses_its_claim_says_nothing_and_leaves_the_winner_governing() {
    for token in [WIPE_COMMAND, COMPACT_COMMAND] {
        let (mut fixture, mut replies) = reset_fixture().await;
        let (key, source, source_ids) = flooded_group(&fixture, &mut replies, "claim-room").await;

        let armed = Arc::new(AtomicBool::new(true));
        {
            let store = fixture.store.clone();
            let key = key.clone();
            let armed = Arc::clone(&armed);
            fixture
                .assistant
                .pause_between_reset_delete_and_claim(Arc::new(move || {
                    let store = store.clone();
                    let key = key.clone();
                    let armed = Arc::clone(&armed);
                    Box::pin(async move {
                        // Once: the racing claim, taken while the reset is
                        // between its delete and its claim.
                        if armed.swap(false, Ordering::SeqCst) {
                            claim_channel_for(&store, &key, source).await;
                        }
                    })
                }));
        }

        let (answer, reset) = invoke(
            &fixture.assistant,
            command_message(&key, "5", Authority::Moderator, token, "claim-1"),
        )
        .await;
        assert_eq!(answer, None, "{token} that lost its claim answers silence");
        assert_eq!(
            reset,
            ChannelReset::Kept,
            "{token} that lost its claim fires no directive"
        );
        assert!(
            replies.try_recv().is_err(),
            "{token} that lost its claim sends nothing to the chat"
        );
        assert_eq!(
            mapped_conversation(&fixture.store, &key).await,
            Some(source),
            "the winner's session governs the channel"
        );
        assert_kept_whole(
            &fixture.store,
            source,
            &source_ids,
            "the source of a reset that lost its claim",
        )
        .await;
    }
}

/// An ingestion that resolved the channel before an unattended compaction
/// moved it lands in the SURVIVING conversation, not the retired one.
///
/// The straddler is real: the wake arrives while a message is queued, and
/// the swap happens under the same lock the ingestion takes afterwards. The
/// mapping is therefore re-read INSIDE that lock — a pre-lock reading would
/// append into a ledger the model has stopped reading, and the adapter's
/// acknowledgment of the update means nothing would ever redeliver it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_queued_across_an_unattended_compact_lands_in_the_surviving_session() {
    let (mut fixture, mut replies) = reset_fixture().await;
    let (key, source, _) = flooded_group(&fixture, &mut replies, "straddle-room").await;

    let (reached_tx, mut reached_rx) = tokio::sync::mpsc::unbounded_channel();
    let resume = Arc::new(tokio::sync::Semaphore::new(0));
    let armed = Arc::new(AtomicBool::new(false));
    {
        let resume = Arc::clone(&resume);
        let armed = Arc::clone(&armed);
        fixture
            .assistant
            .pause_between_standing_read_and_append(Arc::new(move || {
                let reached_tx = reached_tx.clone();
                let resume = Arc::clone(&resume);
                let armed = Arc::clone(&armed);
                Box::pin(async move {
                    // Only the armed straddler waits; the fixture's other
                    // ingestions pass straight through.
                    if !armed.swap(false, Ordering::SeqCst) {
                        return;
                    }
                    let _ = reached_tx.send(());
                    tokio::time::timeout(support::DEADLINE, resume.acquire())
                        .await
                        .expect("the straddler is resumed before the deadline")
                        .expect("the semaphore outlives the test")
                        .forget();
                })
            }));
    }
    let assistant = Arc::new(fixture.assistant);

    armed.store(true, Ordering::SeqCst);
    let straddler = {
        let assistant = Arc::clone(&assistant);
        let key = key.clone();
        tokio::spawn(async move {
            assistant
                .ingest(with_origin(
                    inbound_unaddressed(&key, ChannelKind::Group, "43", "queued across the swap"),
                    "straddler",
                ))
                .await
                .expect("the straddling message is judged")
        })
    };
    tokio::time::timeout(support::DEADLINE, reached_rx.recv())
        .await
        .expect("the straddler reaches the seam before the deadline")
        .expect("the seam reports");

    // The forced turn end lands while the message waits, and the watcher
    // moves the channel to the fork.
    fixture
        .store
        .insert_status_block(
            BlockDestination::from(source),
            Status::TOOL_CALLS_EXHAUSTED.into(),
            None,
        )
        .await
        .expect("the forced turn end records its marker");
    let deadline = std::time::Instant::now() + support::DEADLINE;
    let fork = loop {
        if let Some(mapped) = mapped_conversation(&fixture.store, &key).await
            && mapped != source
        {
            break mapped;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting the unattended compaction"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };

    resume.add_permits(1);
    let outcome = straddler.await.expect("the straddling ingestion finishes");
    let IngestOutcome::Recorded { receipt, .. } = outcome else {
        panic!("the straddling message is recorded, not refused: {outcome:?}");
    };
    assert_eq!(
        receipt.conversation_id, fork,
        "the queued message lands in the surviving conversation"
    );
    let retired_rows: Vec<String> = fixture
        .store
        .list_blocks(source)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .map(|block| support::field(block, "text"))
        .collect();
    assert!(
        !retired_rows.contains(&"queued across the swap".to_owned()),
        "nothing was appended into the retired conversation"
    );
}

/// The two families' bounds are separate instances, in the direction that
/// costs most: a person who has spent every rights reply they get still has
/// their session reset. One family's flood may not silence the other's.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_spent_rights_window_never_silences_a_reset() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, _, _) = flooded_group(&fixture, &mut replies, "families-room").await;

    for index in 0..PRIVACY_REPLY_CAP {
        let (answer, _) = invoke(
            &fixture.assistant,
            command_message(
                &key,
                "5",
                Authority::Moderator,
                privacy::OPT_IN_COMMAND,
                &format!("rights-{index}"),
            ),
        )
        .await;
        assert_eq!(
            answer.as_deref(),
            Some(privacy::OPT_IN_ALREADY),
            "rights reply {index} is granted"
        );
    }
    let (spent, _) = invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            privacy::OPT_IN_COMMAND,
            "rights-past",
        ),
    )
    .await;
    assert_eq!(spent, None, "the rights window is spent");

    let (reset_answer, reset) = invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            WIPE_COMMAND,
            "families-wipe",
        ),
    )
    .await;
    assert_eq!(
        reset_answer.as_deref(),
        Some(WIPE_DONE),
        "the rights family's flood never reaches the resets' own window"
    );
    assert_eq!(reset, ChannelReset::Replaced);
}

/// AC6: the resets are bounded per person and delete nothing. Past the cap
/// one moderator's flood draws recorded silence, and the reset a silenced
/// command would have made is withheld with the reply.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_resets_are_bounded_per_person_and_the_silenced_one_changes_nothing() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, _, _) = flooded_group(&fixture, &mut replies, "bound-room").await;

    for index in 0..RESET_REPLY_CAP {
        let (answer, _) = invoke(
            &fixture.assistant,
            command_message(
                &key,
                "5",
                Authority::Moderator,
                WIPE_COMMAND,
                &format!("bound-{index}"),
            ),
        )
        .await;
        assert_eq!(
            answer.as_deref(),
            Some(WIPE_DONE),
            "wipe {index} is granted"
        );
    }
    let before = mapped_conversation(&fixture.store, &key).await;
    let (silenced, reset) = invoke(
        &fixture.assistant,
        command_message(&key, "5", Authority::Moderator, WIPE_COMMAND, "bound-past"),
    )
    .await;
    assert_eq!(silenced, None, "past the cap the reply is recorded silence");
    assert_eq!(reset, ChannelReset::Kept);
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        before,
        "a withheld reply withholds its reset"
    );

    // Another moderator's command is bounded independently.
    let (other, _) = invoke(
        &fixture.assistant,
        command_message(&key, "6", Authority::Moderator, WIPE_COMMAND, "bound-other"),
    )
    .await;
    assert_eq!(
        other.as_deref(),
        Some(WIPE_DONE),
        "one person's flood bounds that person alone"
    );
}

/// AC6: the reset window is budget-exempt. A moderator whose answering
/// budget the flood protection has spent still gets their session reset —
/// the budgets bound what the assistant says on its own account, and a
/// moderator command is not the flood they exist for.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_spent_answering_budget_never_silences_a_reset() {
    let (fixture, mut replies) =
        reset_fixture_configured(support::budgets(Some((1, 600)), None)).await;
    let key = support::authorized_group(&fixture.assistant, "budget-room").await;

    // The one answer this moderator's budget allows, spent.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "5", "what changed?"),
    )
    .await;
    let spent = recv_reply(&mut replies).await;
    assert_eq!(spent.text, support::disclosed(CLOSING_ANSWER));
    let source = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");

    let (answer, reset) = invoke(
        &fixture.assistant,
        command_message(&key, "5", Authority::Moderator, WIPE_COMMAND, "budget-wipe"),
    )
    .await;
    assert_eq!(
        answer.as_deref(),
        Some(WIPE_DONE),
        "the refusing budget does not reach the reset"
    );
    assert_eq!(reset, ChannelReset::Replaced);
    assert_ne!(
        mapped_conversation(&fixture.store, &key).await,
        Some(source),
        "the reset stood"
    );
}
