//! Telegram adapter for the halogenOS Group Assistant: translates between the
//! Telegram Bot API and the core's message model, in both directions.
//!
//! Invariant: an adapter contains no behavior. Decisions about what the
//! assistant says or does belong to the core; this crate only converts
//! representations and moves messages. It speaks the Bot API directly —
//! long polling in, plain sends out (decision 0013) — and consumes exactly
//! the core's public edges: the ingestion entry point, the outbound
//! subscription, and nothing deeper.
//!
//! The embedder contract is one constructor, one run entry, and one
//! startup identity read. The configuration is the bot token, the API
//! root, the state-file path and the assistant's resolved name — the
//! name is a translation input for the wake trigger, never behavior; the
//! adapter's registered name is the pinned constant
//! [`ADAPTER_NAME`], because it keys channel mappings and principals durably
//! and is therefore a permanent contract, not a parameter. The token appears
//! in no log line and no error string anywhere in this crate.

mod authority;
mod client;
mod driver;
mod state;
mod translate;

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use assistant_core::{Assistant, CoreError};

/// The adapter's registered name. It keys the channel mappings and the
/// principals in the core's durable tables, so it is a permanent contract:
/// changing it would orphan every mapped channel and every recorded person.
pub const ADAPTER_NAME: &str = "telegram";

/// The Bot API host the configuration defaults to. Tests supply a loopback
/// server's address instead, so no test traffic leaves the machine.
pub const BOT_API_ROOT: &str = "https://api.telegram.org";

/// An awaitable pause, injectable so tests pin a wait's length without
/// waiting it out. The client hands the rate-limit retry wait through this,
/// and the poll loop hands its backoff through it, so a suite never sleeps
/// for real.
pub type Sleep = Arc<dyn Fn(Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// The adapter's whole configuration: the token, where the API lives,
/// where the update offset is persisted, and the assistant's resolved name
/// as one more addressing input. The adapter's registered name stays the
/// pinned constant, and every HTTP detail lives in the client.
#[derive(Clone)]
pub struct Config {
    /// The bot token. It authenticates every request and appears in no
    /// logged output and no error text.
    pub token: String,
    /// The API root requests are built on, [`BOT_API_ROOT`] unless a test
    /// points it at a loopback server.
    pub api_root: String,
    /// Where the next update offset is persisted, per decision 0014. An
    /// absent, empty or malformed file is treated as absent and the
    /// redelivered updates are the accepted duplicates.
    pub state_file: PathBuf,
    /// The assistant's resolved name (unit 14): one more input to the
    /// addressing translation — a group message naming the assistant
    /// addresses it, beside the mention and the reply. Translation input,
    /// not behavior: what the name IS the embedder decided; this crate only
    /// matches it, whole-word and case-insensitive, and a name that cannot
    /// form a clean trigger word falls back to mention-and-reply, logged.
    /// `None` translates without the name trigger.
    pub name: Option<String>,
}

impl Config {
    /// A configuration against the real Bot API host, without a name
    /// trigger; the embedder sets [`Config::name`] once the name is
    /// resolved.
    pub fn new(token: impl Into<String>, state_file: impl Into<PathBuf>) -> Self {
        Self {
            token: token.into(),
            api_root: BOT_API_ROOT.into(),
            state_file: state_file.into(),
            name: None,
        }
    }
}

// The token must not leak through a derived or default debug representation;
// spelling the implementation out keeps the redaction in one visible place.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("token", &"[redacted]")
            .field("api_root", &self.api_root)
            .field("state_file", &self.state_file)
            .field("name", &self.name)
            .finish()
    }
}

/// What the run entry can fail with. The loop itself never returns an error:
/// a network failure backs off and re-polls, a failed ingest redelivers, a
/// failed send is bounded, logged and dropped — so the only failure that
/// escapes is the one before the loop starts.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Taking the outbound edge from the core failed; without it every
    /// answer would be lost, so the adapter refuses to start.
    #[error("the outbound edge could not be taken: {0}")]
    Core(#[from] CoreError),

    /// The startup identity read failed: the platform did not answer the
    /// one call the display-name default needs. The text carries the wire
    /// failure's own rendering, never the token.
    #[error("the platform identity read failed: {0}")]
    Identity(String),
}

/// The Telegram adapter, ready to run against a started assembly.
pub struct TelegramAdapter {
    config: Config,
    sleep: Sleep,
}

impl TelegramAdapter {
    /// The constructor: an adapter that waits with the runtime's own timer.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self::with_sleep(
            config,
            Arc::new(|wait| Box::pin(tokio::time::sleep(wait)) as Pin<Box<_>>),
        )
    }

    /// The test seam the client contract names: the same adapter, with every
    /// wait — the rate-limit retry and the poll backoff — handed to the
    /// given sleep instead of the runtime's timer, so a suite pins the waits
    /// without waiting them out.
    #[must_use]
    pub fn with_sleep(config: Config, sleep: Sleep) -> Self {
        Self { config, sleep }
    }

    /// The one startup identity read the embedder performs when no name is
    /// configured: the bot's own display name, from the platform. One
    /// attempt, no retry — the embedder decides whether a failure refuses
    /// the start — and `None` when the platform's answer carries no
    /// display name.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Identity`] when the platform call fails.
    pub async fn read_display_name(config: &Config) -> Result<Option<String>, AdapterError> {
        let sleep: Sleep = Arc::new(|wait| Box::pin(tokio::time::sleep(wait)) as Pin<Box<_>>);
        let client = client::BotClient::new(&config.api_root, &config.token, sleep);
        let me = client
            .get_me()
            .await
            .map_err(|error| AdapterError::Identity(error.to_string()))?;
        Ok(me.first_name)
    }

    /// The run entry: take the outbound edge, then long-poll, translate,
    /// ingest and send until the assembly goes away.
    ///
    /// The edge is taken before the first poll on purpose — the core treats
    /// answers stored before the subscription as history, so the order is
    /// part of the contract. The polling loop and the outbound consumer run
    /// concurrently in this one future; the consumer is sequential, so a
    /// send's bounded retry wait holds later replies back — accepted at this
    /// unit's traffic.
    ///
    /// Returns only when the core drops the outbound edge, which means the
    /// assembly is gone and there is nothing left to serve.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Core`] if the outbound edge cannot be taken.
    pub async fn run(self, assistant: Arc<Assistant>) -> Result<(), AdapterError> {
        driver::run(self.config, self.sleep, assistant).await
    }
}
