//! The deletion mirror (unit 13, AC2–AC4): a group administrator's reply
//! carrying the moderation bot's own deletion command nulls exactly the
//! named row, silently — no reply, no turn, no delivery item — while the
//! command row itself is recorded like any command. The no-ops are silent
//! too: a member's command, a targetless one, an unknown target and an
//! already-erased target all leave the standing state alone. The mirror
//! sits behind the suppression drop and the direct-channel admission, runs
//! inside a turn's absorption window without disturbing the turn, and a
//! deleted message's debt dies with its text while a debt carried by any
//! other row still propagates — a third party's live ask behind the
//! deleted row included, through the walk's read-through (decision 0086).
//! The reply references naming the deleted message are scrubbed with it,
//! the command row's own lawful record excepted (decision 0085).

use agent_ledger::{Block, FromBlock, Projection, Store};
use assistant_core::kind::{AssistantKind, ChatMessage, ERASED_MARKER, LimitedBy};
use assistant_core::mirror::DELETION_COMMAND;
use assistant_core::schema::store_config;
use assistant_core::{
    Authority, ChannelKey, ChannelKind, DeliveryItem, DirectChats, InboundMessage, IngestOutcome,
    ProtectionConfig, ReplyTarget, privacy,
};

use crate::support;

/// One unaddressed group reply carrying the deletion command — the
/// triggering shape, aimed at the given stored origin.
fn del_reply(
    channel: &ChannelKey,
    sender: &str,
    authority: Authority,
    target: &str,
) -> InboundMessage {
    let mut message = support::inbound_as(
        channel,
        ChannelKind::Group,
        sender,
        authority,
        DELETION_COMMAND,
    );
    message.addressed = false;
    support::with_command(
        support::with_reply(
            message,
            ReplyTarget::Message {
                origin: target.into(),
            },
        ),
        DELETION_COMMAND,
    )
}

/// The recorded chat messages of one ledger, in order.
fn chat_messages(blocks: &[Block]) -> Vec<ChatMessage> {
    blocks
        .iter()
        .filter_map(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => Some(message),
            _ => None,
        })
        .collect()
}

/// Ingest one message that must come back recorded with NOTHING to
/// deliver — the mirror's silence, asserted at every call site.
async fn ingest_silent(fixture: &support::Fixture, message: InboundMessage) {
    match fixture
        .assistant
        .ingest(message)
        .await
        .expect("the message ingests")
    {
        IngestOutcome::Recorded { deliver, .. } => {
            assert_eq!(deliver, None, "the mirror delivers nothing");
        }
        refused => panic!("the message was refused: {refused:?}"),
    }
}

