//! The inbound reply's quote (unit 31, AC2–AC8 and AC10–AC12): a member's
//! reply lands preceded by a framework quote block referencing the message
//! it replies to, so the model reads the quoted words `> `-prefixed above
//! the reply — and every case that quotes NOTHING lands exactly as it did
//! before the unit.
//!
//! Every projection assertion here runs the production fold —
//! `blocks_to_messages`, the same function the runtime's request assembly
//! calls — or reads the request the scripted provider actually received.
//! Nothing asserts a quote by inspecting the block it was written as: what
//! this unit exists for is what the model reads.

use agent_ledger::providers::{Message, MessageContent, MessageRole, blocks_to_messages};
use agent_ledger::store::domain_run;
use agent_ledger::{
    Agency, Block, ContentDescriptor, FromBlock, InputBlock, LeafKind, Store, StoreConfig,
};
use assistant_core::kind::{AssistantKind, CHAT_MESSAGE_KIND, COLUMN_TEXT};
use assistant_core::schema::{DOMAIN, store_config};
use assistant_core::{
    AnsweringMode, ChannelKey, ChannelKind, ErasureOutcome, JoinedMember, Observation,
    ObserveOutcome, ObservedFact, ProtectionConfig, QuotedExcerpt, ReplyTarget, SenderIdentity,
};

use crate::support::{self, inbound, inbound_unaddressed, recv_reply, with_origin, with_reply};

/// The stored type string of the framework's quote block, named through the
/// framework leaf's own `KINDS` declaration, never a literal here.
fn quote_kind() -> &'static str {
    agent_ledger::agency::Quote::KINDS[0]
}

/// One projected message rendered to its whole text, in either content
/// mode — the exact-match reading these pins need.
fn rendered(message: &Message) -> String {
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

/// One conversation as the model would read it right now: its stored
/// blocks through the production fold.
async fn projected(store: &Store, conversation_id: i64) -> Vec<(MessageRole, String)> {
    let blocks = store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads");
    blocks_to_messages::<AssistantKind>(&blocks)
        .iter()
        .map(|message| (message.role, rendered(message)))
        .collect()
}

/// Everything one conversation's projection puts in front of the model, as
/// one string — what a "renders nothing of it" assertion reads.
async fn projected_whole(store: &Store, conversation_id: i64) -> String {
    projected(store, conversation_id)
        .await
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The quote blocks one conversation holds, oldest first.
async fn quote_blocks(store: &Store, conversation_id: i64) -> Vec<Block> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == quote_kind())
        .collect()
}

/// The last user-voiced message of the newest recorded request — the
/// group the reply and its quote render into.
fn newest_user_turn(script: &support::ScriptHandle) -> String {
    let requests = script.seen.lock().unwrap();
    let request = requests.last().expect("a turn's request was recorded");
    request
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .map(rendered)
        .next_back()
        .expect("the request carries a user message")
}

/// One recorded member message under an exact origin, resting (no turn).
async fn said(
    fixture: &support::Fixture,
    room: &ChannelKey,
    sender: &str,
    origin: &str,
    text: &str,
) -> assistant_core::IngestReceipt {
    support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound_unaddressed(room, ChannelKind::Group, sender, text),
            origin,
        ),
    )
    .await
}

/// One addressed reply to a stored origin, carrying the given excerpt.
fn reply_to(
    room: &ChannelKey,
    sender: &str,
    origin: &str,
    target: &str,
    text: &str,
    excerpt: Option<QuotedExcerpt>,
) -> assistant_core::InboundMessage {
    let mut message = with_origin(
        with_reply(
            inbound(room, ChannelKind::Group, sender, text),
            ReplyTarget::Message {
                origin: target.into(),
            },
        ),
        origin,
    );
    message.quoted = excerpt;
    message
}

/// The same reply, resting: recorded, summoning nothing. What a reply
/// quotes is decided the same way whether or not it draws a turn, and a
/// resting reply is how a pin about the ledger keeps a turn out of it.
fn resting_reply_to(
    room: &ChannelKey,
    sender: &str,
    origin: &str,
    target: &str,
    text: &str,
    excerpt: Option<QuotedExcerpt>,
) -> assistant_core::InboundMessage {
    let mut message = reply_to(room, sender, origin, target, text, excerpt);
    message.addressed = false;
    message
}

