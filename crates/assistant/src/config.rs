//! The process configuration: one TOML file, located by the binary's single
//! command-line argument.
//!
//! The file names paths, the model id and the endpoint overrides. Secrets are
//! named indirectly — an environment variable name or a file path per secret —
//! and their values never appear in the configuration file, in any error text,
//! or in any log line. Every error below names where a value was looked for,
//! never what was found.

use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};

use assistant_core::tools::search::{self, SearchConfig};
use assistant_core::{
    AnsweringMode, Budget, DirectChats, OperatorConfig, ProtectionConfig, ReasoningLevel,
};
use serde::Deserialize;

use crate::StartError;

/// Everything the process reads from its configuration file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    /// Where the store lives. Created on first start.
    pub store_path: PathBuf,
    /// Where the Telegram update offset is persisted.
    pub telegram_state_path: PathBuf,
    /// The directory holding the system prompt files.
    pub prompt_dir: PathBuf,
    /// Where log lines go.
    pub log: LogDestination,
    /// The provider's identifier for the model every conversation is
    /// created under. There is no title-model key beside it (decision
    /// 0077): title derivation is off in the assembly, and a config key
    /// naming a model for it would decode into dead configuration — a
    /// leftover `title_model` line is refused at the load like every other
    /// unknown key, so a stale file fails loudly instead of implying a
    /// feature that no longer exists.
    pub model: String,
    /// How many tokens the model's context window holds (unit 48,
    /// 2026-08-31). No provider reports it, so it is stated here beside the
    /// model it belongs to; a zero is refused by the type.
    ///
    /// Absent keeps both compaction thresholds silent: the trigger never
    /// fires blind, and a deployment that has not said how big its window
    /// is keeps `/compact` and the forced-turn-end door and gets no
    /// automatic one.
    #[serde(default)]
    pub context_window: Option<NonZeroU32>,
    /// The endpoint overrides; omitted entries keep the real hosts.
    #[serde(default)]
    pub endpoints: Endpoints,
    /// The answering budgets; omitted fields keep the stated defaults.
    #[serde(default)]
    pub protection: Protection,
    /// The operators table — who may admit the assistant into a group.
    /// Absent by default, under which every group add fails closed.
    #[serde(default)]
    pub operators: OperatorsTable,
    /// Whether direct chats are served: the closed words `on` and `off`,
    /// any other value refused at the load. Absent means on, so the
    /// repository's generic behavior is unchanged; a deployment spells
    /// `off` until its direct-chat feature set ships.
    #[serde(default)]
    pub direct_chats: DirectChatsKey,
    /// The reasoning-effort level every conversation is created under: the
    /// framework's closed key set — `off`, `auto`, `minimal`, `low`,
    /// `medium`, `high`, `xhigh`, `max` — any other value refused at the
    /// load. Absent means `low`, the deployment's stated default (decided
    /// 2026-08-23): without a set level the model thinks unboundedly, and
    /// a small budget already carries the moderation assessments.
    #[serde(default)]
    pub reasoning: ReasoningKey,
    /// How group messages summon a turn: the closed words `helpful` and
    /// `addressed`, any other value refused at the load. Absent means
    /// helpful — the operator's stated economics — and a deployment that
    /// wants the quiet shape spells `addressed`.
    #[serde(default)]
    pub answering: AnsweringKey,
    /// The assistant's name; absent takes the display name the process
    /// reads from the platform at startup. Resolved through
    /// [`Configuration::resolve_name`], which trims and refuses an empty
    /// value.
    pub name: Option<String>,
    /// The first-interaction disclosure line; absent composes it from the
    /// resolved name. Resolved through
    /// [`Configuration::resolve_disclosure`], which trims and refuses an
    /// empty value — the duty is not optional, so unset means the composed
    /// default, never no text.
    pub disclosure: Option<String>,
    /// The address the privacy command answers with; absent answers the
    /// not-yet-published line. Resolved through
    /// [`Configuration::resolve_privacy_policy`], which refuses an empty
    /// value.
    pub privacy_policy: Option<String>,
    /// The web search's locale; omitted entries keep the stated defaults.
    #[serde(default)]
    pub search: Search,
    /// The webhook intake's wiring. Absent — the section omitted entirely —
    /// the assistant long-polls, which is what a deployment without a public
    /// address does. Present, both of the section's fields are required;
    /// resolved through [`Configuration::resolve_webhook`].
    pub webhook: Option<WebhookSection>,
    /// The moderation bot's handle the report tool files toward; absent
    /// leaves the report tool unregistered, and so does the `addressed`
    /// answering mode even with the handle set — the autonomous assessment
    /// needs both (unit 15, 2026-08-24). Resolved through
    /// [`Configuration::resolve_moderation_handle`], which trims, strips a
    /// leading `@` and refuses an empty value.
    pub moderation_handle: Option<String>,
    /// Where the two secrets are found — never the secrets themselves.
    pub secrets: Secrets,
}

/// The webhook section: the address the platform is told to deliver to, and
/// the loopback port the listener binds behind the deployment's reverse
/// proxy. Both fields are optional in the SHAPE and required in the
/// RESOLUTION, so a half-filled section refuses the start naming the field it
/// is missing instead of decoding into a default nobody chose. Unknown keys
/// are refused like everywhere else in the file, and no secret appears here:
/// the adapter generates its own and keeps it beside its state file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookSection {
    /// The full HTTPS address the platform calls. Its path is what the
    /// listener answers, so the address and the served path are one value.
    pub public_url: Option<String>,
    /// The loopback port the listener binds — a contract with the reverse
    /// proxy in front of it, so zero is refused here even though the adapter
    /// itself would take it.
    pub listen_port: Option<u16>,
}

/// The operators table: adapter name to the operator's adapter-scoped
/// external id on that adapter. Its own validated type: the table's rules —
/// a non-empty id, a key naming a known adapter — live here beside the
/// shape, not hoisted into aggregate validation.
#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct OperatorsTable(BTreeMap<String, String>);

impl OperatorsTable {
    /// The core's operator wiring these entries name, validated against
    /// the adapters the binary actually assembles.
    ///
    /// # Errors
    ///
    /// [`StartError::OperatorUnknownAdapter`] when a key names no known
    /// adapter — a typoed key would silently refuse every group add on the
    /// intended one; [`StartError::OperatorEmpty`] when an entry carries an
    /// empty id — an empty value would match no adder and silently refuse
    /// every add. Both refuse the start instead. A surviving id is stored
    /// trimmed: a padded value would match no adder either, and the pad is
    /// file formatting, never identity.
    pub fn resolve(&self, known_adapters: &[&str]) -> Result<OperatorConfig, StartError> {
        let mut by_adapter = std::collections::HashMap::new();
        for (adapter, external_id) in &self.0 {
            if !known_adapters.contains(&adapter.as_str()) {
                return Err(StartError::OperatorUnknownAdapter {
                    adapter: adapter.clone(),
                });
            }
            let external_id = external_id.trim();
            if external_id.is_empty() {
                return Err(StartError::OperatorEmpty {
                    adapter: adapter.clone(),
                });
            }
            by_adapter.insert(adapter.clone(), external_id.to_owned());
        }
        Ok(OperatorConfig { by_adapter })
    }
}

/// The direct-chat key's closed word list: decoding is the validation, so
/// a misspelled value refuses the load with the failing place named —
/// never a silently-on deployment that meant to be off.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DirectChatsKey {
    /// Direct chats are served — the absent key's meaning, so the generic
    /// behavior stands unless a deployment says otherwise.
    #[default]
    On,
    /// Direct chats are refused fail-closed before any write.
    Off,
}

