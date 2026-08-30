//! The assistant's own storage schema: the content table the block kind
//! declares, and the two domain tables that hold what never enters the ledger.
//!
//! The identity table holds personal data on its own separate footing, so
//! erasure deletes rows here and touches no block header. The content table's
//! `text` column is the other personal-data surface (decision 0012): nullable,
//! nulled by erasure, NOT NULL everywhere else it matters. The channel table
//! is the one place a channel key is stored.
//!
//! Table and column names come from the modules that own them — the kind
//! module for the content table, this module for the domain tables — and the
//! CHECK constraints quote the closed vocabularies straight from their enums,
//! so neither the names nor the vocabularies exist twice.

use std::sync::LazyLock;

use agent_ledger::{DomainMigrations, FromBlock, StoreConfig};

use crate::kind::{
    AssistantKind, CHAT_MESSAGE_TABLE, COLUMN_ADDRESSED, COLUMN_ANSWER_DUE, COLUMN_AUTHORITY,
    COLUMN_DEBT_AUTHORITY, COLUMN_LIMITED, COLUMN_LITERAL_ADDRESSED, COLUMN_ORIGIN,
    COLUMN_PRINCIPAL_ID, COLUMN_REPLY_TARGET, COLUMN_REPLY_TO_ASSISTANT, COLUMN_ROLE,
    COLUMN_SENT_AT, COLUMN_SPEAKER, COLUMN_TEXT,
};
use crate::message::{Authority, ChannelKind};

/// The domain the assistant's tables live under.
///
/// Every read and write of these tables goes through
/// [`agent_ledger::store::domain_run`] with this name, sharing the store's
/// single writer.
pub const DOMAIN: &str = "assistant";

/// A closed vocabulary as a CHECK constraint's quoted list: `'a','b','c'`.
fn quoted_list<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values
        .map(|value| format!("'{value}'"))
        .collect::<Vec<_>>()
        .join(",")
}

/// The block kind's content table, in the shape its descriptor declares.
/// `text` is nullable on purpose: NULL means erased (decision 0012), and the
/// kind reads it back as the typed absence. Everything identifying the
/// message's provenance is NOT NULL, with the authority vocabulary closed by
/// the same enum the code parses with. The two addressing columns are
/// structure, not personal data, stamped at the write and left by erasure.
///
/// This CREATE TABLE received its last in-place edit with the live-model
/// unit, which shipped the first deployable process; every schema change
/// from here on is an appended, versioned migration step.
static CHAT_MESSAGE_SCHEMA: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {CHAT_MESSAGE_TABLE} (
            block_id             INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {COLUMN_ROLE}         TEXT,
            {COLUMN_TEXT}         TEXT,
            {COLUMN_PRINCIPAL_ID} INTEGER NOT NULL,
            {COLUMN_AUTHORITY}    TEXT NOT NULL CHECK ({COLUMN_AUTHORITY} IN ({authorities})),
            {COLUMN_ORIGIN}       TEXT,
            {COLUMN_SENT_AT}      TEXT,
            {COLUMN_ADDRESSED}    INTEGER NOT NULL CHECK ({COLUMN_ADDRESSED} IN (0, 1)),
            {COLUMN_ANSWER_DUE}   INTEGER NOT NULL CHECK ({COLUMN_ANSWER_DUE} IN (0, 1))
        );",
        authorities = quoted_list(Authority::ALL.iter().map(|a| a.as_str())),
    )
});

/// Sender identity, keyed by principal id — the only place personal identity
/// lives. A principal is scoped to one adapter: the same external id on two
/// adapters is two people until proven otherwise. The `display_name` column
/// this shipped CREATE still names is dropped by the appended retirement
/// step below (decision 0077); the CREATE itself stays frozen as it shipped.
///
/// The id column is AUTOINCREMENT because erasure hard-deletes rows here
/// while ledger blocks keep their principal id forever: a bare rowid key
/// would reissue the newest erased id to the next new sender, and the erased
/// person's retained blocks would then resolve to a living stranger.
const PRINCIPALS_SCHEMA: &str = "
    CREATE TABLE principals (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        adapter      TEXT NOT NULL,
        external_id  TEXT NOT NULL,
        display_name TEXT NOT NULL,
        username     TEXT,
        UNIQUE (adapter, external_id)
    );";

