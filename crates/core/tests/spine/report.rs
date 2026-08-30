//! The autonomous moderation report at the core's edges (unit 15, AC2–AC7):
//! the model assesses a violating group message and files through the tool
//! naming that message's projected id, the named origin validates against
//! the turn's co-summoner set, per-origin dedup bounds the re-summon path,
//! the guards decline with their pinned copy, the threaded delivery holds
//! on completion and failure alike, erasure reaches the block, and the
//! registration gates on the handle plus helpful answering.

use std::sync::Arc;

use agent_ledger::{
    Block, CoreEvent, ProviderModule, ProviderRequest, ProviderResponse, Store, StreamEvent,
};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::note::RULES_NOTE_LEAD;
use assistant_core::schema::store_config;
use assistant_core::tools::ToolSet;
use assistant_core::tools::report::{self};
use assistant_core::{
    AnsweringMode, ChannelKind, CoreError, ErasureOutcome, FAILURE_NOTICE, IngestReceipt,
    MODERATION_TEACHING, Observation, ObserveOutcome, ObservedFact, ProtectionConfig, ReplyKind,
    ReplyTarget, ReplyThread,
};
use serde_json::json;
use tokio::sync::{Semaphore, mpsc};

use crate::support::{
    self, CLOSING_ANSWER, MODERATION_HANDLE, ScriptHandle, ToolScript, carries, channel, field,
    inbound, inbound_unaddressed, provider_stub, recv_reply, settle_shape, tool_scripted_provider,
    with_origin, with_reply,
};

/// The outbound edge a fixture's replies arrive on.
type Replies = mpsc::UnboundedReceiver<assistant_core::Outbound>;

/// The fixed line every report in this suite files, under the suite's
/// configured handle.
fn fixture_line() -> String {
    report::report_line(MODERATION_HANDLE)
}

/// The full one-report turn shape on a fresh group conversation: the
/// offense summons the assessment, the call names it, the report files,
/// the result records, the turn closes.
const ASSESSED_TURN: [&str; 7] = [
    "system_prompt",
    "tool_palette",
    "chat_message",
    "tool_call",
    "report",
    "tool_result",
    "text",
];

/// One assembled report fixture over the given provider: helpful
/// answering under the suite's moderation handle — the two conditions the
/// registration takes — plus the outbound edge.
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
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    (fixture, replies)
}

/// The one-turn assessment fixture: the opening turn calls the report tool
/// with the given input, the closing turn answers.
async fn assessing_fixture(
    input: &str,
    narration: Option<String>,
    hold: Option<Arc<support::TurnHold>>,
) -> (support::Fixture, Replies) {
    let (provider, handle) = tool_scripted_provider(
        ToolScript {
            tool: report::NAME.into(),
            input: input.into(),
            narration,
        },
        hold,
    );
    report_fixture_with(provider, handle, ProtectionConfig::default()).await
}

/// Record one offending group line under the given origin — which, under
/// helpful answering, summons the assessment turn itself — and return its
/// receipt.
async fn record_offense(
    fixture: &support::Fixture,
    key: &assistant_core::ChannelKey,
    sender: &str,
    origin: &str,
) -> IngestReceipt {
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(key, ChannelKind::Group, sender, "an offending line"),
            origin,
        ),
    )
    .await
}

/// Pin the group's rules through the observation edge, asserting the
/// acknowledged delta.
async fn pin_rules(fixture: &support::Fixture, key: &assistant_core::ChannelKey, rules: &str) {
    let outcome = fixture
        .assistant
        .observe(Observation {
            channel: key.clone(),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::PinnedAnnouncement(format!("Rules:\n{rules}")),
        })
        .await
        .expect("the rules observation is judged");
    assert!(
        matches!(outcome, ObserveOutcome::Observed { deliver: Some(_) }),
        "the rules delta is observed and acknowledged"
    );
}