impl DirectChatsKey {
    /// The core's direct-chat switch this key names.
    #[must_use]
    pub fn resolve(self) -> DirectChats {
        match self {
            Self::On => DirectChats::On,
            Self::Off => DirectChats::Off,
        }
    }
}

/// The reasoning key's closed word list, one variant per key the framework's
/// level parser accepts, spelled identically — the mirror of
/// [`DirectChatsKey`]: decoding is the validation, so a misspelled value
/// refuses the load with the failing place named, and a value this file
/// accepts can never be one the stored key's reader drops as unknown. The
/// tests hold the two vocabularies equal, so a level added to the framework
/// grows a variant here instead of silently staying unspellable.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningKey {
    /// No reasoning.
    Off,
    /// The model decides.
    Auto,
    /// The smallest budget the provider offers.
    Minimal,
    /// A small budget — the absent key's meaning: the deployment default
    /// (decided 2026-08-23), because moderation assessments ride on some
    /// thinking while an unset level lets the model think unboundedly.
    #[default]
    Low,
    /// A middling budget.
    Medium,
    /// A large budget.
    High,
    /// Larger than high, where the provider offers it.
    XHigh,
    /// The largest budget the provider offers.
    Max,
}

impl ReasoningKey {
    /// The framework's reasoning level this key names.
    #[must_use]
    pub fn resolve(self) -> ReasoningLevel {
        match self {
            Self::Off => ReasoningLevel::Off,
            Self::Auto => ReasoningLevel::Auto,
            Self::Minimal => ReasoningLevel::Minimal,
            Self::Low => ReasoningLevel::Low,
            Self::Medium => ReasoningLevel::Medium,
            Self::High => ReasoningLevel::High,
            Self::XHigh => ReasoningLevel::XHigh,
            Self::Max => ReasoningLevel::Max,
        }
    }
}

/// The answering key's closed word list: decoding is the validation, so a
/// misspelled value refuses the load with the failing place named — never
/// a deployment that meant to be quiet answering every message.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnsweringKey {
    /// Every group message summons a turn and the model decides whether to
    /// speak — the absent key's meaning, per the operator's stated
    /// economics.
    #[default]
    Helpful,
    /// A group message summons a turn only when it addresses the
    /// assistant.
    Addressed,
}

impl AnsweringKey {
    /// The core's answering mode this key names.
    #[must_use]
    pub fn resolve(self) -> AnsweringMode {
        match self {
            Self::Helpful => AnsweringMode::Helpful,
            Self::Addressed => AnsweringMode::Addressed,
        }
    }
}

/// Where log lines go, decoded into its own arms so no caller compares
/// strings: a later destination is a new arm here. The console arm is tried
/// first, so the bare word decodes as the console and a file literally
/// named after it takes the table spelling.
/// The console word is matched exactly and lowercase: any other bare string,
/// including an uppercased spelling of it, names a file of that name.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LogDestination {
    /// A console stream, named by its bare word.
    Console(ConsoleStream),
    /// A file, appended to.
    File(FileDestination),
}

/// The console streams the bare-word spelling can name — a closed word
/// list, so the console arm matches exactly its words and nothing else.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleStream {
    /// The standard error stream.
    Stderr,
}

/// A log file's two spellings: any other bare string, or the
/// `{ file = "…" }` table — the table is what makes a file literally named
/// `stderr` expressible, since that bare word decodes as the console.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FileDestination {
    /// The bare path string.
    Bare(PathBuf),
    /// The table spelling.
    Table(FileTable),
}

/// The table spelling's one field, its own struct so unknown keys inside the
/// log table are refused like everywhere else in the file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileTable {
    /// The file the lines are appended to.
    pub file: PathBuf,
}

impl FileDestination {
    /// The path either spelling names.
    pub fn path(&self) -> &Path {
        match self {
            Self::Bare(path) => path,
            Self::Table(table) => &table.file,
        }
    }
}

/// The endpoint overrides. All default to the real hosts; tests point them
/// at loopback servers.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoints {
    /// The Telegram Bot API root.
    pub telegram: Option<String>,
    /// The chat-completions endpoint's base URL — any OpenAI-compatible
    /// host. Named for the interface the process speaks, not for a vendor.
    pub chat_completions: Option<String>,
    /// The canonical forge's base URL — the commit lookup's host.
    pub forge: Option<String>,
    /// The mirror API's base URL — the release lookup's host.
    pub mirror: Option<String>,
    /// The wiki's raw base address — the wiki lookup's page-content host.
    /// Resolved through [`Configuration::resolve_wiki_endpoint`], which
    /// trims and refuses an empty value.
    pub wiki: Option<String>,
    /// The forge base address the wiki lookup's page enumeration reads the
    /// rendered wiki index from. Resolved through
    /// [`Configuration::resolve_wiki_index_endpoint`], which trims and
    /// refuses an empty value.
    pub wiki_index: Option<String>,
    /// The web search vendor's base address — the search tool's host.
    /// Resolved through [`Configuration::resolve_web_search`], which trims
    /// and refuses an empty value; omitted keeps the real vendor.
    pub search: Option<String>,
}

/// The web search's locale table: which country and language the vendor is
/// asked to answer for. Both are omitted by default, and the defaults are
/// stated here because they are what an unconfigured deployment gets: the
/// LANGUAGE defaults to English, and the COUNTRY is sent only when it is
/// configured — an international community's results are a deployment
/// choice, never a vendor default nobody chose. Unknown keys are refused
/// like everywhere else in the file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    /// The country code the vendor answers for. Absent sends none.
    pub country: Option<String>,
    /// The language code the vendor answers in. Absent sends
    /// [`DEFAULT_SEARCH_LANGUAGE`].
    pub language: Option<String>,
}

/// The language the search asks for when the configuration names none.
pub const DEFAULT_SEARCH_LANGUAGE: &str = "en";

/// The protection table: the answering budgets, four fields with per-field
/// defaults, so a partial table overrides only what it names. A window of
/// zero disables that budget explicitly; an answer count of zero is refused
/// by [`Protection::resolve`] — an assistant configured to answer no one is
/// a misconfiguration, not a policy. Unknown keys are refused like
/// everywhere else in the file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Protection {
    /// How many of one sender's messages are answered per window, counted
    /// across every chat.
    pub principal_answers: u32,
    /// The sender window, in seconds. Zero disables the sender budget.
    pub principal_window_seconds: u64,
    /// How many messages are answered per window in one chat.
    pub channel_answers: u32,
    /// The chat window, in seconds. Zero disables the chat budget.
    pub channel_window_seconds: u64,
}

impl Default for Protection {
    fn default() -> Self {
        Self {
            principal_answers: ProtectionConfig::DEFAULT_PRINCIPAL_ANSWERS,
            principal_window_seconds: ProtectionConfig::DEFAULT_PRINCIPAL_WINDOW_SECONDS,
            channel_answers: ProtectionConfig::DEFAULT_CHANNEL_ANSWERS,
            channel_window_seconds: ProtectionConfig::DEFAULT_CHANNEL_WINDOW_SECONDS,
        }
    }
}

impl Protection {
    /// The core's budget configuration these fields name.
    ///
    /// # Errors
    ///
    /// [`StartError::ProtectionAnswersZero`] naming the field when an
    /// answer count is zero — refused even beside a disabling window, so a
    /// nonsense line in the file never passes silently.
    pub fn resolve(&self) -> Result<ProtectionConfig, StartError> {
        Ok(ProtectionConfig {
            principal: budget(
                self.principal_answers,
                self.principal_window_seconds,
                "principal_answers",
                "principal_window_seconds",
            )?,
            channel: budget(
                self.channel_answers,
                self.channel_window_seconds,
                "channel_answers",
                "channel_window_seconds",
            )?,
        })
    }
}

