//! The adapter's loop: take the outbound edge, then long-poll, translate,
//! ingest and observe, persist the offset, and send replies — all
//! mechanics, no decisions of its own.
//!
//! The batch discipline is the spec's: on the first transiently failed
//! ingest the batch stops, the offset is persisted up to the last success,
//! the loop backs off and re-polls, and the failed update with its
//! successors redelivers — at-least-once, the same outcome the offset
//! decision accepts. A deterministic refusal from the core is terminal for
//! that update instead: logged and acknowledged past, because retrying it
//! forever would wedge every later message in the chat behind it.
//!
//! Authority resolution is deferred to the core's need (refined
//! 2026-08-23): a failed administrator fetch no longer halts the batch
//! here — the message is delivered with its authority unresolved, the core
//! refuses an unadmitted group before ever reading authority, and an
//! admitted message with no authority draws the core's typed transient
//! refusal, which halts the batch below exactly as before. Nothing is ever
//! recorded with a defaulted authority.
//!
//! The deterministic items the core returns are carried out here as
//! translation: a returned item's text becomes the platform's send, the
//! withdraw directive becomes the platform's leave call. A failed leave is
//! logged and left to the authorization check's self-healing — the group's
//! next contact is refused and re-directed all over again — and a performed
//! withdraw rests per chat, like the failed lookup below: a refused chat's
//! message flood draws one leave per rest window, not one per message. The
//! rest suppresses the administrator fetch too: while a chat's withdrawal
//! rests, its messages are delivered with authority unresolved and no list
//! is fetched — the core refuses them before reading authority — and an
//! admission inside the rest forgets it.
//!
//! The first-contact lookup is lazy: once per group chat per process, on
//! the first update seen from it, the chat is looked up and its title and
//! exposed pinned announcement are reported as observations — the title
//! only when the first contact IS a pin event, whose text outranks the
//! lookup's by-sending-date pin. A failed lookup is logged and retried on
//! the chat's next contact after a rest — the once-per-process memory is
//! not set on failure, but the failure rests for a bounded window so a
//! chat whose lookup keeps failing pays one platform call per window
//! instead of one per message — and never stops the batch: group facts are
//! enrichment, not authority. A lookup that answered sets the memory
//! whether the core observed or withdrew: an unadmitted group already
//! draws its rested leave, and re-running its lookup on every message
//! would amplify that into an extra platform call each time. An admitted
//! membership entry clears the memory instead — reporting its own
//! observation before the lookup's, so authorization is judged first —
//! and the fresh lookup behind it puts the group's facts on the ledger
//! even when an earlier refused contact had already spent the chat's
//! lookup.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use assistant_core::{
    Assistant, ChannelKind, FailureKind, InboundMessage, IngestOutcome, Observation,
    ObserveOutcome, ObservedFact, OutboundReply,
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::authority::AdminCache;
use crate::client::{BotClient, BotIdentity, Update};
use crate::translate::{self, LookupScope, Translation};
use crate::{ADAPTER_NAME, AdapterError, Config, Sleep, state};

/// How long the loop pauses after a failed poll or a halted batch before
/// re-polling; the loop never busy-spins.
const POLL_BACKOFF: Duration = Duration::from_secs(2);

/// How long a failed chat lookup rests before the chat's next contact may
/// retry it. Without the rest, a chat whose lookup keeps failing would pay
/// one extra platform call per message — and under a rate-limit refusal,
/// the call's bounded waits with it — for as long as the failure persists.
/// One minute matches the wire client's rate-limit wait ceiling, so even
/// the slowest failed lookup has fully unwound before the retry.
const LOOKUP_RETRY_REST: Duration = Duration::from_mins(1);

/// How long a performed withdraw rests before the chat's next refusal
/// re-performs the leave call, under the same reasoning as the lookup
/// rest: a refused chat's message flood must not draw one platform call
/// per message. A leave that failed rests too — the gate's self-healing
/// re-directs the chat's next contact past the rest.
const WITHDRAW_RETRY_REST: Duration = Duration::from_mins(1);

/// The most chats the answered-lookup memory holds. Past the cap the
/// memory is cleared whole: it only suppresses repeat lookups, so losing
/// it costs one extra platform call per chat, while an unbounded set would
/// grow with every chat the process ever saw.
const ANSWERED_MEMORY_CAP: usize = 4096;

/// A per-chat rest: a recorded chat suppresses the action's repeat until
/// the rest window passes. Expired entries are swept on every read, so the
/// map holds only resting chats.
struct ChatRest {
    rest: Duration,
    at: HashMap<i64, Instant>,
}

