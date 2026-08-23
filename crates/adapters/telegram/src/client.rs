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

/// The HTTP status the platform rate-limits with.
const TOO_MANY_REQUESTS: u16 = 429;

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

/// A send that did not deliver its whole reply. The delivered count exists
/// so the caller can state what actually happened: zero means the reply was
/// dropped whole, more means the chat holds the reply's head and the tail
/// was dropped — two different outcomes a log must not conflate.
#[derive(Debug)]
pub(crate) struct SendError {
    /// How many chunks reached the chat before the failing one.
    pub delivered_chunks: usize,
    /// What the failing chunk's request failed with.
    pub error: ClientError,
}

/// The update types the poll consumes, named explicitly on every poll: an
/// absent selection inherits whatever an earlier setting left on the token,
/// so the selection is stated instead of assumed. Messages and their edits,
/// and the assistant's own membership updates.
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
    /// The pinned message a pin service note points at, reduced to what the
    /// pin observation reads. Present exactly on the pin service message.
    pub pinned_message: Option<PinnedContent>,
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

/// The replied-to message, reduced to what the adapter reads: its author —
/// addressing asks whether that author is the bot itself — and its own
/// message id, which translation stores as the reply target (2026-08-23).
/// The id decodes leniently: a reply without a usable id stores no target.
#[derive(Debug, Deserialize)]
pub(crate) struct RepliedTo {
    pub from: Option<User>,
    pub message_id: Option<i64>,
}

/// The chat a message lives in: its id and its platform type string.
#[derive(Debug, Deserialize)]
pub(crate) struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
}

/// A sending person's identity fields.
#[derive(Debug, Deserialize)]
pub(crate) struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

/// The bot's own identity, from `getMe`: what mention and reply-to-self
/// resolution compare against. Fetched before the first poll; no message is
/// translated without it.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BotIdentity {
    /// The bot's own user id — what a reply's author is compared to.
    pub id: i64,
    /// The bot's username — what a mention names. The platform requires one
    /// for every bot, but the decoder keeps it optional so a malformed
    /// answer degrades to mention-blindness instead of refusing to decode.
    pub username: Option<String>,
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

/// The thin Bot API client: three methods over two endpoints and the
/// administrator list, with every HTTP concern kept inside.
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

    /// Send one reply's text to its chat, threaded onto `reply_to` where
    /// one is given. Text past the platform's message cap goes out as
    /// consecutive chunks, per decision 0019: the cap is the platform's,
    /// and dropping or truncating the reply instead would lose the answer.
    /// Only the first chunk threads (2026-08-23): a thread is one answer,
    /// and the platform shows the continuation chunks right under it. The
    /// reply travels as the platform's current reply parameters — the old
    /// reply field was replaced two platform versions ago — with
    /// send-without-reply tolerance, so a deleted target degrades to a
    /// plain send. A chunk that fails ends the reply there: sending the
    /// tail after a lost middle would deliver a spliced statement, so the
    /// caller drops the rest with it — and the error carries how many
    /// chunks were already delivered, because "dropped" and "cut short in
    /// the chat" are different outcomes to report.
    pub(crate) async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<(), SendError> {
        for (delivered_chunks, chunk) in chunks_within_cap(text).into_iter().enumerate() {
            let threaded = if delivered_chunks == 0 {
                reply_to
            } else {
                None
            };
            if let Err(error) = self.send_chunk(chat_id, chunk, threaded).await {
                return Err(SendError {
                    delivered_chunks,
                    error,
                });
            }
        }
        Ok(())
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

    /// One chunk's `sendMessage` call.
    async fn send_chunk(
        &self,
        chat_id: i64,
        chunk: &str,
        reply_to: Option<i64>,
    ) -> Result<(), ClientError> {
        let mut body = serde_json::json!({ "chat_id": chat_id, "text": chunk });
        if let Some(message_id) = reply_to {
            body["reply_parameters"] = serde_json::json!({
                "message_id": message_id,
                "allow_sending_without_reply": true,
            });
        }
        let response = self
            .request("sendMessage", &body, Some(MAX_RATE_LIMIT_WAIT))
            .await?;
        let _sent: serde_json::Value = self.decode(response).await?;
        Ok(())
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
