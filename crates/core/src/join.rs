//! The join notice: the consumer block kind that records one person
//! entering a group, as the platform announced them (unit 36, 2026-08-29).
//!
//! A join is a platform fact like a title or a pinned announcement, so it
//! rides the observation surface and is stored behind that surface's
//! existing authorization gate. It is not a context note: a note's text is
//! group governance and lies beyond every erasure pass (decision 0055),
//! while a join carries a PERSON — the shown name, the handle, the
//! principal — so it takes a table of its own with the chat message's
//! erasure discipline.
//!
//! One block per joiner, all of one service message's joiners under that
//! message's one origin: the person-keyed pass nulls one joiner's row and
//! leaves a co-joiner's standing, which a single multi-person row could
//! never do. The name column is nullable for exactly one reason — erasure
//! nulls it — so a stored row without a name is an erased join and projects
//! nothing at all. An empty stored name is a different fact: the platform
//! showed no name, nothing was invented in its place, and the projected
//! line falls back to the handle.
//!
//! The kind is agency-inert and frontier-transparent, and the assembly's
//! owing-tail walk reads through it: a join is appended by an independent
//! path at an arbitrary moment, so a member's unanswered question behind
//! one still owes its turn. A join summons nothing by itself — it simply
//! carries no summons — and is seen when a turn composes over a window
//! holding it.

use agent_ledger::store::{StoreError, StoreTx, domain_run};
use agent_ledger::{
    Agency, Block, Column, ColumnType, ContentDescriptor, ContentPart, LeafKind, Projection, Role,
};
use serde_json::{Value, json};

use crate::erasure::OriginSource;
use crate::kind::{Envelope, enveloped, storable_speaker};

/// The stored type string of the join-notice kind.
pub const JOIN_NOTICE_KIND: &str = "join_notice";

/// The content table the kind's descriptor owns.
pub const JOIN_NOTICE_TABLE: &str = "block_join_notice";

/// The name the platform displayed for the joiner — the event's own
/// content, the way a message's text is its content. Nullable: NULL is the
/// one legal absence and means erased, so an erased join projects nothing.
/// An empty string is the other stored meaning: the platform showed no
/// name, and the line falls back to the handle.
pub const COLUMN_NAME: &str = "name";
/// The joiner's public handle at the moment they joined, where the platform
/// had one — the same historically honest value decision 0065 records for a
/// speaker, under the same storable bound. Personal data: the person-keyed
/// pass nulls it beside the name.
pub const COLUMN_HANDLE: &str = "handle";
/// The joiner's principal id in the identity tables, resolved through the
/// same path a sender's is — a joiner is a member. The person-keyed erasure
/// pass keys on it.
pub const COLUMN_PRINCIPAL_ID: &str = "principal_id";
/// The service message's own id, opaque and shared by every joiner of one
/// event: what the projection marks the line with and what a report names.
/// Personal data under the person-keyed null, like the message origin it
/// mirrors.
pub const COLUMN_ORIGIN: &str = "origin";
/// When the platform says the service message was sent, RFC 3339. The block
/// header's own creation time is the store's, so the ledger keeps both.
pub const COLUMN_JOINED_AT: &str = "joined_at";

/// What the model reads ahead of a join notice's shown name — the
/// platform-fact voice of the context note's leads: the ledger states what
/// the platform announced, in the system voice, and nothing more.
pub const JOIN_NOTICE_LEAD: &str = "A member joined the group: ";

/// The whole projected statement of a join whose platform showed neither a
/// name nor a handle: the fact stands and no identifier is invented to
/// stand in for the person (decision 0056's rule, restated for this kind).
pub const UNNAMED_JOINER_LINE: &str = "A member joined the group.";

/// The joiner as one write records them: the resolved principal, the shown
/// name and the public handle — three facts that enter the row together,
/// carried as one value so the append's field map cannot take them apart.
#[derive(Debug, Clone, Copy)]
pub struct RecordedJoiner<'a> {
    /// The resolved principal id.
    pub principal_id: i64,
    /// The name the platform displayed, empty where it displayed none.
    pub name: &'a str,
    /// The joiner's public handle; `None` stores NULL, and a handle
    /// outside [`storable_speaker`]'s bound stores NULL the same way.
    pub handle: Option<&'a str>,
}

