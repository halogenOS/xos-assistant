//! Erasure (AC3, amended with the decision-0012 reconciliation): one call
//! nulls the principal's message text everywhere, removes the principal's
//! direct conversations with their mappings, and deletes the identity rows —
//! while every other block, mapping and principal keeps its place.

use std::collections::HashMap;

use agent_ledger::store::domain_run;
use agent_ledger::{FromBlock, Projection};
use assistant_core::kind::{AssistantKind, ERASED_MARKER};
use assistant_core::{ChannelKind, ErasureOutcome, schema};

use crate::support;

/// The stored mapping rows, keyed by channel identifier — read raw, because
/// the assertion is about what the tables hold, not what an API reports.
async fn stored_channels(store: &agent_ledger::Store) -> HashMap<String, i64> {
    domain_run(&store.tx(), schema::DOMAIN, |conn| {
        let mut statement = conn.prepare("SELECT channel, conversation_id FROM channels")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<HashMap<String, i64>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the mapping table reads")
}

/// The stored identity rows' external ids.
async fn stored_principals(store: &agent_ledger::Store) -> Vec<String> {
    domain_run(&store.tx(), schema::DOMAIN, |conn| {
        let mut statement = conn.prepare("SELECT external_id FROM principals ORDER BY id")?;
        let rows = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(rows)
    })
    .await
    .expect("the identity table reads")
}

/// Erasure frees an identity row, never its id: group blocks keep the
/// principal id forever, so a reissued id would resolve the erased person's
/// retained messages to whoever arrives next. The newest principal is the
/// one case a bare rowid key would reuse, so that is the one this test
/// erases.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_erased_principal_id_is_never_reissued() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let store = &fixture.store;
    support::ingest_recorded(
        assistant,
        support::inbound(
            &support::channel("dm-a"),
            ChannelKind::Direct,
            "A",
            "a direct question from A",
        ),
    )
    .await;
    let receipt_b = support::ingest_recorded(
        assistant,
        support::inbound(
            &support::channel("dm-b"),
            ChannelKind::Direct,
            "B",
            "a direct question from B",
        ),
    )
    .await;
    // B also spoke in a group, so blocks carrying B's principal id survive
    // the erasure — the retention that makes id reuse dangerous at all.
    let reissue_room = support::authorized_group(assistant, "room-reissue").await;
    let receipt_group = support::ingest_recorded(
        assistant,
        support::inbound(
            &reissue_room,
            ChannelKind::Group,
            "B",
            "a group question from B",
        ),
    )
    .await;
    // Every turn settles before the erasure, so no stream is open while the
    // direct conversation under it goes away.
    support::settle(store, receipt_b.conversation_id, "B's direct turn", 4).await;
    support::settle(store, receipt_group.conversation_id, "B's group turn", 4).await;

    assert_eq!(
        assistant
            .erase_principal(receipt_b.principal_id)
            .await
            .expect("erasure succeeds in one call"),
        ErasureOutcome::Erased {
            deleted_conversations: vec![receipt_b.conversation_id],
        }
    );

    // A new sender arrives after the erasure and must get a fresh id.
    let receipt_c = support::ingest_recorded(
        assistant,
        support::inbound(
            &support::channel("dm-c"),
            ChannelKind::Direct,
            "C",
            "a direct question from C",
        ),
    )
    .await;
    assert_ne!(
        receipt_c.principal_id, receipt_b.principal_id,
        "the erased id is reissued: the erased sender's retained blocks would \
         resolve to the new sender's identity row"
    );
    assert_eq!(stored_principals(store).await, vec!["A", "C"]);
}

/// The staged state the AC3 test erases into: two principals, a direct
/// conversation each, and the group conversation both spoke in.
struct Staged {
    principal_a: i64,
    principal_b: i64,
    conv_a: i64,
    conv_b: i64,
    conv_group: i64,
}

/// Stage AC3's shape: a direct conversation for each of two senders, and one
/// group conversation both spoke in. Each ingestion settles into an answered
/// turn, so the block shape under erasure is exact.
async fn stage_two_principals(fixture: &support::Fixture) -> Staged {
    let assistant = &fixture.assistant;
    let store = &fixture.store;
    let group = support::authorized_group(assistant, "room-1").await;

    let receipt_a = support::ingest_recorded(
        assistant,
        support::inbound(
            &support::channel("dm-a"),
            ChannelKind::Direct,
            "A",
            "first direct question",
        ),
    )
    .await;
    support::settle(
        store,
        receipt_a.conversation_id,
        "A's answered direct turn",
        4,
    )
    .await;

    let receipt_b = support::ingest_recorded(
        assistant,
        support::inbound(
            &support::channel("dm-b"),
            ChannelKind::Direct,
            "B",
            "second direct question",
        ),
    )
    .await;
    let conv_group = support::ingest_recorded(
        assistant,
        support::inbound(&group, ChannelKind::Group, "A", "a group question from A"),
    )
    .await
    .conversation_id;
    support::settle(
        store,
        receipt_b.conversation_id,
        "B's answered direct turn",
        4,
    )
    .await;
    support::settle(store, conv_group, "the first group turn", 4).await;
    support::ingest_recorded(
        assistant,
        support::inbound(&group, ChannelKind::Group, "B", "a group question from B"),
    )
    .await;
    support::settle(store, conv_group, "the second group turn", 6).await;

    Staged {
        principal_a: receipt_a.principal_id,
        principal_b: receipt_b.principal_id,
        conv_a: receipt_a.conversation_id,
        conv_b: receipt_b.conversation_id,
        conv_group,
    }
}

/// AC3, block by block. Two principals share a group conversation and hold a
/// direct conversation each; erasing one must reach exactly that principal's
/// data — prose included — and nothing else's.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn erasing_one_principal_reaches_its_prose_and_nothing_else() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let store = &fixture.store;
    let Staged {
        principal_a,
        principal_b,
        conv_a,
        conv_b,
        conv_group,
    } = stage_two_principals(&fixture).await;

    let group_ids_before: Vec<i64> = store
        .list_blocks(conv_group)
        .await
        .expect("the group ledger reads before erasure")
        .iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(stored_principals(store).await, vec!["A", "B"]);

    // The one call under test.
    assert_eq!(
        assistant
            .erase_principal(principal_a)
            .await
            .expect("erasure succeeds in one call"),
        ErasureOutcome::Erased {
            deleted_conversations: vec![conv_a],
        }
    );

    // The identity rows are gone; the other principal's remain.
    assert_eq!(stored_principals(store).await, vec!["B"]);

    // A's direct conversation is removed entirely — the mapping, the
    // conversation row and its blocks — while B's direct conversation and
    // the group conversation keep theirs.
    let channels = stored_channels(store).await;
    assert!(
        !channels.contains_key("dm-a"),
        "A's direct mapping is erased"
    );
    assert_eq!(channels["dm-b"], conv_b);
    assert_eq!(channels["room-1"], conv_group);
    assert!(
        store
            .find_conversation(conv_a)
            .await
            .expect("the conversation table reads")
            .is_none(),
        "A's direct conversation is removed entirely"
    );
    assert!(
        store
            .list_blocks(conv_a)
            .await
            .expect("the removed conversation reads as empty")
            .is_empty(),
        "A's direct blocks are collected with the conversation"
    );
    let b_blocks =
        support::consumer_view(&store.list_blocks(conv_b).await.expect("B's ledger reads"));
    assert_eq!(
        b_blocks.len(),
        4,
        "B's direct conversation keeps its blocks"
    );

    assert_group_prose_reached_exactly(
        store,
        conv_group,
        &group_ids_before,
        principal_a,
        principal_b,
    )
    .await;

    // A principal id matching nothing reports the not-found outcome — the
    // never-existed case and the second call after a completed erasure alike
    // — and the second call changes nothing.
    assert_eq!(
        assistant
            .erase_principal(9_999_999)
            .await
            .expect("the unknown-principal call itself succeeds"),
        ErasureOutcome::NotFound
    );
    assert_eq!(
        assistant
            .erase_principal(principal_a)
            .await
            .expect("the second call itself succeeds"),
        ErasureOutcome::NotFound
    );
    assert_eq!(stored_principals(store).await, vec!["B"]);
    assert_eq!(
        store
            .list_blocks(conv_group)
            .await
            .expect("the group ledger reads after the second call")
            .len(),
        group_ids_before.len()
    );
}

