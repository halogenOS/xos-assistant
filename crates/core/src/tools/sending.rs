//! What the two sending tools share (unit 55, 2026-09-02): the parameters,
//! the caps, the reply target's validation, the filing, and every sentence
//! the model reads back.
//!
//! `send_message` and `reply_message` are two names for one act — put words
//! in the group's chat — differing in exactly one place: whether the send
//! threads onto a message the model named. Each is its own module, because
//! each is its own registered tool with its own description and its own
//! answer to the admission hook; everything BEHIND that difference lives
//! here, once, so the two can never drift into two behaviours.
//!
//! # What a call does
//!
//! It appends one [`OutgoingMessage`]
//! block recording the text, the reply target and the call block, and it
//! returns [`ToolOutcome::Pending`]. Nothing is sent from inside the body:
//! the outbound edge classifies the block exactly as it classifies a
//! reaction and hands it to the adapter, and the delivery receipt is what
//! settles the call — with the platform's ids when the whole message went
//! out, with a failure when it did not. So the model learns the real id of
//! what it sent, and a turn holding an unresolved send stays open until the
//! send is settled, by the framework's standing rule for a system-owed
//! call.
//!
//! The body is IDEMPOTENT on the call block. A re-run of one call — a
//! restart-recovered round, a redelivered wakeup — finds its own outgoing
//! block on the ledger and appends no second one, returning pending again,
//! so a recovered round never doubles a message in the chat.
//!
//! # The caps
//!
//! Three tiers per conversation, shared by both tools:
//! [`CAPS`]. They are counted over the conversation's own outgoing blocks
//! in each trailing span — the ones that delivered and the ones still
//! pending, never the ones that failed, because a failed send posted
//! nothing and burns no allowance.
//!
//! The count runs ONCE, inside the ADMISSION answer, over the ledger the
//! runner's admission pass already loaded — which is why it is spelled here
//! and not as a framework window: the bound is shared across two tool names
//! and read per conversation, and the framework's per-tool window holds one
//! allowance per name. There is no second check behind it and no filing
//! lock beside it: the two tools run in order, so that ledger holds every
//! earlier send of the conversation and the count is exact. A spent tier
//! declines with [`Admission::Refuse`](agent_ledger::Admission::Refuse),
//! which the framework records as a refusal — a standing no, a run of which
//! ends the turn — so the model stops and resumes on a later one.
//!
//! # The reply target
//!
//! `reply_to` must name an id the serving conversation's ledger HOLDS: a
//! member message's own id or the id it was revised under, a join notice's
//! event id, or one of the assistant's own delivered ids — of any age. An
//! id the ledger does not hold is refused, never sent plain: a silently
//! dropped thread hides a hallucinated id from the model, and a refusal
//! costs one round and tells it the truth. "Holds" is exactly the loaded
//! ledger, so an id compacted below the cut and one whose message an
//! erasure nulled are both refused with the same sentence — the model never
//! saw either under an envelope it could still name.
//!
//! The refusal is a [`ToolOutcome::Error`], which the framework records as
//! a FAILURE and not a refusal: the model can correct it inside the turn by
//! naming another id or sending plainly, so it must not count toward the
//! run that ends a turn.

use std::sync::Arc;

use agent_ledger::store::StoreError;
use agent_ledger::{Block, CoreEvent, FromBlock, Store, ToolContext, ToolOutcome};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::composing::SendStops;
use crate::kind::AssistantKind;
use crate::message::Authority;
use crate::outgoing::{self, OutgoingMessage};

/// The pair's two registered names, enumerated once. Which tools count as
/// sending is a fact about the pair, and two readers ask it: the typing
/// cue's begin condition, over the name a starting call carries, and the
/// contract notice's condition, over the names a recorded tool choice
/// holds. Each name still belongs to the module that registers it — this
/// is the enumeration, not a second spelling.
pub const NAMES: [&str; 2] = [crate::tools::send::NAME, crate::tools::reply::NAME];

/// Whether a tool name is one of the pair's — the one reading of
/// [`NAMES`], so a third sending tool reaches every reader at once.
#[must_use]
pub fn is_sending_tool(name: &str) -> bool {
    NAMES.contains(&name)
}

/// The words to send — the one parameter both tools take.
pub const PARAMETER_TEXT: &str = "text";

/// The message a threaded send answers, named by the id its envelope
/// showed. The reply tool's own second parameter.
pub const PARAMETER_REPLY_TO: &str = "reply_to";

/// The authority a send requires — member: the turns that answer a group
/// are summoned by ordinary members' messages. The admission check supplies
/// no extra protection at this bar; the tools sit under it because every
/// tool does (stated, not implied).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// One cap tier: how many messages the conversation may send inside one
/// trailing span, and the word the refusal names that span by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tier {
    /// How many sends the span admits.
    pub sends: usize,
    /// How long the span is, in seconds.
    pub seconds: i64,
    /// What the refusal calls the span. A word and not a computed unit:
    /// the three spans are named in the decision that set them, and a
    /// number of seconds read back to the model would be a different
    /// sentence than the one that was decided.
    pub span: &'static str,
}

