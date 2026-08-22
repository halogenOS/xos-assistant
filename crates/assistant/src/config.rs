//! The process configuration: one TOML file, located by the binary's single
//! command-line argument.
//!
//! The file names paths, the model id and the endpoint overrides. Secrets are
//! named indirectly — an environment variable name or a file path per secret —
//! and their values never appear in the configuration file, in any error text,
//! or in any log line. Every error below names where a value was looked for,
//! never what was found.

use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};

use assistant_core::{Budget, ProtectionConfig};
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
    /// The endpoint overrides; omitted entries keep the real hosts.
    #[serde(default)]
    pub endpoints: Endpoints,
    /// The answering budgets; omitted fields keep the stated defaults.
    #[serde(default)]
    pub protection: Protection,
    /// Where the two secrets are found — never the secrets themselves.
    pub secrets: Secrets,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A carrier for the log key alone, so each spelling decodes through
    /// the same serde path the configuration file uses.
    #[derive(Deserialize)]
    struct Probe {
        log: LogDestination,
    }

    fn decode(toml: &str) -> LogDestination {
        toml::from_str::<Probe>(toml)
            .unwrap_or_else(|error| panic!("{toml:?} decodes: {error}"))
            .log
    }

    #[test]
    fn the_bare_word_names_the_console() {
        assert!(matches!(
            decode("log = \"stderr\""),
            LogDestination::Console(ConsoleStream::Stderr)
        ));
    }

    #[test]
    fn any_other_bare_string_names_a_file() {
        let LogDestination::File(file) = decode("log = \"assistant.log\"") else {
            panic!("a bare path string decodes as a file destination");
        };
        assert_eq!(file.path(), Path::new("assistant.log"));
    }

    #[test]
    fn the_table_spelling_names_a_file_even_one_named_after_the_console() {
        let LogDestination::File(file) = decode("log = { file = \"stderr\" }") else {
            panic!("the table spelling decodes as a file destination");
        };
        assert_eq!(file.path(), Path::new("stderr"));
    }

    /// A carrier for the protection table alone, decoding through the same
    /// serde path the configuration file uses. `#[serde(default)]` mirrors
    /// the field's attribute on [`Configuration`], so the absent table
    /// decodes here exactly as it does there.
    #[derive(Deserialize)]
    struct ProtectionProbe {
        #[serde(default)]
        protection: Protection,
    }

    fn decode_protection(toml: &str) -> Result<ProtectionConfig, StartError> {
        toml::from_str::<ProtectionProbe>(toml)
            .unwrap_or_else(|error| panic!("{toml:?} decodes: {error}"))
            .protection
            .resolve()
    }

    #[test]
    fn an_absent_protection_table_takes_the_stated_defaults() {
        let resolved = decode_protection("").expect("the defaults resolve");
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
        let resolved = decode_protection("[protection]\nprincipal_answers = 2\n")
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
        let resolved = decode_protection("[protection]\nchannel_window_seconds = 0\n")
            .expect("the disabling window resolves");
        assert!(resolved.channel.is_none(), "a zero window disables");
        assert!(
            resolved.principal.is_some(),
            "the other budget stays enabled"
        );
    }

    #[test]
    fn an_over_year_window_is_refused_naming_the_field() {
        let refused = decode_protection("[protection]\nprincipal_window_seconds = 1000000000000\n")
            .expect_err("a window past the year bound must be refused");
        match refused {
            StartError::ProtectionWindowOverBound { field } => {
                assert_eq!(field, "principal_window_seconds");
            }
            other => panic!("the refusal names the window field; got {other}"),
        }
        // The bound itself still resolves.
        assert!(
            decode_protection("[protection]\nchannel_window_seconds = 31536000\n")
                .expect("the one-year window resolves")
                .channel
                .is_some()
        );
    }

    #[test]
    fn a_zero_answer_count_is_refused_naming_the_field() {
        let refused = decode_protection("[protection]\nchannel_answers = 0\n")
            .expect_err("a zero count must be refused");
        match refused {
            StartError::ProtectionAnswersZero { field } => {
                assert_eq!(field, "channel_answers");
            }
            other => panic!("the refusal names the zero field; got {other}"),
        }
        // Refused even beside the window that would disable the budget.
        assert!(
            decode_protection(
                "[protection]\nprincipal_answers = 0\nprincipal_window_seconds = 0\n"
            )
            .is_err(),
            "a zero count beside a disabling window is still a refusal"
        );
    }

    #[test]
    fn an_unknown_protection_key_is_refused() {
        assert!(
            toml::from_str::<ProtectionProbe>("[protection]\nprincipal_burst = 3\n").is_err(),
            "unknown keys in the protection table are refused"
        );
    }
}
