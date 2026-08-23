//! The group-context unit over the public wire: the explicit update
//! selection, the membership-driven admission with the scripted leave call
//! (AC5), the pin events' notes with the single acknowledgment (AC2), and
//! the privacy command's suffix forms (AC6) — driven through the scripted
//! Bot API server and asserted on its recorded requests and the ledger.

use std::sync::Arc;

use agent_ledger::providers::{Message, MessageContent, MessageRole};
use serde_json::json;

use crate::server::BotApiServer;
use crate::support::{
    self, OPERATOR_ID, TempStateFile, authorize_group, await_chat_messages, await_conversations,
    group_update, membership_update, pin_update, recording_sleep, spawn_adapter, start_assistant,
};

/// The poll names the update types it consumes on every request: an absent
/// selection would inherit whatever an earlier setting left on the token.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_poll_names_the_update_types_it_consumes() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let state = TempStateFile::new("update-selection");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let polls = server.await_recorded("getUpdates", 1).await;
    assert_eq!(
        polls[0].body["allowed_updates"],
        json!(["message", "edited_message", "my_chat_member"]),
        "the selection is explicit on the wire"
    );
}

/// The operator's add over the wire admits the group: the membership update
/// authorizes it, the first-contact lookup puts the title AND the
/// already-pinned rules on the ledger before anyone speaks — the lookup is
/// the only route for rules pinned before the assistant arrived, since no
/// pin event ever fires for them — the pickup draws the acknowledgment,
/// and a member's message is then answered, with no leave call anywhere.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_operator_add_over_the_wire_admits_the_group() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -600;
    server.set_admins(chat, &[]);
    server.set_chat_info(
        chat,
        "The kernel room",
        Some((1_700_000_000, "Rules:\nBe kind.")),
    );
    server.push_update(membership_update(
        1,
        "group",
        chat,
        OPERATOR_ID,
        "left",
        "member",
    ));
    server.push_update(group_update(
        2,
        chat,
        7,
        &format!("@{} are you with us?", support::BOT_USERNAME),
    ));

    let state = TempStateFile::new("operator-add");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 2).await;
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::RULES_ACKNOWLEDGMENT),
        "the lookup's rules pickup drew the single acknowledgment"
    );
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(sends[1].body["chat_id"], json!(chat));
    assert!(
        server.recorded("leaveChat").is_empty(),
        "the admitted group draws no leave call"
    );

    // The add's lookup put the title and rules notes on the ledger ahead
    // of the message: the group's facts exist before anyone spoke.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let title = blocks
        .iter()
        .find(|block| block.fields.get("topic") == Some(&json!("title")))
        .expect("the lookup's title note is on the ledger");
    assert_eq!(title.fields["text"], json!("The kernel room"));
    let rules = blocks
        .iter()
        .find(|block| block.fields.get("topic") == Some(&json!("rules")))
        .expect("the lookup's rules note is on the ledger");
    assert_eq!(
        rules.fields["text"],
        json!("Be kind."),
        "the wire pin decoded and the prefix line stripped"
    );
    let message = blocks
        .iter()
        .find(|block| block.block_type == "chat_message")
        .expect("the member's message is recorded");
    assert!(
        title.id < message.id && rules.id < message.id,
        "both lookup notes precede the first message"
    );
}

/// A foreign add draws the scripted leave call and records nothing; a
/// replayed foreign add re-returns the directive idempotently (pinned at
/// the core edge), while the performed leave rests per chat — one platform
/// call inside the rest window, not one per replay.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_foreign_add_draws_the_leave_call_and_the_replay_rests() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -601;
    server.push_update(membership_update(1, "group", chat, 555, "left", "member"));
    server.push_update(membership_update(2, "group", chat, 555, "left", "member"));

    let state = TempStateFile::new("foreign-add");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 3).await;
    let leaves = server.recorded("leaveChat");
    assert_eq!(
        leaves.len(),
        1,
        "the replayed directive rests instead of repeating the platform call"
    );
    assert_eq!(leaves[0].body["chat_id"], json!(chat));
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "a foreign add maps no conversation"
    );
}

/// A group message from a channel the operator never admitted draws the
/// leave call and touches no ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_group_message_without_authorization_draws_the_leave_call() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -602;
    server.push_update(group_update(1, chat, 7, "hello from a stranger group"));

    let state = TempStateFile::new("stranger-group");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let leaves = server.await_recorded("leaveChat", 1).await;
    assert_eq!(leaves[0].body["chat_id"], json!(chat));
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "the refused message touched no ledger"
    );
}

