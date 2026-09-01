//! The session resets at the core's edges (unit 45, and the compaction of
//! unit 48): `/wipe` starting a group over on an empty conversation,
//! `/compact` summarizing the first half of a conversation and carrying the
//! second forward verbatim, the outbound edge's seam that keeps inherited
//! answers from going out twice, the floor and the direct-chat fence, the
//! unattended compaction the framework's forced turn end triggers, the
//! erasure scrub that replaces a whole compacted lineage, and the promise
//! that runs through all of it: nothing established is deleted.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_ledger::agency::{
    AncestorReference, LeafKind, Status, SystemPrompt, Text, ToolCall, ToolChoice, ToolResult,
};
use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::store::{BlockDestination, domain_run};
use agent_ledger::{Block, CoreEvent, Role, Store, ToolContext, ToolHandler, ToolOutcome};
use assistant_core::commands::{COMPACT_COMMAND, COMPACT_DONE, WIPE_COMMAND, WIPE_DONE};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::note::CONTEXT_NOTE_KIND;
use assistant_core::schema::{DOMAIN, store_config};
use assistant_core::tools::ToolSet;
use assistant_core::{
    Assistant, Authority, ChannelKey, ChannelKind, ChannelReset, ErasureOutcome, InboundMessage,
    IngestOutcome, Observation, ObservedFact, PRIVACY_REPLY_CAP, PRIVACY_UNPUBLISHED,
    ProtectionConfig, RESET_REPLY_CAP, privacy,
};
use serde_json::json;

use crate::support::{
    self, CLOSING_ANSWER, SCRIPTED_SUMMARY, ToolScript, channel, inbound, inbound_as,
    inbound_unaddressed, recv_reply, settle_shape, tool_scripted_provider, with_command,
    with_origin,
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

/// How many filler chat rows the flooded fixture appends. Enough that the
/// ledger splits with room on both sides of the cut; the exact number is
/// this fixture's business and nothing reads it as policy.
const FILLER_ROWS: usize = 12;

/// What the held provider streams before it stops mid-turn, so the source's
/// ledger provably holds a streaming tail when the compaction reaches it.
const HELD_NARRATION: &str = "Looking that up now.";

/// One assembled fixture over the tool-scripted provider with the probe
/// registered, plus the outbound edge taken before anything is ingested —
/// which is what makes everything stored afterwards this edge's business.
async fn reset_fixture() -> (support::Fixture, Replies) {
    reset_fixture_configured(ProtectionConfig::default()).await
}

/// The reset fixture over a provider that HOLDS its first turn open, having
/// narrated into it first — the shape a compaction has to survive: a source
/// caught mid-stream, with a streaming tail already in its ledger. The
/// compaction's own turns are never held; the scripted provider answers those
/// ahead of the hold.
async fn held_reset_fixture(hold: &Arc<support::TurnHold>) -> (support::Fixture, Replies) {
    reset_fixture_built(ProtectionConfig::default(), Some(Arc::clone(hold))).await
}

/// The reset fixture with the answering budgets spelled out, for the pin
/// that a spent budget never silences a moderator's reset.
async fn reset_fixture_configured(protection: ProtectionConfig) -> (support::Fixture, Replies) {
    reset_fixture_built(protection, None).await
}

/// The one assembly every reset fixture is built from: the probe tool, the
/// tool script, and whatever turn hold the caller needs.
async fn reset_fixture_built(
    protection: ProtectionConfig,
    hold: Option<Arc<support::TurnHold>>,
) -> (support::Fixture, Replies) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let mut tools = ToolSet::new();
    tools.admit(ProbeTool(Arc::new(AtomicBool::new(false))));
    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: PROBE.into(),
            input: "{}".into(),
            // A held turn has to have written something before it stops, or
            // there is no streaming tail for the settle to read.
            narration: hold.as_ref().map(|_| HELD_NARRATION.to_owned()),
        },
        hold,
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
        IngestOutcome::Recorded { deliver, reset, .. } => (
            deliver.and_then(|item| item.text().map(ToOwned::to_owned)),
            reset,
        ),
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