/// A hand-selected excerpt, as the adapter reports one.
fn manual(text: &str) -> QuotedExcerpt {
    QuotedExcerpt {
        text: text.into(),
        manual: true,
    }
}

/// AC2: a reply reaches the model as the quoted text above the member's
/// own words — pinned on the request the scripted provider received, not
/// on the stored block. The whole message is the span, so the model reads
/// the same thing a person scrolling the chat does.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_reaches_the_model_quoted_above_its_own_words() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-quote-e2e").await;

    let target = said(
        &fixture,
        &room,
        "A",
        "org-font",
        "The text font tiring my eyes",
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        reply_to(&room, "B", "org-ask", "org-font", "which one?", None),
    )
    .await;
    recv_reply(&mut replies).await;

    assert_eq!(
        newest_user_turn(&fixture.script),
        "[org-font] The text font tiring my eyes\n\n\
         > The text font tiring my eyes\n\n\
         [org-ask] which one?",
        "the quoted message reaches the model `> `-prefixed, ahead of the \
         reply that pointed at it"
    );
    assert_eq!(
        quote_blocks(&fixture.store, target.conversation_id)
            .await
            .len(),
        1,
        "one reply, one quote"
    );
}

/// AC3: a hand-selected excerpt narrows the span to exactly that excerpt —
/// found by searching the stored text, across a multibyte character
/// boundary where a byte offset would name different words — and an
/// excerpt the stored text no longer holds falls back to the whole
/// message rather than to nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_manual_excerpt_narrows_and_a_drifted_one_quotes_the_whole_message() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-quote-narrow").await;

    let target = said(
        &fixture,
        &room,
        "A",
        "org-groesse",
        "die Größe — the text font tiring my eyes",
    )
    .await;

    support::ingest_recorded(
        &fixture.assistant,
        reply_to(
            &room,
            "B",
            "org-narrow",
            "org-groesse",
            "which one?",
            Some(manual("the text font")),
        ),
    )
    .await;
    recv_reply(&mut replies).await;
    assert_eq!(
        newest_user_turn(&fixture.script),
        "[org-groesse] die Größe — the text font tiring my eyes\n\n\
         > the text font\n\n\
         [org-narrow] which one?",
        "the excerpt is located by searching the stored text, so the \
         multibyte characters ahead of it shift the span by characters and \
         not by bytes"
    );

    // The first turn settles before the second reply, so the second turn's
    // request carries its own user group: the quote and the reply.
    support::settle(
        &fixture.store,
        target.conversation_id,
        "the narrowed turn",
        6,
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        reply_to(
            &room,
            "C",
            "org-drift",
            "org-groesse",
            "and this?",
            Some(manual("a sentence the message never held")),
        ),
    )
    .await;
    recv_reply(&mut replies).await;
    assert_eq!(
        newest_user_turn(&fixture.script),
        "> die Größe — the text font tiring my eyes\n\n[org-drift] and this?",
        "an excerpt that drifted away from the stored text quotes the \
         message whole; the reply keeps its context"
    );
}

