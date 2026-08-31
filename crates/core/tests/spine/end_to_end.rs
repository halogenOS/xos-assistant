//! The end-to-end turn (AC4), channel isolation (AC5), the mid-turn arrival
//! (AC6) and the restarted process, all through the public entry point and
//! the outbound edge.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use agent_ledger::{Awaiting, CoreEvent, FromBlock, Role, Store};
use assistant_core::kind::{AssistantKind, CHAT_MESSAGE_KIND};
use assistant_core::schema::store_config;
use assistant_core::{Authority, ChannelKind};
use serde_json::json;

use crate::support;
use crate::support::{
    answer_to, await_ledger, carries, channel, first_answer_to, inbound, recv_reply,
};

/// The wake, proven on the bus: the append put the assistant's kind at the
/// frontier, and the runtime's own conversation state must say so — work due,
/// awaiting a model turn — BEFORE the turn's stream completes. The bus is
/// ordered per subscriber, so the order of the two events IS the proof.
async fn assert_wake_precedes_turn_end(
    events: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
    conversation_id: i64,
) {
    let mut woke = false;
    loop {
        let event = tokio::time::timeout(support::DEADLINE, events.recv())
            .await
            .expect("the owed-turn state and the stream end both reach the bus")
            .expect("the bus outlives the test");
        match event {
            CoreEvent::ConversationState {
                conversation_id: conv,
                latched: false,
                work_due: true,
                awaiting: Some(Awaiting::Model),
            } if conv == conversation_id => woke = true,
            CoreEvent::StreamDone {
                conversation_id: conv,
                ..
            } if conv == conversation_id => {
                assert!(
                    woke,
                    "the owed-turn state must be observable on the bus before \
                     the turn's stream completes"
                );
                return;
            }
            _ => {}
        }
    }
}

/// AC4: one inbound message through the public entry point wakes the runtime,
/// the scripted provider streams the answer, and the outbound edge yields it
/// bound to the correct channel key — asserted on the ledger block by block,
/// with the switched-off title derivation pinned silent (decision 0077).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_inbound_message_becomes_an_outbound_reply() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    // Subscribed BEFORE the ingest so the whole event order is observed.
    let mut events = fixture.bus.subscribe();

    let key = channel("dm-1");
    let mut message = inbound(&key, ChannelKind::Direct, "42", "What is the plan?");
    message.origin = Some("origin-7".into());
    let sent_at = message.timestamp.to_rfc3339();
    let receipt = support::ingest_recorded(&fixture.assistant, message).await;

    let conversations = fixture
        .store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    assert_eq!(conversations.len(), 1, "first message, one conversation");
    let conv = conversations[0].id;
    assert_eq!(
        receipt.conversation_id, conv,
        "the receipt names the conversation the message was recorded in"
    );

    assert_wake_precedes_turn_end(&mut events, conv).await;

    // The outbound edge yields the answer, bound to the channel key — the
    // sender's first answer ever, so it opens with the disclosure line.
    let answer = first_answer_to("What is the plan?");
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, key);
    assert_eq!(reply.text, answer);

    // The ledger, block by block: the recorded prompt, the palette, the
    // recorded message, then the answer.
    let blocks = support::viewed_ledger(&fixture.store, conv, "the answered turn", |blocks| {
        blocks.len() == 4
            && blocks
                .last()
                .is_some_and(|b| b.block_type == "text" && b.fields["content"] == json!(answer))
    })
    .await;
    assert_eq!(blocks[0].block_type, "system_prompt");
    assert_eq!(blocks[1].block_type, "tool_palette");
    assert_eq!(blocks[2].block_type, CHAT_MESSAGE_KIND);
    assert_eq!(blocks[2].role, Some(Role::User));
    assert_eq!(blocks[2].fields["text"], json!("What is the plan?"));
    assert_eq!(blocks[2].fields["authority"], json!("member"));
    assert_eq!(blocks[2].fields["origin"], json!("origin-7"));
    match AssistantKind::from_block(&blocks[2]) {
        AssistantKind::ChatMessage(recorded) => {
            assert_eq!(recorded.text.as_deref(), Some("What is the plan?"));
            assert_eq!(recorded.authority, Some(Authority::Member));
            assert_eq!(
                recorded.principal_id,
                Some(receipt.principal_id),
                "the recorded block carries the receipt's principal id"
            );
            assert_eq!(
                recorded.sent_at.as_deref(),
                Some(sent_at.as_str()),
                "the ledger records the platform's send time, not its own clock"
            );
        }
        AssistantKind::Core(_)
        | AssistantKind::ToolPalette(_)
        | AssistantKind::ContextNote(_)
        | AssistantKind::JoinNotice(_)
        | AssistantKind::Report(_)
        | AssistantKind::Delivered(_)
        | AssistantKind::MessageMark(_)
        | AssistantKind::Retraction(_) => {
            panic!("the stored row resolved through the delegate")
        }
    }
    assert_eq!(blocks[3].role, Some(Role::Assistant));

    // One owed turn, one request, and the projection carried the message's
    // text to the provider.
    assert_eq!(fixture.script.turns.load(Ordering::SeqCst), 1);
    {
        // Scoped so the lock is released before the next await.
        let requests = fixture.script.seen.lock().unwrap();
        assert!(
            requests[0].iter().any(|m| carries(m, "What is the plan?")),
            "the projected messages carry the recorded text: {requests:?}"
        );
    }

    assert_no_title_derivation(&fixture, conv, &mut replies).await;
}