/// The shape a compacted thread has, whichever door produced it, asserted
/// against the source it came from: the current prompt, a block naming that
/// source, the captured summary, the source's own tool choice carried
/// across, and then a SUFFIX of the source's own ledger — the same blocks,
/// shared by id, never copies. Both doors are held to this one reading,
/// which is what "one mechanism, three doors" has to mean observably.
///
/// The carried choice is the library's own append (unit 52, 2026-09-01): a
/// compacted thread continues the same session, and an inherited row may
/// itself owe a turn, so the record has to be in place before the rows
/// arrive.
///
/// The summarized half is what is missing from the front of that suffix, and
/// the assertion below is exactly that: the thread's inherited ids are a
/// non-empty proper suffix of the source's, so something was summarized and
/// something was carried.
async fn assert_compacted_shape(store: &Store, source: i64, thread: i64) {
    let blocks = store.list_blocks(thread).await.expect("the ledger reads");
    let kinds: Vec<&str> = blocks
        .iter()
        .map(|block| block.block_type.as_str())
        .collect();
    assert!(
        blocks.len() >= 5,
        "a compacted thread opens with four blocks and inherits at least one: {kinds:?}"
    );

    assert_eq!(blocks[0].block_type, SystemPrompt::KINDS[0]);
    assert_eq!(
        support::block_text(&blocks[0], "content"),
        support::composed_prompt(),
        "the thread records the current prompt"
    );

    assert_eq!(
        blocks[1].block_type,
        AncestorReference::KINDS[0],
        "the thread's first content block names where it came from: {kinds:?}"
    );
    assert_eq!(
        blocks[1].fields["ancestor_conversation_id"],
        json!(source),
        "the reference names the conversation this thread continues"
    );

    assert_eq!(
        blocks[2].block_type,
        Text::KINDS[0],
        "the compaction message is its own append behind the reference: {kinds:?}"
    );
    assert_eq!(blocks[2].role, Some(Role::System));
    assert_eq!(
        support::block_text(&blocks[2], "content"),
        SCRIPTED_SUMMARY,
        "the compaction message carries the captured summary"
    );

    let source_ids: Vec<i64> = store
        .list_blocks(source)
        .await
        .expect("the source reads")
        .iter()
        .map(|block| block.id)
        .collect();
    assert_eq!(
        blocks[3].block_type,
        ToolChoice::KINDS[0],
        "the source's tool choice is recorded ahead of the inherited rows: {kinds:?}"
    );
    assert_eq!(
        support::tool_choice_names(&blocks[..4]),
        store
            .newest_tool_choice(source)
            .await
            .expect("the source's recorded choice reads")
            .expect("the source recorded one"),
        "the thread continues the session's own tools"
    );

    let inherited: Vec<i64> = blocks[4..].iter().map(|block| block.id).collect();
    let at = source_ids.len() - inherited.len();
    assert!(
        at > 0,
        "the summarized half is what the thread does NOT inherit"
    );
    assert_eq!(
        inherited,
        source_ids[at..],
        "the second half rides across by identity: shared junction rows, never copies"
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

/// A group whose conversation holds real tool traffic, two title notes and
/// a long tail of chat — a ledger with two halves, the shape a compaction
/// exists for. Answers the channel, the conversation, and its block ids.
async fn flooded_group(
    fixture: &support::Fixture,
    replies: &mut Replies,
    id: &str,
) -> (ChannelKey, i64, Vec<i64>) {
    flooded_group_of(fixture, replies, id, FILLER_ROWS).await
}

/// The flooded group with its filler count spelled out, for the fixture
/// that needs the delivered answer to land in the half a compaction carries
/// forward rather than the half it summarizes.
async fn flooded_group_of(
    fixture: &support::Fixture,
    replies: &mut Replies,
    id: &str,
    fillers: usize,
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
            ToolChoice::KINDS[0],
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

    for index in 0..fillers {
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
/// conversation — the current prompt and tool choice, no inherited block —
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
        vec![SystemPrompt::KINDS[0], ToolChoice::KINDS[0]],
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

/// AC2 and AC3: `/compact` runs THE mechanism. The channel moves to a
/// thread carrying the current prompt, a block naming the conversation it
/// continues, the captured summary and the second half of the ledger
/// verbatim; the source keeps every block it had; and the next member
/// message is answered from the compacted thread as usual.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_moderators_compact_summarizes_the_first_half_and_carries_the_second() {
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
        "a compaction carries the second half across, so the adapter forgets nothing"
    );

    let thread = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    assert_ne!(thread, source, "the channel points at the compacted thread");
    assert_compacted_shape(&fixture.store, source, thread).await;

    // Nothing was deleted: the thread SHARES the second half's blocks, so
    // the source still reads exactly as it did, with the command row that
    // asked for the compaction behind it.
    assert_kept_whole(&fixture.store, source, &source_ids, "the compacted source").await;

    // The temporary conversation is retired: nothing but the source and the
    // thread hold the channel's history, and no third conversation is left
    // mapped anywhere.
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(thread),
        "the temporary conversation never took the channel"
    );

    // And the thread is served like any other: the next member message
    // lands in it and draws its answer.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "and now?"),
    )
    .await;
    assert_eq!(receipt.conversation_id, thread);
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, CLOSING_ANSWER);
}

