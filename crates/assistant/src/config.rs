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

use assistant_core::{Budget, OperatorConfig, ProtectionConfig};
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
    /// created under.
    pub model: String,
    /// The provider's identifier for the model conversation titles are
    /// derived with; absent, titles derive on the main model. Resolved
    /// through [`Configuration::resolve_title_model`], which trims and
    /// refuses an empty value.
    pub title_model: Option<String>,
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
    /// The address the privacy command answers with; absent answers the
    /// not-yet-published line. Resolved through
    /// [`Configuration::resolve_privacy_policy`], which refuses an empty
    /// value.
    pub privacy_policy: Option<String>,
    /// The moderation bot's handle the report tool files toward; absent
    /// leaves the report tool unregistered. Resolved through
    /// [`Configuration::resolve_moderation_handle`], which trims, strips a
    /// leading `@` and refuses an empty value.
    pub moderation_handle: Option<String>,
    /// Where the two secrets are found — never the secrets themselves.
    pub secrets: Secrets,
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
    /// The `OpenRouter` base URL.
    pub openrouter: Option<String>,
    /// The canonical forge's base URL — the commit lookup's host.
    pub forge: Option<String>,
    /// The mirror API's base URL — the release lookup's host.
    pub mirror: Option<String>,
    /// The wiki's raw base address — the wiki lookup's host. Resolved
    /// through [`Configuration::resolve_wiki_endpoint`], which trims and
    /// refuses an empty value.
    pub wiki: Option<String>,
}

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

    /// The model id conversation titles are derived with, `None` when the
    /// key is absent — under which titles derive on the main model, so a
    /// deployment pinned to one model never has its title traffic sent to a
    /// model nobody named.
    ///
    /// # Errors
    ///
    /// [`StartError::TitleModelEmpty`] when the key is present but empty or
    /// whitespace — an empty id would name no model and fail every title
    /// derivation silently; omitting the key is how the main-model default
    /// is chosen. A surviving id resolves trimmed: the pad is file
    /// formatting, never identity.
    pub fn resolve_title_model(&self) -> Result<Option<String>, StartError> {
        match &self.title_model {
            Some(model) => match model.trim() {
                "" => Err(StartError::TitleModelEmpty),
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
    /// The `OpenRouter` API key.
    pub openrouter_key: SecretRef,
    /// The mirror API token the release lookup sends as its authorization
    /// header. Optional: absent, the lookup runs unauthenticated at the
    /// mirror's lower rate limit and sends no header.
    pub mirror_token: Option<SecretRef>,
}

/// One secret's indirection: an environment variable name or a file path,
/// exactly one of the two.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretRef {
    /// The environment variable holding the value.
    pub env: Option<String>,
    /// The file holding the value; surrounding whitespace is trimmed, so a
    /// trailing newline in a secrets file is harmless.
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
        match (&self.env, &self.file) {
            (Some(name), None) => std::env::var(name).map_err(|_| StartError::SecretUnread {
                key,
                source_name: format!("environment variable {name}"),
            }),
            (None, Some(path)) => std::fs::read_to_string(path)
                .map(|value| value.trim().to_owned())
                .map_err(|_| StartError::SecretUnread {
                    key,
                    source_name: format!("file {}", path.display()),
                }),
            _ => Err(StartError::SecretRef { key }),
        }
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
             [secrets.openrouter_key]\n\
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

    // ─── The title model ─────────────────────────────────────────────────

    #[test]
    fn the_title_model_resolves_trimmed_and_absent_means_the_main_model() {
        let configuration = loaded(
            "log = \"stderr\"",
            "title_model = \" cheap/title-model \"\n",
        );
        assert_eq!(
            configuration
                .resolve_title_model()
                .expect("the model id resolves")
                .as_deref(),
            Some("cheap/title-model"),
            "the pad is file formatting, never identity"
        );
        assert_eq!(
            loaded("log = \"stderr\"", "")
                .resolve_title_model()
                .expect("the absent key resolves"),
            None,
            "no title model by default — titles derive on the main model"
        );
    }

    #[test]
    fn an_empty_or_whitespace_title_model_is_refused() {
        for spelling in ["title_model = \"\"\n", "title_model = \"   \"\n"] {
            let refused = loaded("log = \"stderr\"", spelling)
                .resolve_title_model()
                .expect_err("an empty model id must be refused");
            assert!(
                matches!(refused, StartError::TitleModelEmpty),
                "the refusal names the empty model id; got {refused}"
            );
        }
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
