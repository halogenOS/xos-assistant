//! The retention rule: a conversation nobody has touched for the configured
//! span is deleted whole (unit 53, 2026-09-02).
//!
//! One rule, one measurement. A conversation's freshness is its newest
//! ledger entry — the newest block its junction holds — and a conversation
//! whose newest entry is older than the span expires. Every conversation is
//! measured the same way: serving, replaced, ancestor, direct. Activity
//! refreshes the whole conversation, so a group that is still talking never
//! loses history, and a compacted ancestor that stopped growing at its cut
//! goes a span after it while the thread standing on it serves on.
//!
//! A conversation whose junction holds no blocks is never named. Emptiness
//! is a creation state raced by the mapping claim, and sweeping it would
//! delete a conversation mid-birth; the residue a crash can leave in that
//! shape is bounded and already cleaned by the lost-claim and startup paths.
//!
//! # The clock is the store's
//!
//! The comparison runs inside the store, in SQL, against the store's own
//! `datetime('now')`. The application reads no wall clock for it and no
//! clock crate enters the tree — the standing decision of unit 34, checked
//! mechanically by the consumer's clock-source scan.
//!
//! Stored stamps come in two encodings and both are read as instants, never
//! as text. The framework's own writer stamps a block with a local time and
//! its numeric offset; the header's column default is `SQLite`'s UTC
//! `datetime('now')`, which is what a row inserted without a stamp carries.
//! `datetime()` normalizes either one to UTC, so the comparison is between
//! two instants and an offset never shifts a conversation's age. A stamp
//! `datetime()` cannot read at all answers NULL, which fails the comparison
//! and leaves that conversation standing: an unreadable time is never a
//! reason to delete.
//!
//! # What a sweep does
//!
//! Each tick asks the store for the expired conversations and, for each one,
//! unmaps its channel and retires it through the one existing door. After
//! the conversations, one orphan collection; then the identity rows nothing
//! names any more. A tick that finds nothing does nothing at all.
//!
//! The deletion phase runs under the erasure fence, the arbiter erasure and
//! the session resets already hold, so a sweep and a deletion request never
//! interleave. A request never waits for the schedule either: erasure takes
//! the fence on demand and the next tick simply finds less to do.
//!
//! A failure inside one conversation's retirement fails that conversation
//! and nothing else. The sweep logs it and moves on, and the next tick
//! retries it, because the conversation is still expired.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use agent_ledger::store::domain_run;
use agent_ledger::{Store, StoreError};

use crate::erasure;
use crate::session::Sessions;

/// How often the sweep runs. The cadence carries no meaning beyond freshness
/// of enforcement: every tick is idempotent and deletes exactly what the rule
/// names, so an hour is how long a conversation can outlive its span and
/// nothing else.
const SWEEP_INTERVAL: Duration = Duration::from_hours(1);

/// How long a conversation may lie untouched before it expires — the
/// deployment's one retention number.
///
/// The default is the decision; the field exists so a deployment can be told
/// apart from the code, the shape the answering budgets already take.
/// Disabling is the absent span, never a zero: a process configured with no
/// span runs no sweep at all and deletes nothing on any schedule.
#[derive(Debug, Clone, Copy)]
pub struct RetentionConfig {
    /// The span in whole days, absent when the deployment switched the sweep
    /// off.
    pub days: Option<NonZeroU32>,
}

impl RetentionConfig {
    /// The span an unconfigured deployment gets, in days.
    pub const DEFAULT_DAYS: u32 = 90;

    /// The configuration for a stated span, with zero meaning off.
    #[must_use]
    pub const fn of_days(days: u32) -> Self {
        Self {
            days: NonZeroU32::new(days),
        }
    }

    /// The switched-off configuration: no sweep task, nothing ever expires.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { days: None }
    }
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self::of_days(Self::DEFAULT_DAYS)
    }
}

