//! The reaction at the core's edges (unit 39): the model calls the react
//! tool naming a message it is reading and the emoji it chose, the block
//! files with all three stored values, the outbound edge yields the mark
//! arm beside the answer, the tool's guards decline with their pinned
//! copy, a reaction buries no unanswered question, and both nulling paths
//! — the marked person's erasure and the deletion mirror — leave the edge
//! nothing to place.

use agent_ledger::{ProviderModule, Store};
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::mark;
use assistant_core::tools::report;
use assistant_core::{
    Authority, ChannelKey, ChannelKind, ErasureOutcome, IngestReceipt, Outbound, ProtectionConfig,
    ReplyKind,
};
use serde_json::json;
use tokio::sync::mpsc;

use crate::support::{
    self, CLOSING_ANSWER, ToolScript, channel, field, inbound, recv_mark, recv_reply, settle_shape,
    tool_scripted_provider, with_origin,
};

/// The outbound edge a fixture's items arrive on.
type Outgoing = mpsc::UnboundedReceiver<Outbound>;

/// The emoji every fixture in this module has the model choose, written as
/// an escape sequence and never as a pasted glyph: a literal is what
/// silently gains a variation selector on its way through an editor, and
/// this suite asserts stored bytes.
const CHOSEN: &str = "\u{1F389}";

/// The one-reaction turn's ledger shape on a fresh conversation: the
/// message summons the turn, the call names it, the mark files, the result
/// records, the turn closes.
const REACTING_TURN: [&str; 7] = [
    "system_prompt",
    "tool_choice",
    "chat_message",
    "tool_call",
    "message_mark",
    "tool_result",
    "text",
];

/// One assembled reaction fixture: the assistant over a provider scripted
/// to call the react tool with the given input, plus the outbound edge.
/// The tool itself is registered by the assembly unconditionally, so
/// nothing here configures it — which is the registration's own proof.
async fn reacting_fixture(input: String) -> (support::Fixture, Outgoing) {
    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: mark::NAME.into(),
            input,
            narration: None,
        },
        None,
    );
    assemble(provider, handle).await
}

/// The assembly preamble every fixture here shares.
async fn assemble(
    provider: Box<dyn ProviderModule>,
    handle: support::ScriptHandle,
) -> (support::Fixture, Outgoing) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        provider,
        handle,
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

/// The call input naming one origin and the suite's chosen emoji, built
/// through the serializer so the escape survives as itself.
fn call_input(origin: &str) -> String {
    json!({ "message_id": origin, "emoji": CHOSEN }).to_string()
}

/// Record one member message under an exact origin, addressed so it
/// summons a turn.
async fn record_message(
    fixture: &support::Fixture,
    key: &ChannelKey,
    sender: &str,
    origin: &str,
) -> IngestReceipt {
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(key, ChannelKind::Direct, sender, "we shipped it"),
            origin,
        ),
    )
    .await
}

// ─── AC-D: the filing and the placement, end to end ──────────────────────

/// The whole flow, block by block: a member's message summons a turn, the
/// model calls the react tool naming that message and an emoji, the mark
/// block files with the target origin, the marked person and the emoji
/// stored verbatim, the tool result is the pinned filed copy, and the edge
/// yields the mark arm — carrying the LIST-free core's own bytes and the
/// target — before the answer's reply arm.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reaction_files_its_block_and_reaches_the_edge_as_its_own_arm() {
    let (fixture, mut outgoing) = reacting_fixture(call_input("origin-share-1")).await;
    let key = channel("dm-reaction");
    let receipt = record_message(&fixture, &key, "member-1", "origin-share-1").await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the reacting turn",
        &REACTING_TURN,
    )
    .await;

    let stored = &blocks[4];
    assert_eq!(field(stored, "target_origin"), "origin-share-1");
    assert_eq!(
        stored.fields["marked_principal_id"],
        json!(receipt.principal_id),
        "the block names the marked message's sender, so erasure reaches it"
    );
    assert_eq!(
        field(stored, "emoji"),
        CHOSEN,
        "the core stored the model's emoji verbatim, as content"
    );
    assert_eq!(
        field(&blocks[5], "content"),
        mark::MARKED_RESULT,
        "the model reads the filed result, not a delivery report"
    );

    // The edge: the mark first — ledger order puts the tool's block ahead
    // of the answer it filed during — then the answer's own arm.
    let placed = recv_mark(&mut outgoing).await;
    assert_eq!(placed.channel, key);
    assert_eq!(placed.target_origin, "origin-share-1");
    assert_eq!(
        placed.emoji, CHOSEN,
        "the core hands the adapter the stored emoji and decides nothing about it"
    );
    let answer = recv_reply(&mut outgoing).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    assert_eq!(answer.text, support::disclosed(CLOSING_ANSWER));
    let extra = outgoing.try_recv();
    assert!(
        extra.is_err(),
        "one turn, one reaction, one answer: {extra:?}"
    );
}

