//! Erasure: remove a person from stored data, per decision 0012.
//!
//! Three idempotent steps, composed from the modules that own the touched
//! tables:
//!
//! 1. The personal columns of the principal's messages — text, origin
//!    reference, platform send time, the reply-target reference and the
//!    speaker (both extended 2026-08-23) — are nulled in every
//!    conversation: the kind's own write on its content table, which is
//!    the separate personal-data table of decision 0003. First, though,
//!    the reply-target copies OTHER people's rows hold — a reply stores
//!    the replied-to message's id, the erased person's own identifier —
//!    are nulled by the target-keyed pass (2026-08-23) — with the report
//!    block's stored target beside them since unit 36 (2026-08-29), the
//!    second place a copy of somebody else's platform id is held — which
//!    joins on the very origins the author-keyed pass nulls next, so the
//!    order between the two is
//!    load-bearing. Block header rows are never touched; positions,
//!    references and conversation order keep their shape, and an erased
//!    message projects none of its prose to the model — only the kind's
//!    fixed marker. Beside it, every join notice recording the principal
//!    loses its shown name, its handle, its event origin and its send time
//!    (unit 36, 2026-08-29) — one row per joiner, so a co-joiner of the
//!    same event keeps theirs — and every report block naming the
//!    principal as the reported person loses its target origin
//!    (2026-08-23, narrowing the 0045 lineage), so the report line goes
//!    undeliverable. A report filed against a join event several people
//!    share names no reported person at all, by design, so that last pass
//!    cannot reach it: the target-keyed pass above is what nulls its
//!    filed target, from the same collection of the person's own origins.
//!    Last in the step, every mark block naming the principal as the
//!    marked person loses its target origin too (unit 39, 2026-08-30), so
//!    an unplaced reaction is skipped by the edge; one pass suffices there
//!    because a mark's stored principal is always the marked message's own
//!    author. The reaction already visible in the chat is not withdrawn —
//!    the residual is stated in the records of processing.
//! 2. The principal's direct conversations are removed entirely — a
//!    two-party chat that lost its human is metadata that still identifies
//!    the person. Each one is unmapped through the mapping module first,
//!    since the channel key is the personal identifier, then deleted through
//!    the framework's conversation deletion; the orphaned blocks are
//!    collected afterwards. The affected conversations are found by reading
//!    the ledger through the public load path, bounded by the number of
//!    direct channels.
//! 3. The principal's identity rows are concluded, last on purpose: as long
//!    as they exist, a retried erasure still finds the principal and runs
//!    the earlier steps again instead of reporting not-found over remaining
//!    data. The conclusion carries one conditional (2026-08-23, the
//!    privacy-self-service unit): a row whose suppression flag stands is
//!    EMPTIED instead of deleted — the username gone, the flag surviving
//!    its own person's deletion, so collection stays stopped — while an
//!    unflagged row is deleted whole as before. With the stub surviving,
//!    the documented idempotency refines (recorded on decision 0012): for a
//!    flagged person a repeat erasure re-runs over emptiness and reports
//!    completion rather than not-found — honest, harmless, stated.
//!
//! The operation is split into [`plan`] and [`execute`] on purpose: the
//! plan decides not-found and names the direct conversations, and the caller
//! — the assembly — settles those conversations' open streams between the
//! two calls, so the deletion set and the settle set are one derivation. The
//! steps are separate store operations, so the caller holds its erasure
//! fence across plan, settle and execute alike; the fence is also what lets
//! execute trust the plan — without it an ingestion could record a new
//! message or map a new direct channel for the person between the steps.

use agent_ledger::store::{StoreTx, domain_run};
use agent_ledger::{Block, FromBlock, Store, StoreError};

use crate::identity;
use crate::join;
use crate::kind::{self, AssistantKind};
use crate::lineage;
use crate::mapping;
use crate::message::ChannelKind;
use crate::session::{StrippedHop, StrippedLineage};

/// Where one kind records a person under a platform id — what the
/// target-keyed reply pass joins against to find the copies OTHER people's
/// rows hold of an erased person's own message ids.
///
/// Each kind that records a person beside an origin exports one of these;
/// the composition below names the whole set, so no kind knows another's
/// table and adding a third source is a data change here rather than a
/// second spelling of the join. The names are the kinds' own column
/// constants, never literals.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OriginSource {
    /// The content table the kind owns.
    pub table: &'static str,
    /// The column holding the platform's own id for the record.
    pub origin_column: &'static str,
    /// The column holding the recorded person's principal id.
    pub principal_column: &'static str,
}