/// One stored join notice. Absences are typed per the kind contract: the
/// absent name is erasure's own mark, and everything else absent is a row
/// the store did not produce.
#[derive(Debug, Clone)]
pub struct JoinNotice {
    /// The shown name. `None` is an erased join — the row projects nothing
    /// — while `Some("")` is a joiner the platform showed no name for.
    pub name: Option<String>,
    /// The joiner's public handle. `None` three ways: the platform gave
    /// none, the handle fell outside the storable bound, or erasure nulled
    /// it.
    pub handle: Option<String>,
    /// The joiner's principal id. `None` only for a row the store did not
    /// produce (the schema stores it NOT NULL).
    pub principal_id: Option<i64>,
    /// The service message's origin, shared by the event's joiners. `None`
    /// after erasure and for a row the store did not produce.
    pub origin: Option<String>,
    /// The platform's send time for the service message, RFC 3339.
    pub joined_at: Option<String>,
}

impl JoinNotice {
    /// The stored shape of one join notice: the field map the observation
    /// append carries, named by the same columns [`LeafKind::parse`] reads
    /// back. An empty name is stored as the empty string on purpose — the
    /// absence of the column is erasure's meaning, and the two must not
    /// collide.
    #[must_use]
    pub fn stored_fields(
        joiner: RecordedJoiner<'_>,
        origin: &str,
        joined_at: &str,
    ) -> serde_json::Map<String, Value> {
        let mut fields = serde_json::Map::new();
        fields.insert(COLUMN_NAME.into(), json!(joiner.name));
        if let Some(handle) = joiner.handle.filter(|handle| storable_speaker(handle)) {
            fields.insert(COLUMN_HANDLE.into(), json!(handle));
        }
        fields.insert(COLUMN_PRINCIPAL_ID.into(), json!(joiner.principal_id));
        fields.insert(COLUMN_ORIGIN.into(), json!(origin));
        fields.insert(COLUMN_JOINED_AT.into(), json!(joined_at));
        fields
    }

    /// The system line this join projects, `None` for an erased row and for
    /// a row the store did not produce — an erased join says nothing at
    /// all, not even a placeholder, because the fact it stated was about
    /// the person it no longer names.
    ///
    /// A live row carries the same envelope a live message does (unit 55,
    /// 2026-09-02): the joiner's handle as its author, the platform's own
    /// send time for the service message, and the event's id — the one the
    /// report tool and the reply tool both name it by, so a join without an
    /// envelope would be visible to the model and unnameable by it. Under
    /// the envelope stands the platform-fact statement of the shown name,
    /// with the handle beside it where one is stored; an absent name falls
    /// back to the handle alone, and a joiner with neither reads as the
    /// unnamed entry.
    fn line(&self) -> Option<String> {
        let name = self.name.as_deref()?;
        let statement = match (name.is_empty(), self.handle.as_deref()) {
            (false, Some(handle)) => format!("{JOIN_NOTICE_LEAD}{name} (@{handle})"),
            (false, None) => format!("{JOIN_NOTICE_LEAD}{name}"),
            (true, Some(handle)) => format!("{JOIN_NOTICE_LEAD}@{handle}"),
            (true, None) => UNNAMED_JOINER_LINE.to_owned(),
        };
        Some(enveloped(
            Envelope {
                from: self.handle.as_deref(),
                date: self.joined_at.as_deref(),
                msgid: self.origin.as_deref(),
                edited: false,
            },
            &statement,
        ))
    }
}