/// Start the sweep beside the compaction driver, or start nothing when the
/// deployment configured no span.
///
/// The first tick is at spawn, and a boot is a tick like any other: the
/// sweep has no catch-up behaviour and no first-run grace, because the rule
/// is the whole mechanism and a tick deletes only what the rule names. The
/// task holds the sessions weakly and ends with the assembly, the compaction
/// driver's own shape.
pub(crate) fn spawn_sweep(sessions: &Arc<Sessions>, retention: RetentionConfig) {
    let Some(days) = retention.days else {
        return;
    };
    let weak = Arc::downgrade(sessions);
    let mut ticks = tokio::time::interval(SWEEP_INTERVAL);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    tokio::spawn(async move {
        loop {
            ticks.tick().await;
            let Some(sessions) = weak.upgrade() else {
                break;
            };
            sweep(&sessions, days).await;
        }
    });
}

/// One tick: retire what expired, collect what nothing holds, and say so in
/// the log.
///
/// Nothing here fails a caller. A sweep is unattended enforcement of a
/// standing rule, and every way it can go wrong leaves the store exactly as
/// it was for the next tick to find.
async fn sweep(sessions: &Sessions, days: NonZeroU32) {
    let store = sessions.context().store();
    let expired = match expired_conversations(store, days).await {
        Ok(expired) => expired,
        Err(error) => {
            tracing::warn!(
                %error,
                "the retention reading failed; the next sweep reads again"
            );
            return;
        }
    };
    if expired.is_empty() {
        return;
    }
    // The deletion phase, and only it, is ordered against an erasure: the
    // reading above touches nothing, and a conversation an erasure removed
    // between the two is simply gone when this tick reaches it.
    let _no_erasure_mid_sweep = sessions.erasure_fence().write().await;
    let mut retired = 0_usize;
    for conversation_id in expired {
        if let Err(error) = sessions.retire_expired(conversation_id).await {
            tracing::warn!(
                conversation_id,
                %error,
                "an expired conversation did not retire; the next sweep finds it expired still"
            );
            continue;
        }
        retired += 1;
    }
    if let Err(error) = store.gc_orphan_blocks().await {
        tracing::warn!(
            retired,
            %error,
            "the orphan collection failed after a sweep; the next sweep collects"
        );
        return;
    }
    match erasure::collect_unnamed_principals(store).await {
        Ok(collected) => tracing::info!(retired, collected, "the retention sweep ran"),
        Err(error) => tracing::warn!(
            retired,
            %error,
            "the principal collection failed after a sweep; the next sweep collects"
        ),
    }
}

/// The conversations whose newest ledger entry is older than the span,
/// lowest id first, read in one query against the store's own clock.
///
/// The junction is what the grouping reads, so a conversation holding no
/// blocks produces no row and is never named. The comparison is between
/// instants: each stamp and the cutoff both go through `datetime()`, which
/// normalizes the framework's offset-carrying stamps and the header
/// default's UTC ones alike.
///
/// The framework table names it joins carry the deliberate coupling decision
/// 0032 records, exactly as the owing-tail walk and the forced-turn-end read
/// already do.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
async fn expired_conversations(store: &Store, days: NonZeroU32) -> Result<Vec<i64>, StoreError> {
    let span = format!("-{days} days");
    domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
        let mut statement = conn.prepare(
            "SELECT cb.conversation_id FROM conversation_blocks cb \
             JOIN blocks b ON b.id = cb.block_id \
             GROUP BY cb.conversation_id \
             HAVING MAX(datetime(b.created_at)) < datetime('now', ?1) \
             ORDER BY cb.conversation_id",
        )?;
        let expired = statement
            .query_map([span], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        Ok(expired)
    })
    .await
}

#[cfg(test)]
mod tests {
    use agent_ledger::providers::ReasoningLevel;
    use agent_ledger::{CoreEvent, EventBus, ProviderRegistry, Role, RuntimeContext, ToolRegistry};
    use tokio::sync::{Mutex, RwLock};

