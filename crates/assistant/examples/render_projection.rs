//! Render a real ledger through the model-facing projection.
//!
//! Answers one question with evidence rather than reasoning: do the empty
//! assistant blocks a silent turn records actually appear in the messages a
//! request would carry? Reads a store snapshot, never the live file.
//!
//! Usage: `cargo run -p assistant --example render_projection -- <store.db> <id>`

use std::path::Path;

use agent_ledger::providers::{MessageContent, MessageRole};
use agent_ledger::{Store, blocks_to_messages};
use assistant_core::kind::AssistantKind;
use assistant_core::schema::store_config;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: render_projection <store.db> <conversation-id>...");
    let ids: Vec<i64> = args.map(|a| a.parse().expect("conversation id")).collect();
    assert!(!ids.is_empty(), "give at least one conversation id");
    let store = Store::open_with(Path::new(&path), store_config()).expect("open store");

    for conv in ids {
        let blocks = store.list_blocks(conv).await.expect("list blocks");
        let messages = blocks_to_messages::<AssistantKind>(&blocks);
        println!(
            "\n=== conversation {conv}: {} blocks -> {} messages ===",
            blocks.len(),
            messages.len()
        );
        for (i, m) in messages.iter().enumerate() {
            let role = match m.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            };
            let body = match &m.content {
                MessageContent::Text(t) => {
                    if t.is_empty() {
                        "<<EMPTY STRING>>".to_string()
                    } else if t.trim().is_empty() {
                        format!("<<WHITESPACE ONLY, {} chars>>", t.chars().count())
                    } else {
                        let flat: String = t.chars().filter(|c| *c != '\n').take(60).collect();
                        format!("{flat:?}")
                    }
                }
                MessageContent::Parts(p) => format!("<{} parts>", p.len()),
            };
            println!("  [{i:>2}] {role:<9} {body}");
        }
        let empties = messages
            .iter()
            .filter(|m| matches!(&m.content, MessageContent::Text(t) if t.is_empty()))
            .count();
        println!("  --> empty-string messages in this projection: {empties}");
    }
}
