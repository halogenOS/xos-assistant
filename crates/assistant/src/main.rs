//! The runnable assistant: one process assembling the core, the `OpenRouter`
//! provider and the Telegram adapter.
//!
//! The binary takes exactly one argument, the configuration file's path. It
//! reads the configuration, resolves the two secrets through their
//! indirections, loads the system prompt from the prompt directory, opens the
//! store with the assistant's schema, and runs the adapter until SIGTERM —
//! the run future is selected against the signal and abandoned, so an
//! in-flight send may be cut short, an accepted cost of a prompt stop.
//!
//! Startup facts are logged; secrets never are. A configuration the process
//! cannot read, decode or resolve exits nonzero before anything starts.

mod config;
mod prompt;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use agent_ledger::{EventBus, ProviderModule, ProviderRegistry, Store};
use assistant_adapter_telegram::{AdapterError, TelegramAdapter};
use assistant_core::provider::MemoryConfiguredProvider;
use assistant_core::schema::store_config;
use assistant_core::tools::commit;
use assistant_core::tools::release;
use assistant_core::tools::wiki;
use assistant_core::tools::{LookupEndpoints, ToolSet};
use assistant_core::{Assistant, CoreError, ModelBinding};
use tokio::signal::unix::{SignalKind, signal};

use crate::config::{Configuration, ConsoleStream, LogDestination};

/// Everything that can end the process with a failure. Every variant's text
/// names locations and causes, never a secret's value.
#[derive(Debug, thiserror::Error)]
enum StartError {
    /// The binary was invoked with anything but the one expected argument.
    #[error("usage: assistant <configuration file>")]
    Usage,

    /// The configuration file could not be read at all.
    #[error("the configuration file {path} could not be read: {error}")]
    ConfigurationUnread {
        path: PathBuf,
        error: std::io::Error,
    },

    /// The configuration file's content does not decode. The refusal points
    /// at the failing place and never repeats the file's text: a decode
    /// error's own prose can quote the offending line whole, and a secret
    /// pasted inline where its indirection belongs must not reach stderr.
    #[error("the configuration file {path} does not decode, {location}")]
    ConfigurationInvalid { path: PathBuf, location: String },