/// Every place a person's own platform ids are recorded, in the order they
/// were added: the chat message's origin, and the join notice's event
/// origin (unit 36, 2026-08-29) — members reply to join notices, a welcome
/// reply is ordinary, and a replier's row holds the event's id, so without
/// this second source the residual decisions 0063 and 0085 closed would
/// come back through the join table.
const ORIGIN_SOURCES: &[OriginSource] = &[kind::ORIGIN_SOURCE, join::ORIGIN_SOURCE];

/// Where one kind holds a COPY of a platform id that belongs to another
/// record — the mirror image of [`OriginSource`], which is where a kind
/// records a person's OWN ids.
///
/// A copy is the residual decisions 0063 and 0085 close: the record it
/// names can be erased or deleted, and the copy would keep a verbatim
/// identifier of an erased person that no later pass could reach. Both
/// passes below are keyed differently — by the person whose records are
/// named, and by the one record a deletion removed — and both nullify
/// through the site the owning kind exports, so the join they run is
/// spelled once for every kind that holds such a copy.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReferenceSite {
    /// The content table holding the copy.
    pub table: &'static str,
    /// The column the copy sits in, nullable for exactly this reason.
    pub column: &'static str,
}

/// Null every held copy of a platform id this principal's records carry —
/// the target-keyed pass (2026-08-23; widened to the report block's target
/// by unit 36, 2026-08-29). The match runs through each source's origin
/// column within the same conversation: platform message ids are opaque
/// and unique only per channel, so a bare id match across conversations
/// would null a stranger's copy. The framework-table names it joins carry
/// the deliberate coupling decision 0032 records.
///
/// The caller runs this BEFORE the passes that null the origins it joins
/// on; within one attempt that order guarantees the join still matches,
/// and across retries any attempt that reached the origin-nulling passes
/// had already completed this one. Nulling already-null columns is a
/// no-op, so the step is idempotent.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn null_references_naming(
    tx: &StoreTx,
    site: ReferenceSite,
    principal_id: i64,
    sources: &'static [OriginSource],
) -> Result<(), StoreError> {
    if sources.is_empty() {
        // No source records the person under an origin, so no copy can
        // name them: the pass has nothing to match and says so instead of
        // building a predicate out of nothing.
        return Ok(());
    }
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        let authored = sources
            .iter()
            .map(|source| {
                format!(
                    "EXISTS (\
                       SELECT 1 FROM {source_table} author \
                       JOIN conversation_blocks acb ON acb.block_id = author.block_id \
                       JOIN conversation_blocks rcb ON rcb.block_id = {table}.block_id \
                       WHERE author.{principal} = ?1 \
                       AND author.{origin} = {table}.{column} \
                       AND acb.conversation_id = rcb.conversation_id\
                     )",
                    source_table = source.table,
                    principal = source.principal_column,
                    origin = source.origin_column,
                    table = site.table,
                    column = site.column,
                )
            })
            .collect::<Vec<String>>()
            .join(" OR ");
        conn.execute(
            &format!(
                "UPDATE {table} SET {column} = NULL \
                 WHERE {column} IS NOT NULL AND ({authored})",
                table = site.table,
                column = site.column,
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

/// Null every held copy of ONE origin inside one conversation — the
/// deletion mirror's own reach (decision 0085; widened to the report
/// block's target by unit 36, 2026-08-29). Without it each holder would
/// keep a verbatim copy of the deleted record's identifier that no later
/// erasure could reach, because [`null_references_naming`] joins on the
/// very origins the deletion just nulled.
///
/// Returns how many copies were nulled. Idempotent: nulling already-null
/// columns is a no-op, and a second run finds nothing left naming the
/// origin.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn null_references_to(
    tx: &StoreTx,
    site: ReferenceSite,
    conversation_id: i64,
    origin: &str,
) -> Result<usize, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn.execute(
            &format!(
                "UPDATE {table} SET {column} = NULL \
                 WHERE {column} = ?1 AND EXISTS (\
                   SELECT 1 FROM conversation_blocks cb \
                   WHERE cb.block_id = {table}.block_id \
                   AND cb.conversation_id = ?2\
                 )",
                table = site.table,
                column = site.column,
            ),
            (&origin, conversation_id),
        )?)
    })
    .await
}