/// The caps, as the operator set them (2026-09-01): five messages a
/// minute, thirty an hour, a hundred a day — per group chat, shared by both
/// sending tools.
///
/// Ordered narrowest first, so a conversation that trips two tiers at once
/// is told about the one that reopens soonest: the model's next useful
/// question is when it may speak again, and the minute's answer is the one
/// it can act on.
pub const CAPS: [Tier; 3] = [
    Tier {
        sends: 5,
        seconds: 60,
        span: "minute",
    },
    Tier {
        sends: 30,
        seconds: 60 * 60,
        span: "hour",
    },
    Tier {
        sends: 100,
        seconds: 24 * 60 * 60,
        span: "day",
    },
];

/// The empty-text refusal: a send with no words is not a send. The model
/// can correct it inside the turn — by writing something, or by ending the
/// turn silently — so the sentence teaches no never-again.
pub const NEEDS_TEXT_ERROR: &str = "declined: a message needs text. Write what you want the \
     group to read, or end your turn without sending anything.";

/// The missing-target refusal of the reply tool: a threaded send names the
/// message it answers. Correctable inside the turn, so it points at where
/// the id is shown and names the plain send beside it.
pub const NEEDS_TARGET_ERROR: &str = "declined: a reply names the message it answers, by the \
     msgid shown in that message's envelope. Name one, or send the message with \
     send_message instead.";

/// The unknown-target refusal: the named id is not one this conversation
/// holds, so there is nothing to thread onto. Never sent plain instead — a
/// dropped thread would hide an id the model invented — and correctable
/// inside the turn, which is why it is a failure and not a standing no.
pub const UNKNOWN_TARGET_ERROR: &str = "declined: this conversation holds no message with that \
     id, so there is nothing to reply to. Take the id from the msgid line of the message you \
     mean, or send the message with send_message instead.";

/// The transient failure: a read or the append did not stand, so nothing
/// was filed and nothing was sent. No never-again teaching — the fact may
/// not hold beyond this failure, and the idempotence read finds nothing
/// filed, so a later turn sends cleanly.
#[must_use]
pub fn transient_error() -> String {
    "the message could not be sent right now; nothing was sent.".to_owned()
}

/// The spent-tier refusal, naming the tier and when it reopens (unit 55).
///
/// Both facts are the ones the model can act on: which allowance is gone,
/// and the moment it is not. The time is the instant the oldest counted
/// send in that span ages out of it, in UTC — the same clock every stored
/// block header is written on.
#[must_use]
pub fn cap_refusal(tier: Tier, reopens_at: DateTime<Utc>) -> String {
    format!(
        "declined: this conversation has sent its {sends} messages for the last {span}, and \
         this one was not sent. The allowance reopens at {reopens}. Send nothing more this \
         turn; continue on a later one.",
        sends = tier.sends,
        span = tier.span,
        reopens = reopens_at.to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

/// The first spent tier over one loaded ledger, with the moment it
/// reopens — the caps' whole reading, taken in the admission answer.
///
/// What counts is a filed send whose call DELIVERED or is still PENDING: a
/// failed send put nothing in the chat, so it burns no allowance. A block
/// header whose stored time cannot be read counts anyway, in the limiting
/// direction: an unreadable clock must never widen an allowance.
///
/// The reopening moment is the oldest counted send's own time plus the
/// span. That is exactly when the count inside the span drops below the
/// tier again, so the sentence names a moment the model can wait for
/// instead of a guess.
#[must_use]
pub fn spent_tier(ledger: &[Block], now: DateTime<Utc>) -> Option<(Tier, DateTime<Utc>)> {
    let counted: Vec<DateTime<Utc>> = outgoing::filed_sends(ledger)
        .into_iter()
        .filter(|send| outgoing::send_state(ledger, send.call_block) != outgoing::SendState::Failed)
        .map(|send| counted_at(&send.created_at, now))
        .collect();
    for tier in CAPS {
        let span = TimeDelta::seconds(tier.seconds);
        let mut inside: Vec<DateTime<Utc>> = counted
            .iter()
            .copied()
            .filter(|sent| now - *sent < span)
            .collect();
        if inside.len() < tier.sends {
            continue;
        }
        inside.sort_unstable();
        // The oldest send inside the span is the one whose ageing out frees
        // the slot this call wanted.
        let oldest = inside.first().copied().unwrap_or(now);
        return Some((tier, oldest + span));
    }
    None
}

/// When one filed send counts, from its block header's stored stamp.
///
/// The store writes that column itself, in RFC 3339 with milliseconds and
/// an offset, so an unreadable value is a row the store did not produce.
/// It is counted at `now` — the limiting direction, since an unreadable
/// clock must never widen an allowance — and SAID rather than swallowed:
/// a silent fallback here would let a broken stamp quietly shift what the
/// caps admit, with nothing anywhere naming why.
fn counted_at(created_at: &str, now: DateTime<Utc>) -> DateTime<Utc> {
    match DateTime::parse_from_rfc3339(created_at) {
        Ok(stamped) => stamped.with_timezone(&Utc),
        Err(error) => {
            tracing::warn!(
                created_at,
                %error,
                "a filed send carries no readable stamp; it counts against the caps as if \
                 it were sent now"
            );
            now
        }
    }
}

/// The caps as the admission hook asks them: the decline sentence, or
/// nothing. Read against the wall clock, since the spans the operator set
/// are wall-clock spans and the block headers they are measured against
/// carry the store's own wall-clock time.
#[must_use]
pub fn cap_decline(ledger: &[Block]) -> Option<String> {
    spent_tier(ledger, Utc::now()).map(|(tier, reopens_at)| cap_refusal(tier, reopens_at))
}

/// What one call named: the words to send, and the message it threads onto
/// where the tool takes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSend {
    /// The text, exactly as the model wrote it. Content, so it is stored
    /// verbatim: the core rewrites nothing a person or a model wrote.
    pub text: String,
    /// The message this send threads onto; `None` for a plain send.
    pub reply_to: Option<String>,
}

/// Read one call's input for the plain send. `Err` is the refusal the model
/// reads: every unusable shape — a missing field, a non-string, a text that
/// is nothing but whitespace, input that is not a JSON object — is
/// [`NEEDS_TEXT_ERROR`], because each of them is the same missing thing.
///
/// The text is NOT trimmed: it is content, and the core stores content as
/// it was written. What the emptiness check reads is the trimmed form, so a
/// message of nothing but spaces is refused rather than sent.
///
/// # Errors
///
/// [`NEEDS_TEXT_ERROR`] when the call names no usable text.
pub fn named_send(input: &str) -> Result<NamedSend, &'static str> {
    let value: Value = serde_json::from_str(input).map_err(|_| NEEDS_TEXT_ERROR)?;
    Ok(NamedSend {
        text: named_text(&value)?,
        reply_to: None,
    })
}

