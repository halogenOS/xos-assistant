//! The first-interaction disclosure: the assistant's first answer to each
//! person opens with the line naming it an AI system (unit 12, 2026-08-23).
//!
//! The transparency duty attaches to the natural person, at the latest at
//! their first interaction, and the ledger is the memory of who was already
//! introduced — no table, no flag (decision 0078). An answer's summoning
//! people are read through the same debt-origin walk every per-person fact
//! uses ([`crate::tools::provenance`]), and a person counts as introduced
//! exactly when an earlier answer block they co-summoned OPENS with the
//! line. The stored line marks the introduction's HANDOFF, not its arrival:
//! a process death before the edge's read loses the answer and introduces
//! nobody, but once the line is committed and the reply handed to the
//! adapter, a send that fails every attempt is dropped without feedback —
//! the same accepted loss the report delivery records — and that person is
//! recorded introduced although the line never reached them. The at-least
//! honest bound: the window is one handoff wide, a rare total send failure,
//! and the person's next answer is bare instead of doubled — the accepted
//! direction here, stated plainly instead of calling the line a delivery
//! receipt.
//!
//! The line is stored, not added at delivery (decision 0079): before an
//! answer's first delivery, the edge writes the line into the stored answer
//! block itself, idempotently, and the same line opens the text that goes
//! out — so the model reads in its own history that this person was already
//! introduced, and the introduction the channel received is the one the
//! ledger records. The stored answer and the delivered one are the same
//! prose under the same line, differing only where the send cut a leaked
//! reasoning prefix (unit 43): the resolution answers which opening the
//! answer takes, never with prose, and the edge composes the line over the
//! cut text — so a re-delivered answer that already carries the line goes
//! out lined again, with the trace gone and the introduction kept. The
//! framework owns the
//! finalize transaction, so the consumer's prepend rides the edge's first
//! read of the finalized block: the earliest consumer-owned moment, ahead
//! of every delivery. The prepend is mechanical — the model neither writes
//! the line nor can omit it.
//!
//! The line is a configured VALUE since unit 14 (2026-08-23): the
//! `disclosure` key overrides the text whole, and an unset key composes it
//! from the assistant's resolved name — never empty, because the duty is
//! not optional. The introduction receipt reads the CURRENT line's prefix,
//! so a deployment that edits the line re-introduces people the old line
//! already reached — the harmless direction, one repeated line, the same
//! fold the module already takes for unreadable provenance.
//!
//! Unreadable provenance folds TOWARD the line: an answer whose summoners
//! cannot be read is introduced as if everyone were new, because a repeated
//! line is harmless and a skipped first one is the violation — the inverse
//! of the admission fold, for the inverse duty.

use agent_ledger::store::StoreTx;
use agent_ledger::{Block, BlockKind, FromBlock, Role, Store, StoreError};
use serde_json::json;

use crate::kind::{self, AssistantKind, FrameworkKind};
use crate::tools::provenance;

/// The disclosure line composed from the assistant's name — what an unset
/// `disclosure` key resolves to. The shape follows the operator's original
/// copy (decision 0079) with the name as its one slot.
#[must_use]
pub fn composed_disclosure_line(name: &str) -> String {
    format!("Hi, I'm {name}, an AI system, made to assist members of the community.")
}

/// Which opening one answer goes out under, and the whole of what the
/// resolution answers ([`Disclosure::introduction_for`]). The value carries
/// no prose: the caller composes the text it opens, so the introduction is
/// resolved in one place and the wire text is built in one other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Introduction {
    /// The line and a blank line open the answer: someone it speaks to is
    /// new, or the stored block already carries the line from an earlier
    /// delivery.
    Lined,
    /// The answer opens with its own first word: everyone it speaks to was
    /// introduced by an earlier answer.
    Bare,
}

/// The first-interaction disclosure as one resolved value: the line the
/// first answer to each person opens with, and the prefix — the line plus a
/// blank line — the prepend writes and the introduction receipt reads.
/// Resolved once at the assembly from the `disclosure` key and the
/// assistant's name; the mechanism around it is unit 12's, unchanged.
#[derive(Debug, Clone)]
pub struct Disclosure {
    line: String,
    prefix: String,
}

