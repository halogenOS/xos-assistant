//! Speaking as an ACT (unit 55, 2026-09-02): what becomes of the call
//! behind a message that did not reach the chat whole, what a turn that
//! writes and sends nothing costs, and the record that tells an old
//! conversation the contract changed.
//!
//! The delivered path is pinned where the delivery lives — `delivery`, for
//! the receipt, `threading` for the target the model aims at, `end_to_end`
//! for the round trip. What lives here is everything a send can come to
//! BESIDES arriving: the platform refusing it, the platform taking part of
//! it, the process dying with it, the conversation being retired under it —
//! and the two readings that key on delivery rather than on words, the
//! budget's counted debt and the contract notice.
//!
//! Every settlement is read off the ledger as a tool result or a tool
//! error paired with the send's own call, because that is the only thing
//! the model ever learns about its message: the call it made either
//! completed with ids or failed with a reason.

use agent_ledger::{Block, LeafKind, Store};
use assistant_core::schema::store_config;
use assistant_core::{ChannelKind, Outbound, ProtectionConfig, SendOutcome};

use crate::support::{self, channel, inbound};

/// The reason the stand-in platform gives for refusing a send — a fixture
/// string, and the exact text the failed call must carry back.
const REFUSED: &str = "the platform refused the request";

/// What one conversation's sends came to, in ledger order, read off the
/// store — the suite's one pairing of a send with its settlement lives in
/// `support`, where the consumer view already needs it, and this reads it
/// there instead of pairing a second time.
async fn send_outcomes(store: &Store, conversation_id: i64) -> Vec<String> {
    support::send_settlements(
        &store
            .list_blocks(conversation_id)
            .await
            .expect("the ledger reads"),
    )
}

/// The outgoing blocks one conversation holds, oldest first.
async fn sent_blocks(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND)
        .collect()
}

/// An assembly with NOBODY reporting deliveries, over a fresh store: the
/// fixture every case here needs, because each one answers the send itself
/// — or leaves it unanswered on purpose.
async fn unreported_fixture() -> support::Fixture {
    unreported_over(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        ProtectionConfig::default(),
    )
    .await
}

/// The same assembly over a given store and budgets.
async fn unreported_over(store: Store, protection: ProtectionConfig) -> support::Fixture {
    let (provider, script) = support::scripted_provider(None);
    let mut config = support::assembly_config();
    config.protection = protection;
    support::start_assistant_unreported(
        store,
        provider,
        script,
        assistant_core::tools::ToolSet::new(),
        config,
    )
    .await
}

/// Await the one message a turn filed, and hand back the reply the edge
/// carried it out on — what the adapter would answer for.
async fn one_reply(items: &mut tokio::sync::mpsc::UnboundedReceiver<Outbound>) -> Outbound {
    tokio::time::timeout(support::DEADLINE, items.recv())
        .await
        .expect("the send reaches the edge before the deadline")
        .expect("the edge outlives the test")
}

/// AC4, the refused send: the platform took nothing, so the call FAILS with
/// the platform's own reason, nothing is recorded as delivered, and the
/// block stands in the ledger as what was attempted.
///
/// The failure is what the model reads: a send whose call quietly completed
/// would have it believe the group is holding words nobody ever saw.
///
/// The typing cue's stop is read here too (AC12), through
/// [`support::SendEndings`], which states why the raw carrier and not a
/// composing edge answers it. What is asserted is that the ENDING was
/// raised, and that it was raised by this report — nothing had reported
/// before it. Narrowing the receipt door's stop to delivered sends leaves
/// the await below with nothing to receive and fails the case.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_send_fails_its_call_with_the_platforms_reason() {
    let fixture = unreported_fixture().await;
    let mut endings = support::SendEndings::watching(&fixture.assistant);
    let mut items = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-send-refused");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "does this one arrive?"),
    )
    .await;
    let Outbound::Reply(reply) = one_reply(&mut items).await else {
        panic!("a send reaches the edge as a reply");
    };
    endings.none_yet();

    fixture
        .assistant
        .report_delivery(
            reply.delivery,
            &[],
            &SendOutcome::Failed {
                reason: REFUSED.into(),
            },
        )
        .await;
    endings
        .one_for(
            asked.conversation_id,
            "a send the platform refused is done, so the chat stops showing the \
             assistant typing",
        )
        .await;

    let outcomes = support::await_ledger(
        &fixture.store,
        asked.conversation_id,
        "the refused send's settlement",
        |blocks| blocks.iter().any(|block| block.block_type == "tool_error"),
    )
    .await;
    assert_eq!(
        send_outcomes(&fixture.store, asked.conversation_id).await,
        vec![assistant_core::outgoing::send_failed(REFUSED)],
        "the call fails, carrying the reason the platform gave"
    );
    assert!(
        !outcomes
            .iter()
            .any(|block| block.block_type == assistant_core::delivery::DELIVERED_KIND),
        "a send that put nothing in the chat records no delivery"
    );
    assert_eq!(
        sent_blocks(&fixture.store, asked.conversation_id)
            .await
            .len(),
        1,
        "the message the model asked for stands in the ledger as what was \
         attempted; the failure is on the call, not on the record"
    );
}