/// AC2, block by block: the administrator's reply deletion nulls exactly
/// the target row — text, origin, send time, reply reference and speaker —
/// the placeholder projects the erased marker under its stored voice, the
/// command row is recorded with the command stamp, and nothing goes out
/// for it: the first item the outbound edge ever yields is a later
/// canary's answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn an_administrators_reply_deletion_nulls_exactly_the_target_row() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let mut replies = assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(assistant, "room-mirror").await;

    let target = support::with_reply(
        support::with_origin(
            support::with_username(
                support::inbound_unaddressed(
                    &room,
                    ChannelKind::Group,
                    "casey-ext",
                    "an offending line",
                ),
                "casey",
            ),
            "target-1",
        ),
        ReplyTarget::Message {
            origin: "earlier-9".into(),
        },
    );
    let receipt = support::ingest_recorded(assistant, target).await;
    let conversation = receipt.conversation_id;

    let before = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(before.len(), 1);
    assert!(
        before[0].text.is_some()
            && before[0].origin.is_some()
            && before[0].sent_at.is_some()
            && before[0].speaker.is_some()
            && before[0].reply_target.is_some(),
        "the five personal columns stand before the mirror — the delta below is provable"
    );

    let mut command = del_reply(&room, "root-ext", Authority::Admin, "target-1");
    command = support::with_origin(command, "del-1");
    ingest_silent(&fixture, command).await;

    let after = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads after the mirror"),
    );
    assert_eq!(after.len(), 2, "the target row and the command row");

    let erased = &after[0];
    assert_eq!(erased.text, None, "the prose is nulled");
    assert_eq!(erased.origin, None, "the origin reference is nulled");
    assert_eq!(erased.sent_at, None, "the platform send time is nulled");
    assert_eq!(erased.reply_target, None, "the reply reference is nulled");
    assert_eq!(erased.speaker, None, "the speaker is nulled");
    assert_eq!(
        erased.principal_id,
        Some(receipt.principal_id),
        "the structural columns stay: the row still names its principal"
    );
    assert_eq!(erased.authority, Some(Authority::Member));
    assert_eq!(erased.addressed, Some(false));
    assert_eq!(
        erased.group_role(),
        Some(agent_ledger::Role::User),
        "the placeholder keeps its stored voice"
    );
    assert_eq!(
        erased.llm_text().as_deref(),
        Some(ERASED_MARKER),
        "the placeholder projects the erased marker, none of the prose"
    );

    let recorded = &after[1];
    assert_eq!(
        recorded.text.as_deref(),
        Some(DELETION_COMMAND),
        "the command row records the request verbatim — the lawful record"
    );
    assert_eq!(
        recorded.limited,
        Some(LimitedBy::Command),
        "the command stamp keeps the mirror out of the answer machinery"
    );
    assert_eq!(recorded.answer_due, Some(false));
    assert_eq!(recorded.authority, Some(Authority::Admin));
    assert_eq!(recorded.origin.as_deref(), Some("del-1"));
    assert_eq!(
        recorded.reply_target.as_deref(),
        Some("target-1"),
        "the command row keeps its reply reference: the request is the record"
    );

    // The silence, ordered: a canary's answer is the FIRST item the edge
    // ever yields, so the mirror provably sent nothing ahead of it.
    let canary = support::ingest_recorded(
        assistant,
        support::inbound(&room, ChannelKind::Group, "bystander", "a canary question"),
    )
    .await;
    support::settle(&fixture.store, canary.conversation_id, "the canary turn", 6).await;
    let first = support::recv_reply(&mut replies).await;
    assert_eq!(
        first.text,
        support::first_answer_to(&format!(
            "{ERASED_MARKER}\n\n{DELETION_COMMAND}\n\na canary question"
        )),
        "the first outbound item is the canary's answer — derived from a projection \
         holding the marker and the verbatim command, never the erased prose — with \
         nothing of the mirror's ahead of it"
    );
}

/// AC3: the four silent no-ops — a member's command, a targetless command
/// (a reply to the assistant's own message included: no origin rides it),
/// an unknown target and an already-erased target — null nothing beyond
/// the standing state and deliver nothing; each command row is recorded,
/// the non-triggering ones as ordinary messages.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn the_silent_no_ops_leave_the_standing_state_alone() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-noop").await;

    let receipt = support::ingest_recorded(
        assistant,
        support::with_origin(
            support::with_username(
                support::inbound_unaddressed(
                    &room,
                    ChannelKind::Group,
                    "casey-ext",
                    "the standing line",
                ),
                "casey",
            ),
            "target-a",
        ),
    )
    .await;
    let conversation = receipt.conversation_id;
    let standing_line = |messages: &[ChatMessage]| {
        messages
            .first()
            .expect("the target row stands")
            .text
            .clone()
    };

    // A member's deletion command mirrors nothing and records ordinarily.
    ingest_silent(
        &fixture,
        del_reply(&room, "peer-ext", Authority::Member, "target-a"),
    )
    .await;
    // An administrator's command without a reply names nothing.
    let mut bare = support::inbound_as(
        &room,
        ChannelKind::Group,
        "root-ext",
        Authority::Admin,
        DELETION_COMMAND,
    );
    bare.addressed = false;
    ingest_silent(&fixture, support::with_command(bare, DELETION_COMMAND)).await;
    // A reply to the assistant's own message carries no origin to name.
    let mut to_assistant = support::inbound_as(
        &room,
        ChannelKind::Group,
        "root-ext",
        Authority::Admin,
        DELETION_COMMAND,
    );
    to_assistant.addressed = false;
    ingest_silent(
        &fixture,
        support::with_command(
            support::with_reply(to_assistant, ReplyTarget::AssistantMessage),
            DELETION_COMMAND,
        ),
    )
    .await;
    // An administrator's command naming a target the store never held.
    ingest_silent(
        &fixture,
        del_reply(&room, "root-ext", Authority::Admin, "never-recorded"),
    )
    .await;

    let messages = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(messages.len(), 5, "the target and four recorded commands");
    assert_eq!(
        standing_line(&messages).as_deref(),
        Some("the standing line"),
        "no no-op touched the target"
    );
    for row in &messages[1..] {
        assert!(
            row.text.is_some(),
            "every command row keeps its own recorded text"
        );
    }
    assert_eq!(
        messages[1].limited, None,
        "a member's deletion command is an ordinary message"
    );
    assert_eq!(
        messages[2].limited, None,
        "a targetless deletion command is an ordinary message"
    );
    assert_eq!(
        messages[3].limited, None,
        "a reply to the assistant's own message is an ordinary message"
    );
    assert_eq!(
        messages[4].limited,
        Some(LimitedBy::Command),
        "an unknown target is still the recognized command — the trigger \
         reads the message, never the store"
    );

    // The triggering command erases the target; its repeat is the
    // idempotent no-op: still erased, nothing else moved.
    ingest_silent(
        &fixture,
        del_reply(&room, "root-ext", Authority::Admin, "target-a"),
    )
    .await;
    let erased_once = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads after the mirror"),
    );
    assert_eq!(standing_line(&erased_once), None, "the target is erased");

    ingest_silent(
        &fixture,
        del_reply(&room, "root-ext", Authority::Admin, "target-a"),
    )
    .await;
    let erased_twice = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads after the repeat"),
    );
    assert_eq!(
        erased_twice.len(),
        erased_once.len() + 1,
        "the repeat adds its command row and nothing else"
    );
    for (before, after) in erased_once.iter().zip(&erased_twice) {
        assert_eq!(
            (&before.text, &before.origin, &before.speaker),
            (&after.text, &after.origin, &after.speaker),
            "an already-erased target mirrors nothing, idempotently"
        );
    }
}