/// One scripted step of the sequenced provider: what the next model
/// request draws, in arrival order.
#[derive(Clone, Copy)]
enum Step {
    /// Stream this prose and end the turn; an empty text streams nothing,
    /// so the turn ends with no text and the framework commits the empty
    /// answer block.
    Answer(&'static str),
    /// Call the report tool with this input, narrating first when given
    /// prose; with the hold, announce after the call events and wait for
    /// one permit before the trailing done — the window a test acts in
    /// while the stream is provably open.
    Call {
        input: &'static str,
        narration: Option<&'static str>,
        hold_before_done: bool,
    },
    /// Fail the stream with this error text.
    Fail(&'static str),
    /// Call the report tool twice in one round, both with this input —
    /// the parallel same-origin shape the tool's filing lock serializes.
    TwinCall(&'static str),
}

/// A call step with neither narration nor hold — the common shape.
fn call(input: &'static str) -> Step {
    Step::Call {
        input,
        narration: None,
        hold_before_done: false,
    }
}

/// Build a provider playing one scripted step per model request, in
/// arrival order across the binding — the turns in these stories run
/// strictly in sequence, so the order is deterministic. A request past
/// the script closes with [`CLOSING_ANSWER`]. Returns the release
/// semaphore and the started receiver a holding step announces on.
// The length is the provider's whole wire vocabulary in one match; splitting
// it would scatter the stream shapes the steps exist to keep side by side.
#[allow(clippy::too_many_lines)]
fn sequenced_provider(
    steps: Vec<Step>,
) -> (
    Box<dyn ProviderModule>,
    ScriptHandle,
    Arc<Semaphore>,
    mpsc::UnboundedReceiver<()>,
) {
    let handle = ScriptHandle::fresh();
    let observed = handle.clone();
    let release = Arc::new(Semaphore::new(0));
    let (started_tx, started) = mpsc::unbounded_channel();
    let steps = Arc::new(std::sync::Mutex::new(
        steps.into_iter().collect::<std::collections::VecDeque<_>>(),
    ));
    let permits = Arc::clone(&release);
    let provider = provider_stub("Sequenced", "plays one scripted step per request", {
        move || {
            let (request_tx, mut requests) = mpsc::unbounded_channel();
            let (response_tx, responses) = mpsc::unbounded_channel();
            let turns = Arc::clone(&observed.turns);
            let seen = Arc::clone(&observed.seen);
            let title_requests = Arc::clone(&observed.title_requests);
            let steps = Arc::clone(&steps);
            let started_tx = started_tx.clone();
            let permits = Arc::clone(&permits);
            tokio::spawn(async move {
                let mut calls = 0_usize;
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
                    seen.lock().unwrap().push(messages);
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::Connected));
                    let step = steps
                        .lock()
                        .unwrap()
                        .pop_front()
                        .unwrap_or(Step::Answer(CLOSING_ANSWER));
                    match step {
                        Step::Answer(text) => {
                            if !text.is_empty() {
                                let _ = response_tx
                                    .send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                                let _ = response_tx.send(ProviderResponse::Event(
                                    StreamEvent::TextDelta { text: text.into() },
                                ));
                            }
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::MessageEnd {
                                    usage: agent_ledger::providers::Usage::default(),
                                    stop_reason: agent_ledger::StopReason::EndTurn,
                                },
                            ));
                        }
                        Step::Fail(error) => {
                            let _ = response_tx.send(ProviderResponse::Error(error.into()));
                            continue;
                        }
                        Step::TwinCall(input) => {
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::MessageEnd {
                                    usage: agent_ledger::providers::Usage::default(),
                                    stop_reason: agent_ledger::StopReason::ToolUse,
                                },
                            ));
                            for _ in 0..2 {
                                calls += 1;
                                let _ = response_tx.send(ProviderResponse::Event(
                                    StreamEvent::ToolUseStart {
                                        id: format!("call-{calls}"),
                                        name: report::NAME.into(),
                                    },
                                ));
                                let _ = response_tx.send(ProviderResponse::Event(
                                    StreamEvent::ToolUseInputDelta { json: input.into() },
                                ));
                                let _ = response_tx
                                    .send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                            }
                        }
                        Step::Call {
                            input,
                            narration,
                            hold_before_done,
                        } => {
                            if let Some(narration) = narration {
                                let _ = response_tx
                                    .send(ProviderResponse::Event(StreamEvent::TextBlockStart));
                                let _ = response_tx.send(ProviderResponse::Event(
                                    StreamEvent::TextDelta {
                                        text: narration.into(),
                                    },
                                ));
                            }
                            // The production order: the message end
                            // finalizes any narration before the tool
                            // lifecycle streams.
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::MessageEnd {
                                    usage: agent_ledger::providers::Usage::default(),
                                    stop_reason: agent_ledger::StopReason::ToolUse,
                                },
                            ));
                            calls += 1;
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::ToolUseStart {
                                    id: format!("call-{calls}"),
                                    name: report::NAME.into(),
                                },
                            ));
                            let _ = response_tx.send(ProviderResponse::Event(
                                StreamEvent::ToolUseInputDelta { json: input.into() },
                            ));
                            let _ =
                                response_tx.send(ProviderResponse::Event(StreamEvent::ToolUseEnd));
                            if hold_before_done {
                                let _ = started_tx.send(());
                                match permits.acquire().await {
                                    Ok(permit) => permit.forget(),
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                    let _ = response_tx.send(ProviderResponse::Done);
                }
            });
            (request_tx, responses)
        }
    });
    (provider, handle, release, started)
}

// ─── AC2: the autonomous filing, end to end at the core edge ─────────────

/// The whole flow, block by block: a group message violating the pinned
/// rules summons a helpful-mode turn whose request carries the rules note
/// and the message's bracketed id; the model names that id, the origin
/// validates against the turn's co-summoner set, the report files — the
/// block carries the target origin, the reported principal and the fixed
/// line — and the edge delivers the line as a threaded report BEFORE the
/// answer, while the answer itself quotes nobody: no message this turn
/// absorbed addressed the assistant.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_violating_message_is_assessed_and_the_edge_threads_the_report_before_the_answer() {
    let (fixture, mut replies) =
        assessing_fixture(r#"{"message_id":"origin-spam-1"}"#, None, None).await;
    let key = support::authorized_group(&fixture.assistant, "room-report").await;
    pin_rules(&fixture, &key, "No spam links.").await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    let blocks = settle_shape(
        &fixture.store,
        offense.conversation_id,
        "the assessed turn",
        &[
            "system_prompt",
            "tool_palette",
            "context_note",
            "chat_message",
            "tool_call",
            "report",
            "tool_result",
            "text",
        ],
    )
    .await;

    // The block: target origin, reported principal, the fixed line.
    assert_eq!(field(&blocks[5], "target_origin"), "origin-spam-1");
    assert_eq!(
        blocks[5].fields["reported_principal_id"],
        json!(offense.principal_id),
        "the block names the offending message's sender for erasure"
    );
    assert_eq!(field(&blocks[5], "line"), fixture_line());
    // The tool result claims filing, not arrival.
    assert_eq!(field(&blocks[6], "content"), report::FILED_RESULT);

    // The request the model assessed on: the rules note and the offending
    // message's bracketed id both reached it. Found by the id it carries —
    // the rules pin's own acknowledgment completion (unit 20) is recorded
    // in the same log, so arrival order no longer names the opening turn.
    {
        let requests = fixture.script.seen.lock().unwrap();
        let opening = requests
            .iter()
            .find(|request| request.iter().any(|m| carries(m, "[origin-spam-1]")))
            .expect("the opening request was seen");
        assert!(
            opening
                .iter()
                .any(|m| carries(m, &format!("{RULES_NOTE_LEAD}No spam links."))),
            "the rules note rides the assessed request"
        );
        assert!(
            opening.iter().any(|m| carries(m, "[origin-spam-1]")),
            "the offending message's id is shown to the model"
        );
    }

    // The delivery order: the report first, threaded onto the offending
    // message; then the answer, which threads onto nothing — the offending
    // line never addressed the assistant, helpful answering summoned this
    // turn, and an answer is a reply only to the member who asked
    // (unit 26).
    let first = recv_reply(&mut replies).await;
    assert_eq!(first.kind, ReplyKind::Report);
    assert_eq!(first.text, fixture_line());
    assert_eq!(
        first.reply_target,
        Some(ReplyThread::OntoOnly("origin-spam-1".into())),
        "the report is threaded onto the offending message or not \
         delivered: its line files nothing as a plain message"
    );
    let second = recv_reply(&mut replies).await;
    assert_eq!(second.kind, ReplyKind::Answer);
    assert_eq!(
        second.text,
        support::disclosed(CLOSING_ANSWER),
        "the summoner's first answer opens with the disclosure line"
    );
    assert_eq!(
        second.reply_target, None,
        "nobody addressed the assistant, so the answer quotes nobody"
    );
    let extra = replies.try_recv();
    assert!(extra.is_err(), "one report, one answer; got {extra:?}");
}