/// The channel-to-conversation mapping, with the channel's kind recorded at
/// creation and its vocabulary closed by the same enum the code parses with.
/// `conversation_id` is unique because the mapping is read both ways: a
/// channel key finds its conversation on ingestion, a conversation finds its
/// channel key on the outbound edge.
static CHANNELS_SCHEMA: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE channels (
            adapter         TEXT NOT NULL,
            channel         TEXT NOT NULL,
            kind            TEXT NOT NULL CHECK (kind IN ({kinds})),
            conversation_id INTEGER NOT NULL UNIQUE,
            PRIMARY KEY (adapter, channel)
        );",
        kinds = quoted_list(ChannelKind::ALL.iter().map(|k| k.as_str())),
    )
});

/// The principal count's index, named once: the appended migration step
/// below creates it, and the suite's schema pins read it back under this
/// name — three call sites, one spelling.
pub static PRINCIPAL_ADDRESSED_INDEX: LazyLock<String> =
    LazyLock::new(|| format!("idx_{CHAT_MESSAGE_TABLE}_principal_addressed"));

// ─── The appended steps' frozen vocabularies ─────────────────────────────
//
// Every appended migration step quotes vocabulary lists frozen at the
// moment the step shipped, never the live enums: an applied step's
// generated SQL must stay byte-identical under the appended-steps
// discipline, and a step quoting a live enum would silently diverge fresh
// stores from upgraded ones the moment the enum grows its next variant.
// The live vocabularies stay on the enums; a future widening is a NEW
// appended step quoting its own frozen list. The tests at the bottom pin
// each newest frozen list to its enum, so growing an enum fails loudly
// right here — the failure is the reminder to append the widening step.

/// The limited vocabulary the protection unit shipped; the widening step
/// further down is where the stored constraint caught up with the command
/// kind.
const SHIPPED_PROTECTION_LIMITS: [&str; 2] = ["principal", "channel"];

/// The authority vocabulary as it stood when the protection and widening
/// steps shipped.
const SHIPPED_AUTHORITIES: [&str; 3] = ["member", "moderator", "admin"];

/// The note-topic vocabulary as it stood when the context-note step
/// shipped.
const SHIPPED_NOTE_TOPICS: [&str; 2] = ["title", "rules"];

/// The group channel kind's stored encoding as it stood when the
/// authorization step shipped — what the backfill selects by.
const SHIPPED_GROUP_KIND: &str = "group";

/// The limited vocabulary the widening step widened TO — the full list as
/// it stood at the group-context unit, the command kind included.
const WIDENED_COMMAND_LIMITS: [&str; 3] = ["principal", "channel", "command"];

/// The protection stamp — the first appended migration step, per decision
/// 0026's discipline: the shipped CREATE TABLE above stays as it was written
/// and every schema change from the live-model unit on is a new entry here.
/// The framework counts entry `i` of the domain's list as version `i + 1`,
/// so a store created before this step holds version 3 and runs every
/// appended step from here on at open, while a fresh store runs the whole
/// list in order.
///
/// The step adds the two protection columns — both nullable, so every
/// pre-existing row reads NULL in both, with their vocabularies closed by
/// the enums as they stood when the step shipped — and the index the
/// principal budget count runs on. Both columns are structure, not personal
/// data: erasure leaves them.
static PROTECTION_STAMP_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "ALTER TABLE {CHAT_MESSAGE_TABLE}
             ADD COLUMN {COLUMN_LIMITED} TEXT CHECK ({COLUMN_LIMITED} IN ({limits}));
         ALTER TABLE {CHAT_MESSAGE_TABLE}
             ADD COLUMN {COLUMN_DEBT_AUTHORITY} TEXT
                 CHECK ({COLUMN_DEBT_AUTHORITY} IN ({authorities}));
         CREATE INDEX {index}
             ON {CHAT_MESSAGE_TABLE}({COLUMN_PRINCIPAL_ID}, {COLUMN_ADDRESSED});",
        index = PRINCIPAL_ADDRESSED_INDEX.as_str(),
        limits = quoted_list(SHIPPED_PROTECTION_LIMITS.iter().copied()),
        authorities = quoted_list(SHIPPED_AUTHORITIES.iter().copied()),
    )
});

/// The tool palette's content table — the appended migration step of the
/// tools unit, per decision 0026's discipline. The table shape is the
/// palette kind's descriptor contract: the block header row is the ledger
/// entry, this row carries the admitted-names list. Structure, not personal
/// data: erasure leaves it, and a direct conversation's deletion removes it
/// through the block cascade like every content row.
static TOOL_PALETTE_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {table} (
            block_id INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {tools}  TEXT NOT NULL
        );",
        table = crate::tools::palette::TOOL_PALETTE_TABLE,
        tools = crate::tools::palette::COLUMN_TOOLS,
    )
});

