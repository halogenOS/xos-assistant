//! The direct-chat switch (decision 0069): off refuses a direct-channel
//! inbound before anything is written — no mapping, no principal row, no
//! ledger block, no answer, no deterministic reply — while group channels
//! are served exactly as ever; on, the default, changes nothing.

use std::collections::HashMap;

use agent_ledger::Store;
use agent_ledger::store::domain_run;
use assistant_core::schema::store_config;
use assistant_core::{ChannelKind, DirectChats, IngestOutcome, ProtectionConfig, privacy, schema};

use crate::support;
use crate::support::{channel, first_answer_to, inbound, recv_reply, with_command};

/// The stored mapping rows, keyed by channel identifier — read raw, because
/// the assertion is about what the tables hold, not what an API reports.
async fn stored_channels(store: &Store) -> HashMap<String, i64> {
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
async fn stored_principals(store: &Store) -> Vec<String> {
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

/// Assemble a running assistant whose configuration serves no direct chats,
/// over the scripted provider and the suite's default everything else.
async fn start_assistant_direct_off() -> support::Fixture {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let (provider, script) = support::scripted_provider(None);
    support::start_assistant_config(
        store,
        provider,
        script,
        support::production_toolset(),
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
            direct_chats: DirectChats::Off,
            privacy_policy_address: None,
            moderation_handle: None,
            web_search: None,
        },
    )
    .await
}

/// Off, a direct message — addressed, unaddressed, or the privacy command —
/// is disregarded before anything is written: no conversation, no mapping
/// row, no principal row, no model turn, no deterministic reply. The group
/// path is untouched: an authorized group's addressed message is recorded
/// and answered on the same assembly, and its answer is the FIRST item the
/// outbound edge ever yields — the ordered proof that the disregarded
/// direct messages produced nothing ahead of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_direct_message_under_off_touches_nothing_and_groups_are_served() {
    let fixture = start_assistant_direct_off().await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");

    let dm = channel("dm-off");
    for message in [
        inbound(&dm, ChannelKind::Direct, "51", "anyone there?"),
        support::inbound_unaddressed(&dm, ChannelKind::Direct, "51", "just noting things"),
        with_command(
            inbound(&dm, ChannelKind::Direct, "51", "/privacy"),
            privacy::PRIVACY_COMMAND,
        ),
    ] {
        let outcome = fixture
            .assistant
            .ingest(message)
            .await
            .expect("the entry point judges the message");
        assert_eq!(
            outcome,
            IngestOutcome::Disregarded,
            "a direct message under off is disregarded, the command included \
             — no deterministic reply rides the outcome"
        );
    }
    assert!(
        fixture
            .store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .is_empty(),
        "no conversation was created for the disregarded messages"
    );
    assert!(
        stored_channels(&fixture.store).await.is_empty(),
        "no channel mapping was claimed"
    );
    assert!(
        stored_principals(&fixture.store).await.is_empty(),
        "no principal row was resolved or created"
    );

    // The group path on the very same assembly: admitted, recorded,
    // answered — the switch names direct channels and nothing else.
    let group = support::authorized_group(&fixture.assistant, "group-off").await;
    let asked = "does the switch reach groups?";
    support::ingest_recorded(
        &fixture.assistant,
        inbound(&group, ChannelKind::Group, "52", asked),
    )
    .await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(
        reply.channel, group,
        "the answer binds to the group channel"
    );
    assert_eq!(
        reply.text,
        first_answer_to(asked),
        "the group's answer is the edge's first item — the direct messages \
         put nothing ahead of it"
    );
    assert_eq!(
        fixture
            .script
            .turns
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "exactly the group's turn reached the model"
    );
}

/// The default is on — spelled by the type itself — and an assembly under
/// it serves a direct chat exactly as the rest of this suite proves
/// everywhere: recorded and answered.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_default_serves_direct_chats_unchanged() {
    assert_eq!(
        DirectChats::default(),
        DirectChats::On,
        "the absent knob means on, so the generic assembly is unchanged"
    );
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let dm = channel("dm-default");
    let receipt = support::ingest_recorded(
        &fixture.assistant,
        inbound(&dm, ChannelKind::Direct, "61", "still with me?"),
    )
    .await;
    assert_eq!(
        stored_principals(&fixture.store).await,
        vec!["61".to_owned()],
        "the sender's principal exists under the default"
    );
    assert_eq!(
        stored_channels(&fixture.store).await.get("dm-default"),
        Some(&receipt.conversation_id),
        "the direct channel maps under the default"
    );
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, dm);
    assert_eq!(reply.text, first_answer_to("still with me?"));
}

/// AC6's direct case (unit T3, 2026-08-31): in a direct channel every
/// message is addressed by the channel's nature, so a revision summons and
/// is answered there exactly as every message is — the answer following the
/// version the person now means.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_revision_in_a_direct_chat_summons_like_every_message_there() {
    let fixture = support::start_assistant(None).await;
    let mut replies = fixture
        .assistant
        .outbound(support::ADAPTER)
        .await
        .expect("the outbound edge opens");
    let dm = channel("dm-revision");

    let asked = support::with_origin(
        inbound(&dm, ChannelKind::Direct, "62", "how do I flesh it?"),
        "dm-1",
    );
    let receipt = support::ingest_recorded(&fixture.assistant, asked.clone()).await;
    assert_eq!(
        recv_reply(&mut replies).await.text,
        first_answer_to("how do I flesh it?")
    );
    support::settle(&fixture.store, receipt.conversation_id, "the ask", 4).await;

    let mut corrected = asked;
    corrected.text = "how do I flash it?".into();
    support::ingest_recorded(&fixture.assistant, support::revising(corrected, "dm-1")).await;
    let reply = recv_reply(&mut replies).await;
    assert_eq!(reply.channel, dm);
    assert_eq!(
        reply.text,
        support::answer_to(&format!(
            "{marker} how do I flash it?",
            marker = assistant_core::kind::EDITED_MARKER
        )),
        "the corrected wording is answered, like every message in a direct chat"
    );
}