impl LeafKind for JoinNotice {
    const KINDS: &'static [&'static str] = &[JOIN_NOTICE_KIND];

    const DESCRIPTORS: &'static [ContentDescriptor] = &[ContentDescriptor {
        table: JOIN_NOTICE_TABLE,
        domain: crate::schema::DOMAIN,
        kinds: &[JOIN_NOTICE_KIND],
        columns: &[
            Column::new(COLUMN_NAME, ColumnType::Text),
            Column::new(COLUMN_HANDLE, ColumnType::Text),
            Column::new(COLUMN_PRINCIPAL_ID, ColumnType::Integer),
            Column::new(COLUMN_ORIGIN, ColumnType::Text),
            Column::new(COLUMN_JOINED_AT, ColumnType::Text),
        ],
        reference_columns: &[],
        quoted_text_column: None,
        ephemeral: false,
    }];

    fn parse(block: &Block) -> Self {
        Self {
            name: string_field(block, COLUMN_NAME),
            handle: string_field(block, COLUMN_HANDLE),
            principal_id: block
                .fields
                .get(COLUMN_PRINCIPAL_ID)
                .and_then(Value::as_i64),
            origin: string_field(block, COLUMN_ORIGIN),
            joined_at: string_field(block, COLUMN_JOINED_AT),
        }
    }
}

/// Agency-inert, and frontier-transparent on purpose — the context note's
/// twin properties, for the same reason: a join is appended by an
/// independent path at an arbitrary moment, so the owed-turn frontier must
/// read through it. A join over an unanswered message buries nothing, and
/// a join by itself owes nothing.
impl Agency for JoinNotice {
    fn frontier_transparent(&self) -> bool {
        true
    }
}

/// A system-voiced line, like the context note's: the ledger states a
/// platform fact in its own voice, and providers join system lines instead
/// of overwriting, so a join never displaces the system prompt. An erased
/// or malformed row is boundary-invisible and contributes nothing.
impl Projection for JoinNotice {
    fn group_role(&self) -> Option<Role> {
        self.line().map(|_| Role::System)
    }

    fn llm_parts(&self) -> Option<Vec<ContentPart>> {
        Some(vec![ContentPart::Text { text: self.line()? }])
    }

    fn llm_text(&self) -> Option<String> {
        self.line()
    }
}

/// This kind's own recorded platform ids, for the target-keyed reply
/// pass: a member replying to a join notice — a welcome is ordinary —
/// stores the event's origin, so an erased joiner's events must be
/// reachable from that pass exactly as their messages are. The erasure
/// composition names both sources; neither kind knows the other's table.
pub(crate) const ORIGIN_SOURCE: OriginSource = OriginSource {
    reference: PRINCIPAL_REFERENCE,
    origin_column: COLUMN_ORIGIN,
};

/// Where this kind records the person a notice is about: the table it owns
/// and the column holding their principal id, named once for every
/// person-wide reach the consumer runs.
pub(crate) const PRINCIPAL_REFERENCE: crate::erasure::PrincipalReference =
    crate::erasure::PrincipalReference {
        table: JOIN_NOTICE_TABLE,
        principal_column: COLUMN_PRINCIPAL_ID,
    };

/// The row's personal columns — the shown name, the handle, the event
/// origin and the platform send time. The principal id is deliberately
/// absent: it is the key the person-keyed pass matches on and the message
/// kind retains it through both of its own passes, so an erased row still
/// answers whose it was without naming them.
const PERSONAL_COLUMNS: [&str; 4] = [COLUMN_NAME, COLUMN_HANDLE, COLUMN_ORIGIN, COLUMN_JOINED_AT];

/// The SET clause that empties [`PERSONAL_COLUMNS`], spelled once for both
/// writers: the person-keyed pass and the origin-keyed one null exactly the
/// same columns, and a column added to the row must not have to be
/// remembered twice.
fn null_personal_columns() -> String {
    PERSONAL_COLUMNS
        .iter()
        .map(|column| format!("{column} = NULL"))
        .collect::<Vec<String>>()
        .join(", ")
}

/// Null the personal columns — the shown name, the handle, the event
/// origin and the platform send time — of every join notice recording this
/// principal, in every conversation: the person-keyed pass, owned by the
/// kind because the nullable columns are the kind's own contract. The block
/// header rows are never touched; an affected row keeps its shape, projects
/// nothing, and names nobody. Nulling already-null columns is a no-op, so
/// the step is idempotent, and a co-joiner's own row is untouched — which
/// is exactly why one event lands one row per joiner.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_principal_joins(
    tx: &StoreTx,
    principal_id: i64,
) -> Result<(), StoreError> {
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        conn.execute(
            &format!(
                "UPDATE {JOIN_NOTICE_TABLE} SET {nulls} WHERE {COLUMN_PRINCIPAL_ID} = ?1",
                nulls = null_personal_columns(),
            ),
            [principal_id],
        )?;
        Ok(())
    })
    .await
}

