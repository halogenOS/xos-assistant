//! Erasure: remove a person from stored data, in one call, per decision 0012.
//!
//! Three idempotent steps, composed from the modules that own the touched
//! tables:
//!
//! 1. The personal columns of the principal's messages — text, origin
//!    reference and platform send time — are nulled in every conversation:
//!    the kind's own write on its content table, which is the separate
//!    personal-data table of decision 0003. Block header rows are never
//!    touched; positions, references and conversation order keep their
//!    shape, and an erased message projects nothing to the model.
//! 2. The principal's direct conversations are removed entirely — a
//!    two-party chat that lost its human is metadata that still identifies
//!    the person. Each one is unmapped through the mapping module first,
//!    since the channel key is the personal identifier, then deleted through
//!    the framework's conversation deletion; the orphaned blocks are
//!    collected afterwards. The affected conversations are found by reading
//!    the ledger through the public load path, bounded by the number of
//!    direct channels.
//! 3. The principal's identity rows are deleted, last on purpose: as long as
//!    they exist, a retried erasure still finds the principal and runs the
//!    earlier steps again instead of reporting not-found over remaining
//!    data.
//!
//! The steps are separate store operations, so the caller — the assembly —
//! holds its erasure fence across the whole call; without it an ingestion
//! could record a new message or map a new direct channel for the person
//! between the steps.

use agent_ledger::{FromBlock, Store, StoreError};

use crate::identity;
use crate::kind::{self, AssistantKind};
use crate::mapping;
use crate::message::ChannelKind;

/// What one erasure call reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasureOutcome {
    /// The principal existed; its personal columns are nulled, its direct
    /// conversations and their mappings are removed, its identity rows are
    /// deleted. Carries the removed conversation ids so the caller can drop
    /// its own per-conversation state for them.
    Erased {
        /// The direct conversations that were removed entirely.
        deleted_conversations: Vec<i64>,
    },
    /// No identity row matched the principal id — nothing was touched. Said
    /// plainly instead of succeeding idly, so a caller acting on a wrong id
    /// learns it.
    NotFound,
}

/// Erase one principal per decision 0012. Returns
/// [`ErasureOutcome::NotFound`] — touching nothing — when no identity row
/// matches the principal id; erasing is keyed on identity, and a second call
/// after a completed erasure reports the same.
///
/// # Errors
///
/// [`StoreError`] if a read, a write or a deletion fails, or the store's
/// actor has stopped.
pub(crate) async fn erase_principal(
    store: &Store,
    principal_id: i64,
) -> Result<ErasureOutcome, StoreError> {
    let tx = store.tx();
    if !identity::exists(&tx, principal_id).await? {
        return Ok(ErasureOutcome::NotFound);
    }

    kind::erase_principal_content(&tx, principal_id).await?;

    let deleted = direct_conversations_of(store, principal_id).await?;
    for &conversation_id in &deleted {
        mapping::delete_by_conversation(&tx, conversation_id).await?;
        store.delete_conversation(conversation_id).await?;
    }
    store.gc_orphan_blocks().await?;

    identity::delete(&tx, principal_id).await?;
    Ok(ErasureOutcome::Erased {
        deleted_conversations: deleted,
    })
}

/// The direct conversations that carry this principal's messages, read from
/// the ledger through the public load path. A direct conversation is found
/// through its mapping row; the principal's authorship is a fact on its
/// recorded blocks, which erasure's text nulling preserves.
async fn direct_conversations_of(store: &Store, principal_id: i64) -> Result<Vec<i64>, StoreError> {
    let mut affected = Vec::new();
    for record in mapping::all(&store.tx()).await? {
        if record.kind != ChannelKind::Direct {
            continue;
        }
        let blocks = store.list_blocks(record.conversation_id).await?;
        let carries_principal = blocks.iter().any(|block| {
            matches!(
                AssistantKind::from_block(block),
                AssistantKind::ChatMessage(message)
                    if message.principal_id == Some(principal_id)
            )
        });
        if carries_principal {
            affected.push(record.conversation_id);
        }
    }
    Ok(affected)
}