/// A stranger group whose lookup succeeds spends exactly one lookup: the
/// core withdraws the looked-up facts, the once-per-process memory is set,
/// and each further message draws only the refusal's own leave call — never a
/// fresh platform lookup. The operator's later add clears the memory, and
/// the admitted group's fresh lookup puts its title on the ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unadmitted_group_spends_one_lookup_and_a_later_add_re_looks() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -605;
    server.set_admins(chat, &[]);
    server.set_chat_info(chat, "The stranger den", None);
    server.push_update(group_update(1, chat, 7, "hello from a stranger group"));
    server.push_update(group_update(2, chat, 7, "still here"));
    server.push_update(membership_update(
        3,
        "group",
        chat,
        OPERATOR_ID,
        "left",
        "member",
    ));

    let state = TempStateFile::new("withdrawn-lookup");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 4).await;
    assert_eq!(
        server.recorded("getChat").len(),
        2,
        "one wasted lookup against the refused chat, one fresh lookup on admission"
    );
    assert_eq!(
        server.recorded("leaveChat").len(),
        1,
        "the refused messages draw one rested leave, never one per message"
    );

    // The admission's fresh lookup put the title on the ledger: the earlier
    // refusal did not strand the group's facts.
    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let note = blocks
        .iter()
        .find(|block| block.block_type == "context_note")
        .expect("the re-looked title note is on the ledger");
    assert_eq!(note.fields["topic"], json!("title"));
    assert_eq!(note.fields["text"], json!("The stranger den"));
}

/// A chat whose lookup fails pays no extra platform call per message: the
/// failure rests, so the second message inside the rest window skips the
/// lookup and draws only the refusal's own leave call. The chat is left
/// unscripted on purpose — its lookup answers the scripted failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_lookup_rests_instead_of_retrying_on_every_message() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -606;
    server.push_update(group_update(1, chat, 7, "hello from a stranger group"));
    server.push_update(group_update(2, chat, 7, "still here"));

    let state = TempStateFile::new("failed-lookup-rest");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 3).await;
    assert_eq!(
        server.recorded("getChat").len(),
        1,
        "the failed lookup rests; the second message draws no fresh platform call"
    );
    assert_eq!(
        server.recorded("leaveChat").len(),
        1,
        "the refusal's leave rests with it; one call for the burst"
    );
}

/// The withdrawal rest suppresses the administrator fetch: a stranger
/// flood draws zero `getChatAdministrators` calls while the chat's
/// withdrawal rests — the core refuses the resting chat before authority
/// is ever read, so a rate-limited list cannot park the sequential batch
/// one bounded wait per refused message.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_chats_flood_draws_zero_admin_fetches_while_the_withdrawal_rests() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -610;
    // The list is scripted to answer, so any fetch WOULD succeed and be
    // recorded: the zero below proves the fetch is skipped, not failing.
    server.set_admins(chat, &[]);
    server.set_chat_info(chat, "The stranger den", None);
    for id in 1..=4 {
        server.push_update(group_update(id, chat, 7, "flooding from a stranger group"));
    }

    let state = TempStateFile::new("resting-flood");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 5).await;
    assert_eq!(
        server.recorded("leaveChat").len(),
        1,
        "one rested leave for the whole burst"
    );
    assert!(
        server.recorded("getChatAdministrators").is_empty(),
        "no administrator fetch is spent on a chat whose withdrawal rests"
    );
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "the refused flood touched no ledger"
    );
}

/// The admission forgets the withdrawal rest (pinned 2026-08-23): a
/// stranger's message records the rest with the refusal's leave, the
/// operator's add lands INSIDE that window, and the admitted chat's next
/// message must resolve authority at once — answered, the offset advancing
/// past it. Were the rest left standing, the message would be delivered
/// with authority unresolved, the core would return the transient refusal,
/// and the batch would halt for the rest's full span — a bounded re-entry
/// of the starvation the deferred authority resolution closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admission_inside_the_withdrawal_rest_resolves_authority_at_once() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -611;
    server.set_admins(chat, &[]);
    server.set_chat_info(chat, "The stranger den", None);
    server.push_update(group_update(1, chat, 7, "hello from a stranger group"));
    server.push_update(membership_update(
        2,
        "group",
        chat,
        OPERATOR_ID,
        "left",
        "member",
    ));
    server.push_update(support::mention_update(3, chat, 7, "are you with us?"));

    let state = TempStateFile::new("rest-forgotten-on-admission");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(support::answer_to(&format!(
            "@{} are you with us?",
            support::BOT_USERNAME
        ))),
        "authority resolved on the first delivery after the admission"
    );
    assert_eq!(
        server.recorded("leaveChat").len(),
        1,
        "the refusal's own leave stands alone; the admitted chat draws none"
    );
    support::await_state_file(state.path(), 4).await;
}