/// Decision 0077's zero pin over the full conversation flow: the answered
/// turn is done, and title derivation — switched off in the assembly —
/// dispatched NOTHING. The window a derivation would fire in is held open,
/// then every surface is read: no title request reached the provider, the
/// conversation's title stays as creation left it, the ledger holds the
/// turn's own blocks and nothing besides them, and nothing leaked onto the
/// outbound edge.
///
/// The ledger half reads the consumer view: the framework's date record is
/// an expected row of the real ledger, written by the day's first append
/// and filtered out here, so the count that follows is about what the
/// assistant's own paths wrote.
async fn assert_no_title_derivation(
    fixture: &support::Fixture,
    conv: i64,
    replies: &mut tokio::sync::mpsc::UnboundedReceiver<assistant_core::Outbound>,
) {
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        fixture.script.title_requests.load(Ordering::SeqCst),
        0,
        "a full conversation flow dispatches zero title requests"
    );
    let conversation = fixture
        .store
        .find_conversation(conv)
        .await
        .expect("the conversation reads")
        .expect("the conversation exists");
    assert_eq!(
        conversation.title, None,
        "the conversation keeps its unset title"
    );
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        1,
        "the answered turn stays the only request of any kind"
    );
    let blocks = support::consumer_view(
        &fixture
            .store
            .list_blocks(conv)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(
        blocks.len(),
        4,
        "the turn's own four blocks, and no derivation artifact beside them: {:?}",
        blocks
            .iter()
            .map(|b| b.block_type.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        matches!(
            replies.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ),
        "nothing further reaches the outbound edge"
    );
}

/// AC5: two messages on different channel keys produce two conversations
/// with no cross-talk — each turn's request carries only its own channel's
/// text, each reply's text is its own channel's answer, and the recorded
/// authority is each message's own. The scripted answers derive from the
/// request, so a reply bound to the other channel's key cannot pass.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_channels_stay_two_conversations() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let key_one = support::authorized_group(&fixture.assistant, "room-1").await;
    let key_two = support::authorized_group(&fixture.assistant, "room-2").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &key_one,
            ChannelKind::Group,
            "42",
            "the first channel's question",
        ),
    )
    .await;
    // The second channel's message arrives from an admin, so the stored
    // authority below is provably each message's own, not a constant — and
    // from a different sender, so each channel's first answer is that
    // person's own first answer and both carry the line deterministically.
    support::ingest_recorded(
        &fixture.assistant,
        support::inbound_as(
            &key_two,
            ChannelKind::Group,
            "57",
            Authority::Admin,
            "the second channel's question",
        ),
    )
    .await;

    // Two replies arrive, in whichever order the two conversations finished,
    // each carrying its OWN channel's answer under its own key.
    let mut expected = HashMap::from([
        (
            key_one.clone(),
            first_answer_to("the first channel's question"),
        ),
        (
            key_two.clone(),
            first_answer_to("the second channel's question"),
        ),
    ]);
    for reply in [
        recv_reply(&mut replies).await,
        recv_reply(&mut replies).await,
    ] {
        let own_answer = expected
            .remove(&reply.channel)
            .expect("each channel key is answered exactly once");
        assert_eq!(
            reply.text, own_answer,
            "the reply on {:?} carries that channel's answer",
            reply.channel
        );
    }
    assert!(expected.is_empty(), "both channels were answered");

    // Two conversations, each holding exactly its own message — with that
    // message's own authority — and its own answer.
    let conversations = fixture
        .store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    assert_eq!(conversations.len(), 2);
    for conversation in &conversations {
        let blocks = support::settle(&fixture.store, conversation.id, "an answered turn", 4).await;
        let text = support::block_text(&blocks[2], "text");
        let other = if text == "the first channel's question" {
            assert_eq!(blocks[2].fields["authority"], json!("member"));
            "the second channel's question"
        } else {
            assert_eq!(text, "the second channel's question");
            assert_eq!(blocks[2].fields["authority"], json!("admin"));
            "the first channel's question"
        };
        assert_eq!(
            support::block_text(&blocks[3], "content"),
            first_answer_to(&text)
        );
        assert!(
            !support::block_text(&blocks[3], "content").contains(other),
            "an answer never carries the other channel's text"
        );
    }

    // Each turn's projected request carried its own text and never the
    // other's.
    assert_eq!(fixture.script.turns.load(Ordering::SeqCst), 2);
    let requests = fixture.script.seen.lock().unwrap();
    for (own, other) in [
        (
            "the first channel's question",
            "the second channel's question",
        ),
        (
            "the second channel's question",
            "the first channel's question",
        ),
    ] {
        let request = requests
            .iter()
            .find(|messages| messages.iter().any(|m| carries(m, own)))
            .unwrap_or_else(|| panic!("one request carries '{own}'"));
        assert!(
            !request.iter().any(|m| carries(m, other)),
            "no request carries the other channel's text"
        );
    }
}

