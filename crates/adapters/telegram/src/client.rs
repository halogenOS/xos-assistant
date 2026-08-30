//! The one module that owns the Bot API wire: request building, the bot
//! token, and JSON decoding into the adapter's own minimal update model.
//!
//! Every HTTP detail lives here and nowhere else — the request timeouts, the
//! long-poll timeout parameter, and the rate-limit retry with its injectable
//! sleep. The token authenticates requests through the URL path, so every
//! failure that could carry a URL is reduced to a token-free detail string
//! before it leaves this module: no log line and no error string ever
//! carries the token.

use std::time::Duration;

use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::Sleep;

/// The long-poll timeout handed to the API, in seconds: how long one update
/// request may hang before answering empty.
const LONG_POLL_SECONDS: u64 = 25;

/// The whole-request timeout: the long poll plus room for transport.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(LONG_POLL_SECONDS + 10);

/// How many times one request is attempted while the platform rate-limits
/// it: the first try plus bounded retries, each honoring the stated wait.
/// The bound holds for every endpoint — a poll or an administrator fetch
/// that ignored the stated wait would re-ask a limiter on a fixed backoff,
/// amplifying the very load being limited.
const RATE_LIMIT_ATTEMPTS: u32 = 3;

/// The platform's cap on one message's text, in UTF-16 code units — the
/// unit the platform measures text in — so a chunk within this bound fits
/// no matter which characters it carries.
const MESSAGE_UTF16_UNIT_LIMIT: usize = 4096;

/// The rate-limit wait used when the reply states none — the platform always
/// states one, so this only keeps a malformed reply from retrying instantly.
const FALLBACK_RETRY_WAIT: Duration = Duration::from_secs(1);

/// The longest stated rate-limit wait the client honors. The outbound
/// consumer is sequential, so honoring an unbounded stated wait would park
/// every later reply behind one flooded request; a reply stating a wait
/// past this ceiling fails the request instead, and the caller applies its
/// usual failure rule — for a send, logged and dropped.
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_mins(1);

/// The typing action's own rate-limit wait ceiling: one refresh period,
/// not [`MAX_RATE_LIMIT_WAIT`]. The action is a presence cue re-sent on
/// the refresh cadence, and the platform lets the indicator expire in
/// about five seconds — a wait longer than the refresh would deliver a
/// cue whose moment has passed and whose next tick was due to re-send it
/// anyway, so a stated wait past one period fails the call at once and
/// the refresh loop keeps its cadence.
const CHAT_ACTION_WAIT_CEILING: Duration = crate::driver::TYPING_REFRESH;

/// The reaction's own rate-limit wait ceiling: none at all (unit 39,
/// 2026-08-30). The platform always states a wait and the fallback is a
/// second, so every stated wait exceeds this and the call fails at once.
///
/// Two reasons, and both would pick zero on their own. The outbound
/// consumer is sequential, so a reaction honouring the send's ceiling
/// could park every later answer behind a cosmetic call for up to two
/// minutes — the tree already refused that for the other cosmetic call,
/// giving the typing action its own ceiling. And a reaction has no value
/// late: it says the assistant read this, and arriving minutes after the
/// conversation moved on it is noise. Zero is the honest ceiling, not a
/// tuned number.
const REACTION_WAIT_CEILING: Duration = Duration::ZERO;

/// The HTTP status the platform rate-limits with.
const TOO_MANY_REQUESTS: u16 = 429;

/// The status range that says the request itself was wrong, so the API
/// declined it and performed nothing: the client-error range, rate limits
/// excluded above. A status outside it — a server error from the API or
/// from whatever sits in front of it — says only that the answer failed,
/// never that the send did not happen.
const CLIENT_ERROR_STATUSES: std::ops::Range<u16> = 400..500;

/// What a wire operation fails with. Constructed only in this module, and
/// never from a raw transport error's text without redaction — the token
/// travels in the URL, so the URL never travels in an error.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ClientError {
    /// The request did not complete: connect, timeout or transport failure.
    #[error("the request did not complete: {detail}")]
    Transport { detail: String },
    /// The API answered outside the success range, with no rate-limit shape.
    #[error("the API answered status {status}")]
    Status { status: u16 },
    /// The API answered `ok: false`.
    #[error("the API refused: {description}")]
    Refused { description: String },
    /// The answer did not decode into the expected shape.
    #[error("the answer did not decode: {detail}")]
    Decode { detail: String },
    /// Every attempt of a request was rate-limited; the bound is spent.
    #[error("rate-limited on all {RATE_LIMIT_ATTEMPTS} attempts")]
    RateLimitedOut,
    /// The rate-limit reply stated a wait past the caller's ceiling —
    /// [`MAX_RATE_LIMIT_WAIT`], or [`CHAT_ACTION_WAIT_CEILING`] for the
    /// typing action; honoring it would park the caller past its own
    /// bounds, so the request fails at once instead of waiting.
    #[error("rate-limited with a stated wait of {stated_seconds}s, past the honored ceiling")]
    RateLimitWaitOverCeiling { stated_seconds: u64 },
}

