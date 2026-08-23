//! The report at the core's edges (AC4–AC7): the filing over the
//! tool-scripted provider, the origin-walk target resolution with its
//! refusals, the atomic report window, the delivery contract on both
//! stream events, erasure's reach, and the palette supersession that
//! carries the new tools into existing conversations.

use std::sync::Arc;

use agent_ledger::{
    Block, CoreEvent, ProviderModule, ProviderRequest, ProviderResponse, Store, StreamEvent,
};
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::report::{self};
use assistant_core::{
    ChannelKind, CoreError, ErasureOutcome, FAILURE_NOTICE, ProtectionConfig, ReplyKind,
    ReplyTarget,
};
use serde_json::json;
use tokio::sync::mpsc;

use crate::support::{
    self, CLOSING_ANSWER, MODERATION_HANDLE, Round, ScriptHandle, ToolScript, carries_tool_result,
    channel, field, inbound, inbound_unaddressed, provider_stub, recv_reply,
    round_scripted_provider, settle_shape, tool_scripted_provider, with_origin, with_reply,
};

/// The outbound edge a fixture's replies arrive on.
type Replies = mpsc::UnboundedReceiver<assistant_core::OutboundReply>;

/// The fixed line every report in this suite files, under the suite's
/// configured handle.
fn fixture_line() -> String {
    report::report_line(MODERATION_HANDLE)
}

/// One assembled report fixture over the given provider: the report tool
/// alone — no lookups, so the palette and the ledger shapes stay minimal —
/// under the suite's moderation handle, plus the outbound edge.
async fn report_fixture_with(
    provider: Box<dyn ProviderModule>,
    handle: ScriptHandle,
    protection: ProtectionConfig,
) -> (support::Fixture, Replies) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    report_fixture_on(store, provider, handle, protection).await
}

/// The same fixture over the given store — for the phases of a
/// multi-process story.
async fn report_fixture_on(
    store: Store,
    provider: Box<dyn ProviderModule>,
    handle: ScriptHandle,
    protection: ProtectionConfig,
) -> (support::Fixture, Replies) {
    let fixture =
        support::start_assistant_reporting(store, provider, handle, ToolSet::new(), protection)
            .await;
    let replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    (fixture, replies)
}

/// The one-turn report fixture: the opening turn calls the report tool
/// with an empty input, the closing turn answers.
async fn report_fixture(
    narration: Option<String>,
    hold: Option<Arc<support::TurnHold>>,
) -> (support::Fixture, Replies) {
    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: report::NAME.into(),
            input: "{}".into(),
            narration,
        },
        hold,
    );
    report_fixture_with(provider, handle, ProtectionConfig::default()).await
}

/// A provider that calls the report tool on every turn: the requests of
/// one binding alternate call, close, call, close — every turn in these
/// stories is exactly one call round and one closing round, filed or
/// refused alike, so the alternation is the turn structure itself.
fn repeating_report_provider() -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = fresh_handle();
    let observed = handle.clone();
    let provider = provider_stub("Repeating reporter", "calls the report tool every turn", {
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let turns = Arc::clone(&observed.turns);
            let title_requests = Arc::clone(&observed.title_requests);
            tokio::spawn(async move {
                let mut calls = 0_usize;
                let mut close_next = false;
                while let Some(request) = requests.recv().await {
                    let ProviderRequest::Stream { messages, .. } = request else {
                        continue;
                    };
                    // Titles are off (decision 0077): count the regression,
                    // answer nothing.
                    if messages
                        .iter()
                        .any(|m| support::carries(m, support::TITLE_INSTRUCTION_MARK))
                    {
                        title_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    turns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    if close_next {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: CLOSING_ANSWER.into(),
                        }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: agent_ledger::StopReason::EndTurn,
                            }));
                    } else {
                        calls += 1;
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: agent_ledger::StopReason::ToolUse,
                            }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                                id: format!("call-{calls}"),
                                name: report::NAME.into(),
                            }));
                        let _ = response_tx.send(ProviderResponse::Event(
                            StreamEvent::ToolUseInputDelta { json: "{}".into() },
                        ));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                    }
                    close_next = !close_next;
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        }
    });
    (provider, handle)
}

/// The full one-report turn shape on a fresh group conversation: the spam
/// line, the ask, the call, the filed report, the result, the answer.
const FILED_TURN: [&str; 8] = [
    "system_prompt",
    "tool_palette",
    "chat_message",
    "chat_message",
    "tool_call",
    "report",
    "tool_result",
    "text",
];

/// Record one offending group line under the given origin and return its
/// sender's principal id.
async fn record_offense(
    fixture: &support::Fixture,
    key: &assistant_core::ChannelKey,
    sender: &str,
    origin: &str,
) -> i64 {
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(key, ChannelKind::Group, sender, "an offending line"),
            origin,
        ),
    )
    .await
    .principal_id
}

/// The member's report ask: addressed, replying to the given origin.
fn report_ask(key: &assistant_core::ChannelKey, origin: &str) -> assistant_core::InboundMessage {
    with_reply(
        inbound(key, ChannelKind::Group, "member-7", "please report this"),
        ReplyTarget::Message {
            origin: origin.into(),
        },
    )
}

// ─── AC4: the filing, end to end at the core edge ────────────────────────