/// The group conversation after the erasure: its block count is unchanged
/// and every block still loads; the erased principal's messages carry no
/// stored text and project nothing to the model, the other principal's
/// message is untouched.
async fn assert_group_prose_reached_exactly(
    store: &agent_ledger::Store,
    conv_group: i64,
    group_ids_before: &[i64],
    erased: i64,
    other: i64,
) {
    let group_blocks = store
        .list_blocks(conv_group)
        .await
        .expect("every group block still loads after erasure");
    let group_ids_after: Vec<i64> = group_blocks.iter().map(|b| b.id).collect();
    assert_eq!(
        group_ids_after, group_ids_before,
        "the group conversation keeps its blocks"
    );
    for block in &group_blocks {
        let AssistantKind::ChatMessage(message) = AssistantKind::from_block(block) else {
            continue;
        };
        if message.principal_id == Some(erased) {
            assert_eq!(
                message.text, None,
                "the erased group message carries no stored text"
            );
            assert_eq!(
                message.origin, None,
                "the erased group message carries no origin reference"
            );
            assert_eq!(
                message.sent_at, None,
                "the erased group message carries no platform send time"
            );
            assert_eq!(
                message.group_role(),
                block.role,
                "an erased message keeps its stored voice in the grouping \
                 pass — the run-continuity shape that closes 0012's \
                 alternation OPEN — while contributing only the marker"
            );
            assert_eq!(
                message.llm_text().as_deref(),
                Some(ERASED_MARKER),
                "an erased message projects the fixed marker, none of its prose"
            );
        } else {
            assert_eq!(message.principal_id, Some(other));
            assert_eq!(
                message.text.as_deref(),
                Some("a group question from B"),
                "the other principal's message is untouched"
            );
            assert!(
                message.origin.is_some() && message.sent_at.is_some(),
                "the other principal's personal columns are untouched"
            );
        }
    }
}