impl ClientError {
    /// Whether the platform answered this request by declining it: the API
    /// replied, refused, and delivered nothing. Both shapes the API
    /// declines in are named — a client-error status, and a success status
    /// carrying `ok: false` — because the platform uses the first for a bad
    /// request and the second for the rest.
    ///
    /// A server-error status is deliberately outside, and the range is why
    /// the status is read instead of matched whole: a 500 from the API, or
    /// a 502 or 504 from whatever fronts it, does not say the send was not
    /// performed. It leaves the message's fate exactly as unknown as a
    /// transport failure does, and repeating an unknown fate is how the
    /// same text reaches the chat twice. A transport failure and an
    /// undecodable answer are outside for that same reason, and a spent
    /// rate-limit bound asks for time, not for a different request.
    fn is_refusal(&self) -> bool {
        match self {
            Self::Status { status } => CLIENT_ERROR_STATUSES.contains(status),
            Self::Refused { .. } => true,
            Self::Transport { .. }
            | Self::Decode { .. }
            | Self::RateLimitedOut
            | Self::RateLimitWaitOverCeiling { .. } => false,
        }
    }
}

/// How one send threads, in the platform's own terms: the decoded message
/// id, carrying the core's stated recovery for a threaded send the
/// platform refuses. Built only by
/// [`translate::send_thread`](crate::translate::send_thread), which is
/// where the core's [`assistant_core::ReplyThread`] is read; nothing here
/// consults the reply's kind, because what a reply is worth without its
/// thread is the core's judgment and not the wire's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendThread {
    /// No target: the text goes out as a plain message.
    Plain,
    /// Onto the message, and plain where the platform refuses that send.
    OntoOrPlainly(i64),
    /// Onto the message or not at all; a refusal is the send's failure.
    OntoOnly(i64),
}

impl SendThread {
    /// The message the first chunk threads onto, if any.
    fn target(self) -> Option<i64> {
        match self {
            Self::Plain => None,
            Self::OntoOrPlainly(message_id) | Self::OntoOnly(message_id) => Some(message_id),
        }
    }

    /// Whether a refused threaded send is followed by one plain send of
    /// the same text.
    fn plain_when_refused(self) -> bool {
        matches!(self, Self::OntoOrPlainly(_))
    }
}

/// A send that did not deliver its whole reply. The delivered ids exist so
/// the caller can state what actually happened and record it: an empty list
/// means the reply was dropped whole, a non-empty one means the chat holds
/// the reply's head and the tail was dropped — two different outcomes a log
/// must not conflate, and exactly the messages a delivery receipt may name
/// (unit 38, 2026-08-30).
#[derive(Debug)]
pub(crate) struct SendError {
    /// The platform's ids for the chunks that reached the chat before the
    /// failing one, in send order.
    pub delivered: Vec<i64>,
    /// What the failing chunk's request failed with.
    pub error: ClientError,
}

/// The update types the poll consumes, named explicitly on every poll: an
/// absent selection inherits whatever an earlier setting left on the token,
/// so the selection is stated instead of assumed. Messages and their edits,
/// and the assistant's own membership updates.
///
/// Neither reaction update type is here, and neither is an oversight (unit
/// 39, 2026-08-30). The platform delivers both only to a bot that is an
/// ADMINISTRATOR of the chat, and the operator contract requires this
/// assistant to stay an ordinary member so its reports reach the
/// moderation bot. Subscribing anyway would add two decode paths that
/// never execute, with a privacy notice attached to collection that never
/// happens. The assistant therefore places reactions and reads nobody
/// else's; the operator contract records the same fact where an operator
/// will look for it.
pub(crate) const CONSUMED_UPDATE_TYPES: [&str; 3] = ["message", "edited_message", "my_chat_member"];

/// One update, decoded into the minimal model this adapter reads. Unknown
/// fields are ignored by the decoder, so the model stays exactly as small as
/// the translation needs.
#[derive(Debug, Deserialize)]
pub(crate) struct Update {
    /// The update's own id, `update_id` on the wire — what the offset
    /// acknowledges by.
    #[serde(rename = "update_id")]
    pub id: i64,
    /// A newly sent message. Absent on every other update kind.
    pub message: Option<Incoming>,
    /// An edit to an existing message — present so the edit skip is a named
    /// case instead of an anonymous non-message update.
    pub edited_message: Option<Incoming>,
    /// A change to the assistant's own membership in a chat.
    pub my_chat_member: Option<MemberUpdate>,
}

/// One incoming message, reduced to what translation reads.
#[derive(Debug, Deserialize)]
pub(crate) struct Incoming {
    pub message_id: i64,
    /// The platform's send time, unix seconds.
    pub date: i64,
    pub chat: Chat,
    /// The sending person. Absent on service messages and channel posts.
    pub from: Option<User>,
    /// Present when the message was sent on behalf of a chat — an anonymous
    /// administrator or a linked channel — which decision 0016 skips.
    pub sender_chat: Option<Chat>,
    pub text: Option<String>,
    /// A media message's caption, the fallback text per decision 0017.
    pub caption: Option<String>,
    /// The message this one replies to, reduced to the author the
    /// reply-to-self addressing check reads.
    pub reply_to_message: Option<RepliedTo>,
    /// The part of the replied-to message this reply quotes, where the
    /// sender selected one (unit 31, 2026-08-28).
    pub quote: Option<QuotedPart>,
    /// The pinned message a pin service note points at, reduced to what the
    /// pin observation reads. Present exactly on the pin service message.
    pub pinned_message: Option<PinnedContent>,
    /// The people a join service message announces — present exactly on
    /// that service message, one entry per person the event names, the bot
    /// itself included when it was added among them.
    pub new_chat_members: Option<Vec<Joiner>>,
    /// The person a departure service message announces — the platform's
    /// one form for a member leaving and for a member removed. Presence is
    /// the whole reading: the adapter names the shape and records nothing,
    /// so it decodes as the presence marker.
    pub left_chat_member: Option<IgnoredAny>,
    /// A group's creation service message.
    #[serde(default)]
    pub group_chat_created: bool,
    /// A supergroup's creation service message.
    #[serde(default)]
    pub supergroup_chat_created: bool,
    /// A broadcast channel's creation service message.
    #[serde(default)]
    pub channel_chat_created: bool,
}