/// The whole flow, block by block: the member's reply ask files the report
/// — the block carries the target origin, the reported principal and the
/// fixed line, the tool result names the filing — and the edge delivers
/// the line as a threaded report BEFORE the answer, while the answer
/// itself stays unthreaded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_members_reply_ask_files_and_the_edge_threads_the_report_before_the_answer() {
    let (fixture, mut replies) = report_fixture(None, None).await;
    let key = support::authorized_group(&fixture.assistant, "room-report").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the filed report turn",
        &FILED_TURN,
    )
    .await;

    // The block: target origin, reported principal, the fixed line.
    assert_eq!(field(&blocks[5], "target_origin"), "origin-spam-1");
    assert_eq!(
        blocks[5].fields["reported_principal_id"],
        json!(spammer),
        "the block names the offending message's sender for erasure"
    );
    assert_eq!(field(&blocks[5], "line"), fixture_line());
    // The tool result claims filing, not arrival.
    assert_eq!(field(&blocks[6], "content"), report::FILED_RESULT);

    // The delivery order: the report first, threaded; then the answer,
    // unthreaded — decision 0018's judgment stands for answers.
    let first = recv_reply(&mut replies).await;
    assert_eq!(first.kind, ReplyKind::Report);
    assert_eq!(first.text, fixture_line());
    assert_eq!(first.reply_target.as_deref(), Some("origin-spam-1"));
    let second = recv_reply(&mut replies).await;
    assert_eq!(second.kind, ReplyKind::Answer);
    assert_eq!(second.text, CLOSING_ANSWER);
    assert_eq!(second.reply_target, None, "the answer stays unthreaded");
    let extra = replies.try_recv();
    assert!(extra.is_err(), "one report, one answer; got {extra:?}");
}

/// The absorbed-bystander shape (AC4): an unaddressed bystander line with
/// its own reply target lands mid-narration, NEWER than the asking
/// co-summoner — and the resolution still reads the co-summoner's target:
/// a bystander co-summons nothing, so its reply loses even when newer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bystanders_newer_reply_target_loses_to_the_co_summoners() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) =
        report_fixture(Some("One moment.".into()), Some(hold.clone())).await;
    let key = support::authorized_group(&fixture.assistant, "room-bystander-reply").await;
    let reported = record_offense(&fixture, &key, "spammer-1", "origin-reported").await;
    record_offense(&fixture, &key, "spammer-2", "origin-bystanders-target").await;

    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-reported")).await;
    let conv = receipt.conversation_id;

    // Mid-narration — before the call block exists — the bystander's own
    // reply lands, unaddressed, pointing at the OTHER recorded message.
    hold.started().await;
    support::await_ledger(&fixture.store, conv, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            inbound_unaddressed(&key, ChannelKind::Group, "bystander-9", "lol that one"),
            ReplyTarget::Message {
                origin: "origin-bystanders-target".into(),
            },
        ),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the filed turn behind the bystander",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "chat_message",
            "chat_message",
            "text",
            "tool_call",
            "report",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The premise: the absorbed line is unaddressed and carries a target.
    assert_eq!(blocks[5].fields["addressed"], json!(false));
    assert_eq!(
        field(&blocks[5], "reply_target"),
        "origin-bystanders-target"
    );
    // The resolution: the co-summoner's target, not the newer bystander's.
    assert_eq!(field(&blocks[8], "target_origin"), "origin-reported");
    assert_eq!(blocks[8].fields["reported_principal_id"], json!(reported));
    while recv_reply(&mut replies).await.text != CLOSING_ANSWER {}
}

/// The other half of the origin walk (AC4): the ANCHOR carries no reply —
/// an addressed summons that merely opened the turn — and the report ask,
/// replying to the offending message, is absorbed mid-narration as a
/// co-summoner. The filed block's target must be the absorbed ask's: a
/// resolution reduced to reading the anchor's own stored reply finds
/// nothing here and refuses the filing instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_absorbed_asks_reply_target_answers_for_a_replyless_anchor() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) =
        report_fixture(Some("One moment.".into()), Some(hold.clone())).await;
    let key = support::authorized_group(&fixture.assistant, "room-absorbed-ask").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    // The summons: addressed, NOT a reply — it opens the turn and anchors
    // the dispatch.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "member-3", "someone is spamming"),
    )
    .await;
    let conv = receipt.conversation_id;

    // Mid-narration — before the call block exists — the report ask lands,
    // addressed and replying to the offending message: an absorbed
    // co-summoner.
    hold.started().await;
    support::await_ledger(&fixture.store, conv, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
    support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the filed turn behind the absorbed ask",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "chat_message",
            "text",
            "tool_call",
            "report",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The premise: the anchoring summons stored no reply, and the absorbed
    // ask is the one row carrying the target.
    assert_eq!(blocks[3].fields["addressed"], json!(true));
    assert!(
        blocks[3].fields.get("reply_target").is_none(),
        "the summons is not a reply"
    );
    assert_eq!(blocks[4].fields["addressed"], json!(true));
    assert_eq!(field(&blocks[4], "reply_target"), "origin-spam-1");
    // The resolution: the absorbed ask's target, through the origin walk.
    assert_eq!(field(&blocks[7], "target_origin"), "origin-spam-1");
    assert_eq!(blocks[7].fields["reported_principal_id"], json!(spammer));
    while recv_reply(&mut replies).await.text != CLOSING_ANSWER {}
}