// ─── The editing unit's pins (unit T3, 2026-08-31) ───────────────────────

/// The recorded chat messages of one conversation, in ledger order.
async fn message_rows(
    store: &agent_ledger::Store,
    conversation_id: i64,
) -> Vec<assistant_core::kind::ChatMessage> {
    store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .iter()
        .filter_map(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => Some(message),
            _ => None,
        })
        .collect()
}

/// AC11's author-keyed half: a person's erasure nulls the revision
/// reference on every row they wrote, beside the five columns it already
/// reached. The reference is personal data of its author — it is the
/// identifier of a message that person sent — so leaving it would keep a
/// pointer at what the erasure emptied.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_persons_erasure_nulls_the_revision_reference_too() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-erase-revision").await;

    let first = support::with_origin(
        support::with_username(
            support::inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "casey-ext",
                "the first wording",
            ),
            "casey",
        ),
        "erase-1",
    );
    let receipt = support::ingest_recorded(assistant, first.clone()).await;
    let mut edited = first;
    edited.text = "the second wording".into();
    support::ingest_recorded(assistant, support::revising(edited, "erase-1")).await;

    let before = message_rows(&fixture.store, receipt.conversation_id).await;
    assert_eq!(
        before[1].revises.as_deref(),
        Some("erase-1"),
        "the revision reference stands before the erasure — the delta is provable"
    );

    assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("erasure succeeds in one call");

    let after = message_rows(&fixture.store, receipt.conversation_id).await;
    for row in &after {
        assert_eq!(row.text, None, "the prose is nulled");
        assert_eq!(row.origin, None);
        assert_eq!(
            row.revises, None,
            "the revision reference goes with the rest of the person's row"
        );
    }
}