/// The context note's content table — an appended migration step of the
/// group-context unit, per decision 0026's discipline. The table shape is
/// the note kind's descriptor contract: the block header row is the ledger
/// entry, this row carries the topic and the observed text, with the topic
/// vocabulary closed by the same enum the code parses with. The text is
/// the group's own published governance, not a person's conversation;
/// erasure's boundary here is recorded OPEN in its own decision.
static CONTEXT_NOTE_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {table} (
            block_id INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {topic}  TEXT NOT NULL CHECK ({topic} IN ({topics})),
            {text}   TEXT NOT NULL
        );",
        table = crate::note::CONTEXT_NOTE_TABLE,
        topic = crate::note::COLUMN_TOPIC,
        text = crate::note::COLUMN_TEXT,
        topics = quoted_list(SHIPPED_NOTE_TOPICS.iter().copied()),
    )
});

/// The group authorization table — an appended migration step of the
/// group-context unit. One row per group channel the operator admitted,
/// keyed like the channel mapping; absence is refusal, which is what makes
/// the check fail closed across restarts. The backfill authorizes every
/// group mapping that exists at migration time: those groups were admitted
/// under the old regime by the operator's own hand.
static GROUP_AUTHORIZATION_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE group_authorizations (
            adapter TEXT NOT NULL,
            channel TEXT NOT NULL,
            PRIMARY KEY (adapter, channel)
        );
        INSERT INTO group_authorizations (adapter, channel)
            SELECT adapter, channel FROM channels WHERE kind = '{SHIPPED_GROUP_KIND}';",
    )
});

/// The limited vocabulary's widening — an appended migration step of the
/// group-context unit, adding the command kind to the stored constraint. A
/// column CHECK cannot be altered in place, so the step recreates the
/// content table under the frozen [`WIDENED_COMMAND_LIMITS`] vocabulary,
/// copies every row, and rebuilds the principal count's index the table
/// drop removed.
/// Every stored value survives unchanged; only the constraint widens. On a
/// fresh store the step follows the shipped two-kind step and widens it the
/// same way, so both paths end at one schema.
static COMMAND_STAMP_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    let columns = format!(
        "block_id, {COLUMN_ROLE}, {COLUMN_TEXT}, {COLUMN_PRINCIPAL_ID}, {COLUMN_AUTHORITY}, \
         {COLUMN_ORIGIN}, {COLUMN_SENT_AT}, {COLUMN_ADDRESSED}, {COLUMN_ANSWER_DUE}, \
         {COLUMN_LIMITED}, {COLUMN_DEBT_AUTHORITY}"
    );
    format!(
        "CREATE TABLE {CHAT_MESSAGE_TABLE}_widened (
            block_id             INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {COLUMN_ROLE}         TEXT,
            {COLUMN_TEXT}         TEXT,
            {COLUMN_PRINCIPAL_ID} INTEGER NOT NULL,
            {COLUMN_AUTHORITY}    TEXT NOT NULL CHECK ({COLUMN_AUTHORITY} IN ({authorities})),
            {COLUMN_ORIGIN}       TEXT,
            {COLUMN_SENT_AT}      TEXT,
            {COLUMN_ADDRESSED}    INTEGER NOT NULL CHECK ({COLUMN_ADDRESSED} IN (0, 1)),
            {COLUMN_ANSWER_DUE}   INTEGER NOT NULL CHECK ({COLUMN_ANSWER_DUE} IN (0, 1)),
            {COLUMN_LIMITED}      TEXT CHECK ({COLUMN_LIMITED} IN ({limits})),
            {COLUMN_DEBT_AUTHORITY} TEXT
                CHECK ({COLUMN_DEBT_AUTHORITY} IN ({authorities}))
        );
        INSERT INTO {CHAT_MESSAGE_TABLE}_widened ({columns})
            SELECT {columns} FROM {CHAT_MESSAGE_TABLE};
        DROP TABLE {CHAT_MESSAGE_TABLE};
        ALTER TABLE {CHAT_MESSAGE_TABLE}_widened RENAME TO {CHAT_MESSAGE_TABLE};
        CREATE INDEX {index}
            ON {CHAT_MESSAGE_TABLE}({COLUMN_PRINCIPAL_ID}, {COLUMN_ADDRESSED});",
        authorities = quoted_list(SHIPPED_AUTHORITIES.iter().copied()),
        limits = quoted_list(WIDENED_COMMAND_LIMITS.iter().copied()),
        index = PRINCIPAL_ADDRESSED_INDEX.as_str(),
    )
});