/// AC4: every reply the ledger holds no message for lands quoteless and
/// exactly as it did before this unit — an origin from before the
/// assistant joined, an origin whose message was skipped as no-text (it
/// never entered the ledger, so the core meets an origin it holds no
/// message for), an origin recorded in ANOTHER conversation, and a join
/// event's origin, which the widened reply-target column can name and no
/// chat message matches. Nothing is invented in place of any of them.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_to_a_message_the_ledger_does_not_hold_lands_quoteless() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-quote-none").await;
    let elsewhere = support::authorized_group(&fixture.assistant, "room-quote-elsewhere").await;

    // A message in another conversation, and a join event in this one:
    // both carry origins this room's replies will name.
    said(&fixture, &elsewhere, "A", "org-elsewhere", "said next door").await;
    let outcome = fixture
        .assistant
        .observe(Observation {
            channel: room.clone(),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::MembersJoined {
                joiners: vec![JoinedMember {
                    identity: SenderIdentity {
                        external_id: "J".into(),
                        username: Some("jo".into()),
                    },
                    name: "Jo".into(),
                }],
                origin: "org-join-event".into(),
                timestamp: chrono::Utc::now(),
            },
        })
        .await
        .expect("the join observation is judged");
    assert_eq!(outcome, ObserveOutcome::Observed { deliver: None });

    let mut conversation = None;
    for (case, target) in [
        ("an origin from before the assistant joined", "org-pre-join"),
        (
            "an origin whose message was skipped as no-text",
            "org-photo",
        ),
        (
            "an origin recorded in another conversation",
            "org-elsewhere",
        ),
        ("a join event's own origin", "org-join-event"),
    ] {
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            resting_reply_to(
                &room,
                "B",
                &format!("org-reply-{target}"),
                target,
                "what about this?",
                Some(manual("said next door")),
            ),
        )
        .await;
        conversation = Some(receipt.conversation_id);
        assert!(
            quote_blocks(&fixture.store, receipt.conversation_id)
                .await
                .is_empty(),
            "{case} quotes nothing: no block references it, and no \
             placeholder is written in the member's voice"
        );
    }

    let conversation = conversation.expect("the room recorded its replies");
    let projection = projected_whole(&fixture.store, conversation).await;
    assert!(
        !projection.contains("> "),
        "no quoted line reaches the model at all: {projection}"
    );
    assert!(
        !projection.contains("said next door"),
        "the other conversation's words never cross into this one: {projection}"
    );
}

/// AC5: erasing the quoted message empties the quote — the projection
/// carries no quoted text and no marker standing in for it — while the
/// quote block itself stays exactly where it was. The reference resolves
/// at read time, so erasure needs no pass over quotes at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasing_the_quoted_message_empties_the_quote_and_keeps_the_block() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-quote-erasure").await;

    let target = said(
        &fixture,
        &room,
        "A",
        "org-cadence",
        "the release cadence is monthly",
    )
    .await;
    support::ingest_recorded(
        &fixture.assistant,
        resting_reply_to(&room, "B", "org-follow", "org-cadence", "since when?", None),
    )
    .await;
    let conversation = target.conversation_id;
    assert!(
        projected_whole(&fixture.store, conversation)
            .await
            .contains("> the release cadence is monthly"),
        "the quote resolves while the message stands"
    );

    fixture
        .assistant
        .erase_principal(target.principal_id)
        .await
        .expect("the erasure runs");

    let projection = projected_whole(&fixture.store, conversation).await;
    assert!(
        !projection.contains("the release cadence is monthly"),
        "the erased words are gone from the quote too: {projection}"
    );
    assert!(
        !projection.contains("> "),
        "an empty quote renders nothing at all — no marker stands in for \
         the erased words: {projection}"
    );
    assert_eq!(
        quote_blocks(&fixture.store, conversation).await.len(),
        1,
        "the quote block keeps its place in the ledger; only what it \
         resolves to is gone"
    );
}

/// AC6: a reply to one of the assistant's own messages lands quoteless —
/// no stored fact says WHICH of her blocks it answers, and nothing is
/// guessed — while her waking is untouched: the reply is addressed and it
/// is answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_reply_to_the_assistant_lands_quoteless_and_still_wakes_her() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-quote-assistant").await;

    let first = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(
                &room,
                ChannelKind::Group,
                "A",
                "where did the setting move?",
            ),
            "org-first",
        ),
    )
    .await;
    recv_reply(&mut replies).await;
    support::settle(
        &fixture.store,
        first.conversation_id,
        "the assistant's answer",
        4,
    )
    .await;

    let mut answer_reply = with_origin(
        inbound(&room, ChannelKind::Group, "A", "and on the tablet?"),
        "org-second",
    );
    answer_reply.reply_target = Some(ReplyTarget::AssistantMessage);
    answer_reply.quoted = Some(manual("the setting moved"));
    support::ingest_recorded(&fixture.assistant, answer_reply).await;
    recv_reply(&mut replies).await;

    assert!(
        quote_blocks(&fixture.store, first.conversation_id)
            .await
            .is_empty(),
        "her own messages carry no stored origin, so no reply to her can \
         name one of her blocks — and none is guessed"
    );
}

