//! Telegram adapter for the halogenOS Group Assistant: translates between the
//! Telegram Bot API and the core's message model, in both directions.
//!
//! Invariant: an adapter contains no behavior. Decisions about what the
//! assistant says or does belong to the core; this crate only converts
//! representations and moves messages. It speaks the Bot API directly —
//! updates in, plain sends out (decision 0013) — and consumes exactly
//! the core's public edges: the ingestion entry point, the outbound
//! subscription, and nothing deeper.
//!
//! Updates arrive by one of two answering modes, chosen by one predicate
//! (2026-08-29): with a [`WebhookConfig`] the adapter registers a public
//! address with the platform and serves the deliveries on a loopback
//! listener; without one it long-polls, exactly as it always did. Both feed
//! the same per-update step, and nothing past that step knows which one
//! brought an update.
//!
//! The embedder contract is one constructor, one run entry, and one
//! startup identity read. The configuration is the bot token, the API
//! root, the state-file path, the assistant's resolved name and the optional
//! webhook wiring — the name is a translation input for the wake trigger,
//! never behavior; the adapter's registered name is the pinned constant
//! [`ADAPTER_NAME`], because it keys channel mappings and principals durably
//! and is therefore a permanent contract, not a parameter. The token appears
//! in no log line and no error string anywhere in this crate, and neither
//! does the webhook secret, which no configuration carries at all: the
//! adapter generates it and keeps it beside its state file.

mod authority;
mod client;
mod driver;
mod formatting;
mod state;
mod translate;
mod webhook;

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

pub use webhook::{WebhookAddress, WebhookAddressError};

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
    /// The webhook wiring, and the one predicate that decides how updates
    /// arrive: `Some` registers the address and serves the deliveries,
    /// `None` long-polls. There is no third state and no partial one — a
    /// deployment either has a public door or it does not.
    pub webhook: Option<WebhookConfig>,
}

/// Where the listener reports the address it bound, called once, after the
/// bind and before the registration. The test seam beside [`Sleep`], not a
/// deployment decision: a suite that asked for an ephemeral port learns the
/// port it got here, because the listener is the only place that knows.
pub type BoundListener = Arc<dyn Fn(SocketAddr) + Send + Sync>;

/// The webhook intake's wiring: where the platform is told to deliver, and
/// which loopback port the listener binds. Two deployment decisions and
/// nothing else.
///
/// The address carries the path the listener answers, so the address called
/// and the path served are one recorded value. There is no secret here:
/// the adapter generates its own, keeps it beside the state file, and no
/// human ever handles it.
#[derive(Clone, Debug)]
pub struct WebhookConfig {
    /// The public address the platform calls — HTTPS, terminated by whatever
    /// sits in front of the listener.
    pub address: WebhookAddress,
    /// The loopback port the listener binds. Any port is accepted here,
    /// including zero for an ephemeral one; a deployment's own configuration
    /// is where a port is a contract with a reverse proxy.
    pub listen_port: u16,
}

/// Where the webhook intake keeps its secret: beside the given state file,
/// under that name plus a fixed suffix. One derivation, exported because a
/// deployment inspecting its own state directory — and the suite pinning the
/// file — must read the same path the adapter writes, never a second copy of
/// the rule.
#[must_use]
pub fn webhook_secret_path(state_file: &std::path::Path) -> PathBuf {
    state::secret_path(state_file)
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
            webhook: None,
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
            .field("webhook", &self.webhook)
            .finish()
    }
}

/// What the run entry can fail with. Neither answering mode returns an error
/// once it runs: a network failure backs off and re-polls, a failed ingest
/// redelivers, a failed send is bounded, logged and dropped, and a refused
/// delivery is a status code — so every failure that escapes is one from
/// before the updates start arriving.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Taking the outbound edge from the core failed; without it every
    /// answer would be lost, so the adapter refuses to start.
    #[error("the outbound edge could not be taken: {0}")]
    Core(#[from] CoreError),

    /// The startup identity read failed: the platform did not answer the
    /// one call the display-name default needs — and, in webhook mode, the
    /// one call the translation cannot proceed without. The text carries the
    /// wire failure's own rendering, never the token.
    #[error("the platform identity read failed: {0}")]
    Identity(String),

    /// The webhook listener could not bind its loopback port, so nothing
    /// would answer the address the platform is about to be told to call.
    /// The port is named because a port already taken is what this usually
    /// is.
    #[error("the webhook listener could not bind port {port}: {detail}")]
    Listener { port: u16, detail: String },

    /// The webhook registration was refused, so the platform would deliver
    /// nowhere. A deployment that silently cannot register would sit deaf,
    /// which is the outage the webhook intake exists to end. The text
    /// carries the platform's own rendering with the secret scrubbed out.
    #[error("the webhook address could not be registered: {0}")]
    Registration(String),

    /// No webhook secret could be kept: the operating system's randomness
    /// did not read, or the generated secret did not persist. A door without
    /// a secret is one anybody who finds the path could feed updates
    /// through, and a secret that cannot be written is one the next start
    /// would replace, breaking the delivery authentication across a restart.
    #[error("the webhook secret could not be kept: {0}")]
    Secret(String),

    /// The core stated that nothing it is asked from here on can succeed —
    /// its database is damaged, is not a database, or its one writer is gone
    /// (2026-09-01). Serving on would spin the intake against a store that
    /// can never answer, so the run ends, the process exits, and the
    /// supervisor starts a replacement against the durable state. What the
    /// core said is in the log line the run made where it stopped; there is
    /// nothing to add here that a member's message could not appear in.
    #[error("the core cannot serve any message; the run ended for a restart")]
    CoreCannotServe,
}

/// The Telegram adapter, ready to run against a started assembly.
pub struct TelegramAdapter {
    config: Config,
    sleep: Sleep,
    announce_bound: Option<BoundListener>,
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
        Self {
            config,
            sleep,
            announce_bound: None,
        }
    }

    /// The second test seam, beside the sleep: the same adapter, reporting
    /// the address its webhook listener bound. A deployment names its own
    /// port and never asks; a suite that asked for an ephemeral port has no
    /// other way to learn the port it got. It is a property of this adapter
    /// instance and not of [`WebhookConfig`], which carries the deployment's
    /// decisions alone.
    #[must_use]
    pub fn announcing_bound(mut self, announce: BoundListener) -> Self {
        self.announce_bound = Some(announce);
        self
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

    /// The run entry: take the outbound edge, then take in updates —
    /// polled or delivered, per the configuration — translate, ingest and
    /// send until the assembly goes away.
    ///
    /// The edge is taken before the first update on purpose — the core treats
    /// answers stored before the subscription as history, so the order is
    /// part of the contract. The intake and the outbound consumer run
    /// concurrently in this one future; the consumer is sequential, so a
    /// send's bounded retry wait holds later replies back — accepted at this
    /// unit's traffic.
    ///
    /// Returns only when the core drops the outbound edge, which means the
    /// assembly is gone and there is nothing left to serve, or when the
    /// webhook intake's own halves end.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Core`] if the outbound edge cannot be taken, and — in
    /// webhook mode, where a start that cannot serve must not be quiet —
    /// [`AdapterError::Identity`], [`AdapterError::Secret`],
    /// [`AdapterError::Listener`] or [`AdapterError::Registration`] when the
    /// identity, the secret, the port or the registration refuses.
    pub async fn run(self, assistant: Arc<Assistant>) -> Result<(), AdapterError> {
        driver::run(self.config, self.sleep, self.announce_bound, assistant).await
    }
}