    use super::*;
    use crate::assembly::ModelBinding;
    use crate::compaction::ContextWatch;
    use crate::identity;
    use crate::join::{self, JoinNotice, RecordedJoiner};
    use crate::kind::{self, ChatMessage, RecordedOrigin, RecordedSender, Stamp, Summons};
    use crate::message::{Authority, ChannelKey, ChannelKind, SenderIdentity};
    use crate::schema::{DOMAIN, store_config};
    use crate::session::SessionCoordination;
    use crate::{mapping, streams};

    /// A sessions object over an in-memory store with nothing registered:
    /// no provider, no tool, no reactor. Every block in the ledger is one
    /// the test wrote, so what the sweep reads is stored state and nothing
    /// else.
    fn quiet_sessions() -> (Arc<Sessions>, Store) {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let bus: Arc<EventBus<CoreEvent>> = Arc::new(EventBus::new());
        let ctx = RuntimeContext::new(
            store.clone(),
            Arc::clone(&bus),
            Arc::new(ProviderRegistry::new()),
            Arc::new(ToolRegistry::new()),
        );
        let context = Arc::new(ContextWatch::new(streams::spawn_observer(&bus), None));
        let sessions = Sessions::new(
            ctx,
            ModelBinding {
                provider_instance: "p".into(),
                provider_display_name: "P".into(),
                vendor: "v".into(),
                model: "m".into(),
                model_display_name: "M".into(),
                context_window: None,
            },
            ReasoningLevel::Low,
            "the system prompt".into(),
            Vec::new(),
            SessionCoordination {
                stamp_lock: Arc::new(Mutex::new(())),
                erasure_fence: Arc::new(RwLock::new(())),
                context,
            },
        );
        (Arc::new(sessions), store)
    }

    /// The span every test measures against.
    const SPAN: NonZeroU32 = NonZeroU32::new(90).expect("the test span is nonzero");

    /// A conversation with one stored block, answering its id.
    async fn conversation_with_a_block(store: &Store) -> i64 {
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        store
            .insert_final_text_block(conversation, Role::User, "a stored line".into(), None)
            .await
            .expect("the line stores");
        conversation
    }

    /// One member message appended through the consumer write path, so the
    /// row naming its principal is the production shape.
    async fn message_from(store: &Store, conversation: i64, principal_id: i64) -> i64 {
        store
            .append_consumer_block(
                conversation,
                Some(Role::User),
                kind::CHAT_MESSAGE_KIND,
                ChatMessage::stored_fields(
                    "a member's line",
                    RecordedSender {
                        principal_id,
                        authority: Authority::Member,
                        speaker: None,
                    },
                    RecordedOrigin::default(),
                    None,
                    "2026-09-02T00:00:00Z",
                    Stamp::compose(
                        Summons {
                            summoned: true,
                            literal_addressed: true,
                        },
                        Authority::Member,
                        None,
                        None,
                    ),
                ),
                None,
            )
            .await
            .expect("the message appends")
    }

    /// One resolved identity row on the test adapter.
    async fn person(store: &Store, external_id: &str) -> i64 {
        identity::resolve_principal(
            &store.tx(),
            "a".into(),
            SenderIdentity {
                external_id: external_id.into(),
                username: Some("someone".into()),
                bot: false,
            },
        )
        .await
        .expect("the principal resolves")
    }

    /// One join notice recording a person, the second family that names a
    /// principal without any message of theirs behind it.
    async fn join_notice_for(store: &Store, conversation: i64, principal_id: i64) {
        store
            .append_consumer_block(
                conversation,
                None,
                join::JOIN_NOTICE_KIND,
                JoinNotice::stored_fields(
                    RecordedJoiner {
                        principal_id,
                        name: "A Newcomer",
                        handle: Some("newcomer"),
                    },
                    "org-join",
                    "2026-09-02T00:00:00Z",
                ),
                None,
            )
            .await
            .expect("the join notice appends");
    }

