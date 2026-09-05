//! The reaction over the wire (unit 39): the model calls the react tool
//! naming a message it is reading and an emoji, and the adapter places
//! exactly one platform reaction with the list's own bytes. Beside it: an
//! out-of-list pick reaching no platform call at all, a rate-limited
//! placement dropped at once with no sleep and no later item lost, a
//! failing placement drawing no text message in its stead, and the
//! subscription pin that keeps the receiving half unbuilt.
//!
//! The emoji is written as an escape sequence everywhere here, never as a
//! pasted glyph: a literal is what silently gains a variation selector on
//! its way through an editor, and these assertions are about bytes.

use std::sync::Arc;

use assistant_core::tools::ToolSet;
use assistant_core::tools::mark;
use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, TOOL_CLOSING_ANSWER, TempStateFile, ToolScript, message_id_of, recording_sleep,
    spawn_adapter, start_assistant_with_tools,
};

/// The emoji the scripted model picks, in the platform's own bare form.
const CHOSEN: &str = "\u{1F389}";

/// One react call naming the given update's message and the given emoji.
fn react_to(update_id: i64, emoji: &str) -> ToolScript {
    ToolScript {
        tool: mark::NAME.into(),
        input: json!({
            "message_id": message_id_of(update_id).to_string(),
            "emoji": emoji,
        })
        .to_string(),
        narration: None,
        announce: None,
    }
}

/// The whole reaction round trip over the wire: a direct message summons a
/// turn, the scripted model reacts to it, and the wire shows ONE
/// `setMessageReaction` carrying the chat, the message and a one-element
/// emoji-typed reaction array — the entire shape a bot may set, with no
/// custom-emoji parameter anywhere in it, because the request has no place
/// for one. The answer follows on its own path, unaffected.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reaction_reaches_the_wire_as_one_emoji_typed_reaction() {
    let person = 4_100;
    let fixture = start_assistant_with_tools(Some(react_to(1, CHOSEN)), ToolSet::new()).await;
    let server = BotApiServer::start().await;
    server.push_update(support::private_update(1, person, "we shipped it"));

    let state = TempStateFile::new("reaction-e2e");
    let (sleep, slept) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let placements = server.await_recorded("setMessageReaction", 1).await;
    assert_eq!(placements.len(), 1, "one reaction, one call");
    assert_eq!(placements[0].body["chat_id"], json!(person));
    assert_eq!(placements[0].body["message_id"], json!(message_id_of(1)));
    assert_eq!(
        placements[0].body["reaction"],
        json!([{ "type": "emoji", "emoji": CHOSEN }]),
        "the whole request shape: one emoji-typed reaction, the list's own bytes"
    );
    assert_eq!(
        placements[0].body.get("is_big"),
        None,
        "nothing asks the platform to make it large"
    );
    assert!(
        !placements[0].body.to_string().contains("custom_emoji"),
        "no custom-emoji parameter is built, and none can be: {}",
        placements[0].body
    );

    // The answer travels its own path, and the reaction cost no message.
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(support::disclosed(TOOL_CLOSING_ANSWER)),
        "the only message on the wire is the model's answer"
    );
    assert!(
        slept.lock().unwrap().is_empty(),
        "an accepted reaction waits for nothing"
    );
}

/// The selector-blind membership rule at the wire: the model picks the
/// heart in the form a chat client hands out — with its variation
/// selector — and the platform is sent the LIST'S bare bytes, never the
/// model's. This is the one rule that keeps a legal reaction from dropping
/// invisibly.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_selector_carrying_pick_is_sent_as_the_lists_own_bytes() {
    let person = 4_101;
    let fixture =
        start_assistant_with_tools(Some(react_to(1, "\u{2764}\u{FE0F}")), ToolSet::new()).await;
    let server = BotApiServer::start().await;
    server.push_update(support::private_update(1, person, "we shipped it"));

    let state = TempStateFile::new("reaction-selector");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let placements = server.await_recorded("setMessageReaction", 1).await;
    assert_eq!(
        placements[0].body["reaction"][0]["emoji"],
        json!("\u{2764}"),
        "the selector never travels: the platform's own entry is what is sent"
    );
}