/// AC4: an edit naming a message the store holds no recorded version of —
/// emptied by erasure here — records nothing and delivers nothing. The
/// erased row stays erased and its text reappears nowhere, in the ledger or
/// in any projection. The platform fires edit updates nobody asked for, so
/// without this rule an erased message could resurrect itself with no human
/// act anywhere in the path.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_of_an_erased_message_records_nothing() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-erased-edit").await;

    let said = support::with_origin(
        support::inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "casey-ext",
            "a line they later had erased",
        ),
        "gone-1",
    );
    let receipt = support::ingest_recorded(assistant, said.clone()).await;
    let conversation = receipt.conversation_id;

    assistant
        .erase_principal(receipt.principal_id)
        .await
        .expect("erasure succeeds in one call");

    // The platform delivers an edit of that same message — a link preview
    // attaching, hours later, is enough to produce one.
    let mut edit = said;
    edit.text = "a line they later had erased, with a link".into();
    let outcome = assistant
        .ingest(support::revising(edit, "gone-1"))
        .await
        .expect("the edit is acknowledged");
    assert!(
        matches!(outcome, assistant_core::IngestOutcome::Disregarded),
        "an edit of a message the store holds no version of records nothing: {outcome:?}"
    );

    let rows = message_rows(&fixture.store, conversation).await;
    assert_eq!(rows.len(), 1, "nothing was appended");
    assert_eq!(rows[0].text, None, "the erased row stays erased");
    let projected = agent_ledger::providers::blocks_to_messages::<AssistantKind>(
        &fixture
            .store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads"),
    );
    let rendered = format!("{projected:?}");
    assert!(
        !rendered.contains("a line they later had erased"),
        "the erased text reaches no projection: {rendered}"
    );
}

/// AC4's other half: an edit naming a message the store NEVER held — no
/// erasure anywhere in the path — records nothing and delivers nothing
/// either. The rule reads the store's answer, not the reason for it: a
/// caption typed onto a photo that arrived without one is the case given
/// up, and it is given up so that the erased case above cannot resurrect
/// itself. Without this pin the drop could be narrowed to erased rows alone
/// and every test would still pass.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edit_of_a_message_never_held_records_nothing() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-unheld-edit").await;

    // One ordinary message, so the conversation exists and the count below
    // measures the edit alone.
    let conversation = support::ingest_recorded(
        assistant,
        support::with_origin(
            support::inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "casey-ext",
                "an ordinary line",
            ),
            "held-1",
        ),
    )
    .await
    .conversation_id;

    let unheld = support::with_origin(
        support::inbound_unaddressed(
            &room,
            ChannelKind::Group,
            "casey-ext",
            "a caption typed onto a photo that arrived without one",
        ),
        "never-held",
    );
    let outcome = assistant
        .ingest(support::revising(unheld, "never-held"))
        .await
        .expect("the edit is acknowledged");
    assert!(
        matches!(outcome, assistant_core::IngestOutcome::Disregarded),
        "an edit naming a message the store never held records nothing: {outcome:?}"
    );

    let rows = message_rows(&fixture.store, conversation).await;
    assert_eq!(
        rows.len(),
        1,
        "nothing was appended beside the ordinary line"
    );
    assert_eq!(
        rows[0].text.as_deref(),
        Some("an ordinary line"),
        "the message the store does hold is untouched"
    );
}

/// AC4's exemption: a privacy self-service command invoked through an edit
/// records and is answered even when the store holds no version of the
/// message being revised — a rights command is answered whatever the store
/// holds, exactly as it is exempt from the suppression re-read one line
/// above it. Its row carries the revision reference even though nothing
/// joins to it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_edited_privacy_command_is_answered_though_nothing_joins_to_it() {
    let fixture = support::start_assistant(None).await;
    let assistant = &fixture.assistant;
    let room = support::authorized_group(assistant, "room-edited-rights").await;

    let asked = support::with_command(
        support::with_origin(
            support::inbound_unaddressed(
                &room,
                ChannelKind::Group,
                "casey-ext",
                assistant_core::privacy::OPT_OUT_COMMAND,
            ),
            "never-held",
        ),
        assistant_core::privacy::OPT_OUT_COMMAND,
    );

    match assistant
        .ingest(support::revising(asked, "never-held"))
        .await
        .expect("the edited command ingests")
    {
        assistant_core::IngestOutcome::Recorded {
            receipt, deliver, ..
        } => {
            assert!(
                deliver.is_some(),
                "the rights command is answered through the edit"
            );
            let rows = message_rows(&fixture.store, receipt.conversation_id).await;
            let command = rows.last().expect("the command row is recorded");
            assert_eq!(
                command.revises.as_deref(),
                Some("never-held"),
                "the row carries its revision fact even though nothing joins to it"
            );
        }
        refused => panic!("the rights command was refused: {refused:?}"),
    }
}