/// The compacted thread is born delivered. No answer the edge already sent
/// goes out again after a compaction, no disclosure line is written a second
/// time into a block the source still holds, and the thread's own next
/// answer delivers normally.
///
/// The fixture is deliberately SHORT, so the cut leaves the already-answered
/// turn's own text block in the half that rides across verbatim: that is the
/// block a re-send would come from, and a longer ledger would summarize it
/// away and prove nothing.
///
/// The last of the three is what rules out the durable ratchet cursor as
/// the seed: that cursor stands past the answer the wake is about by the
/// time the edge reads it, so seeding from it would swallow the very
/// answer this test receives. The inherited boundary is the seed, and it
/// leaves no re-send residual to accept.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_compacted_thread_delivers_its_own_answers_and_never_the_inherited_ones() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, source, _) = flooded_group_of(&fixture, &mut replies, "seam-room", 4).await;
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
    let thread = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    let inherited_answers = fixture
        .store
        .list_blocks(thread)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| {
            block.role == Some(Role::Assistant) && Text::KINDS.contains(&block.block_type.as_str())
        })
        .count();
    assert_eq!(
        inherited_answers, 1,
        "the fixture is short enough that the delivered answer rides across, \
         which is what makes a re-send observable at all"
    );

    // The thread's own turn: its answer is the one item on the edge.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "and now?"),
    )
    .await;
    assert_eq!(receipt.conversation_id, thread);
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.text, CLOSING_ANSWER,
        "the thread's own answer goes out, and the inherited history carries the \
         introduction receipt across, so nobody is introduced twice"
    );
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "no inherited answer is re-sent after a compaction; got {extra:?}"
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

