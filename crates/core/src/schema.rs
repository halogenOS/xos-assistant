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
    COLUMN_DEBT_AUTHORITY, COLUMN_LIMITED, COLUMN_ORIGIN, COLUMN_PRINCIPAL_ID, COLUMN_ROLE,
    COLUMN_SENT_AT, COLUMN_TEXT, LimitedBy,
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
/// adapters is two people until proven otherwise.
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

/// The protection stamp — the first appended migration step, per decision
/// 0026's discipline: the shipped CREATE TABLE above stays as it was written
/// and every schema change from the live-model unit on is a new entry here.
/// The framework counts entry `i` of the domain's list as version `i + 1`,
/// so a store created before this step holds version 3 and runs the two
/// appended steps — this one, then the palette step below — at open, while
/// a fresh store runs all five in order.
///
/// The step adds the two protection columns — both nullable, so every
/// pre-existing row reads NULL in both, with their vocabularies closed by
/// the same enums the code parses with — and the index the principal budget
/// count runs on. Both columns are structure, not personal data: erasure
/// leaves them.
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
        limits = quoted_list(LimitedBy::ALL.iter().map(|l| l.as_str())),
        authorities = quoted_list(Authority::ALL.iter().map(|a| a.as_str())),
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
            ],
        }],
    }
}
