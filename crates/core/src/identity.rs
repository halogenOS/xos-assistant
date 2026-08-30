//! Principal resolution: the seam between a sender's platform identity and
//! the principal id the ledger stores.
//!
//! Identity rows live in the `principals` table, keyed by principal id,
//! scoped to one adapter. The entry point creates the principal on first
//! contact and refreshes the username on later messages; erasure concludes
//! on a principal's row through this module's own conclusion. Nothing here
//! ever writes into the ledger. The display name is not stored (decision
//! 0077): it was written on every refresh and read by nothing, and its
//! column is dropped by the schema's appended step.
//!
//! Since the privacy-self-service unit (2026-08-23) the row also carries
//! the suppression flag — the one lawful remnant of an opt-out, the
//! suppression-list shape: storing the identifier is what honoring the
//! objection takes. The flag is adapter-scoped like the identity it hangs
//! on. Beside the resolving lookup there is a READ-ONLY one,
//! [`find_standing`], which writes nothing: the entry point consults it
//! before any write, and records an opted-out person's exempt command
//! through it so the username stays frozen — after a deletion, no command
//! re-materializes the emptied field.

use agent_ledger::StoreError;
use agent_ledger::store::{StoreTx, domain_run};
use rusqlite::OptionalExtension;

use crate::message::SenderIdentity;
use crate::schema::DOMAIN;

/// The suppression flag's column: `INTEGER NOT NULL DEFAULT 0`, a boolean
/// by the schema's own precedent, added by the appended suppression step.
/// From the moment it stands, the person's inbound messages are dropped at
/// ingestion; erasure leaves the flag standing and empties the row around
/// it.
pub(crate) const COLUMN_OPTED_OUT: &str = "opted_out";

/// One sender's stored standing, as the read-only lookup answers it: the
/// principal id the identity tables hold, and whether the suppression flag
/// stands on it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Standing {
    /// The stored principal id.
    pub principal_id: i64,
    /// Whether the suppression flag stands.
    pub opted_out: bool,
}

/// Resolve or create the principal for a sender on one adapter, refreshing
/// the username when — and only when — it changed. Returns the principal id.
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
        let existing: Option<(i64, Option<String>)> = conn
            .query_row(
                "SELECT id, username FROM principals
                 WHERE adapter = ?1 AND external_id = ?2",
                rusqlite::params![adapter, sender.external_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, username)) = existing
            && username == sender.username
        {
            return Ok(id);
        }
        let id = conn.query_row(
            "INSERT INTO principals (adapter, external_id, username)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (adapter, external_id) DO UPDATE
             SET username = excluded.username
             RETURNING id",
            rusqlite::params![adapter, sender.external_id, sender.username],
            |row| row.get(0),
        )?;
        Ok(id)
    })
    .await
}

/// The stored standing of a sender on one adapter, read without writing —
/// the read-only lookup beside the resolving one (decided 2026-08-23): the
/// entry point's suppression check runs before every write the ingestion
/// path can make, and an exempted command resolves its principal through
/// this so the username is never refreshed.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn find_standing(
    tx: &StoreTx,
    adapter: String,
    external_id: String,
) -> Result<Option<Standing>, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let found = conn
            .query_row(
                &format!(
                    "SELECT id, {COLUMN_OPTED_OUT} FROM principals
                     WHERE adapter = ?1 AND external_id = ?2"
                ),
                rusqlite::params![adapter, external_id],
                |row| {
                    Ok(Standing {
                        principal_id: row.get(0)?,
                        opted_out: row.get::<_, i64>(1)? != 0,
                    })
                },
            )
            .optional()?;
        Ok(found)
    })
    .await
}

/// Raise the suppression flag on one principal's row, touching nothing
/// else — no username refresh rides along. Returns whether the write changed
/// the row: `false` says the flag already stood (or the row is gone), which
/// is the already-so answer. The check and the write are one statement, so
/// two racing opt-outs report one change between them.
///
/// # Errors
///
/// [`StoreError`] if the write fails or the store's actor has stopped.
pub(crate) async fn set_opt_out(tx: &StoreTx, principal_id: i64) -> Result<bool, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let changed = conn.execute(
            &format!(
                "UPDATE principals SET {COLUMN_OPTED_OUT} = 1
                 WHERE id = ?1 AND {COLUMN_OPTED_OUT} = 0"
            ),
            [principal_id],
        )?;
        Ok(changed > 0)
    })
    .await
}