/// One react call, twice in one round, both times naming the same message
/// — the parallel same-origin shape the shared filing door exists for,
/// since the runner executes a round's calls in parallel tasks.
fn twin_call_provider(input: String) -> (Box<dyn ProviderModule>, support::ScriptHandle) {
    support::same_round_calls_provider(vec![
        support::RoundCall {
            tool: mark::NAME.into(),
            input: input.clone(),
        },
        support::RoundCall {
            tool: mark::NAME.into(),
            input,
        },
    ])
}

/// The per-origin bound end to end, under the hardest shape it faces: two
/// calls naming one message in a single round, executed in parallel tasks.
/// Exactly one mark block files, the other call reads the pinned duplicate
/// decline, and exactly one reaction reaches the edge. Without the shared
/// filing door both calls would scan before either appended and the message
/// would carry two reactions.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_reactions_on_one_message_file_once_and_the_second_declines() {
    let (provider, handle) = twin_call_provider(call_input("origin-share-2"));
    let (fixture, mut outgoing) = assemble(provider, handle).await;
    let key = channel("dm-reaction-twice");
    let receipt = record_message(&fixture, &key, "member-2", "origin-share-2").await;

    let blocks = support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the twin-call turn",
        |blocks| {
            blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == mark::MESSAGE_MARK_KIND)
            .count(),
        1,
        "one message, one filed reaction, however many calls named it"
    );
    let outcomes: Vec<String> = blocks
        .iter()
        .filter_map(|block| match block.block_type.as_str() {
            "tool_result" => Some(field(block, "content")),
            "tool_error" => Some(field(block, "error")),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes.len(), 2, "both calls answered the model");
    assert!(
        outcomes.contains(&mark::MARKED_RESULT.to_owned()),
        "one call filed: {outcomes:?}"
    );
    assert!(
        outcomes.contains(&mark::ALREADY_MARKED_ERROR.to_owned()),
        "the other read the pinned duplicate decline: {outcomes:?}"
    );

    assert_eq!(
        recv_mark(&mut outgoing).await.target_origin,
        "origin-share-2"
    );
    let answer = recv_reply(&mut outgoing).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    let extra = outgoing.try_recv();
    assert!(extra.is_err(), "exactly one reaction was placed: {extra:?}");
}

