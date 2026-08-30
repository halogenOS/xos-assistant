//! A bot triggers nothing without a mention (unit 42, 2026-08-30): an
//! automated sender is summoned only by addressing the assistant, its
//! unsummoned message takes no debt of its own and carries nobody else's,
//! and the debt it did not carry waits behind it for the next message
//! entitled to. Programmatic commands stay sender-blind throughout: a bot
//! asking for the privacy notice gets exactly the member's answer.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use agent_ledger::{Block, CoreEvent, EventBus, Store};
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::schema::store_config;
use assistant_core::{
    AnsweringMode, Assistant, Authority, ChannelKind, DeliveryItem, IngestOutcome,
    ProtectionConfig, privacy,
};
use serde_json::json;

use crate::support::{
    self, await_ledger, from_a_bot, inbound, inbound_as, inbound_unaddressed, recv_reply,
};

/// The recorded chat rows of one ledger, in order.
fn message_rows(blocks: &[Block]) -> Vec<&Block> {
    blocks
        .iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect()
}

/// A running assistant whose model never answers, over a fresh store: a
/// summoned message's turn is dispatched and nothing durable comes back,
/// so the ask stays the conversation's owing tail — the shape every stamp
/// below is decided against.
async fn withholding_assistant() -> (Assistant, Store) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let assistant = Assistant::start(
        store.clone(),
        Arc::new(EventBus::<CoreEvent>::new()),
        support::registry_of(support::silent_provider()),
        assistant_core::tools::ToolSet::new(),
        assistant_core::AssemblyConfig {
            started_at: std::time::Instant::now(),
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: assistant_core::DirectChats::default(),
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
    .expect("the assembly starts");
    (assistant, store)
}

/// AC3: in helpful answering — the mode that evaluates every message — a
/// bot's plain group messages are recorded, take no debt, open no turn and
/// consult no budget; the same bot's mentioned message summons a turn,
/// which is also the proof that neither budget counted the plain ones: one
/// answer per window stands on both, and it is still available.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bots_plain_messages_open_no_turn_and_spend_no_budget() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_answering(
        store,
        None,
        support::budgets(Some((1, 600)), Some((1, 600))),
        AnsweringMode::Helpful,
    )
    .await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-bot-quiet").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        from_a_bot(inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "moderation-bot",
            "solve the captcha to stay in the group",
        )),
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        from_a_bot(inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "moderation-bot",
            "welcome, please read the rules",
        )),
    )
    .await;
    let conversation = receipt.conversation_id;

    let blocks = await_ledger(&fixture.store, conversation, "the bot's rows", |blocks| {
        message_rows(blocks).len() == 2
    })
    .await;
    for row in message_rows(&blocks) {
        assert_eq!(
            row.fields["addressed"],
            json!(false),
            "helpful answering summons people; a bot is summoned by address alone"
        );
        assert_eq!(
            row.fields["answer_due"],
            json!(false),
            "no debt of its own, and no tail carried onto it"
        );
        assert!(
            row.fields.get("limited").is_none(),
            "an unsummoned message consults no budget, so none refused it"
        );
        assert!(row.fields.get("debt_authority").is_none());
    }
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        0,
        "a bot's plain message opens no turn at all"
    );

    // The same bot, now addressing the assistant: summoned, answered.
    support::ingest_recorded(
        &fixture.assistant,
        from_a_bot(inbound(
            &room,
            ChannelKind::Group,
            "moderation-bot",
            "which kernel does it run?",
        )),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, room);
    assert!(
        reply.text.contains("which kernel does it run?"),
        "the mentioned message is answered: {}",
        reply.text
    );
    let blocks = await_ledger(
        &fixture.store,
        conversation,
        "the mentioned bot message",
        |blocks| message_rows(blocks).len() == 3,
    )
    .await;
    let mentioned = message_rows(&blocks)[2];
    assert_eq!(mentioned.fields["addressed"], json!(true));
    assert_eq!(mentioned.fields["answer_due"], json!(true));
    assert!(
        mentioned.fields.get("limited").is_none(),
        "both budgets admit it: the plain messages above counted for nothing"
    );
    assert_eq!(fixture.script.turns.load(Ordering::SeqCst), 1);
}

/// `AC3b`: the owed tail waits for a carrier it is entitled to. A member's
/// ask whose turn never ran stays owing; a bot's plain message on its heels
/// stamps false and carries nothing; the next member's message — unaddressed,
/// so its answer-due can only be the carried debt — opens the turn with that
/// debt intact, the walk having read through the bot's row.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_owed_tail_survives_a_bots_message_and_the_next_member_carries_it() {
    let (assistant, store) = withholding_assistant().await;
    let room = support::authorized_group(&assistant, "room-bot-tail").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(&room, ChannelKind::Group, "42", "the never-answered ask"),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        from_a_bot(inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "moderation-bot",
            "solve the captcha to stay in the group",
        )),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "43", "an aside after the bot"),
    )
    .await;

    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let rows = message_rows(&blocks);
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].fields["answer_due"],
        json!(true),
        "the member's ask owes a turn nothing has paid"
    );
    assert_eq!(rows[1].fields["addressed"], json!(false));
    assert_eq!(
        rows[1].fields["answer_due"],
        json!(false),
        "the bot's message takes no debt and carries no one else's"
    );
    assert!(
        rows[1].fields.get("debt_authority").is_none(),
        "a debt it never carried has no authority on it either"
    );
    assert_eq!(rows[2].fields["addressed"], json!(false));
    assert_eq!(
        rows[2].fields["answer_due"],
        json!(true),
        "the walk read through the bot's row: the older debt is still owed"
    );
    assert_eq!(
        rows[2].fields["debt_authority"],
        json!("member"),
        "and it arrives with the authority it was opened at"
    );
}