/// The reply-target columns — an appended migration step of the wiki-and-
/// report unit, per decision 0026's discipline. Two nullable columns on
/// the message table, so every pre-existing row reads NULL in both: the
/// replied-to message's origin (personal data under the author-keyed
/// erasure null, like the origin itself) and whether the reply points at
/// one of the assistant's own messages (structure, which erasure leaves).
/// No frozen vocabulary list: the step closes a boolean, not an enum.
static REPLY_TARGET_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "ALTER TABLE {CHAT_MESSAGE_TABLE}
             ADD COLUMN {COLUMN_REPLY_TARGET} TEXT;
         ALTER TABLE {CHAT_MESSAGE_TABLE}
             ADD COLUMN {COLUMN_REPLY_TO_ASSISTANT} INTEGER
                 CHECK ({COLUMN_REPLY_TO_ASSISTANT} IN (0, 1));"
    )
});

/// The report block's content table — an appended migration step of the
/// wiki-and-report unit. The table shape is the report kind's descriptor
/// contract: the block header row is the ledger entry, this row carries
/// the target origin, the reported principal and the fixed line. The
/// target origin is nullable for exactly one reason: the reported person's
/// erasure nulls it, keyed by the reported principal this table stores.
/// No frozen vocabulary list: the step quotes no enum.
static REPORT_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {table} (
            block_id INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {target}   TEXT,
            {reported} INTEGER NOT NULL,
            {line}     TEXT NOT NULL
        );",
        table = crate::tools::report::REPORT_TABLE,
        target = crate::tools::report::COLUMN_TARGET_ORIGIN,
        reported = crate::tools::report::COLUMN_REPORTED_PRINCIPAL_ID,
        line = crate::tools::report::COLUMN_LINE,
    )
});

/// The speaker column — the appended migration step of the username-
/// projection unit, per decision 0026's discipline. One nullable column on
/// the message table, so every pre-existing row reads NULL and projects
/// bare: the sender's public username as the platform delivered it at
/// receipt (decision 0065). Personal data under the author-keyed erasure
/// null, like the text and the origin. No frozen vocabulary list: the step
/// quotes no enum.
static SPEAKER_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "ALTER TABLE {CHAT_MESSAGE_TABLE}
             ADD COLUMN {COLUMN_SPEAKER} TEXT;"
    )
});

/// The display name's retirement — the appended migration step of the
/// minimization ruling (decision 0077). The column was written on every
/// refresh and read by nothing, so the step deletes the stored values and
/// the surface that would accumulate them in one move; identity resolution
/// stopped naming the column in the same change. A fresh store runs the
/// shipped CREATE and this drop in order and ends at the same schema as an
/// upgraded one. No frozen vocabulary list: the step quotes no enum.
const DISPLAY_NAME_DROP_MIGRATION: &str = "ALTER TABLE principals DROP COLUMN display_name;";

/// The suppression flag — the appended migration step of the privacy-self-
/// service unit, per decision 0026's discipline. One boolean column on the
/// identity table, `INTEGER NOT NULL DEFAULT 0` so every pre-existing row
/// reads unflagged: whether the person opted out of collection on this
/// adapter, the one lawful remnant an erasure leaves standing (decision
/// 0071). The flag is the suppression mechanism's whole storage — from the
/// moment it stands, the person's inbound messages are dropped at
/// ingestion. No frozen vocabulary list: the step closes a boolean, not an
/// enum — the reply-target step's own precedent.
static SUPPRESSION_FLAG_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "ALTER TABLE principals
             ADD COLUMN {flag} INTEGER NOT NULL DEFAULT 0;",
        flag = crate::identity::COLUMN_OPTED_OUT,
    )
});