impl Configuration {
    /// The privacy address the command answers with, `None` when the key
    /// is absent.
    ///
    /// # Errors
    ///
    /// [`StartError::PrivacyPolicyEmpty`] when the key is present but
    /// empty or whitespace — a blank legal pointer is a misconfiguration,
    /// refused at load like an empty operator id; omitting the key is how
    /// the not-yet-published line is chosen. A surviving address resolves
    /// trimmed: the pad is file formatting, and it must not flow into the
    /// fixed answer line.
    pub fn resolve_privacy_policy(&self) -> Result<Option<String>, StartError> {
        match &self.privacy_policy {
            Some(address) => match address.trim() {
                "" => Err(StartError::PrivacyPolicyEmpty),
                trimmed => Ok(Some(trimmed.to_owned())),
            },
            None => Ok(None),
        }
    }

    /// The moderation handle the report tool files toward, `None` when the
    /// key is absent — under which the report tool does not register.
    ///
    /// # Errors
    ///
    /// [`StartError::ModerationHandleEmpty`] when the key is present but
    /// empty after trimming and stripping one leading `@` — a blank handle
    /// would file `/report@` at nobody, silently; omitting the key is how
    /// no-report deployments are chosen. A surviving handle resolves
    /// trimmed with the `@` stripped: the pad is file formatting, and the
    /// `@` is the report line's own to add.
    pub fn resolve_moderation_handle(&self) -> Result<Option<String>, StartError> {
        match &self.moderation_handle {
            Some(handle) => {
                let handle = handle.trim();
                let handle = handle.strip_prefix('@').unwrap_or(handle).trim();
                if handle.is_empty() {
                    return Err(StartError::ModerationHandleEmpty);
                }
                Ok(Some(handle.to_owned()))
            }
            None => Ok(None),
        }
    }

    /// The assistant's configured name, `None` when the key is absent —
    /// under which the display name read from the platform at startup
    /// stands in.
    ///
    /// # Errors
    ///
    /// [`StartError::NameEmpty`] when the key is present but empty or
    /// whitespace — a blank name would compose a nameless identity and a
    /// broken disclosure line silently; omitting the key is how the
    /// platform default is chosen. A surviving name resolves trimmed.
    pub fn resolve_name(&self) -> Result<Option<String>, StartError> {
        match &self.name {
            Some(name) => match name.trim() {
                "" => Err(StartError::NameEmpty),
                trimmed => Ok(Some(trimmed.to_owned())),
            },
            None => Ok(None),
        }
    }

    /// The configured disclosure line, `None` when the key is absent —
    /// under which the line composes from the resolved name. Unset never
    /// means no line: the transparency duty is not optional.
    ///
    /// # Errors
    ///
    /// [`StartError::DisclosureEmpty`] when the key is present but empty
    /// or whitespace — a blank line would discharge nothing silently;
    /// omitting the key is how the composed default is chosen. A surviving
    /// line resolves trimmed.
    pub fn resolve_disclosure(&self) -> Result<Option<String>, StartError> {
        match &self.disclosure {
            Some(line) => match line.trim() {
                "" => Err(StartError::DisclosureEmpty),
                trimmed => Ok(Some(trimmed.to_owned())),
            },
            None => Ok(None),
        }
    }

    /// The wiki lookup's raw base address, `None` when the key is absent —
    /// under which the real host stands.
    ///
    /// # Errors
    ///
    /// [`StartError::WikiEndpointEmpty`] when the key is present but empty
    /// or whitespace — an empty base would build unroutable addresses
    /// silently; omitting the key is how the real host is chosen. A
    /// surviving address resolves trimmed.
    pub fn resolve_wiki_endpoint(&self) -> Result<Option<String>, StartError> {
        match &self.endpoints.wiki {
            Some(address) => match address.trim() {
                "" => Err(StartError::WikiEndpointEmpty),
                trimmed => Ok(Some(trimmed.to_owned())),
            },
            None => Ok(None),
        }
    }

    /// The wiki index's base address — the page enumeration's host —
    /// `None` when the key is absent, under which the real forge host
    /// stands.
    ///
    /// # Errors
    ///
    /// [`StartError::WikiIndexEndpointEmpty`] when the key is present but
    /// empty or whitespace — an empty base would build unroutable
    /// addresses silently; omitting the key is how the real host is
    /// chosen. A surviving address resolves trimmed.
    pub fn resolve_wiki_index_endpoint(&self) -> Result<Option<String>, StartError> {
        match &self.endpoints.wiki_index {
            Some(address) => match address.trim() {
                "" => Err(StartError::WikiIndexEndpointEmpty),
                trimmed => Ok(Some(trimmed.to_owned())),
            },
            None => Ok(None),
        }
    }

    /// The webhook wiring the adapter takes, or `None` when the section is
    /// absent — under which updates are long-polled, exactly as a deployment
    /// without a public address needs.
    ///
    /// The section is the one predicate: present, both of its fields are
    /// required, and every value is validated here, at the load, instead of
    /// at the first delivery.
    ///
    /// # Errors
    ///
    /// [`StartError::WebhookFieldMissing`] naming the field when the section
    /// carries only one of the two, or carries a blank address — half a
    /// webhook is not a mode; [`StartError::WebhookAddressInvalid`] when the
    /// address is not one the platform can call, carrying the reason and
    /// never inventing one; [`StartError::WebhookPortZero`] when the port is
    /// zero, because a deployment's port is a contract with its reverse
    /// proxy and an ephemeral one would break it silently.
    pub fn resolve_webhook(
        &self,
    ) -> Result<Option<assistant_adapter_telegram::WebhookConfig>, StartError> {
        let Some(section) = &self.webhook else {
            return Ok(None);
        };
        let public_url = section
            .public_url
            .as_deref()
            .map(str::trim)
            .filter(|address| !address.is_empty())
            .ok_or(StartError::WebhookFieldMissing {
                field: "public_url",
            })?;
        let listen_port = section.listen_port.ok_or(StartError::WebhookFieldMissing {
            field: "listen_port",
        })?;
        if listen_port == 0 {
            return Err(StartError::WebhookPortZero);
        }
        let address =
            assistant_adapter_telegram::WebhookAddress::parse(public_url).map_err(|error| {
                StartError::WebhookAddressInvalid {
                    reason: error.to_string(),
                }
            })?;
        Ok(Some(assistant_adapter_telegram::WebhookConfig {
            address,
            listen_port,
        }))
    }

    /// The web search's whole wiring, or `None` when no key is configured —
    /// under which the search tool is not admitted and its teaching is not
    /// composed. The key is the one predicate, so this function is the one
    /// place that answers "is the search configured": the address and the
    /// locale are read only when a key exists, and an address or a locale
    /// entry set without a key is inert configuration the load refuses to
    /// pretend is a search.
    ///
    /// # Errors
    ///
    /// [`StartError::SecretUnread`] or [`StartError::SecretRef`] when a
    /// configured key cannot be read — optional by policy, not by leniency,
    /// exactly like the mirror token. [`StartError::SearchEndpointEmpty`]
    /// when the address override is present but blank, and
    /// [`StartError::SearchLocaleEmpty`] naming the field when a locale
    /// entry is present but blank: a blank locale would be sent to the
    /// vendor as an empty code instead of the default nobody chose.
    pub fn resolve_web_search(&self) -> Result<Option<SearchConfig>, StartError> {
        let Some(reference) = &self.secrets.search_api_key else {
            return Ok(None);
        };
        let api_key = reference.resolve("search_api_key")?;
        let base_url = match &self.endpoints.search {
            Some(address) => match address.trim() {
                "" => return Err(StartError::SearchEndpointEmpty),
                trimmed => trimmed.to_owned(),
            },
            None => search::DEFAULT_BASE_URL.to_owned(),
        };
        Ok(Some(SearchConfig {
            base_url,
            api_key,
            country: locale_code(self.search.country.as_deref(), "country")?,
            language: locale_code(self.search.language.as_deref(), "language")?
                .unwrap_or_else(|| DEFAULT_SEARCH_LANGUAGE.to_owned()),
        }))
    }
}