/// The shared filing door across the two tools that file against a message
/// origin: one round names one message with react AND with `report_spam`,
/// and the runner executes the pair in parallel tasks.
///
/// What the door guarantees, and what this pins: neither call sees a
/// half-filed ledger. At most one reaction and at most one report stand,
/// and a reaction NEVER files behind a standing report — the direction the
/// design enforces, checked here by ledger order and by the pinned
/// overlap decline. Without the door both calls scan before either
/// appends, and the reaction lands on a message the same round reported.
///
/// The other interleaving — the reaction filing first, the report landing
/// beside it — is left standing ON PURPOSE, and this test asserts it is
/// not treated as a defect. The governing reaction design of 2026-08-25
/// rules the refusal one-way and names "refusing a report whose origin
/// already carries a mark" as a REJECTED alternative: a cosmetic
/// acknowledgement must not suppress a moderation assessment, which is
/// decision 0070's direction. So "exactly one of the two lands, whichever"
/// is not what the tree may promise — closing that half means superseding
/// that ruling in the design document, not tightening a tool.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reaction_and_a_report_naming_one_message_never_cross_half_filed() {
    let (provider, handle) = support::same_round_calls_provider(vec![
        support::RoundCall {
            tool: mark::NAME.into(),
            input: call_input("origin-both"),
        },
        support::RoundCall {
            tool: report::NAME.into(),
            input: json!({ "message_id": "origin-both" }).to_string(),
        },
    ]);
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_reporting(
        store,
        provider,
        handle,
        ToolSet::new(),
        ProtectionConfig::default(),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-reaction-and-report").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            support::inbound_unaddressed(&key, ChannelKind::Group, "member-7", "we shipped it"),
            "origin-both",
        ),
    )
    .await;

    let blocks = support::await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the react-and-report round",
        |blocks| {
            blocks
                .last()
                .is_some_and(|block| block.block_type == "text")
        },
    )
    .await;
    let filed = |kind: &str| -> Vec<i64> {
        blocks
            .iter()
            .filter(|block| block.block_type == kind)
            .map(|block| block.id)
            .collect()
    };
    let marks = filed(mark::MESSAGE_MARK_KIND);
    let reports = filed(report::REPORT_KIND);
    assert!(marks.len() <= 1, "one message, at most one reaction");
    assert!(reports.len() <= 1, "one message, at most one report");
    assert!(
        !marks.is_empty() || !reports.is_empty(),
        "the round filed nothing at all, so it proves nothing about the door"
    );

    let outcomes: Vec<String> = blocks
        .iter()
        .filter_map(|block| match block.block_type.as_str() {
            "tool_result" => Some(field(block, "content")),
            "tool_error" => Some(field(block, "error")),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes.len(), 2, "both calls answered the model");

    match (marks.first(), reports.first()) {
        (Some(mark_id), Some(report_id)) => {
            assert!(
                mark_id < report_id,
                "the reaction filed first and the report landed beside it, which the \
                 one-way rule allows; a reaction filed BEHIND a standing report would \
                 mean the two calls crossed half-filed"
            );
            assert!(
                outcomes.contains(&mark::MARKED_RESULT.to_owned()),
                "the reaction that won the door read the filed result: {outcomes:?}"
            );
        }
        (None, Some(_)) => assert!(
            outcomes.contains(&mark::ALREADY_REPORTED_ERROR.to_owned()),
            "the report won the door and the reaction read the pinned overlap \
             decline: {outcomes:?}"
        ),
        (Some(_), None) => panic!(
            "the reaction filed and the report did not, though nothing declines a report \
             on a marked message: {outcomes:?}"
        ),
        (None, None) => unreachable!("the non-vacuity assertion above covers this"),
    }
}

/// The anti-aiming bound end to end: a call naming an id the turn is not
/// reading declines with the pinned copy and files nothing. This is also
/// the refusal that answers an attempt at the assistant's own message —
/// her voice writes no chat rows, so no id of hers is ever in the set —
/// which is why no separate own-message decline exists to pin.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reaction_aimed_outside_the_turn_declines_and_files_nothing() {
    let (fixture, mut outgoing) = reacting_fixture(call_input("origin-not-here")).await;
    let key = channel("dm-reaction-aimed");
    let receipt = record_message(&fixture, &key, "member-3", "origin-share-4").await;

    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the declined turn",
        &[
            "system_prompt",
            "tool_choice",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[4], "error"), mark::NOT_ASSESSED_ERROR);
    assert!(
        blocks
            .iter()
            .all(|block| block.block_type != mark::MESSAGE_MARK_KIND),
        "the declined call filed nothing"
    );
    let answer = recv_reply(&mut outgoing).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    let extra = outgoing.try_recv();
    assert!(extra.is_err(), "nothing was placed: {extra:?}");
}

// ─── Transparency: a reaction buries no unanswered question ──────────────

