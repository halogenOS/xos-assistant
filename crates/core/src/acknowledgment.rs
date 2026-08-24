//! The rules acknowledgment's bounded one-shot generation (unit 20,
//! 2026-08-24): a real rules delta draws a short confirmation in the
//! assistant's own voice, produced by one collected model completion —
//! never a turn. The observation path is deterministic and synchronous,
//! so the call here is bounded three ways: a request timeout, an output
//! cap, and the assembly's configured reasoning level. Whatever the call
//! does — fail, time out, come back empty, or over the cap — the caller
//! receives a deliverable text, because the retired fixed line stands in
//! as the deterministic fallback: a real delta always delivers something.
//!
//! Deliberately NOT the answer machinery: no debt opens, no turn runs, no
//! disclosure fold, no budget row, no co-summoner chain, no empty-answer
//! swallowing. A rules acknowledgment is a service event, not a member
//! answer, and the probe behind the unit's spec proved a member-less turn
//! breaks every one of those mechanisms. What is borrowed instead is the
//! provider binding the answer machinery already uses — the registered
//! module, bound once for this completion and torn down when the collected
//! result is in.

use std::fmt;
use std::time::Duration;

use agent_ledger::providers::{
    FinalContentBlock, Message, MessageContent, MessageRole, ModelSelector, ProviderRequest,
    ProviderResponse, ProviderRx, ReasoningLevel, StreamEvent,
};
use agent_ledger::{ProviderRegistry, StoreError};

use crate::assembly::ModelBinding;
use crate::outbound::RULES_ACKNOWLEDGMENT;

/// How long the whole generation may take, connection to collected result.
/// The observation path awaits this call inline before it returns the
/// delivery, and an adapter drives observations serially — so this bound is
/// also the longest a single rules pin (or a hung provider) can stall that
/// adapter's update batch. It matches the ambient model- and network-wait
/// bound the rest of the workspace uses, and it is ample for the one or two
/// short sentences the instruction asks for; past it the fixed fallback goes
/// out and only the in-voice wording is lost.
pub(crate) const GENERATION_TIMEOUT: Duration = Duration::from_secs(10);

/// The most collected CHARACTERS a usable acknowledgment may hold — counted
/// in code points, not bytes, so a non-Latin acknowledgment (Cyrillic, CJK)
/// is measured by its length, not penalised for its encoding. The
/// instruction asks for one or two short sentences; a result past this cap
/// is not the acknowledgment that was asked for, and collection stops the
/// moment the cap is crossed instead of buffering an unbounded stream.
pub(crate) const OUTPUT_CAP: usize = 600;

/// The acknowledgment a real rules delta delivers: the bounded one-shot
/// completion's usable text, or the deterministic [`RULES_ACKNOWLEDGMENT`]
/// fallback. Always answers a deliverable line — the delivery guarantee
/// the fixed wording carried is preserved whole, and the fallback path is
/// recorded in the log with its cause.
pub(crate) async fn rules_acknowledgment(
    providers: &ProviderRegistry,
    binding: &ModelBinding,
    reasoning: ReasoningLevel,
    name: &str,
    conversation_id: i64,
    rules_text: &str,
) -> String {
    match generated(
        providers,
        binding,
        reasoning,
        name,
        conversation_id,
        rules_text,
    )
    .await
    {
        Ok(acknowledgment) => acknowledgment,
        Err(failure) => {
            tracing::warn!(
                conversation_id,
                %failure,
                "the acknowledgment generation yielded nothing usable; \
                 the fixed line delivers instead"
            );
            RULES_ACKNOWLEDGMENT.to_owned()
        }
    }
}