/// AC3: the framework's forced turn end drives the SAME mechanism
/// unattended — nothing is answered in chat, the thread has the compacted
/// shape, and the incident it answered never re-fires it.
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
    let thread = loop {
        // The re-point drops the source's mapping row and then claims the
        // channel for the thread, so a poll landing between the two reads no
        // mapping at all. That gap is the driver mid-flight, not a
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

    assert_compacted_shape(&fixture.store, source, thread).await;
    assert!(
        replies.try_recv().is_err(),
        "the unattended compaction answers nothing in chat"
    );

    // The incident is answered. The marker the thread INHERITED with its
    // second half is older than the thread's own opening, so no amount of
    // later activity re-fires the door on it — without that scoping the
    // thread would compact itself, and its successor again, once per round.
    let inherited_marker = fixture
        .store
        .list_blocks(thread)
        .await
        .expect("the ledger reads")
        .iter()
        .any(|block| Status::KINDS.contains(&block.block_type.as_str()));
    let before = kinds(&fixture.store, thread).await.len();
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&key, ChannelKind::Group, "43", "more chatter"),
            "post-compaction",
        ),
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(thread),
        "an answered incident never compacts the thread that inherited its marker \
         (the marker rode across: {inherited_marker})"
    );
    assert!(
        kinds(&fixture.store, thread).await.len() > before,
        "the message really landed in the thread"
    );

    // The swept source is unmapped, so however many late changes wake the
    // driver it is never compacted again.
    let source_before = kinds(&fixture.store, source).await.len();
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
        Some(thread),
        "an unmapped source is never compacted"
    );
    assert_eq!(
        kinds(&fixture.store, source).await.len(),
        source_before + 1,
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

/// A compacted lineage ONE hop deep, whose summarized half holds one
/// person's words: the channel, the erased principal, the root conversation
/// and the serving thread. The test below erases at this depth; the one after
/// it compacts a second time and erases at two.
async fn compacted_lineage(
    fixture: &support::Fixture,
    replies: &mut Replies,
    id: &str,
) -> (ChannelKey, i64, i64, i64) {
    let key = support::authorized_group(&fixture.assistant, id).await;
    // The person whose words end up in the summarized half.
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
            ToolChoice::KINDS[0],
            CHAT_MESSAGE_KIND,
            ToolCall::KINDS[0],
            ToolResult::KINDS[0],
            Text::KINDS[0],
        ],
    )
    .await;
    let _ = recv_reply(replies).await;
    // A tail of somebody else's chatter, so the cut leaves the erased
    // person's message in the half a digest is written from.
    for index in 0..FILLER_ROWS {
        support::ingest_recorded(
            &fixture.assistant,
            with_origin(
                inbound_unaddressed(&key, ChannelKind::Group, "43", "chatter"),
                &format!("{id}-filler-{index}"),
            ),
        )
        .await;
    }
    invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            COMPACT_COMMAND,
            &format!("{id}-compact"),
        ),
    )
    .await;
    let thread = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    (key, receipt.principal_id, receipt.conversation_id, thread)
}

/// AC5: erasing a principal whose words fed a compacted digest replaces the
/// whole lineage — clone, strip, regenerate, swap, delete — and the prose
/// written from those words stops existing.
///
/// At this depth the lineage is two conversations and both are replaced: the
/// root, whose first half the digest was written from, and the serving thread
/// that carries the digest. What is asserted is the design's own economy —
/// every unchanged row is SHARED by block identity, never copied — and its
/// ordering: the channel is on the scrubbed thread and the two originals are
/// gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasing_a_principal_whose_words_fed_a_digest_replaces_the_lineage() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, erased, source, thread) =
        compacted_lineage(&fixture, &mut replies, "scrub-room").await;
    assert_compacted_shape(&fixture.store, source, thread).await;

    let ancestor_ids: Vec<i64> = fixture
        .store
        .list_blocks(source)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.id)
        .collect();
    let thread_blocks = fixture
        .store
        .list_blocks(thread)
        .await
        .expect("the ledger reads");
    let old_digest = thread_blocks[2].id;
    let erased_ids = principal_blocks(&fixture.store, source, erased).await;
    assert!(
        !erased_ids.is_empty(),
        "the erased person has blocks in the summarized half"
    );

    fixture
        .assistant
        .erase_principal(erased)
        .await
        .expect("the erasure runs");

    let scrubbed = assert_lineage_retired(&fixture, &key, source, thread, old_digest).await;
    let scrubbed_blocks = fixture
        .store
        .list_blocks(scrubbed)
        .await
        .expect("the ledger reads");
    assert_eq!(scrubbed_blocks[1].block_type, AncestorReference::KINDS[0]);
    let ancestor_clone = scrubbed_blocks[1].fields["ancestor_conversation_id"]
        .as_i64()
        .expect("the reference names a conversation");
    assert_ne!(
        ancestor_clone, source,
        "the reference names the scrubbed ancestor, not the deleted one"
    );
    assert_eq!(
        support::block_text(&scrubbed_blocks[2], "content"),
        SCRIPTED_SUMMARY,
        "the regenerated digest is the capture from the scrubbed history"
    );
    assert_ne!(
        scrubbed_blocks[2].id, old_digest,
        "the digest is regenerated, never edited"
    );

    // The ancestor clone is the old ancestor minus the erased blocks, and
    // every surviving row is the SAME BLOCK — shared by identity, not
    // copied.
    let clone_ids: Vec<i64> = fixture
        .store
        .list_blocks(ancestor_clone)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.id)
        .collect();
    let expected: Vec<i64> = ancestor_ids
        .iter()
        .copied()
        .filter(|id| !erased_ids.contains(id))
        .collect();
    assert_eq!(
        clone_ids, expected,
        "the clone shares every unchanged row and drops exactly the erased ones"
    );

    assert_span_partitions(&clone_ids, &scrubbed_blocks, &erased_ids);
}