/// AC4, the cut-short send: some of the message posted and the rest did
/// not, so the call FAILS with a sentence naming the ids that reached the
/// chat — and exactly those ids are recorded as delivered.
///
/// Both halves matter to the model. What the group read is not what it
/// wrote, and a member replying to the part that posted replies to one of
/// those ids.
///
/// The typing cue's stop is read here too (AC12), on the same terms as the
/// refused send above and through the same observation. A send that posted
/// half a message is done, and the chat stops showing the assistant typing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cut_short_send_fails_its_call_with_the_ids_that_posted() {
    let fixture = unreported_fixture().await;
    let mut endings = support::SendEndings::watching(&fixture.assistant);
    let mut items = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-send-cut-short");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "does all of it arrive?"),
    )
    .await;
    let Outbound::Reply(reply) = one_reply(&mut items).await else {
        panic!("a send reaches the edge as a reply");
    };

    let posted = vec!["71".to_owned()];
    endings.none_yet();
    fixture
        .assistant
        .report_delivery(
            reply.delivery,
            &posted,
            &SendOutcome::Failed {
                reason: REFUSED.into(),
            },
        )
        .await;
    endings
        .one_for(
            asked.conversation_id,
            "a send that posted part of its message is done too",
        )
        .await;

    let blocks = support::await_ledger(
        &fixture.store,
        asked.conversation_id,
        "the cut-short send's settlement",
        |blocks| blocks.iter().any(|block| block.block_type == "tool_error"),
    )
    .await;
    assert_eq!(
        send_outcomes(&fixture.store, asked.conversation_id).await,
        vec![assistant_core::outgoing::send_cut_short(&posted, REFUSED)],
        "the call fails, naming what did reach the chat"
    );
    let receipts: Vec<Option<String>> = blocks
        .iter()
        .filter(|block| block.block_type == assistant_core::delivery::DELIVERED_KIND)
        .map(|block| assistant_core::delivery::Delivered::parse(block).origin)
        .collect();
    assert_eq!(
        receipts,
        vec![Some("71".to_owned())],
        "exactly the message that reached the chat is recorded"
    );
}

/// AC12's first stop, the DELIVERED one: a whole send is done when the
/// receipt says so, and that report is what ended the cue — nothing had
/// declared this send done before it.
///
/// The stop is attributable because it is read where the receipt door and
/// the refusing tool write, not off a composing edge: the edge stops on the
/// round's own ending too, so a stop seen there would prove nothing about
/// the send. The call's own settlement is asserted beside it, so the case
/// reads one delivered send end to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_delivered_sends_report_is_what_stops_the_cue() {
    let fixture = unreported_fixture().await;
    let mut endings = support::SendEndings::watching(&fixture.assistant);
    let mut items = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-send-delivered-cue");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "and this one arrives?"),
    )
    .await;
    let Outbound::Reply(reply) = one_reply(&mut items).await else {
        panic!("a send reaches the edge as a reply");
    };
    endings.none_yet();

    fixture
        .assistant
        .report_delivery(reply.delivery, &["83".to_owned()], &SendOutcome::Whole)
        .await;
    endings
        .one_for(
            asked.conversation_id,
            "the delivered send's own report is what declared it done",
        )
        .await;

    support::await_ledger(
        &fixture.store,
        asked.conversation_id,
        "the delivered send's settlement",
        |blocks| blocks.iter().any(|block| block.block_type == "tool_result"),
    )
    .await;
    assert_eq!(
        send_outcomes(&fixture.store, asked.conversation_id).await,
        vec![assistant_core::outgoing::sent_result(&["83".to_owned()])],
        "the call completes with the id the message was posted under"
    );
}

