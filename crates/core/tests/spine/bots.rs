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
            retention: assistant_core::RetentionConfig::disabled(),
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
        .outbound(support::ADAPTER)
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

/// `AC3b` across a RUN: the debt survives several unsummoned bot rows, not
/// just one, because the query behind the tail condition skips the whole
/// transparent run in one read. The admin's ask is the origin, so the
/// authority that reaches the carrier is the admin's own — a debt
/// re-opened anywhere in the run would read member instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_run_of_bot_rows_leaves_the_admins_debt_owed_for_the_next_member() {
    let (assistant, store) = withholding_assistant().await;
    let room = support::authorized_group(&assistant, "room-bot-run").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound_as(
            &room,
            ChannelKind::Group,
            "root-ext",
            Authority::Admin,
            "the administrator's never-answered ask",
        ),
    )
    .await;
    for line in [
        "solve the captcha to stay in the group",
        "welcome, please read the rules",
        "the captcha expired",
    ] {
        support::ingest_recorded(
            &assistant,
            from_a_bot(inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "moderation-bot",
                line,
            )),
        )
        .await;
    }
    // The carrier speaks at the administrator's own standing, so the
    // minimum rule folds nothing away and the debt's own authority is
    // readable on the row that carried it.
    let mut aside = inbound_as(
        &room,
        ChannelKind::Group,
        "root-ext",
        Authority::Admin,
        "an aside after the run",
    );
    aside.addressed = false;
    support::ingest_recorded(&assistant, aside).await;

    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let rows = message_rows(&blocks);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].fields["answer_due"], json!(true));
    for row in &rows[1..4] {
        assert_eq!(
            row.fields["answer_due"],
            json!(false),
            "every row of the run stamps false, each read behind the last"
        );
        assert!(row.fields.get("debt_authority").is_none());
    }
    assert_eq!(
        rows[4].fields["answer_due"],
        json!(true),
        "the whole run is transparent: the debt is still owed behind it"
    );
    assert_eq!(
        rows[4].fields["debt_authority"],
        json!("admin"),
        "and it arrives at the standing the administrator opened it with"
    );
}

/// `AC3b` for the carrier the rule deliberately allows: a bot that MENTIONS
/// the assistant is summoned, so its stamp is composed against the owing
/// tail like anyone's and it carries the older debt into its own turn. The
/// carried authority is the minimum of the debt's and the sender's, so an
/// admin-standing bot carrying a member's debt reads member — the proof
/// that what it carries is the member's debt and not a fresh one of its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_mentioned_bots_own_message_carries_the_owed_debt() {
    let (assistant, store) = withholding_assistant().await;
    let room = support::authorized_group(&assistant, "room-bot-carrier").await;

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
        from_a_bot(inbound_as(
            &room,
            ChannelKind::Group,
            "moderation-bot",
            Authority::Admin,
            "@assistant the captcha service is down",
        )),
    )
    .await;

    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let rows = message_rows(&blocks);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].fields["answer_due"], json!(false));
    assert_eq!(
        rows[2].fields["addressed"],
        json!(true),
        "the mention summons the bot's message"
    );
    assert_eq!(rows[2].fields["answer_due"], json!(true));
    assert_eq!(
        rows[2].fields["debt_authority"],
        json!("member"),
        "the member's debt rode through, folded against the sender's own standing"
    );
}

/// The mixed transparent run, in the order production writes one: an
/// erased row and an unsummoned bot's row between a live debt and the row
/// stamped above them. The administrator's deletion command is that row —
/// its own debt is refused by the command stamp, so its answer-due can
/// only be the debt read from behind the run, and the authority proves
/// WHOSE: the member's, not the deleting administrator's. A run that
/// settled on either transparent shape would stamp it false and bury an
/// ask nobody answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_erased_row_and_a_bot_row_are_one_transparent_run_above_a_live_debt() {
    let (assistant, store) = withholding_assistant().await;
    let room = support::authorized_group(&assistant, "room-bot-mixed").await;

    let receipt = support::ingest_recorded(
        &assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "casey-ext",
            "the member's never-answered ask",
        ),
    )
    .await;
    support::ingest_recorded(
        &assistant,
        support::with_origin(
            inbound_unaddressed(&room, ChannelKind::Group, "peer-ext", "a line soon deleted"),
            "gone-2",
        ),
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
        support::deletion_reply(&room, "root-ext", Authority::Admin, "gone-2"),
    )
    .await;

    let blocks = store
        .list_blocks(receipt.conversation_id)
        .await
        .expect("the ledger reads");
    let rows = message_rows(&blocks);
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].fields["answer_due"], json!(true));
    assert!(
        rows[1]
            .fields
            .get("text")
            .and_then(serde_json::Value::as_str)
            .is_none(),
        "the deleted row is erased: transparent by its nulled text, whatever its stamp"
    );
    assert_eq!(
        rows[2].fields["answer_due"],
        json!(false),
        "the bot's row is transparent by its false stamp, its text intact"
    );
    assert_eq!(
        rows[3].fields["limited"],
        json!("command"),
        "the deletion command's own debt is refused"
    );
    assert_eq!(
        rows[3].fields["answer_due"],
        json!(true),
        "so its answer-due is the debt read from behind the mixed run"
    );
    assert_eq!(
        rows[3].fields["debt_authority"],
        json!("member"),
        "the member's debt, not the deleting administrator's standing"
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
            .outbound(support::ADAPTER)
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
    let IngestOutcome::Recorded {
        deliver, receipt, ..
    } = outcome
    else {
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