/// One person a join service message names (unit 36, 2026-08-29): the id
/// and handle every identity crosses the boundary with, plus the platform's
/// name fields.
///
/// A type of its own precisely so [`User`] stays exactly as decision 0077
/// left it — a message still decodes no display name, because a message's
/// content is what was said. A join notice's content IS the shown name, so
/// it is decoded here, on the one event that carried it.
#[derive(Debug, Deserialize)]
pub(crate) struct Joiner {
    pub id: i64,
    pub username: Option<String>,
    /// The joiner's first name. The platform requires one, but the decoder
    /// keeps it optional so a malformed entry degrades to a nameless
    /// joiner instead of refusing the whole update batch.
    pub first_name: Option<String>,
    /// The joiner's last name, where they set one.
    pub last_name: Option<String>,
    /// Whether the joining account is a bot, decoded exactly as
    /// [`User::is_bot`] is: the platform states it on both, and absent
    /// decodes as false. A joiner's identity carries their OWN flag —
    /// a bot walking in is a bot.
    #[serde(default)]
    pub is_bot: bool,
}

/// A pinned message, reduced to its date discriminator and its text. The
/// platform delivers a pin it withholds the content of in the inaccessible
/// form, whose discriminator is a date of zero — such a pin yields no
/// observation. The date decodes leniently on purpose: a payload without
/// one reads as inaccessible instead of refusing the whole update batch.
#[derive(Debug, Deserialize)]
pub(crate) struct PinnedContent {
    /// The pinned message's send date, unix seconds; zero is the
    /// inaccessible form.
    #[serde(default)]
    pub date: i64,
    pub text: Option<String>,
    /// A pinned media message's caption, the fallback text.
    pub caption: Option<String>,
}

/// One membership update about the assistant itself: which chat, who acted,
/// and the member states before and after. The states decode leniently so a
/// malformed update degrades to a skip instead of refusing the batch.
#[derive(Debug, Deserialize)]
pub(crate) struct MemberUpdate {
    pub chat: Chat,
    /// Who performed the change — the acting principal of a membership
    /// observation.
    pub from: Option<User>,
    pub old_chat_member: Option<MemberState>,
    pub new_chat_member: Option<MemberState>,
}

/// One side of a membership transition, reduced to what membership is
/// judged by: the status string, and the restricted form's own member flag.
#[derive(Debug, Deserialize)]
pub(crate) struct MemberState {
    pub status: String,
    /// Whether a restricted member is still in the chat; the platform
    /// carries it on the restricted form only.
    pub is_member: Option<bool>,
}

/// What the channel lookup reads from one chat: the title and the exposed
/// pinned announcement — the first-contact enrichment the adapter reports
/// as observations.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatInfo {
    pub title: Option<String>,
    pub pinned_message: Option<PinnedContent>,
}

/// The quoted part of a replied-to message, reduced to the two fields the
/// core is told about: the quoted text, and whether the sender selected it
/// by hand (unit 31, 2026-08-28).
///
/// The platform carries a third field here — the excerpt's offset into the
/// replied-to text, counted in UTF-16 code units — and it is deliberately
/// NOT decoded. Converting that count onto our UTF-8 text is exactly the
/// arithmetic this project's history warns about, and the core needs none
/// of it: it finds the excerpt by searching the text it stored. A field
/// nothing reads is a field nothing can get wrong.
///
/// Both members decode leniently, so a payload shaped unexpectedly
/// degrades to a plain reply instead of refusing the whole update batch.
#[derive(Debug, Deserialize)]
pub(crate) struct QuotedPart {
    /// The quoted text as the platform delivers it.
    pub text: Option<String>,
    /// Whether the sender chose the excerpt themselves. The platform sends
    /// the flag only when it is true, so its absence reads as false.
    #[serde(default)]
    pub is_manual: bool,
}

/// The replied-to message, reduced to what the adapter reads: its author —
/// addressing asks whether that author is the bot itself — and its own
/// message id, which translation stores as the reply target (2026-08-23).
/// The id decodes leniently: a reply without a usable id stores no target.
#[derive(Debug, Deserialize)]
pub(crate) struct RepliedTo {
    pub from: Option<User>,
    pub message_id: Option<i64>,
}

/// A message the assistant just sent, reduced to the one field the send
/// path reads back: the platform's own id for it (unit 38, 2026-08-30).
/// Every other field of the answer is the text the caller already holds.
#[derive(Debug, Deserialize)]
pub(crate) struct Sent {
    pub message_id: i64,
}