/// A private membership shape — the platform's block and unblock — produces
/// no observation over the wire: no leave call, nothing recorded, and the
/// update acknowledged past.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_private_membership_shape_produces_no_observation_over_the_wire() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    server.push_update(membership_update(1, "private", 5, 5, "member", "kicked"));
    server.push_update(membership_update(2, "private", 5, 5, "kicked", "member"));

    let state = TempStateFile::new("private-membership");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 3).await;
    assert!(server.recorded("leaveChat").is_empty());
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "a private block or unblock is nobody's invitation"
    );
}

/// The verified wedge, closed by the deferred authority resolution
/// (refined 2026-08-23): a stranger group whose administrator list is
/// unreadable — as it is right after our own leave — used to halt the
/// batch BEFORE the core's refusal could run, freezing the offset for
/// every chat. Now the message is delivered with authority unresolved, the
/// core withdraws before reading authority, the leave goes out, and the
/// direct message behind the stranger is still answered with the offset
/// advancing past both.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stranger_group_with_an_unreadable_admin_list_cannot_wedge_the_batch() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let stranger = -607;
    // The wedge shape whole: the admin fetch fails, and the chat's lookup
    // — left unscripted — fails too.
    server.fail_admins(stranger);
    server.push_update(group_update(1, stranger, 7, "hello from a stranger group"));
    server.push_update(support::private_update(2, 42, "a direct message behind it"));

    let state = TempStateFile::new("stranger-wedge");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(42));
    assert_eq!(
        sends[0].body["text"],
        json!(support::answer_to("a direct message behind it")),
        "the direct message behind the stranger is answered"
    );
    let leaves = server.await_recorded("leaveChat", 1).await;
    assert_eq!(leaves[0].body["chat_id"], json!(stranger));
    support::await_state_file(state.path(), 3).await;
}

/// The other half of the deferred resolution: for an ADMITTED group, an
/// unreadable administrator list is still the typed transient refusal the
/// batch halts on — nothing is recorded with a defaulted authority, the
/// offset stays put, and once the list answers again the redelivered
/// update records and is answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admitted_group_with_an_unreadable_admin_list_still_halts_and_recovers() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -608;
    server.set_chat_info(chat, "The admitted room", None);
    authorize_group(&fixture.assistant, chat).await;
    server.fail_admins(chat);
    server.push_update(support::mention_update(1, chat, 7, "are you there?"));

    let state = TempStateFile::new("admitted-halt");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    // Several redelivering polls prove the halt: the update is retried,
    // nothing is recorded, nothing sent, the offset never persisted.
    server.await_recorded("getUpdates", 3).await;
    assert!(
        server.recorded("sendMessage").is_empty(),
        "the halted update is never answered from a defaulted authority"
    );
    assert!(
        !state.path().exists(),
        "the offset stays put while the batch halts"
    );

    // The list answers again: the redelivered update records and answers.
    server.set_admins(chat, &[]);
    let sends = server.await_recorded("sendMessage", 1).await;
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    support::await_state_file(state.path(), 2).await;
}

/// A pin event outranks the lookup's pin (refined 2026-08-23): when the
/// pin event is the chat's first contact, the lookup reports the title
/// only — the stale by-sending-date pin the platform exposes never lands
/// as a note, and the one acknowledgment is spent on the fresh text.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pin_event_outranks_the_lookups_stale_pin() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -609;
    authorize_group(&fixture.assistant, chat).await;
    // The lookup exposes an old rules pin by sending date; the fresh pin
    // event arrives first on the wire.
    server.set_chat_info(
        chat,
        "The kernel room",
        Some((100, "Rules:\nThe stale rules.")),
    );
    server.push_update(pin_update(1, chat, 9, "Rules:\nThe fresh rules."));

    let state = TempStateFile::new("stale-pin-outranked");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 2).await;
    let sends = server.recorded("sendMessage");
    assert_eq!(sends.len(), 1, "one acknowledgment, for the fresh text");
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::RULES_ACKNOWLEDGMENT)
    );

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let rules: Vec<&agent_ledger::Block> = blocks
        .iter()
        .filter(|block| block.fields.get("topic") == Some(&json!("rules")))
        .collect();
    assert_eq!(rules.len(), 1, "exactly one rules note landed");
    assert_eq!(
        rules[0].fields["text"],
        json!("The fresh rules."),
        "the event's text won; the lookup's stale pin never landed"
    );
    let title = blocks
        .iter()
        .find(|block| block.fields.get("topic") == Some(&json!("title")))
        .expect("the title-only lookup still reported the title");
    assert_eq!(title.fields["text"], json!("The kernel room"));
}