    /// Age every block of one conversation by whole days, keeping the
    /// framework's own stamp encoding exactly: only the leading calendar
    /// date is moved, and the time of day and the stored offset behind it
    /// are the row's own, so what the reading meets is the shape production
    /// writes.
    async fn age(store: &Store, conversation: i64, days: i64) {
        domain_run(&store.tx(), DOMAIN, move |conn| {
            conn.execute(
                "UPDATE blocks SET created_at = \
                 date(substr(created_at, 1, 10), ?2) || substr(created_at, 11) \
                 WHERE id IN (\
                   SELECT block_id FROM conversation_blocks WHERE conversation_id = ?1\
                 )",
                rusqlite::params![conversation, format!("-{days} days")],
            )?;
            Ok(())
        })
        .await
        .expect("the blocks age");
    }

    /// Restamp every block of one conversation in the header column's own
    /// default encoding — `SQLite`'s UTC `datetime('now')` form, which is what
    /// a row inserted without a stamp carries.
    async fn restamp_utc(store: &Store, conversation: i64, days: i64) {
        domain_run(&store.tx(), DOMAIN, move |conn| {
            conn.execute(
                "UPDATE blocks SET created_at = datetime('now', ?2) \
                 WHERE id IN (\
                   SELECT block_id FROM conversation_blocks WHERE conversation_id = ?1\
                 )",
                rusqlite::params![conversation, format!("-{days} days")],
            )?;
            Ok(())
        })
        .await
        .expect("the blocks restamp");
    }

    /// The conversations the store still holds, lowest id first.
    async fn surviving(store: &Store) -> Vec<i64> {
        let mut ids: Vec<i64> = store
            .list_conversations()
            .await
            .expect("the conversation list reads")
            .into_iter()
            .map(|conversation| conversation.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// The identity rows the store still holds, lowest id first.
    async fn principals(store: &Store) -> Vec<i64> {
        domain_run(&store.tx(), DOMAIN, |conn| {
            let mut statement = conn.prepare("SELECT id FROM principals ORDER BY id")?;
            let ids = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?;
            Ok(ids)
        })
        .await
        .expect("the identity rows read")
    }

    /// How many junction rows one conversation still holds.
    async fn junction_rows(store: &Store, conversation: i64) -> i64 {
        domain_run(&store.tx(), DOMAIN, move |conn| {
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM conversation_blocks WHERE conversation_id = ?1",
                [conversation],
                |row| row.get(0),
            )?)
        })
        .await
        .expect("the junction reads")
    }

    /// A channel key on the suite's adapter.
    fn channel(id: &str) -> ChannelKey {
        ChannelKey {
            adapter: "a".into(),
            channel: id.into(),
        }
    }

    /// AC2: the reading names the conversation whose newest entry is past
    /// the span, in either stored stamp encoding, and never names a
    /// conversation whose junction holds nothing.
    #[tokio::test]
    async fn the_reading_names_the_stale_conversations_and_no_empty_one() {
        let store = Store::in_memory_with(store_config()).expect("an in-memory store opens");
        let stale = conversation_with_a_block(&store).await;
        let stale_in_utc = conversation_with_a_block(&store).await;
        let fresh = conversation_with_a_block(&store).await;
        let empty = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        age(&store, stale, 91).await;
        restamp_utc(&store, stale_in_utc, 91).await;
        age(&store, fresh, 1).await;

        assert_eq!(
            expired_conversations(&store, SPAN)
                .await
                .expect("the reading answers"),
            vec![stale, stale_in_utc],
            "both stamp encodings read as instants, and the fresh conversation is inside the span"
        );
        assert!(
            !expired_conversations(&store, SPAN)
                .await
                .expect("the reading answers")
                .contains(&empty),
            "a conversation whose junction holds no blocks is never named"
        );
    }

    /// AC4: one entry inside the span keeps the whole conversation,
    /// however old its oldest entry is. Freshness is the NEWEST entry, and
    /// a living conversation never loses its history to the schedule.
    #[tokio::test]
    async fn one_fresh_entry_keeps_the_whole_conversation() {
        let (sessions, store) = quiet_sessions();
        let conversation = conversation_with_a_block(&store).await;
        age(&store, conversation, 91).await;
        store
            .insert_final_text_block(conversation, Role::User, "spoken today".into(), None)
            .await
            .expect("the fresh line stores");

        sweep(&sessions, SPAN).await;

        assert_eq!(
            surviving(&store).await,
            vec![conversation],
            "the conversation stands on its newest entry"
        );
        assert_eq!(
            junction_rows(&store, conversation).await,
            2,
            "the aged entry stands with it: the rule expires conversations, never messages"
        );
    }