/// The chat a message lives in: its id and its platform type string.
#[derive(Debug, Deserialize)]
pub(crate) struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
}

/// A sending person's identity fields — the three the translation carries:
/// the id, the username (decision 0077), and the platform's own bot flag
/// (2026-08-30). The platform's name fields are not decoded at all, so a
/// display name never enters the process as a typed value; the decoder skips
/// them like any other unknown key.
#[derive(Debug, Deserialize)]
pub(crate) struct User {
    pub id: i64,
    pub username: Option<String>,
    /// Whether the account is a bot, as the platform states it on every
    /// sender object. Absent decodes as false, which is the wire's own
    /// meaning — the flag is the platform's assertion that an account is
    /// automated, and nothing else asserts it.
    #[serde(default)]
    pub is_bot: bool,
}

/// The bot's own identity, from `getMe`: what mention and reply-to-self
/// resolution compare against, and — since unit 14 — where the embedder
/// reads the display name the assistant's name defaults to. Fetched before
/// the first poll; no message is translated without it.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BotIdentity {
    /// The bot's own user id — what a reply's author is compared to.
    pub id: i64,
    /// The bot's username — what a mention names. The platform requires one
    /// for every bot, but the decoder keeps it optional so a malformed
    /// answer degrades to mention-blindness instead of refusing to decode.
    pub username: Option<String>,
    /// The bot's display name — the platform requires one, but the decoder
    /// keeps it optional so a malformed answer degrades instead of
    /// refusing to decode; the embedder's startup read refuses loudly on
    /// its own when the default it needs is absent.
    pub first_name: Option<String>,
}

/// One entry of a chat's administrator list: who, with which status string.
#[derive(Debug, Deserialize)]
pub(crate) struct ChatMember {
    pub user: MemberUser,
    pub status: String,
}

/// The administrator list's user, reduced to the id the cache keys on.
#[derive(Debug, Deserialize)]
pub(crate) struct MemberUser {
    pub id: i64,
}

/// The envelope every Bot API answer arrives in.
#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
    parameters: Option<ResponseParameters>,
}

/// The extra parameters a refusal may carry; the rate-limit wait lives here.
#[derive(Debug, Deserialize)]
struct ResponseParameters {
    retry_after: Option<u64>,
}

/// The thin Bot API client: one method per call the two intakes and the
/// outbound path make — the identity, the poll, the webhook registration and
/// its deletion, the chat lookups, the sends and the leave — with every HTTP
/// concern kept inside.
pub(crate) struct BotClient {
    http: reqwest::Client,
    root: String,
    token: String,
    sleep: Sleep,
}

impl BotClient {
    /// A client on the given root. The constructor's expectation is
    /// documented at the one place it can fail: building the HTTP client
    /// fails only when the TLS backend cannot initialize, which is a broken
    /// build, not a runtime condition — so it is a panic, not an error path.
    ///
    /// An ambient system proxy is ignored: requests go where the root
    /// points and nowhere else, so the token-bearing URL cannot be routed
    /// through a host the configuration never named. A deployment that
    /// needs a proxy takes it up as its own wiring decision, not from the
    /// environment.
    pub(crate) fn new(root: &str, token: &str, sleep: Sleep) -> Self {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .build()
            .expect("the HTTP client builds with its default configuration");
        Self {
            http,
            root: root.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            sleep,
        }
    }

    /// The bot's own identity, from `getMe`.
    pub(crate) async fn get_me(&self) -> Result<BotIdentity, ClientError> {
        let response = self.request("getMe", &serde_json::json!({}), None).await?;
        self.decode(response).await
    }

    /// One long poll: every update at or past the offset, or an empty batch
    /// when the poll times out quietly. The consumed update types are named
    /// on every poll ([`CONSUMED_UPDATE_TYPES`]): an absent selection would
    /// inherit whatever an earlier setting left on the token.
    pub(crate) async fn get_updates(
        &self,
        offset: Option<i64>,
    ) -> Result<Vec<Update>, ClientError> {
        let mut body = serde_json::json!({
            "timeout": LONG_POLL_SECONDS,
            "allowed_updates": CONSUMED_UPDATE_TYPES,
        });
        if let Some(offset) = offset {
            body["offset"] = serde_json::json!(offset);
        }
        let response = self.request("getUpdates", &body, None).await?;
        self.decode(response).await
    }

    /// Register the webhook: the public address the platform will call, the
    /// secret token every delivery must carry back, and the same consumed
    /// update types the poll request pins — one list, named on both intakes,
    /// so neither inherits whatever an earlier setting left on the token.
    /// Pending updates are explicitly not dropped: whatever queued through an
    /// outage is delivered, which is the at-least-once promise working.
    ///
    /// The secret travels in the request body and comes back in nothing: the
    /// answer is a bare acknowledgement, and a refusal carries the platform's
    /// own description, which the caller scrubs before it reaches a log.
    pub(crate) async fn set_webhook(
        &self,
        url: &str,
        secret_token: &str,
    ) -> Result<(), ClientError> {
        let body = serde_json::json!({
            "url": url,
            "secret_token": secret_token,
            "allowed_updates": CONSUMED_UPDATE_TYPES,
            "drop_pending_updates": false,
        });
        let response = self.request("setWebhook", &body, None).await?;
        let _registered: serde_json::Value = self.decode(response).await?;
        Ok(())
    }

