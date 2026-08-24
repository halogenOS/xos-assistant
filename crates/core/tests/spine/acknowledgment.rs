//! The rules-acknowledgment unit (unit 20) at the core's edges: a real
//! rules delta runs one bounded one-shot completion with the new rules in
//! its request and delivers the model's text (AC2); every failure mode —
//! the failed call, the sentinel, the empty result, the over-cap stream,
//! the timeout — delivers the deterministic fallback instead, so a real
//! delta never results in silence (AC3, AC6); the admission is unchanged —
//! an identical re-pin and a title change run no call (AC4); and none of
//! the answer machinery is touched: no turn opens, no message block lands,
//! and no disclosure line rides the acknowledgment (AC5).

use std::sync::atomic::Ordering;

use agent_ledger::providers::{MessageContent, MessageRole, ReasoningLevel};
use agent_ledger::{ProviderResponse, Store, StreamEvent};
use assistant_core::schema::store_config;
use assistant_core::{
    ABSTENTION_SENTINEL, ChannelKey, ChannelKind, DeliveryItem, MISS_SENTINEL, Observation,
    ObserveOutcome, ObservedFact, ProtectionConfig, RULES_ACKNOWLEDGMENT,
};
use serde_json::json;
use tokio::sync::mpsc;

use crate::support::{self, ScriptHandle, authorized_group, scripted_acknowledgment};

/// One rules pin on the given channel — the observation every test here
/// judges.
fn rules_pin(key: &ChannelKey, pinned: &str) -> Observation {
    Observation {
        channel: key.clone(),
        channel_kind: ChannelKind::Group,
        fact: ObservedFact::PinnedAnnouncement(pinned.into()),
    }
}

/// The delivered acknowledgment of one judged observation, or the panic
/// that names what arrived instead.
fn delivered(outcome: ObserveOutcome) -> String {
    let ObserveOutcome::Observed {
        deliver: Some(DeliveryItem::Acknowledgment(text)),
    } = outcome
    else {
        panic!("the rules delta delivers an acknowledgment: {outcome:?}");
    };
    text
}

/// AC2, AC5 and the reasoning half of AC6: the real delta runs exactly one
/// bounded completion whose request carries the new rules text verbatim
/// and the configured reasoning level, the model's output is what the
/// channel receives — with no disclosure line prepended — and the ledger
/// shows the observation path opened no turn and recorded no message: the
/// conversation holds its creation blocks and the note, nothing else.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_real_delta_generates_the_acknowledgment_from_the_new_rules() {
    let fixture = support::start_assistant(None).await;
    let key = authorized_group(&fixture.assistant, "group-ack-generated").await;

    let outcome = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nBe kind to newcomers."))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        delivered(outcome),
        scripted_acknowledgment("Be kind to newcomers."),
        "the model's completion is the delivered acknowledgment"
    );

    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        1,
        "one real delta, one completion"
    );
    {
        let seen = fixture.script.seen.lock().expect("the request log locks");
        let request = seen.last().expect("the completion's request was recorded");
        assert_eq!(
            request.len(),
            2,
            "the one-shot request is the instruction and the rules, nothing else"
        );
        assert_eq!(request[0].role, MessageRole::System);
        let MessageContent::Text(instruction) = &request[0].content else {
            panic!("the instruction is plain text");
        };
        assert!(
            instruction.contains(support::NAME),
            "the instruction speaks in the assistant's name: {instruction}"
        );
        assert_eq!(request[1].role, MessageRole::User);
        let MessageContent::Text(rules) = &request[1].content else {
            panic!("the rules are plain text");
        };
        assert_eq!(
            rules, "Be kind to newcomers.",
            "the request provably carries the new rules text"
        );
    }
    assert_eq!(
        fixture
            .script
            .reasonings
            .lock()
            .expect("the reasoning log locks")
            .last()
            .copied(),
        Some(Some(ReasoningLevel::Low)),
        "the configured reasoning level rides the bounded call"
    );

    // AC5's ledger half: no debt, no turn, no message — the conversation
    // holds exactly its creation blocks and the appended note.
    let conversations = fixture
        .store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    assert_eq!(conversations.len(), 1, "one channel, one conversation");
    let blocks = fixture
        .store
        .list_blocks(conversations[0].id)
        .await
        .expect("the ledger reads");
    let shape: Vec<&str> = blocks.iter().map(|b| b.block_type.as_str()).collect();
    assert_eq!(
        shape,
        vec!["system_prompt", "tool_palette", "context_note"],
        "the acknowledgment left no message and no answer on the ledger"
    );
    assert_eq!(blocks[2].fields["text"], json!("Be kind to newcomers."));
}

/// AC4: the admission is exactly the on-delta comparison — an identical
/// re-pin appends nothing, delivers nothing and runs no completion, and a
/// title change never touches the model either. Exactly one call per real
/// delta.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_identical_re_pin_and_the_title_change_run_no_call() {
    let fixture = support::start_assistant(None).await;
    let key = authorized_group(&fixture.assistant, "group-ack-admission").await;

    let first = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nStay on topic."))
        .await
        .expect("the first pin is judged");
    assert_eq!(delivered(first), scripted_acknowledgment("Stay on topic."));
    assert_eq!(fixture.script.turns.load(Ordering::SeqCst), 1);

    let repeated = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nStay on topic."))
        .await
        .expect("the re-pin is judged");
    assert_eq!(
        repeated,
        ObserveOutcome::Observed { deliver: None },
        "the identical re-pin delivers nothing"
    );

    let titled = fixture
        .assistant
        .observe(Observation {
            channel: key.clone(),
            channel_kind: ChannelKind::Group,
            fact: ObservedFact::Title("The renamed room".into()),
        })
        .await
        .expect("the title change is judged");
    assert_eq!(
        titled,
        ObserveOutcome::Observed { deliver: None },
        "a title change acknowledges nothing"
    );

    assert_eq!(
        fixture.script.turns.load(Ordering::SeqCst),
        1,
        "neither the re-pin nor the title change ran a completion"
    );
}

