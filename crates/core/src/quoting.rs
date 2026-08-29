//! The reply's quote: a member's reply lands preceded by the framework's
//! own quote block, referencing the message it replies to (unit 31,
//! 2026-08-28).
//!
//! Until this unit the assistant saw a reply's own text and nothing else.
//! The relationship was stored — the chat message records the replied-to
//! origin — but no part of it reached the model, so a reply read as a
//! free-standing sentence and was answered as a question nobody had asked.
//! A quote block ahead of the reply is what a human sees in the chat: the
//! quoted words, `> `-prefixed, above the member's own.
//!
//! Nothing of the quoted text is copied. The block stores a SPAN — which
//! block, which character offsets — and the store resolves it at read time
//! through the chat-message kind's declared quotable column, so an erased
//! target resolves to the empty string and renders nothing, with no
//! erasure pass needing to know quotes exist at all.
//!
//! What the span covers is decided here, from the stored text and the
//! excerpt the member selected, and never from a platform offset: the
//! excerpt is searched for in the stored text, so no encoding conversion
//! is performed anywhere in this path.

use agent_ledger::agency::Quote;
use agent_ledger::{Block, InputBlock, LeafKind, Store, StoreError};
use serde_json::Value;

use crate::kind;
use crate::message::{InboundMessage, QuotedExcerpt, ReplyTarget};

/// The span one quote block references: a single recorded message, and the
/// character range of it the member pointed at.
///
/// Both endpoints name the same block by construction — a reply quotes one
/// message — which is also the framework resolver's membership-free
/// substring path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuotedSpan {
    /// The quoted message's block.
    block_id: i64,
    /// The first quoted character, counted in characters.
    start: i64,
    /// One past the last quoted character, counted in characters.
    end: i64,
}

impl QuotedSpan {
    /// The span of one reply over one recorded message: the whole message
    /// by default, narrowed to the member's hand-selected excerpt when
    /// that excerpt is still found in the stored text.
    ///
    /// The FIRST occurrence wins, which makes the choice deterministic
    /// without consulting the platform's own offset — and every occurrence
    /// of the same string carries the same words, so there is nothing to
    /// tell apart. A miss is the ordinary case of an edited or otherwise
    /// drifted message, and it falls back to the whole message rather than
    /// to nothing: the reply keeps its context.
    fn of(target: &kind::QuotableMessage, excerpt: Option<&QuotedExcerpt>) -> Self {
        let whole = Self {
            block_id: target.block_id,
            start: 0,
            end: characters(&target.text),
        };
        // Only a hand-selected excerpt narrows: a platform-composed one is
        // not a member pointing at a part of what they answered. An empty
        // excerpt narrows nothing either — it names no part.
        let Some(excerpt) = excerpt.filter(|excerpt| excerpt.manual && !excerpt.text.is_empty())
        else {
            return whole;
        };
        let Some(byte_offset) = target.text.find(excerpt.text.as_str()) else {
            return whole;
        };
        let start = characters(&target.text[..byte_offset]);
        Self {
            start,
            end: start + characters(&excerpt.text),
            ..whole
        }
    }

    /// The block to append for this span.
    fn input_block(self) -> InputBlock {
        InputBlock::Quote {
            start_block_id: self.block_id,
            start_pos: self.start,
            end_block_id: self.block_id,
            end_pos: self.end,
        }
    }

    /// Whether one stored block is already a quote of exactly this span —
    /// the crash-retry signature described on [`land_reply_quote`]. Read
    /// off the loaded block's own fields, which is where the store puts a
    /// quote's endpoints.
    fn is_stored_as(self, block: &Block) -> bool {
        if !Quote::KINDS.contains(&block.block_type.as_str()) {
            return false;
        }
        let endpoint = |name: &str| block.fields.get(name).and_then(Value::as_i64);
        endpoint("start_block_id") == Some(self.block_id)
            && endpoint("end_block_id") == Some(self.block_id)
            && endpoint("start_pos") == Some(self.start)
            && endpoint("end_pos") == Some(self.end)
    }
}

/// How many characters a string holds, as a stored offset. Saturating on
/// purpose: the offsets are a substring bound the resolver clamps anyway,
/// and a message long enough to overflow an `i64` character count cannot
/// exist in any store this reads.
fn characters(text: &str) -> i64 {
    i64::try_from(text.chars().count()).unwrap_or(i64::MAX)
}