/// AC4, the turn that says nothing: a turn whose text is NON-EMPTY delivers
/// nothing at all. Written text is notes, and notes reach nobody.
///
/// The marker conversation is what makes the silence mean something: the
/// edge is ordered, so the marker's own send arriving first proves the
/// notes turn produced nothing rather than merely not having produced it
/// yet.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_of_notes_writes_real_text_and_delivers_nothing() {
    let fixture = support::start_assistant(None).await;
    let mut items = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let quiet = channel("dm-notes-only");
    let marker = channel("dm-notes-marker");

    let notes = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &quiet,
            ChannelKind::Direct,
            "42",
            &format!("write this down {cue}", cue = support::NOTES_CUE),
        ),
    )
    .await;
    let written = support::settle(&fixture.store, notes.conversation_id, "the notes turn", 4).await;
    let notes_text = written
        .iter()
        .find(|block| block.block_type == "text")
        .and_then(|block| block.fields["content"].as_str())
        .expect("the notes turn wrote text");
    assert!(
        !notes_text.is_empty(),
        "non-vacuity: the turn under test wrote real words"
    );
    assert!(
        sent_blocks(&fixture.store, notes.conversation_id)
            .await
            .is_empty(),
        "a turn that sent nothing filed no message"
    );

    support::ingest_recorded(
        &fixture.assistant,
        inbound(&marker, ChannelKind::Direct, "43", "and this one speaks"),
    )
    .await;
    let Outbound::Reply(first) = one_reply(&mut items).await else {
        panic!("a send reaches the edge as a reply");
    };
    assert_eq!(
        first.channel, marker,
        "the marker's message is the FIRST thing the edge carries: the \
         notes turn put nothing on it"
    );
}

/// AC4, the startup sweep: an outgoing block whose call is still open at
/// process start is failed with the restart sentence before anything is
/// served, and it is never delivered afterwards.
///
/// A send the process died with cannot arrive late — the edge's startup
/// seed marks everything already stored as history — so the model is told
/// plainly that its message counts as unsent instead of waiting forever on
/// a call nothing will answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unfinished_send_is_failed_at_startup_and_never_delivered() {
    let fixture = unreported_fixture().await;
    let key = channel("dm-send-unfinished");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the interrupted send"),
    )
    .await;
    // Nobody has an edge open, so the filed message sits undelivered and
    // its call stays pending — the state a process death leaves behind.
    support::await_ledger(
        &fixture.store,
        asked.conversation_id,
        "the filed send",
        |blocks| {
            blocks
                .iter()
                .any(|block| block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND)
        },
    )
    .await;

    assert_eq!(
        fixture
            .assistant
            .fail_unfinished_sends()
            .await
            .expect("the sweep reads the ledger"),
        1,
        "the one send nothing confirmed is settled"
    );
    assert_eq!(
        send_outcomes(&fixture.store, asked.conversation_id).await,
        vec![assistant_core::outgoing::RESTARTED_BEFORE_CONFIRMED.to_owned()],
        "the model is told its message counts as unsent"
    );
    assert_eq!(
        fixture
            .assistant
            .fail_unfinished_sends()
            .await
            .expect("the second sweep reads the ledger"),
        0,
        "a settled send is swept a second time for nothing"
    );

    // Taken AFTER the sweep, the way the binary takes its edges: the
    // undelivered block is history to it, so nothing goes out.
    let mut items = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), items.recv())
            .await
            .is_err(),
        "the swept send is never delivered"
    );
}