/// The report-silence independence (AC2's closing clause): a turn that
/// files and then ends with no text still delivers the report — the
/// empty-answer check swallows the answer alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_that_reports_and_says_nothing_still_delivers_the_report() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(""),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-report-silent").await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;

    let blocks = settle_shape(
        &fixture.store,
        offense.conversation_id,
        "the silent assessment",
        &ASSESSED_TURN,
    )
    .await;
    assert_eq!(
        field(&blocks[6], "content"),
        "",
        "the stored answer is the framework's empty block"
    );

    let only = recv_reply(&mut replies).await;
    assert_eq!(only.kind, ReplyKind::Report, "the report goes out alone");
    assert_eq!(
        only.reply_target,
        Some(ReplyThread::OntoOnly("origin-spam-1".into()))
    );
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "the silent answer delivers nothing: {extra:?}"
    );
}

// ─── AC6: the rules reach the model ──────────────────────────────────────

/// The newest rules note is present in the projected request the model
/// assesses on: two supersessions later, the request carries the newest
/// statement — a durable note never falls out of a windowed history,
/// because the projection folds the whole ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_newest_rules_note_projects_into_the_request_the_model_assesses_on() {
    let (provider, handle) = support::scripted_provider(None);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-rules-context").await;
    pin_rules(&fixture, &key, "Be kind.").await;
    pin_rules(&fixture, &key, "No spam links.").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "42", "what do the rules say?"),
    )
    .await;
    recv_reply(&mut replies).await;
    support::settle(&fixture.store, receipt.conversation_id, "the turn", 6).await;

    let requests = fixture.script.seen.lock().unwrap();
    let request = requests.last().expect("the turn's request was recorded");
    assert!(
        request
            .iter()
            .any(|m| carries(m, &format!("{RULES_NOTE_LEAD}No spam links."))),
        "the newest rules note rides the assessed request"
    );
    assert!(
        request
            .iter()
            .any(|m| carries(m, &format!("{RULES_NOTE_LEAD}Be kind."))),
        "the superseded note stays in stream order behind it; the \
         supersession wording makes the newest authoritative"
    );
}

// ─── AC3: the validated target ───────────────────────────────────────────

/// The anti-aiming decline: a named origin outside the turn's co-summoner
/// set — here, a message an earlier turn already answered — is refused
/// with the pinned copy and nothing files.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_origin_outside_the_turns_assessment_set_is_declined() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        Step::Answer("Noted."),
        call(r#"{"message_id":"origin-old"}"#),
        Step::Answer(CLOSING_ANSWER),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-anti-aiming").await;

    // The earlier message is answered: its debt is settled, so the next
    // turn is not assessing it.
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&key, ChannelKind::Group, "member-3", "the earlier line"),
            "origin-old",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed("Noted.")
    );

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&key, ChannelKind::Group, "member-3", "a fresh line"),
            "origin-new",
        ),
    )
    .await;
    let blocks = settle_shape(
        &fixture.store,
        receipt.conversation_id,
        "the declined aim",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "text",
            "chat_message",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(field(&blocks[6], "error"), report::NOT_ASSESSED_ERROR);
    assert!(
        !blocks.iter().any(|block| block.block_type == "report"),
        "an aim outside the assessment set files nothing"
    );
    assert_eq!(recv_reply(&mut replies).await.text, CLOSING_ANSWER);
}

/// The multi-co-summoner shape the probe raised: several messages absorbed
/// into one turn, and the model names the one violator — only that one is
/// reported, and a co-summoner's stored reply fact steers nothing (the
/// removed reply-target resolution has no successor to fall back on).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn with_several_messages_absorbed_the_model_names_the_one_violator() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) = assessing_fixture(
        r#"{"message_id":"origin-b"}"#,
        Some("One moment.".into()),
        Some(hold.clone()),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-absorbed").await;

    let opener = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&key, ChannelKind::Group, "member-3", "a benign question"),
            "origin-a",
        ),
    )
    .await;
    let conv = opener.conversation_id;

    // Mid-narration — before the call block exists — two more messages
    // are absorbed: the violator, and a bystander whose stored reply
    // points at the OTHER message.
    hold.started().await;
    support::await_ledger(&fixture.store, conv, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
    let violator = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(&key, ChannelKind::Group, "spammer-2", "the violating line"),
            "origin-b",
        ),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            with_origin(
                inbound_unaddressed(&key, ChannelKind::Group, "member-9", "lol that one"),
                "origin-c",
            ),
            ReplyTarget::Message {
                origin: "origin-a".into(),
            },
        ),
    )
    .await;
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the absorbed assessment",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            // The bystander's reply points at a message this conversation
            // holds, so unit 31 lands its quote ahead of it — context the
            // model reads, and no input to what the tool may name.
            "quote",
            "chat_message",
            "text",
            "tool_call",
            "report",
            "tool_result",
            "text",
        ],
    )
    .await;
    // The premise: the newest co-summoner carries a stored reply fact —
    // the shape the removed resolution would have read — and the filed
    // target is the NAMED violator regardless.
    assert_eq!(field(&blocks[5], "reply_target"), "origin-a");
    assert_eq!(field(&blocks[8], "target_origin"), "origin-b");
    assert_eq!(
        blocks[8].fields["reported_principal_id"],
        json!(violator.principal_id),
        "the report names the violator's sender, not the replier's target"
    );
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == "report")
            .count(),
        1,
        "one violator named, one report filed"
    );
    // The narration committed ahead of the filing, so it delivers first;
    // the report follows it, threaded onto the named violator.
    let narration = recv_reply(&mut replies).await;
    assert_eq!(narration.kind, ReplyKind::Answer);
    assert_eq!(narration.text, support::disclosed("One moment."));
    let filed = recv_reply(&mut replies).await;
    assert_eq!(filed.kind, ReplyKind::Report);
    assert_eq!(
        filed.reply_target,
        Some(ReplyThread::OntoOnly("origin-b".into()))
    );
    while !recv_reply(&mut replies)
        .await
        .text
        .ends_with(CLOSING_ANSWER)
    {}
}