/// AC2 end to end: a pin event carrying a rules-prefixed text appends one
/// rules note and delivers exactly one acknowledgment; the same text
/// re-observed appends and acknowledges nothing; a changed text appends
/// again, silently inside the acknowledgment window — pinned block by block
/// on the ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rules_pin_appends_on_delta_and_acknowledges_once_over_the_wire() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -603;
    server.set_chat_info(chat, "The kernel room", None);
    authorize_group(&fixture.assistant, chat).await;
    server.push_update(pin_update(1, chat, 9, "Rules:\n1. Be kind."));
    server.push_update(pin_update(2, chat, 9, "Rules:\n1. Be kind."));
    server.push_update(pin_update(
        3,
        chat,
        9,
        "Rules:\n1. Be kind.\n2. Stay on topic.",
    ));

    let state = TempStateFile::new("rules-pin");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 4).await;
    let sends = server.recorded("sendMessage");
    assert_eq!(sends.len(), 1, "exactly one acknowledgment went out");
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::RULES_ACKNOWLEDGMENT)
    );

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let blocks = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads");
    let shape: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
    assert_eq!(
        shape,
        vec![
            "system_prompt",
            "tool_palette",
            "context_note",
            "context_note",
            "context_note"
        ],
        "the lookup's title note, then one rules note per delta"
    );
    assert_eq!(blocks[2].fields["topic"], json!("title"));
    assert_eq!(blocks[3].fields["topic"], json!("rules"));
    assert_eq!(blocks[3].fields["text"], json!("1. Be kind."));
    assert_eq!(blocks[4].fields["topic"], json!("rules"));
    assert_eq!(
        blocks[4].fields["text"],
        json!("1. Be kind.\n2. Stay on topic.")
    );

    // The next turn projects the notes in the system voice: the mention is
    // answered, and its request carried the newest rules as a system line.
    server.set_admins(chat, &[]);
    server.push_update(support::mention_update(4, chat, 9, "what are the rules?"));
    server.await_recorded("sendMessage", 2).await;
    let seen = fixture.seen.lock().expect("the request log locks");
    let request = seen.last().expect("the turn's request was recorded");
    assert!(
        request.iter().any(|message| {
            message.role == MessageRole::System
                && text_of(message)
                    .contains("The group's rules are now:\n1. Be kind.\n2. Stay on topic.")
        }),
        "the newest rules note reaches the model in the system voice"
    );
}

/// One projected message's whole text, in either content mode.
fn text_of(message: &Message) -> String {
    match &message.content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                agent_ledger::providers::ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// AC6 over the wire, refined 2026-08-23: the own-handle suffix form
/// answers the privacy line with the stored text VERBATIM — the adapter
/// reports the invoked command beside it and the core matches the report —
/// while a foreign-suffix form is recorded as sent, reports no command,
/// and answers nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_privacy_command_answers_the_own_handle_form_and_not_a_foreign_one() {
    let fixture = start_assistant().await;
    let server = BotApiServer::start().await;
    let chat = -604;
    server.set_admins(chat, &[]);
    authorize_group(&fixture.assistant, chat).await;
    // Mixed casing on the handle: normalization is case-insensitive there.
    server.push_update(group_update(
        1,
        chat,
        7,
        &format!("/privacy@{}", support::BOT_USERNAME.to_uppercase()),
    ));
    server.push_update(group_update(2, chat, 7, "/privacy@some_other_bot"));

    let state = TempStateFile::new("privacy-suffix");
    let (sleep, _) = recording_sleep();
    let _adapter = spawn_adapter(&server, state.path(), Arc::clone(&fixture.assistant), sleep);

    support::await_state_file(state.path(), 3).await;
    let sends = server.recorded("sendMessage");
    assert_eq!(sends.len(), 1, "only the own-handle form was answered");
    assert_eq!(sends[0].body["chat_id"], json!(chat));
    assert_eq!(
        sends[0].body["text"],
        json!(assistant_core::PRIVACY_UNPUBLISHED),
        "no address is configured, so the fixed not-published line answers"
    );

    let conversation = await_conversations(&fixture.store, 1).await[0];
    let messages = await_chat_messages(&fixture.store, conversation, 2).await;
    assert_eq!(
        messages[0].fields["text"],
        json!(format!("/privacy@{}", support::BOT_USERNAME.to_uppercase())),
        "the ledger records what the person typed, verbatim"
    );
    assert_eq!(
        messages[0].fields["limited"],
        json!("command"),
        "the reported invocation, not the text, drew the command stamp"
    );
    assert_eq!(
        messages[1].fields["text"],
        json!("/privacy@some_other_bot"),
        "the foreign suffix stands as sent"
    );
    assert!(
        messages[1].fields.get("limited").is_none(),
        "the foreign form takes no command stamp"
    );
}