/// AC6: a message appended while the scripted stream is open draws no turn of
/// its own and appears in the next turn's projected context.
///
/// The observed order, stated: message one opens a turn and its stream is
/// held; message two lands in the ledger behind the held stream; the stream
/// ends and the ledger settles as [message one, message two, answer one] with
/// exactly one turn taken — the absorbed message drew none. Message three
/// then opens the second turn, whose projected context carries message two,
/// and the ledger ends [message one, message two, answer one, message three,
/// answer two].
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_mid_turn_message_is_absorbed_into_the_next_turn() {
    let hold = support::TurnHold::new();
    let fixture = support::start_assistant(Some(hold.clone())).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = support::authorized_group(&fixture.assistant, "room-9").await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "message one"),
    )
    .await;
    let turn = hold.started().await;
    assert_eq!(turn, 1, "message one opened the first turn");

    // The stream is open and held. The mid-turn arrival:
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "43", "message two"),
    )
    .await;
    let conversations = fixture
        .store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    assert_eq!(conversations.len(), 1, "one channel, one conversation");
    let conv = conversations[0].id;
    await_ledger(
        &fixture.store,
        conv,
        "message two behind the open stream",
        |blocks| {
            blocks
                .iter()
                .filter(|b| b.block_type == CHAT_MESSAGE_KIND)
                .count()
                == 2
        },
    )
    .await;

    hold.release();
    let reply = recv_reply(&mut replies).await;
    // Both people are new and both co-summoned the held turn — its answer
    // is their shared introduction.
    assert_eq!(reply.text, first_answer_to("message one"));

    // The ledger settles with the absorbed message BEFORE the answer that
    // streamed over it, and no second turn fires for it.
    let blocks = support::settle(&fixture.store, conv, "the settled first turn", 5).await;
    assert_eq!(support::block_text(&blocks[2], "text"), "message one");
    assert_eq!(support::block_text(&blocks[3], "text"), "message two");
    assert_eq!(
        support::block_text(&blocks[4], "content"),
        first_answer_to("message one")
    );
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        1,
        "the absorbed message drew no turn of its own"
    );

    // The next appended message opens the next turn, and the absorbed
    // message joins that turn's projected context.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "42", "message three"),
    )
    .await;
    let turn = hold.started().await;
    assert_eq!(turn, 2, "message three opened the second turn");
    hold.release();
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, answer_to("message three"));

    let blocks = support::settle(&fixture.store, conv, "the settled second turn", 7).await;
    let shape: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
    assert_eq!(
        shape,
        vec![
            "system_prompt",
            "tool_palette",
            CHAT_MESSAGE_KIND,
            CHAT_MESSAGE_KIND,
            "text",
            CHAT_MESSAGE_KIND,
            "text"
        ],
        "the stated order holds"
    );
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        2,
        "two turns in total"
    );
    let requests = fixture.script.seen.lock().unwrap();
    assert!(
        requests[1].iter().any(|m| carries(m, "message two")),
        "the absorbed message appears in the next turn's projected context"
    );
}