/// The debt walk's read-through set carries the report kind: a filed
/// report landing on top of an unanswered message buries nothing at the
/// stamp — the next unaddressed line still carries the debt forward, the
/// same shape as the context-note pin. Dropping the report entry from
/// [`DEBT_READ_THROUGH`] fails this: the report would read as a settled
/// tail and the debt would die under it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn debt_propagation_reads_through_a_filed_report_at_the_stamp() {
    let (fixture, _replies) = report_fixture_with(
        support::silent_provider(),
        fresh_handle(),
        ProtectionConfig::default(),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-report-read-through").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "member-7", "the owed ask"),
    )
    .await;
    fixture
        .store
        .append_consumer_block(
            receipt.conversation_id,
            None,
            report::REPORT_KIND,
            report::Report::stored_fields("origin-spam-1", receipt.principal_id, &fixture_line()),
            None,
        )
        .await
        .expect("the report block appends on top of the owed ask");

    // Non-vacuity: the report really is the stored tail the stamp reads
    // behind.
    let tail = fixture
        .store
        .latest_block(receipt.conversation_id)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(tail.block_type, report::REPORT_KIND);

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "B", "an aside behind the report"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks
        .iter()
        .find(|block| block.fields.get("text") == Some(&json!("an aside behind the report")))
        .expect("the aside is recorded");
    assert_eq!(
        aside.fields["answer_due"],
        json!(true),
        "the debt propagates through the filed report"
    );
    assert_eq!(
        aside.fields["debt_authority"],
        json!("member"),
        "the carried debt folds through the report unchanged"
    );
}

/// The same pin for the palette kind: a superseding palette append landing
/// on top of an unanswered message buries nothing at the stamp. Dropping
/// the palette entry from [`DEBT_READ_THROUGH`] fails this the same way
/// the report pin above fails without its entry.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn debt_propagation_reads_through_a_superseding_palette_at_the_stamp() {
    let (fixture, _replies) = report_fixture_with(
        support::silent_provider(),
        fresh_handle(),
        ProtectionConfig::default(),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-palette-read-through").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "member-7", "the owed ask"),
    )
    .await;
    fixture
        .store
        .append_consumer_block(
            receipt.conversation_id,
            None,
            assistant_core::tools::palette::TOOL_PALETTE_KIND,
            assistant_core::tools::palette::ToolPalette::stored_fields(&reporting_palette()),
            None,
        )
        .await
        .expect("the superseding palette appends on top of the owed ask");

    // Non-vacuity: the superseding palette really is the stored tail.
    let tail = fixture
        .store
        .latest_block(receipt.conversation_id)
        .await
        .expect("the tail reads")
        .expect("the ledger is non-empty");
    assert_eq!(
        tail.block_type,
        assistant_core::tools::palette::TOOL_PALETTE_KIND
    );

    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "B", "an aside behind the palette"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let aside = blocks
        .iter()
        .find(|block| block.fields.get("text") == Some(&json!("an aside behind the palette")))
        .expect("the aside is recorded");
    assert_eq!(
        aside.fields["answer_due"],
        json!(true),
        "the debt propagates through the superseding palette"
    );
    assert_eq!(
        aside.fields["debt_authority"],
        json!("member"),
        "the carried debt folds through the palette unchanged"
    );
}

// ─── AC5: the bounds and the failure shapes ──────────────────────────────

/// The refusal shapes, each a recorded tool error with the pinned wording
/// and no report block: an ask without a reply, a reply to the assistant's
/// own message, and a reply pointing at a message the ledger never
/// recorded.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_no_reply_self_report_and_unrecorded_target_asks_are_refused() {
    for (name, ask_reply, expected) in [
        ("no reply", None, report::NEEDS_REPLY_ERROR),
        (
            "self report",
            Some(ReplyTarget::AssistantMessage),
            report::SELF_REPORT_ERROR,
        ),
        (
            "unrecorded target",
            Some(ReplyTarget::Message {
                origin: "origin-nobody-recorded".into(),
            }),
            report::UNRECORDED_TARGET_ERROR,
        ),
    ] {
        let (fixture, mut replies) = report_fixture(None, None).await;
        let key = support::authorized_group(&fixture.assistant, "room-refusals").await;
        let mut ask = inbound(&key, ChannelKind::Group, "member-7", "report please");
        ask.reply_target = ask_reply;
        let receipt = support::ingest_recorded(&fixture.assistant, ask).await;
        let blocks = settle_shape(
            &fixture.store,
            receipt.conversation_id,
            "the refused turn",
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
            expected,
            "the {name} ask records its pinned refusal"
        );
        assert_eq!(
            recv_reply(&mut replies).await.text,
            CLOSING_ANSWER,
            "the {name} turn still closes with the model's answer"
        );
        let extra = replies.try_recv();
        assert!(extra.is_err(), "no report went out for {name}: {extra:?}");
    }
}

/// The direct-conversation refusal: reports belong to groups, and a DM ask
/// draws the pinned group-only error with nothing filed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_conversation_ask_is_refused() {
    let (fixture, mut replies) = report_fixture(None, None).await;
    let key = channel("dm-report");
    let ask = with_reply(
        inbound(&key, ChannelKind::Direct, "42", "report this"),
        ReplyTarget::Message {
            origin: "origin-anything".into(),
        },
    );
    let receipt = support::ingest_recorded(&fixture.assistant, ask).await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the refused direct turn",
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
    assert_eq!(field(&blocks[4], "error"), report::GROUP_ONLY_ERROR);
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