/// The channel is on a NEW thread, and the two conversations carrying the
/// erased words are gone — with the prose written FROM those words gone with
/// them: the old digest was nobody else's block, so deleting its thread left
/// it to the collector. Answers the scrubbed thread.
async fn assert_lineage_retired(
    fixture: &support::Fixture,
    key: &ChannelKey,
    source: i64,
    thread: i64,
    old_digest: i64,
) -> i64 {
    let scrubbed = mapped_conversation(&fixture.store, key)
        .await
        .expect("the channel is mapped");
    assert_ne!(scrubbed, thread, "the channel took the scrubbed thread");
    for retired in [source, thread] {
        assert!(
            fixture
                .store
                .find_conversation(retired)
                .await
                .expect("the conversation reads")
                .is_none(),
            "the conversation carrying the erased words is deleted"
        );
    }
    assert!(
        fixture
            .store
            .find_block(old_digest)
            .await
            .expect("the block reads")
            .is_none(),
        "the digest written from the erased words is gone"
    );
    scrubbed
}

/// The regeneration span is the complement of what the serving thread
/// inherited, and the complement is a PREFIX of the ancestor clone: nothing
/// drops out of the serving view, nothing is reported twice beside the
/// verbatim second half, and no block of the erased person survives in
/// either.
///
/// The prefix property is the one the mechanism rests on and the one a
/// counting identity cannot see. The regeneration forks the clone up to its
/// LAST non-inherited block, so an inherited block sitting anywhere before
/// that boundary would be summarized AND carried forward verbatim, while a
/// non-inherited block sitting after it would drop out of both. Both are
/// asserted position by position here, against the boundary itself.
///
/// The thread's own four opening blocks — the prompt, the ancestor
/// reference, the summary and the carried tool choice — are what the slice
/// below steps past; everything behind them is inherited.
fn assert_span_partitions(clone_ids: &[i64], scrubbed: &[Block], erased_ids: &[i64]) {
    let inherited: std::collections::HashSet<i64> =
        scrubbed[4..].iter().map(|block| block.id).collect();
    let boundary = clone_ids
        .iter()
        .position(|id| inherited.contains(id))
        .expect("the serving thread inherited part of the scrubbed ancestor");
    assert!(
        boundary > 0,
        "the regeneration covers a non-empty first half: {clone_ids:?}"
    );
    assert!(
        clone_ids[boundary..]
            .iter()
            .all(|id| inherited.contains(id)),
        "the summarized span is a PREFIX: nothing past the boundary is \
         summarized as well as carried verbatim ({clone_ids:?} against \
         {inherited:?})"
    );
    // The tool choice a thread opens with is a record of what the session
    // has, not a word anybody said, and a thread compacted twice inherits
    // the previous thread's record along with the history behind it. It
    // belongs to no ancestor and is summarized by nobody, so it is not part
    // of the history this partition is about.
    assert!(
        scrubbed[4..]
            .iter()
            .filter(|block| block.block_type != ToolChoice::KINDS[0])
            .all(|block| clone_ids.contains(&block.id)),
        "nothing the thread carries verbatim is missing from the ancestor \
         clone: the two together are the whole scrubbed history"
    );
    assert!(
        !scrubbed.iter().any(|block| erased_ids.contains(&block.id)),
        "no block of the erased person survives in the serving thread"
    );
    assert!(
        !clone_ids.iter().any(|id| erased_ids.contains(id)),
        "no block of the erased person survives in the regenerated span"
    );
}