/// AC4, the retirement: a conversation retired under a pending send fails
/// that send with the retirement sentence, before the fork carries the
/// channel on.
///
/// The block was filed into a session the channel no longer serves, so the
/// message will not happen — and the call behind it would otherwise stay
/// open in a conversation nothing reads.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_retired_conversations_pending_send_is_failed_with_the_retirement_sentence() {
    let db = support::TempDb::new("send-retired");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");
    let fixture = unreported_over(store.clone(), ProtectionConfig::default()).await;
    let key = channel("dm-send-retired");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &key,
            ChannelKind::Direct,
            "42",
            "the send under a retirement",
        ),
    )
    .await;
    support::await_ledger(
        &fixture.store,
        asked.conversation_id,
        "the filed send",
        |blocks| {
            blocks
                .iter()
                .any(|block| block.block_type == assistant_core::outgoing::OUTGOING_MESSAGE_KIND)
        },
    )
    .await;

    // The restart a prompt edit produces: the channel's conversation
    // recorded the old wording, so it retires — with the send still open in
    // it.
    let mut edited = support::assembly_config();
    edited.system_prompt = "a different system prompt entirely".into();
    let (provider, script) = support::scripted_provider(None);
    let restarted = support::start_assistant_unreported(
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
        "the one channel serving the old prompt retires"
    );

    assert_eq!(
        send_outcomes(&store, asked.conversation_id).await,
        vec![assistant_core::outgoing::RETIRED_BEFORE_CONFIRMED.to_owned()],
        "the retired conversation's open send is failed with its own sentence"
    );
}

/// AC13: a debt counts against a budget when its turn DELIVERED a message,
/// and not otherwise. One answer per window, and three turns spend it in
/// three different ways.
///
/// A turn of notes costs nothing — the person was answered with silence, so
/// nothing was spent on them — while a turn that put words in the chat
/// spends the window's one slot and the next ask is refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_of_notes_spends_no_budget_and_a_delivered_send_spends_one() {
    let fixture = support::start_assistant_configured(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        None,
        support::budgets(Some((1, 600)), None),
    )
    .await;
    let key = channel("dm-budget-notes");

    // The notes turn: real text, nothing sent, nothing spent.
    let notes = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &key,
            ChannelKind::Direct,
            "42",
            &format!("think about this {cue}", cue = support::NOTES_CUE),
        ),
    )
    .await;
    support::settle(&fixture.store, notes.conversation_id, "the notes turn", 4).await;
    assert_eq!(
        recorded_limit(&fixture.store, notes.conversation_id, 0).await,
        None,
        "non-vacuity: the first message was admitted"
    );

    // The next ask is admitted too, which is the whole reading: the notes
    // turn spent nothing.
    let spoken = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "now answer me"),
    )
    .await;
    assert_eq!(
        recorded_limit(&fixture.store, spoken.conversation_id, 1).await,
        None,
        "the turn of notes left the person's one answer unspent"
    );
    support::await_ledger(
        &fixture.store,
        spoken.conversation_id,
        "the delivered send",
        |blocks| {
            blocks
                .iter()
                .any(|block| block.block_type == assistant_core::delivery::DELIVERED_KIND)
        },
    )
    .await;

    // And now the window is spent: the delivered send counted.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "and once more"),
    )
    .await;
    assert_eq!(
        recorded_limit(&fixture.store, spoken.conversation_id, 2).await,
        Some("principal".to_owned()),
        "the delivered message spent the person's one answer"
    );
}

/// AC13's other half: a turn whose only send FAILED spends nothing. The
/// person's words never reached the group, so nothing was spent on them.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_whose_only_send_failed_spends_no_budget() {
    let fixture = unreported_over(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::budgets(Some((1, 600)), None),
    )
    .await;
    let mut items = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-budget-failed");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the send that fails"),
    )
    .await;
    let Outbound::Reply(reply) = one_reply(&mut items).await else {
        panic!("a send reaches the edge as a reply");
    };
    fixture
        .assistant
        .report_delivery(
            reply.delivery,
            &[],
            &SendOutcome::Failed {
                reason: REFUSED.into(),
            },
        )
        .await;
    support::await_ledger(
        &fixture.store,
        asked.conversation_id,
        "the failed send's settlement",
        |blocks| blocks.iter().any(|block| block.block_type == "tool_error"),
    )
    .await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "ask again"),
    )
    .await;
    assert_eq!(
        recorded_limit(&fixture.store, asked.conversation_id, 1).await,
        None,
        "a message that never reached the chat spent nothing"
    );
}