/// Clear the suppression flag on one principal's row. Returns whether the
/// write changed the row: `false` says no flag stood, the already-so
/// answer. Nothing that was deleted comes back — this touches one column.
///
/// # Errors
///
/// [`StoreError`] if the write fails or the store's actor has stopped.
pub(crate) async fn clear_opt_out(tx: &StoreTx, principal_id: i64) -> Result<bool, StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let changed = conn.execute(
            &format!(
                "UPDATE principals SET {COLUMN_OPTED_OUT} = 0
                 WHERE id = ?1 AND {COLUMN_OPTED_OUT} = 1"
            ),
            [principal_id],
        )?;
        Ok(changed > 0)
    })
    .await
}

/// Whether an identity row exists for this principal id. Two callers ask
/// it: erasure before it starts, so an unknown principal is reported
/// instead of run through the steps idly, and the privacy tool before it
/// acts, so an erased row's stored principal id declines instead of
/// raising a flag no lookup would ever find.
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

/// Conclude an erasure on a principal's identity row (the conditional of
/// 2026-08-23): a row whose suppression flag stands is EMPTIED — the
/// username to its typed absence — so the flag survives its own person's
/// deletion and collection stays stopped; an unflagged row is deleted
/// whole, as it always was. Emptying an already-empty stub and
/// deleting an already-absent row both change nothing, so the step is
/// idempotent — for a flagged person a repeat erasure re-runs over
/// emptiness and reports completion. The freed id is never reissued — the
/// table's key is AUTOINCREMENT for exactly that reason.
///
/// # Errors
///
/// [`StoreError`] if a write fails or the store's actor has stopped.
pub(crate) async fn conclude_erasure(tx: &StoreTx, principal_id: i64) -> Result<(), StoreError> {
    domain_run(tx, DOMAIN, move |conn| {
        let emptied = conn.execute(
            &format!(
                "UPDATE principals SET username = NULL
                 WHERE id = ?1 AND {COLUMN_OPTED_OUT} = 1"
            ),
            [principal_id],
        )?;
        if emptied == 0 {
            conn.execute(
                &format!("DELETE FROM principals WHERE id = ?1 AND {COLUMN_OPTED_OUT} = 0"),
                [principal_id],
            )?;
        }
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use agent_ledger::Store;

    use super::*;
    use crate::schema::store_config;

    fn sender(external_id: &str, username: Option<&str>) -> SenderIdentity {
        SenderIdentity {
            external_id: external_id.into(),
            username: username.map(Into::into),
            bot: false,
        }
    }

    #[tokio::test]
    async fn a_principal_is_created_once_and_refreshed_after() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let first = resolve_principal(&tx, "a".into(), sender("42", None))
            .await
            .unwrap();
        // An unchanged sender takes the no-write path and still resolves.
        let unchanged = resolve_principal(&tx, "a".into(), sender("42", None))
            .await
            .unwrap();
        assert_eq!(first, unchanged, "one sender, one principal");
        let second = resolve_principal(&tx, "a".into(), sender("42", Some("ada")))
            .await
            .unwrap();
        assert_eq!(first, second, "one sender, one principal");

        let username: Option<String> = domain_run(&tx, DOMAIN, {
            move |conn| {
                let row = conn.query_row(
                    "SELECT username FROM principals WHERE id = ?1",
                    [first],
                    |row| row.get(0),
                )?;
                Ok(row)
            }
        })
        .await
        .unwrap();
        assert_eq!(username.as_deref(), Some("ada"));
    }

    /// Decision 0077's row-shape pin: a recorded principal's row holds the
    /// adapter scope, the external id, the username and the suppression
    /// flag — no display name, not as a column and not smuggled into
    /// another. The principals table IS the identity store, so its whole
    /// column list is the claim.
    #[tokio::test]
    async fn a_principal_row_stores_no_display_name() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();
        resolve_principal(&tx, "a".into(), sender("42", Some("ada")))
            .await
            .unwrap();

        let columns: Vec<String> = domain_run(&tx, DOMAIN, |conn| {
            let mut statement = conn.prepare("SELECT name FROM pragma_table_info('principals')")?;
            let names = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(names)
        })
        .await
        .unwrap();
        assert_eq!(
            columns,
            vec!["id", "adapter", "external_id", "username", COLUMN_OPTED_OUT],
            "the identity row is exactly the fields resolution and suppression use"
        );
    }

    #[tokio::test]
    async fn the_same_external_id_on_two_adapters_is_two_principals() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let on_a = resolve_principal(&tx, "a".into(), sender("42", None))
            .await
            .unwrap();
        let on_b = resolve_principal(&tx, "b".into(), sender("42", None))
            .await
            .unwrap();
        assert_ne!(on_a, on_b);
    }

    #[tokio::test]
    async fn an_unflagged_principals_erasure_deletes_the_row_idempotently() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let id = resolve_principal(&tx, "a".into(), sender("42", None))
            .await
            .unwrap();
        assert!(exists(&tx, id).await.unwrap());
        conclude_erasure(&tx, id).await.unwrap();
        assert!(!exists(&tx, id).await.unwrap());
        conclude_erasure(&tx, id).await.unwrap();
    }

    /// The read-only lookup answers the stored standing without writing,
    /// and the flag writes report change-or-already-so through their one
    /// statement.
    #[tokio::test]
    async fn the_standing_reads_and_the_flag_writes_report_honestly() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        assert!(
            find_standing(&tx, "a".into(), "42".into())
                .await
                .unwrap()
                .is_none(),
            "an unknown sender has no standing and none is created"
        );
        let id = resolve_principal(&tx, "a".into(), sender("42", Some("ada")))
            .await
            .unwrap();
        let standing = find_standing(&tx, "a".into(), "42".into())
            .await
            .unwrap()
            .expect("the resolved sender has a standing");
        assert_eq!(standing.principal_id, id);
        assert!(!standing.opted_out, "a fresh principal carries no flag");

        assert!(set_opt_out(&tx, id).await.unwrap(), "the first set changes");
        assert!(
            !set_opt_out(&tx, id).await.unwrap(),
            "the second set is the already-so answer"
        );
        assert!(
            find_standing(&tx, "a".into(), "42".into())
                .await
                .unwrap()
                .expect("the row stands")
                .opted_out,
            "the standing reads the raised flag back"
        );
        assert!(clear_opt_out(&tx, id).await.unwrap());
        assert!(
            !clear_opt_out(&tx, id).await.unwrap(),
            "clearing a clear flag is the already-so answer"
        );
    }

    /// The stub-keeping conclusion: a flagged row survives its erasure
    /// emptied — the flag standing, the username gone — and a repeat
    /// re-runs over emptiness instead of reporting not-found.
    #[tokio::test]
    async fn a_flagged_principals_erasure_keeps_the_emptied_stub() {
        let store = Store::in_memory_with(store_config()).unwrap();
        let tx = store.tx();

        let id = resolve_principal(&tx, "a".into(), sender("42", Some("ada")))
            .await
            .unwrap();
        set_opt_out(&tx, id).await.unwrap();
        conclude_erasure(&tx, id).await.unwrap();
        assert!(exists(&tx, id).await.unwrap(), "the stub survives");
        let (username, opted_out): (Option<String>, i64) = domain_run(&tx, DOMAIN, move |conn| {
            Ok(conn.query_row(
                &format!(
                    "SELECT username, {COLUMN_OPTED_OUT}
                         FROM principals WHERE id = ?1"
                ),
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?)
        })
        .await
        .unwrap();
        assert_eq!(username, None, "the username empties to its typed absence");
        assert_eq!(opted_out, 1, "the flag survives its own person's deletion");
        conclude_erasure(&tx, id).await.unwrap();
        assert!(
            exists(&tx, id).await.unwrap(),
            "a repeat re-runs over emptiness and keeps the stub"
        );
    }
}