/// A compaction that lands while the SOURCE is mid-turn settles that turn
/// before it copies anything.
///
/// Left running, the turn lands its answer in the source AFTER the second
/// half was copied — in a conversation the swap has just unmapped, which the
/// outbound edge delivers nothing from, so the member's question rides across
/// into the thread while its answer simply vanishes. Worse, the streaming
/// tail the turn had already written would ride across into the thread as
/// shared junction rows, and the source's own finalization deletes those
/// blocks by id, cascading the thread's rows away underneath its born cursor.
///
/// The settle is the same one the erasure runs ahead of its deletions, and
/// what it leaves is what is asserted here: the interrupt went out for the
/// source, its tail is swept with the interrupt's status recorded in its
/// place, and the thread inherits no streaming row at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_compaction_settles_the_sources_open_turn_before_it_copies_anything() {
    let hold = support::TurnHold::new();
    let (fixture, _replies) = held_reset_fixture(&hold).await;
    let key = support::authorized_group(&fixture.assistant, "held-compact-room").await;
    for index in 0..FILLER_ROWS {
        support::ingest_recorded(
            &fixture.assistant,
            with_origin(
                inbound_unaddressed(&key, ChannelKind::Group, "43", "chatter"),
                &format!("held-compact-{index}"),
            ),
        )
        .await;
    }
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "the held ask"),
    )
    .await;
    let source = receipt.conversation_id;
    hold.started().await;
    // The stream is provably open once its tail is in stored state; waiting
    // for that is what makes this deterministic instead of racing the reader.
    support::await_ledger(&fixture.store, source, "the streaming tail", |blocks| {
        blocks.iter().any(|block| block.block_type == "streaming")
    })
    .await;

    let mut events = fixture.bus.subscribe();
    let (answer, _) = invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            COMPACT_COMMAND,
            "held-compact-1",
        ),
    )
    .await;
    assert_eq!(
        answer.as_deref(),
        Some(COMPACT_DONE),
        "the compaction completes over a settled source"
    );

    let mut interrupted = false;
    while let Ok(event) = events.try_recv() {
        if matches!(event, CoreEvent::InterruptRequested { conversation_id } if conversation_id == source)
        {
            interrupted = true;
        }
    }
    assert!(
        interrupted,
        "the source's open turn was interrupted before its history was copied"
    );

    let source_kinds = kinds(&fixture.store, source).await;
    assert!(
        !source_kinds
            .iter()
            .any(|kind| kind.starts_with("streaming")),
        "the interrupt swept the source's tail: {source_kinds:?}"
    );
    assert!(
        source_kinds.iter().any(|kind| kind == Status::KINDS[0]),
        "and recorded its own status in the tail's place: {source_kinds:?}"
    );

    let thread = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    let thread_kinds = kinds(&fixture.store, thread).await;
    assert!(
        !thread_kinds
            .iter()
            .any(|kind| kind.starts_with("streaming")),
        "the thread inherited no streaming row, so nothing the source finalizes \
         can cascade out from under it: {thread_kinds:?}"
    );
}