/// One configured locale code, trimmed; `None` for an absent key and a
/// refusal for a blank one.
fn locale_code(
    configured: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, StartError> {
    match configured {
        Some(code) => match code.trim() {
            "" => Err(StartError::SearchLocaleEmpty { field }),
            trimmed => Ok(Some(trimmed.to_owned())),
        },
        None => Ok(None),
    }
}

/// The longest accepted window: one year. Far past this, the database's
/// date arithmetic returns null and the count would silently admit
/// everything; the bound turns that cliff into a parse refusal.
const MAX_WINDOW_SECONDS: u64 = 31_536_000;

/// One budget from its two fields: `None` for a zero window, refusal for a
/// zero count or an over-year window. The file's seconds carry into the
/// core's budget as seconds — the window's one unit end to end, whole by
/// both types.
fn budget(
    answers: u32,
    window_seconds: u64,
    answers_field: &'static str,
    window_field: &'static str,
) -> Result<Option<Budget>, StartError> {
    let answers = NonZeroU32::new(answers).ok_or(StartError::ProtectionAnswersZero {
        field: answers_field,
    })?;
    if window_seconds > MAX_WINDOW_SECONDS {
        return Err(StartError::ProtectionWindowOverBound {
            field: window_field,
        });
    }
    Ok(
        NonZeroU64::new(window_seconds).map(|window_seconds| Budget {
            answers,
            window_seconds,
        }),
    )
}

/// Where each secret is found.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Secrets {
    /// The Telegram bot token.
    pub bot_token: SecretRef,
    /// The chat-completions endpoint's API key.
    pub chat_completions_api_key: SecretRef,
    /// The mirror API token the release lookup sends as its authorization
    /// header. Optional: absent, the lookup runs unauthenticated at the
    /// mirror's lower rate limit and sends no header.
    pub mirror_token: Option<SecretRef>,
    /// The web search vendor's API key. Optional, and it is the search
    /// tool's whole predicate (unit 27): absent, the tool is not admitted
    /// and the composed prompt teaches no search — there is no call path on
    /// which an unconfigured search can answer.
    pub search_api_key: Option<SecretRef>,
}

/// One secret's indirection: an environment variable name or a file path,
/// exactly one of the two. Surrounding whitespace is trimmed off the value
/// whichever source carried it — a secrets file ends in a newline and a
/// shell export picks one up just as easily, and either would otherwise
/// travel verbatim into an authorization header and come back as a refusal
/// that reads like a broken key.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretRef {
    /// The environment variable holding the value.
    pub env: Option<String>,
    /// The file holding the value.
    pub file: Option<PathBuf>,
}

impl SecretRef {
    /// The secret's value, read from wherever the reference points.
    ///
    /// # Errors
    ///
    /// [`StartError::SecretRef`] when the reference names both sources or
    /// neither; [`StartError::SecretUnread`] when the named source cannot be
    /// read. The errors carry the secret's configuration key and the named
    /// source, never a value.
    pub fn resolve(&self, key: &'static str) -> Result<String, StartError> {
        self.resolve_from(key, |name| std::env::var(name).ok())
    }

    /// The same resolution over a given reader of process variables. The
    /// seam exists because this workspace forbids `unsafe` and setting a
    /// process variable is unsafe in this edition, so the environment
    /// branch would otherwise be the one branch no test can reach.
    fn resolve_from(
        &self,
        key: &'static str,
        read_env: impl Fn(&str) -> Option<String>,
    ) -> Result<String, StartError> {
        let value = match (&self.env, &self.file) {
            (Some(name), None) => read_env(name).ok_or_else(|| StartError::SecretUnread {
                key,
                source_name: format!("environment variable {name}"),
            }),
            (None, Some(path)) => {
                std::fs::read_to_string(path).map_err(|_| StartError::SecretUnread {
                    key,
                    source_name: format!("file {}", path.display()),
                })
            }
            _ => Err(StartError::SecretRef { key }),
        }?;
        // One trim, after every source has answered: the promise belongs to
        // the reference, not to whichever branch read it.
        Ok(value.trim().to_owned())
    }
}

impl Configuration {
    /// Read and decode the configuration file.
    ///
    /// # Errors
    ///
    /// [`StartError::ConfigurationUnread`] when the file cannot be read;
    /// [`StartError::ConfigurationInvalid`] when it does not decode.
    pub fn load(path: &Path) -> Result<Self, StartError> {
        let text =
            std::fs::read_to_string(path).map_err(|error| StartError::ConfigurationUnread {
                path: path.to_path_buf(),
                error,
            })?;
        toml::from_str(&text).map_err(|error| StartError::ConfigurationInvalid {
            path: path.to_path_buf(),
            location: locate(&text, error.span()),
        })
    }
}