/// AC4: the mirror inside a turn's absorption window does not disturb the
/// turn — the held turn's answer arrives exactly as scripted while the
/// target row is provably nulled mid-turn, and exactly one turn ran.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_mirror_inside_an_absorption_window_leaves_the_turn_untouched() {
    let hold = support::TurnHold::new();
    let fixture = support::start_assistant(Some(std::sync::Arc::clone(&hold))).await;
    let assistant = &fixture.assistant;
    let mut replies = assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(assistant, "room-absorb").await;

    support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound_unaddressed(&room, ChannelKind::Group, "casey-ext", "a line to erase"),
            "absorb-target",
        ),
    )
    .await;
    let receipt = support::ingest_recorded(
        assistant,
        support::inbound(&room, ChannelKind::Group, "asker-ext", "please answer this"),
    )
    .await;
    hold.started().await;

    // The stream is provably open; the mirror runs inside the window.
    ingest_silent(
        &fixture,
        del_reply(&room, "root-ext", Authority::Admin, "absorb-target"),
    )
    .await;
    let mid_turn = chat_messages(
        &fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the mid-turn ledger reads"),
    );
    assert_eq!(
        mid_turn[0].text, None,
        "the target is erased while the turn still streams"
    );

    hold.release();
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the held turn's answer",
        6,
    )
    .await;
    assert_eq!(
        support::recv_reply(&mut replies).await.text,
        support::first_answer_to("a line to erase\n\nplease answer this"),
        "the absorbed mirror left the turn's answer exactly as scripted — the \
         request was projected before the mirror, so the prose it folded is the \
         pre-erasure text"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one turn ran; the mirror summoned none"
    );
}

/// AC4: a mirrored message that carried an unanswered debt leaves the
/// conversation's liveness intact. Deleting the owing tail lets its debt
/// die with the erased text — the shared owes-answer reading covers the
/// erased row, so the command row is stamped debt-free and no ghost debt
/// haunts later traffic, while a fresh ask still opens its own debt. A
/// debt already carried forward by another row is untouched: the mirror
/// erases one row, and the tail's own stamp still propagates.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_deleted_messages_debt_dies_while_a_carried_debt_propagates() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let assistant = &fixture.assistant;

    // The owing tail itself is deleted: its debt dies with the text.
    let room = support::authorized_group(assistant, "room-debt").await;
    let receipt = support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound(&room, ChannelKind::Group, "casey-ext", "answer me"),
            "debt-target",
        ),
    )
    .await;
    ingest_silent(
        &fixture,
        del_reply(&room, "root-ext", Authority::Admin, "debt-target"),
    )
    .await;
    support::ingest_recorded(
        assistant,
        support::inbound_unaddressed(&room, ChannelKind::Group, "peer-ext", "later prose"),
    )
    .await;
    support::ingest_recorded(
        assistant,
        support::inbound(&room, ChannelKind::Group, "peer-ext", "a fresh question"),
    )
    .await;
    let messages = chat_messages(
        &fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(
        messages[0].answer_due,
        Some(true),
        "the ask owed at its write"
    );
    assert_eq!(messages[0].text, None, "the ask is erased");
    assert!(
        !messages[0].owes_answer(),
        "the erased row's debt is cancelled"
    );
    assert_eq!(
        messages[1].answer_due,
        Some(false),
        "the command row is stamped against the post-mirror tail: the debt died"
    );
    assert_eq!(
        messages[2].answer_due,
        Some(false),
        "no ghost debt reaches later unaddressed traffic"
    );
    assert_eq!(
        (messages[3].answer_due, messages[3].debt_authority),
        (Some(true), Some(Authority::Member)),
        "a fresh ask opens its own debt — the conversation stayed live"
    );

    // The debt was already carried forward by another row: it stands.
    let carried = support::authorized_group(assistant, "room-carried").await;
    let carried_receipt = support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound(&carried, ChannelKind::Group, "casey-ext", "the first ask"),
            "keep-1",
        ),
    )
    .await;
    support::ingest_recorded(
        assistant,
        support::inbound_unaddressed(&carried, ChannelKind::Group, "peer-ext", "a second line"),
    )
    .await;
    ingest_silent(
        &fixture,
        del_reply(&carried, "root-ext", Authority::Admin, "keep-1"),
    )
    .await;
    let carried_rows = chat_messages(
        &fixture
            .store
            .list_blocks(carried_receipt.conversation_id)
            .await
            .expect("the carried ledger reads"),
    );
    assert_eq!(carried_rows[0].text, None, "the first ask is erased");
    assert_eq!(
        carried_rows[2].answer_due,
        Some(true),
        "the debt the second row already carries propagates through the command row"
    );
}