/// An emoji outside the platform's reaction set reaches no platform call
/// at all: the adapter drops it before the wire, with a log line, and the
/// model is never told — the tool has already returned, and an act whose
/// whole point is being cheap earns no delivery report. The answer behind
/// it still goes out, which is what proves the consumer moved on rather
/// than stalling.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_out_of_list_emoji_places_nothing_and_calls_nothing() {
    let person = 4_102;
    // A perfectly ordinary emoji the platform's reaction list does not
    // carry — the shape of an honest mis-pick.
    let fixture = start_assistant_with_tools(Some(react_to(1, "\u{1F643}")), ToolSet::new()).await;
    let server = BotApiServer::start().await;
    server.push_update(support::private_update(1, person, "we shipped it"));

    let state = TempStateFile::new("reaction-outside");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // The answer is what tells us the consumer got past the dropped
    // reaction; the placement count is read afterwards, so the read is
    // provably not a race with a call that had not happened yet.
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(support::disclosed(TOOL_CLOSING_ANSWER))
    );
    assert!(
        server.recorded("setMessageReaction").is_empty(),
        "an out-of-list pick never reaches the platform"
    );
}

/// A rate-limited placement is dropped at once: the reaction's ceiling is
/// zero, so any stated wait exceeds it and the request fails immediately —
/// exactly one recorded call, no sleep, and the answer queued behind it
/// delivered normally. The outbound consumer is sequential, so this is
/// what keeps a cosmetic call from parking an answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rate_limited_reaction_is_dropped_at_once_and_delays_nothing() {
    let person = 4_103;
    let fixture = start_assistant_with_tools(Some(react_to(1, CHOSEN)), ToolSet::new()).await;
    let server = BotApiServer::start().await;
    server.script_rate_limited_reactions(30, 4);
    server.push_update(support::private_update(1, person, "we shipped it"));

    let state = TempStateFile::new("reaction-limited");
    let (sleep, slept) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(
        sends[0].body["text"],
        json!(support::disclosed(TOOL_CLOSING_ANSWER)),
        "the answer behind the refused reaction is delivered normally"
    );
    assert_eq!(
        server.recorded("setMessageReaction").len(),
        1,
        "the stated wait is past the reaction's ceiling: one attempt, no retry"
    );
    assert!(
        slept.lock().unwrap().is_empty(),
        "nothing waited for a reaction"
    );
}

/// A placement the platform refuses outright is one log line and nothing
/// else: no retry, and above all no text message in its stead. The only
/// message on the wire is the model's own answer — a fallback would spend
/// a message at the worst possible moment, which is the whole thing a
/// reaction exists to avoid.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_reaction_draws_no_message_in_its_stead() {
    let person = 4_104;
    let fixture = start_assistant_with_tools(Some(react_to(1, CHOSEN)), ToolSet::new()).await;
    let server = BotApiServer::start().await;
    server.fail_reactions();
    server.push_update(support::private_update(1, person, "we shipped it"));

    let state = TempStateFile::new("reaction-refused");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends.len(), 1, "one send: the answer");
    assert_eq!(
        sends[0].body["text"],
        json!(support::disclosed(TOOL_CLOSING_ANSWER)),
        "the refused reaction produced no message of its own"
    );
    assert_eq!(
        server.recorded("setMessageReaction").len(),
        1,
        "the refusal is not retried"
    );
}

/// The receiving half stays unbuilt, and the reason is pinned with it: the
/// poll names neither reaction update type, because the platform delivers
/// both only to an ADMINISTRATOR of the chat while the operator contract
/// keeps this assistant an ordinary member so its reports reach the
/// moderation bot. A future change has to read that reason before removing
/// this.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_poll_subscribes_to_no_reaction_update() {
    let fixture = support::start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("reaction-subscription");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let polls = server.await_recorded("getUpdates", 1).await;
    let allowed = polls[0].body["allowed_updates"]
        .as_array()
        .expect("the poll states its selection instead of inheriting one")
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_owned())
        .collect::<Vec<String>>();
    for never_subscribed in ["message_reaction", "message_reaction_count"] {
        assert!(
            !allowed.contains(&never_subscribed.to_owned()),
            "the assistant is not a chat administrator, so the platform would send \
             {never_subscribed} to it never: {allowed:?}"
        );
    }
}