/// Where in the file a decode failure sits, named without repeating any of
/// the file's own text. Neither the decoder's rendering nor serde's own
/// prose is safe to echo — both can quote the offending value — and a
/// secret pasted inline where its indirection belongs must not reach
/// stderr or a log through the refusal.
fn locate(text: &str, span: Option<std::ops::Range<usize>>) -> String {
    let Some(span) = span else {
        return "at a place the decoder does not name".into();
    };
    let start = span.start.min(text.len());
    let line_start = text[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let line = text[..start].matches('\n').count() + 1;
    let column = text[line_start..start].chars().count() + 1;
    format!("at line {line}, column {column}")
}

// Every test below binds to the real load path: a whole file on disk, read
// and decoded through [`Configuration::load`] exactly as the binary does —
// never a private probe copy of the shape, which could drift from the real
// one without a test noticing.
#[cfg(test)]
mod tests {
    use super::*;

    /// The adapters the binary assembles, as the resolution tests validate
    /// against.
    const KNOWN_ADAPTERS: [&str; 1] = ["telegram"];

    /// A configuration file on disk, removed when dropped.
    struct TempConfigFile(PathBuf);

    impl TempConfigFile {
        fn new(content: &str) -> Self {
            // The name is collision-proof by construction: the pid separates
            // processes and the process-wide counter separates threads — a
            // clock-based name collides under the parallel runner, which
            // starts many fixtures inside one tick.
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = format!(
                "assistant-config-{}-{}.toml",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::write(&path, content).expect("the configuration file writes");
            Self(path)
        }
    }

    impl Drop for TempConfigFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn load(content: &str) -> Result<Configuration, StartError> {
        let file = TempConfigFile::new(content);
        Configuration::load(&file.0)
    }

    /// A whole valid file around the given log line and extra content —
    /// bare keys in `extra` land ahead of the secrets tables, so both
    /// spellings compose.
    fn full(log_line: &str, extra: &str) -> String {
        format!(
            "store_path = \"assistant.db\"\n\
             telegram_state_path = \"telegram.offset\"\n\
             prompt_dir = \"prompts\"\n\
             {log_line}\n\
             model = \"test-model\"\n\
             {extra}\n\
             [secrets.bot_token]\n\
             env = \"UNUSED\"\n\
             \n\
             [secrets.chat_completions_api_key]\n\
             env = \"UNUSED\"\n"
        )
    }

    fn loaded(log_line: &str, extra: &str) -> Configuration {
        load(&full(log_line, extra)).expect("the configuration loads")
    }

    /// The README's own example, extracted verbatim and decoded through the
    /// real load path, with every resolution the binary performs behind the
    /// decode — so the shown example cannot rot away from the real shape.
    #[test]
    fn the_readme_example_loads_and_resolves() {
        let readme = include_str!("../../../README.md");
        let lines: Vec<&str> = readme.lines().collect();
        let start = lines
            .iter()
            .position(|line| line.trim_start().starts_with("store_path"))
            .expect("the README shows the example configuration");
        let end = lines
            .iter()
            .rposition(|line| line.contains("telegram = \"<"))
            .expect("the example ends with the operators entry");
        let example: String = lines[start..=end]
            .iter()
            .map(|line| line.strip_prefix("    ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        let configuration = load(&example).expect("the README example decodes");
        configuration
            .operators
            .resolve(&KNOWN_ADAPTERS)
            .expect("the example's operators table resolves");
        configuration
            .protection
            .resolve()
            .expect("the example's protection table resolves");
        configuration
            .resolve_privacy_policy()
            .expect("the example's privacy key resolves");
        assert!(
            configuration
                .resolve_web_search()
                .expect("the example's search wiring resolves")
                .is_none(),
            "the example leaves the search key commented out, so it configures no search"
        );
    }

    // ─── The log destination ─────────────────────────────────────────────

    #[test]
    fn the_bare_word_names_the_console() {
        assert!(matches!(
            loaded("log = \"stderr\"", "").log,
            LogDestination::Console(ConsoleStream::Stderr)
        ));
    }

    #[test]
    fn any_other_bare_string_names_a_file() {
        let LogDestination::File(file) = loaded("log = \"assistant.log\"", "").log else {
            panic!("a bare path string decodes as a file destination");
        };
        assert_eq!(file.path(), Path::new("assistant.log"));
    }

    #[test]
    fn the_table_spelling_names_a_file_even_one_named_after_the_console() {
        let LogDestination::File(file) = loaded("log = { file = \"stderr\" }", "").log else {
            panic!("the table spelling decodes as a file destination");
        };
        assert_eq!(file.path(), Path::new("stderr"));
    }

    // ─── The protection table ────────────────────────────────────────────

    fn resolved_protection(extra: &str) -> Result<ProtectionConfig, StartError> {
        loaded("log = \"stderr\"", extra).protection.resolve()
    }

    #[test]
    fn an_absent_protection_table_takes_the_stated_defaults() {
        let resolved = resolved_protection("").expect("the defaults resolve");
        let principal = resolved.principal.expect("the principal budget is on");
        let channel = resolved.channel.expect("the channel budget is on");
        assert_eq!(
            principal.answers.get(),
            ProtectionConfig::DEFAULT_PRINCIPAL_ANSWERS
        );
        assert_eq!(
            principal.window_seconds.get(),
            ProtectionConfig::DEFAULT_PRINCIPAL_WINDOW_SECONDS
        );
        assert_eq!(
            channel.answers.get(),
            ProtectionConfig::DEFAULT_CHANNEL_ANSWERS
        );
        assert_eq!(
            channel.window_seconds.get(),
            ProtectionConfig::DEFAULT_CHANNEL_WINDOW_SECONDS
        );
    }

    #[test]
    fn a_partial_protection_table_takes_per_field_defaults() {
        let resolved = resolved_protection("[protection]\nprincipal_answers = 2\n")
            .expect("the partial table resolves");
        let principal = resolved.principal.expect("the principal budget is on");
        assert_eq!(principal.answers.get(), 2, "the named field overrides");
        assert_eq!(
            principal.window_seconds.get(),
            ProtectionConfig::DEFAULT_PRINCIPAL_WINDOW_SECONDS,
            "the unnamed window keeps its default"
        );
        let channel = resolved.channel.expect("the channel budget is on");
        assert_eq!(
            channel.answers.get(),
            ProtectionConfig::DEFAULT_CHANNEL_ANSWERS,
            "the untouched budget keeps its defaults"
        );
    }

    #[test]
    fn a_zero_window_disables_that_budget_and_only_that_budget() {
        let resolved = resolved_protection("[protection]\nchannel_window_seconds = 0\n")
            .expect("the disabling window resolves");
        assert!(resolved.channel.is_none(), "a zero window disables");
        assert!(
            resolved.principal.is_some(),
            "the other budget stays enabled"
        );
    }

    #[test]
    fn an_over_year_window_is_refused_naming_the_field() {
        let refused =
            resolved_protection("[protection]\nprincipal_window_seconds = 1000000000000\n")
                .expect_err("a window past the year bound must be refused");
        match refused {
            StartError::ProtectionWindowOverBound { field } => {
                assert_eq!(field, "principal_window_seconds");
            }
            other => panic!("the refusal names the window field; got {other}"),
        }
        // The bound itself still resolves.
        assert!(
            resolved_protection("[protection]\nchannel_window_seconds = 31536000\n")
                .expect("the one-year window resolves")
                .channel
                .is_some()
        );
    }

    #[test]
    fn a_zero_answer_count_is_refused_naming_the_field() {
        let refused = resolved_protection("[protection]\nchannel_answers = 0\n")
            .expect_err("a zero count must be refused");
        match refused {
            StartError::ProtectionAnswersZero { field } => {
                assert_eq!(field, "channel_answers");
            }
            other => panic!("the refusal names the zero field; got {other}"),
        }
        // Refused even beside the window that would disable the budget.
        assert!(
            resolved_protection(
                "[protection]\nprincipal_answers = 0\nprincipal_window_seconds = 0\n"
            )
            .is_err(),
            "a zero count beside a disabling window is still a refusal"
        );
    }

    #[test]
    fn an_unknown_key_is_refused_at_the_load() {
        assert!(
            load(&full(
                "log = \"stderr\"",
                "[protection]\nprincipal_burst = 3\n"
            ))
            .is_err(),
            "unknown keys in the protection table are refused"
        );
        assert!(
            load(&full("log = \"stderr\"", "surprise = true\n")).is_err(),
            "unknown top-level keys are refused"
        );
    }

    // ─── The operators table and the privacy address ─────────────────────

    #[test]
    fn the_operators_table_and_the_privacy_address_resolve() {
        let configuration = loaded(
            "log = \"stderr\"",
            "privacy_policy = \"https://example.org/privacy\"\n\
             [operators]\n\
             telegram = \"12345\"\n",
        );
        let resolved = configuration
            .operators
            .resolve(&KNOWN_ADAPTERS)
            .expect("the wiring resolves");
        assert_eq!(
            resolved.by_adapter.get("telegram").map(String::as_str),
            Some("12345")
        );
        assert_eq!(
            configuration
                .resolve_privacy_policy()
                .expect("the address resolves")
                .as_deref(),
            Some("https://example.org/privacy")
        );
    }

    #[test]
    fn both_keys_are_absent_by_default() {
        let configuration = loaded("log = \"stderr\"", "");
        let resolved = configuration
            .operators
            .resolve(&KNOWN_ADAPTERS)
            .expect("the absent wiring resolves");
        assert!(resolved.by_adapter.is_empty(), "no operator by default");
        assert_eq!(
            configuration
                .resolve_privacy_policy()
                .expect("the absent key resolves"),
            None
        );
    }

    #[test]
    fn a_padded_operator_id_and_privacy_address_resolve_trimmed() {
        let configuration = loaded(
            "log = \"stderr\"",
            "privacy_policy = \" https://example.org/privacy \"\n\
             [operators]\n\
             telegram = \" 12345 \"\n",
        );
        let resolved = configuration
            .operators
            .resolve(&KNOWN_ADAPTERS)
            .expect("the padded id resolves");
        assert_eq!(
            resolved.by_adapter.get("telegram").map(String::as_str),
            Some("12345"),
            "the pad is file formatting, never identity — untrimmed it would \
             match no adder and silently refuse every add"
        );
        assert_eq!(
            configuration
                .resolve_privacy_policy()
                .expect("the padded address resolves")
                .as_deref(),
            Some("https://example.org/privacy"),
            "no whitespace flows into the fixed answer line"
        );
    }

    #[test]
    fn an_empty_operator_id_is_refused_naming_the_adapter() {
        let refused = loaded("log = \"stderr\"", "[operators]\ntelegram = \"  \"\n")
            .operators
            .resolve(&KNOWN_ADAPTERS)
            .expect_err("an empty operator id must be refused");
        match refused {
            StartError::OperatorEmpty { adapter } => assert_eq!(adapter, "telegram"),
            other => panic!("the refusal names the adapter; got {other}"),
        }
    }

    #[test]
    fn an_operator_key_naming_no_known_adapter_is_refused() {
        let refused = loaded("log = \"stderr\"", "[operators]\ntelegrm = \"12345\"\n")
            .operators
            .resolve(&KNOWN_ADAPTERS)
            .expect_err("a typoed adapter key must be refused");
        match refused {
            StartError::OperatorUnknownAdapter { adapter } => assert_eq!(adapter, "telegrm"),
            other => panic!("the refusal names the unknown key; got {other}"),
        }
    }

    // ─── The direct-chat switch ──────────────────────────────────────────

    #[test]
    fn the_direct_chat_key_decodes_its_two_words_and_defaults_on() {
        assert!(
            matches!(
                loaded("log = \"stderr\"", "").direct_chats.resolve(),
                DirectChats::On
            ),
            "the absent key means on — the generic behavior is unchanged"
        );
        assert!(matches!(
            loaded("log = \"stderr\"", "direct_chats = \"on\"\n")
                .direct_chats
                .resolve(),
            DirectChats::On
        ));
        assert!(matches!(
            loaded("log = \"stderr\"", "direct_chats = \"off\"\n")
                .direct_chats
                .resolve(),
            DirectChats::Off
        ));
    }

    #[test]
    fn a_direct_chat_value_outside_the_two_words_is_refused_at_the_load() {
        for spelling in [
            "direct_chats = \"maybe\"\n",
            "direct_chats = \"OFF\"\n",
            "direct_chats = \"\"\n",
            "direct_chats = true\n",
        ] {
            assert!(
                load(&full("log = \"stderr\"", spelling)).is_err(),
                "the value must be exactly `on` or `off`; {spelling:?} decoded"
            );
        }
    }

    // ─── The reasoning level ─────────────────────────────────────────────

    /// Every accepted spelling beside the level it names — the whole closed
    /// set, so a key the file accepts that the framework's parser would drop
    /// as unknown, or a framework key this file cannot spell, fails a test
    /// here instead of surfacing as a silently deferring conversation.
    const REASONING_KEYS: [(&str, ReasoningLevel); 8] = [
        ("off", ReasoningLevel::Off),
        ("auto", ReasoningLevel::Auto),
        ("minimal", ReasoningLevel::Minimal),
        ("low", ReasoningLevel::Low),
        ("medium", ReasoningLevel::Medium),
        ("high", ReasoningLevel::High),
        ("xhigh", ReasoningLevel::XHigh),
        ("max", ReasoningLevel::Max),
    ];

    #[test]
    fn the_reasoning_key_decodes_every_framework_key_and_defaults_low() {
        assert_eq!(
            loaded("log = \"stderr\"", "").reasoning.resolve(),
            ReasoningLevel::Low,
            "the absent key means low — the deployment's stated default"
        );
        for (spelling, level) in REASONING_KEYS {
            let resolved = loaded("log = \"stderr\"", &format!("reasoning = \"{spelling}\"\n"))
                .reasoning
                .resolve();
            assert_eq!(resolved, level, "the spelling {spelling:?} names its level");
            assert_eq!(
                ReasoningLevel::from_key(spelling),
                Some(level),
                "the framework's parser accepts the same spelling"
            );
            assert_eq!(
                resolved.as_key(),
                spelling,
                "the resolved level stores back under the file's own spelling"
            );
        }
    }

    #[test]
    fn a_reasoning_value_outside_the_framework_keys_is_refused_at_the_load() {
        for spelling in [
            "reasoning = \"unbounded\"\n",
            "reasoning = \"LOW\"\n",
            "reasoning = \"\"\n",
            "reasoning = 3\n",
        ] {
            assert!(
                load(&full("log = \"stderr\"", spelling)).is_err(),
                "the value must be one of the framework's keys; {spelling:?} decoded"
            );
        }
    }

    // ─── The moderation handle and the wiki endpoint ─────────────────────

    #[test]
    fn the_moderation_handle_resolves_trimmed_with_the_leading_at_stripped() {
        for spelling in [
            "moderation_bot",
            "@moderation_bot",
            "  @moderation_bot  ",
            " moderation_bot ",
        ] {
            let configuration = loaded(
                "log = \"stderr\"",
                &format!("moderation_handle = \"{spelling}\"\n"),
            );
            assert_eq!(
                configuration
                    .resolve_moderation_handle()
                    .expect("the handle resolves")
                    .as_deref(),
                Some("moderation_bot"),
                "the pad is file formatting and the `@` is the report line's \
                 own to add; spelling {spelling:?}"
            );
        }
        assert_eq!(
            loaded("log = \"stderr\"", "")
                .resolve_moderation_handle()
                .expect("the absent key resolves"),
            None,
            "no handle by default — the report tool stays unregistered"
        );
    }

    #[test]
    fn an_empty_moderation_handle_is_refused() {
        for spelling in [
            "moderation_handle = \"\"\n",
            "moderation_handle = \"   \"\n",
            "moderation_handle = \"@\"\n",
            "moderation_handle = \" @ \"\n",
        ] {
            let refused = loaded("log = \"stderr\"", spelling)
                .resolve_moderation_handle()
                .expect_err("a blank handle must be refused");
            assert!(
                matches!(refused, StartError::ModerationHandleEmpty),
                "the refusal names the empty handle; got {refused}"
            );
        }
    }

    #[test]
    fn the_wiki_endpoint_resolves_trimmed_and_refuses_empty() {
        let configuration = loaded(
            "log = \"stderr\"",
            "[endpoints]\nwiki = \" http://127.0.0.1:1 \"\n",
        );
        assert_eq!(
            configuration
                .resolve_wiki_endpoint()
                .expect("the address resolves")
                .as_deref(),
            Some("http://127.0.0.1:1")
        );
        assert_eq!(
            loaded("log = \"stderr\"", "")
                .resolve_wiki_endpoint()
                .expect("the absent key resolves"),
            None,
            "no override by default — the real host stands"
        );
        let refused = loaded("log = \"stderr\"", "[endpoints]\nwiki = \"  \"\n")
            .resolve_wiki_endpoint()
            .expect_err("an empty address must be refused");
        assert!(
            matches!(refused, StartError::WikiEndpointEmpty),
            "the refusal names the empty address; got {refused}"
        );
    }

    #[test]
    fn the_wiki_index_endpoint_resolves_trimmed_and_refuses_empty() {
        let configuration = loaded(
            "log = \"stderr\"",
            "[endpoints]\nwiki_index = \" http://127.0.0.1:1 \"\n",
        );
        assert_eq!(
            configuration
                .resolve_wiki_index_endpoint()
                .expect("the address resolves")
                .as_deref(),
            Some("http://127.0.0.1:1")
        );
        assert_eq!(
            loaded("log = \"stderr\"", "")
                .resolve_wiki_index_endpoint()
                .expect("the absent key resolves"),
            None,
            "no override by default — the real forge host stands"
        );
        let refused = loaded("log = \"stderr\"", "[endpoints]\nwiki_index = \"  \"\n")
            .resolve_wiki_index_endpoint()
            .expect_err("an empty address must be refused");
        assert!(
            matches!(refused, StartError::WikiIndexEndpointEmpty),
            "the refusal names the empty address; got {refused}"
        );
    }

    // ─── The web search's wiring (unit 27) ───────────────────────────────

    /// The key is the whole predicate: with none configured the resolution
    /// answers `None`, whatever else the file names, and the assembly
    /// therefore admits no search tool and teaches no search.
    #[test]
    fn no_search_key_means_no_search_however_the_rest_is_configured() {
        assert!(
            loaded("log = \"stderr\"", "")
                .resolve_web_search()
                .expect("an unconfigured search resolves")
                .is_none(),
            "no key, no search"
        );
        assert!(
            loaded(
                "log = \"stderr\"",
                "[endpoints]\nsearch = \"http://127.0.0.1:1\"\n\n[search]\ncountry = \"de\"\n",
            )
            .resolve_web_search()
            .expect("an unconfigured search resolves")
            .is_none(),
            "an address and a locale without a key are inert, never a search"
        );
    }

    /// The secrets file every search test points its key reference at —
    /// a file, never an environment variable: this workspace forbids
    /// `unsafe`, and setting a process variable is unsafe in this edition.
    fn key_file() -> TempConfigFile {
        TempConfigFile::new("FAKE-SEARCH-KEY\n")
    }

    /// The `[secrets.search_api_key]` table pointing at that file.
    fn key_secret(file: &TempConfigFile) -> String {
        format!(
            "[secrets.search_api_key]\nfile = \"{}\"\n",
            file.0.display()
        )
    }

    /// A configured key resolves the whole wiring: the real vendor by
    /// default, the stated language default, and no country until one is
    /// configured. The key resolves trimmed, so a secrets file's trailing
    /// newline never travels to the vendor.
    #[test]
    fn a_configured_key_resolves_the_defaults_the_documentation_states() {
        let file = key_file();
        let configured = loaded("log = \"stderr\"", &key_secret(&file))
            .resolve_web_search()
            .expect("the configured search resolves")
            .expect("a key configures the search");
        assert_eq!(configured.base_url, search::DEFAULT_BASE_URL);
        assert_eq!(configured.api_key, "FAKE-SEARCH-KEY");
        assert_eq!(
            configured.language, DEFAULT_SEARCH_LANGUAGE,
            "the language default is the one the documentation states"
        );
        assert_eq!(
            configured.country, None,
            "the country is sent only where a deployment chose one"
        );
    }

    /// The trim belongs to the reference and not to one of its sources: a
    /// key read from a process variable loses the trailing newline a shell
    /// export picks up, exactly as a secrets file's key does. An untrimmed
    /// environment value would reach the vendor inside the key header and
    /// come back as a refusal that reads like a wrong key.
    #[test]
    fn a_secret_resolves_trimmed_from_either_source() {
        let file = key_file();
        let from_file = SecretRef {
            env: None,
            file: Some(file.0.clone()),
        };
        assert_eq!(
            from_file
                .resolve_from("search_api_key", |_| None)
                .expect("the file source reads"),
            "FAKE-SEARCH-KEY"
        );
        let from_env = SecretRef {
            env: Some("ASSISTANT_SEARCH_API_KEY".to_owned()),
            file: None,
        };
        assert_eq!(
            from_env
                .resolve_from("search_api_key", |name| {
                    (name == "ASSISTANT_SEARCH_API_KEY").then(|| " FAKE-SEARCH-KEY \n".to_owned())
                })
                .expect("the environment source reads"),
            "FAKE-SEARCH-KEY"
        );
        assert!(
            matches!(
                from_env.resolve_from("search_api_key", |_| None),
                Err(StartError::SecretUnread { key, source_name })
                    if key == "search_api_key"
                        && source_name.contains("ASSISTANT_SEARCH_API_KEY")
            ),
            "an unset variable names itself and never a value"
        );
    }

    /// The address override and the locale entries resolve trimmed, and a
    /// blank one refuses the start instead of reaching the vendor as an
    /// empty code.
    #[test]
    fn the_search_address_and_locale_resolve_trimmed_and_refuse_blanks() {
        let file = key_file();
        let secret = key_secret(&file);
        let configured = loaded(
            "log = \"stderr\"",
            &format!(
                "[endpoints]\nsearch = \" http://127.0.0.1:1 \"\n\n\
                 [search]\ncountry = \" de \"\nlanguage = \" fr \"\n\n{secret}"
            ),
        )
        .resolve_web_search()
        .expect("the configured search resolves")
        .expect("a key configures the search");
        assert_eq!(configured.base_url, "http://127.0.0.1:1");
        assert_eq!(configured.country.as_deref(), Some("de"));
        assert_eq!(configured.language, "fr");

        let refused = loaded(
            "log = \"stderr\"",
            &format!("[endpoints]\nsearch = \"  \"\n\n{secret}"),
        )
        .resolve_web_search()
        .expect_err("an empty address must be refused");
        assert!(
            matches!(refused, StartError::SearchEndpointEmpty),
            "the refusal names the empty address; got {refused}"
        );

        for (field, table) in [
            ("country", "[search]\ncountry = \"  \"\n"),
            ("language", "[search]\nlanguage = \"  \"\n"),
        ] {
            let refused = loaded("log = \"stderr\"", &format!("{table}\n{secret}"))
                .resolve_web_search()
                .expect_err("an empty locale code must be refused");
            assert!(
                matches!(refused, StartError::SearchLocaleEmpty { field: named } if named == field),
                "the refusal names the blank {field}; got {refused}"
            );
        }
    }

    /// A configured key that cannot be read refuses the start — optional by
    /// policy, not by leniency, exactly like the mirror token's — and the
    /// refusal names the source, never a value.
    #[test]
    fn an_unreadable_search_key_refuses_the_start() {
        let refused = loaded(
            "log = \"stderr\"",
            "[secrets.search_api_key]\nfile = \"/nonexistent/search.key\"\n",
        )
        .resolve_web_search()
        .expect_err("an unreadable key must refuse the start");
        assert!(
            matches!(refused, StartError::SecretUnread { key, .. } if key == "search_api_key"),
            "the refusal names the key's configuration entry; got {refused}"
        );
        assert!(
            refused.to_string().contains("/nonexistent/search.key"),
            "the refusal names the source it looked in: {refused}"
        );
    }

    // ─── The answering mode, the name and the disclosure ─────────────────

    #[test]
    fn the_answering_key_decodes_its_two_words_and_defaults_helpful() {
        assert!(
            matches!(
                loaded("log = \"stderr\"", "").answering.resolve(),
                AnsweringMode::Helpful
            ),
            "the absent key means helpful — the operator's stated default"
        );
        assert!(matches!(
            loaded("log = \"stderr\"", "answering = \"helpful\"\n")
                .answering
                .resolve(),
            AnsweringMode::Helpful
        ));
        assert!(matches!(
            loaded("log = \"stderr\"", "answering = \"addressed\"\n")
                .answering
                .resolve(),
            AnsweringMode::Addressed
        ));
    }

    #[test]
    fn an_answering_value_outside_the_two_words_is_refused_at_the_load() {
        for spelling in [
            "answering = \"quiet\"\n",
            "answering = \"HELPFUL\"\n",
            "answering = \"\"\n",
            "answering = true\n",
        ] {
            assert!(
                load(&full("log = \"stderr\"", spelling)).is_err(),
                "the value must be exactly `helpful` or `addressed`; {spelling:?} decoded"
            );
        }
    }

    #[test]
    fn the_name_and_the_disclosure_resolve_trimmed_and_absent_means_default() {
        let configuration = loaded(
            "log = \"stderr\"",
            "name = \" Xenia \"\ndisclosure = \" I am a machine. \"\n",
        );
        assert_eq!(
            configuration
                .resolve_name()
                .expect("the name resolves")
                .as_deref(),
            Some("Xenia"),
            "the pad is file formatting, never identity"
        );
        assert_eq!(
            configuration
                .resolve_disclosure()
                .expect("the disclosure resolves")
                .as_deref(),
            Some("I am a machine."),
            "no whitespace flows into the stored line"
        );

        let absent = loaded("log = \"stderr\"", "");
        assert_eq!(
            absent.resolve_name().expect("the absent key resolves"),
            None,
            "no name key means the platform display name"
        );
        assert_eq!(
            absent
                .resolve_disclosure()
                .expect("the absent key resolves"),
            None,
            "no disclosure key means the line composed from the name"
        );
    }

    #[test]
    fn an_empty_name_or_disclosure_is_refused() {
        for spelling in ["name = \"\"\n", "name = \"   \"\n"] {
            let refused = loaded("log = \"stderr\"", spelling)
                .resolve_name()
                .expect_err("a blank name must be refused");
            assert!(
                matches!(refused, StartError::NameEmpty),
                "the refusal names the empty name; got {refused}"
            );
        }
        for spelling in ["disclosure = \"\"\n", "disclosure = \"   \"\n"] {
            let refused = loaded("log = \"stderr\"", spelling)
                .resolve_disclosure()
                .expect_err("a blank disclosure must be refused");
            assert!(
                matches!(refused, StartError::DisclosureEmpty),
                "the refusal names the empty line; got {refused}"
            );
        }
    }

    // ─── The webhook section (unit 35) ───────────────────────────────────

    /// The section is the one predicate: absent, the deployment polls;
    /// present and whole, it resolves the address the platform is told to
    /// call and the port the listener binds — and the address carries the
    /// path the listener answers, so the two cannot diverge.
    #[test]
    fn the_webhook_section_is_the_whole_predicate_and_resolves_both_fields() {
        assert!(
            loaded("log = \"stderr\"", "")
                .resolve_webhook()
                .expect("an absent section resolves")
                .is_none(),
            "no section, no webhook — the deployment polls"
        );
        let resolved = loaded(
            "log = \"stderr\"",
            "[webhook]\n\
             public_url = \" https://xenia.example.org/telegram/webhook \"\n\
             listen_port = 8085\n",
        )
        .resolve_webhook()
        .expect("the whole section resolves")
        .expect("a present section configures the webhook");
        assert_eq!(
            resolved.address.url(),
            "https://xenia.example.org/telegram/webhook",
            "the pad is file formatting, never the address"
        );
        assert_eq!(
            resolved.address.path(),
            "/telegram/webhook",
            "the path the listener answers comes from the address itself"
        );
        assert_eq!(resolved.listen_port, 8085);
    }

    /// Half a webhook is not an answering mode: the missing field is named,
    /// and a blank address is the same refusal — omitting the section is how
    /// polling is chosen.
    #[test]
    fn a_half_filled_webhook_section_is_refused_naming_the_missing_field() {
        for (missing, section) in [
            (
                "listen_port",
                "[webhook]\npublic_url = \"https://x.example.org/hook\"\n",
            ),
            ("public_url", "[webhook]\nlisten_port = 8085\n"),
            ("public_url", "[webhook]\n"),
            (
                "public_url",
                "[webhook]\npublic_url = \"   \"\nlisten_port = 8085\n",
            ),
        ] {
            let refused = loaded("log = \"stderr\"", section)
                .resolve_webhook()
                .expect_err("a half-filled section must be refused");
            assert!(
                matches!(refused, StartError::WebhookFieldMissing { field } if field == missing),
                "the refusal names {missing}; got {refused}"
            );
        }
    }

    /// An address the platform could not call, or one the listener could not
    /// match a delivery against, refuses the start where it is configured —
    /// with the reason stated and no guess at it.
    #[test]
    fn a_webhook_address_the_platform_cannot_call_is_refused() {
        for address in [
            "http://xenia.example.org/hook",
            "https://xenia.example.org",
            "https://xenia.example.org/hook?token=1",
        ] {
            let refused = loaded(
                "log = \"stderr\"",
                &format!("[webhook]\npublic_url = \"{address}\"\nlisten_port = 8085\n"),
            )
            .resolve_webhook()
            .expect_err("an unusable address must be refused");
            assert!(
                matches!(refused, StartError::WebhookAddressInvalid { .. }),
                "the refusal names the address; got {refused}"
            );
        }
    }

    /// A deployment's port is a contract with its reverse proxy, so zero —
    /// which would bind an ephemeral port nothing forwards to — is refused.
    #[test]
    fn a_zero_webhook_port_is_refused() {
        let refused = loaded(
            "log = \"stderr\"",
            "[webhook]\npublic_url = \"https://x.example.org/hook\"\nlisten_port = 0\n",
        )
        .resolve_webhook()
        .expect_err("a zero port must be refused");
        assert!(
            matches!(refused, StartError::WebhookPortZero),
            "the refusal names the port; got {refused}"
        );
    }

    /// Unknown keys inside the section are refused at the load, like
    /// everywhere else in the file — a misspelled key must not sit inert
    /// beside a half-configured webhook. A secret key above all: the adapter
    /// generates its own, and a file naming one would be a credential nobody
    /// should be handling.
    #[test]
    fn an_unknown_webhook_key_is_refused_at_the_load() {
        assert!(
            load(&full(
                "log = \"stderr\"",
                "[webhook]\n\
                 public_url = \"https://x.example.org/hook\"\n\
                 listen_port = 8085\n\
                 secret_token = \"nobody-carries-this\"\n"
            ))
            .is_err(),
            "an unknown key in the webhook section is refused"
        );
    }

    // ─── The retired title-model key ─────────────────────────────────────

    /// Decision 0077's config pin: `title_model` is no key of this file.
    /// A stale deployment file still naming it is refused at the load —
    /// the unknown-key rule doing its job — instead of decoding into
    /// configuration for a feature that no longer exists.
    #[test]
    fn a_leftover_title_model_key_is_refused_at_the_load() {
        assert!(
            load(&full(
                "log = \"stderr\"",
                "title_model = \"cheap/title-model\"\n"
            ))
            .is_err(),
            "the retired key is refused like any unknown key"
        );
    }

    #[test]
    fn an_empty_or_whitespace_privacy_address_is_refused() {
        for address in ["privacy_policy = \"\"\n", "privacy_policy = \"   \"\n"] {
            let refused = loaded("log = \"stderr\"", address)
                .resolve_privacy_policy()
                .expect_err("a blank legal pointer must be refused");
            assert!(
                matches!(refused, StartError::PrivacyPolicyEmpty),
                "the refusal names the empty address; got {refused}"
            );
        }
    }
}