/// Null the WHOLE event a deletion mirror names: every joiner's row under
/// that origin, inside the one conversation. Deleting the service message
/// removes the event, not one person's part of it, so the origin-keyed pass
/// reaches all of its rows — the person-keyed pass above is what reaches
/// exactly one.
///
/// Platform message ids are opaque and unique only per channel, so the
/// match runs through the conversation junction and never reaches a
/// stranger conversation's row; the framework-table name it joins carries
/// the deliberate coupling decision 0032 records. Returns how many rows
/// were nulled — zero for an origin this table never held. Idempotent: an
/// already-nulled row's origin is NULL and matches nothing.
///
/// # Errors
///
/// [`StoreError`] if the update fails or the store's actor has stopped.
pub(crate) async fn erase_event_named(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<usize, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn.execute(
            &format!(
                "UPDATE {JOIN_NOTICE_TABLE} SET {nulls} \
                 WHERE {COLUMN_ORIGIN} = ?1 AND EXISTS (\
                   SELECT 1 FROM conversation_blocks cb \
                   WHERE cb.block_id = {JOIN_NOTICE_TABLE}.block_id \
                   AND cb.conversation_id = ?2\
                 )",
                nulls = null_personal_columns(),
            ),
            (&origin, conversation_id),
        )?)
    })
    .await
}

/// Whether this conversation already holds a stored notice of the event —
/// the redelivery check (unit 36, 2026-08-29). Both transports promise
/// at-least-once delivery, so the same join service message arrives again
/// after a failed acknowledgment; the event's origin is the platform's own
/// id for it, shared by every joiner, so one stored row under that origin
/// says the whole event is recorded and a redelivery must store nothing.
///
/// An erased event holds no origin, so it matches nothing here — the same
/// reading every origin-keyed pass in this module makes, and the erasure
/// fence the observation holds is what keeps a live erasure from moving
/// the answer under the caller.
///
/// # Errors
///
/// [`StoreError`] if the read fails or the store's actor has stopped.
pub(crate) async fn event_recorded(
    tx: &StoreTx,
    conversation_id: i64,
    origin: &str,
) -> Result<bool, StoreError> {
    let origin = origin.to_owned();
    domain_run(tx, crate::schema::DOMAIN, move |conn| {
        Ok(conn.query_row(
            &format!(
                "SELECT EXISTS (\
                   SELECT 1 FROM {JOIN_NOTICE_TABLE} \
                   JOIN conversation_blocks cb \
                     ON cb.block_id = {JOIN_NOTICE_TABLE}.block_id \
                   WHERE {JOIN_NOTICE_TABLE}.{COLUMN_ORIGIN} = ?1 \
                   AND cb.conversation_id = ?2\
                 )"
            ),
            (&origin, conversation_id),
            |row| row.get::<_, i64>(0),
        )? == 1)
    })
    .await
}

