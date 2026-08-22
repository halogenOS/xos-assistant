//! Erasure: remove a person from stored data, per decision 0012.
//!
//! Three idempotent steps, composed from the modules that own the touched
//! tables:
//!
//! 1. The personal columns of the principal's messages — text, origin
//!    reference and platform send time — are nulled in every conversation:
//!    the kind's own write on its content table, which is the separate
//!    personal-data table of decision 0003. Block header rows are never
//!    touched; positions, references and conversation order keep their
//!    shape, and an erased message projects none of its prose to the model
//!    — only the kind's fixed marker.
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
//! The operation is split into [`plan`] and [`execute`] on purpose: the
//! plan decides not-found and names the direct conversations, and the caller
//! — the assembly — settles those conversations' open streams between the
//! two calls, so the deletion set and the settle set are one derivation. The
//! steps are separate store operations, so the caller holds its erasure
//! fence across plan, settle and execute alike; the fence is also what lets
//! execute trust the plan — without it an ingestion could record a new
//! message or map a new direct channel for the person between the steps.

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

/// One erasure's prepared plan: the principal exists, and these are the
/// direct conversations [`execute`] will remove — the same set whose open
/// streams the caller settles between [`plan`] and [`execute`], so the
/// settle set cannot drift from the deletion set.
pub(crate) struct ErasurePlan {
    principal_id: i64,
    direct_conversations: Vec<i64>,
}

impl ErasurePlan {
    /// The direct conversations the execute step will remove entirely.
    pub(crate) fn direct_conversations(&self) -> &[i64] {
        &self.direct_conversations
    }
}

/// Decide one erasure per decision 0012: `None` when no identity row matches
/// the principal id — erasing is keyed on identity, and a second call after
/// a completed erasure reports the same — otherwise the plan [`execute`]
/// runs. Reads only; nothing is touched.
///
/// # Errors
///
/// [`StoreError`] if a read fails or the store's actor has stopped.
pub(crate) async fn plan(
    store: &Store,
    principal_id: i64,
) -> Result<Option<ErasurePlan>, StoreError> {
    if !identity::exists(&store.tx(), principal_id).await? {
        return Ok(None);
    }
    Ok(Some(ErasurePlan {
        principal_id,
        direct_conversations: direct_conversations_of(store, principal_id).await?,
    }))
}

/// Run one planned erasure's three steps. Trusting the plan is the caller's
/// fence at work: it holds the fence exclusively from [`plan`] through this
/// call, so nothing about the principal changed in between.
///
/// # Errors
///
/// [`StoreError`] if a write or a deletion fails, or the store's actor has
/// stopped.
pub(crate) async fn execute(
    store: &Store,
    plan: ErasurePlan,
) -> Result<ErasureOutcome, StoreError> {
    let tx = store.tx();
    kind::erase_principal_content(&tx, plan.principal_id).await?;

    for &conversation_id in &plan.direct_conversations {
        mapping::delete_by_conversation(&tx, conversation_id).await?;
        store.delete_conversation(conversation_id).await?;
    }
    store.gc_orphan_blocks().await?;

    identity::delete(&tx, plan.principal_id).await?;
    Ok(ErasureOutcome::Erased {
        deleted_conversations: plan.direct_conversations,
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