/// AC7: the quote path adds no refusal and no silence — under BOTH
/// answering modes, a reply draws exactly the turn it would have drawn
/// before this unit, and the answer goes out.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_quoted_reply_still_takes_its_turn_in_both_answering_modes() {
    for mode in [AnsweringMode::Addressed, AnsweringMode::Helpful] {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let fixture =
            support::start_assistant_answering(store, None, ProtectionConfig::default(), mode)
                .await;
        let mut replies = fixture
            .assistant
            .replies(support::ADAPTER)
            .await
            .expect("the outbound edge opens");
        let room = support::authorized_group(&fixture.assistant, "room-quote-answering").await;

        let opener = said(
            &fixture,
            &room,
            "A",
            "org-mode",
            "the build failed at step two",
        )
        .await;
        // Helpful answering summons a turn for the resting message too, so
        // its answer is awaited and settled first: the reply then opens a
        // turn of its own instead of being absorbed into that one, and the
        // quote renders into the reply's own user group.
        let quoted = if matches!(mode, AnsweringMode::Helpful) {
            recv_reply(&mut replies).await;
            support::settle(
                &fixture.store,
                opener.conversation_id,
                "the resting message's own turn",
                4,
            )
            .await;
            "> the build failed at step two\n\nwhy?".to_owned()
        } else {
            "the build failed at step two\n\n\
             > the build failed at step two\n\n\
             why?"
                .to_owned()
        };
        let receipt = support::ingest_recorded(
            &fixture.assistant,
            reply_to(&room, "B", "org-mode-ask", "org-mode", "why?", None),
        )
        .await;
        let reply = recv_reply(&mut replies).await;
        assert!(
            reply.text.ends_with(&support::answer_to(&quoted)),
            "{mode:?}: the turn happens and the model answered what it \
             read — the quoted context included; got {text}",
            text = reply.text
        );
        assert_eq!(
            quote_blocks(&fixture.store, receipt.conversation_id)
                .await
                .len(),
            1,
            "{mode:?}: one quote, whichever mode summoned the turn"
        );
    }
}

/// AC8: the chat-message descriptor declares its quotable column and the
/// other four declare none — and the framework's own open-time validation
/// is what stands behind the declaration, driven from this workspace: the
/// production configuration opens, and a misdeclared copy of the same
/// descriptor is refused.
#[test]
fn the_quotable_declaration_is_the_one_the_framework_validates_at_open() {
    let descriptors = AssistantKind::DESCRIPTORS;
    assert_eq!(
        descriptors[0].quoted_text_column,
        Some(COLUMN_TEXT),
        "a quote of a recorded message resolves to what was said"
    );
    for descriptor in &descriptors[1..] {
        assert_eq!(
            descriptor.quoted_text_column, None,
            "{} declares no quotable text: nothing quotes it",
            descriptor.table
        );
    }

    let config = store_config();
    Store::in_memory_with(config).expect("the declared column passes the open-time validation");

    // The same descriptors, with the chat message's declaration pointed at
    // a column that is not text — the framework refuses the store rather
    // than resolving quotes to a serialized number.
    let misdeclared: Vec<ContentDescriptor> = descriptors
        .iter()
        .map(|descriptor| ContentDescriptor {
            quoted_text_column: if descriptor.table == descriptors[0].table {
                Some("principal_id")
            } else {
                descriptor.quoted_text_column
            },
            ..*descriptor
        })
        .collect();
    let refused = Store::in_memory_with(StoreConfig {
        descriptors: Box::leak(misdeclared.into_boxed_slice()),
        domain_migrations: store_config().domain_migrations,
    });
    let error = refused
        .err()
        .expect("a misdeclared quotable column is refused at open");
    assert!(
        error.to_string().contains("quoted_text_column"),
        "the refusal names the declaration: {error}"
    );
}