/// A text field read by column name from a loaded block's fields, absent
/// when the row holds no value — a NULL column never surfaces as a field,
/// and this keeps it from surfacing as an invented empty string.
fn string_field(block: &Block, name: &str) -> Option<String> {
    block
        .fields
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join_block(fields: serde_json::Map<String, Value>) -> Block {
        Block {
            id: 1,
            role: None,
            block_type: JOIN_NOTICE_KIND.into(),
            created_at: String::new(),
            dispatch_anchor: None,
            fields,
        }
    }

    /// One parsed join notice under the given shown name and handle.
    fn notice(name: &str, handle: Option<&str>) -> JoinNotice {
        JoinNotice::parse(&join_block(JoinNotice::stored_fields(
            RecordedJoiner {
                principal_id: 9,
                name,
                handle,
            },
            "origin-join-1",
            "2026-08-29T00:00:00Z",
        )))
    }

    #[test]
    fn the_stored_fields_round_trip_through_the_parse() {
        let join = notice("Ada Lovelace", Some("ada"));
        assert_eq!(join.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(join.handle.as_deref(), Some("ada"));
        assert_eq!(join.principal_id, Some(9));
        assert_eq!(join.origin.as_deref(), Some("origin-join-1"));
        assert_eq!(join.joined_at.as_deref(), Some("2026-08-29T00:00:00Z"));
    }

    /// AC8's join half (unit 55): the same envelope a message carries —
    /// the joiner's handle, the platform's send time, the event's id — over
    /// the platform-fact statement, with the handle beside the name where
    /// one exists, the handle alone where the platform showed no name, and
    /// the unnamed entry where it showed neither.
    #[test]
    fn the_line_marks_the_event_and_falls_back_from_name_to_handle() {
        assert_eq!(
            notice("Ada Lovelace", Some("ada")).llm_text().as_deref(),
            Some(
                "---\nfrom: @ada\ndate: 2026-08-29T00:00:00Z\nmsgid: origin-join-1\n---\n\
                 A member joined the group: Ada Lovelace (@ada)"
            )
        );
        assert_eq!(
            notice("Ada Lovelace", None).llm_text().as_deref(),
            Some(
                "---\ndate: 2026-08-29T00:00:00Z\nmsgid: origin-join-1\n---\n\
                 A member joined the group: Ada Lovelace"
            ),
            "a joiner with no stored handle declares no author"
        );
        assert_eq!(
            notice("", Some("ada")).llm_text().as_deref(),
            Some(
                "---\nfrom: @ada\ndate: 2026-08-29T00:00:00Z\nmsgid: origin-join-1\n---\n\
                 A member joined the group: @ada"
            ),
            "an absent name falls back to the handle"
        );
        assert_eq!(
            notice("", None).llm_text().as_deref(),
            Some(
                "---\ndate: 2026-08-29T00:00:00Z\nmsgid: origin-join-1\n---\n\
                 A member joined the group."
            ),
            "neither name nor handle invents no identifier"
        );
    }

    /// A handle outside the storable bound stores NULL, exactly like no
    /// handle at all — the projected prefix's one spelling of the bound.
    #[test]
    fn an_unstorable_handle_stores_nothing() {
        let join = notice("Ada", Some("two words"));
        assert_eq!(join.handle, None);
        assert_eq!(
            join.llm_text().as_deref(),
            Some(
                "---\ndate: 2026-08-29T00:00:00Z\nmsgid: origin-join-1\n---\n\
                 A member joined the group: Ada"
            )
        );
    }

    #[test]
    fn a_join_is_inert_transparent_and_system_voiced() {
        let join = notice("Ada", Some("ada"));
        assert_eq!(join.awaiting(), None, "a join summons nothing");
        assert!(join.frontier_transparent(), "the frontier reads through it");
        assert!(join.durable(), "a join is a durable ledger row");
        assert_eq!(join.group_role(), Some(Role::System));
    }

    /// The SET clause both erasure passes write: every personal column
    /// nulled, and the principal id — the key the person-keyed pass matches
    /// on, retained exactly as the message kind retains its own — left
    /// alone. One recording, so the two passes cannot drift apart.
    #[test]
    fn the_null_list_names_every_personal_column_and_no_key() {
        let clause = null_personal_columns();
        assert_eq!(
            PERSONAL_COLUMNS.len(),
            4,
            "the public policy counts the four things a deletion removes from a join \
             record, so a fifth column moves the policy too"
        );
        for column in PERSONAL_COLUMNS {
            assert!(
                clause.contains(&format!("{column} = NULL")),
                "the erasure passes empty {column}"
            );
        }
        assert!(
            !clause.contains(COLUMN_PRINCIPAL_ID),
            "the principal id is the key, not personal content: {clause}"
        );
    }

    /// An erased join — the name nulled by the person-keyed pass — projects
    /// nothing at all: no line, no role, no placeholder.
    #[test]
    fn an_erased_join_projects_nothing() {
        let erased = JoinNotice::parse(&join_block(serde_json::Map::new()));
        assert_eq!(erased.group_role(), None);
        assert_eq!(erased.llm_text(), None);
        assert_eq!(erased.llm_parts(), None);
    }
}
