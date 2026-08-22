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

    assistant
        .ingest(support::inbound(
            &support::channel("dm-a"),
            ChannelKind::Direct,
            "A",
            "a direct question from A",
        ))
        .await
        .expect("the direct message from A ingests");
    let receipt_b = assistant
        .ingest(support::inbound(
            &support::channel("dm-b"),
            ChannelKind::Direct,
            "B",
            "a direct question from B",
        ))
        .await
        .expect("the direct message from B ingests");
    // B also spoke in a group, so blocks carrying B's principal id survive
    // the erasure — the retention that makes id reuse dangerous at all.
    let receipt_group = assistant
        .ingest(support::inbound(
            &support::channel("room-reissue"),
            ChannelKind::Group,
            "B",
            "a group question from B",
        ))
        .await
        .expect("the group message from B ingests");
    // Every turn settles before the erasure, so no stream is open while the
    // direct conversation under it goes away.
    support::settle(store, receipt_b.conversation_id, "B's direct turn", 3).await;
    support::settle(store, receipt_group.conversation_id, "B's group turn", 3).await;

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
    let receipt_c = assistant
        .ingest(support::inbound(
            &support::channel("dm-c"),
            ChannelKind::Direct,
            "C",
            "a direct question from C",
        ))
        .await
        .expect("the direct message from C ingests");
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
    let group = support::channel("room-1");

    let receipt_a = assistant
        .ingest(support::inbound(
            &support::channel("dm-a"),
            ChannelKind::Direct,
            "A",
            "first direct question",
        ))
        .await
        .expect("the direct message from A ingests");
    support::settle(
        store,
        receipt_a.conversation_id,
        "A's answered direct turn",
        3,
    )
    .await;

    let receipt_b = assistant
        .ingest(support::inbound(
            &support::channel("dm-b"),
            ChannelKind::Direct,
            "B",
            "second direct question",
        ))
        .await
        .expect("the direct message from B ingests");
    let conv_group = assistant
        .ingest(support::inbound(
            &group,
            ChannelKind::Group,
            "A",
            "a group question from A",
        ))
        .await
        .expect("the group message from A ingests")
        .conversation_id;
    support::settle(
        store,
        receipt_b.conversation_id,
        "B's answered direct turn",
        3,
    )
    .await;
    support::settle(store, conv_group, "the first group turn", 3).await;
    assistant
        .ingest(support::inbound(
            &group,
            ChannelKind::Group,
            "B",
            "a group question from B",
        ))
        .await
        .expect("the group message from B ingests");
    support::settle(store, conv_group, "the second group turn", 5).await;

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
    let b_blocks = store.list_blocks(conv_b).await.expect("B's ledger reads");
    assert_eq!(
        b_blocks.len(),
        3,
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
