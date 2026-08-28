//! The first-interaction disclosure (unit 12, AC2 and AC3): the first
//! answer to each person opens with the fixed line, stored into the answer
//! block itself; later answers to the same person carry no line; the
//! deterministic replies never do.

use agent_ledger::Role;
use assistant_core::kind::CHAT_MESSAGE_KIND;
use assistant_core::{
    ChannelKind, DeliveryItem, ErasureOutcome, FAILURE_NOTICE, IngestOutcome, Observation,
    ObserveOutcome, ObservedFact, PRIVACY_UNPUBLISHED, composed_disclosure_line,
};
use serde_json::json;

use crate::support::{
    self, answer_to, await_ledger, channel, first_answer_to, inbound, recv_reply, with_command,
};

/// The copy the fixtures deliver, pinned beside the tests that deliver it
/// (amended with unit 14): the line composed from the fixture's name — the
/// unset-key default — one line, no legalese, shaped after the operator's
/// original copy with the name as its slot.
#[test]
fn the_disclosure_copy_composes_from_the_name() {
    assert_eq!(
        support::fixture_disclosure().line(),
        format!(
            "Hi, I'm {}, an AI system, made to assist members of the community.",
            support::NAME
        )
    );
    assert_eq!(
        composed_disclosure_line(support::NAME),
        support::fixture_disclosure().line(),
        "the assembly's unset-key default is the composed line"
    );
    assert_eq!(
        support::disclosed("the answer"),
        format!("{}\n\nthe answer", support::fixture_disclosure().line()),
        "the delivered shape is the line, a blank line, then the answer"
    );
}

/// AC2, block by block: the first answer to a new person carries the line
/// then the answer — stored into the answer block, so the ledger holds the
/// delivered text — and the same person's second answer carries no line, in
/// the store and on the edge alike.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_first_answer_carries_the_line_and_the_second_does_not() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let key = channel("dm-disclosure");

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the first question"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, key);
    assert_eq!(reply.text, first_answer_to("the first question"));
    assert!(
        reply
            .text
            .starts_with(&format!("{}\n\n", support::fixture_disclosure().line())),
        "the line comes first, separated by a blank line"
    );

    // The ledger's answer block carries exactly what the channel saw: the
    // delivery happens after the stored prepend, so this read is not racing
    // the rewrite.
    let conv = receipt.conversation_id;
    let blocks = support::consumer_view(
        &fixture
            .store
            .list_blocks(conv)
            .await
            .expect("the ledger reads"),
    );
    assert_eq!(blocks[3].role, Some(Role::Assistant));
    assert_eq!(
        blocks[3].fields["content"],
        json!(first_answer_to("the first question")),
        "the stored answer block opens with the line"
    );

    // The same person's second answer: no line, stored or delivered.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&key, ChannelKind::Direct, "42", "the second question"),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.text, answer_to("the second question"));
    let blocks = support::settle(&fixture.store, conv, "the second turn", 6).await;
    assert_eq!(
        blocks[5].fields["content"],
        json!(answer_to("the second question")),
        "the second stored answer carries no line"
    );
}

/// AC2: a second new person in the same conversation gets the line on their
/// own first answer — the disclosure is person-keyed, not
/// conversation-keyed — and each person's next answer is bare.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_new_person_in_the_same_conversation_gets_their_own_line() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-two-people").await;

    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "A",
            "the first person's question",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the first person's question"),
        "the first person's first answer carries the line"
    );

    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "B",
            "the second person's question",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the second person's question"),
        "the second person is new to the store and gets the line too"
    );

    for (sender, text) in [
        ("A", "the first person again"),
        ("B", "the second person again"),
    ] {
        support::ingest_recorded(
            &fixture.assistant,
            inbound(&room, ChannelKind::Group, sender, text),
        )
        .await;
        assert_eq!(
            recv_reply(&mut replies).await.text,
            answer_to(text),
            "an introduced person's later answer is bare"
        );
    }
}

