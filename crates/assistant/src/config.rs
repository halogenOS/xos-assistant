//! The process configuration: one TOML file, located by the binary's single
//! command-line argument.
//!
//! The file names paths, the model id and the endpoint overrides. Secrets are
//! named indirectly — an environment variable name or a file path per secret —
//! and their values never appear in the configuration file, in any error text,
//! or in any log line. Every error below names where a value was looked for,
//! never what was found.

use std::path::{Path, PathBuf};

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

/// The endpoint overrides. Both default to the real hosts; tests point them
/// at loopback servers.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoints {
    /// The Telegram Bot API root.
    pub telegram: Option<String>,
    /// The `OpenRouter` base URL.
    pub openrouter: Option<String>,
}

/// Where each secret is found.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Secrets {
    /// The Telegram bot token.
    pub bot_token: SecretRef,
    /// The `OpenRouter` API key.
    pub openrouter_key: SecretRef,
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
}