/// The literal addressed column — the appended migration step of the
/// grounded-answer unit (unit 16, 2026-08-24), per decision 0026's
/// discipline. One nullable boolean column on the message table, stored
/// beside the untouched summons column: the safe default is NULL, and that
/// absence is genuinely safe because no historical row is ever read for its
/// literal value — the one reader, the outbound answer threading, reads
/// only the messages the current turn absorbed, and folds an absent value
/// to unaddressed, which sends the answer plain. Structure, not personal
/// data: erasure leaves it. No
/// frozen vocabulary list: the step closes a boolean, not an enum — the
/// reply-target step's own precedent.
static LITERAL_ADDRESSED_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "ALTER TABLE {CHAT_MESSAGE_TABLE}
             ADD COLUMN {COLUMN_LITERAL_ADDRESSED} INTEGER
                 CHECK ({COLUMN_LITERAL_ADDRESSED} IN (0, 1));"
    )
});

/// The join notice's content table — an appended migration step of the
/// join-notice unit (unit 36, 2026-08-29), per decision 0026's discipline.
/// The table shape is the join kind's descriptor contract: the block header
/// row is the ledger entry, this row carries one joiner's shown name, their
/// handle, their principal, the shared event origin and the platform send
/// time. Four of the five are personal data under the person-keyed null,
/// which is why they are nullable; the principal id is NOT NULL, because a
/// join nobody is recorded for is a record erasure could never reach. No
/// frozen vocabulary list: the step quotes no enum.
static JOIN_NOTICE_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {table} (
            block_id     INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {name}       TEXT,
            {handle}     TEXT,
            {principal}  INTEGER NOT NULL,
            {origin}     TEXT,
            {joined_at}  TEXT
        );
        CREATE INDEX {principal_index} ON {table}({principal});
        CREATE INDEX {origin_index} ON {table}({origin});",
        table = crate::join::JOIN_NOTICE_TABLE,
        name = crate::join::COLUMN_NAME,
        handle = crate::join::COLUMN_HANDLE,
        principal = crate::join::COLUMN_PRINCIPAL_ID,
        origin = crate::join::COLUMN_ORIGIN,
        joined_at = crate::join::COLUMN_JOINED_AT,
        principal_index = JOIN_NOTICE_PRINCIPAL_INDEX.as_str(),
        origin_index = JOIN_NOTICE_ORIGIN_INDEX.as_str(),
    )
});

/// The join notice's person-keyed index, named once: the appended step
/// creates it and the suite's schema pins read it back under this name.
/// The erasure pass and the target-keyed reply join both key on the
/// principal column.
pub static JOIN_NOTICE_PRINCIPAL_INDEX: LazyLock<String> =
    LazyLock::new(|| format!("idx_{}_principal", crate::join::JOIN_NOTICE_TABLE));

/// The join notice's event-keyed index, named once beside the person's.
/// The table's other two access paths key on the event origin: the
/// deletion mirror's whole-event null, and the reference collection that
/// drives the reply-target and report-target nulls — both of them
/// per-conversation lookups over a table that grows with every join the
/// assistant ever saw.
pub static JOIN_NOTICE_ORIGIN_INDEX: LazyLock<String> =
    LazyLock::new(|| format!("idx_{}_origin", crate::join::JOIN_NOTICE_TABLE));

/// The reported column's nullability — an appended migration step of the
/// join-notice unit (unit 36, 2026-08-29). The column stood NOT NULL
/// because every reportable was exactly one person; a join event naming
/// several joiners is the first that is not, and a filing against it
/// attaches no single principal rather than recording the wrong one. A
/// column constraint cannot be altered in place, so the step recreates the
/// report table without it, copies every row, and drops the old one. Every
/// stored value survives unchanged; only the constraint relaxes. The table
/// carries no index to rebuild.
static REPORTED_NULLABLE_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    let columns = format!(
        "block_id, {target}, {reported}, {line}",
        target = crate::tools::report::COLUMN_TARGET_ORIGIN,
        reported = crate::tools::report::COLUMN_REPORTED_PRINCIPAL_ID,
        line = crate::tools::report::COLUMN_LINE,
    );
    format!(
        "CREATE TABLE {table}_nullable (
            block_id   INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {target}   TEXT,
            {reported} INTEGER,
            {line}     TEXT NOT NULL
        );
        INSERT INTO {table}_nullable ({columns}) SELECT {columns} FROM {table};
        DROP TABLE {table};
        ALTER TABLE {table}_nullable RENAME TO {table};",
        table = crate::tools::report::REPORT_TABLE,
        target = crate::tools::report::COLUMN_TARGET_ORIGIN,
        reported = crate::tools::report::COLUMN_REPORTED_PRINCIPAL_ID,
        line = crate::tools::report::COLUMN_LINE,
    )
});