    /// Unregister the webhook, so the poll may run: the two intakes are
    /// mutually exclusive on the platform's side. Idempotent there —
    /// deleting nothing succeeds — which is why the polling start calls it
    /// unconditionally instead of asking first. Pending updates are not
    /// dropped: they are exactly what the poll is about to fetch.
    pub(crate) async fn delete_webhook(&self) -> Result<(), ClientError> {
        let body = serde_json::json!({ "drop_pending_updates": false });
        let response = self.request("deleteWebhook", &body, None).await?;
        let _deleted: serde_json::Value = self.decode(response).await?;
        Ok(())
    }

    /// One chat's own facts — the first-contact lookup's wire call. The
    /// wait ceiling applies because the lookup runs inside the sequential
    /// batch, where an unbounded stated wait would park every later
    /// update — and the caller treats any failure as best-effort, logged
    /// and retried on the chat's next contact, so a ceiling refusal costs
    /// nothing but the retry the lookup already has.
    pub(crate) async fn get_chat(&self, chat_id: i64) -> Result<ChatInfo, ClientError> {
        let body = serde_json::json!({ "chat_id": chat_id });
        let response = self
            .request("getChat", &body, Some(MAX_RATE_LIMIT_WAIT))
            .await?;
        self.decode(response).await
    }

    /// Leave one chat — what the core's withdraw directive maps to. A
    /// failure is the caller's to log and leave to the authorization
    /// check's self-healing: the group's next contact is refused and
    /// re-directed all over again. The send ceiling applies because the
    /// call runs inside the sequential batch, where an unbounded stated
    /// wait would park every later update.
    pub(crate) async fn leave_chat(&self, chat_id: i64) -> Result<(), ClientError> {
        let body = serde_json::json!({ "chat_id": chat_id });
        let response = self
            .request("leaveChat", &body, Some(MAX_RATE_LIMIT_WAIT))
            .await?;
        let _left: serde_json::Value = self.decode(response).await?;
        Ok(())
    }