/// The limit stamped on the nth recorded message of one conversation,
/// counting from zero — the budget's own answer, read off the ledger.
async fn recorded_limit(store: &Store, conversation_id: i64, nth: usize) -> Option<String> {
    let messages = support::await_ledger(store, conversation_id, "the stamped message", |blocks| {
        blocks
            .iter()
            .filter(|block| block.block_type == assistant_core::kind::CHAT_MESSAGE_KIND)
            .count()
            > nth
    })
    .await;
    messages
        .iter()
        .filter(|block| block.block_type == assistant_core::kind::CHAT_MESSAGE_KIND)
        .nth(nth)
        .expect("the awaited message exists")
        .fields
        .get(assistant_core::kind::COLUMN_LIMITED)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

/// AC10: a conversation that ran under the OLD contract is told the
/// contract changed — once, in the same act as the tool choice that grants
/// it the sending tools — and a second activity adds no second notice.
///
/// The old conversation is built the way one exists in a real store: a
/// recorded tool choice that lacks the two sending tools, written while the
/// conversation was being served, and a fresh process meeting it
/// afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_old_conversation_is_told_the_contract_changed_exactly_once() {
    let db = support::TempDb::new("contract-notice");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");
    let first = support::start_assistant_on(store.clone(), None).await;
    let key = channel("dm-contract-old");

    let asked = support::ingest_recorded(
        &first.assistant,
        inbound(&key, ChannelKind::Direct, "42", "under the old contract"),
    )
    .await;
    assert_eq!(
        notices(&store, asked.conversation_id).await,
        0,
        "non-vacuity: the conversation carries no notice yet"
    );

    // The choice a build without the sending tools recorded. Written
    // through the framework's own door, so the row is the shape that build
    // would have left.
    store
        .append_tool_choice(
            asked.conversation_id,
            vec!["lookup_wiki".to_owned(), "react".to_owned()],
        )
        .await
        .expect("the old choice appends");

    // A fresh process: its first activity on this conversation reconciles
    // the choice, finds the delta, and records where the line falls.
    let restarted = support::start_assistant_on(store.clone(), None).await;
    let again = support::ingest_recorded(
        &restarted.assistant,
        inbound(&key, ChannelKind::Direct, "42", "and after it"),
    )
    .await;
    assert_eq!(
        again.conversation_id, asked.conversation_id,
        "the same conversation keeps serving the channel"
    );
    assert_eq!(
        notices(&store, asked.conversation_id).await,
        1,
        "the conversation that crossed into the sending contract is told so"
    );

    // A second activity in the same process reconciles nothing, and a third
    // process finds the choice already current: neither adds a notice.
    support::ingest_recorded(
        &restarted.assistant,
        inbound(&key, ChannelKind::Direct, "42", "and again"),
    )
    .await;
    let third = support::start_assistant_on(store.clone(), None).await;
    support::ingest_recorded(
        &third.assistant,
        inbound(&key, ChannelKind::Direct, "42", "and once more"),
    )
    .await;
    assert_eq!(
        notices(&store, asked.conversation_id).await,
        1,
        "the notice is recorded once and never again"
    );
}

/// AC10's other half: a conversation born under this build has no relayed
/// answer to explain, so it is never told anything.
///
/// The notice answers a change; a conversation that never lived through one
/// would read it as a claim about history it does not have.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_conversation_born_under_the_sending_contract_is_told_nothing() {
    let fixture = support::start_assistant(None).await;
    let key = channel("dm-contract-fresh");

    let asked = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "born speaking"),
    )
    .await;
    support::settle(&fixture.store, asked.conversation_id, "the first turn", 4).await;
    assert_eq!(
        notices(&fixture.store, asked.conversation_id).await,
        0,
        "nothing changed under this conversation, so nothing is recorded"
    );
}