impl ChatRest {
    fn new(rest: Duration) -> Self {
        Self {
            rest,
            at: HashMap::new(),
        }
    }

    /// Whether the chat's action is still resting.
    fn resting(&mut self, chat_id: i64) -> bool {
        let now = Instant::now();
        let rest = self.rest;
        self.at.retain(|_, at| now.duration_since(*at) < rest);
        self.at.contains_key(&chat_id)
    }

    /// The action was performed (or attempted); the chat rests from now.
    fn record(&mut self, chat_id: i64) {
        self.at.insert(chat_id, Instant::now());
    }

    /// Forget the chat, so its next contact acts afresh.
    fn forget(&mut self, chat_id: i64) {
        self.at.remove(&chat_id);
    }
}

/// The first-contact lookup's per-process memory. An answered chat is
/// remembered — until an admission voids it, or the bounded set is cleared
/// at its cap — and a failed one merely rests: the retry on a later
/// contact stays, rate-bounded to one lookup per rest window per chat.
struct LookupMemory {
    /// Chats whose lookup answered: its reports observed, or the core
    /// withdrew from the chat. Bounded by [`ANSWERED_MEMORY_CAP`].
    answered: HashSet<i64>,
    /// Chats whose lookup last failed inside the rest window.
    failed: ChatRest,
}

impl LookupMemory {
    fn new() -> Self {
        Self {
            answered: HashSet::new(),
            failed: ChatRest::new(LOOKUP_RETRY_REST),
        }
    }

    /// Whether this contact skips the lookup: the chat already answered,
    /// or its last failure is still resting.
    fn skips(&mut self, chat_id: i64) -> bool {
        self.answered.contains(&chat_id) || self.failed.resting(chat_id)
    }

    /// The chat's lookup answered; no contact retries it until an
    /// admission voids the memory.
    fn record_answered(&mut self, chat_id: i64) {
        self.failed.forget(chat_id);
        if self.answered.len() >= ANSWERED_MEMORY_CAP {
            tracing::debug!("the answered-lookup memory reached its cap and was cleared");
            self.answered.clear();
        }
        self.answered.insert(chat_id);
    }

    /// The chat's lookup failed; the next contact past the rest retries.
    fn record_failure(&mut self, chat_id: i64) {
        self.failed.record(chat_id);
    }

    /// Forget the chat entirely, so its next contact looks it up afresh.
    fn void(&mut self, chat_id: i64) {
        self.answered.remove(&chat_id);
        self.failed.forget(chat_id);
    }
}

/// The poll loop's per-process memories, threaded through one carrier so
/// the update-processing signatures stay readable as the memories grow.
struct Memories {
    admins: AdminCache,
    lookups: LookupMemory,
    withdrawals: ChatRest,
}

impl Memories {
    fn new() -> Self {
        Self {
            admins: AdminCache::new(),
            lookups: LookupMemory::new(),
            withdrawals: ChatRest::new(WITHDRAW_RETRY_REST),
        }
    }
}

/// What one update's processing came to.
enum Step {
    /// Recorded, skipped, observed, or refused deterministically:
    /// acknowledged past.
    Acknowledged,
    /// A transient failure: the batch stops here and the update redelivers.
    Halted,
}

/// What handling one channel's deterministic items came to.
enum Handled {
    /// The items are carried out; the update's own content may proceed.
    Proceed,
    /// The core withdrew from the channel; nothing further of this update
    /// concerns it.
    Withdrew,
    /// A transient core failure: the caller halts the batch.
    Halted,
}