/// One bounded completion against the registered provider module: bind,
/// send the single request, collect the streamed text whole, and judge it
/// usable. The request sender is held until the collection ends — the
/// binding's contract makes dropping it the teardown — so a timeout or an
/// over-cap return tears the in-flight stream down by construction.
async fn generated(
    providers: &ProviderRegistry,
    binding: &ModelBinding,
    reasoning: ReasoningLevel,
    name: &str,
    conversation_id: i64,
    rules_text: &str,
) -> Result<String, GenerationFailure> {
    let module = providers
        .get(&binding.vendor)
        .ok_or(GenerationFailure::MissingProvider)?;
    let config = module
        .get_config(binding.provider_instance.clone())
        .await
        .map_err(GenerationFailure::Configuration)?
        .ok_or(GenerationFailure::MissingConfiguration)?;
    let (requests, mut responses) =
        module.bind(conversation_id, binding.provider_instance.clone(), config);
    requests
        .send(ProviderRequest::Stream {
            messages: request_messages(name, rules_text),
            model: ModelSelector::Specific(binding.model.clone()),
            tools: Vec::new(),
            reasoning: Some(reasoning),
        })
        .map_err(|_| GenerationFailure::RequestRefused)?;
    let collected = tokio::time::timeout(GENERATION_TIMEOUT, collect(&mut responses))
        .await
        .map_err(|_elapsed| GenerationFailure::TimedOut)??;
    // Past the collection the binding is done with; the sender drops with
    // this scope and the module tears the binding down.
    drop(requests);
    let text = collected.trim();
    if text.is_empty() {
        return Err(GenerationFailure::Empty);
    }
    Ok(text.to_owned())
}

/// The one-shot request: the instruction as the system message, the new
/// rules text verbatim as the user message. The rules travel unwrapped on
/// purpose — the request provably carries exactly the text the note
/// stored, and the instruction names what the message is.
fn request_messages(name: &str, rules_text: &str) -> Vec<Message> {
    vec![
        Message {
            role: MessageRole::System,
            content: MessageContent::Text(format!(
                "You are {name}, a community group's assistant. The message \
                 that follows is the group's newly pinned rules, replacing \
                 the previous ones. Confirm in your own voice that you have \
                 read the new rules and will follow them: one or two short \
                 sentences, plain text, no list, and do not quote the rules \
                 back. Answer with the confirmation alone."
            )),
        },
        Message {
            role: MessageRole::User,
            content: MessageContent::Text(rules_text.to_owned()),
        },
    ]
}