/// The delivery receipt's content table — an appended migration step of
/// the her-replies-quote unit (unit 38, 2026-08-30), per decision 0026's
/// discipline. The table shape is the delivery kind's descriptor contract:
/// the block header row is the ledger entry, this row carries the
/// platform's id for one delivered message, the key of the send it
/// belonged to, and the stored block a reply to that message quotes. The
/// answer block is nullable because most sends carry none — a
/// deterministic item, the failure notice, a report's line — and the two
/// text columns are nullable for no reason of erasure's: the row is
/// structure, erasure leaves it, and the conversation's own deletion
/// removes it through the block cascade. No frozen vocabulary list: the
/// step quotes no enum.
///
/// Both keyed access paths are indexed in the same step, as the protection
/// stamp's own precedent shows: the origin, which the reply resolution
/// matches per conversation, and the delivery key, which ties the messages
/// of one send together. Neither index is unique — one send reports once
/// by construction, and the newest-row resolution tolerates a duplicate
/// rather than refusing to record it.
static DELIVERY_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {table} (
            block_id   INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {origin}   TEXT,
            {delivery} TEXT,
            {answer}   INTEGER
        );
        CREATE INDEX {origin_index} ON {table}({origin});
        CREATE INDEX {delivery_index} ON {table}({delivery});",
        table = crate::delivery::DELIVERED_TABLE,
        origin = crate::delivery::COLUMN_ORIGIN,
        delivery = crate::delivery::COLUMN_DELIVERY,
        answer = crate::delivery::COLUMN_ANSWER_BLOCK,
        origin_index = DELIVERY_ORIGIN_INDEX.as_str(),
        delivery_index = DELIVERY_KEY_INDEX.as_str(),
    )
});

/// The delivery receipt's origin-keyed index, named once: the appended
/// step creates it and the suite's schema pins read it back under this
/// name. The reply resolution matches one delivered message's origin
/// inside one conversation.
pub static DELIVERY_ORIGIN_INDEX: LazyLock<String> =
    LazyLock::new(|| format!("idx_{}_origin", crate::delivery::DELIVERED_TABLE));

/// The delivery receipt's send-keyed index, named once beside the
/// origin's: the key ties the messages of one send together, which is the
/// table's other keyed access path.
pub static DELIVERY_KEY_INDEX: LazyLock<String> =
    LazyLock::new(|| format!("idx_{}_delivery", crate::delivery::DELIVERED_TABLE));

/// The message mark's content table — an appended migration step of the
/// reactions unit (unit 39, 2026-08-30), per decision 0026's discipline.
/// The table shape is the mark kind's descriptor contract: the block
/// header row is the ledger entry, this row carries the marked message's
/// origin, the marked person and the chosen emoji. The target origin is
/// nullable for erasure's two reaches into it — the marked person's own
/// erasure and the deletion mirror — while the principal is NOT NULL,
/// because a mark nobody is recorded for is a record erasure could never
/// reach.
///
/// The emoji column carries the schema twin of the tool's own bound: NOT
/// NULL, and constrained non-empty and at most [`MARK_EMOJI_BYTE_LIMIT`]
/// BYTES.
/// The cast to a blob is what makes the count bytes and not characters —
/// `length` over text counts characters, and a per-character bound would
/// be a different rule with the same spelling. No frozen vocabulary list:
/// the column holds content, not a closed vocabulary, so nothing here
/// quotes an enum and no later widening step is owed.
static MESSAGE_MARK_MIGRATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "CREATE TABLE {table} (
            block_id   INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE,
            {target}   TEXT,
            {marked}   INTEGER NOT NULL,
            {emoji}    TEXT NOT NULL
                CHECK (length(CAST({emoji} AS BLOB)) \
                       BETWEEN 1 AND {MARK_EMOJI_BYTE_LIMIT})
        );
        CREATE INDEX {origin_index} ON {table}({target});",
        table = crate::tools::mark::MESSAGE_MARK_TABLE,
        target = crate::tools::mark::COLUMN_TARGET_ORIGIN,
        marked = crate::tools::mark::COLUMN_MARKED_PRINCIPAL_ID,
        emoji = crate::tools::mark::COLUMN_EMOJI,
        origin_index = MESSAGE_MARK_ORIGIN_INDEX.as_str(),
    )
});