    /// AC3: an expired conversation goes through the one door — the mapping
    /// row first, then settle, delete and forget — and leaves neither
    /// junction rows nor a channel pointing at it.
    #[tokio::test]
    async fn an_expired_conversation_is_retired_with_its_mapping() {
        let (sessions, store) = quiet_sessions();
        let conversation = conversation_with_a_block(&store).await;
        let room = channel("room-quiet");
        mapping::claim(&store.tx(), &room, ChannelKind::Group, conversation)
            .await
            .expect("the channel claims the conversation");
        age(&store, conversation, 91).await;

        sweep(&sessions, SPAN).await;

        assert!(
            surviving(&store).await.is_empty(),
            "the expired conversation is gone"
        );
        assert_eq!(
            junction_rows(&store, conversation).await,
            0,
            "its junction rows went with it"
        );
        assert!(
            mapping::find(&store.tx(), &room)
                .await
                .expect("the mapping reads")
                .is_none(),
            "the channel maps to nothing, so its next message opens a fresh session"
        );
    }

    /// AC9: a store whose every conversation is inside the span sweeps
    /// nothing — no conversation, no mapping, no identity row. This is the
    /// mechanical half of the first-activation requirement: a boot is a
    /// tick like any other, and a tick deletes only what the rule names.
    #[tokio::test]
    async fn a_store_inside_the_span_sweeps_nothing() {
        let (sessions, store) = quiet_sessions();
        let conversation = conversation_with_a_block(&store).await;
        let room = channel("room-busy");
        mapping::claim(&store.tx(), &room, ChannelKind::Group, conversation)
            .await
            .expect("the channel claims the conversation");
        let speaker = person(&store, "42").await;
        message_from(&store, conversation, speaker).await;
        age(&store, conversation, 89).await;

        sweep(&sessions, SPAN).await;

        assert_eq!(
            surviving(&store).await,
            vec![conversation],
            "nothing is deleted"
        );
        assert!(
            mapping::find(&store.tx(), &room)
                .await
                .expect("the mapping reads")
                .is_some(),
            "no mapping changes"
        );
        assert_eq!(
            principals(&store).await,
            vec![speaker],
            "no identity row is collected"
        );
    }

    /// AC6: after the conversations and the orphan collection, the sweep
    /// takes every unflagged identity row nothing names any more — and
    /// keeps the one a join notice in a surviving conversation names, and
    /// the flagged one whatever the rows say.
    #[tokio::test]
    async fn the_collection_takes_the_unnamed_and_keeps_the_named_and_the_flagged() {
        let (sessions, store) = quiet_sessions();
        let quiet = conversation_with_a_block(&store).await;
        let living = conversation_with_a_block(&store).await;
        let gone = person(&store, "1").await;
        let joined = person(&store, "2").await;
        let flagged = person(&store, "3").await;
        message_from(&store, quiet, gone).await;
        message_from(&store, quiet, flagged).await;
        // The kept person's only trace is a join notice in the conversation
        // that survives: no message of theirs is anywhere.
        join_notice_for(&store, living, joined).await;
        identity::set_opt_out(&store.tx(), flagged)
            .await
            .expect("the flag rises");
        age(&store, quiet, 91).await;

        sweep(&sessions, SPAN).await;

        assert_eq!(
            surviving(&store).await,
            vec![living],
            "only the quiet conversation goes"
        );
        assert_eq!(
            principals(&store).await,
            vec![joined, flagged],
            "the unnamed row is collected, the join notice keeps one and the flag keeps the other"
        );
    }

