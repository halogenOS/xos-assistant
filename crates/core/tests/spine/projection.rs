//! Role alternation under erasure, closed at the kind's projection shape:
//! the projected request of a ledger holding the two erased shapes from
//! decision 0012 carries no same-role adjacency and no leading assistant
//! message. The fold under test is the same one the runtime feeds every
//! provider request from.

use agent_ledger::providers::{Message, MessageContent, MessageRole, blocks_to_messages};
use agent_ledger::{Block, Role};
use assistant_core::kind::{AssistantKind, CHAT_MESSAGE_KIND, ERASED_MARKER};
use assistant_core::{ChannelKind, ErasureOutcome};
use serde_json::json;

use crate::support::{self, inbound, recv_reply};

/// No two adjacent messages share a role, the first non-system message is
/// not the assistant's, and no message carries empty content — the strict
/// vendors that reject same-role adjacency reject empty content too, so an
/// all-erased run must project the marker, not an empty separator.
fn assert_alternation_holds(messages: &[Message], ledger: &str) {
    for pair in messages.windows(2) {
        assert_ne!(
            pair[0].role, pair[1].role,
            "two same-role messages in a row over {ledger}: {messages:?}"
        );
    }
    for message in messages {
        let empty = match &message.content {
            MessageContent::Text(text) => text.is_empty(),
            MessageContent::Parts(parts) => parts.is_empty(),
        };
        assert!(
            !empty,
            "a message with empty content over {ledger}: {messages:?}"
        );
    }
    let first_spoken = messages
        .iter()
        .find(|message| message.role != MessageRole::System);
    if let Some(first) = first_spoken {
        assert_ne!(
            first.role,
            MessageRole::Assistant,
            "the request opens with the assistant's voice over {ledger}: {messages:?}"
        );
    }
}

/// One synthetic chat-message block; `text: None` builds the erased shape —
/// no text field, the stored NULL's parse.
fn chat_block(id: i64, text: Option<&str>) -> Block {
    let mut fields = serde_json::Map::new();
    if let Some(text) = text {
        fields.insert("text".into(), json!(text));
    }
    fields.insert("principal_id".into(), json!(1));
    fields.insert("authority".into(), json!("member"));
    fields.insert("addressed".into(), json!(true));
    fields.insert("answer_due".into(), json!(true));
    Block {
        id,
        role: Some(Role::User),
        block_type: CHAT_MESSAGE_KIND.into(),
        created_at: String::new(),
        dispatch_anchor: None,
        fields,
    }
}

/// One synthetic finalized answer block in the assistant's voice.
fn answer_block(id: i64, text: &str) -> Block {
    let mut fields = serde_json::Map::new();
    fields.insert("content".into(), json!(text));
    Block {
        id,
        role: Some(Role::Assistant),
        block_type: "text".into(),
        created_at: String::new(),
        dispatch_anchor: None,
        fields,
    }
}

/// The synthetic system prompt block, as the store loads it.
fn prompt_block(id: i64) -> Block {
    let mut fields = serde_json::Map::new();
    fields.insert("content".into(), json!("the prompt"));
    Block {
        id,
        role: Some(Role::System),
        block_type: "system_prompt".into(),
        created_at: String::new(),
        dispatch_anchor: None,
        fields,
    }
}

/// 0012's first erased shape: an erased message in the middle, between its
/// author's neighbours' turns. Without the run-continuity shape the erased
/// block would split the user run and project user, user.
#[test]
fn an_erased_message_in_the_middle_leaves_no_same_role_adjacency() {
    let ledger = vec![
        prompt_block(1),
        chat_block(2, Some("the first ask")),
        answer_block(3, "the first answer"),
        chat_block(4, None),
        chat_block(5, Some("the ask after the erased one")),
        answer_block(6, "the second answer"),
    ];
    let messages = blocks_to_messages::<AssistantKind>(&ledger);
    assert_alternation_holds(&messages, "the middle-erasure ledger");
    // The user run survives whole: one user message carrying the surviving
    // text, with the erased block contributing the marker and none of its
    // prose.
    assert_eq!(messages.len(), 5);
    let MessageContent::Text(text) = &messages[3].content else {
        panic!("the user group renders as text");
    };
    assert_eq!(
        text,
        &format!("{ERASED_MARKER}\n\nthe ask after the erased one")
    );
}

