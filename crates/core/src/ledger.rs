//! Bounded kind-agnostic reads over the ledger's framework tables — the
//! queries that answer a question about the conversation's shape without
//! belonging to any one block kind. A kind's own content reads live with
//! the kind; what lives here names no content table at all, so a new
//! consumer kind joins a caller's kind list as data instead of dragging a
//! leaf module into the assembly's vocabulary.

use agent_ledger::Store;
use agent_ledger::store::{StoreError, domain_run};
use rusqlite::OptionalExtension;

/// The newest block of one conversation outside the given read-through
/// kinds — the bounded read behind the entry point's owing-tail walk. The
/// caller names the kinds independent paths append mid-history (the
/// assembly's set: the context note, the superseding palette, the report)
/// and this query answers what stands behind that run in one row instead
/// of hydrating the conversation; the placeholder list is built from the
/// slice, so a widened set is a data change at the caller, never an edit
/// here. An empty slice degrades to the plain newest block.
///
/// # Errors
///
/// [`StoreError`] if the query fails or the store's actor has stopped.
pub(crate) async fn newest_block_id_past(
    store: &Store,
    conversation_id: i64,
    read_through: &'static [&'static str],
) -> Result<Option<i64>, StoreError> {
    domain_run(&store.tx(), crate::schema::DOMAIN, move |conn| {
        let exclusion = if read_through.is_empty() {
            String::new()
        } else {
            let placeholders: Vec<String> = (0..read_through.len())
                .map(|index| format!("?{}", index + 2))
                .collect();
            format!("AND b.block_type NOT IN ({}) ", placeholders.join(", "))
        };
        let mut parameters: Vec<&dyn rusqlite::ToSql> = vec![&conversation_id];
        parameters.extend(read_through.iter().map(|kind| kind as &dyn rusqlite::ToSql));
        Ok(conn
            .query_row(
                &format!(
                    "SELECT cb.block_id FROM conversation_blocks cb \
                     JOIN blocks b ON b.id = cb.block_id \
                     WHERE cb.conversation_id = ?1 \
                     {exclusion}\
                     ORDER BY cb.id DESC LIMIT 1"
                ),
                parameters.as_slice(),
                |row| row.get(0),
            )
            .optional()?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use agent_ledger::Role;

    use super::*;
    use crate::note::{CONTEXT_NOTE_KIND, ContextNote, NoteTopic};

    /// A tail that is a run of read-through kinds answers the block behind
    /// the whole run; an empty kind list answers the plain newest block —
    /// the arity is the caller's data, not this query's shape.
    #[tokio::test]
    async fn the_read_answers_past_the_given_kinds_and_plainly_past_none() {
        let store =
            Store::in_memory_with(crate::schema::store_config()).expect("an in-memory store opens");
        let conversation = store
            .create_conversation("p".into(), "m".into(), "M".into(), "v".into())
            .await
            .expect("a conversation row");
        assert_eq!(
            newest_block_id_past(&store, conversation, &[CONTEXT_NOTE_KIND])
                .await
                .expect("the empty read runs"),
            None,
            "an empty conversation holds nothing to answer"
        );

        let behind = store
            .insert_text_block(conversation, Role::User, "the block behind".into())
            .await
            .expect("the text block appends");
        for text in ["the first note", "the newest note"] {
            store
                .append_consumer_block(
                    conversation,
                    None,
                    CONTEXT_NOTE_KIND,
                    ContextNote::stored_fields(NoteTopic::Rules, text),
                    None,
                )
                .await
                .expect("the note appends");
        }

        assert_eq!(
            newest_block_id_past(&store, conversation, &[CONTEXT_NOTE_KIND])
                .await
                .expect("the read-through runs"),
            Some(behind),
            "the whole note run is read through to the block behind it"
        );
        let newest = newest_block_id_past(&store, conversation, &[])
            .await
            .expect("the plain read runs")
            .expect("the conversation has a newest block");
        assert!(
            newest > behind,
            "an empty kind list reads the newest block plainly"
        );
    }
}
