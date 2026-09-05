//! The composing indicator over the scripted wire: a model turn shows the
//! platform's typing action before its answer, the refresh provably stops
//! when the turn ends — watched past two refresh periods, on the answered
//! ending and on a failed one whatever its error says — a deterministic
//! reply shows none, and a failing action send leaves the answer's
//! delivery untouched.
//!
//! The determinism fixture is the provider's turn hold: a held turn has
//! recorded the start of its sending tool's call — which raises the core's
//! typing cue, keyed on that call start since unit 55 — but has not
//! completed, so its message provably has not reached the wire and a typing
//! action recorded during the hold came before it. No scheduling race is
//! being bet on.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, TempStateFile, authorize_group, await_chat_messages, await_conversations,
    first_answer_to, group_update, mention_update, private_update, recording_sleep, spawn_adapter,
    start_assistant,
};

/// The stop pins' watch window: longer than two of the adapter's
/// four-second refresh periods, so a refresher still running when a turn
/// ends would re-send the typing action at least twice inside it. A
/// standing action count over this window therefore pins the stop
/// mechanism itself — with the stop deleted, the count keeps moving and
/// these pins fail.
const PAST_TWO_REFRESH_PERIODS: Duration = Duration::from_secs(9);

/// A summoned model turn draws at least one typing action before its
/// answer: the actions recorded during the held turn precede the answer by
/// construction, and at a recorded barrier past the answer the count has
/// not moved. The barrier is one observation instant, not a watch — the
/// sustained no-typing-after-the-turn fact lives in the stop pins below,
/// which hold the count still past two refresh periods.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_model_turn_shows_typing_before_the_answer_and_none_after() {
    let fixture = start_assistant().await;
    let hold = fixture.hold_turns();
    let server = BotApiServer::start().await;
    let chat = -100_640_100;
    let asked = "Does composing show?";
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(mention_update(1, chat, 7, asked));

    let state = TempStateFile::new("composing-shows");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The typing action arrives while the turn is held — its send is
    // recorded but nothing has completed, so no message can have reached
    // the chat — aimed at the summoning chat with the platform's action.
    let actions = server.await_recorded("sendChatAction", 1).await;
    assert_eq!(actions[0].body["chat_id"], json!(chat));
    assert_eq!(actions[0].body["action"], json!("typing"));
    assert!(
        server.recorded("sendMessage").is_empty(),
        "the held turn keeps the answer off the wire while the action is asserted"
    );

    hold.notify_one();
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    let mention = format!("@{} {asked}", support::BOT_USERNAME);
    assert_eq!(sends[0].body["text"], json!(first_answer_to(&mention)));

    // The barrier: an unaddressed message is recorded through the whole
    // pipeline, so everything the answer's turn was going to send has been
    // sent — and the action count has not moved since the answer.
    let actions_at_answer = server.recorded("sendChatAction").len();
    server.push_update(group_update(2, chat, 7, "just chatting"));
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(
        server.recorded("sendChatAction").len(),
        actions_at_answer,
        "no typing action follows the delivered answer"
    );
}

/// A deterministic reply — the privacy command's fixed answer — takes no
/// model turn and draws no typing action. The second command is the
/// barrier: recorded through the whole pipeline with recorded silence, it
/// proves nothing else was going to be sent.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_deterministic_reply_draws_no_typing_action() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let person = 41_640;
    server.push_update(private_update(1, person, "/privacy"));

    let state = TempStateFile::new("composing-deterministic");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(person));
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::PRIVACY_UNPUBLISHED)
    );

    server.push_update(private_update(2, person, "/privacy"));
    let conversation = await_conversations(&fixture.store, 1).await[0];
    await_chat_messages(&fixture.store, conversation, 2).await;
    assert!(
        server.recorded("sendChatAction").is_empty(),
        "a deterministic reply sends instantly; nothing is being composed"
    );
}

/// A failed turn stops the refresh, and it sends nothing at all doing it
/// (unit 49): the adapter's stop-on-reply backstop never fires, so only the
/// core's own stop transition can end the refresh. No other test isolates the core's stop
/// transition this exactly — every other ending also delivers a send that
/// stops the refresher in passing. Two actions are awaited before the
/// release, so the refresh loop is provably looping — not merely showing
/// its first action — when the stop must end it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_turn_stops_the_typing_refresh_without_a_send() {
    let fixture = start_assistant().await;
    fixture.failures.store(1, Ordering::SeqCst);
    let hold = fixture.hold_turns();
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 41_641, "this turn fails"));

    let state = TempStateFile::new("composing-failed-turn");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    server.await_recorded("sendChatAction", 2).await;
    hold.notify_one();

    // No send marks the ending, so the released turn's instant scripted
    // failure is given ample time to land before the watch begins; a
    // refresher stopped inside the settling window still holds the count
    // still through the watch.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let settled = server.recorded("sendChatAction").len();
    tokio::time::sleep(PAST_TWO_REFRESH_PERIODS).await;
    assert!(
        server.recorded("sendMessage").is_empty(),
        "a failed turn sends nothing"
    );
    assert_eq!(
        server.recorded("sendChatAction").len(),
        settled,
        "nothing stopped the refresh on the failed turn"
    );
}