/// Land the quote a reply owes, ahead of the reply's own message.
///
/// Called from inside the ingest's stamp-locked stretch, immediately
/// before the chat message is appended, so no other ingestion can slide a
/// block between the pair. A framework turn-close finishing inside that
/// window can still write between them, which is harmless: the quote
/// precedes its own message, which is all the projection and the
/// resolution need.
///
/// Nothing lands, and nothing is invented, unless the reply names a
/// message this conversation actually holds:
///
/// - a message that is no reply, and a reply to one of the assistant's own
///   messages, quote nothing — no stored fact says which of her blocks a
///   reply answers, and guessing one would reproduce the misattribution
///   this unit exists to end;
/// - a reply naming an origin the conversation has no live message for —
///   from before the assistant joined, skipped as no-text, said in another
///   conversation, recorded by another kind such as a join event, or
///   already erased — lands exactly as it did before this unit.
///
/// **The tail-skip.** Delivery is at-least-once and the two appends are
/// not one transaction, so a crash between them is redelivered and would
/// otherwise land a second identical quote ahead of the message. When the
/// conversation's newest block is already a quote of this very span, the
/// append is skipped: that is precisely the crash signature, in one read
/// of the tail. A retry whose tail has moved on re-lands the pair whole,
/// riding the message doubling that at-least-once delivery already has —
/// unchanged by this unit and not hidden by it. The date-marker seam does
/// not disturb the read: a marker and the quote commit in one
/// transaction, so the tail after a crash is the quote, never a bare
/// marker.
///
/// # Errors
///
/// [`StoreError`] if the target read, the tail read or the append fails,
/// or the store's actor has stopped.
pub(crate) async fn land_reply_quote(
    store: &Store,
    conversation_id: i64,
    message: &InboundMessage,
) -> Result<(), StoreError> {
    let Some(ReplyTarget::Message { origin }) = message.reply_target.as_ref() else {
        return Ok(());
    };
    let Some(target) = kind::newest_message_of_origin(&store.tx(), conversation_id, origin).await?
    else {
        return Ok(());
    };
    let span = QuotedSpan::of(&target, message.quoted.as_ref());
    if store
        .latest_block(conversation_id)
        .await?
        .is_some_and(|tail| span.is_stored_as(&tail))
    {
        tracing::debug!(
            conversation_id,
            block_id = span.block_id,
            "the conversation's tail is already this quote; the redelivered reply re-lands none"
        );
        return Ok(());
    }
    store
        .insert_user_blocks(conversation_id, vec![span.input_block()])
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One recorded target message.
    fn target(text: &str) -> kind::QuotableMessage {
        kind::QuotableMessage {
            block_id: 7,
            text: text.into(),
        }
    }

    /// A hand-selected excerpt, as the adapter reports one.
    fn manual(text: &str) -> QuotedExcerpt {
        QuotedExcerpt {
            text: text.into(),
            manual: true,
        }
    }

    /// The default span is the whole stored message, counted in
    /// CHARACTERS: the resolver skips and takes characters, so a
    /// byte length would cut a multibyte message short.
    #[test]
    fn a_plain_reply_spans_the_whole_message() {
        let multibyte = target("die Größe — schön");
        assert_eq!(
            QuotedSpan::of(&multibyte, None),
            QuotedSpan {
                block_id: 7,
                start: 0,
                end: 17,
            },
            "seventeen characters, not the twenty-one bytes they occupy"
        );
    }

    /// A hand-selected excerpt narrows to its FIRST occurrence, in
    /// character offsets — across a multibyte boundary, where a byte
    /// offset would name a different substring entirely.
    #[test]
    fn a_manual_excerpt_narrows_to_its_first_occurrence_in_characters() {
        let text = target("die Größe — schön ist schön");
        assert_eq!(
            QuotedSpan::of(&text, Some(&manual("schön"))),
            QuotedSpan {
                block_id: 7,
                start: 12,
                end: 17,
            },
            "the first occurrence, past two multibyte characters"
        );
    }

    /// Everything that narrows nothing falls back to the whole message,
    /// each for its own recorded reason: an excerpt the stored text no
    /// longer holds (an edit, a caption mismatch), an excerpt the platform
    /// composed rather than the member selecting it, and an empty one,
    /// which names no part at all.
    #[test]
    fn every_unusable_excerpt_falls_back_to_the_whole_message() {
        let text = target("the setting moved to the top");
        let whole = QuotedSpan::of(&text, None);
        for excerpt in [
            manual("a sentence this message never held"),
            QuotedExcerpt {
                text: "the setting".into(),
                manual: false,
            },
            manual(""),
        ] {
            assert_eq!(
                QuotedSpan::of(&text, Some(&excerpt)),
                whole,
                "{excerpt:?} narrows nothing, so the reply quotes the message whole"
            );
        }
    }

    /// The tail-skip recognizes exactly its own span, and nothing else: a
    /// quote of another span, a quote of another block, and a
    /// non-quote tail all read as "not already landed".
    #[test]
    fn the_tail_skip_recognizes_exactly_this_span() {
        let span = QuotedSpan {
            block_id: 7,
            start: 0,
            end: 12,
        };
        let quote_block = |start_block: i64, start: i64, end: i64| Block {
            id: 42,
            role: Some(agent_ledger::Role::User),
            block_type: Quote::KINDS[0].into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields: serde_json::json!({
                "start_block_id": start_block,
                "start_pos": start,
                "end_block_id": start_block,
                "end_pos": end,
                "text": "",
            })
            .as_object()
            .expect("the fixture is an object")
            .clone(),
        };

        assert!(span.is_stored_as(&quote_block(7, 0, 12)));
        assert!(
            !span.is_stored_as(&quote_block(7, 0, 5)),
            "a narrower quote of the same message is a different quote"
        );
        assert!(
            !span.is_stored_as(&quote_block(9, 0, 12)),
            "the same offsets over another message are another quote"
        );

        let mut message = quote_block(7, 0, 12);
        message.block_type = crate::kind::CHAT_MESSAGE_KIND.into();
        assert!(
            !span.is_stored_as(&message),
            "a tail that is not a quote never skips the append"
        );
    }
}