/// The read-through, at the seam that matters: an unanswered member
/// question stands behind a filed reaction, and the next message still
/// carries the debt forward — its stored answer-due stamp true, composed
/// against a tail the walk read THROUGH the reaction to reach. Without the
/// mark kind in the consumer's read-through list the walk would settle on
/// the reaction and the standing question would stop owing, which is
/// exactly the burial a reaction is most exposed to: it is placed on turns
/// that answer nothing, so it stands at the tail more often than anything
/// else the list holds.
///
/// The provider is silent, so the owed message stays owed and no answer
/// races the stamp under test. The follow-up summons nothing of its own —
/// the fixture answers addressed messages only, and this one is not — so
/// its answer-due can be the propagated debt and nothing else.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unanswered_question_behind_a_reaction_still_owes_its_turn() {
    let fixture = support::start_assistant_config(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        ToolSet::new(),
        assistant_core::AssemblyConfig {
            answering: assistant_core::AnsweringMode::Addressed,
            ..support::assembly_config()
        },
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-reaction-debt").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&key, ChannelKind::Group, "asker", "the owed ask"),
            "origin-owed-ask",
        ),
    )
    .await;
    let conversation = receipt.conversation_id;

    fixture
        .store
        .append_consumer_block(
            conversation,
            None,
            mark::MESSAGE_MARK_KIND,
            mark::MessageMark::stored_fields("origin-owed-ask", receipt.principal_id, CHOSEN),
            None,
        )
        .await
        .expect("the mark appends on top");
    let tail = fixture
        .store
        .latest_block(conversation)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(
        tail.block_type,
        mark::MESSAGE_MARK_KIND,
        "non-vacuity: the reaction really is the tail the walk must read through"
    );

    support::ingest_recorded(
        &fixture.assistant,
        support::inbound_unaddressed(
            &key,
            ChannelKind::Group,
            "asker",
            "an aside behind the reaction",
        ),
    )
    .await;
    let blocks = store_blocks(&fixture, conversation).await;
    let aside = blocks
        .iter()
        .find(|block| block.fields.get("text") == Some(&json!("an aside behind the reaction")))
        .expect("the aside is recorded");
    assert_eq!(
        aside.fields["answer_due"],
        json!(true),
        "the debt behind the reaction reached the next message's stamp"
    );
}

/// The ledger of one conversation, or the loud read failure.
async fn store_blocks(fixture: &support::Fixture, conversation: i64) -> Vec<agent_ledger::Block> {
    fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads")
}

// ─── AC-E's mechanism: erasure and the deletion mirror ───────────────────

/// The marked person's erasure nulls the block's target while the emoji
/// stays — it records what the ASSISTANT expressed and names nobody — and
/// a reaction still unplaced at that moment reaches the platform as
/// nothing: the edge skips the targetless mark. A second erasure changes
/// nothing, which is the idempotency the operation promises.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_marked_persons_erasure_nulls_the_target_and_the_edge_skips_it() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_on(store, None).await;
    // A GROUP conversation on purpose: erasing a person removes their
    // DIRECT conversations whole, which would take the mark block with
    // them and prove nothing about the pass under test.
    let key = support::authorized_group(&fixture.assistant, "room-reaction-erased").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            support::inbound_unaddressed(&key, ChannelKind::Group, "member-4", "we shipped it"),
            "origin-erased",
        ),
    )
    .await;
    let conversation = receipt.conversation_id;
    fixture
        .store
        .append_consumer_block(
            conversation,
            None,
            mark::MESSAGE_MARK_KIND,
            mark::MessageMark::stored_fields("origin-erased", receipt.principal_id, CHOSEN),
            None,
        )
        .await
        .expect("the mark appends");

    let outcome = fixture
        .assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));
    assert_marked_state(&fixture, conversation, receipt.principal_id, "the erasure").await;

    // The repeat: the identity row is gone, so the operation reports
    // not-found and touches nothing — the block stands exactly as the
    // first pass left it.
    let repeat = fixture
        .assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("the repeat erasure runs");
    assert_eq!(repeat, ErasureOutcome::NotFound);
    assert_marked_state(&fixture, conversation, receipt.principal_id, "the repeat").await;
}