/// AC10, the act interrupted: the notice is written ahead of the choice, so
/// a process that died between the two leaves the notice standing and the
/// delta unwritten. The next activity reads the same pre-contract choice,
/// finds the notice, and appends only what is missing — one notice, and the
/// crossing recorded after all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_notice_standing_without_its_choice_draws_no_second_notice() {
    let db = support::TempDb::new("contract-interrupted");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");
    let first = support::start_assistant_on(store.clone(), None).await;
    let key = channel("dm-contract-interrupted");

    let asked = support::ingest_recorded(
        &first.assistant,
        inbound(&key, ChannelKind::Direct, "42", "under the old contract"),
    )
    .await;
    store
        .append_tool_choice(asked.conversation_id, vec!["lookup_wiki".to_owned()])
        .await
        .expect("the old choice appends");
    // What an interrupted act leaves behind: the notice written, the choice
    // it rides with never reached.
    store
        .append_consumer_block(
            asked.conversation_id,
            None,
            assistant_core::contract::CONTRACT_NOTICE_KIND,
            assistant_core::contract::ContractNotice::stored_fields(
                assistant_core::contract::CONTRACT_NOTICE,
            ),
            None,
        )
        .await
        .expect("the standing notice appends");

    let restarted = support::start_assistant_on(store.clone(), None).await;
    support::ingest_recorded(
        &restarted.assistant,
        inbound(&key, ChannelKind::Direct, "42", "and after it"),
    )
    .await;

    assert_eq!(
        notices(&store, asked.conversation_id).await,
        1,
        "the standing notice is the one notice: the crossing is explained once"
    );
    let recorded = store
        .newest_tool_choice(asked.conversation_id)
        .await
        .expect("the choice reads")
        .expect("a choice is recorded");
    for sending in assistant_core::tools::sending::NAMES {
        assert!(
            recorded.iter().any(|name| name == sending),
            "the half the interrupted act never wrote is written now: {sending} in {recorded:?}"
        );
    }
}

/// How many contract notices one conversation holds.
async fn notices(store: &Store, conversation_id: i64) -> usize {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter(|block| block.block_type == assistant_core::contract::CONTRACT_NOTICE_KIND)
        .count()
}

/// The notice reaches the model in the SYSTEM voice, carrying the recorded
/// sentence and nothing else — read through the production fold, not
/// through the kind's own projection.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_recorded_notice_reaches_the_model_in_the_system_voice() {
    let db = support::TempDb::new("contract-projection");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");
    let first = support::start_assistant_on(store.clone(), None).await;
    let key = channel("dm-contract-voice");

    let asked = support::ingest_recorded(
        &first.assistant,
        inbound(&key, ChannelKind::Direct, "42", "before the line"),
    )
    .await;
    store
        .append_tool_choice(asked.conversation_id, vec!["lookup_wiki".to_owned()])
        .await
        .expect("the old choice appends");

    let restarted = support::start_assistant_on(store.clone(), None).await;
    support::ingest_recorded(
        &restarted.assistant,
        inbound(&key, ChannelKind::Direct, "42", "after the line"),
    )
    .await;
    support::await_ledger(&store, asked.conversation_id, "the notice", |blocks| {
        blocks
            .iter()
            .any(|block| block.block_type == assistant_core::contract::CONTRACT_NOTICE_KIND)
    })
    .await;

    let blocks = store
        .list_blocks(asked.conversation_id)
        .await
        .expect("the ledger reads");
    let projected =
        agent_ledger::providers::blocks_to_messages::<assistant_core::kind::AssistantKind>(&blocks);
    let system: Vec<String> = projected
        .iter()
        .filter(|message| message.role == agent_ledger::providers::MessageRole::System)
        .map(|message| match &message.content {
            agent_ledger::providers::MessageContent::Text(text) => text.clone(),
            agent_ledger::providers::MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|part| match part {
                    agent_ledger::providers::ContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        })
        .collect();
    assert!(
        system
            .iter()
            .any(|text| text.contains(assistant_core::contract::CONTRACT_NOTICE)),
        "the notice reaches the model in the system voice; system lines: {system:?}"
    );
}