// ─── AC4: nothing to report, and the per-origin dedup ────────────────────

/// A quiet assessment files nothing: a turn whose model stays silent
/// without calling the tool leaves no report block and delivers nothing —
/// the judgment half of AC4 lives in the prompt teaching, pinned at the
/// composition.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_turn_that_calls_no_tool_files_nothing() {
    let (provider, handle) = support::scripted_provider(None);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-quiet").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(
            &key,
            ChannelKind::Group,
            "42",
            &format!("members talking among themselves {}", support::SILENT_CUE),
        ),
    )
    .await;
    let blocks =
        support::settle(&fixture.store, receipt.conversation_id, "the quiet turn", 4).await;
    assert!(
        !blocks.iter().any(|block| block.block_type == "report"),
        "no call, no report"
    );
    let extra = replies.try_recv();
    assert!(extra.is_err(), "the quiet turn delivers nothing: {extra:?}");
}

/// The die-after-filing re-summon path, bounded per origin: a turn files
/// and dies, the unanswered message re-co-summons the next turn, the model
/// names the same origin again — and the dedup declines it. The decline is
/// [`report::ALREADY_REPORTED_ERROR`], not the anti-aiming refusal, which
/// is itself the proof the re-summoned message was back in the assessment
/// set. One report block, one delivered report.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reported_message_is_not_reported_again_when_it_re_summons() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Fail("scripted stream failure"),
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(CLOSING_ANSWER),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-dedup").await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let conv = offense.conversation_id;

    // The dying turn still files: the report goes out threaded, ahead of
    // the failure notice.
    let first = recv_reply(&mut replies).await;
    assert_eq!(
        first.kind,
        ReplyKind::Report,
        "the filing outlives the turn"
    );
    assert_eq!(
        first.reply_target,
        Some(ReplyThread::OntoOnly("origin-spam-1".into())),
        "the report is threaded onto the offending message or not \
         delivered: its line files nothing as a plain message"
    );
    let second = recv_reply(&mut replies).await;
    assert_eq!(second.kind, ReplyKind::Notice);
    assert_eq!(second.text, FAILURE_NOTICE);

    // The re-summon: a later message re-engages the conversation, the owed
    // offense joins the turn's assessment set again, and the repeat naming
    // is declined by the dedup.
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "member-7", "the second line"),
    )
    .await;
    let blocks = support::await_ledger(&fixture.store, conv, "the declined repeat", |blocks| {
        blocks.iter().any(|block| block.block_type == "tool_error")
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    let declined = blocks
        .iter()
        .find(|block| block.block_type == "tool_error")
        .expect("the repeat records its decline");
    assert_eq!(field(declined, "error"), report::ALREADY_REPORTED_ERROR);
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == "report")
            .count(),
        1,
        "the re-assessed message filed exactly once"
    );
    let third = recv_reply(&mut replies).await;
    assert_eq!(third.kind, ReplyKind::Answer, "no second report goes out");
    assert_eq!(third.text, support::disclosed(CLOSING_ANSWER));
    let extra = replies.try_recv();
    assert!(extra.is_err(), "nothing further arrives: {extra:?}");
}

/// The parallel half of the dedup: two calls in ONE round naming the same
/// origin — the runner executes same-round calls in parallel tasks — file
/// exactly once, because the tool's filing lock serializes the
/// scan-then-append pair: the second call's scan runs only after the first
/// call's block landed, and the dedup declines it. Removing the filing
/// lock from [`report::ReportTool`] races this test: both scans pass
/// before either append and the same message files twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_parallel_calls_naming_the_same_origin_file_exactly_once() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        Step::TwinCall(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(CLOSING_ANSWER),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-twin-calls").await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let conv = offense.conversation_id;

    let blocks = support::await_ledger(&fixture.store, conv, "the settled twin round", |blocks| {
        blocks.iter().any(|b| b.block_type == "tool_result")
            && blocks.iter().any(|b| b.block_type == "tool_error")
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    assert_eq!(
        blocks
            .iter()
            .filter(|block| block.block_type == "report")
            .count(),
        1,
        "two parallel same-origin calls file exactly one report"
    );
    let filed = blocks
        .iter()
        .find(|block| block.block_type == "tool_result")
        .expect("one of the two calls files");
    assert_eq!(field(filed, "content"), report::FILED_RESULT);
    let declined = blocks
        .iter()
        .find(|block| block.block_type == "tool_error")
        .expect("the other call records its decline");
    assert_eq!(field(declined, "error"), report::ALREADY_REPORTED_ERROR);

    let first = recv_reply(&mut replies).await;
    assert_eq!(first.kind, ReplyKind::Report, "one report goes out");
    assert_eq!(
        first.reply_target,
        Some(ReplyThread::OntoOnly("origin-spam-1".into())),
        "the report is threaded onto the offending message or not \
         delivered: its line files nothing as a plain message"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLOSING_ANSWER)
    );
    let extra = replies.try_recv();
    assert!(extra.is_err(), "no second report delivers: {extra:?}");
}