/// One read call's text, from the value the caller already parsed.
fn named_text(value: &Value) -> Result<String, &'static str> {
    value
        .get(PARAMETER_TEXT)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or(NEEDS_TEXT_ERROR)
        .map(ToOwned::to_owned)
}

/// Read one call's input for the threaded send: the plain send's text, plus
/// the target it must name. Both fields come out of ONE parse of the input,
/// and the text is read first, so a call naming neither is told about the
/// words it exists to carry.
///
/// # Errors
///
/// [`NEEDS_TEXT_ERROR`] when the call names no usable text,
/// [`NEEDS_TARGET_ERROR`] when it names no usable target.
pub fn named_reply(input: &str) -> Result<NamedSend, &'static str> {
    let value: Value = serde_json::from_str(input).map_err(|_| NEEDS_TEXT_ERROR)?;
    let text = named_text(&value)?;
    let reply_to = value
        .get(PARAMETER_REPLY_TO)
        .and_then(reads_as_id)
        .ok_or(NEEDS_TARGET_ERROR)?;
    Ok(NamedSend {
        text,
        reply_to: Some(reply_to),
    })
}

/// One JSON value as a named message id: a non-empty string with its
/// transcription whitespace trimmed away, or a bare number, which several
/// providers emit for an id the projection showed as digits. Anything else
/// names no message.
fn reads_as_id(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim())
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Whether the serving conversation's ledger HOLDS the named id — the reply
/// target's whole validation, over the vector the tool already loaded.
///
/// Three kinds of record answer to an id, and each is read through its own
/// parse rather than a column scan here:
///
/// - a member's message, by the id it was recorded under or the id it was
///   revised under, since every version of one message shows the model one
///   token to name it by. An erased row holds neither, having lost both
///   with its text, and is therefore not held;
/// - a join notice, by its event id — the model reads join notices under
///   the same envelope and may answer one;
/// - one of the assistant's own delivered messages, by the platform id her
///   delivery receipt recorded.
///
/// Anything else — an id from another conversation, one below a compaction
/// cut, one an erasure nulled, one the model invented — is not held.
#[must_use]
pub fn ledger_holds(ledger: &[Block], id: &str) -> bool {
    ledger
        .iter()
        .any(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => {
                message.origin.as_deref() == Some(id) || message.revises.as_deref() == Some(id)
            }
            AssistantKind::JoinNotice(join) => join.origin.as_deref() == Some(id),
            AssistantKind::Delivered(delivered) => delivered.origin.as_deref() == Some(id),
            _ => false,
        })
}

/// The shared body of both sending tools: the ledger read, the idempotence
/// check, the target validation, the one append — and the composing cue's
/// stop for every call that ends without filing anything.
///
/// One hold and no second one (unit 55, 2026-09-02). The erasure fence is
/// held shared across the validation and the append, which is why a send
/// cannot thread onto an origin the person-wide erasure just nulled. NO
/// filing lock stands beside it: the two tools run IN ORDER — the
/// framework parks a ready call of either while an earlier one of the
/// conversation is unresolved — so a sibling call of the same round cannot
/// be filing while this one scans, and the order is the one mechanism that
/// says so.
pub struct Sender {
    /// The erasure fence, held shared across the validation and the append.
    /// Taken as the bare shared lock, not as the assembly's own alias for
    /// it — a leaf tool names nothing in the module that registers it.
    fence: Arc<RwLock<()>>,
    /// Where the composing cue is told this send is done. A call refused
    /// before anything was filed lit the cue at its start and files no
    /// block, so no delivery report will ever end it: the tool says so
    /// itself, on the same channel the receipt door uses.
    stops: SendStops,
}