/// Collect one stream into its whole text, mirroring the framework's own
/// collected reading: deltas accumulate, a final restates the whole turn
/// and replaces the partial, a restart discards what the dropped attempt
/// streamed, an error is the turn's verdict, and the close returns what
/// stands. The cap is enforced as the text grows, so a runaway stream is
/// abandoned at the cap instead of buffered to its end.
async fn collect(responses: &mut ProviderRx) -> Result<String, GenerationFailure> {
    let mut text = String::new();
    while let Some(response) = responses.recv().await {
        match response {
            ProviderResponse::Event(StreamEvent::TextDelta { text: fragment }) => {
                text.push_str(&fragment);
            }
            ProviderResponse::Event(StreamEvent::TextFinal { text: whole }) => {
                text = whole;
            }
            ProviderResponse::Event(StreamEvent::ContentFinal { blocks }) => {
                let restated: String = blocks
                    .iter()
                    .filter_map(|block| match block {
                        FinalContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                if !restated.is_empty() {
                    text = restated;
                }
            }
            ProviderResponse::Restart => text.clear(),
            ProviderResponse::Error(error) => return Err(GenerationFailure::Failed(error)),
            ProviderResponse::Done => return Ok(text),
            ProviderResponse::Event(_) => {}
        }
        if text.chars().count() > OUTPUT_CAP {
            return Err(GenerationFailure::OverCap {
                collected: text.chars().count(),
            });
        }
    }
    Err(GenerationFailure::ClosedWithoutDone)
}

/// Why a generation produced no usable acknowledgment — the log line's
/// vocabulary, every variant answered by the same fixed fallback.
#[derive(Debug)]
enum GenerationFailure {
    /// No registered module answers to the binding's vendor.
    MissingProvider,
    /// The module holds no configuration for the binding's instance.
    MissingConfiguration,
    /// Reading the module's configuration failed.
    Configuration(StoreError),
    /// The bound module's request channel refused the send.
    RequestRefused,
    /// The provider failed the completion, with its rendered error.
    Failed(String),
    /// The collected text crossed the output cap.
    OverCap { collected: usize },
    /// The stream closed without its terminal done.
    ClosedWithoutDone,
    /// The generation ran past its timeout.
    TimedOut,
    /// The completion finished with nothing but whitespace.
    Empty,
}

impl fmt::Display for GenerationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProvider => write!(f, "no provider module answers the binding's vendor"),
            Self::MissingConfiguration => write!(f, "the provider holds no configuration"),
            Self::Configuration(error) => write!(f, "the configuration read failed: {error}"),
            Self::RequestRefused => write!(f, "the bound provider refused the request"),
            Self::Failed(error) => write!(f, "the provider failed the completion: {error}"),
            Self::OverCap { collected } => write!(
                f,
                "the collected text crossed the {OUTPUT_CAP}-byte cap at {collected} bytes"
            ),
            Self::ClosedWithoutDone => write!(f, "the stream closed without its done"),
            Self::TimedOut => write!(
                f,
                "the generation ran past its {}s timeout",
                GENERATION_TIMEOUT.as_secs()
            ),
            Self::Empty => write!(f, "the completion was empty or whitespace"),
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    /// The request carries the new rules text verbatim as the user message
    /// and the instruction as the system message, with the assistant's name
    /// in the instruction — the in-voice half of the unit's contract.
    #[test]
    fn the_request_carries_the_rules_verbatim_under_the_named_instruction() {
        let messages = request_messages("Probe", "1. Be kind.\n2. Stay on topic.");
        assert_eq!(messages.len(), 2, "one instruction, one rules message");
        assert_eq!(messages[0].role, MessageRole::System);
        let MessageContent::Text(instruction) = &messages[0].content else {
            panic!("the instruction is plain text");
        };
        assert!(
            instruction.contains("Probe"),
            "the instruction names the assistant"
        );
        assert_eq!(messages[1].role, MessageRole::User);
        let MessageContent::Text(rules) = &messages[1].content else {
            panic!("the rules are plain text");
        };
        assert_eq!(
            rules, "1. Be kind.\n2. Stay on topic.",
            "the rules text travels verbatim"
        );
    }

    /// The collected reading over one scripted response sequence: deltas
    /// accumulate, a restart discards the dropped attempt's partial, a
    /// final restates the whole, and the done returns what stands.
    #[tokio::test]
    async fn the_collection_accumulates_restarts_clean_and_honors_the_final() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        for response in [
            ProviderResponse::Event(StreamEvent::Connected),
            ProviderResponse::Event(StreamEvent::TextDelta {
                text: "the dropped ".into(),
            }),
            ProviderResponse::Restart,
            ProviderResponse::Event(StreamEvent::TextDelta {
                text: "the partial".into(),
            }),
            ProviderResponse::Event(StreamEvent::TextFinal {
                text: "the whole acknowledgment".into(),
            }),
            ProviderResponse::Done,
        ] {
            tx.send(response).expect("the scripted channel is open");
        }
        assert_eq!(
            collect(&mut rx).await.expect("the collection succeeds"),
            "the whole acknowledgment"
        );
    }

    /// A stream past the output cap is abandoned at the cap: the collection
    /// returns the over-cap failure without reading the stream to its end.
    #[tokio::test]
    async fn a_stream_past_the_cap_is_abandoned_at_the_cap() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
            text: "a".repeat(OUTPUT_CAP + 1),
        }))
        .expect("the scripted channel is open");
        // No done ever arrives: returning proves the cap ended the read.
        let failure = collect(&mut rx)
            .await
            .expect_err("the over-cap stream is refused");
        assert!(
            matches!(failure, GenerationFailure::OverCap { collected } if collected > OUTPUT_CAP),
            "the failure names the cap: {failure}"
        );
    }

    /// A provider error is the completion's verdict, returned as the
    /// failure that sends the caller to the fallback.
    #[tokio::test]
    async fn a_provider_error_is_the_completions_verdict() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ProviderResponse::Error("scripted refusal".into()))
            .expect("the scripted channel is open");
        let failure = collect(&mut rx).await.expect_err("the error surfaces");
        assert!(
            matches!(&failure, GenerationFailure::Failed(error) if error == "scripted refusal"),
            "the failure carries the provider's rendering: {failure}"
        );
    }

    /// A channel that closes without the terminal done is a failure, never
    /// a half-collected acknowledgment delivered as whole.
    #[tokio::test]
    async fn a_close_without_done_is_a_failure() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ProviderResponse::Event(StreamEvent::TextDelta {
            text: "half an ack".into(),
        }))
        .expect("the scripted channel is open");
        drop(tx);
        let failure = collect(&mut rx).await.expect_err("the torn stream fails");
        assert!(
            matches!(failure, GenerationFailure::ClosedWithoutDone),
            "the failure names the missing done: {failure}"
        );
    }
}
