//! Principal resolution: the seam between a sender's platform identity and
//! the principal id the ledger stores.
//!
//! Identity rows live in the `principals` table, keyed by principal id,
//! scoped to one adapter. The entry point creates the principal on first
//! contact and refreshes the display fields on later messages; erasure
//! deletes a principal's row through this module's own delete. Nothing here
//! ever writes into the ledger.

use agent_ledger::StoreError;
use agent_ledger::store::{StoreTx, domain_run};
use rusqlite::OptionalExtension;

use crate::message::SenderIdentity;
use crate::schema::DOMAIN;

/// Resolve or create the principal for a sender on one adapter, refreshing
/// the display fields when — and only when — they changed. Returns the
/// principal id.
///
/// The read and the conditional write run in one store operation, so first
/// contact and refresh cannot race each other: the store serializes
/// operations on its one connection, and the unique key on (adapter,
/// external id) makes the insert an update when the principal already
/// exists. A message that changes nothing writes nothing.
///
/// # Errors
///
/// [`StoreError`] if the write fails or the store's actor has stopped.
pub(crate) async fn resolve_principal(
    tx: &StoreTx,
    adapter: String,
    sender: SenderIdentity,
) -> Result<i64, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let existing: Option<(i64, String, Option<String>)> = conn
            .query_row(
                "SELECT id, display_name, username FROM principals
                 WHERE adapter = ?1 AND external_id = ?2",
                rusqlite::params![adapter, sender.external_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((id, display_name, username)) = existing
            && display_name == sender.display_name
            && username == sender.username
        {
            return Ok(id);
        }
        let id = conn.query_row(
            "INSERT INTO principals (adapter, external_id, display_name, username)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (adapter, external_id) DO UPDATE
             SET display_name = excluded.display_name, username = excluded.username
             RETURNING id",
            rusqlite::params![
                adapter,
                sender.external_id,
                sender.display_name,
                sender.username
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    })
    .await
}

/// Whether an identity row exists for this principal id — what erasure asks
/// before it starts, so an unknown principal is reported instead of run
/// through the steps idly.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn exists(tx: &StoreTx, principal_id: i64) -> Result<bool, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let found = conn
            .query_row(
                "SELECT 1 FROM principals WHERE id = ?1",
                [principal_id],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    })
    .await
}

/// Delete a principal's identity rows. Deleting an already-absent principal
/// deletes nothing, so the operation is idempotent. The freed id is never
/// reissued — the table's key is AUTOINCREMENT for exactly that reason.
///
/// # Errors
///
/// [`StoreError`] if the delete fails or the store's actor has stopped.
pub(crate) async fn delete(tx: &StoreTx, principal_id: i64) -> Result<(), StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        conn.execute("DELETE FROM principals WHERE id = ?1", [principal_id])?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use agent_ledger::Store;

    use super::*;
    use crate::schema::store_config;

    fn sender(external_id: &str, display_name: &str, username: Option<&str>) -> SenderIdentity {
        SenderIdentity {
            external_id: external_id.into(),
            display_name: display_name.into(),
            username: username.map(Into::into),
        }
    }

    #[tokio::test]
    async fn a_principal_is_created_once_and_refreshed_after() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let first = resolve_principal(&tx, "a".into(), sender("42", "Ada", None))
            .await
            .unwrap();
        // An unchanged sender takes the no-write path and still resolves.
        let unchanged = resolve_principal(&tx, "a".into(), sender("42", "Ada", None))
            .await
            .unwrap();
        assert_eq!(first, unchanged, "one sender, one principal");
        let second = resolve_principal(&tx, "a".into(), sender("42", "Ada L.", Some("ada")))
            .await
            .unwrap();
        assert_eq!(first, second, "one sender, one principal");

        let (display_name, username): (String, Option<String>) = domain_run(&tx, DOMAIN, {
            move |conn| {
                let row = conn.query_row(
                    "SELECT display_name, username FROM principals WHERE id = ?1",
                    [first],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                Ok(row)
            }
        })
        .await
        .unwrap();
        assert_eq!(display_name, "Ada L.");
        assert_eq!(username.as_deref(), Some("ada"));
    }

    #[tokio::test]
    async fn the_same_external_id_on_two_adapters_is_two_principals() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let on_a = resolve_principal(&tx, "a".into(), sender("42", "Ada", None))
            .await
            .unwrap();
        let on_b = resolve_principal(&tx, "b".into(), sender("42", "Ada", None))
            .await
            .unwrap();
        assert_ne!(on_a, on_b);
    }

    #[tokio::test]
    async fn a_deleted_principal_stops_existing_and_deletes_idempotently() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let id = resolve_principal(&tx, "a".into(), sender("42", "Ada", None))
            .await
            .unwrap();
        assert!(exists(&tx, id).await.unwrap());
        delete(&tx, id).await.unwrap();
        assert!(!exists(&tx, id).await.unwrap());
        delete(&tx, id).await.unwrap();
    }
}