impl Sender {
    /// The sender both tools are constructed with, holding the erasure
    /// fence and the cue's stop channel the assembly injects at
    /// registration.
    #[must_use]
    pub fn new(fence: Arc<RwLock<()>>, stops: SendStops) -> Self {
        Self { fence, stops }
    }

    /// The caps as each tool's admission hook asks them, with the cue's
    /// stop where they refuse: a call declined before its body ran filed
    /// nothing, so the indicator its start lit is ended here.
    ///
    /// The count itself is [`cap_decline`]'s, read once over the ledger the
    /// admission pass already loaded. Nothing else is asked and nothing is
    /// re-read: the calls run in order, so that ledger holds every earlier
    /// send of the conversation and the count is exact.
    #[must_use]
    pub fn declined_by_the_caps(&self, conversation_id: i64, ledger: &[Block]) -> Option<String> {
        let declined = cap_decline(ledger);
        if declined.is_some() {
            self.stop_the_cue(conversation_id);
        }
        declined
    }

    /// One tool call's whole answer: the read input, or the refusal the
    /// parameters answered with.
    ///
    /// Every path that ends WITHOUT a filed block stops the composing cue
    /// here — a refused parameter, an unknown target, a spent transient
    /// read — because the cue lit when the call started and only a filed
    /// send reaches a delivery report that would end it. One place, so no
    /// refusal can be added that leaves the chat typing.
    pub async fn answer(
        &self,
        ctx: &ToolContext<'_, CoreEvent>,
        named: Result<NamedSend, &'static str>,
    ) -> ToolOutcome {
        let outcome = match named {
            Ok(named) => self.file(ctx, &named).await,
            Err(refusal) => ToolOutcome::Error(refusal.to_owned()),
        };
        if matches!(outcome, ToolOutcome::Error(_)) {
            self.stop_the_cue(ctx.agency.conversation_id);
        }
        outcome
    }

    /// Tell every composing edge this conversation's send is over. A
    /// conversation no edge is watching answers an error, which is nothing
    /// to act on: the cue is live-only.
    pub(crate) fn stop_the_cue(&self, conversation_id: i64) {
        let _ = self.stops.send(conversation_id);
    }

    /// File one send, or answer the refusal the model reads.
    ///
    /// `Ok` is always [`ToolOutcome::Pending`]: the call is settled by the
    /// delivery receipt, whether this run appended the block or found the
    /// block a previous run of the same call appended.
    async fn file(&self, ctx: &ToolContext<'_, CoreEvent>, named: &NamedSend) -> ToolOutcome {
        let _no_erasure_mid_filing = self.fence.read().await;
        match self.append(ctx, named).await {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(
                    conversation_id = ctx.agency.conversation_id,
                    %error,
                    "the send's ledger read or append failed; nothing was filed"
                );
                ToolOutcome::Error(transient_error())
            }
        }
    }

    /// The filing itself, behind both holds. Separated so the store's own
    /// failures answer one transient sentence in one place, instead of once
    /// per read.
    async fn append(
        &self,
        ctx: &ToolContext<'_, CoreEvent>,
        named: &NamedSend,
    ) -> Result<ToolOutcome, StoreError> {
        let conversation_id = ctx.agency.conversation_id;
        let store: &Store = &ctx.agency.store;
        let ledger = store.list_blocks(conversation_id).await?;
        // Idempotent on the call block: a re-run of this very call finds
        // the block it already filed and appends no second one, so a
        // restart-recovered round never doubles a message in the chat.
        if let Some(filed) = outgoing::send_of_call(&ledger, ctx.block_id) {
            tracing::debug!(
                conversation_id,
                block_id = filed.block_id,
                "this call already filed its message; the re-run appends none"
            );
            return Ok(ToolOutcome::Pending);
        }
        if let Some(reply_to) = &named.reply_to
            && !ledger_holds(&ledger, reply_to)
        {
            return Ok(ToolOutcome::Error(UNKNOWN_TARGET_ERROR.to_owned()));
        }
        store
            .append_consumer_block(
                conversation_id,
                None,
                outgoing::OUTGOING_MESSAGE_KIND,
                OutgoingMessage::stored_fields(
                    &named.text,
                    named.reply_to.as_deref(),
                    ctx.block_id,
                ),
                None,
            )
            .await?;
        Ok(ToolOutcome::Pending)
    }
}

#[cfg(test)]
mod tests {
    use agent_ledger::store::ToolCallInsert;
    use agent_ledger::{
        AgencyCtx, CallOrigin, EventBus, Role, ToolHandler, ToolRegistry, ToolRunner,
    };
    use serde_json::json;

    use super::*;
    use crate::tools::send::SendMessage;

    /// One outgoing block filed at the given moment for the given call.
    fn filed(id: i64, call_block: i64, created_at: &str) -> Block {
        Block {
            id,
            role: None,
            block_type: outgoing::OUTGOING_MESSAGE_KIND.into(),
            created_at: created_at.into(),
            dispatch_anchor: None,
            fields: OutgoingMessage::stored_fields("a message", None, call_block),
        }
    }