/// What an erased mark block must show, asserted after each pass: the
/// message reference emptied, the erasure-keyed identifier surviving, and
/// the emoji untouched — the three facts the records of processing state
/// for the mark's erasure row.
async fn assert_marked_state(
    fixture: &support::Fixture,
    conversation: i64,
    principal_id: i64,
    after: &str,
) {
    let blocks = store_blocks(fixture, conversation).await;
    let stored = blocks
        .iter()
        .find(|block| block.block_type == mark::MESSAGE_MARK_KIND)
        .expect("the mark block stands");
    assert!(
        stored.fields.get("target_origin").is_none(),
        "{after}: the marked message's reference is empty"
    );
    assert_eq!(
        stored.fields["marked_principal_id"],
        json!(principal_id),
        "{after}: the erasure-keyed identifier survives, as the record states"
    );
    assert_eq!(
        field(stored, "emoji"),
        CHOSEN,
        "{after}: the emoji stays; it names nobody"
    );
}

/// The nulled mark is skipped at the edge and accounted delivered: a
/// re-read does not meet it again, and nothing reaches the channel for it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_targetless_reaction_places_nothing() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_on(store, None).await;
    let key = channel("dm-reaction-targetless");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&key, ChannelKind::Direct, "member-5", "we shipped it"),
            "origin-targetless",
        ),
    )
    .await;
    let mut outgoing = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    // The answer to the recorded message is what proves the edge is live;
    // the mark appended behind it carries no target at all.
    let answer = recv_reply(&mut outgoing).await;
    assert_eq!(answer.kind, ReplyKind::Answer);
    fixture
        .store
        .append_consumer_block(
            receipt.conversation_id,
            None,
            mark::MESSAGE_MARK_KIND,
            {
                let mut fields =
                    mark::MessageMark::stored_fields("origin-gone", receipt.principal_id, CHOSEN);
                fields.remove("target_origin");
                fields
            },
            None,
        )
        .await
        .expect("the targetless mark appends");
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "member-5", "one more"),
    )
    .await;

    // The next thing on the edge is the second answer: the targetless
    // reaction was skipped, not placed and not held.
    let next = recv_reply(&mut outgoing).await;
    assert_eq!(
        next.kind,
        ReplyKind::Answer,
        "the targetless reaction never reached the channel"
    );
}

/// The deletion mirror reaches the reaction: an administrator's reply
/// deletion command nulls the mark's stored target for the deleted
/// message, an unknown origin stays a full no-op, and a second run changes
/// nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_deletion_mirror_nulls_the_reactions_target() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_on(store, None).await;
    let key = support::authorized_group(&fixture.assistant, "room-reaction-deleted").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            support::inbound_unaddressed(&key, ChannelKind::Group, "member-6", "we shipped it"),
            "origin-deleted",
        ),
    )
    .await;
    let conversation = receipt.conversation_id;
    fixture
        .store
        .append_consumer_block(
            conversation,
            None,
            mark::MESSAGE_MARK_KIND,
            mark::MessageMark::stored_fields("origin-deleted", receipt.principal_id, CHOSEN),
            None,
        )
        .await
        .expect("the mark appends");

    // An unknown origin first: the command is recognized and the mirror
    // finds nothing, so every stored target stands.
    support::ingest_recorded(
        &fixture.assistant,
        support::deletion_reply(&key, "admin-1", Authority::Admin, "origin-never-here"),
    )
    .await;
    assert_eq!(
        marked_target(&fixture, conversation).await.as_deref(),
        Some("origin-deleted"),
        "an unknown target leaves the mirror a full no-op"
    );

    for pass in 1..=2 {
        support::ingest_recorded(
            &fixture.assistant,
            support::deletion_reply(&key, "admin-1", Authority::Admin, "origin-deleted"),
        )
        .await;
        assert_eq!(
            marked_target(&fixture, conversation).await,
            None,
            "pass {pass}: the mirror nulled the reaction's copy of the deleted id"
        );
    }
}

/// The stored target of the one mark block in a conversation.
async fn marked_target(fixture: &support::Fixture, conversation: i64) -> Option<String> {
    store_blocks(fixture, conversation)
        .await
        .iter()
        .find(|block| block.block_type == mark::MESSAGE_MARK_KIND)
        .expect("the mark block stands")
        .fields
        .get("target_origin")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}