/// The transient-failure half of the dedup: a filing whose append failed
/// spends nothing — the dedup scan finds no stored report, so the healed
/// re-assessment files cleanly.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_transient_append_failure_leaves_the_origin_reportable() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Fail("scripted stream failure"),
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(CLOSING_ANSWER),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-sabotage").await;

    support::sabotage_appends(&fixture.store, report::REPORT_TABLE).await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let conv = offense.conversation_id;
    let blocks = support::await_ledger(&fixture.store, conv, "the sabotaged turn", |blocks| {
        blocks.iter().any(|block| block.block_type == "tool_error")
    })
    .await;
    let error = blocks
        .iter()
        .find(|block| block.block_type == "tool_error")
        .map(|block| field(block, "error"))
        .expect("the sabotaged filing records its error");
    assert!(
        error.contains("right now"),
        "the failed append is the transient error: {error}"
    );
    assert_eq!(recv_reply(&mut replies).await.text, FAILURE_NOTICE);

    // Healed, the re-summoned assessment files: the failure filed nothing,
    // so the dedup has nothing to decline.
    support::heal_appends(&fixture.store, report::REPORT_TABLE).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "member-7", "the second line"),
    )
    .await;
    support::await_ledger(&fixture.store, conv, "the healed filing", |blocks| {
        blocks.iter().any(|block| block.block_type == "report")
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLOSING_ANSWER)
    );
}

// ─── AC5: the guards ─────────────────────────────────────────────────────

/// The self-report guard over a co-summoner the turn really absorbed: a
/// named message resolving to the assistant's own stored voice declines —
/// reachable now that the model names ids — with the pinned copy, and
/// nothing files. (The unrecorded-principal sibling is pinned at the pure
/// resolution: the schema's NOT NULL keeps that shape out of every stored
/// ledger, so no runtime choreography can build it.)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_named_message_in_the_assistants_own_voice_is_declined() {
    let hold = support::TurnHold::new();
    let (fixture, mut replies) = assessing_fixture(
        r#"{"message_id":"origin-probe"}"#,
        Some("One moment.".into()),
        Some(hold.clone()),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-guards").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "member-7", "watch the next line"),
    )
    .await;
    let conv = receipt.conversation_id;

    // Mid-narration, the probe row is absorbed into the turn's span: a
    // summoned co-summoner in the assistant's own stored voice — no
    // ingestion writes this shape, so the public write surface stands in
    // for the in-principle adapters the guard covers.
    hold.started().await;
    support::await_ledger(&fixture.store, conv, "the streaming tail", |blocks| {
        blocks
            .last()
            .is_some_and(|block| block.block_type.starts_with("streaming"))
    })
    .await;
    let mut fields = serde_json::Map::new();
    fields.insert("text".into(), json!("a probe line"));
    fields.insert("origin".into(), json!("origin-probe"));
    fields.insert("principal_id".into(), json!(1));
    fields.insert("authority".into(), json!("member"));
    fields.insert("addressed".into(), json!(true));
    fields.insert("answer_due".into(), json!(false));
    fixture
        .store
        .append_consumer_block(
            conv,
            Some(agent_ledger::Role::Assistant),
            CHAT_MESSAGE_KIND,
            fields,
            None,
        )
        .await
        .expect("the probe row appends");
    hold.release();

    let blocks = settle_shape(
        &fixture.store,
        conv,
        "the declined probe",
        &[
            "system_prompt",
            "tool_palette",
            "chat_message",
            "chat_message",
            "text",
            "tool_call",
            "tool_error",
            "text",
        ],
    )
    .await;
    assert_eq!(
        field(&blocks[6], "error"),
        report::SELF_REPORT_ERROR,
        "the self-report shape records its pinned decline"
    );
    assert!(
        !blocks.iter().any(|block| block.block_type == "report"),
        "the assistant does not report itself"
    );
    while !recv_reply(&mut replies)
        .await
        .text
        .ends_with(CLOSING_ANSWER)
    {}
}

/// The remaining declines: a call naming no id draws the needs-a-target
/// copy, and a direct conversation draws the group-only copy — before the
/// named origin is even looked at.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_targetless_call_and_a_direct_conversation_are_declined() {
    for (name, dm, input, expected) in [
        ("missing target", false, "{}", report::NEEDS_TARGET_ERROR),
        (
            "direct conversation",
            true,
            r#"{"message_id":"origin-dm"}"#,
            report::GROUP_ONLY_ERROR,
        ),
    ] {
        let (fixture, mut replies) = assessing_fixture(input, None, None).await;
        let receipt = if dm {
            support::ingest_recorded(
                &fixture.assistant,
                with_origin(
                    inbound(
                        &channel("dm-report"),
                        ChannelKind::Direct,
                        "42",
                        "look at this",
                    ),
                    "origin-dm",
                ),
            )
            .await
        } else {
            let key = support::authorized_group(&fixture.assistant, "room-targetless").await;
            record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await
        };
        let blocks = settle_shape(
            &fixture.store,
            receipt.conversation_id,
            "the declined call",
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
            "the {name} call records its pinned decline"
        );
        assert!(
            !blocks.iter().any(|block| block.block_type == "report"),
            "the {name} call files nothing"
        );
        assert_eq!(
            recv_reply(&mut replies).await.text,
            support::disclosed(CLOSING_ANSWER),
            "the {name} turn still closes with the model's answer"
        );
    }
}

