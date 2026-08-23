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
//! block itself, idempotently, and delivers the stored text — so the ledger
//! carries exactly what the channel saw, and the model reads in its own
//! history that this person was already introduced. The framework owns the
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

use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{Block, BlockKind, FromBlock, Role, Store, StoreError};
use serde_json::json;

use crate::kind::{self, AssistantKind, FrameworkKind};
use crate::schema::DOMAIN;
use crate::tools::provenance;

/// The disclosure line composed from the assistant's name — what an unset
/// `disclosure` key resolves to. The shape follows the operator's original
/// copy (decision 0079) with the name as its one slot.
#[must_use]
pub fn composed_disclosure_line(name: &str) -> String {
    format!("Hi, I'm {name}, an AI system, made to assist members of the community.")
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

    /// The deliverable text of one undelivered answer block, with the
    /// introduction resolved: when any summoning person of the answer was
    /// never introduced, the line is written into the stored block first —
    /// one idempotent statement — and the loaded vector's copy is updated
    /// with it, so a second undelivered answer in the same pass reads the
    /// receipt. The returned text is the stored text either way; delivery
    /// and ledger cannot disagree.
    ///
    /// # Errors
    ///
    /// [`StoreError`] if a read or the prepend write fails.
    pub(crate) async fn deliverable_answer(
        &self,
        store: &Store,
        conversation_id: i64,
        ledger: &mut [Block],
        index: usize,
    ) -> Result<String, StoreError> {
        let content = answer_content(&ledger[index]);
        if content.starts_with(&self.prefix) {
            return Ok(content);
        }
        let answer_id = ledger[index].id;
        if !self
            .first_answer_to_someone(store, conversation_id, ledger, answer_id)
            .await?
        {
            return Ok(content);
        }
        self.store_line(&store.tx(), answer_id).await?;
        let stored = self.disclosed(&content);
        ledger[index].fields.insert("content".into(), json!(stored));
        Ok(stored)
    }

    /// Whether this answer is the first to any of its summoning people: the
    /// co-summoner set is read from the answer's own dispatch anchor,
    /// several people are each checked, and the line shows if ANY of them
    /// is new (decision 0078). An empty or unreadable summoner set answers
    /// true — the fold toward the line the module doc states.
    async fn first_answer_to_someone(
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
            .filter(|block| block.id < before && self.lined_answer(block))
            .any(|answer| {
                provenance::co_summoners(blocks, answer.id)
                    .iter()
                    .any(|summoner| summoner.principal_id == Some(principal))
            })
    }

    /// Whether a block is an answer that opens with the line — the stored
    /// receipt of a delivered introduction.
    fn lined_answer(&self, block: &Block) -> bool {
        matches!(
            AssistantKind::from_block(block),
            AssistantKind::Core(FrameworkKind(BlockKind::Text(text)))
                if text.role == Some(Role::Assistant)
                    && text.content.starts_with(&self.prefix)
        )
    }

    /// Write the line into the stored answer block, in one idempotent
    /// statement: the prepend applies only while the content does not
    /// already open with the prefix, so a repeated call cannot stack a
    /// second line. The framework's `block_text` table is named directly —
    /// the same deliberate coupling decision 0032 records for the
    /// framework's header and junction tables, extended to the text table
    /// by decision 0079.
    async fn store_line(&self, tx: &StoreTx, block_id: i64) -> Result<(), StoreError> {
        let prefix = self.prefix.clone();
        domain_run(tx, DOMAIN, move |conn| {
            conn.execute(
                "UPDATE block_text SET content = ?2 || content \
                 WHERE block_id = ?1 AND substr(content, 1, length(?2)) <> ?2",
                rusqlite::params![block_id, prefix],
            )?;
            Ok(())
        })
        .await
    }
}

/// One answer block's stored prose, read through the composed kind's one
/// parse path. The caller hands this an answer block it already classified;
/// anything else reads as empty, and nothing downstream invents text for it.
fn answer_content(block: &Block) -> String {
    match AssistantKind::from_block(block) {
        AssistantKind::Core(FrameworkKind(BlockKind::Text(text))) => text.content,
        _ => String::new(),
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

    /// The fold toward the line, driven through the store: an answer
    /// written through the public write surface carries no dispatch anchor,
    /// so its summoners are unreadable — and the resolution answers with
    /// the line, prepends it once, and a repeated call finds the receipt
    /// instead of stacking a second line.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unreadable_provenance_folds_toward_the_line_and_the_prepend_is_idempotent() {
        let disclosure = Disclosure::resolve(None, "Probe");
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        store
            .insert_final_text_block(conversation, Role::Assistant, "an answer".into(), None)
            .await
            .expect("the answer stores");
        let mut ledger = store
            .list_blocks(conversation)
            .await
            .expect("the ledger reads");
        let index = ledger.len() - 1;

        let delivered = disclosure
            .deliverable_answer(&store, conversation, &mut ledger, index)
            .await
            .expect("the resolution reads");
        assert_eq!(delivered, disclosure.disclosed("an answer"));

        // The stored block carries the same text the delivery got.
        let stored = store
            .list_blocks(conversation)
            .await
            .expect("the ledger re-reads");
        assert_eq!(
            stored[index].fields["content"],
            json!(disclosure.disclosed("an answer")),
            "the ledger carries what the channel saw"
        );

        // The repeated resolution reads the receipt and changes nothing.
        let mut reread = stored;
        let again = disclosure
            .deliverable_answer(&store, conversation, &mut reread, index)
            .await
            .expect("the repeated resolution reads");
        assert_eq!(
            again,
            disclosure.disclosed("an answer"),
            "one line, never two"
        );
    }
}