impl Disclosure {
    /// Resolve the disclosure: the configured text verbatim, or the line
    /// composed from the name when no text is configured. Never empty by
    /// construction — the embedder's configuration refuses an empty
    /// `disclosure` value and an empty name, and the composed line is
    /// non-empty on its own.
    #[must_use]
    pub fn resolve(configured: Option<&str>, name: &str) -> Self {
        let line = configured.map_or_else(|| composed_disclosure_line(name), ToOwned::to_owned);
        let prefix = format!("{line}\n\n");
        Self { line, prefix }
    }

    /// The resolved line itself.
    #[must_use]
    pub fn line(&self) -> &str {
        &self.line
    }

    /// An answer as the first answer to someone delivers it: the line, a
    /// blank line, then the answer — the exact text the store carries after
    /// the prepend.
    #[must_use]
    pub fn disclosed(&self, answer: &str) -> String {
        format!("{prefix}{answer}", prefix = self.prefix)
    }

    /// One answer's own prose: the stored content without the line an
    /// earlier delivery may already have written into it.
    ///
    /// The send's cut runs on this and never on the stored content whole
    /// (unit 43), so a leaked reasoning prefix inside the prose cannot take
    /// the introduction away with it, and [`Self::disclosed`] composes the
    /// one line back in front of whatever the cut left.
    /// The prefix test here and the one in [`Self::introduction_for`] are
    /// ONE decision — a block opening with the line counts as lined — kept
    /// textually adjacent on purpose; change both or neither. Stripping and
    /// re-adding the same prefix is byte-identity, so a model answer that
    /// happens to open with the exact line is delivered as written, never
    /// rewritten.
    #[must_use]
    pub(crate) fn prose_of<'a>(&self, content: &'a str) -> &'a str {
        content.strip_prefix(&self.prefix).unwrap_or(content)
    }

    /// The introduction one undelivered answer block goes out under, with
    /// the receipt resolved: when any summoning person of the answer was
    /// never introduced, the line is written into the stored block first —
    /// one idempotent statement — and the loaded vector's copy is updated
    /// with it, so a second undelivered answer in the same pass reads the
    /// receipt. A block that already opens with the line was resolved by an
    /// earlier delivery and is lined again: at-least-once delivery repeats
    /// the introduction that block carries, and never drops it.
    ///
    /// No text crosses this seam's SIGNATURE in either direction — the
    /// wire text is composed by the caller, in exactly one place, so the
    /// resolution cannot be handed a text that skipped the cut. Text does
    /// cross via persistence, deliberately: a first introduction writes the
    /// lined content into the stored block ([`Self::store_line`]), which is
    /// how a later re-delivery finds the line already in place.
    ///
    /// # Errors
    ///
    /// [`StoreError`] if a read or the prepend write fails.
    pub(crate) async fn introduction_for(
        &self,
        store: &Store,
        conversation_id: i64,
        ledger: &mut [Block],
        index: usize,
    ) -> Result<Introduction, StoreError> {
        let content = message_content(&ledger[index]);
        if content.starts_with(&self.prefix) {
            return Ok(Introduction::Lined);
        }
        let block_id = ledger[index].id;
        // The summoning key is the CALL an outgoing block answers, since a
        // consumer append carries no dispatch anchor of its own; an
        // unreadable key folds toward the line like every other absence
        // here.
        let summoning_key = summoning_key(&ledger[index]).unwrap_or(block_id);
        if !self
            .first_message_to_someone(store, conversation_id, ledger, summoning_key)
            .await?
        {
            return Ok(Introduction::Bare);
        }
        self.store_line(&store.tx(), block_id).await?;
        ledger[index].fields.insert(
            crate::outgoing::COLUMN_TEXT.into(),
            json!(self.disclosed(&content)),
        );
        Ok(Introduction::Lined)
    }

    /// Whether this message is the first to any of its summoning people:
    /// the co-summoner set is read from the summoning key's dispatch
    /// anchor, several people are each checked, and the line shows if ANY
    /// of them is new (decision 0078). An empty or unreadable summoner set
    /// answers true — the fold toward the line the module doc states.
    async fn first_message_to_someone(
        &self,
        store: &Store,
        conversation_id: i64,
        ledger: &[Block],
        answer_id: i64,
    ) -> Result<bool, StoreError> {
        let summoners = provenance::co_summoners(ledger, answer_id);
        if summoners.is_empty() {
            return Ok(true);
        }
        let mut checked: Vec<i64> = Vec::new();
        for summoner in &summoners {
            let Some(principal) = summoner.principal_id else {
                return Ok(true);
            };
            if checked.contains(&principal) {
                continue;
            }
            checked.push(principal);
            if !self
                .introduced_before(store, conversation_id, ledger, principal, answer_id)
                .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether this person was introduced before the given block id,
    /// anywhere in the store: some earlier answer block they co-summoned
    /// opens with the line. Per person across conversations — the duty
    /// attaches to the natural person, so a person introduced in one
    /// channel is not introduced again in another. The current conversation
    /// is read from the vector already loaded; every other conversation the
    /// person spoke in is read once. Block ids are monotonic across the
    /// whole store, so "earlier" is one comparison everywhere.
    async fn introduced_before(
        &self,
        store: &Store,
        conversation_id: i64,
        ledger: &[Block],
        principal: i64,
        before: i64,
    ) -> Result<bool, StoreError> {
        if self.introduction_in(ledger, principal, before) {
            return Ok(true);
        }
        for spoke_in in kind::conversations_of_principal(&store.tx(), principal).await? {
            if spoke_in == conversation_id {
                continue;
            }
            let blocks = store.list_blocks(spoke_in).await?;
            if self.introduction_in(&blocks, principal, before) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether one conversation's loaded vector holds the person's
    /// introduction before the given id: a lined answer whose co-summoners
    /// include them. A person returning after full deletion resolves to a
    /// fresh principal id, which no stored answer's co-summoners name — so
    /// the line shows again, correct in both directions: the store
    /// genuinely does not know them, and the duty resets with the erased
    /// memory.
    fn introduction_in(&self, blocks: &[Block], principal: i64, before: i64) -> bool {
        blocks
            .iter()
            .filter(|block| block.id < before && self.lined_message(block))
            .filter_map(summoning_key)
            .any(|key| {
                provenance::co_summoners(blocks, key)
                    .iter()
                    .any(|summoner| summoner.principal_id == Some(principal))
            })
    }

    /// Whether a block is one of the assistant's own messages that opens
    /// with the line — the stored receipt of a delivered introduction.
    ///
    /// TWO shapes answer, and both are honest (unit 55, 2026-09-02). A
    /// filed outgoing message is what carries the line from this unit on.
    /// An assistant TEXT block carrying it is history: while answers were
    /// relayed, the line was written into exactly such a block before its
    /// send, and a person that line reached is introduced whether or not
    /// the mechanism that reached them still exists. Reading only the new
    /// shape would re-introduce everyone the old one had already met.
    fn lined_message(&self, block: &Block) -> bool {
        match AssistantKind::from_block(block) {
            AssistantKind::OutgoingMessage(outgoing) => outgoing
                .text
                .is_some_and(|text| text.starts_with(&self.prefix)),
            AssistantKind::Core(FrameworkKind(BlockKind::Text(text))) => {
                text.role == Some(Role::Assistant) && text.content.starts_with(&self.prefix)
            }
            _ => false,
        }
    }

    /// Write the line into the stored outgoing block, in one idempotent
    /// statement owned by the kind whose column it writes: the prepend
    /// applies only while the text does not already open with the prefix,
    /// so a repeated send cannot stack a second line.
    async fn store_line(&self, tx: &StoreTx, block_id: i64) -> Result<(), StoreError> {
        crate::outgoing::prepend_line(tx, block_id, &self.prefix).await
    }
}

/// The stored text of one message the model filed, read through the
/// composed kind's one parse path. One shape reaches the introduction — the
/// outgoing block, which is the only thing the edge introduces — so
/// anything else reads as empty and nothing downstream invents text for it.
/// The old relay's lined text blocks are read by `lined_message` instead,
/// where the history question is asked.
fn message_content(block: &Block) -> String {
    match AssistantKind::from_block(block) {
        AssistantKind::OutgoingMessage(outgoing) => outgoing.text.unwrap_or_default(),
        _ => String::new(),
    }
}

/// The block id one of the assistant's own messages reads its summoning
/// people from — the id whose dispatch anchor the provenance walk keys on.
///
/// For an outgoing block that is the CALL it answers, not the block itself:
/// a consumer append carries no anchor, while the tool call the model made
/// carries the anchor of the turn's summoning frontier. For a lined
/// assistant text block of the old relay it is the block's own id, which is
/// where the framework's dispatch stamped the anchor.
fn summoning_key(block: &Block) -> Option<i64> {
    match AssistantKind::from_block(block) {
        AssistantKind::OutgoingMessage(outgoing) => outgoing.call_block,
        _ => Some(block.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::store_config;

    /// The composition: the line, one blank line, the answer — under the
    /// name-composed default and under a configured text alike, and the
    /// configured text wins over the name.
    #[test]
    fn the_disclosed_shape_is_line_blank_line_answer() {
        let composed = Disclosure::resolve(None, "Probe");
        assert_eq!(
            composed.line(),
            "Hi, I'm Probe, an AI system, made to assist members of the community.",
            "an unset key composes the line from the name"
        );
        assert_eq!(
            composed.disclosed("the answer"),
            format!("{}\n\nthe answer", composed.line()),
            "the composition is the line, a blank line, then the answer"
        );
        assert!(
            !composed.line().contains('\n'),
            "the composed disclosure is one line"
        );

        let configured = Disclosure::resolve(Some("I am a configured machine."), "Probe");
        assert_eq!(
            configured.line(),
            "I am a configured machine.",
            "a configured text overrides the composition whole"
        );
        assert_eq!(
            configured.disclosed("the answer"),
            "I am a configured machine.\n\nthe answer"
        );
    }

    /// The fold toward the line, driven through the store: a filed send
    /// whose call block this ledger does not hold has unreadable summoners
    /// — and the resolution answers with the line, prepends it once into
    /// the STORED text, and a repeated call finds the receipt instead of
    /// stacking a second line.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unreadable_provenance_folds_toward_the_line_and_the_prepend_is_idempotent() {
        let disclosure = Disclosure::resolve(None, "Probe");
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        store
            .append_consumer_block(
                conversation,
                None,
                crate::outgoing::OUTGOING_MESSAGE_KIND,
                crate::outgoing::OutgoingMessage::stored_fields("an answer", None, 9_999),
                None,
            )
            .await
            .expect("the send files");
        let mut ledger = store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let index = ledger.len() - 1;

        let introduction = disclosure
            .introduction_for(&store, conversation, &mut ledger, index)
            .await
            .expect("the resolution reads");
        assert_eq!(
            introduction,
            Introduction::Lined,
            "a send whose summoners cannot be read is introduced"
        );

        // The stored block carries the line over the model's own words.
        let stored = store
            .list_blocks(conversation)
            .await
            .expect("the ledger re-reads");
        assert_eq!(
            stored[index].fields[crate::outgoing::COLUMN_TEXT],
            json!(disclosure.disclosed("an answer")),
            "the ledger carries the introduction the channel received"
        );

        // The repeated resolution reads the receipt: the same introduction
        // again — a re-delivery repeats the line the block carries — and no
        // second line in the store.
        let mut reread = stored;
        let again = disclosure
            .introduction_for(&store, conversation, &mut reread, index)
            .await
            .expect("the repeated resolution reads");
        assert_eq!(
            again,
            Introduction::Lined,
            "a lined block is delivered lined again"
        );
        let twice = store
            .list_blocks(conversation)
            .await
            .expect("the ledger re-reads");
        assert_eq!(
            twice[index].fields[crate::outgoing::COLUMN_TEXT],
            json!(disclosure.disclosed("an answer")),
            "one line, never two"
        );
    }

    /// The prose the line opens, told apart from the line itself: a stored
    /// block already carrying the prefix yields the answer under it, and
    /// everything else yields itself. This is what the send's cut reads
    /// (unit 43), so the line can never be what a cut takes away.
    #[test]
    fn the_prose_is_the_content_under_a_carried_line() {
        let disclosure = Disclosure::resolve(None, "Probe");
        assert_eq!(
            disclosure.prose_of(&disclosure.disclosed("the answer")),
            "the answer",
            "a lined block's prose is what stands under the line"
        );
        assert_eq!(
            disclosure.prose_of("the answer"),
            "the answer",
            "an unlined block is its own prose"
        );
        assert_eq!(
            disclosure.prose_of(&format!("Look: {}", disclosure.line())),
            format!("Look: {}", disclosure.line()),
            "the line quoted inside an answer is prose, not a carried line"
        );
    }
}