    /// Send one reply's text to its chat, threaded as `thread` states.
    /// Text past the platform's message cap goes out as consecutive
    /// chunks, per decision 0019: the cap is the platform's, and dropping
    /// or truncating the reply instead would lose the answer.
    /// Only the first chunk threads (2026-08-23): a thread is one answer,
    /// and the platform shows the continuation chunks right under it. The
    /// reply travels as the platform's current reply parameters — the old
    /// reply field was replaced two platform versions ago — with
    /// send-without-reply tolerance, so a deleted target degrades to a
    /// plain send. Every other refusal of a threaded send is met by
    /// [`Self::send_chunk_threaded`], which recovers exactly where the
    /// core asked for it. A chunk that fails ends the reply there: sending
    /// the tail after a lost middle would deliver a spliced statement, so
    /// the caller drops the rest with it — and the error carries the ids of
    /// the chunks already delivered, because "dropped" and "cut short in
    /// the chat" are different outcomes to report.
    ///
    /// The platform answers each send with the sent message, so the ids
    /// come back either way (unit 38, 2026-08-30): the whole send answers
    /// its ids in send order, and a cut-short one carries exactly the ids
    /// that reached the chat on its error. The caller records them; this
    /// module keeps no state about what it sent.
    pub(crate) async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        thread: SendThread,
    ) -> Result<Vec<i64>, SendError> {
        let mut delivered = Vec::new();
        for chunk in chunks_within_cap(text) {
            let threaded = if delivered.is_empty() {
                thread
            } else {
                SendThread::Plain
            };
            match self.send_chunk_threaded(chat_id, chunk, threaded).await {
                Ok(message_id) => delivered.push(message_id),
                Err(error) => return Err(SendError { delivered, error }),
            }
        }
        Ok(delivered)
    }

    /// One chunk's send, with the thread's own recovery where the core
    /// asked for one (2026-08-24): where the platform refuses a send that
    /// carried a reply target, the same text goes out once more without
    /// it.
    ///
    /// An answer must never be lost to the courtesy of threading, and the
    /// tolerance stated on the request covers exactly one cause — a target
    /// the platform can no longer find. Every other cause reaches here: a
    /// target in another chat, a topic the platform will not reply into,
    /// any refusal a future platform version invents. The recovery is
    /// bounded to that one cause and that one attempt, because it is the
    /// thread that failed and not the text: an untargeted send that fails
    /// fails, and a second refusal is returned as the send's own failure.
    ///
    /// It is bounded to the replies the core asked it for, too. A reply
    /// that arrived as [`SendThread::OntoOnly`] is worth nothing without
    /// its thread — the report's line files nothing unless it is a reply —
    /// so its refusal stays the send's failure, and the caller drops it as
    /// it always did. Which replies those are is stated by the core and
    /// read off the thread here; this module never asks what kind of reply
    /// it is holding.
    async fn send_chunk_threaded(
        &self,
        chat_id: i64,
        chunk: &str,
        thread: SendThread,
    ) -> Result<i64, ClientError> {
        let error = match self.send_chunk(chat_id, chunk, thread.target()).await {
            Ok(message_id) => return Ok(message_id),
            Err(error) => error,
        };
        if !thread.plain_when_refused() || !error.is_refusal() {
            return Err(error);
        }
        tracing::warn!(
            chat_id,
            %error,
            "the threaded send was refused; the same text goes out plain"
        );
        self.send_chunk(chat_id, chunk, None).await
    }

    /// One typing action for a chat — the platform's rendering of the
    /// core's composing signal. The platform expires the indicator on its
    /// own after roughly five seconds, so the caller refreshes it on a
    /// named interval while the signal holds. A failure is the caller's to
    /// log and swallow: a presence cue must never disturb anything else.
    /// The ceiling is the cue's own — [`CHAT_ACTION_WAIT_CEILING`], one
    /// refresh period — because the loop must not park behind a stated
    /// wait longer than the cadence it exists to keep.
    pub(crate) async fn send_chat_action(&self, chat_id: i64) -> Result<(), ClientError> {
        let body = serde_json::json!({ "chat_id": chat_id, "action": "typing" });
        let response = self
            .request("sendChatAction", &body, Some(CHAT_ACTION_WAIT_CEILING))
            .await?;
        let _shown: serde_json::Value = self.decode(response).await?;
        Ok(())
    }

    /// Put one emoji on one message — what the core's mark arm maps to
    /// (unit 39, 2026-08-30). The emoji arrives already resolved to a
    /// member of the platform's own reaction list, so this method decides
    /// nothing about which token is legal; it writes the request.
    ///
    /// The request shape is the whole of what a bot may set: a
    /// one-element array of one emoji-typed reaction. No custom-emoji
    /// parameter is built here and none can be — the platform allows one
    /// only conditionally and a bot may not use paid reactions at all, so
    /// the shape simply has no place for either.
    ///
    /// A failure is the caller's to log and drop: a group that restricted
    /// its reactions, a permission switched off, a service message that
    /// cannot be decorated, a deleted target. Nothing retries, and nothing
    /// falls back to a text message — the whole point of the reaction is
    /// that it costs no message. The ceiling is [`REACTION_WAIT_CEILING`],
    /// so a flood-controlled reaction is dropped at once rather than
    /// parking the answers queued behind it.
    pub(crate) async fn set_message_reaction(
        &self,
        chat_id: i64,
        message_id: i64,
        emoji: &str,
    ) -> Result<(), ClientError> {
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reaction": [{ "type": "emoji", "emoji": emoji }],
        });
        let response = self
            .request("setMessageReaction", &body, Some(REACTION_WAIT_CEILING))
            .await?;
        let _placed: serde_json::Value = self.decode(response).await?;
        Ok(())
    }

    /// One chunk's `sendMessage` call, answering the platform's own id for
    /// the message it became.
    async fn send_chunk(
        &self,
        chat_id: i64,
        chunk: &str,
        reply_to: Option<i64>,
    ) -> Result<i64, ClientError> {
        match self
            .send_body(chat_id, &crate::formatting::to_html(chunk), true, reply_to)
            .await
        {
            Err(ClientError::Refused { description }) if Self::rejects_formatting(&description) => {
                // The formatting was refused, so the formatting is what goes.
                // An answer arriving with its asterisks showing is a blemish;
                // an answer not arriving is the bug this fallback exists for,
                // and the converter cannot be trusted absolutely against
                // prose nobody has seen yet.
                tracing::warn!(
                    chat_id,
                    %description,
                    "the formatted send was refused; the same text goes out unformatted"
                );
                self.send_body(chat_id, chunk, false, reply_to).await
            }
            other => other,
        }
    }

    /// One `sendMessage`, formatted or plain, answering the id the platform
    /// gave the message.
    ///
    /// The answer to a send IS the sent message, so the id costs no second
    /// call (unit 38, 2026-08-30). It is read strictly: an answer the
    /// platform called a success while naming no message is an anomaly, and
    /// failing the send says so rather than recording a delivery under an
    /// invented id. The failure is a decode failure, which is not a refusal,
    /// so nothing re-sends the text on it.
    async fn send_body(
        &self,
        chat_id: i64,
        text: &str,
        formatted: bool,
        reply_to: Option<i64>,
    ) -> Result<i64, ClientError> {
        let mut body = serde_json::json!({ "chat_id": chat_id, "text": text });
        if formatted {
            body["parse_mode"] = serde_json::json!("HTML");
        }
        if let Some(message_id) = reply_to {
            body["reply_parameters"] = serde_json::json!({
                "message_id": message_id,
                "allow_sending_without_reply": true,
            });
        }
        let response = self
            .request("sendMessage", &body, Some(MAX_RATE_LIMIT_WAIT))
            .await?;
        let sent: Sent = self.decode(response).await?;
        Ok(sent.message_id)
    }

    /// One chat's administrator list, statuses included.
    pub(crate) async fn chat_administrators(
        &self,
        chat_id: i64,
    ) -> Result<Vec<ChatMember>, ClientError> {
        let body = serde_json::json!({ "chat_id": chat_id });
        let response = self.request("getChatAdministrators", &body, None).await?;
        self.decode(response).await
    }

    /// Whether a refusal names the formatting rather than the message. The
    /// API answers a malformed entity with a parse complaint; every other
    /// refusal — a blocked bot, a chat that is gone, a member who left — is
    /// about the send itself, and retrying it unformatted would only fail
    /// again, more slowly.
    fn rejects_formatting(description: &str) -> bool {
        let lowered = description.to_ascii_lowercase();
        lowered.contains("parse entities")
            || lowered.contains("can't parse")
            || lowered.contains("cannot parse")
            || lowered.contains("unsupported start tag")
            || lowered.contains("unmatched end tag")
    }

    /// One method call under the rate-limit contract, which binds every
    /// endpoint: a rate-limited answer hands the stated wait to the
    /// injectable sleep and the call retries, up to
    /// [`RATE_LIMIT_ATTEMPTS`] attempts in total; past the bound the call
    /// fails with [`ClientError::RateLimitedOut`]. The ceiling applies only
    /// where a caller asks for it: the send holds a queue of pending
    /// replies behind it, the leave call and the first-contact lookup
    /// run inside the sequential update batch, and the typing action keeps
    /// a refresh cadence — so for those a stated wait past the caller's
    /// ceiling fails the call at once with
    /// [`ClientError::RateLimitWaitOverCeiling`]. The callers that park
    /// nothing honor whatever the limiter states: the identity fetch and
    /// the poll, which run ahead of any batch with nothing queued behind
    /// them, and the administrator fetch, whose failure leaves the message
    /// authority-unresolved — for an admitted chat the core's transient
    /// refusal halts the batch on it — so waiting the stated time is
    /// strictly better than failing, and re-asking early would amplify the
    /// very load being limited.
    async fn request(
        &self,
        method: &str,
        body: &serde_json::Value,
        wait_ceiling: Option<Duration>,
    ) -> Result<reqwest::Response, ClientError> {
        for attempt in 1..=RATE_LIMIT_ATTEMPTS {
            let response = self.post(method, body).await?;
            if response.status().as_u16() != TOO_MANY_REQUESTS {
                return Ok(response);
            }
            let wait = self.stated_wait(response).await;
            if wait_ceiling.is_some_and(|ceiling| wait > ceiling) {
                return Err(ClientError::RateLimitWaitOverCeiling {
                    stated_seconds: wait.as_secs(),
                });
            }
            if attempt < RATE_LIMIT_ATTEMPTS {
                (self.sleep)(wait).await;
            }
        }
        Err(ClientError::RateLimitedOut)
    }

    /// One method call's request, built on the root and the token. The URL
    /// exists only inside this function and the transport; a failure leaves
    /// as a redacted detail string.
    async fn post(
        &self,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, ClientError> {
        self.http
            .post(format!("{}/bot{}/{method}", self.root, self.token))
            .json(body)
            .send()
            .await
            .map_err(|error| ClientError::Transport {
                detail: self.redact(error),
            })
    }

    /// The wait a rate-limit reply states, or the fallback when the body
    /// does not decode — a malformed refusal must not retry instantly.
    async fn stated_wait(&self, response: reqwest::Response) -> Duration {
        let stated = response
            .json::<Envelope<serde_json::Value>>()
            .await
            .ok()
            .and_then(|envelope| envelope.parameters)
            .and_then(|parameters| parameters.retry_after);
        stated.map_or(FALLBACK_RETRY_WAIT, Duration::from_secs)
    }

    /// Decode a non-rate-limited answer: reject failure statuses, unwrap the
    /// envelope, refuse `ok: false`.
    async fn decode<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ClientError> {
        let status = response.status();
        if !status.is_success() {
            return Err(ClientError::Status {
                status: status.as_u16(),
            });
        }
        let envelope: Envelope<T> = response.json().await.map_err(|error| ClientError::Decode {
            detail: self.redact(error),
        })?;
        if !envelope.ok {
            return Err(ClientError::Refused {
                description: envelope
                    .description
                    .unwrap_or_else(|| "no description given".into()),
            });
        }
        envelope.result.ok_or_else(|| ClientError::Decode {
            detail: "an ok answer carried no result".into(),
        })
    }

    /// A transport error's text with the token unable to appear: the URL is
    /// stripped from the error, and any remaining occurrence of the token is
    /// replaced — a second protection, because this string reaches logs.
    fn redact(&self, error: reqwest::Error) -> String {
        let detail = error.without_url().to_string();
        detail.replace(&self.token, "[redacted]")
    }
}