/// What one erasure call reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErasureOutcome {
    /// The principal existed; its personal columns are nulled, its direct
    /// conversations and their mappings are removed, its identity rows are
    /// concluded — deleted, or emptied to the suppression stub when the
    /// opt-out flag stands (2026-08-23). Carries the removed conversation
    /// ids so the caller can drop its own per-conversation state for them.
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
/// an unflagged person's completed erasure reports the same, while a
/// flagged person's surviving stub keeps matching, so their repeat re-runs
/// and reports completion (the idempotency refinement of 2026-08-23) —
/// otherwise the plan [`execute`] runs. Reads only; nothing is touched.
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
    // The target-keyed pass runs first: it finds the repliers' rows by
    // joining their stored reply target against the principal's origins,
    // and the author-keyed pass below is about to null those origins.
    // Within one attempt the order guarantees the join still matches, and
    // across retries any attempt that reached the author-keyed pass had
    // already completed this one. The pass reaches exactly what its join
    // matches: a reply whose stored target matches none of the principal's
    // recorded origins — recorded between a failed attempt and its retry,
    // recorded after a completed erasure (the person's next appearance
    // resolves to a new principal), or naming a message never recorded —
    // keeps its copy, unlinked inside the store but stored. Decision
    // 0063's refinements record that residual with its follow-up, a reach
    // key resolved when the reply is recorded.
    kind::erase_reply_targets_naming(&tx, plan.principal_id, ORIGIN_SOURCES).await?;
    // The report block's stored target is the second held copy, nulled by
    // the same collection and for the same reason (unit 36, 2026-08-29): a
    // report filed against a join event several people share attaches NO
    // reported principal, so the principal-keyed pass below cannot reach
    // it, while the filed target is a verbatim copy of the erased joiner's
    // own event id. Ordered with the pass above, ahead of everything that
    // nulls the origins both join on.
    crate::tools::report::erase_report_targets_naming(&tx, plan.principal_id, ORIGIN_SOURCES)
        .await?;
    kind::erase_principal_content(&tx, plan.principal_id).await?;
    // The join notice's own person-keyed pass (unit 36, 2026-08-29): the
    // shown name, the handle, the event origin and the send time of every
    // join recording the person are nulled, one row at a time, so a
    // co-joiner of the same event keeps their own row whole. It runs
    // beside the message kind's for the same reason — the columns are the
    // kind's own contract — and after the target-keyed pass above, which
    // joins on the very origins this one nulls.
    join::erase_principal_joins(&tx, plan.principal_id).await?;
    // The report table's person-keyed pass (2026-08-23, narrowing the 0045
    // lineage): every report naming the person as the reported principal
    // loses its target origin, so the line goes undeliverable and the edge
    // skips it. The block stores that principal id precisely so this pass
    // can reach it. It is not the target-keyed pass above restated: this
    // one reaches a report by WHOM it names, which still matches after the
    // reported record's own origin is gone — a message the deletion mirror
    // already nulled leaves no origin for the collection to join on — while
    // the pass above reaches a report by WHICH record it points at, the
    // only reach a filing that names several people has.
    crate::tools::report::erase_reported_origin(&tx, plan.principal_id).await?;
    // The mark table's person-keyed pass (unit 39, 2026-08-30): every
    // reaction naming the person as the marked one loses its target
    // origin, so the reaction goes unplaceable and the edge skips it. One
    // pass reaches every mark, unlike the report's two — a mark's stored
    // principal IS the author of the marked message, never a third
    // party's, so no filing escapes this key. The chosen emoji stays: it
    // records what the ASSISTANT expressed and names nobody, exactly as
    // the report's line does.
    crate::tools::mark::erase_marked_origin(&tx, plan.principal_id).await?;

    for &conversation_id in &plan.direct_conversations {
        mapping::delete_by_conversation(&tx, conversation_id).await?;
        store.delete_conversation(conversation_id).await?;
    }
    store.gc_orphan_blocks().await?;

    identity::conclude_erasure(&tx, plan.principal_id).await?;
    Ok(ErasureOutcome::Erased {
        deleted_conversations: plan.direct_conversations,
    })
}