/// `AC3b`'s other half — the outcome equality the walk widening rests on, on
/// the false-row shape production actually writes: a command's limited
/// false row above a SETTLED tail. Reading through it reaches the same
/// frontier stopping at it did, so the message behind it carries no debt in
/// either answering mode — in the addressed mode nothing is owed at all,
/// and in the helpful mode the row owes its OWN debt at its OWN standing,
/// which a wrongly carried member debt would have lowered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_commands_false_row_above_a_settled_tail_carries_no_debt_either_way() {
    for answering in [AnsweringMode::Addressed, AnsweringMode::Helpful] {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let fixture =
            support::start_assistant_answering(store, None, ProtectionConfig::default(), answering)
                .await;
        let mut replies = fixture
            .assistant
            .replies(support::ADAPTER)
            .await
            .expect("the outbound edge opens");
        let room = support::authorized_group(&fixture.assistant, "room-settled").await;

        // The answered ask settles the conversation: nothing is owed behind
        // the command row appended next.
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            inbound(&room, ChannelKind::Group, "42", "the answered ask"),
        )
        .await;
        recv_reply(&mut replies).await;
        let conversation = receipt.conversation_id;
        support::settle(&fixture.store, conversation, "the answered turn", 4).await;

        support::ingest_recorded(
            &fixture.assistant,
            support::with_command(
                inbound_unaddressed(&room, ChannelKind::Group, "42", privacy::PRIVACY_COMMAND),
                privacy::PRIVACY_COMMAND,
            ),
        )
        .await;

        let mut trailing = inbound_as(
            &room,
            ChannelKind::Group,
            "root-ext",
            Authority::Admin,
            "an administrator's aside",
        );
        trailing.addressed = false;
        support::ingest_recorded(&fixture.assistant, trailing).await;

        let blocks = await_ledger(&fixture.store, conversation, "the trailing row", |blocks| {
            message_rows(blocks).len() == 3
        })
        .await;
        let rows = message_rows(&blocks);
        assert_eq!(
            rows[1].fields["limited"],
            json!("command"),
            "{answering:?}: the command row is the limited false row production writes"
        );
        assert_eq!(rows[1].fields["answer_due"], json!(false));

        let trailing = rows[2];
        match answering {
            AnsweringMode::Addressed => {
                assert_eq!(
                    trailing.fields["answer_due"],
                    json!(false),
                    "an unsummoned message behind a settled tail owes nothing"
                );
                assert!(trailing.fields.get("debt_authority").is_none());
            }
            AnsweringMode::Helpful => {
                assert_eq!(trailing.fields["answer_due"], json!(true));
                assert_eq!(
                    trailing.fields["debt_authority"],
                    json!("admin"),
                    "its own fresh debt at its own standing — no member's debt was carried"
                );
            }
        }
    }
}

/// One recognized command's own answer and receipt, straight off the
/// ingestion: a deterministic reply rides the return, never the outbound
/// edge, so the two senders below are compared on the same value.
async fn notice_answer(
    assistant: &Assistant,
    message: assistant_core::InboundMessage,
) -> (Option<DeliveryItem>, assistant_core::IngestReceipt) {
    let outcome = assistant
        .ingest(message)
        .await
        .expect("the command ingests");
    let IngestOutcome::Recorded { deliver, receipt } = outcome else {
        panic!("the command is recorded");
    };
    (deliver, receipt)
}

/// `AC4b`: programmatic commands are sender-blind. A bot's unmentioned
/// `/privacy` answers the fixed notice exactly as a member's does, under
/// the same per-channel window — one grant each, so the two channels — and
/// opens no model turn; the command row takes the command stamp like any
/// other.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bots_privacy_command_answers_exactly_as_a_members() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_answering(
        store,
        None,
        ProtectionConfig::default(),
        AnsweringMode::Helpful,
    )
    .await;
    let member_room = support::authorized_group(&fixture.assistant, "room-notice-member").await;
    let bot_room = support::authorized_group(&fixture.assistant, "room-notice-bot").await;

    let command = |room, sender| {
        support::with_command(
            inbound_unaddressed(room, ChannelKind::Group, sender, privacy::PRIVACY_COMMAND),
            privacy::PRIVACY_COMMAND,
        )
    };

    let (member_answer, _) = notice_answer(&fixture.assistant, command(&member_room, "A")).await;
    let (bot_answer, bot_receipt) = notice_answer(
        &fixture.assistant,
        from_a_bot(command(&bot_room, "moderation-bot")),
    )
    .await;
    assert_eq!(
        member_answer,
        Some(DeliveryItem::CommandAnswer(
            assistant_core::PRIVACY_UNPUBLISHED.to_owned()
        ))
    );
    assert_eq!(
        bot_answer, member_answer,
        "a bot's rights command answers exactly as anyone's"
    );

    let blocks = await_ledger(
        &fixture.store,
        bot_receipt.conversation_id,
        "the bot's command row",
        |blocks| message_rows(blocks).len() == 1,
    )
    .await;
    let row = message_rows(&blocks)[0];
    assert_eq!(row.fields["limited"], json!("command"));
    assert_eq!(row.fields["answer_due"], json!(false));
    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        0,
        "a deterministic command is no model turn, whoever asked"
    );
}
