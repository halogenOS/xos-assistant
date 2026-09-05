//! Write the projection-equivalence fixture: one conversation of recorded
//! member rows, in the shapes the projection has to keep rendering.
//!
//! Run at the PREVIOUS build's commit, this produces the database
//! `crates/core/tests/fixtures/previous-build.sqlite` — a store written by a
//! build that predates unit 55, which the equivalence test then opens under
//! this build's migrations. See the README beside the fixture for the
//! generating command.
//!
//! Everything here is deterministic: fixed identifiers, fixed platform send
//! times, fixed prose. Nothing reads a clock except the store's own block
//! headers, which no assertion depends on.
//!
//! The example is committed because the fixture is: a binary artifact whose
//! recipe is not in the tree is a file nobody can reproduce or audit, and
//! this one has to be regenerated whenever a later unit needs an older
//! shape.

use agent_ledger::store::domain_run;
use agent_ledger::{Role, Store};
use assistant_core::Authority;
use assistant_core::join::{JoinNotice, RecordedJoiner};
use assistant_core::kind::{CHAT_MESSAGE_KIND, ChatMessage, RecordedOrigin, RecordedSender, Stamp};
use assistant_core::schema::store_config;

/// The person every recorded row here belongs to.
const MEMBER: i64 = 1;

/// The person whose rows the erasure below empties — a second principal, so
/// the erasure reaches exactly their row and no other.
const ERASED: i64 = 2;

/// The system prompt the fixture's conversation opens with, so the ledger
/// has the head a served conversation has.
const PROMPT: &str = "The previous build's recorded system prompt.";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "previous-build.sqlite".to_owned());
    let store = Store::open_with(std::path::Path::new(&path), store_config())
        .expect("the fixture store opens under the shipped configuration");

    let conversation = store
        .create_conversation(
            "fixture-provider".into(),
            "fixture-model".into(),
            "Fixture Model".into(),
            "fixture-vendor".into(),
        )
        .await
        .expect("the fixture conversation is created");
    store
        .insert_system_prompt(conversation, PROMPT.into())
        .await
        .expect("the prompt is the conversation's head");

    record_messages(&store, conversation).await;

    // A join notice: the other kind that carries a person, an id and a
    // platform time.
    store
        .append_consumer_block(
            conversation,
            None,
            assistant_core::join::JOIN_NOTICE_KIND,
            JoinNotice::stored_fields(
                RecordedJoiner {
                    principal_id: MEMBER,
                    name: "Ada Lovelace",
                    handle: Some("ada"),
                },
                "join-1",
                "2026-08-20T09:14:00+00:00",
            ),
            None,
        )
        .await
        .expect("the join notice appends");

    // The erasure, as the person-wide pass performs it: the six nulls it
    // applies to every row the erased person wrote. Spelled here rather
    // than driven through the assembly, because a fixture writer has no
    // provider to run a whole assistant over.
    domain_run(&store.tx(), assistant_core::schema::DOMAIN, |conn| {
        conn.execute(
            "UPDATE block_chat_message SET text = NULL, origin = NULL, sent_at = NULL, \
             reply_target = NULL, speaker = NULL, revises = NULL WHERE principal_id = ?1",
            [ERASED],
        )?;
        Ok(())
    })
    .await
    .expect("the erased row is emptied");

    println!("wrote {path}: conversation {conversation}");
}

/// The recorded messages the fixture carries, in ledger order.
///
/// Their own function so the writer's main stays a list of steps: the
/// shapes are the point of the fixture and belong together.
async fn record_messages(store: &Store, conversation: i64) {
    // The recorded shapes, in order: a handle and an origin and a send
    // time; the same without a stored handle, which stores NULL rather than
    // minting a substitute; a message whose own prose carries a fence line,
    // the shape that would break a header rendered around it; a revision of
    // that one, recorded under its own id; and the row the erasure below
    // empties, written by a second person so the erasure reaches exactly
    // theirs.
    for (text, speaker, origin, revises, sent_at, person) in [
        (
            "where did the setting move?",
            Some("ada"),
            "m-1",
            None,
            "2026-08-20T09:15:00+00:00",
            MEMBER,
        ),
        (
            "and on the tablet?",
            None,
            "m-2",
            None,
            "2026-08-20T09:16:30+00:00",
            MEMBER,
        ),
        (
            "here is my log:\n---\nfailed to mount\n---\nany ideas?",
            Some("ada"),
            "m-3",
            None,
            "2026-08-20T09:18:00+00:00",
            MEMBER,
        ),
        (
            "here is my log — corrected:\nfailed to mount /data",
            Some("ada"),
            "m-4",
            Some("m-3"),
            "2026-08-20T09:19:10+00:00",
            MEMBER,
        ),
        (
            "a line the person later had erased",
            Some("bern"),
            "m-5",
            None,
            "2026-08-20T09:20:00+00:00",
            ERASED,
        ),
    ] {
        append(
            store,
            conversation,
            ChatMessage::stored_fields(
                text,
                RecordedSender {
                    principal_id: person,
                    authority: Authority::Member,
                    speaker,
                },
                RecordedOrigin {
                    origin: Some(origin),
                    revises,
                },
                None,
                sent_at,
                summoned(),
            ),
        )
        .await;
    }
}

/// The stamp every recorded row here carries: a message that summoned the
/// assistant and owes it a turn, refused by no budget.
fn summoned() -> Stamp {
    Stamp {
        addressed: true,
        literal_addressed: true,
        limited: None,
        answer_due: true,
        debt_authority: Some(Authority::Member),
    }
}

/// Append one recorded chat message under the user's voice.
async fn append(
    store: &Store,
    conversation: i64,
    fields: serde_json::Map<String, serde_json::Value>,
) {
    store
        .append_consumer_block(
            conversation,
            Some(Role::User),
            CHAT_MESSAGE_KIND,
            fields,
            None,
        )
        .await
        .expect("the recorded message appends");
}