    /// One recorded call under the given provider echo.
    fn call(id: i64, echo: &str) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("tool_call_id".into(), json!(echo));
        fields.insert("name".into(), json!("send_message"));
        Block {
            id,
            role: Some(Role::Assistant),
            block_type: "tool_call".into(),
            created_at: String::new(),
            dispatch_anchor: Some(1),
            fields,
        }
    }

    /// One recorded resolution naming the call block it answers.
    fn resolved(id: i64, kind: &str, call_block: i64) -> Block {
        let mut fields = serde_json::Map::new();
        fields.insert("source_block_id".into(), json!(call_block));
        Block {
            id,
            role: None,
            block_type: kind.into(),
            created_at: String::new(),
            dispatch_anchor: Some(1),
            fields,
        }
    }

    /// A ledger of `count` sends, each `apart` seconds before `now`, all
    /// still pending — the shape the tier edges are read against.
    fn pending_sends(count: usize, apart: i64, now: DateTime<Utc>) -> Vec<Block> {
        let mut ledger = Vec::new();
        for index in 0..count {
            let id = i64::try_from(index).expect("the fixture is small");
            let at = now - TimeDelta::seconds(apart * (id + 1));
            ledger.push(call(100 + id * 2, &format!("echo-{id}")));
            ledger.push(filed(
                101 + id * 2,
                100 + id * 2,
                &at.to_rfc3339_opts(SecondsFormat::Millis, false),
            ));
        }
        ledger
    }

    /// The caps as the operator set them, pinned where they are declared:
    /// five a minute, thirty an hour, a hundred a day, narrowest first.
    #[test]
    fn the_caps_are_the_three_decided_tiers() {
        assert_eq!(
            CAPS,
            [
                Tier {
                    sends: 5,
                    seconds: 60,
                    span: "minute"
                },
                Tier {
                    sends: 30,
                    seconds: 3600,
                    span: "hour"
                },
                Tier {
                    sends: 100,
                    seconds: 86400,
                    span: "day"
                },
            ]
        );
    }

    /// Each tier's own edge (AC11): the allowance's last send passes and
    /// the one past it is refused, per tier, with the tier that reopens
    /// soonest doing the refusing.
    #[test]
    fn every_tier_admits_its_allowance_and_refuses_the_send_past_it() {
        let now = DateTime::parse_from_rfc3339("2026-09-02T12:00:00Z")
            .expect("the fixture parses")
            .with_timezone(&Utc);

        // The minute: four sends inside it admit, five refuse.
        assert_eq!(spent_tier(&pending_sends(4, 10, now), now), None);
        let (tier, reopens) =
            spent_tier(&pending_sends(5, 10, now), now).expect("the minute is spent");
        assert_eq!(tier.span, "minute");
        assert_eq!(
            reopens,
            now - TimeDelta::seconds(50) + TimeDelta::seconds(60),
            "the minute reopens when its oldest counted send ages out"
        );

        // The hour: twenty-nine sends spread past the minute admit, thirty
        // refuse. The spacing keeps at most four inside any one minute, so
        // the narrower tier stays silent and the hour is what answers.
        assert_eq!(spent_tier(&pending_sends(29, 20, now), now), None);
        let (tier, _) = spent_tier(&pending_sends(30, 20, now), now).expect("the hour is spent");
        assert_eq!(tier.span, "hour");

        // The day: ninety-nine sends spread past the hour admit, a hundred
        // refuse.
        assert_eq!(spent_tier(&pending_sends(99, 700, now), now), None);
        let (tier, _) = spent_tier(&pending_sends(100, 700, now), now).expect("the day is spent");
        assert_eq!(tier.span, "day");
    }

    /// A failed send counts for nothing (AC11): five sends inside the
    /// minute of which one failed leave the allowance open, and the
    /// delivered and pending ones alike burn a slot.
    #[test]
    fn a_failed_send_burns_no_allowance_and_a_delivered_one_does() {
        let now = DateTime::parse_from_rfc3339("2026-09-02T12:00:00Z")
            .expect("the fixture parses")
            .with_timezone(&Utc);
        let mut ledger = pending_sends(5, 5, now);
        assert!(
            spent_tier(&ledger, now).is_some(),
            "five pending sends spend the minute"
        );

        ledger.push(resolved(900, "tool_error", 100));
        assert_eq!(
            spent_tier(&ledger, now),
            None,
            "the failed send posted nothing, so it burns no slot"
        );

        ledger.push(resolved(901, "tool_result", 102));
        assert_eq!(
            spent_tier(&ledger, now),
            None,
            "a delivered send still counts as exactly one"
        );
        ledger.push(call(902, "echo-fresh"));
        ledger.push(filed(
            903,
            902,
            &now.to_rfc3339_opts(SecondsFormat::Millis, false),
        ));
        assert!(
            spent_tier(&ledger, now).is_some(),
            "the fifth counted send spends the minute again"
        );
    }

    /// A send older than the span is outside it: the same five sends, an
    /// hour apart, spend no tier.
    #[test]
    fn sends_outside_the_span_are_not_counted() {
        let now = DateTime::parse_from_rfc3339("2026-09-02T12:00:00Z")
            .expect("the fixture parses")
            .with_timezone(&Utc);
        assert_eq!(spent_tier(&pending_sends(5, 3600, now), now), None);
    }

    /// The refusal names the tier and the moment it reopens, in UTC, and
    /// tells the model to continue later rather than to retry now.
    #[test]
    fn the_cap_refusal_names_the_tier_and_its_reopening() {
        let reopens = DateTime::parse_from_rfc3339("2026-09-02T12:01:00Z")
            .expect("the fixture parses")
            .with_timezone(&Utc);
        assert_eq!(
            cap_refusal(CAPS[0], reopens),
            "declined: this conversation has sent its 5 messages for the last minute, and \
             this one was not sent. The allowance reopens at 2026-09-02T12:01:00Z. Send \
             nothing more this turn; continue on a later one."
        );
    }

    /// The parameters, both tools (AC3, AC5): a well-formed plain call
    /// reads its verbatim text and names no target; a well-formed reply
    /// reads both; every unusable text shape is the text refusal and every
    /// unusable target shape is the target refusal — a bare number aside,
    /// which several providers emit for an id the envelope showed as
    /// digits.
    #[test]
    fn the_parameters_read_the_text_and_the_target_and_refuse_every_unusable_shape() {
        let plain = named_send(&json!({ PARAMETER_TEXT: "  the answer  " }).to_string())
            .expect("a well-formed send reads");
        assert_eq!(
            plain,
            NamedSend {
                text: "  the answer  ".to_owned(),
                reply_to: None,
            },
            "the text is content and is not trimmed"
        );

        let threaded = named_reply(
            &json!({ PARAMETER_TEXT: "the answer", PARAMETER_REPLY_TO: "  12345  " }).to_string(),
        )
        .expect("a well-formed reply reads");
        assert_eq!(
            threaded,
            NamedSend {
                text: "the answer".to_owned(),
                reply_to: Some("12345".to_owned()),
            },
            "the id is trimmed: it is an identifier the envelope showed"
        );
        assert_eq!(
            named_reply(&json!({ PARAMETER_TEXT: "t", PARAMETER_REPLY_TO: 12345 }).to_string())
                .expect("a numeric id reads")
                .reply_to,
            Some("12345".to_owned())
        );

        for unusable in [
            "{}",
            r#"{"text":""}"#,
            r#"{"text":"   "}"#,
            r#"{"text":7}"#,
            "not json",
        ] {
            assert_eq!(
                named_send(unusable).err(),
                Some(NEEDS_TEXT_ERROR),
                "the text refusal covers: {unusable:?}"
            );
            assert_eq!(
                named_reply(unusable).err(),
                Some(NEEDS_TEXT_ERROR),
                "the reply reads the text first: {unusable:?}"
            );
        }
        for unusable in [
            r#"{"text":"t"}"#,
            r#"{"text":"t","reply_to":""}"#,
            r#"{"text":"t","reply_to":"   "}"#,
            r#"{"text":"t","reply_to":null}"#,
        ] {
            assert_eq!(
                named_reply(unusable).err(),
                Some(NEEDS_TARGET_ERROR),
                "the target refusal covers: {unusable:?}"
            );
        }
    }

    /// The target validation (AC5), each accepted kind and the refusal: a
    /// member message by its own id and by the id it was revised under, a
    /// join notice's event id, one of the assistant's delivered ids — and
    /// nothing else, an erased row's emptied ids included.
    #[test]
    fn the_ledger_holds_exactly_the_ids_a_reply_may_name() {
        let message = |id: i64, origin: Option<&str>, revises: Option<&str>| Block {
            id,
            role: Some(Role::User),
            block_type: crate::kind::CHAT_MESSAGE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields: crate::kind::ChatMessage::stored_fields(
                "a recorded line",
                crate::kind::RecordedSender {
                    principal_id: 7,
                    authority: Authority::Member,
                    speaker: None,
                },
                crate::kind::RecordedOrigin { origin, revises },
                None,
                "2026-09-01T00:00:00Z",
                crate::kind::Stamp::compose(
                    crate::kind::Summons {
                        summoned: true,
                        literal_addressed: true,
                    },
                    Authority::Member,
                    None,
                    None,
                ),
            ),
        };
        let ledger = vec![
            message(1, Some("member-1"), None),
            message(2, Some("member-2"), Some("member-root")),
            Block {
                id: 3,
                role: None,
                block_type: crate::join::JOIN_NOTICE_KIND.into(),
                created_at: String::new(),
                dispatch_anchor: None,
                fields: crate::join::JoinNotice::stored_fields(
                    crate::join::RecordedJoiner {
                        principal_id: 9,
                        name: "A joiner",
                        handle: Some("joiner"),
                    },
                    "join-1",
                    "2026-09-01T00:00:00Z",
                ),
            },
            Block {
                id: 4,
                role: None,
                block_type: crate::delivery::DELIVERED_KIND.into(),
                created_at: String::new(),
                dispatch_anchor: None,
                fields: crate::delivery::Delivered::stored_fields("hers-1", "hers-1", Some(2)),
            },
            // An erased row: the erasure nulled its text and both of its
            // identifiers, so it holds no id at all.
            Block {
                id: 5,
                role: Some(Role::User),
                block_type: crate::kind::CHAT_MESSAGE_KIND.into(),
                created_at: String::new(),
                dispatch_anchor: None,
                fields: serde_json::Map::new(),
            },
        ];

        for held in ["member-1", "member-2", "member-root", "join-1", "hers-1"] {
            assert!(ledger_holds(&ledger, held), "the ledger holds {held}");
        }
        for absent in ["member-3", "join-2", "hers-2", "invented", ""] {
            assert!(
                !ledger_holds(&ledger, absent),
                "the ledger does not hold {absent}"
            );
        }
    }

    /// The refusal wording, pinned where it is written: the two shapes the
    /// model can correct inside the turn teach no never-again, and the
    /// unknown target sends nothing plain instead.
    #[test]
    fn the_refusal_wording_is_pinned_verbatim() {
        assert_eq!(
            NEEDS_TEXT_ERROR,
            "declined: a message needs text. Write what you want the group to read, or end \
             your turn without sending anything."
        );
        assert_eq!(
            NEEDS_TARGET_ERROR,
            "declined: a reply names the message it answers, by the msgid shown in that \
             message's envelope. Name one, or send the message with send_message instead."
        );
        assert_eq!(
            UNKNOWN_TARGET_ERROR,
            "declined: this conversation holds no message with that id, so there is nothing \
             to reply to. Take the id from the msgid line of the message you mean, or send \
             the message with send_message instead."
        );
        for correctable in [NEEDS_TEXT_ERROR, NEEDS_TARGET_ERROR, UNKNOWN_TARGET_ERROR] {
            assert!(
                !correctable.contains(crate::tools::admission::NO_RETRY),
                "a refusal the model can correct this turn teaches no never-again: \
                 {correctable}"
            );
        }
        let transient = transient_error();
        assert!(
            transient.contains("right now"),
            "a transient fact names the moment: {transient}"
        );
    }

    /// The pair is enumerated once and both names read through it: the
    /// typing cue and the contract notice ask the same question of the same
    /// list.
    #[test]
    fn the_pair_is_enumerated_once() {
        assert_eq!(NAMES, [crate::tools::send::NAME, crate::tools::reply::NAME]);
        assert!(is_sending_tool(crate::tools::send::NAME));
        assert!(is_sending_tool(crate::tools::reply::NAME));
        assert!(!is_sending_tool(crate::tools::mark::NAME));
        assert!(!is_sending_tool("send_message "));
    }

    /// The idempotent re-run (AC3), through the registered tool over a real
    /// store: the first run of a call files one message and answers
    /// pending; the SAME call run again — a restart-recovered round, a
    /// redelivered wakeup — finds its own outgoing block, appends no second
    /// one and answers pending again. One message in the chat, never two.
    #[tokio::test]
    async fn a_re_run_of_one_call_files_no_second_message() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation_id = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let input = json!({ PARAMETER_TEXT: "the answer" }).to_string();
        let call = store
            .insert_tool_call_block(
                conversation_id,
                Role::Assistant,
                ToolCallInsert {
                    tool_call_id: "echo-1".into(),
                    name: crate::tools::send::NAME.into(),
                    input: input.clone(),
                    interactive: false,
                },
                None,
            )
            .await
            .expect("the call block appends");
        let agency = AgencyCtx {
            conversation_id,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let tool = SendMessage::new(Arc::new(RwLock::new(())), crate::composing::stops());

        let first = tool
            .execute(
                &input,
                ToolContext {
                    agency: &agency,
                    tool_call_id: "echo-1",
                    block_id: call,
                },
            )
            .await;
        assert!(
            matches!(first, ToolOutcome::Pending),
            "the filed send waits on the platform"
        );
        let filed = filed_of(&agency.store, conversation_id).await;
        assert_eq!(filed.len(), 1, "the call filed exactly one message");
        assert_eq!(
            filed[0].call_block, call,
            "the block records the call it answers"
        );
        assert_eq!(
            stored_text(&agency.store, conversation_id, filed[0].block_id).await,
            Some("the answer".to_owned()),
            "the stored text is the model's own words"
        );

        let again = tool
            .execute(
                &input,
                ToolContext {
                    agency: &agency,
                    tool_call_id: "echo-1",
                    block_id: call,
                },
            )
            .await;
        assert!(
            matches!(again, ToolOutcome::Pending),
            "the re-run answers pending again"
        );
        assert_eq!(
            filed_of(&agency.store, conversation_id).await,
            filed,
            "the re-run appends nothing: the one block the first run filed still stands alone"
        );
    }

    /// Every send one conversation holds, as the idempotence read sees them.
    async fn filed_of(store: &Store, conversation_id: i64) -> Vec<outgoing::FiledSend> {
        outgoing::filed_sends(
            &store
                .list_blocks(conversation_id)
                .await
                .expect("the ledger reads"),
        )
    }

    /// The caps through the FRAMEWORK'S RUNNER against a real store (AC11):
    /// five calls of the registered tool inside one minute each file their
    /// message, and the sixth is refused with the recorded sentence and
    /// files nothing.
    ///
    /// Driven end to end on purpose — registry, runner, admission, body,
    /// ledger — because the count is answered inside the admission hook and
    /// nothing but the runner asks that hook. Each send is settled as
    /// delivered before the next call, which is what the platform's receipt
    /// does and what the in-order rule waits for.
    #[tokio::test]
    async fn the_sixth_send_in_one_minute_is_refused_through_the_runner() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation_id = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let mut registry = ToolRegistry::<CoreEvent>::new();
        registry.register(
            crate::tools::send::NAME,
            SendMessage::new(Arc::new(RwLock::new(())), crate::composing::stops()),
        );
        let runner = ToolRunner::<AssistantKind, CoreEvent>::new(Arc::new(registry));
        let agency = AgencyCtx {
            conversation_id,
            store,
            bus: Arc::new(EventBus::new()),
        };

        let mut calls = Vec::new();
        for index in 0..6 {
            let call = runner
                .insert_call(
                    &agency,
                    false,
                    format!("echo-{index}"),
                    crate::tools::send::NAME.into(),
                    json!({ PARAMETER_TEXT: format!("message {index}") }).to_string(),
                    CallOrigin::default(),
                )
                .await
                .expect("the call block appends");
            runner.run_wakeup(&agency, false, call).await;
            calls.push(call);
            // The receipt the platform would report, so the next in-order
            // call is not parked behind this one — and so the send counts
            // as one that reached the chat.
            if index < 5 {
                agency
                    .store
                    .resolve_tool_call(
                        conversation_id,
                        call,
                        agent_ledger::ToolCallResult::Success {
                            content: "sent".into(),
                        },
                    )
                    .await
                    .expect("the settlement writes");
            }
        }

        let filed = filed_of(&agency.store, conversation_id).await;
        assert_eq!(
            filed.len(),
            5,
            "the minute admits five messages and the sixth files none"
        );
        assert_eq!(
            filed.iter().map(|send| send.call_block).collect::<Vec<_>>(),
            calls[..5],
            "the five filed messages are the five admitted calls"
        );

        let refusals: Vec<String> = agency
            .store
            .list_blocks(conversation_id)
            .await
            .expect("the ledger reads")
            .iter()
            .filter_map(|block| match agent_ledger::BlockKind::from_block(block) {
                agent_ledger::BlockKind::ToolError(failure)
                    if failure.call_block_id == Some(calls[5]) =>
                {
                    Some(failure.error)
                }
                _ => None,
            })
            .collect();
        assert_eq!(refusals.len(), 1, "the sixth call is answered exactly once");
        assert!(
            refusals[0].starts_with(
                "declined: this conversation has sent its 5 messages for the last minute, and \
                 this one was not sent. The allowance reopens at "
            ) && refusals[0].ends_with("Send nothing more this turn; continue on a later one."),
            "the refusal is the recorded sentence: {}",
            refusals[0]
        );
    }

    /// A call refused before anything is filed stops the composing cue
    /// (AC12): the cue lit when the call started and no delivery will ever
    /// report on a send that was never filed, so the tool says the send is
    /// done itself — on the caps' refusal, and on every refusal its body
    /// answers.
    #[tokio::test]
    async fn a_refusal_before_filing_stops_the_cue() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation_id = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        let stops = crate::composing::stops();
        let mut stopped = stops.subscribe();
        let sender = Sender::new(Arc::new(RwLock::new(())), stops);
        let now = Utc::now();

        assert_eq!(
            sender.declined_by_the_caps(conversation_id, &pending_sends(5, 5, now)),
            cap_decline(&pending_sends(5, 5, now)),
            "the spent tier declines with the caps' own sentence"
        );
        assert_eq!(
            stopped.try_recv().expect("the spent tier stops the cue"),
            conversation_id
        );

        assert_eq!(
            sender.declined_by_the_caps(conversation_id, &[]),
            None,
            "an unspent conversation is admitted"
        );
        assert!(
            stopped.try_recv().is_err(),
            "an admitted call stops nothing: its send is still coming"
        );

        let agency = AgencyCtx {
            conversation_id,
            store,
            bus: Arc::new(EventBus::new()),
        };
        let call = agency
            .store
            .insert_tool_call_block(
                conversation_id,
                Role::Assistant,
                ToolCallInsert {
                    tool_call_id: "echo-refused".into(),
                    name: crate::tools::send::NAME.into(),
                    input: "{}".into(),
                    interactive: false,
                },
                None,
            )
            .await
            .expect("the call block appends");
        let ctx = ToolContext {
            agency: &agency,
            tool_call_id: "echo-refused",
            block_id: call,
        };
        assert!(
            matches!(
                sender.answer(&ctx, named_send("{}")).await,
                ToolOutcome::Error(refusal) if refusal == NEEDS_TEXT_ERROR
            ),
            "a call with no text is refused"
        );
        assert_eq!(
            stopped.try_recv().expect("the refused call stops the cue"),
            conversation_id
        );
        assert!(
            filed_of(&agency.store, conversation_id).await.is_empty(),
            "a refused call files nothing"
        );
    }

    /// The stored text of one filed send, read back off the store.
    async fn stored_text(store: &Store, conversation_id: i64, block_id: i64) -> Option<String> {
        store
            .list_blocks(conversation_id)
            .await
            .expect("the ledger reads")
            .iter()
            .find(|block| block.id == block_id)
            .and_then(|block| match AssistantKind::from_block(block) {
                AssistantKind::OutgoingMessage(outgoing) => outgoing.text,
                _ => None,
            })
    }
}