/// AC3's failed-call arm: the provider fails the completion and the
/// deterministic fallback delivers — while the note itself already stands,
/// so the delta was recorded whatever the model did.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_call_delivers_the_deterministic_fallback() {
    let fixture = support::start_assistant(None).await;
    let key = authorized_group(&fixture.assistant, "group-ack-failure").await;
    fixture.script.fail_next_turns(1);

    let outcome = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nNo spam."))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        delivered(outcome),
        RULES_ACKNOWLEDGMENT,
        "the failed call falls back to the fixed line"
    );

    let conversations = fixture
        .store
        .list_conversations()
        .await
        .expect("the conversation list reads");
    let blocks = fixture
        .store
        .list_blocks(conversations[0].id)
        .await
        .expect("the ledger reads");
    assert!(
        blocks
            .iter()
            .any(|block| block.fields.get("text") == Some(&json!("No spam."))),
        "the note stands although the completion failed"
    );
}

/// AC3's sentinel arm: a completion that is nothing but the abstention or
/// miss sentinel is machinery vocabulary, never a chat line — the fallback
/// delivers for both.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sentinel_result_delivers_the_deterministic_fallback() {
    let fixture = support::start_assistant(None).await;
    let key = authorized_group(&fixture.assistant, "group-ack-sentinel").await;

    // The scripted provider answers the cues with the raw sentinels — the
    // stand-in for a model that misread the instruction as a turn.
    for cue in [support::ABSTAIN_CUE, support::MISS_CUE] {
        let outcome = fixture
            .assistant
            .observe(rules_pin(&key, &format!("Rules:\n{cue}")))
            .await
            .expect("the cued pin is judged");
        let acknowledgment = delivered(outcome);
        assert_eq!(
            acknowledgment, RULES_ACKNOWLEDGMENT,
            "a sentinel completion falls back to the fixed line"
        );
        assert!(
            !acknowledgment.contains(ABSTENTION_SENTINEL)
                && !acknowledgment.contains(MISS_SENTINEL),
            "no machinery sentinel ever reaches the chat"
        );
    }
}

/// A provider stub whose every completion streams the given fragments and
/// closes cleanly — the seam for the empty and over-cap deliveries.
fn streaming_stub(fragments: Vec<String>) -> Box<dyn agent_ledger::ProviderModule> {
    support::provider_stub("Fixed stream", "streams a fixed text", move || {
        let (request_tx, mut requests) = mpsc::unbounded_channel();
        let (response_tx, responses) = mpsc::unbounded_channel();
        let fragments = fragments.clone();
        tokio::spawn(async move {
            while requests.recv().await.is_some() {
                for fragment in &fragments {
                    let _ = response_tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
                        text: fragment.clone(),
                    }));
                }
                let _ = response_tx.send(ProviderResponse::Done);
            }
        });
        (request_tx, responses)
    })
}

/// One assembled assistant over the given provider stub, on the suite's
/// defaults — the seam the bound-pinning tests share.
async fn assistant_over(
    provider: Box<dyn agent_ledger::ProviderModule>,
) -> (support::Fixture, ChannelKey) {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        provider,
        ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let key = authorized_group(&fixture.assistant, "group-ack-bounds").await;
    (fixture, key)
}

/// AC3's empty arm: a completion of nothing but whitespace is unusable and
/// the fallback delivers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_whitespace_result_delivers_the_deterministic_fallback() {
    let (fixture, key) = assistant_over(streaming_stub(vec!["  \n\t ".into()])).await;
    let outcome = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nBe patient."))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        delivered(outcome),
        RULES_ACKNOWLEDGMENT,
        "a whitespace completion falls back to the fixed line"
    );
}

/// AC6's output-cap half: a runaway stream is abandoned at the cap and the
/// fallback delivers — the acknowledgment is bounded by construction, not
/// by the model's cooperation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_over_cap_stream_delivers_the_deterministic_fallback() {
    let (fixture, key) =
        assistant_over(streaming_stub(vec!["An unending word. ".repeat(64)])).await;
    let outcome = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nKeep it short."))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        delivered(outcome),
        RULES_ACKNOWLEDGMENT,
        "an over-cap completion falls back to the fixed line"
    );
}

/// AC6's timeout half, under paused time: a provider that never answers
/// holds nothing hostage — the bounded call times out and the fallback
/// delivers, so the observation path always returns.
#[tokio::test(start_paused = true)]
async fn a_timed_out_call_delivers_the_deterministic_fallback() {
    let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let fixture = support::start_assistant_full(
        store,
        support::silent_provider(),
        ScriptHandle::fresh(),
        support::production_toolset(),
        ProtectionConfig::default(),
    )
    .await;
    let key = authorized_group(&fixture.assistant, "group-ack-timeout").await;

    let outcome = fixture
        .assistant
        .observe(rules_pin(&key, "Rules:\nAnswer promptly."))
        .await
        .expect("the rules pin is judged");
    assert_eq!(
        delivered(outcome),
        RULES_ACKNOWLEDGMENT,
        "the timed-out call falls back to the fixed line"
    );
}