/// AC5 on a lineage compacted TWICE — the depth a one-hop reading cannot
/// see.
///
/// A thread's digest is written from the half below it, and that half holds
/// the digest of the compaction before it, so prose about an erased person's
/// words survives one generation on as prose about that prose. The person
/// here speaks ONCE, in the root's summarized half, and never again: neither
/// the serving thread nor the thread below it holds a block of theirs, which
/// is exactly the shape a walk that stopped at the newest ancestor would find
/// nothing to scrub in while the derived digest kept serving.
///
/// What is asserted is that the whole chain is replaced: every conversation
/// in it retired, BOTH old digests gone with them, and the thread the channel
/// took standing on a scrubbed ancestry two deep whose root holds none of the
/// erased person's blocks.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_twice_compacted_lineage_loses_every_digest_the_erased_words_reached() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, erased, root, first) =
        compacted_lineage(&fixture, &mut replies, "deep-scrub-room").await;
    let erased_ids = principal_blocks(&fixture.store, root, erased).await;
    assert!(
        !erased_ids.is_empty(),
        "the erased person has blocks in the root's summarized half"
    );
    assert!(
        principal_blocks(&fixture.store, first, erased)
            .await
            .is_empty(),
        "and none at all in the thread above it: the digest is the only trace left"
    );

    // A second round of chatter, so the first thread has two halves of its
    // own, and a second compaction over it.
    for index in 0..FILLER_ROWS {
        support::ingest_recorded(
            &fixture.assistant,
            with_origin(
                inbound_unaddressed(&key, ChannelKind::Group, "43", "more chatter"),
                &format!("deep-scrub-room-second-{index}"),
            ),
        )
        .await;
    }
    invoke(
        &fixture.assistant,
        command_message(
            &key,
            "5",
            Authority::Moderator,
            COMPACT_COMMAND,
            "deep-scrub-room-recompact",
        ),
    )
    .await;
    let serving = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    assert_ne!(serving, first, "the channel took the second compaction");
    assert_compacted_shape(&fixture.store, first, serving).await;
    let first_digest = digest_of(&fixture.store, first).await;
    let serving_digest = digest_of(&fixture.store, serving).await;
    assert!(
        principal_blocks(&fixture.store, serving, erased)
            .await
            .is_empty(),
        "the serving thread holds none of the erased person's blocks either"
    );

    fixture
        .assistant
        .erase_principal(erased)
        .await
        .expect("the erasure runs");

    let scrubbed = mapped_conversation(&fixture.store, &key)
        .await
        .expect("the channel is mapped");
    assert_ne!(scrubbed, serving, "the channel took the scrubbed thread");
    for retired in [root, first, serving] {
        assert!(
            fixture
                .store
                .find_conversation(retired)
                .await
                .expect("the conversation reads")
                .is_none(),
            "every conversation in the lineage is retired, not just the newest two"
        );
    }
    for digest in [first_digest, serving_digest] {
        assert!(
            fixture
                .store
                .find_block(digest)
                .await
                .expect("the block reads")
                .is_none(),
            "a digest the erased words reached — directly or through the digest \
             below it — stops existing"
        );
    }

    assert_rebuilt_ancestry(&fixture, scrubbed, &erased_ids).await;
}

/// Every hop of a scrubbed lineage was REBUILT, not carried across: the
/// serving clone and the clone one hop below it each carry a fresh capture as
/// their digest, and no conversation in the chain — the root's clone
/// included — still holds a block of the erased person.
async fn assert_rebuilt_ancestry(fixture: &support::Fixture, scrubbed: i64, erased_ids: &[i64]) {
    let scrubbed_blocks = fixture
        .store
        .list_blocks(scrubbed)
        .await
        .expect("the ledger reads");
    assert_eq!(
        support::block_text(&scrubbed_blocks[2], "content"),
        SCRIPTED_SUMMARY,
        "the serving digest is a fresh capture"
    );
    let first_clone = ancestor_named_by(&scrubbed_blocks);
    let first_clone_blocks = fixture
        .store
        .list_blocks(first_clone)
        .await
        .expect("the ledger reads");
    assert_eq!(
        support::block_text(&first_clone_blocks[2], "content"),
        SCRIPTED_SUMMARY,
        "the digest one hop down was regenerated too, not carried across"
    );
    let root_clone = ancestor_named_by(&first_clone_blocks);
    for (conversation, what) in [
        (root_clone, "the root's clone"),
        (first_clone, "the middle thread's clone"),
        (scrubbed, "the serving clone"),
    ] {
        let ids: Vec<i64> = fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads")
            .iter()
            .map(|block| block.id)
            .collect();
        assert!(
            !ids.iter().any(|id| erased_ids.contains(id)),
            "{what} holds none of the erased person's blocks"
        );
    }
}

/// The conversation a thread's ancestor-reference block names.
fn ancestor_named_by(blocks: &[Block]) -> i64 {
    assert_eq!(
        blocks[1].block_type,
        AncestorReference::KINDS[0],
        "a compacted thread's first content block names where it came from"
    );
    blocks[1].fields["ancestor_conversation_id"]
        .as_i64()
        .expect("the reference names a conversation")
}

/// The block id of one compacted thread's digest — the compaction message
/// its own opening appended, behind the ancestor reference.
async fn digest_of(store: &Store, conversation_id: i64) -> i64 {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")[2]
        .id
}

/// The blocks in one conversation that record this principal as their
/// author — what an erasure's scrub has to make disappear from a clone.
async fn principal_blocks(store: &Store, conversation_id: i64, principal_id: i64) -> Vec<i64> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| {
            block.block_type == CHAT_MESSAGE_KIND
                && block.fields["principal_id"] == json!(principal_id)
        })
        .map(|block| block.id)
        .collect()
}