/// The emoji bound as the migration froze it, quoted from the tool's own
/// constant at the moment this step shipped. An applied step's generated
/// SQL must stay byte-identical, so the step names a frozen number and the
/// pin below is what fails loudly if the tool's bound ever moves — the
/// reminder that a widened bound is a NEW appended step recreating the
/// table, exactly as a widened vocabulary is.
const MARK_EMOJI_BYTE_LIMIT: usize = 32;

/// The message mark's origin-keyed index, named once: the appended step
/// creates it and the suite's schema pins read it back under this name.
/// The deletion mirror's null is a per-conversation lookup by the marked
/// origin over a table that grows with every reaction the assistant ever
/// placed.
pub static MESSAGE_MARK_ORIGIN_INDEX: LazyLock<String> =
    LazyLock::new(|| format!("idx_{}_origin", crate::tools::mark::MESSAGE_MARK_TABLE));

/// The store configuration the assistant opens with: the composed kind's
/// descriptors and the domain migrations — the three creating steps, then
/// every appended step in order.
#[must_use]
pub fn store_config() -> StoreConfig {
    StoreConfig {
        descriptors: AssistantKind::DESCRIPTORS,
        domain_migrations: vec![DomainMigrations {
            domain: DOMAIN,
            sqls: vec![
                CHAT_MESSAGE_SCHEMA.as_str(),
                PRINCIPALS_SCHEMA,
                CHANNELS_SCHEMA.as_str(),
                PROTECTION_STAMP_MIGRATION.as_str(),
                TOOL_PALETTE_MIGRATION.as_str(),
                CONTEXT_NOTE_MIGRATION.as_str(),
                GROUP_AUTHORIZATION_MIGRATION.as_str(),
                COMMAND_STAMP_MIGRATION.as_str(),
                REPLY_TARGET_MIGRATION.as_str(),
                REPORT_MIGRATION.as_str(),
                SPEAKER_MIGRATION.as_str(),
                DISPLAY_NAME_DROP_MIGRATION,
                SUPPRESSION_FLAG_MIGRATION.as_str(),
                LITERAL_ADDRESSED_MIGRATION.as_str(),
                JOIN_NOTICE_MIGRATION.as_str(),
                REPORTED_NULLABLE_MIGRATION.as_str(),
                DELIVERY_MIGRATION.as_str(),
                MESSAGE_MARK_MIGRATION.as_str(),
            ],
        }],
    }
}

// Each vocabulary's NEWEST frozen list is pinned to its live enum: while
// they coincide, fresh stores and upgraded ones end at one schema. The
// moment an enum grows, its pin here fails — the loud reminder that the
// growth needs a new appended widening step with its own frozen list, after
// which the pin is re-pointed at that step's list.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::LimitedBy;
    use crate::note::NoteTopic;

    #[test]
    fn the_newest_frozen_limited_list_matches_the_live_enum() {
        let live: Vec<&str> = LimitedBy::ALL.iter().map(|l| l.as_str()).collect();
        assert_eq!(
            live, WIDENED_COMMAND_LIMITS,
            "the limited vocabulary grew; append a widening step with its own frozen list"
        );
    }

    #[test]
    fn the_frozen_authority_list_matches_the_live_enum() {
        let live: Vec<&str> = Authority::ALL.iter().map(|a| a.as_str()).collect();
        assert_eq!(
            live, SHIPPED_AUTHORITIES,
            "the authority vocabulary grew; append a widening step with its own frozen list"
        );
    }

    /// The mark's frozen byte bound against the tool's live one: while
    /// they coincide, a fresh store and an upgraded one bound the column
    /// identically. The moment the tool's bound moves, this fails — the
    /// reminder that a widened bound needs its own appended step, because
    /// a column CHECK cannot be altered in place.
    #[test]
    fn the_frozen_mark_bound_matches_the_tools_live_bound() {
        assert_eq!(
            MARK_EMOJI_BYTE_LIMIT,
            crate::tools::mark::EMOJI_BYTE_LIMIT,
            "the emoji bound moved; append a step recreating the table under its own \
             frozen bound"
        );
    }

    #[test]
    fn the_frozen_note_topic_list_and_group_kind_match_the_live_enums() {
        let live: Vec<&str> = NoteTopic::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            live, SHIPPED_NOTE_TOPICS,
            "the note-topic vocabulary grew; append a widening step with its own frozen list"
        );
        assert_eq!(SHIPPED_GROUP_KIND, ChannelKind::Group.as_str());
    }
}