/// AC2: an absorbed co-summoner who is new gets counted — the answer to an
/// already-introduced person carries the line because the absorbed person
/// co-summoned the turn — and that lined answer is the absorbed person's
/// own introduction, so their next answer is bare.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_absorbed_new_co_summoner_draws_the_line_and_counts_as_introduced() {
    let hold = support::TurnHold::new();
    let fixture = support::start_assistant(Some(hold.clone())).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-absorbed").await;

    // A is introduced by their own first answer.
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "A", "the opening question"),
    )
    .await;
    hold.started().await;
    hold.release();
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the opening question")
    );

    // A's second turn is held open; B's addressed message is absorbed into
    // it, so B co-summons a turn whose summons was A's.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "A", "the second question"),
    )
    .await;
    hold.started().await;
    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "B",
            "the absorbed newcomer's ask",
        ),
    )
    .await;
    await_ledger(
        &fixture.store,
        receipt.conversation_id,
        "the absorbed message behind the open stream",
        |blocks| {
            blocks
                .iter()
                .filter(|b| b.block_type == CHAT_MESSAGE_KIND)
                .count()
                == 3
        },
    )
    .await;
    hold.release();
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the second question"),
        "the answer to an introduced person carries the line for the \
         absorbed newcomer"
    );

    // The lined answer introduced B: their own first summoned answer is
    // bare.
    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "B",
            "the newcomer's own question",
        ),
    )
    .await;
    hold.started().await;
    hold.release();
    assert_eq!(
        recv_reply(&mut replies).await.text,
        answer_to("the newcomer's own question"),
        "the absorbed introduction already reached this person"
    );
}

/// AC2: a person returning after full deletion is a new person to the store
/// and gets the line again — their fresh principal id appears in no stored
/// answer's summoners, even though the erased rows and the old lined
/// answers stay in the group ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_person_returning_after_deletion_gets_the_line_again() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-return").await;

    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            "the question before deletion",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the question before deletion")
    );
    support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            "the second ask before deletion",
        ),
    )
    .await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        answer_to("the second ask before deletion"),
        "the introduced person's answer is bare before the deletion"
    );
    support::settle(
        &fixture.store,
        receipt.conversation_id,
        "the pre-deletion turns",
        6,
    )
    .await;

    assert_eq!(
        fixture
            .assistant
            .erase_principal(receipt.principal_id)
            .await
            .expect("the erasure runs"),
        ErasureOutcome::Erased {
            deleted_conversations: vec![],
        }
    );

    let returned = support::ingest_recorded(
        &fixture.assistant,
        inbound(
            &room,
            ChannelKind::Group,
            "42",
            "the question after returning",
        ),
    )
    .await;
    assert_ne!(
        returned.principal_id, receipt.principal_id,
        "the returning person is a fresh principal to the store"
    );
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("the question after returning"),
        "the duty resets with the erased memory"
    );
}

/// AC3: the replies outside the answer path carry no disclosure — the
/// privacy command's fixed answer and the failure notice are texts a
/// person wrote and arrive exactly as written, and the rules
/// acknowledgment (model-generated since unit 20) rides the observation
/// return, never the answer edge, so no disclosure fold ever touches it —
/// even when its recipient was never answered before.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deterministic_replies_carry_no_disclosure() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .replies(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let room = support::authorized_group(&fixture.assistant, "room-fixed-lines").await;

    // The privacy command, from a person the store has never answered.
    let outcome = fixture
        .assistant
        .ingest(with_command(
            support::inbound_unaddressed(&room, ChannelKind::Group, "fresh-1", "/privacy"),
            "/privacy",
        ))
        .await
        .expect("the command ingests");
    let IngestOutcome::Recorded {
        deliver: Some(DeliveryItem::CommandAnswer(answer)),
        ..
    } = outcome
    else {
        panic!("the privacy command answers deterministically: {outcome:?}");
    };
    assert_eq!(answer, PRIVACY_UNPUBLISHED);
    assert!(
        !answer.contains(support::fixture_disclosure().line()),
        "a fixed command answer is never introduced"
    );

    // The rules acknowledgment: the model's own text, delivered with no
    // disclosure line prepended — it is a service confirmation, not a
    // member answer, so the first-interaction fold never touches it.
    let outcome = fixture
        .assistant
        .observe(Observation {
            channel: room.clone(),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::PinnedAnnouncement("Rules:\nStay civil.".into()),
        })
        .await
        .expect("the observation is judged");
    let ObserveOutcome::Observed {
        deliver: Some(DeliveryItem::Acknowledgment(acknowledgment)),
    } = outcome
    else {
        panic!("a first rules note draws the acknowledgment: {outcome:?}");
    };
    assert_eq!(
        acknowledgment,
        support::scripted_acknowledgment("Stay civil.")
    );
    assert!(
        !acknowledgment.contains(support::fixture_disclosure().line()),
        "the acknowledgment is never introduced"
    );

    // The failure notice, for a person whose first turn failed: no answer
    // exists, no introduction rides the notice.
    fixture.script.fail_next_turns(1);
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&room, ChannelKind::Group, "fresh-2", "the ask that fails"),
    )
    .await;
    let notice = recv_reply(&mut replies).await;
    assert_eq!(notice.text, FAILURE_NOTICE);
    assert!(
        !notice.text.contains(support::fixture_disclosure().line()),
        "the notice is the core's fixed line, never introduced"
    );
}