/// The debt walk's read-through set carries the report kind: a filed
/// report landing on top of an unanswered message buries nothing at the
/// stamp — the next unaddressed line still carries the debt forward, the
/// same shape as the context-note pin. Dropping the report entry from
/// [`DEBT_READ_THROUGH`] fails this: the report would read as a settled
/// tail and the debt would die under it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn debt_propagation_reads_through_a_filed_report_at_the_stamp() {
    let fixture = support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        ScriptHandle::fresh(),
        ToolSet::new(),
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
            report::Report::stored_fields(
                "origin-spam-1",
                Some(receipt.principal_id),
                &fixture_line(),
            ),
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
    let fixture = support::start_assistant_full(
        Store::in_memory_with(store_config()).expect("an in-memory store opens"),
        support::silent_provider(),
        ScriptHandle::fresh(),
        ToolSet::new(),
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

// ─── Erasure's reach and the fence ───────────────────────────────────────

/// The reported person's erasure nulls the block's target while the line
/// stays, and a report still undelivered at that moment goes out as
/// nothing: the edge skips the targetless report and delivers only the
/// answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reported_persons_erasure_nulls_the_target_and_the_edge_skips_it() {
    let (provider, handle, release, mut started) = sequenced_provider(vec![
        Step::Call {
            input: r#"{"message_id":"origin-spam-1"}"#,
            narration: None,
            hold_before_done: true,
        },
        Step::Answer(CLOSING_ANSWER),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-erased-target").await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let conv = offense.conversation_id;

    // The round holds before its trailing done: the report is filed, the
    // stream provably open, nothing delivered yet.
    tokio::time::timeout(support::DEADLINE, started.recv())
        .await
        .expect("the round holds before its done")
        .expect("the provider outlives the test");
    support::await_ledger(&fixture.store, conv, "the filed report", |blocks| {
        blocks.iter().any(|block| block.block_type == "report")
    })
    .await;
    let outcome = fixture
        .assistant
        .erase_principal(offense.principal_id)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));
    release.add_permits(1);

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
    assert_eq!(only.text, support::disclosed(CLOSING_ANSWER));
    let extra = replies.try_recv();
    assert!(
        extra.is_err(),
        "the targetless report never sends: {extra:?}"
    );
}

/// After the reported person's erasure, a later assessment naming the
/// erased origin resolves nothing: the nulled origin matches no
/// co-summoner, so no report can re-materialize what erasure removed. The
/// trigger's own stored reply target survives as the recorded residual of
/// decision 0063 — a reply recorded after the erasure completed sits
/// where no erasure pass will match it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_erased_targets_origin_cannot_be_re_reported() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        Step::Answer("Noted."),
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(CLOSING_ANSWER),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-re-report").await;
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let conv = offense.conversation_id;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed("Noted.")
    );
    support::await_ledger(&fixture.store, conv, "the first turn's close", |blocks| {
        blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;

    let outcome = fixture
        .assistant
        .erase_principal(offense.principal_id)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));

    let trigger = support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            with_origin(
                inbound_unaddressed(&key, ChannelKind::Group, "member-7", "look at that one"),
                "origin-trigger",
            ),
            ReplyTarget::Message {
                origin: "origin-spam-1".into(),
            },
        ),
    )
    .await;
    let blocks = support::await_ledger(&fixture.store, conv, "the refused repeat", |blocks| {
        blocks.iter().any(|block| block.block_type == "tool_error")
            && blocks.last().is_some_and(|b| b.block_type == "text")
    })
    .await;
    let declined = blocks
        .iter()
        .find(|block| block.block_type == "tool_error")
        .expect("the refused aim records its decline");
    assert_eq!(field(declined, "error"), report::NOT_ASSESSED_ERROR);
    assert!(
        !blocks.iter().any(|block| block.block_type == "report"),
        "nothing re-materializes the erased origin"
    );
    // The recorded residual, pinned as PRESENT on purpose (decision 0063's
    // refinements): the trigger was recorded after the erasure completed,
    // so its row stores the erased person's message identifier where no
    // erasure pass will ever match it. The ingestion-time reach key that
    // closes this ships as its own unit; until it does, this assertion is
    // the tree's record of what stays.
    let trigger_row = blocks
        .iter()
        .find(|block| block.fields.get("origin") == Some(&json!("origin-trigger")))
        .expect("the trigger row is recorded");
    assert_eq!(field(trigger_row, "reply_target"), "origin-spam-1");
    assert_eq!(trigger.conversation_id, conv);
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLOSING_ANSWER),
        "the trigger's sender is a new person; their first answer opens with the line"
    );
}

/// The author-keyed erasure pass nulls a replier's stored reply target
/// beside their prose, while a non-reply stores no target at all — the
/// reply fact is two people's data and erasure reaches it from the
/// author's end here.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_repliers_erasure_nulls_their_reply_target_and_a_non_reply_stores_none() {
    let (provider, handle) = support::scripted_provider(None);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-replier-erasure").await;
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    recv_reply(&mut replies).await;
    let reply_receipt = support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            inbound_unaddressed(&key, ChannelKind::Group, "member-7", "look at this"),
            ReplyTarget::Message {
                origin: "origin-spam-1".into(),
            },
        ),
    )
    .await;
    recv_reply(&mut replies).await;
    support::await_ledger(
        &fixture.store,
        reply_receipt.conversation_id,
        "both turns settled",
        |blocks| {
            blocks
                .iter()
                .filter(|block| block.block_type == "text")
                .count()
                == 2
        },
    )
    .await;

    let outcome = fixture
        .assistant
        .erase_principal(reply_receipt.principal_id)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));
    let blocks = fixture
        .store
        .list_blocks(reply_receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let reply_row = blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .nth(1)
        .expect("the reply row stands");
    assert!(
        reply_row.fields.get("reply_target").is_none(),
        "the author-keyed pass nulled the reply target"
    );
    assert!(
        reply_row.fields.get("text").is_none(),
        "the same pass erased the prose"
    );
    let offense_row = blocks
        .iter()
        .find(|block| block.block_type == CHAT_MESSAGE_KIND)
        .expect("the offending row stands");
    assert!(
        offense_row.fields.get("reply_target").is_none(),
        "a non-reply stored no target to begin with"
    );
}