impl Handled {
    /// The step this outcome ends an update with, where withdrawing and
    /// proceeding acknowledge alike — mapped once, so the call sites
    /// cannot drift. The one caller that treats the two differently
    /// matches the variants itself.
    fn step(self) -> Step {
        match self {
            Self::Proceed | Self::Withdrew => Step::Acknowledged,
            Self::Halted => Step::Halted,
        }
    }
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
///
/// The bot's own identity comes first, with the poll's own backoff: mention
/// and reply-to-self resolution compare against it, so no message is
/// translated before it is known.
async fn poll_loop(client: &BotClient, state_file: &Path, sleep: &Sleep, assistant: &Assistant) {
    let me = fetch_identity(client, sleep).await;
    let mut next_offset = state::read(state_file);
    let mut memories = Memories::new();
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
            match process(update, &me, client, &mut memories, assistant).await {
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

/// The bot's identity from `getMe`, retried on the poll backoff until it
/// answers: without it addressing cannot be resolved, and translating
/// blind would record wrong facts into a durable ledger.
async fn fetch_identity(client: &BotClient, sleep: &Sleep) -> BotIdentity {
    loop {
        match client.get_me().await {
            Ok(me) => return me,
            Err(error) => {
                tracing::warn!(%error, "the identity fetch failed; backing off");
                sleep(POLL_BACKOFF).await;
            }
        }
    }
}

/// One update: translate, then — per its shape — report observations,
/// resolve authority where a group owes it, and ingest.
async fn process(
    update: &Update,
    me: &BotIdentity,
    client: &BotClient,
    memories: &mut Memories,
    assistant: &Assistant,
) -> Step {
    let pending = match translate::translate(update, me) {
        Translation::Skip(reason) => {
            tracing::debug!(update_id = update.id, %reason, "update skipped");
            return Step::Acknowledged;
        }
        Translation::Observe(observation) => {
            return observed(observation, update.id, client, memories, assistant).await;
        }
        Translation::Record(pending) => pending,
    };
    if pending.channel_kind == ChannelKind::Group {
        match first_contact(
            client,
            pending.chat_id,
            LookupScope::Whole,
            memories,
            assistant,
        )
        .await
        {
            Handled::Proceed => {}
            // The core withdrew from the chat during the lookup's reports;
            // the message belongs to a channel the assistant just left.
            Handled::Withdrew => return Step::Acknowledged,
            Handled::Halted => return Step::Halted,
        }
    }
    let authority = match pending.authority {
        Some(authority) => Some(authority),
        // A resting withdrawal names a chat the core just refused: its
        // messages are refused before authority is ever read, so no
        // administrator fetch is spent on them — under a rate-limited
        // list, a refused chat's flood would otherwise park the
        // sequential batch one bounded wait per message. Delivered with
        // authority unresolved, exactly like the failed fetch below; an
        // admission inside the rest forgets it, so an admitted chat
        // never waits out a stale rest.
        None if memories.withdrawals.resting(pending.chat_id) => None,
        None => match memories
            .admins
            .authority_for(client, pending.chat_id, pending.sender_id)
            .await
        {
            Ok(authority) => Some(authority),
            Err(error) => {
                // Delivered unresolved, per the module doc: the core
                // refuses an unadmitted group before reading authority,
                // and its typed transient refusal for an admitted one
                // halts the batch below — never a defaulted record, never
                // a stranger group wedging the batch.
                tracing::warn!(
                    %error,
                    "the administrator list did not resolve; delivered with authority unresolved"
                );
                None
            }
        },
    };
    let message = InboundMessage {
        channel: translate::channel_key(pending.chat_id),
        channel_kind: pending.channel_kind,
        sender: pending.sender,
        authority,
        addressed: pending.addressed,
        command: pending.command,
        text: pending.text,
        origin: Some(pending.origin),
        timestamp: pending.sent_at,
    };
    match assistant.ingest(message).await {
        Ok(IngestOutcome::Recorded { deliver, .. }) => {
            if let Some(item) = deliver {
                send_item(client, pending.chat_id, item.text()).await;
            }
            Step::Acknowledged
        }
        Ok(IngestOutcome::Withdraw) => {
            leave(client, pending.chat_id, &mut memories.withdrawals).await;
            Step::Acknowledged
        }
        // The batch discipline reads the core's own terminal-or-transient
        // statement, never its variant names: the vocabulary of what can go
        // wrong is the core's to grow.
        Err(refusal) if refusal.failure_kind() == FailureKind::Terminal => {
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

/// One observation update. A membership entry is judged before any lookup,
/// so authorization comes first; every other observed fact — the pin event
/// — is preceded by the chat's lazy lookup, reporting the title only: the
/// event carries the authoritative pinned text.
async fn observed(
    observation: Observation,
    update_id: i64,
    client: &BotClient,
    memories: &mut Memories,
    assistant: &Assistant,
) -> Step {
    let Some(chat_id) = translate::chat_id_of(&observation.channel) else {
        tracing::error!(update_id, "an observation names no chat");
        return Step::Acknowledged;
    };
    if !matches!(observation.fact, ObservedFact::Added { .. }) {
        // A pin event: the chat's lazy lookup enriches first — title only,
        // because this event outranks the lookup's by-sending-date pin —
        // and the event's own fact follows in arrival order.
        match first_contact(client, chat_id, LookupScope::TitleOnly, memories, assistant).await {
            Handled::Proceed => {}
            Handled::Withdrew => return Step::Acknowledged,
            Handled::Halted => return Step::Halted,
        }
        return report(observation, chat_id, client, memories, assistant)
            .await
            .step();
    }
    // A membership entry is judged before the lookup's reports, so
    // authorization comes first; the admitted entry is then this chat's
    // first contact, and the lookup puts its title and rules on the
    // ledger before anyone speaks.
    match report(observation, chat_id, client, memories, assistant).await {
        Handled::Proceed => {}
        Handled::Withdrew => return Step::Acknowledged,
        Handled::Halted => return Step::Halted,
    }
    // The admission voids any lookup memory an earlier refused contact
    // left behind — the group's facts were withdrawn then, and the
    // admitted group must not start with them stranded — and forgets the
    // withdrawal rest with it, so the admitted chat's next message
    // resolves authority instead of waiting out a stale rest.
    memories.lookups.void(chat_id);
    memories.withdrawals.forget(chat_id);
    first_contact(client, chat_id, LookupScope::Whole, memories, assistant)
        .await
        .step()
}

/// The lazy first-contact lookup: once per group chat per process, fetch
/// the chat's facts within the given scope and report them. The memory
/// records the lookup as answered — its reports observed, or the core
/// withdrew — so an unadmitted group spends exactly one lookup instead of
/// one per message; a failed platform lookup records a resting failure
/// instead, retries on the chat's next contact past the rest, and never
/// stops the batch. A halted batch records nothing: the update redelivers
/// whole.
async fn first_contact(
    client: &BotClient,
    chat_id: i64,
    scope: LookupScope,
    memories: &mut Memories,
    assistant: &Assistant,
) -> Handled {
    if memories.lookups.skips(chat_id) {
        return Handled::Proceed;
    }
    let info = match client.get_chat(chat_id).await {
        Ok(info) => info,
        Err(error) => {
            memories.lookups.record_failure(chat_id);
            tracing::warn!(chat_id, %error, "the channel lookup failed; retried after the rest");
            return Handled::Proceed;
        }
    };
    for observation in translate::lookup_observations(chat_id, &info, scope) {
        match report(observation, chat_id, client, memories, assistant).await {
            Handled::Proceed => {}
            Handled::Withdrew => {
                // The lookup answered and the core refused the chat: the
                // memory is set so the refused group's later messages draw
                // only the authorization check's own rested leave, never a
                // fresh lookup — a later admission clears it and re-looks.
                memories.lookups.record_answered(chat_id);
                return Handled::Withdrew;
            }
            Handled::Halted => return Handled::Halted,
        }
    }
    memories.lookups.record_answered(chat_id);
    Handled::Proceed
}

/// Report one observation and carry out what the core returned: an item's
/// text is sent to the chat, the withdraw directive becomes the rested
/// leave call. A terminal refusal is acknowledged like ingestion's; a
/// transient failure halts the batch so the update redelivers.
async fn report(
    observation: Observation,
    chat_id: i64,
    client: &BotClient,
    memories: &mut Memories,
    assistant: &Assistant,
) -> Handled {
    match assistant.observe(observation).await {
        Ok(ObserveOutcome::Observed { deliver }) => {
            if let Some(item) = deliver {
                send_item(client, chat_id, item.text()).await;
            }
            Handled::Proceed
        }
        Ok(ObserveOutcome::Withdraw) => {
            leave(client, chat_id, &mut memories.withdrawals).await;
            Handled::Withdrew
        }
        Err(refusal) if refusal.failure_kind() == FailureKind::Terminal => {
            tracing::error!(chat_id, %refusal, "observation refused and acknowledged");
            Handled::Proceed
        }
        Err(error) => {
            tracing::warn!(chat_id, %error, "observation failed; batch halted");
            Handled::Halted
        }
    }
}

/// Deliver one returned fixed text to its chat; a failure is logged and the
/// item dropped, the same rule the reply consumer applies to a failed send.
async fn send_item(client: &BotClient, chat_id: i64, text: &str) {
    if let Err(failure) = client.send_message(chat_id, text).await {
        tracing::warn!(chat_id, error = %failure.error, "a returned item did not send; dropped");
    }
}

/// Perform the withdraw directive, rested per chat: a chat whose withdraw
/// was performed (or attempted) inside the rest window draws no repeat
/// call. A failure is logged and left to the authorization check's
/// self-healing — the chat's next contact past the rest re-directs it.
async fn leave(client: &BotClient, chat_id: i64, withdrawals: &mut ChatRest) {
    if withdrawals.resting(chat_id) {
        tracing::debug!(
            chat_id,
            "the withdraw was performed within the rest; not repeated"
        );
        return;
    }
    withdrawals.record(chat_id);
    if let Err(error) = client.leave_chat(chat_id).await {
        tracing::warn!(chat_id, %error, "the leave call failed; a later contact re-directs it");
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