/// AC10: the quoter's own erasure leaves the quote standing. A framework
/// user block carries no principal and the quote stores a span and a
/// voice, so there is nothing of the quoter in it to erase — the reply's
/// message is nulled as any message is, the quote still resolves the
/// target's words, and the stored row holds no column that could name
/// anybody.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_quoters_erasure_nulls_their_message_and_leaves_the_quote() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-quote-quoter").await;

    let target = said(
        &fixture,
        &room,
        "A",
        "org-quoted",
        "the setting moved to the top",
    )
    .await;
    let quoter = support::ingest_recorded(
        &fixture.assistant,
        resting_reply_to(
            &room,
            "B",
            "org-quoter",
            "org-quoted",
            "since which build?",
            None,
        ),
    )
    .await;
    let conversation = target.conversation_id;

    assert!(matches!(
        fixture
            .assistant
            .erase_principal(quoter.principal_id)
            .await
            .expect("the erasure runs"),
        ErasureOutcome::Erased { .. }
    ));

    let projection = projected_whole(&fixture.store, conversation).await;
    assert!(
        !projection.contains("since which build?"),
        "the quoter's own words are nulled as any message's are: {projection}"
    );
    assert!(
        projection.contains("> the setting moved to the top"),
        "the quote survives, resolving text whose owner asked for nothing: \
         {projection}"
    );
    assert_eq!(
        quote_blocks(&fixture.store, conversation).await.len(),
        1,
        "the quote block is still there"
    );

    // Nothing in the stored quote could name the erased person: the row is
    // a voice and a span, and that is the whole reason no erasure pass has
    // to find it.
    let columns: Vec<String> = domain_run(&fixture.store.tx(), DOMAIN, |conn| {
        let mut statement = conn.prepare("SELECT name FROM pragma_table_info('block_quote')")?;
        let names = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(names)
    })
    .await
    .expect("the quote table reads");
    assert_eq!(
        columns,
        vec![
            "block_id".to_owned(),
            "role".to_owned(),
            "start_block_id".to_owned(),
            "start_pos".to_owned(),
            "end_block_id".to_owned(),
            "end_pos".to_owned(),
        ],
        "a quote stores where it points and in whose voice — no author, no \
         handle, nothing an erasure would have to reach"
    );
}

/// AC11: the crash state keeps its debt. With a quote appended and its
/// message withheld — the exact shape a crash between the two appends
/// leaves — the lone quote at the tail draws no turn of its own, the
/// retried message's landing still opens the debt the tail owed behind
/// the quote, and the tail-skip keeps the retry from landing a second
/// identical quote.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_crash_shape_keeps_its_debt_and_lands_no_second_quote() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&fixture.assistant, "room-quote-crash").await;

    // An addressed question nobody has answered: the debt the tail owes.
    let asked = support::ingest_recorded(
        &fixture.assistant,
        with_origin(
            inbound(&room, ChannelKind::Group, "A", "which build fixed it?"),
            "org-asked",
        ),
    )
    .await;
    let conversation = asked.conversation_id;
    let target = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads")
        .into_iter()
        .find(|block| block.block_type == CHAT_MESSAGE_KIND)
        .expect("the question is recorded");
    let quoted_characters = i64::try_from(support::field(&target, COLUMN_TEXT).chars().count())
        .expect("the question's length fits an offset");

    // The crash shape, built on the public append the ingest itself uses:
    // the quote lands, its message never does.
    fixture
        .store
        .insert_user_blocks(
            conversation,
            vec![InputBlock::Quote {
                start_block_id: target.id,
                start_pos: 0,
                end_block_id: target.id,
                end_pos: quoted_characters,
            }],
        )
        .await
        .expect("the withheld reply's quote lands");

    let tail = fixture
        .store
        .latest_block(conversation)
        .await
        .expect("the tail reads")
        .expect("the conversation holds blocks");
    assert_eq!(tail.block_type, quote_kind(), "the quote is the tail");
    assert_eq!(
        AssistantKind::from_block(&tail).awaiting(),
        None,
        "a lone quote asks for nothing: it draws no turn of its own"
    );

    // The redelivered reply lands its message behind the very quote its
    // first attempt wrote.
    support::ingest_recorded(
        &fixture.assistant,
        resting_reply_to(
            &room,
            "B",
            "org-retried",
            "org-asked",
            "I think it was last week's",
            None,
        ),
    )
    .await;

    let quotes = quote_blocks(&fixture.store, conversation).await;
    assert_eq!(
        quotes.len(),
        1,
        "the retry finds its own quote at the tail and lands none: the \
         crash-retry signature, read once"
    );
    let messages: Vec<Block> = fixture
        .store
        .list_blocks(conversation)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| block.block_type == CHAT_MESSAGE_KIND)
        .collect();
    let retried = messages.last().expect("the retried message is recorded");
    assert_eq!(
        retried.fields["answer_due"],
        serde_json::json!(true),
        "the debt the tail owed reaches the retried message THROUGH the \
         quote: the walk reads past it instead of settling on it"
    );
    assert_eq!(
        retried.fields["addressed"],
        serde_json::json!(false),
        "the retried reply summoned nothing of its own, so the debt it \
         carries can only have come from the message BEHIND the quote"
    );
}

