//! Projection equivalence with the build before this one (AC9, unit 55,
//! 2026-09-02): a database an older build wrote opens under this build's
//! migrations, and every message in it renders exactly as the same recorded
//! values render when this build writes them.
//!
//! The fixture at `tests/fixtures/previous-build.sqlite` was generated once,
//! by the committed example beside it, run at the previous build's commit;
//! its README carries the command. It records domain version 21, the count
//! before this unit's two appended steps, and that is asserted here — a
//! fixture regenerated at the wrong commit would otherwise quietly become a
//! new-build one and this case would prove nothing.
//!
//! What the case is FOR: rendering is computed at request time and stored
//! nowhere, so a change to the projection re-renders all history. This unit
//! replaced the bracketed origin mark with an envelope. The claim under test
//! is that a stored row from before it renders under the new reading the
//! same way a freshly recorded one does — that the change is in the
//! renderer and not in what the store holds.

use std::path::{Path, PathBuf};

use agent_ledger::providers::{Message, MessageContent, MessageRole, blocks_to_messages};
use agent_ledger::{LeafKind, Store};
use assistant_core::kind::AssistantKind;
use assistant_core::schema::{DOMAIN, store_config};

/// The fixture as it stands in the repository — opened only through a copy,
/// never in place: opening migrates, and a test that rewrote its own fixture
/// would pass once and prove nothing afterwards.
const FIXTURE: &str = "tests/fixtures/previous-build.sqlite";

/// The domain migration version the fixture must carry: the count of
/// appended steps at the previous build, before this unit's outgoing-message
/// and contract-notice steps took 22 and 23.
const PREVIOUS_VERSION: i64 = 21;

/// A copy of the fixture at a path of this test's own, deleted with the
/// value.
struct FixtureCopy(PathBuf);

impl FixtureCopy {
    fn take() -> Self {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
        let copy = std::env::temp_dir().join(format!(
            "assistant-core-previous-build-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("the clock is past the epoch")
                .as_nanos()
        ));
        std::fs::copy(&source, &copy).expect("the committed fixture copies");
        Self(copy)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for FixtureCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        for sidecar in ["-wal", "-shm"] {
            let mut path = self.0.clone().into_os_string();
            path.push(sidecar);
            let _ = std::fs::remove_file(PathBuf::from(path));
        }
    }
}

/// The domain's recorded migration version, read off the file WITHOUT
/// opening it through the store — the store's open is what would upgrade it,
/// and this reading is about what the previous build left behind.
fn recorded_version(path: &Path) -> i64 {
    let conn = rusqlite::Connection::open(path).expect("the fixture opens for a plain read");
    conn.query_row(
        "SELECT version FROM domain_migrations WHERE domain = ?1",
        [DOMAIN],
        |row| row.get(0),
    )
    .expect("the fixture records a domain version")
}

/// The kind constant one stored type string names — the append door takes a
/// `'static` name, and naming them here is what makes a fixture holding a
/// kind this case does not know about a loud failure instead of a silent
/// skip.
fn static_kind(stored: &str) -> &'static str {
    for kind in [
        assistant_core::kind::CHAT_MESSAGE_KIND,
        assistant_core::join::JOIN_NOTICE_KIND,
    ] {
        if kind == stored {
            return kind;
        }
    }
    panic!("the fixture holds a kind this case does not record: {stored}")
}

/// Whether one block is the framework's own calendar row. It is written by
/// the store itself, not by a consumer, so the twin cannot record one and
/// neither ledger's is compared: what this case is about is the recorded
/// messages.
fn is_calendar(block: &agent_ledger::Block) -> bool {
    agent_ledger::agency::DateMarker::KINDS.contains(&block.block_type.as_str())
}

/// One ledger as the model reads it: the projected request, rendered to
/// plain text per message.
async fn projected(store: &Store, conversation_id: i64) -> Vec<(MessageRole, String)> {
    let blocks: Vec<agent_ledger::Block> = store
        .list_blocks(conversation_id)
        .await
        .expect("the ledger reads")
        .into_iter()
        .filter(|block| !is_calendar(block))
        .collect();
    blocks_to_messages::<AssistantKind>(&blocks)
        .iter()
        .map(|message| (message.role, rendered(message)))
        .collect()
}

/// One projected message's whole text, in either content mode.
fn rendered(message: &Message) -> String {
    match &message.content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                agent_ledger::providers::ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// AC9: the fixture predates this unit, opens under this build's migrations,
/// and renders row for row exactly as the same stored values render when
/// this build records them.
///
/// The freshly recorded half is written from the FIXTURE'S OWN rows rather
/// than from a second copy of them spelled here: the fixture is what "the
/// same messages" means, and a hand-written twin would be a third thing that
/// could drift from both.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_previous_builds_database_renders_exactly_as_this_build_records_it() {
    let fixture = FixtureCopy::take();
    assert_eq!(
        recorded_version(fixture.path()),
        PREVIOUS_VERSION,
        "the committed fixture must be the PREVIOUS build's: a database \
         recording this unit's steps would prove nothing about a projection \
         change"
    );

    let stored = Store::open_with(fixture.path(), store_config())
        .expect("the previous build's database opens under this build's migrations");
    let conversation = stored
        .list_conversations()
        .await
        .expect("the conversation list reads")
        .first()
        .expect("the fixture holds its conversation")
        .id;
    let blocks = stored
        .list_blocks(conversation)
        .await
        .expect("the fixture's ledger reads");
    assert!(
        blocks.len() >= 7,
        "non-vacuity: the fixture holds the recorded shapes, not an empty \
         ledger; it holds {}",
        blocks.len()
    );

    // The same stored values, recorded by THIS build into a fresh store.
    let fresh = Store::in_memory_with(store_config()).expect("an in-memory store opens");
    let twin = fresh
        .create_conversation(
            "fixture-provider".into(),
            "fixture-model".into(),
            "Fixture Model".into(),
            "fixture-vendor".into(),
        )
        .await
        .expect("the twin conversation is created");
    for block in &blocks {
        if is_calendar(block) {
            continue;
        }
        if block.block_type == "system_prompt" {
            fresh
                .insert_system_prompt(
                    twin,
                    block.fields["content"]
                        .as_str()
                        .expect("the recorded prompt carries its text")
                        .to_owned(),
                )
                .await
                .expect("the prompt is the twin's head");
            continue;
        }
        fresh
            .append_consumer_block(
                twin,
                block.role,
                static_kind(&block.block_type),
                block.fields.clone(),
                None,
            )
            .await
            .expect("the recorded row appends to the twin");
    }

    let before = projected(&stored, conversation).await;
    let now = projected(&fresh, twin).await;
    assert_eq!(
        before.len(),
        now.len(),
        "the two ledgers project the same number of messages"
    );
    for (nth, (old, new)) in before.iter().zip(now.iter()).enumerate() {
        assert_eq!(
            old, new,
            "message {nth} of the previous build's database renders \
             differently from the same values recorded now"
        );
    }

    // Non-vacuity for the reading itself: the old rows render UNDER THIS
    // BUILD's envelope, and the erased one still renders its marker alone.
    let whole: String = before
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        whole.contains(assistant_core::kind::ENVELOPE_FROM),
        "an old row renders under this build's envelope; rendered: {whole}"
    );
    assert!(
        whole.contains(assistant_core::kind::ENVELOPE_MSGID),
        "an old row's id reaches the model through the envelope"
    );
    assert!(
        whole.contains(assistant_core::kind::ERASED_MARKER),
        "the row the erasure emptied still renders its marker alone"
    );
}