/// The mirror scrubs the reply references that named the deleted message
/// (decision 0085): every replier row's stored copy of the target's origin
/// is nulled with the target, because the person-wide reply pass joins on
/// the very origin the mirror nulls and could never reach those copies
/// afterwards. The deletion command row keeps its own reference — the
/// request's lawful record, appended after the pass ran — and a later
/// erasure of its administrator clears that copy too, so after the mirror
/// an erasure of the target's author finds nothing left dangling.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn the_mirror_scrubs_the_reply_references_naming_the_deleted_message() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-scrub").await;

    let author = support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "casey-ext",
                "a line replied to",
            ),
            "scrub-target",
        ),
    )
    .await;
    support::ingest_recorded(
        assistant,
        support::with_reply(
            support::inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "peer-ext",
                "a reply that names it",
            ),
            ReplyTarget::Message {
                origin: "scrub-target".into(),
            },
        ),
    )
    .await;
    let admin = support::ingest_recorded(
        assistant,
        del_reply(&room, "root-ext", Authority::Admin, "scrub-target"),
    )
    .await;
    let conversation = author.conversation_id;

    let rows = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads after the mirror"),
    );
    assert_eq!(rows.len(), 3, "the target, the replier and the command row");
    assert_eq!(rows[0].text, None, "the target is erased");
    assert_eq!(
        rows[1].text.as_deref(),
        Some("a reply that names it"),
        "the replier's own prose stays"
    );
    assert_eq!(
        rows[1].reply_target, None,
        "the replier's copy of the deleted origin is nulled with the target"
    );
    assert_eq!(
        rows[2].reply_target.as_deref(),
        Some("scrub-target"),
        "the command row keeps the request's lawful record"
    );

    // The author's later erasure completes over the mirrored state and
    // leaves the command row's record as the one remaining reference.
    let outcome = assistant
        .erase_principal(author.principal_id)
        .await
        .expect("the author's erasure succeeds");
    assert!(
        matches!(outcome, assistant_core::ErasureOutcome::Erased { .. }),
        "the author's erasure reaches nothing dangling: {outcome:?}"
    );
    let after = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads after the author's erasure"),
    );
    let naming_target: Vec<usize> = after
        .iter()
        .enumerate()
        .filter(|(_, row)| row.reply_target.as_deref() == Some("scrub-target"))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        naming_target,
        vec![2],
        "the command row's lawful record is the one copy that outlives the author"
    );

    // And the administrator's own erasure clears even that copy: no
    // reference to the deleted message survives every route.
    assistant
        .erase_principal(admin.principal_id)
        .await
        .expect("the administrator's erasure succeeds");
    let cleared = chat_messages(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads after the administrator's erasure"),
    );
    assert!(
        cleared.iter().all(|row| row.reply_target.is_none()),
        "no copy of the deleted origin survives the administrator's erasure"
    );
}