/// The reported person's erasure also reaches the reply-target copies
/// OTHER people's rows hold: a reply stores the replied-to message's
/// platform id, which is the replied-to person's data wherever it sits.
/// Both erasure passes stay keyed per row: a peer room whose offense
/// carries the SAME platform message id under a DIFFERENT, non-erased
/// sender keeps its replier's stored target and its report's target
/// origin, so a pass widened past its key fails here.
// The length is the two-room story itself: two assessed offenses, two
// repliers, one erasure, and the per-row keying pinned on both sides.
#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reported_persons_erasure_nulls_the_repliers_stored_reply_target() {
    let (provider, handle, _release, _started) = sequenced_provider(vec![
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(CLOSING_ANSWER),
        Step::Answer("Seen."),
        call(r#"{"message_id":"origin-spam-1"}"#),
        Step::Answer(CLOSING_ANSWER),
        Step::Answer("Seen."),
    ]);
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;

    // Room one: the offense is assessed and reported, then a member's
    // reply to it is recorded.
    let key = support::authorized_group(&fixture.assistant, "room-target-pass").await;
    let spammer = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    recv_reply(&mut replies).await;
    support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            inbound_unaddressed(&key, ChannelKind::Group, "member-7", "look at that"),
            ReplyTarget::Message {
                origin: "origin-spam-1".into(),
            },
        ),
    )
    .await;
    recv_reply(&mut replies).await;

    // The peer room: a different sender's offense under the same platform
    // message id — ids are unique only per channel — assessed the same
    // way, with its own replier.
    let peer_key = support::authorized_group(&fixture.assistant, "room-target-pass-peer").await;
    let peer = record_offense(&fixture, &peer_key, "other-9", "origin-spam-1").await;
    assert_ne!(
        peer.principal_id, spammer.principal_id,
        "the peer offense has its own sender"
    );
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    recv_reply(&mut replies).await;
    support::ingest_recorded(
        &fixture.assistant,
        with_reply(
            inbound_unaddressed(&peer_key, ChannelKind::Group, "member-8", "look at that"),
            ReplyTarget::Message {
                origin: "origin-spam-1".into(),
            },
        ),
    )
    .await;
    recv_reply(&mut replies).await;
    support::await_ledger(
        &fixture.store,
        peer.conversation_id,
        "the peer room settled",
        |blocks| {
            blocks
                .iter()
                .filter(|block| block.block_type == "text")
                .count()
                == 2
        },
    )
    .await;

    let outcome = fixture
        .assistant
        .erase_principal(spammer.principal_id)
        .await
        .expect("the erasure runs");
    assert!(matches!(outcome, ErasureOutcome::Erased { .. }));

    let blocks = fixture
        .store
        .list_blocks(spammer.conversation_id)
        .await
        .expect("the ledger reads");
    let reply_row = blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .nth(1)
        .expect("the replier's row stands");
    assert!(
        reply_row.fields.get("reply_target").is_none(),
        "the target-keyed pass nulled the replier's copy of the erased person's message id"
    );
    assert_eq!(
        field(reply_row, "text"),
        "look at that",
        "the replier's own prose stays; only the erased person's identifier left the row"
    );
    let report_row = blocks
        .iter()
        .find(|block| block.block_type == "report")
        .expect("the report block stands");
    assert!(
        report_row.fields.get("target_origin").is_none(),
        "the report pass nulled the erased person's report target"
    );

    // The peer room survives whole: its rows name another person's
    // message under the same platform id.
    let peer_blocks = fixture
        .store
        .list_blocks(peer.conversation_id)
        .await
        .expect("the peer ledger reads");
    let peer_reply = peer_blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .nth(1)
        .expect("the peer replier's row stands");
    assert_eq!(
        field(peer_reply, "reply_target"),
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
        json!(peer.principal_id),
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
    Arc<Semaphore>,
    mpsc::UnboundedReceiver<()>,
) {
    let handle = ScriptHandle::fresh();
    let release = Arc::new(Semaphore::new(0));
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
                    let resolved = messages
                        .iter()
                        .filter(|m| support::carries_tool_result(m))
                        .count();
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
                            StreamEvent::ToolUseInputDelta {
                                json: r#"{"message_id":"origin-spam-1"}"#.into(),
                            },
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

/// A filing racing an erasure waits on the fence: while the erasure holds
/// it exclusively — provably, from its interrupt going out to its loud
/// settle failure — the tool's filing appends nothing, and the report
/// lands only after the erasure released. Deleting the fence hold in the
/// tool's filing fails this test: the report block would land inside the
/// held window.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_filing_racing_an_erasure_waits_on_the_fence() {
    let (provider, handle, release, mut started) = racing_report_provider();
    let (fixture, mut replies) =
        report_fixture_with(provider, handle, ProtectionConfig::default()).await;
    let key = support::authorized_group(&fixture.assistant, "room-fence-race").await;

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
    support::await_ledger(
        &fixture.store,
        dm.conversation_id,
        "the deaf tail",
        |blocks| blocks.iter().any(|b| b.block_type.starts_with("streaming")),
    )
    .await;

    // The group offense reaches the round's pause: the turn is open and
    // the tool has not run.
    let offense = record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    assert_eq!(
        dm.principal_id, offense.principal_id,
        "one platform sender, one person"
    );
    let conv = offense.conversation_id;
    tokio::time::timeout(support::DEADLINE, started.recv())
        .await
        .expect("the round pauses before its call")
        .expect("the provider outlives the test");

    // The erasure of the reported person, driven by polling: its
    // interrupt on the bus proves it holds the fence now, and between
    // polls the future sits still with the fence held — a stable window.
    let mut events = fixture.bus.subscribe();
    let mut erasure = Box::pin(fixture.assistant.erase_principal(offense.principal_id));
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
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLOSING_ANSWER)
    );
}

