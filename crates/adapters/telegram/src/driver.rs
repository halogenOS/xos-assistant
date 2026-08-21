//! The adapter's loop: take the outbound edge, then long-poll, translate,
//! ingest, persist the offset, and send replies — all mechanics, no
//! decisions of its own.
//!
//! The batch discipline is the spec's: on the first transiently failed
//! ingest the batch stops, the offset is persisted up to the last success,
//! the loop backs off and re-polls, and the failed update with its
//! successors redelivers — at-least-once, the same outcome the offset
//! decision accepts. A deterministic refusal from the core is terminal for
//! that update instead: logged and acknowledged past, because retrying it
//! forever would wedge every later message in the chat behind it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use assistant_core::{Assistant, CoreError, InboundMessage, OutboundReply};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::authority::AdminCache;
use crate::client::{BotClient, Update};
use crate::translate::{self, Translation};
use crate::{ADAPTER_NAME, AdapterError, Config, Sleep, state};

/// How long the loop pauses after a failed poll or a halted batch before
/// re-polling; the loop never busy-spins.
const POLL_BACKOFF: Duration = Duration::from_secs(2);

/// What one update's processing came to.
enum Step {
    /// Recorded, skipped, or refused deterministically: acknowledged past.
    Acknowledged,
    /// A transient failure: the batch stops here and the update redelivers.
    Halted,
}

/// The run entry's body: the outbound edge first — answers stored before the
/// subscription are history, so taking it before the first poll is part of
/// the contract — then the poll loop and the reply consumer, concurrently in
/// this one future.
pub(crate) async fn run(
    config: Config,
    sleep: Sleep,
    assistant: Arc<Assistant>,
) -> Result<(), AdapterError> {
    let replies = assistant.replies(ADAPTER_NAME).await?;
    let client = BotClient::new(&config.api_root, &config.token, Arc::clone(&sleep));
    tokio::select! {
        () = poll_loop(&client, &config.state_file, &sleep, &assistant) => {}
        () = consume_replies(replies, &client) => {}
    }
    Ok(())
}

/// Long-poll and ingest until the future is dropped. Never returns on its
/// own: a network error backs off and re-polls, a halted batch backs off
/// and redelivers.
async fn poll_loop(client: &BotClient, state_file: &Path, sleep: &Sleep, assistant: &Assistant) {
    let mut next_offset = state::read(state_file);
    let mut admins = AdminCache::new();
    loop {
        let updates = match client.get_updates(next_offset).await {
            Ok(updates) => updates,
            Err(error) => {
                tracing::warn!(%error, "the poll failed; backing off");
                sleep(POLL_BACKOFF).await;
                continue;
            }
        };
        let mut halted = false;
        let offset_before = next_offset;
        for update in &updates {
            match process(update, client, &mut admins, assistant).await {
                Step::Acknowledged => next_offset = Some(update.id + 1),
                Step::Halted => {
                    halted = true;
                    break;
                }
            }
        }
        if next_offset != offset_before
            && let Some(offset) = next_offset
            && let Err(error) = state::write(state_file, offset)
        {
            // Not persisting is safe — the acknowledged updates redeliver
            // after a restart as accepted duplicates — so the loop goes on.
            tracing::error!(%error, "the offset did not persist");
        }
        if halted {
            sleep(POLL_BACKOFF).await;
        }
    }
}

/// One update: translate, resolve authority where a group owes it, ingest.
async fn process(
    update: &Update,
    client: &BotClient,
    admins: &mut AdminCache,
    assistant: &Assistant,
) -> Step {
    let pending = match translate::translate(update) {
        Translation::Skip(reason) => {
            tracing::debug!(update_id = update.id, %reason, "update skipped");
            return Step::Acknowledged;
        }
        Translation::Record(pending) => pending,
    };
    let authority = match pending.authority {
        Some(authority) => authority,
        None => match admins
            .authority_for(client, pending.chat_id, pending.sender_id)
            .await
        {
            Ok(authority) => authority,
            Err(error) => {
                // Authority is never silently defaulted into the ledger: the
                // update stays unacknowledged and the next poll retries it.
                tracing::warn!(%error, "the administrator list did not resolve; batch halted");
                return Step::Halted;
            }
        },
    };
    let message = InboundMessage {
        channel: translate::channel_key(pending.chat_id),
        channel_kind: pending.channel_kind,
        sender: pending.sender,
        authority,
        text: pending.text,
        origin: Some(pending.origin),
        timestamp: pending.sent_at,
    };
    match assistant.ingest(message).await {
        Ok(_) => Step::Acknowledged,
        Err(refusal @ CoreError::ChannelKindMismatch { .. }) => {
            // Deterministic: retrying forever would wedge every later
            // message in the chat behind this one, so it is acknowledged
            // past — the spec's stated data-loss rule.
            tracing::error!(update_id = update.id, %refusal, "refused and acknowledged");
            Step::Acknowledged
        }
        Err(error) => {
            tracing::warn!(update_id = update.id, %error, "ingest failed; batch halted");
            Step::Halted
        }
    }
}

/// Send each reply from the edge to its chat, sequentially. A send that
/// spends its bounded rate-limit retry — or fails outright — is logged and
/// dropped, and the consumer moves on to the next reply.
async fn consume_replies(mut replies: UnboundedReceiver<OutboundReply>, client: &BotClient) {
    while let Some(reply) = replies.recv().await {
        let Some(chat_id) = translate::chat_id_of(&reply.channel) else {
            tracing::error!("an outbound channel key names no chat; reply dropped");
            continue;
        };
        if let Err(failure) = client.send_message(chat_id, &reply.text).await {
            // Two different outcomes, per decision 0019: nothing reached the
            // chat, or earlier chunks did and the tail was dropped with the
            // failing one — the log must state which one happened.
            if failure.delivered_chunks == 0 {
                tracing::warn!(chat_id, error = %failure.error, "the send failed; reply dropped");
            } else {
                tracing::warn!(
                    chat_id,
                    delivered_chunks = failure.delivered_chunks,
                    error = %failure.error,
                    "a chunk failed; reply cut short after the delivered chunks"
                );
            }
        }
    }
}