/// The restarted process: a channel mapped by an earlier process is answered
/// again, because the ingestion edge releases the per-process boot latch on
/// its first ingestion into the pre-existing conversation — and the stored
/// answer history stays off the new edge instead of flooding the channel.
///
/// Each phase runs on a runtime of its own, torn down between them: two live
/// assemblies over one store is not a supported state, and the teardown is
/// what makes the second phase a restart instead of a second writer.
#[test]
fn a_restarted_process_answers_a_known_channel() {
    let db = support::TempDb::new("restart");
    let key = channel("dm-restart");

    support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the first store opens");
        let fixture = support::start_assistant_on(store, None).await;
        let mut replies = fixture
            .assistant
            .outbound(support::ADAPTER)
            .await
            .expect("the outbound edge opens");
        support::ingest_recorded(
            &fixture.assistant,
            inbound(&key, ChannelKind::Direct, "42", "the first question"),
        )
        .await;
        let reply = recv_reply(&mut replies).await;
        assert_eq!(reply.text, first_answer_to("the first question"));
    });

    // The next process over the same file: the conversation exists in the
    // durable mapping, its actor boots latched, and only the ingestion
    // edge's unlatch lets a turn fire.
    support::process_runtime().block_on(async {
        let store = Store::open_with(db.path(), store_config()).expect("the store reopens");
        let fixture = support::start_assistant_on(store, None).await;
        let mut replies = fixture
            .assistant
            .outbound(support::ADAPTER)
            .await
            .expect("the outbound edge reopens");
        support::ingest_recorded(
            &fixture.assistant,
            inbound(
                &key,
                ChannelKind::Direct,
                "42",
                "the question after the restart",
            ),
        )
        .await;

        let reply = recv_reply(&mut replies).await;
        assert_eq!(reply.channel, key);
        // The stored introduction survives the restart: the person was
        // introduced by the earlier process, so no second line shows.
        assert_eq!(reply.text, answer_to("the question after the restart"));

        // No second conversation was created for the known key, the stored
        // ledger continued with exactly one further turn, and the earlier
        // process's answer was not re-delivered.
        let conversations = fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads");
        assert_eq!(conversations.len(), 1, "the mapping survived the restart");
        let blocks = support::consumer_view(
            &fixture
                .store
                .list_blocks(conversations[0].id)
                .await
                .expect("the ledger reads"),
        );
        let shape: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
        assert_eq!(
            shape,
            vec![
                "system_prompt",
                "tool_palette",
                CHAT_MESSAGE_KIND,
                "text",
                CHAT_MESSAGE_KIND,
                "text"
            ],
            "the restarted process continued the stored ledger with one turn"
        );
        let extra = replies.try_recv();
        assert!(
            matches!(extra, Err(tokio::sync::mpsc::error::TryRecvError::Empty)),
            "the stored answer history stays off a fresh edge; received {extra:?}"
        );
    });
}