/// The text in chunks the platform accepts: each at most
/// [`MESSAGE_UTF16_UNIT_LIMIT`] UTF-16 code units, split on character
/// boundaries. Empty text yields no chunk — the core never finalizes an
/// empty answer, and the platform would refuse one.
fn chunks_within_cap(text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut units = 0;
    for (position, character) in text.char_indices() {
        let width = character.len_utf16();
        if units + width > MESSAGE_UTF16_UNIT_LIMIT {
            chunks.push(&text[start..position]);
            start = position;
            units = 0;
        }
        units += width;
    }
    if start < text.len() {
        chunks.push(&text[start..]);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decoder's own convention, stated once here and followed by every
    /// pin below (unit 31, 2026-08-28): **this module's model is proven
    /// against RAW PLATFORM JSON, decoded through serde.**
    ///
    /// Everywhere else in this adapter an update is a struct the test
    /// builds — which proves what translation does with a field, and
    /// nothing at all about whether that field ever arrives. A renamed
    /// key, a wrong shape or a missing `serde` attribute is invisible to a
    /// struct-built fixture and silently degrades a live message to its
    /// absent value. So the wire's own shape is pinned where the wire is
    /// owned.
    ///
    /// The quoted part decodes its two fields, and only those: the
    /// excerpt's UTF-16 offset rides in the same payload and is
    /// deliberately not represented at all, so no conversion of it can
    /// ever be attempted.
    #[test]
    fn the_quoted_part_decodes_from_the_platforms_own_payload() {
        let update: Update = serde_json::from_value(serde_json::json!({
            "update_id": 41,
            "message": {
                "message_id": 1041,
                "date": 1_700_000_000,
                "chat": { "id": -100, "type": "supergroup" },
                "from": { "id": 7, "first_name": "Person 7" },
                "text": "that one is the problem",
                "reply_to_message": {
                    "message_id": 1040,
                    "date": 1_699_999_999,
                    "chat": { "id": -100, "type": "supergroup" },
                    "from": { "id": 9, "first_name": "Person 9" },
                    "text": "the text font is tiring my eyes"
                },
                "quote": {
                    "text": "the text font",
                    "position": 0,
                    "is_manual": true
                }
            }
        }))
        .expect("the platform's reply-with-quote payload decodes");

        let message = update.message.expect("the update carries a message");
        let quote = message.quote.expect("the quoted part decodes");
        assert_eq!(quote.text.as_deref(), Some("the text font"));
        assert!(quote.is_manual, "the hand-selected flag decodes");
    }

    /// The absent and the degraded shapes, decoded: a reply with no quoted
    /// part carries none, an unflagged quoted part reads as not
    /// hand-selected, and a quoted part without text decodes to a part with
    /// no text instead of refusing the whole update batch.
    #[test]
    fn a_missing_or_partial_quoted_part_degrades_instead_of_refusing() {
        let decode = |quote: serde_json::Value| -> Incoming {
            serde_json::from_value(serde_json::json!({
                "message_id": 1042,
                "date": 1_700_000_000,
                "chat": { "id": -100, "type": "supergroup" },
                "from": { "id": 7, "first_name": "Person 7" },
                "text": "a reply",
                "quote": quote,
            }))
            .expect("the message decodes")
        };

        let plain: Incoming = serde_json::from_value(serde_json::json!({
            "message_id": 1043,
            "date": 1_700_000_000,
            "chat": { "id": -100, "type": "supergroup" },
            "from": { "id": 7, "first_name": "Person 7" },
            "text": "a plain reply",
        }))
        .expect("a message without a quoted part decodes");
        assert!(plain.quote.is_none(), "no quoted part, nothing carried");

        let unflagged = decode(serde_json::json!({ "text": "an excerpt", "position": 4 }));
        let unflagged = unflagged.quote.expect("the quoted part decodes");
        assert_eq!(unflagged.text.as_deref(), Some("an excerpt"));
        assert!(
            !unflagged.is_manual,
            "the platform states the flag only when it holds; absent reads false"
        );

        let textless = decode(serde_json::json!({ "position": 4, "is_manual": true }));
        let textless = textless.quote.expect("the quoted part still decodes");
        assert_eq!(textless.text, None, "no text, and no refused batch");
    }

    /// The bot flag decodes off the platform's own payload (2026-08-30), on
    /// the sender of a message and on a joiner alike: the platform states
    /// it on every account object, and an absent flag reads false — the
    /// wire's own meaning, since only the platform asserts that an account
    /// is automated.
    #[test]
    fn the_bot_flag_decodes_on_a_sender_and_on_a_joiner() {
        let update: Update = serde_json::from_value(serde_json::json!({
            "update_id": 42,
            "message": {
                "message_id": 1042,
                "date": 1_700_000_000,
                "chat": { "id": -100, "type": "supergroup" },
                "from": {
                    "id": 9,
                    "is_bot": true,
                    "first_name": "Moderation",
                    "username": "rose_bot"
                },
                "text": "solve the captcha to stay",
                "new_chat_members": [
                    { "id": 11, "is_bot": false, "first_name": "Ada" },
                    { "id": 12, "is_bot": true, "first_name": "Helper" }
                ]
            }
        }))
        .expect("the platform's bot-sent payload decodes");

        let message = update.message.expect("the update carries a message");
        let from = message.from.expect("the sender decodes");
        assert!(from.is_bot, "the sender's own flag decodes");
        let joiners = message.new_chat_members.expect("the joiner list decodes");
        assert_eq!(
            joiners
                .iter()
                .map(|joiner| joiner.is_bot)
                .collect::<Vec<_>>(),
            vec![false, true],
            "each joiner carries its own flag"
        );

        let plain: Incoming = serde_json::from_value(serde_json::json!({
            "message_id": 1043,
            "date": 1_700_000_000,
            "chat": { "id": -100, "type": "supergroup" },
            "from": { "id": 7, "first_name": "Person 7" },
            "text": "a member's message",
            "new_chat_members": [{ "id": 13, "first_name": "Grace" }]
        }))
        .expect("a payload without the flag decodes");
        assert!(
            !plain.from.expect("the sender decodes").is_bot,
            "an absent flag reads false on a sender"
        );
        assert!(
            !plain.new_chat_members.expect("the joiner list decodes")[0].is_bot,
            "an absent flag reads false on a joiner"
        );
    }
}