/// The report window (AC5): the second ask inside the window is declined
/// with the no-retry result and files nothing. The window's REOPENING is
/// pinned under paused time on the injected primitive itself, in the
/// window module beside its clock — a paused full assembly is unsound,
/// because the store's actor is an external thread and the paused clock
/// auto-advances past every deadline while a task awaits it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_ask_inside_the_window_is_declined() {
    let (provider, handle) = repeating_report_provider();
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-window").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let conv = receipt.conversation_id;
    settle_shape(&fixture.store, conv, "the first filed turn", &FILED_TURN).await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);

    // The second ask, inside the window: declined, nothing filed.
    support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let blocks = support::await_ledger(&fixture.store, conv, "the declined turn", |blocks| {
        blocks.len() == FILED_TURN.len() + 4
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    assert_eq!(blocks[FILED_TURN.len() + 2].block_type, "tool_error");
    assert_eq!(
        field(&blocks[FILED_TURN.len() + 2], "error"),
        report::DECLINED_RESULT
    );
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == "report")
            .count(),
        1,
        "the declined ask filed nothing"
    );
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

/// The spend-after-append ordering (AC5): a transiently failed append
/// spends nothing — the sabotaged ask records the transient error and no
/// block, and the healed retry files, undeclined.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_transient_append_failure_spends_no_window_slot() {
    let (provider, handle) = repeating_report_provider();
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-sabotage").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    support::sabotage_appends(&fixture.store, report::REPORT_TABLE).await;
    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let conv = receipt.conversation_id;
    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the sabotaged turn",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    let error = field(&blocks[5], "error");
    assert!(
        error.contains("right now"),
        "the failed append is the transient error: {error}"
    );
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);

    // Healed, the retry files: the failure spent no window slot.
    support::heal_appends(&fixture.store, report::REPORT_TABLE).await;
    support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    support::await_ledger(&fixture.store, conv, "the healed filing", |blocks| {
        blocks.iter().any(|block| block.block_type == "report")
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

/// A provider that files on its opening request — narrating first when
/// given prose, in the production order that finalizes the narration
/// before the call — and errors the continuation carrying the tool
/// result.
fn files_then_dies_provider(
    narration: Option<&'static str>,
) -> (Box<dyn ProviderModule>, ScriptHandle) {
    let handle = fresh_handle();
    let observed = handle.clone();
    let provider = provider_stub(
        "Files then dies",
        "files a report, then errors",
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let title_requests = Arc::clone(&observed.title_requests);
            tokio::spawn(async move {
                while let Some(request) = requests.recv().await {
                    let ProviderRequest::Stream { messages, .. } = request else {
                        continue;
                    };
                    // Titles are off (decision 0077): count the regression,
                    // answer nothing.
                    if messages
                        .iter()
                        .any(|m| support::carries(m, support::TITLE_INSTRUCTION_MARK))
                    {
                        title_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    if messages.iter().any(carries_tool_result) {
                        let _ = response_tx
                            .send(ProviderResponse::Error("scripted stream failure".into()));
                        continue;
                    }
                    if let Some(narration) = narration {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: narration.into(),
                        }));
                    }
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                        usage: agent_ledger::providers::Usage::default(),
                        stop_reason: agent_ledger::StopReason::ToolUse,
                    }));
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                        id: "call-1".into(),
                        name: report::NAME.into(),
                    }));
                    let _ =
                        response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseInputDelta {
                            json: "{}".into(),
                        }));
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        },
    );
    (provider, handle)
}

/// The failure half of the delivery contract (AC5): a turn that errors
/// after filing still delivers the report — threaded, ahead of the notice
/// — and, with no narration committed, nothing else arrives.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_that_errors_after_filing_still_delivers_the_report_beside_the_notice() {
    let (provider, handle) = files_then_dies_provider(None);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-dying-turn").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;

    let first = recv_reply(&mut replies).await;
    assert_eq!(
        first.kind,
        ReplyKind::Report,
        "the filing outlives the turn"
    );
    assert_eq!(first.text, fixture_line());
    assert_eq!(first.reply_target.as_deref(), Some("origin-spam-1"));
    let second = recv_reply(&mut replies).await;
    assert_eq!(
        second.kind,
        ReplyKind::Notice,
        "the notice follows the report"
    );
    assert_eq!(second.text, FAILURE_NOTICE);
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "no model answer exists to deliver: {extra:?}"
    );
}

/// The failure wake reads the whole owed tail, wider than the report
/// alone, on purpose: a turn that narrates, files, then dies delivers
/// everything the dead turn already put on the ledger, in ledger order —
/// the finalized narration, the threaded report, then the notice. The
/// delivery cursor is one high-water mark per conversation, so
/// withholding the committed narration would either drop it for good
/// (the cursor passes it with the report) or repeat the report on the
/// next wake, and the delivery contract refuses re-delivered reports.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_narrating_turn_that_dies_delivers_narration_report_then_notice() {
    let (provider, handle) = files_then_dies_provider(Some("One moment."));
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-dying-narrator").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;

    let first = recv_reply(&mut replies).await;
    assert_eq!(
        first.kind,
        ReplyKind::Answer,
        "the committed narration delivers first, in ledger order"
    );
    assert_eq!(first.text, "One moment.");
    assert_eq!(first.reply_target, None, "narration stays unthreaded");
    let second = recv_reply(&mut replies).await;
    assert_eq!(
        second.kind,
        ReplyKind::Report,
        "the filing follows its prose"
    );
    assert_eq!(second.text, fixture_line());
    assert_eq!(second.reply_target.as_deref(), Some("origin-spam-1"));
    let third = recv_reply(&mut replies).await;
    assert_eq!(third.kind, ReplyKind::Notice, "the notice closes the turn");
    assert_eq!(third.text, FAILURE_NOTICE);
    let extra = replies.try_recv();
    assert!(extra.is_err(), "nothing further arrives: {extra:?}");
}