// ─── AC7: the gating, the palette, and the supersession ──────────────────

/// The stored tool list of one palette block.
fn palette_names(block: &Block) -> Vec<String> {
    serde_json::from_str(&field(block, "tools")).expect("the stored list parses")
}

/// A fresh observation handle for the providers this module builds itself.
fn fresh_handle() -> ScriptHandle {
    ScriptHandle::fresh()
}

/// The full registered set of a moderating deployment, sorted as the
/// palette records it: the three production lookups, the five
/// always-registered tools — the standing lookup, privacy, the react tool,
/// runtime facts and the harness changelog — and the report tool.
fn reporting_palette() -> Vec<String> {
    vec![
        assistant_core::tools::changelog::NAME.into(),
        "lookup_commit".into(),
        "lookup_release".into(),
        "lookup_wiki".into(),
        assistant_core::tools::standing::NAME.into(),
        assistant_core::tools::rights::NAME.into(),
        assistant_core::tools::mark::NAME.into(),
        report::NAME.into(),
        assistant_core::tools::runtime::NAME.into(),
    ]
}

/// The gating, all four corners (AC7): the tool registers and the prompt
/// teaches exactly under a handle plus helpful answering; addressed mode
/// or a missing handle leaves both out — no instruction for a capability
/// that is not there.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_teaching_and_the_tool_gate_on_the_handle_and_helpful_mode() {
    // Handle plus helpful: the recorded prompt carries the teaching and
    // the palette names the tool.
    let (fixture, _replies) = report_fixture_with(
        support::silent_provider(),
        fresh_handle(),
        ProtectionConfig::default(),
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-gating-on").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "42", "recorded under both"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    assert_eq!(
        field(&blocks[0], "content"),
        support::composed_moderating_prompt(),
        "the recorded prompt is the moderating composition"
    );
    assert!(
        field(&blocks[0], "content").contains(MODERATION_TEACHING),
        "the recorded prompt carries the moderation teaching"
    );
    assert!(
        palette_names(&blocks[1]).contains(&report::NAME.to_owned()),
        "the palette names the report tool"
    );

    // Addressed with a handle: no teaching, no tool.
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_reporting_as(
        store,
        support::silent_provider(),
        fresh_handle(),
        ToolSet::new(),
        ProtectionConfig::default(),
        AnsweringMode::Addressed,
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-gating-addressed").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "recorded addressed"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    assert!(
        !field(&blocks[0], "content").contains(MODERATION_TEACHING),
        "addressed mode teaches no moderation even with the handle"
    );
    assert!(
        !palette_names(&blocks[1]).contains(&report::NAME.to_owned()),
        "addressed mode registers no report tool even with the handle"
    );

    // Helpful without a handle: no teaching, no tool.
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_answering(
        store,
        None,
        ProtectionConfig::default(),
        AnsweringMode::Helpful,
    )
    .await;
    let key = support::authorized_group(&fixture.assistant, "room-gating-handleless").await;
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound_unaddressed(&key, ChannelKind::Group, "42", "recorded handleless"),
    )
    .await;
    let blocks = fixture
        .store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    assert!(
        !field(&blocks[0], "content").contains(MODERATION_TEACHING),
        "no handle, no moderation teaching"
    );
    assert!(
        !palette_names(&blocks[1]).contains(&report::NAME.to_owned()),
        "no handle, no registered report tool"
    );
}

/// A pre-unit group conversation whose stored palette predates this unit
/// gains the current tools on its first activity — and because that first
/// activity is itself a summoned assessment, the gained report tool files
/// in the very same turn: the delta append lands ahead of the message, so
/// the turn's admission reads the fresh palette.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pre_unit_palette_gains_the_report_tool_and_files_on_first_activity() {
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
            input: r#"{"message_id":"origin-spam-1"}"#.into(),
            narration: None,
        },
        None,
    );
    let key = channel("room-pre-unit-palette");
    let fixture = support::start_assistant_reporting(
        store,
        provider,
        handle,
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    support::authorize(&fixture.assistant, &key).await;

    // The first activity: the offense lands behind the superseding
    // palette, summons the assessment, and the gained tool files.
    record_offense(&fixture, &key, "spammer-1", "origin-spam-1").await;
    let blocks = support::await_ledger(
        &fixture.store,
        conversation,
        "the superseding palette and the filed report",
        |blocks| {
            blocks
                .iter()
                .filter(|block| block.block_type == "tool_palette")
                .count()
                == 2
                && blocks.iter().any(|block| block.block_type == "report")
                && blocks.last().is_some_and(|b| b.block_type == "text")
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
    assert_eq!(recv_reply(&mut replies).await.kind, ReplyKind::Report);
    assert_eq!(
        recv_reply(&mut replies).await.text,
        support::disclosed(CLOSING_ANSWER)
    );
}

/// No handle configured: the report tool is absent from a fresh
/// conversation's palette — the wiki tool stands with the other lookups —
/// and REMOVED from a pre-existing conversation's palette by the delta
/// append on its first activity under the handleless process.
#[test]
fn without_a_handle_the_report_tool_unregisters_and_the_delta_removes_it() {
    let db = support::TempDb::new("handle-removed");
    let key = channel("room-handle-removed");

    // Process one, handle configured under helpful answering: the group's
    // palette names the full set, report included.
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
                assistant_core::tools::changelog::NAME.to_owned(),
                "lookup_commit".to_owned(),
                "lookup_release".to_owned(),
                "lookup_wiki".to_owned(),
                assistant_core::tools::standing::NAME.to_owned(),
                assistant_core::tools::rights::NAME.to_owned(),
                assistant_core::tools::mark::NAME.to_owned(),
                assistant_core::tools::runtime::NAME.to_owned()
            ],
            "the report tool is removed; the lookups and the unconditional tools stand"
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