/// Every compacted lineage this principal's words reach, and what each
/// conversation in it must lose — the scrub's whole reading (unit 48,
/// 2026-08-31), taken once before anything is written.
///
/// A compacted thread is recognized by the block it opens with: the
/// framework's ancestor reference, whose column names the conversation the
/// thread continues. The digest sitting behind that reference was written
/// from the ancestor's first half, so a principal with blocks in the
/// ancestor is a principal whose words could be in the prose — and a
/// principal with blocks in the thread itself has words the clone must drop
/// even though no digest was written from them.
///
/// The reference chain is walked to its ROOT, never one hop. A thread that
/// has been compacted twice continues a thread that continues a
/// conversation, and the digest each hop carries was written from the half
/// below it — a half that HOLDS the digest below. So a digest two
/// generations up is prose written from prose written from the erased
/// person's words, and a one-hop reading would find nothing to scrub in
/// either of the two newest conversations while exactly that prose kept
/// serving. Nothing forbids the second compaction: `/compact` is a command,
/// and the threshold door re-arms on the successor.
///
/// Only MAPPED conversations start a walk: a thread nothing serves has no
/// channel to hand on and no reader to protect, and its own erasure is the
/// nulling every other conversation gets. Bounded by the number of mapped
/// channels times the depth of their ancestries.
///
/// # Errors
///
/// [`StoreError`] if a read fails or the store's actor has stopped.
pub(crate) async fn compacted_lineages(
    store: &Store,
    principal_id: i64,
) -> Result<Vec<StrippedLineage>, StoreError> {
    let mut found = Vec::new();
    for record in mapping::all(&store.tx()).await? {
        if let Some(lineage) = stripped_lineage(store, record.conversation_id, &|blocks| {
            blocks_of(blocks, principal_id)
        })
        .await?
        {
            found.push(lineage);
        }
    }
    Ok(found)
}

/// One serving thread's whole compacted ancestry with the blocks each
/// conversation in it must lose, oldest conversation first — or `None` when
/// the thread has no ancestry, or when the caller's reading strips nothing
/// anywhere in it.
///
/// Which blocks go is the CALLER's reading, taken once per conversation over
/// that conversation's own ledger: an erased principal's rows, a retracted
/// answer with the quotes derived from it. The walk knows nothing about
/// either, which is what lets one chain-rebuilding mechanism serve both.
///
/// The walk stops at a conversation that continues nothing — the root — and
/// it also stops at a reference whose conversation no longer reads, which is
/// a chain broken below that point: there is no history left to regenerate
/// that hop's digest FROM, so it becomes the root and every digest above it
/// is still rewritten. Conversation ids are reissued after a deletion, so a
/// reference could in principle point back into the chain; the walked set is
/// what keeps that from looping.
///
/// # Errors
///
/// [`StoreError`] if a read fails or the store's actor has stopped.
pub(crate) async fn stripped_lineage(
    store: &Store,
    serving: i64,
    stripped: &(dyn Fn(&[Block]) -> Vec<i64> + Sync),
) -> Result<Option<StrippedLineage>, StoreError> {
    let mut hops = Vec::new();
    let mut walked = std::collections::HashSet::from([serving]);
    let mut current = serving;
    let mut blocks = store.list_blocks(current).await?;
    while let Some(opening) = lineage::own_opening(&blocks) {
        let ancestor_blocks = store.list_blocks(opening.ancestor).await?;
        if ancestor_blocks.is_empty() || !walked.insert(opening.ancestor) {
            tracing::warn!(
                conversation_id = current,
                ancestor = opening.ancestor,
                "a compacted thread's ancestry ends at a conversation that no longer reads; \
                 its own digest stands and every digest above it is regenerated"
            );
            break;
        }
        hops.push(StrippedHop {
            conversation: current,
            opening_ends: opening.opening_ends,
            stripped: stripped(&blocks),
        });
        current = opening.ancestor;
        blocks = ancestor_blocks;
    }
    // Read newest first, so the first hop walked IS the serving thread; the
    // rest are handed over oldest first, because the scrub rebuilds from the
    // root upward, each hop's digest written from the clone below it.
    let mut walked_up = hops.into_iter();
    let Some(serving_hop) = walked_up.next() else {
        return Ok(None);
    };
    let mut below: Vec<StrippedHop> = walked_up.collect();
    below.reverse();
    let in_root = stripped(&blocks);
    if in_root.is_empty()
        && serving_hop.stripped.is_empty()
        && below.iter().all(|hop| hop.stripped.is_empty())
    {
        return Ok(None);
    }
    Ok(Some(StrippedLineage {
        root: current,
        in_root,
        below,
        serving: serving_hop,
    }))
}

/// The blocks in one ledger that record this principal — the two kinds that
/// name a person, read through their own parses rather than a column list
/// here. Erasure nulls what those blocks SAY; the scrub drops the blocks
/// themselves from a clone, so what a regenerated digest is written from
/// holds no trace of them at all.
fn blocks_of(blocks: &[Block], principal_id: i64) -> Vec<i64> {
    blocks
        .iter()
        .filter(|block| match AssistantKind::from_block(block) {
            AssistantKind::ChatMessage(message) => message.principal_id == Some(principal_id),
            AssistantKind::JoinNotice(join) => join.principal_id == Some(principal_id),
            _ => false,
        })
        .map(|block| block.id)
        .collect()
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
