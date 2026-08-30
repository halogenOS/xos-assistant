//! Group authorization: the persisted, fail-closed record of which group
//! channels the operator admitted (decided 2026-08-23).
//!
//! The table is written exactly one way — a membership observation whose
//! adder matches the configured operator, judged by the assembly — and read
//! before anything else touches a group channel: a message or an observation
//! for a group with no row here is refused with the withdraw directive,
//! touching nothing. The row is what makes the refusal survive a restart,
//! so the check needs no delivery guarantee: a lost leave call is healed by
//! the group's next contact, which is refused all over again.
//!
//! Existing group mappings at migration time are backfilled as authorized by
//! the schema's appended step — they were admitted under the old regime by
//! the operator's own hand. Direct channels never touch this table.

use agent_ledger::StoreError;
use agent_ledger::store::{StoreTx, domain_run};
use rusqlite::OptionalExtension;

use crate::message::{ChannelKey, SenderIdentity};
use crate::schema::DOMAIN;

/// Whether a membership observation is the operator's own invitation: the
/// adder is named and its external id matches the configured operator for
/// the observing adapter. No operator configured, or no adder named, is
/// nobody's invitation — refused fail-closed, matching the module's
/// absence-is-refusal rule.
pub(crate) fn operator_admits(operator: Option<&str>, adder: Option<&SenderIdentity>) -> bool {
    match (operator, adder) {
        (Some(operator), Some(adder)) => adder.external_id == operator,
        _ => false,
    }
}

/// Record the operator's admission of one group channel. Recording an
/// already-authorized channel changes nothing, so a replayed membership
/// observation re-returns its outcome idempotently.
///
/// # Errors
///
/// [`StoreError`] if the write fails or the store's actor has stopped.
pub(crate) async fn authorize(tx: &StoreTx, key: &ChannelKey) -> Result<(), StoreError> {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    domain_run(tx, DOMAIN, move |conn| {
        conn.execute(
            "INSERT OR IGNORE INTO group_authorizations (adapter, channel) VALUES (?1, ?2)",
            rusqlite::params![adapter, channel],
        )?;
        Ok(())
    })
    .await
}

/// Whether the operator admitted this group channel. Absence is refusal:
/// the callers fail closed on `false`.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn is_authorized(tx: &StoreTx, key: &ChannelKey) -> Result<bool, StoreError> {
    let (adapter, channel) = (key.adapter.clone(), key.channel.clone());
    domain_run(tx, DOMAIN, move |conn| {
        let found = conn
            .query_row(
                "SELECT 1 FROM group_authorizations WHERE adapter = ?1 AND channel = ?2",
                rusqlite::params![adapter, channel],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    })
    .await
}

#[cfg(test)]
mod tests {
    use agent_ledger::Store;

    use super::*;
    use crate::schema::store_config;

    fn key(channel: &str) -> ChannelKey {
        ChannelKey {
            adapter: "a".into(),
            channel: channel.into(),
        }
    }

    #[test]
    fn only_a_named_adder_matching_the_configured_operator_admits() {
        let adder = |external_id: &str| SenderIdentity {
            external_id: external_id.into(),
            username: None,
            bot: false,
        };
        assert!(operator_admits(Some("op-1"), Some(&adder("op-1"))));
        assert!(
            !operator_admits(Some("op-1"), Some(&adder("stranger"))),
            "a foreign adder is nobody's invitation"
        );
        assert!(
            !operator_admits(Some("op-1"), None),
            "an add nobody is named for fails closed"
        );
        assert!(
            !operator_admits(None, Some(&adder("op-1"))),
            "with no operator configured, even the operator's id fails closed"
        );
    }

    #[tokio::test]
    async fn authorization_is_absent_until_written_and_idempotent_after() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let tx = store.tx();
        assert!(!is_authorized(&tx, &key("g1")).await.unwrap());
        authorize(&tx, &key("g1")).await.unwrap();
        assert!(is_authorized(&tx, &key("g1")).await.unwrap());
        authorize(&tx, &key("g1")).await.unwrap();
        assert!(is_authorized(&tx, &key("g1")).await.unwrap());
        assert!(
            !is_authorized(&tx, &key("g2")).await.unwrap(),
            "one channel's admission says nothing about another's"
        );
    }

    #[tokio::test]
    async fn the_same_channel_id_on_two_adapters_is_two_authorizations() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let tx = store.tx();
        authorize(&tx, &key("g1")).await.unwrap();
        let other = ChannelKey {
            adapter: "b".into(),
            channel: "g1".into(),
        };
        assert!(!is_authorized(&tx, &other).await.unwrap());
    }
}