/// The budget composition (AC5): a report ask consumes one answer slot
/// like any addressed turn — under a one-answer budget the next addressed
/// ask is recorded limited and summons nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_report_ask_consumes_an_answer_slot() {
    let (provider, handle) = repeating_report_provider();
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, support::budgets(Some((1, 600)), None)).await;
    let key = support::authorized_group(&fixture.assistant, "room-budget").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let conv = receipt.conversation_id;
    settle_shape(&fixture.store, conv, "the filed turn", &FILED_TURN).await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);

    // The same member's next addressed ask crosses the one-answer budget.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "member-7", "and another thing"),
    )
    .await;
    let blocks = support::await_ledger(&fixture.store, conv, "the limited ask", |blocks| {
        blocks.len() == FILED_TURN.len() + 1
    })
    .await;
    let limited = blocks.last().expect("the limited ask is newest");
    assert_eq!(limited.fields["limited"], json!("principal"));
    assert_eq!(
        limited.fields["answer_due"],
        json!(false),
        "the report ask spent the slot: the next ask is refused"
    );
}

// ─── AC6: erasure and absence ────────────────────────────────────────────

/// The reported person's erasure nulls the block's target while the line
/// stays, and a report still undelivered at that moment goes out as
/// nothing: the edge skips the targetless report and delivers only the
/// answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reported_persons_erasure_nulls_the_target_and_the_edge_skips_it() {
    let hold = support::TurnHold::new();
    let rounds = vec![
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: true,
            call: Some(report::NAME),
        },
        Round {
            narration: None,
            hold_after_finalize: false,
            hold_before_done: false,
            call: None,
        },
    ];
    let (provider, handle) = round_scripted_provider(rounds, Arc::clone(&hold));
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-erased-target").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let conv = receipt.conversation_id;

    // The round holds before its trailing done: the report is filed, the
    // stream provably open, nothing delivered yet.
    hold.started().await;
    support::await_ledger(&fixture.store, conv, "the filed report", |blocks| {
        blocks.iter().any(|block| block.block_type == "report")
    })
    .await;
    let outcome = fixture
        .assistant
        .erase_principal(spammer)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));
    hold.release();

    let blocks = support::await_ledger(&fixture.store, conv, "the settled turn", |blocks| {
        blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    let stored = blocks
        .iter()
        .find(|block| block.block_type == "report")
        .expect("the report block stands");
    assert!(
        stored.fields.get("target_origin").is_none(),
        "the erasure nulled the block's target origin"
    );
    assert_eq!(
        field(stored, "line"),
        fixture_line(),
        "the line text stays; it names nobody"
    );
    // The delivery: the answer alone — the targetless report was skipped.
    let only = recv_reply(&mut replies).await;
    assert_eq!(only.kind, ReplyKind::Answer);
    assert_eq!(only.text, CLOSING_ANSWER);
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "the targetless report never sends: {extra:?}"
    );
}

/// After the reported person's erasure, a fresh ask replying to the erased
/// message resolves nothing — the nulled origin matches no recorded
/// message, so no report can re-materialize what erasure removed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_erased_targets_origin_cannot_be_re_reported() {
    let (fixture, mut replies) = report_fixture(None, None).await;
    let key = support::authorized_group(&fixture.assistant, "room-re-report").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let outcome = fixture
        .assistant
        .erase_principal(spammer)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));

    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the refused re-report",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[5], "error"), report::UNRECORDED_TARGET_ERROR);
    // The recorded residual, pinned as PRESENT on purpose (decision 0063's
    // refinements): the ask was recorded after the erasure completed, so
    // its row stores the erased person's message identifier where no
    // erasure pass will ever match it — the identity rows are gone and the
    // person's next appearance resolves to a new principal. The
    // ingestion-time reach key that closes this ships as its own unit;
    // until it does, this assertion is the tree's record of what stays.
    assert_eq!(
        field(&blocks[3], "reply_target"),
        "origin-spam-1",
        "the post-erasure ask keeps its stored reply target"
    );
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

/// The reporter's own erasure nulls the reply-target column on their
/// message rows through the author-keyed pass, while the structural
/// assistant-reply fact stays; and a non-reply stores no target at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reporters_erasure_nulls_their_reply_target_and_a_non_reply_stores_none() {
    let (fixture, mut replies) = report_fixture(None, None).await;
    let key = support::authorized_group(&fixture.assistant, "room-reporter-erasure").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let reporter =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    settle_shape(
        &fixture.store,
        reporter.conversation_id,
        "the filed turn",
        &FILED_TURN,
    )
    .await;
    while recv_reply(&mut replies).await.text != CLOSING_ANSWER {}

    let outcome = fixture
        .assistant
        .erase_principal(reporter.principal_id)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));
    let blocks = fixture
        .store
        .list_blocks(reporter.conversation_id)
        .await
        .expect("the ledger reads");
    let ask = blocks
        .iter()
        .filter(|block| block.block_type == "chat_message")
        .nth(1)
        .expect("the ask row stands");
    assert!(
        ask.fields.get("reply_target").is_none(),
        "the author-keyed pass nulled the reply target"
    );
    assert!(
        ask.fields.get("text").is_none(),
        "the same pass erased the prose"
    );
    let offense = blocks
        .iter()
        .find(|block| block.block_type == "chat_message")
        .expect("the offending row stands");
    assert!(
        offense.fields.get("reply_target").is_none(),
        "a non-reply stored no target to begin with"
    );
}