/// The scrub's capture-first ordering, at the edge it exists for: a
/// regeneration that captures nothing changes nothing. The lineage stands
/// exactly as it was, the channel stays where it was, and the erasure of
/// the stored data itself still completes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_regeneration_that_captures_nothing_leaves_the_lineage_standing() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, erased, source, thread) =
        compacted_lineage(&fixture, &mut replies, "scrub-fail-room").await;
    let held: Vec<i64> = fixture
        .store
        .list_blocks(thread)
        .await
        .expect("the ledger reads")
        .iter()
        .map(|block| block.id)
        .collect();

    // The regeneration's own turn fails.
    fixture.script.fail_next_turns(1);
    fixture
        .assistant
        .erase_principal(erased)
        .await
        .expect("the erasure runs whatever the scrub does");

    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(thread),
        "a capture that failed swapped nothing"
    );
    assert!(
        fixture
            .store
            .find_conversation(source)
            .await
            .expect("the conversation reads")
            .is_some(),
        "a capture that failed deleted nothing"
    );
    assert_eq!(
        fixture
            .store
            .list_blocks(thread)
            .await
            .expect("the ledger reads")
            .iter()
            .map(|block| block.id)
            .collect::<Vec<i64>>(),
        held,
        "the serving thread stands exactly as it was"
    );
    // The stored data itself is erased regardless: the scrub completes the
    // erasure of a digest, it never delays the erasure of the data.
    assert!(
        fixture
            .store
            .list_blocks(thread)
            .await
            .expect("the ledger reads")
            .iter()
            .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
            .all(|block| block.fields["principal_id"] != json!(erased)
                || block.fields["text"] == json!(null)),
        "the erased person's stored words are nulled whatever the scrub did"
    );
}

/// The recovery the failure above promises, at the one shape that could not
/// reach it: a REPEAT erasure of a principal whose identity row the first
/// call already concluded still scrubs the digest that stayed standing.
///
/// What stands after a failed regeneration is model prose written from the
/// erased person's words, serving the group. The only stated recovery is
/// running the erasure again — and an unflagged person has no identity row
/// left by then, so a run that read its lineages behind the identity lookup
/// would report `NotFound` and walk straight past that prose. The lineages are
/// read off the BLOCKS, which keep the principal id, so the retry lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_repeat_erasure_scrubs_the_digest_the_first_run_left_standing() {
    let (fixture, mut replies) = reset_fixture().await;
    let (key, erased, source, thread) =
        compacted_lineage(&fixture, &mut replies, "scrub-retry-room").await;
    let old_digest = fixture
        .store
        .list_blocks(thread)
        .await
        .expect("the ledger reads")[2]
        .id;

    fixture.script.fail_next_turns(1);
    assert!(
        matches!(
            fixture
                .assistant
                .erase_principal(erased)
                .await
                .expect("the erasure runs"),
            ErasureOutcome::Erased { .. }
        ),
        "the stored data is erased even where the digest cannot be"
    );
    assert_eq!(
        mapped_conversation(&fixture.store, &key).await,
        Some(thread),
        "the unscrubbed thread is still what serves the group"
    );

    // The retry. The person is gone from the identity table, so the erasure
    // itself has nothing to do — and the scrub, which had, runs.
    assert!(
        matches!(
            fixture
                .assistant
                .erase_principal(erased)
                .await
                .expect("the retry runs"),
            ErasureOutcome::NotFound
        ),
        "a concluded principal has nothing left to erase"
    );
    let scrubbed = assert_lineage_retired(&fixture, &key, source, thread, old_digest).await;
    assert_eq!(
        support::block_text(
            &fixture
                .store
                .list_blocks(scrubbed)
                .await
                .expect("the ledger reads")[2],
            "content"
        ),
        SCRIPTED_SUMMARY,
        "the retry regenerated the digest from the scrubbed history"
    );
    assert!(
        principal_blocks(&fixture.store, scrubbed, erased)
            .await
            .is_empty(),
        "and no block of the erased person survives in what serves the group"
    );
}