/// 0012's second erased shape: the conversation's opening message erased.
/// Without the run-continuity shape the request would open with the
/// assistant's stored answer.
#[test]
fn an_erased_opening_message_leaves_no_leading_assistant_message() {
    let ledger = vec![
        prompt_block(1),
        chat_block(2, None),
        answer_block(3, "the answer to the erased ask"),
        chat_block(4, Some("the later ask")),
    ];
    let messages = blocks_to_messages::<AssistantKind>(&ledger);
    assert_alternation_holds(&messages, "the front-erasure ledger");
    assert_eq!(messages[0].role, MessageRole::System);
    assert_eq!(
        messages[1].role,
        MessageRole::User,
        "the erased opening stands as the user-voice separator"
    );
    let MessageContent::Text(text) = &messages[1].content else {
        panic!("the separator renders as text");
    };
    assert_eq!(
        text, ERASED_MARKER,
        "the separator is the marker alone — non-empty, with none of the erased prose"
    );
    assert_eq!(messages[2].role, MessageRole::Assistant);
}

/// The same two clauses over a REAL ledger: a group conversation whose first
/// speaker is erased through the public erasure call, folded by the same
/// pass the runtime projects requests with.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_really_erased_group_ledger_projects_alternating() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = support::authorized_group(&fixture.assistant, "room-alternation").await;

    // A asks and is answered; B asks and is answered; then A is erased —
    // the opening message of the ledger becomes the erased shape.
    let receipt_a = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "A", "A's opening ask"),
    )
    .await;
    recv_reply(&mut replies).await;
    support::settle(&fixture.store, receipt_a.conversation_id, "A's turn", 4).await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Group, "B", "B's later ask"),
    )
    .await;
    recv_reply(&mut replies).await;
    let conv = receipt_a.conversation_id;
    support::settle(&fixture.store, conv, "B's turn", 6).await;

    assert_eq!(
        fixture
            .assistant
            .erase_principal(receipt_a.principal_id)
            .await
            .expect("the erasure succeeds"),
        ErasureOutcome::Erased {
            deleted_conversations: vec![],
        }
    );

    let blocks = fixture
        .store
        .list_blocks(conv)
        .await
        .expect("the erased ledger reads");
    let messages = blocks_to_messages::<AssistantKind>(&blocks);
    assert_alternation_holds(&messages, "the really erased group ledger");
    // The positive anchors first, so the clauses above cannot pass over a
    // projection that dropped everything: the six stored blocks project as
    // five messages — the prompt, A's erased opening as the marker, the
    // first answer, B's surviving ask, the second answer, with the palette
    // block contributing nothing — and B's text is in a user-voice message.
    assert_eq!(
        messages.len(),
        5,
        "the erased ledger projects every block's message: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .filter(|message| message.role == MessageRole::User)
            .any(|message| match &message.content {
                MessageContent::Text(text) => text.contains("B's later ask"),
                MessageContent::Parts(_) => false,
            }),
        "the surviving speaker's text projects in a user-voice message"
    );
    // The erased prose is gone from every user-voice message. The
    // assistant's own stored answer may still quote it — erasure reaches
    // the person's messages, and the answer is the assistant's (the group
    // title OPEN in decision 0012 records the same boundary).
    assert!(
        !messages
            .iter()
            .filter(|message| message.role == MessageRole::User)
            .any(|message| match &message.content {
                MessageContent::Text(text) => text.contains("A's opening ask"),
                MessageContent::Parts(_) => false,
            }),
        "the erased prose projects in no user-voice message"
    );
}
