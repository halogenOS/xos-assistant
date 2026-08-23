//! The channel-to-conversation mapping: the one place a channel key is
//! stored.
//!
//! A channel key maps to exactly one ledger conversation, created on first
//! message, with the channel's kind recorded at creation. The mapping is read
//! in both directions — key to conversation on ingestion, conversation to key
//! on the outbound edge — and erasure removes direct-channel rows through
//! this module's own delete.
//!
//! The kind column's vocabulary is closed by a CHECK constraint, so a stored
//! kind outside it cannot exist through this schema. The readers still refuse
//! to hand such a row onward: a database manipulated past the constraint must
//! fail the read loudly, because every caller here — the erasure scan above
//! all — decides personal-data handling by the kind.

use agent_ledger::StoreError;
use agent_ledger::store::{StoreTx, domain_run};
use rusqlite::OptionalExtension;

use crate::error::CoreError;
use crate::message::{ChannelKey, ChannelKind};
use crate::schema::DOMAIN;

/// One mapping row, as the walkers read it: the adapter whose channel it is,
/// the kind recorded at creation, and the conversation. The channel
/// identifier itself is not carried: the callers that need the full key
/// resolve it per conversation through [`channel_for_conversation`].
#[derive(Debug, Clone)]
pub(crate) struct ChannelRecord {
    /// The adapter the channel belongs to.
    pub adapter: String,
    /// The kind recorded at creation.
    pub kind: ChannelKind,
    /// The conversation the channel maps to.
    pub conversation_id: i64,
}

/// The conversation a channel key maps to and the kind recorded at creation,
/// if the mapping exists.
///
/// # Errors
///
/// [`StoreError`] if the query fails, the stored kind falls outside the
/// closed vocabulary, or the store's actor has stopped.
pub(crate) async fn find(
    tx: &StoreTx,
    key: &ChannelKey,
) -> Result<Option<(i64, ChannelKind)>, StoreError> {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    domain_run(tx, DOMAIN, move |conn| {
        let found: Option<(i64, String)> = conn
            .query_row(
                "SELECT conversation_id, kind FROM channels
                 WHERE adapter = ?1 AND channel = ?2",
                rusqlite::params![adapter, channel],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        found
            .map(|(conversation_id, kind)| Ok((conversation_id, parse_kind(&kind)?)))
            .transpose()
    })
    .await
}

/// Claim a channel key for a freshly created conversation, and return the
/// conversation the key maps to afterwards.
///
/// Two ingestions can race on the same first message; the insert ignores a
/// lost race and the read after it returns the winner, so the caller learns
/// whether its own conversation took the mapping or lost it and must be
/// discarded.
///
/// # Errors
///
/// [`CoreError::ClaimLost`] if the mapping row is gone again by the read
/// back — deleted between the insert and the read — leaving no winner to
/// name. [`CoreError::Store`] if the write fails or the store's actor has
/// stopped.
pub(crate) async fn claim(
    tx: &StoreTx,
    key: &ChannelKey,
    kind: ChannelKind,
    conversation_id: i64,
) -> Result<i64, CoreError> {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    let winner = domain_run(tx, DOMAIN, move |conn| {
        conn.execute(
            "INSERT OR IGNORE INTO channels (adapter, channel, kind, conversation_id)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![adapter, channel, kind.as_str(), conversation_id],
        )?;
        let winner: Option<i64> = conn
            .query_row(
                "SELECT conversation_id FROM channels WHERE adapter = ?1 AND channel = ?2",
                rusqlite::params![adapter, channel],
                |row| row.get(0),
            )
            .optional()?;
        Ok(winner)
    })
    .await?;
    winner.ok_or(CoreError::ClaimLost)
}

/// The kind a conversation's channel was recorded as at creation, if the
/// mapping exists — what the report tool's group-only refusal reads
/// (decided 2026-08-23).
///
/// # Errors
///
/// [`StoreError`] if the query fails, the stored kind falls outside the
/// closed vocabulary, or the store's actor has stopped.
pub(crate) async fn kind_for_conversation(
    tx: &StoreTx,
    conversation_id: i64,
) -> Result<Option<ChannelKind>, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let found: Option<String> = conn
            .query_row(
                "SELECT kind FROM channels WHERE conversation_id = ?1",
                [conversation_id],
                |row| row.get(0),
            )
            .optional()?;
        found.map(|kind| parse_kind(&kind)).transpose()
    })
    .await
}

/// The channel key a conversation maps back to, if the mapping exists.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn channel_for_conversation(
    tx: &StoreTx,
    conversation_id: i64,
) -> Result<Option<ChannelKey>, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let found = conn
            .query_row(
                "SELECT adapter, channel FROM channels WHERE conversation_id = ?1",
                [conversation_id],
                |row| {
                    Ok(ChannelKey {
                        adapter: row.get(0)?,
                        channel: row.get(1)?,
                    })
                },
            )
            .optional()?;
        Ok(found)
    })
    .await
}

/// Every mapping row. The outbound edge seeds and re-reads through this, and
/// erasure walks the direct rows through it.
///
/// # Errors
///
/// [`StoreError`] if the query fails, a stored kind falls outside the closed
/// vocabulary, or the store's actor has stopped.
pub(crate) async fn all(tx: &StoreTx) -> Result<Vec<ChannelRecord>, StoreError> {
    domain_run(tx, DOMAIN, |conn| {
        let mut statement = conn.prepare("SELECT adapter, kind, conversation_id FROM channels")?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(adapter, kind, conversation_id)| {
                Ok(ChannelRecord {
                    adapter,
                    kind: parse_kind(&kind)?,
                    conversation_id,
                })
            })
            .collect()
    })
    .await
}

/// Delete the mapping row of one conversation — erasure's unmapping step for
/// a direct channel, composed with the other steps by the erasure module.
/// Deleting an already-unmapped conversation deletes nothing, so the
/// operation is idempotent.
///
/// # Errors
///
/// [`StoreError`] if the delete fails or the store's actor has stopped.
pub(crate) async fn delete_by_conversation(
    tx: &StoreTx,
    conversation_id: i64,
) -> Result<(), StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        conn.execute(
            "DELETE FROM channels WHERE conversation_id = ?1",
            [conversation_id],
        )?;
        Ok(())
    })
    .await
}

/// A stored kind, or the loud refusal a row outside the closed vocabulary
/// deserves — the CHECK constraint makes one unwritable, so meeting one
/// means the database was manipulated past the schema, and no caller may
/// treat that row as either kind.
fn parse_kind(stored: &str) -> Result<ChannelKind, StoreError> {
    ChannelKind::parse(stored).ok_or_else(|| {
        StoreError::Other(format!(
            "stored channel kind '{stored}' falls outside the closed vocabulary"
        ))
    })
}