/// The reported person's erasure also reaches the reply-target copies
/// OTHER people's rows hold (added 2026-08-23): a reply
/// stores the replied-to message's platform id, which is the replied-to
/// person's data wherever it sits — the author-keyed pass alone would
/// null it on the erased person's own rows while leaving a verbatim copy
/// on every row that replied to them. The target-keyed pass nulls exactly
/// that copy and nothing else of the replier's — and both erasure passes
/// stay keyed per row (pinned 2026-08-23): a peer room whose
/// offense carries the SAME platform message id under a DIFFERENT,
/// non-erased sender keeps its replier's stored target and its report's
/// target origin, so a pass widened past its key fails here.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reported_persons_erasure_nulls_the_repliers_stored_reply_target() {
    let (fixture, mut replies) = report_fixture(None, None).await;
    let key = support::authorized_group(&fixture.assistant, "room-target-pass").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let reporter =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    settle_shape(
        &fixture.store,
        reporter.conversation_id,
        "the filed turn",
        &FILED_TURN,
    )
    .await;
    while recv_reply(&mut replies).await.text != CLOSING_ANSWER {}

    // The peer room: a different sender's offense under the same platform
    // message id — ids are unique only per channel — reported the same way.
    let peer_key = support::authorized_group(&fixture.assistant, "room-target-pass-peer").await;
    let peer_sender = record_offense(&fixture, &peer_key, "other-9", "origin-spam-1").await;
    assert_ne!(peer_sender, spammer, "the peer offense has its own sender");
    let peer =
        support::ingest_recorded(&fixture.assistant, report_ask(&peer_key, "origin-spam-1")).await;
    settle_shape(
        &fixture.store,
        peer.conversation_id,
        "the peer room's filed turn",
        &FILED_TURN,
    )
    .await;
    while recv_reply(&mut replies).await.text != CLOSING_ANSWER {}

    let outcome = fixture
        .assistant
        .erase_principal(spammer)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));

    let blocks = fixture
        .store
        .list_blocks(reporter.conversation_id)
        .await
        .expect("the ledger reads");
    let ask = blocks
        .iter()
        .filter(|block| block.block_type == "chat_message")
        .nth(1)
        .expect("the ask row stands");
    assert!(
        ask.fields.get("reply_target").is_none(),
        "the target-keyed pass nulled the replier's copy of the erased person's message id"
    );
    assert_eq!(
        field(ask, "text"),
        "please report this",
        "the replier's own prose stays; only the erased person's identifier left the row"
    );

    // The peer room survives whole: its replier's target names another
    // person's message, and its report names another reported principal.
    let peer_blocks = fixture
        .store
        .list_blocks(peer.conversation_id)
        .await
        .expect("the peer ledger reads");
    let peer_ask = peer_blocks
        .iter()
        .filter(|block| block.block_type == "chat_message")
        .nth(1)
        .expect("the peer ask row stands");
    assert_eq!(
        field(peer_ask, "reply_target"),
        "origin-spam-1",
        "the target-keyed pass is keyed per row: the same platform id under \
         a different sender in another conversation survives"
    );
    let peer_report = peer_blocks
        .iter()
        .find(|block| block.block_type == "report")
        .expect("the peer report block stands");
    assert_eq!(
        field(peer_report, "target_origin"),
        "origin-spam-1",
        "the report pass is keyed by the reported principal: another \
         person's report keeps its target origin"
    );
    assert_eq!(
        peer_report.fields["reported_principal_id"],
        json!(peer_sender),
        "the surviving report still names its own reported person"
    );
}

/// The marker a direct message carries to get the deaf stream: opened,
/// tailed, never settled — what keeps a racing erasure on the fence for
/// its whole settle bound.
const UNSETTLED_ASK: &str = "the unsettled ask";

/// A provider for the fence race: a request carrying [`UNSETTLED_ASK`]
/// gets the deaf stream; the group's opening round finalizes its tool-use
/// stop, announces on the returned channel, and waits for one permit
/// before streaming its report call — the window a test starts an erasure
/// in; a request carrying an answered call closes with the answer.
fn racing_report_provider() -> (
    Box<dyn ProviderModule>,
    ScriptHandle,
    Arc<tokio::sync::Semaphore>,
    mpsc::UnboundedReceiver<()>,
) {
    let handle = fresh_handle();
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let (started_tx, started) = mpsc::unbounded_channel();
    let observed = handle.clone();
    let permits = Arc::clone(&release);
    let provider = provider_stub("Fence racer", "pauses the report round for an erasure", {
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let turns = Arc::clone(&observed.turns);
            let title_requests = Arc::clone(&observed.title_requests);
            let started_tx = started_tx.clone();
            let permits = Arc::clone(&permits);
            tokio::spawn(async move {
                while let Some(request) = requests.recv().await {
                    let ProviderRequest::Stream { messages, .. } = request else {
                        continue;
                    };
                    // Titles are off (decision 0077): count the regression,
                    // answer nothing.
                    if messages
                        .iter()
                        .any(|m| support::carries(m, support::TITLE_INSTRUCTION_MARK))
                    {
                        title_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let _ = response_tx.send(ProviderResponse::Done);
                        continue;
                    }
                    turns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    if messages.iter().any(|m| support::carries(m, UNSETTLED_ASK)) {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: "a tail that never ends".into(),
                        }));
                        std::future::pending::<()>().await;
                    }
                    let resolved = messages.iter().filter(|m| carries_tool_result(m)).count();
                    if resolved == 0 {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: agent_ledger::StopReason::ToolUse,
                            }));
                        let _ = started_tx.send(());
                        match permits.acquire().await {
                            Ok(permit) => permit.forget(),
                            Err(_) => break,
                        }
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseStart {
                                id: "call-0".into(),
                                name: report::NAME.into(),
                            }));
                        let _ = response_tx.send(ProviderResponse::Event(
                            StreamEvent::ToolUseInputDelta { json: "{}".into() },
                        ));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                    } else {
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                        let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                            text: CLOSING_ANSWER.into(),
                        }));
                        let _ =
                            response_tx.send(ProviderResponse::Event(StreamEvent::MessageEnd {
                                usage: agent_ledger::providers::Usage::default(),
                                stop_reason: agent_ledger::StopReason::EndTurn,
                            }));
                    }
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        }
    });
    (provider, handle, release, started)
}