/// AC4's liveness, the carried side (decision 0086): deleting the CARRIER
/// — the tail that owed nothing itself but propagated a third party's
/// unanswered ask — leaves that ask alive. The walk reads through the
/// erased row to the live debt behind it, so the command row is stamped
/// answer-due at the carried authority and later traffic still carries the
/// debt; only a deleted row's OWN ask dies with its text.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deleting_the_carrying_tail_keeps_a_third_partys_debt_alive() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-carrier").await;

    let receipt = support::ingest_recorded(
        assistant,
        support::inbound(&room, ChannelKind::Group, "casey-ext", "the standing ask"),
    )
    .await;
    support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "peer-ext",
                "a line that carries it",
            ),
            "carrier-1",
        ),
    )
    .await;
    ingest_silent(
        &fixture,
        del_reply(&room, "root-ext", Authority::Admin, "carrier-1"),
    )
    .await;
    support::ingest_recorded(
        assistant,
        support::inbound_unaddressed(&room, ChannelKind::Group, "peer-ext", "later prose"),
    )
    .await;

    let rows = chat_messages(
        &fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[1].text, None, "the carrier is erased");
    assert_eq!(
        rows[1].answer_due,
        Some(true),
        "the carrier's stamp stays: structure, not personal data"
    );
    assert!(
        !rows[1].owes_answer(),
        "the erased carrier itself owes nothing"
    );
    assert_eq!(
        (rows[2].answer_due, rows[2].debt_authority),
        (Some(true), Some(Authority::Member)),
        "the command row's stamp read through the erased carrier to the live ask behind it"
    );
    assert_eq!(
        rows[3].answer_due,
        Some(true),
        "the third party's debt still reaches later traffic: the conversation stayed live"
    );
}

/// AC4: the suppression drop precedes the mirror — an opted-out
/// administrator's deletion command is disregarded whole: no command row,
/// no nulls, nothing delivered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_suppression_drop_precedes_the_mirror() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-supp").await;

    let receipt = support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound_unaddressed(&room, ChannelKind::Group, "casey-ext", "a kept line"),
            "supp-target",
        ),
    )
    .await;

    let mut opt_out = support::inbound_as(
        &room,
        ChannelKind::Group,
        "root-ext",
        Authority::Admin,
        privacy::OPT_OUT_COMMAND,
    );
    opt_out.addressed = false;
    match assistant
        .ingest(support::with_command(opt_out, privacy::OPT_OUT_COMMAND))
        .await
        .expect("the opt-out ingests")
    {
        IngestOutcome::Recorded { deliver, .. } => assert_eq!(
            deliver,
            Some(DeliveryItem::CommandAnswer(privacy::OPT_OUT_DONE.into())),
            "the administrator's own opt-out answers"
        ),
        refused => panic!("the opt-out was refused: {refused:?}"),
    }

    assert_eq!(
        assistant
            .ingest(del_reply(
                &room,
                "root-ext",
                Authority::Admin,
                "supp-target"
            ))
            .await
            .expect("the suppressed command ingests"),
        IngestOutcome::Disregarded,
        "the standing flag drops the command before the mirror sees it"
    );
    let messages = chat_messages(
        &fixture
            .store
            .list_blocks(receipt.conversation_id)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(
        messages.iter().filter(|m| m.text.is_some()).count(),
        messages.len(),
        "nothing was nulled"
    );
    assert_eq!(
        messages
            .last()
            .expect("the ledger holds rows")
            .text
            .as_deref(),
        Some(privacy::OPT_OUT_COMMAND),
        "the dropped command left no row: the opt-out is still the newest"
    );
}

/// AC4: the direct-channel admission precedes the mirror — with direct
/// chats off, a deletion reply on a direct channel is disregarded before
/// anything is written, the fail-closed shape of decision 0069.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_direct_channel_admission_precedes_the_mirror() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let (provider, script) = support::scripted_provider(None);
    let fixture = support::start_assistant_config(
        store,
        provider,
        script,
        support::production_toolset(),
        assistant_core::AssemblyConfig {
            reasoning: assistant_core::ReasoningLevel::Low,
            binding: support::binding(),
            system_prompt: support::SYSTEM_PROMPT.into(),
            answering: support::FIXTURE_ANSWERING,
            name: support::NAME.into(),
            disclosure: None,
            protection: ProtectionConfig::default(),
            operators: support::operator_config(),
            direct_chats: DirectChats::Off,
            privacy_policy_address: None,
            moderation_handle: None,
        },
    )
    .await;

    let mut command = del_reply(
        &support::channel("dm-admin"),
        "root-ext",
        Authority::Admin,
        "anything",
    );
    command.channel_kind = ChannelKind::Direct;
    assert_eq!(
        fixture
            .assistant
            .ingest(command)
            .await
            .expect("the direct command ingests"),
        IngestOutcome::Disregarded,
        "the direct-channel admission refuses before the mirror runs"
    );
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "nothing was written"
    );
}