/// The same ending under a differently worded error: the stop is bound to
/// the turn failing and to nothing the failure says. Nothing in the core
/// reads the error text any more, and this test holds that open: a wording
/// that changed an outcome would fail here.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_differently_worded_failure_stops_the_refresh_the_same_way() {
    let fixture = start_assistant().await;
    fixture.word_failures_as("the scripted provider gave up mid-stream");
    fixture.failures.store(1, Ordering::SeqCst);
    let hold = fixture.hold_turns();
    let server = BotApiServer::start().await;
    server.push_update(private_update(1, 41_642, "this turn fails too"));

    let state = TempStateFile::new("composing-worded-failure");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    server.await_recorded("sendChatAction", 2).await;
    hold.notify_one();

    tokio::time::sleep(Duration::from_secs(5)).await;
    let settled = server.recorded("sendChatAction").len();
    tokio::time::sleep(PAST_TWO_REFRESH_PERIODS).await;
    assert!(
        server.recorded("sendMessage").is_empty(),
        "the wording changes nothing: the failed turn sends nothing"
    );
    assert_eq!(
        server.recorded("sendChatAction").len(),
        settled,
        "nothing stopped the refresh on the differently worded failure"
    );
}

/// Two turns in two chats overlap: each chat shows its own typing action,
/// and once both answers are delivered no refresher outlives them — the
/// stops are per chat, and neither chat's ending strands the other's
/// refresher running.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overlapping_turns_type_in_their_own_chats_and_both_stop() {
    let fixture = start_assistant().await;
    let hold = fixture.hold_turns();
    let server = BotApiServer::start().await;
    let (left, right) = (-100_640_300, -100_640_400);
    server.set_admins(left, &[]);
    server.set_admins(right, &[]);
    authorize_group(&fixture.assistant, left).await;
    authorize_group(&fixture.assistant, right).await;
    server.push_update(mention_update(1, left, 7, "the left question"));
    server.push_update(mention_update(2, right, 8, "the right question"));

    let state = TempStateFile::new("composing-overlap");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Both held turns show typing in their own chats before any answer.
    let deadline = std::time::Instant::now() + 2 * support::DEADLINE;
    loop {
        let chats: std::collections::BTreeSet<i64> = server
            .recorded("sendChatAction")
            .iter()
            .filter_map(|action| action.body["chat_id"].as_i64())
            .collect();
        if chats.contains(&left) && chats.contains(&right) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out awaiting typing in both chats; saw {chats:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Release both turns. The hold stores at most one pending release, so
    // one notify per loop pass — paced to let a released turn drain —
    // covers a turn that reaches the hold only after an earlier notify.
    for _ in 0..6 {
        hold.notify_one();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let sends = server.await_recorded("sendMessage", 2).await;
    let answered: std::collections::BTreeSet<i64> = sends
        .iter()
        .filter_map(|send| send.body["chat_id"].as_i64())
        .collect();
    assert!(
        answered.contains(&left) && answered.contains(&right),
        "both chats receive their answers; saw {answered:?}"
    );

    let at_answers = server.recorded("sendChatAction").len();
    tokio::time::sleep(PAST_TWO_REFRESH_PERIODS).await;
    assert_eq!(
        server.recorded("sendChatAction").len(),
        at_answers,
        "a refresher outlived both delivered answers"
    );
}

/// A failing typing action is logged and swallowed: the attempt reaches
/// the wire and fails, and the answer's delivery is untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failing_typing_action_leaves_the_answer_untouched() {
    let fixture = start_assistant().await;
    let hold = fixture.hold_turns();
    let server = BotApiServer::start().await;
    server.fail_chat_actions();
    let chat = -100_640_200;
    let asked = "Does a failed action hurt?";
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(mention_update(1, chat, 7, asked));

    let state = TempStateFile::new("composing-failing");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The attempt was made and answered with the scripted failure.
    server.await_recorded("sendChatAction", 1).await;

    hold.notify_one();
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    let mention = format!("@{} {asked}", support::BOT_USERNAME);
    assert_eq!(sends[0].body["text"], json!(first_answer_to(&mention)));
}