    /// AC8: a store failure inside one conversation's retirement fails that
    /// conversation alone. The second expired conversation is swept, the
    /// first stands, and the tick after the failure clears takes it.
    #[tokio::test]
    async fn a_failed_retirement_leaves_its_conversation_for_the_next_sweep() {
        let (sessions, store) = quiet_sessions();
        let obstructed = conversation_with_a_block(&store).await;
        let other = conversation_with_a_block(&store).await;
        age(&store, obstructed, 91).await;
        age(&store, other, 91).await;
        // The injected fault: the first conversation's junction refuses to
        // be deleted, which is a store error raised from inside the
        // framework's own deletion.
        domain_run(&store.tx(), DOMAIN, move |conn| {
            conn.execute_batch(&format!(
                "CREATE TRIGGER obstruct_retirement \
                 BEFORE DELETE ON conversation_blocks \
                 WHEN OLD.conversation_id = {obstructed} \
                 BEGIN SELECT RAISE(ABORT, 'injected deletion failure'); END;"
            ))?;
            Ok(())
        })
        .await
        .expect("the trigger installs");

        sweep(&sessions, SPAN).await;

        assert_eq!(
            surviving(&store).await,
            vec![obstructed],
            "the failure stopped one conversation's deletion and nothing else"
        );

        domain_run(&store.tx(), DOMAIN, |conn| {
            conn.execute_batch("DROP TRIGGER obstruct_retirement;")?;
            Ok(())
        })
        .await
        .expect("the trigger drops");
        sweep(&sessions, SPAN).await;

        assert!(
            surviving(&store).await.is_empty(),
            "the conversation is still expired, so the next sweep takes it"
        );
    }

    /// AC10: the deletion phase runs under the erasure fence. A holder of
    /// the fence keeps the tick's deletions waiting, and they run when the
    /// hold is released.
    #[tokio::test]
    async fn the_deletions_wait_for_the_erasure_fence() {
        let (sessions, store) = quiet_sessions();
        let conversation = conversation_with_a_block(&store).await;
        age(&store, conversation, 91).await;

        let held = sessions.erasure_fence().read().await;
        let ticking = tokio::spawn({
            let sessions = Arc::clone(&sessions);
            async move { sweep(&sessions, SPAN).await }
        });
        // Long enough that a sweep taking no fence would have finished: the
        // whole tick is three store operations on an in-memory database.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            surviving(&store).await,
            vec![conversation],
            "the tick's deletions wait for the fence"
        );

        drop(held);
        ticking.await.expect("the tick finishes");
        assert!(
            surviving(&store).await.is_empty(),
            "the released fence lets the tick delete"
        );
    }

    /// AC1's core half: the stated default is ninety days, and a
    /// configuration with no span spawns no task — a store past the span
    /// keeps everything under it.
    #[tokio::test]
    async fn a_span_of_zero_disables_the_sweep_and_spawns_no_task() {
        assert_eq!(
            RetentionConfig::default().days.map(NonZeroU32::get),
            Some(RetentionConfig::DEFAULT_DAYS),
            "the unconfigured span is the stated default"
        );
        assert_eq!(
            RetentionConfig::of_days(30).days.map(NonZeroU32::get),
            Some(30),
            "a stated span is the span"
        );
        assert!(
            RetentionConfig::of_days(0).days.is_none(),
            "zero is how a deployment switches the sweep off"
        );

        let (sessions, store) = quiet_sessions();
        let conversation = conversation_with_a_block(&store).await;
        age(&store, conversation, 400).await;

        let alive = tokio::runtime::Handle::current()
            .metrics()
            .num_alive_tasks();
        spawn_sweep(&sessions, RetentionConfig::of_days(0));
        assert_eq!(
            tokio::runtime::Handle::current()
                .metrics()
                .num_alive_tasks(),
            alive,
            "a disabled span spawns no task"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            surviving(&store).await,
            vec![conversation],
            "and nothing expires under it, however old"
        );

        spawn_sweep(&sessions, RetentionConfig::default());
        // The first tick is at spawn, so the conversation goes without any
        // wait for the hour.
        for _ in 0..200 {
            if surviving(&store).await.is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the spawned sweep's first tick never took the expired conversation");
    }
}