/// A filing racing an erasure waits on the fence (AC6): while the erasure
/// holds it exclusively — provably, from its interrupt going out to its
/// loud settle failure — the tool's filing appends nothing, and the
/// report lands only after the erasure released. Deleting the fence hold
/// in the tool's filing fails this test: the report block would land
/// inside the held window.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_filing_racing_an_erasure_waits_on_the_fence() {
    let (provider, handle, release, mut started) = racing_report_provider();
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-fence-race").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    // The reported person's own direct chat, held open by the deaf
    // stream: the erasure will interrupt it and stay on the fence for its
    // whole settle bound.
    let dm = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &channel("dm-fence-race"),
            ChannelKind::Direct,
            "spammer-1",
            UNSETTLED_ASK,
        ),
    )
    .await;
    assert_eq!(dm.principal_id, spammer, "one platform sender, one person");
    support::await_ledger(
        &fixture.store,
        dm.conversation_id,
        "the deaf tail",
        |blocks| blocks.iter().any(|b| b.block_type.starts_with("streaming")),
    )
    .await;

    // The report ask reaches the round's pause: the turn is open and the
    // tool has not run.
    let receipt =
        support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    let conv = receipt.conversation_id;
    tokio::time::timeout(support::DEADLINE, started.recv())
        .await
        .expect("the round pauses before its call")
        .expect("the provider outlives the test");

    // The erasure of the reported person, driven by polling: its
    // interrupt on the bus proves it holds the fence now, and between
    // polls the future sits still with the fence held — a stable window.
    let mut events = fixture.bus.subscribe();
    let mut erasure = Box::pin(fixture.assistant.erase_principal(spammer));
    let holding = std::time::Instant::now() + support::DEADLINE;
    'held: loop {
        assert!(
            std::time::Instant::now() < holding,
            "the interrupt proves the held fence before the deadline"
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut erasure)
                .await
                .is_err(),
            "the erasure cannot finish while the deaf stream holds it open"
        );
        while let Ok(Ok(event)) =
            tokio::time::timeout(std::time::Duration::from_millis(10), events.recv()).await
        {
            if matches!(event, CoreEvent::InterruptRequested { .. }) {
                break 'held;
            }
        }
    }

    // The round released into the held fence: the filing must wait, so no
    // report block may land inside this window.
    release.add_permits(1);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let blocks = fixture
        .store
        .list_blocks(conv)
        .await
        .expect("the ledger reads");
    assert!(
        !blocks.iter().any(|b| b.block_type == "report"),
        "the filing waits on the fence while the erasure holds it"
    );

    // The erasure runs to its loud settle failure and releases the fence;
    // only now does the filing land — with its origin intact, because the
    // failed erasure deleted nothing.
    let failure = erasure
        .await
        .expect_err("the deaf stream fails the erasure at the bound");
    assert!(
        matches!(failure, CoreError::ErasureUnsettled { .. }),
        "the erasure failed loudly at the bound; got {failure:?}"
    );
    let blocks = support::await_ledger(&fixture.store, conv, "the filed report", |blocks| {
        blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    let stored = blocks
        .iter()
        .find(|block| block.block_type == "report")
        .expect("the report block lands after the fence released");
    assert_eq!(field(stored, "target_origin"), "origin-spam-1");
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

// ─── AC7: registration, the palette, and the supersession ────────────────

/// The stored tool list of one palette block.
fn palette_names(block: &Block) -> Vec<String> {
    serde_json::from_str(&field(block, "tools")).expect("the stored list parses")
}

/// A fresh observation handle for the providers this module builds itself.
fn fresh_handle() -> ScriptHandle {
    ScriptHandle::fresh()
}

/// The full registered set of a reporting deployment, sorted as the
/// palette records it: the three production lookups plus the report tool.
fn reporting_palette() -> Vec<String> {
    vec![
        "lookup_commit".into(),
        "lookup_release".into(),
        "lookup_wiki".into(),
        report::NAME.into(),
    ]
}

/// A pre-unit group conversation whose stored palette predates this unit
/// gains both new tools on its first activity: the delta append supersedes
/// the old two-lookup list with the full registered set, and the gained
/// report tool files in the very next turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pre_unit_palette_gains_the_wiki_and_report_tools_on_first_activity() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let conversation = store
        .create_conversation(
            "scripted-1".into(),
            "script-model".into(),
            "Script Model".into(),
            support::VENDOR.into(),
        )
        .await
        .expect("a conversation row");
    store
        .insert_system_prompt(conversation, support::SYSTEM_PROMPT.into())
        .await
        .expect("the prompt records");
    // The pre-unit palette: the two lookups of the tools unit.
    store
        .append_consumer_block(
            conversation,
            None,
            assistant_core::tools::palette::TOOL_PALETTE_KIND,
            assistant_core::tools::palette::ToolPalette::stored_fields(&[
                "lookup_commit".into(),
                "lookup_release".into(),
            ]),
            None,
        )
        .await
        .expect("the pre-unit palette appends");
    agent_ledger::store::domain_run(&store.tx(), assistant_core::schema::DOMAIN, move |conn| {
        conn.execute(
            "INSERT INTO channels (adapter, channel, kind, conversation_id) \
             VALUES (?1, ?2, 'group', ?3)",
            (support::ADAPTER, "room-pre-unit-palette", conversation),
        )?;
        Ok(())
    })
    .await
    .expect("the pre-unit mapping writes");

    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: report::NAME.into(),
            input: "{}".into(),
            narration: None,
        },
        None,
    );
    let key = channel("room-pre-unit-palette");
    let (fixture, mut replies) = {
        let fixture = support::start_assistant_reporting(
            store,
            provider,
            handle,
            support::production_toolset(),
            ProtectionConfig::default(),
        )
        .await;
        let replies = fixture
            .assistant
            .replies(support::ADAPTER)
            .await
            .expect("the outbound edge opens");
        (fixture, replies)
    };
    support::authorize(&fixture.assistant, &key).await;

    // The first activity: the offense lands, and the palette supersedes.
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let blocks = support::await_ledger(
        &fixture.store,
        conversation,
        "the superseding palette",
        |blocks| {
            blocks
                .iter()
                .filter(|block| block.block_type == "tool_palette")
                .count()
                == 2
        },
    )
    .await;
    let newest = blocks
        .iter()
        .rev()
        .find(|block| block.block_type == "tool_palette")
        .expect("the delta palette stands");
    assert_eq!(
        palette_names(newest),
        reporting_palette(),
        "the delta append carries the full registered set, report included"
    );

    // The gained report tool admits and files on the next ask.
    support::ingest_recorded(&fixture.assistant, report_ask(&key, "origin-spam-1")).await;
    support::await_ledger(&fixture.store, conversation, "the filed report", |blocks| {
        blocks.iter().any(|block| block.block_type == "report")
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

/// No handle configured (AC7): the report tool is absent from a fresh
/// conversation's palette — the wiki tool stands with the other lookups —
/// and REMOVED from a pre-existing conversation's palette by the delta
/// append on its first activity under the handleless process.
#[test]
fn without_a_handle_the_report_tool_unregisters_and_the_delta_removes_it() {
    let db = support::TempDb::new("handle-removed");
    let key = channel("room-handle-removed");

    // Process one, handle configured: the group's palette names the full
    // set, report included.
    let conversation = support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the first store opens");
        let fixture = support::start_assistant_reporting(
            store,
            support::silent_provider(),
            fresh_handle(),
            support::production_toolset(),
            ProtectionConfig::default(),
        )
        .await;
        support::authorize(&fixture.assistant, &key).await;
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            inbound_unaddressed(&key, ChannelKind::Group, "42", "recorded under the handle"),
        )
        .await;
        let blocks = fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the ledger reads");
        let palettes: Vec<&Block> = blocks
            .iter()
            .filter(|block| block.block_type == "tool_palette")
            .collect();
        assert_eq!(palettes.len(), 1, "one creation palette");
        assert_eq!(
            palette_names(palettes[0]),
            reporting_palette(),
            "the handle registered the report tool into the palette"
        );
        receipt.conversation_id
    });

    // Process two, no handle: the pre-existing palette is superseded
    // without the report tool, and a fresh conversation never names it.
    support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store reopens");
        let fixture = support::start_assistant_full(
            store,
            support::silent_provider(),
            fresh_handle(),
            support::production_toolset(),
            ProtectionConfig::default(),
        )
        .await;
        support::ingest_recorded(
            &fixture.assistant,
            inbound_unaddressed(&key, ChannelKind::Group, "42", "first activity without it"),
        )
        .await;
        let blocks = fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let palettes: Vec<&Block> = blocks
            .iter()
            .filter(|block| block.block_type == "tool_palette")
            .collect();
        assert_eq!(palettes.len(), 2, "the delta append superseded the palette");
        assert_eq!(
            palette_names(palettes[1]),
            vec![
                "lookup_commit".to_owned(),
                "lookup_release".to_owned(),
                "lookup_wiki".to_owned()
            ],
            "the report tool is removed; the wiki tool stands with the lookups"
        );

        let fresh = support::ingest_recorded(
            &fixture.assistant,
            inbound(
                &channel("dm-fresh-no-handle"),
                ChannelKind::Direct,
                "7",
                "hello",
            ),
        )
        .await;
        let blocks = fixture
            .store
            .list_blocks(fresh.conversation_id)
            .await
            .expect("the fresh ledger reads");
        let palette = blocks
            .iter()
            .find(|block| block.block_type == "tool_palette")
            .expect("the creation palette stands");
        assert!(
            !palette_names(palette)
                .iter()
                .any(|name| name == report::NAME),
            "a fresh palette never names the unconfigured report tool"
        );
    });
}