/// AC12: erasure parity across the refresh fork. The startup walk forks a
/// channel onto an edited prompt by SHARING blocks through the junction —
/// nothing is cloned — so the source and the fork read the same recorded
/// message, and one erasure empties the quote in both.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn erasing_the_quoted_member_empties_the_quote_in_the_fork_too() {
    let db = support::TempDb::new("quote-fork-erasure");
    let store = Store::open_with(db.path(), store_config()).expect("the configured store opens");

    let first = support::start_assistant_full(
        store.clone(),
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let room = support::authorized_group(&first.assistant, "room-quote-fork").await;
    let target = said(
        &first,
        &room,
        "A",
        "org-forked",
        "the update rolls out on Fridays",
    )
    .await;
    support::ingest_recorded(
        &first.assistant,
        resting_reply_to(
            &room,
            "B",
            "org-fork-ask",
            "org-forked",
            "every week?",
            None,
        ),
    )
    .await;
    let source = target.conversation_id;
    assert!(
        projected_whole(&store, source)
            .await
            .contains("> the update rolls out on Fridays"),
        "the quote resolves in the conversation it was written in"
    );

    // The restart a prompt edit produces: the walk forks the channel onto
    // the current wording.
    let mut edited = support::assembly_config();
    edited.system_prompt = "a different system prompt entirely".into();
    let restarted = support::start_assistant_config(
        store.clone(),
        support::silent_provider(),
        support::ScriptHandle::fresh(),
        assistant_core::tools::ToolSet::new(),
        edited,
    )
    .await;
    assert_eq!(
        restarted
            .assistant
            .retire_stale_channels()
            .await
            .expect("the retirement reads the ledger"),
        1,
        "the channel serving the old prompt forks"
    );
    let after = support::ingest_recorded(
        &restarted.assistant,
        inbound_unaddressed(&room, ChannelKind::Group, "C", "still here?"),
    )
    .await;
    assert_ne!(
        after.conversation_id, source,
        "the fork is a new conversation"
    );
    assert!(
        projected_whole(&store, after.conversation_id)
            .await
            .contains("> the update rolls out on Fridays"),
        "the fork inherited the quote and the message it points at"
    );

    assert!(matches!(
        restarted
            .assistant
            .erase_principal(target.principal_id)
            .await
            .expect("the erasure runs"),
        ErasureOutcome::Erased { .. }
    ));

    for (name, conversation) in [("the source", source), ("the fork", after.conversation_id)] {
        let projection = projected_whole(&store, conversation).await;
        assert!(
            !projection.contains("the update rolls out on Fridays"),
            "{name} carries none of the erased words: {projection}"
        );
        assert!(
            !projection.contains("> "),
            "{name} renders no quoted line at all: {projection}"
        );
    }
}