    /// A protection field configures zero answers — an assistant that
    /// answers no one. Disabling a budget is a zero window, not a zero
    /// count.
    #[error(
        "the protection field `{field}` must be at least 1; set the budget's \
         window to 0 to disable it instead"
    )]
    ProtectionAnswersZero { field: &'static str },
    /// A protection window is longer than the supported year. Far past it,
    /// the database's date arithmetic would return null and silently admit
    /// everything — the bound keeps the failure at the parse, loudly.
    #[error(
        "the protection field `{field}` must be at most one year          (31536000 seconds); longer windows are refused"
    )]
    ProtectionWindowOverBound { field: &'static str },

    /// An operators entry carries an empty external id. An empty value
    /// would match no adder and silently refuse every group add, so the
    /// line refuses the start instead.
    #[error("the operators entry for `{adapter}` must carry the operator's external id")]
    OperatorEmpty { adapter: String },
    /// An operators key names no adapter this binary assembles. A typoed
    /// key would sit inert while the intended adapter refuses every group
    /// add, so the line refuses the start instead.
    #[error("the operators key `{adapter}` names no known adapter")]
    OperatorUnknownAdapter { adapter: String },

    /// The `privacy_policy` key is present but empty or whitespace — a blank
    /// legal pointer. Omitting the key is how the not-yet-published answer
    /// is chosen.
    #[error(
        "the privacy_policy key must carry an address; omit it to leave the policy unpublished"
    )]
    PrivacyPolicyEmpty,

    /// The `moderation_handle` key is present but empty after trimming and
    /// stripping a leading `@`. Omitting the key is how a deployment
    /// without the report tool is chosen.
    #[error(
        "the moderation_handle key must carry the moderation bot's handle; omit it to \
         leave the report tool unregistered"
    )]
    ModerationHandleEmpty,

    /// The wiki endpoint override is present but empty or whitespace.
    /// Omitting the key is how the real host is chosen.
    #[error("the endpoints.wiki key must carry an address; omit it for the real host")]
    WikiEndpointEmpty,

    /// The `title_model` key is present but empty or whitespace. Omitting
    /// the key is how titles derive on the main model.
    #[error(
        "the title_model key must carry a model id; omit it to derive titles with the main model"
    )]
    TitleModelEmpty,

    /// A secret reference names both sources or neither.
    #[error("the secret `{key}` must name exactly one of `env` or `file`")]
    SecretRef { key: &'static str },

    /// A secret's named source could not be read. The source is named; the
    /// value, present or not, never is.
    #[error("the secret `{key}` could not be read from the {source_name}")]
    SecretUnread {
        key: &'static str,
        source_name: String,
    },

    /// The prompt directory or a file in it could not be read.
    #[error("the prompt directory {dir} could not be read: {error}")]
    PromptUnread { dir: PathBuf, error: std::io::Error },

    /// The prompt directory holds no text at all.
    #[error("the prompt directory {dir} holds no prompt text")]
    PromptEmpty { dir: PathBuf },

    /// The log file could not be opened.
    #[error("the log destination {} could not be opened: {error}", path.display())]
    LogUnopened {
        path: PathBuf,
        error: std::io::Error,
    },

    /// The runtime or the SIGTERM handler could not be set up.
    #[error("the runtime or the SIGTERM handler could not be set up: {0}")]
    Runtime(std::io::Error),

    /// The store could not be opened.
    #[error("the store could not be opened: {0}")]
    Store(#[from] agent_ledger::StoreError),

    /// The core assembly refused to start.
    #[error(transparent)]
    Core(#[from] CoreError),

    /// The adapter refused to start.
    #[error(transparent)]
    Adapter(#[from] AdapterError),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// The whole start sequence, in refusal order: argument, configuration,
/// secrets, prompt, logging, runtime — everything that can refuse does so
/// before the store is touched or a connection is opened.
fn run() -> Result<(), StartError> {
    let mut arguments = std::env::args_os().skip(1);
    let (Some(config_path), None) = (arguments.next(), arguments.next()) else {
        return Err(StartError::Usage);
    };
    let configuration = Configuration::load(config_path.as_ref())?;
    // Resolved right behind the decode: a zero answer count, an empty or
    // unknown-adapter operator entry and a blank privacy address refuse
    // the start here, before any secret is read or a connection is opened.
    let protection = configuration.protection.resolve()?;
    let operators = configuration.operators.resolve(&KNOWN_ADAPTERS)?;
    let privacy_policy = configuration.resolve_privacy_policy()?;
    let moderation_handle = configuration.resolve_moderation_handle()?;
    let wiki_endpoint = configuration.resolve_wiki_endpoint()?;
    let title_model = configuration.resolve_title_model()?;
    let bot_token = configuration.secrets.bot_token.resolve("bot_token")?;
    let openrouter_key = configuration
        .secrets
        .openrouter_key
        .resolve("openrouter_key")?;
    // Optional by policy, not by leniency: a mirror_token entry that IS
    // configured but cannot be read still refuses the start.
    let mirror_token = configuration
        .secrets
        .mirror_token
        .as_ref()
        .map(|reference| reference.resolve("mirror_token"))
        .transpose()?;
    let system_prompt = prompt::load(&configuration.prompt_dir)?;
    init_logging(&configuration.log)?;
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(StartError::Runtime)?
        .block_on(serve(ServeInputs {
            configuration,
            protection,
            operators,
            privacy_policy,
            moderation_handle,
            wiki_endpoint,
            title_model,
            bot_token,
            openrouter_key,
            mirror_token,
            system_prompt,
        }))
}

/// The adapters this binary assembles — what the operators table's keys
/// are validated against, so a typoed key refuses the start instead of
/// silently refusing every group add on the intended adapter.
const KNOWN_ADAPTERS: [&str; 1] = [assistant_adapter_telegram::ADAPTER_NAME];

/// Route log lines to the configured destination. The filter honours the
/// standard environment override and speaks at `info` otherwise.
fn init_logging(destination: &LogDestination) -> Result<(), StartError> {
    use tracing_subscriber::fmt::writer::BoxMakeWriter;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let writer = match destination {
        LogDestination::Console(ConsoleStream::Stderr) => BoxMakeWriter::new(std::io::stderr),
        LogDestination::File(destination) => {
            let path = destination.path();
            let file = std::fs::File::options()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|error| StartError::LogUnopened {
                    path: path.to_path_buf(),
                    error,
                })?;
            BoxMakeWriter::new(std::sync::Mutex::new(file))
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Ok(())
}

/// Everything the resolved start hands the serving future — one carrier,
/// so the resolution order above stays readable as the argument list grows.
struct ServeInputs {
    configuration: Configuration,
    protection: assistant_core::ProtectionConfig,
    operators: assistant_core::OperatorConfig,
    privacy_policy: Option<String>,
    moderation_handle: Option<String>,
    wiki_endpoint: Option<String>,
    title_model: Option<String>,
    bot_token: String,
    openrouter_key: String,
    mirror_token: Option<String>,
    system_prompt: String,
}

/// The lookup hosts the production tool set is pointed at: the configured
/// overrides where present, the real defaults otherwise. The palette every
/// new conversation records names exactly the set built over these.
fn resolved_lookup_endpoints(
    configuration: &Configuration,
    mirror_token: Option<String>,
    wiki_endpoint: Option<&str>,
) -> LookupEndpoints {
    LookupEndpoints {
        forge: configuration
            .endpoints
            .forge
            .clone()
            .unwrap_or_else(|| commit::DEFAULT_BASE_URL.into()),
        mirror: configuration
            .endpoints
            .mirror
            .clone()
            .unwrap_or_else(|| release::DEFAULT_BASE_URL.into()),
        mirror_token,
        wiki: wiki_endpoint.map_or_else(|| wiki::DEFAULT_BASE_URL.into(), ToOwned::to_owned),
    }
}

/// One startup line naming every resolved path and endpoint — never a
/// secret.
fn log_startup(
    configuration: &Configuration,
    wiki_endpoint: Option<&str>,
    title_model: Option<&str>,
) {
    tracing::info!(
        store = %configuration.store_path.display(),
        telegram_state = %configuration.telegram_state_path.display(),
        prompt_dir = %configuration.prompt_dir.display(),
        model = %configuration.model,
        title_model = %title_model.unwrap_or("the main model"),
        telegram_endpoint = %configuration
            .endpoints
            .telegram
            .as_deref()
            .unwrap_or("the real host"),
        openrouter_endpoint = %configuration
            .endpoints
            .openrouter
            .as_deref()
            .unwrap_or("the real host"),
        forge_endpoint = %configuration
            .endpoints
            .forge
            .as_deref()
            .unwrap_or("the real host"),
        mirror_endpoint = %configuration
            .endpoints
            .mirror
            .as_deref()
            .unwrap_or("the real host"),
        wiki_endpoint = %wiki_endpoint.unwrap_or("the real host"),
        "the assistant is up"
    );
}

/// Assemble and serve until SIGTERM or an adapter start refusal.
async fn serve(inputs: ServeInputs) -> Result<(), StartError> {
    let ServeInputs {
        configuration,
        protection,
        operators,
        privacy_policy,
        moderation_handle,
        wiki_endpoint,
        title_model,
        bot_token,
        openrouter_key,
        mirror_token,
        system_prompt,
    } = inputs;
    let store = Store::open_with(&configuration.store_path, store_config())?;
    let provider = MemoryConfiguredProvider::new(
        &store,
        openrouter_key,
        configuration.endpoints.openrouter.clone(),
        title_model.clone(),
    )
    .await;
    let vendor = ProviderModule::type_id(&provider).to_owned();
    let binding = ModelBinding {
        provider_instance: format!("{vendor}-1"),
        provider_display_name: ProviderModule::display_name(&provider).to_owned(),
        vendor,
        model: configuration.model.clone(),
        model_display_name: configuration.model.clone(),
    };
    let mut providers = ProviderRegistry::new();
    providers.register(Box::new(provider));
    let tools = ToolSet::production_lookups(resolved_lookup_endpoints(
        &configuration,
        mirror_token,
        wiki_endpoint.as_deref(),
    ));
    let assistant = Assistant::start(
        store,
        Arc::new(EventBus::new()),
        Arc::new(providers),
        tools,
        assistant_core::AssemblyConfig {
            binding,
            system_prompt,
            protection,
            operators,
            privacy_policy_address: privacy_policy,
            moderation_handle,
        },
    )
    .await?;

    let mut adapter_config = assistant_adapter_telegram::Config::new(
        bot_token,
        configuration.telegram_state_path.clone(),
    );
    if let Some(root) = configuration.endpoints.telegram.clone() {
        adapter_config.api_root = root;
    }
    let adapter = TelegramAdapter::new(adapter_config);

    log_startup(
        &configuration,
        wiki_endpoint.as_deref(),
        title_model.as_deref(),
    );

    let mut sigterm = signal(SignalKind::terminate()).map_err(StartError::Runtime)?;
    tokio::select! {
        _ = sigterm.recv() => {
            tracing::info!("SIGTERM received; stopping");
            Ok(())
        }
        outcome = adapter.run(Arc::new(assistant)) => Ok(outcome?),
    }
}
