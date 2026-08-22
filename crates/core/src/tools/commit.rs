//! The commit lookup: one bounded GET against the project's canonical forge.
//!
//! The forge speaks the Forgejo v1 API and answers unauthenticated — commits
//! are public data, and the forge is the truth for code. The tool reads one
//! commit by repository name within the project organization and a hash or
//! reference, and returns the compact form: subject, author, date, link. It
//! writes nowhere.

use std::time::Duration;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::{Value, json};

use crate::message::Authority;
use crate::tools::lookup::{
    ORGANIZATION, bounded_get, lookup_client, read_string, truncated, valid_reference,
    valid_repository,
};

/// The registered name the model calls the tool by.
pub const NAME: &str = "lookup_commit";

/// The canonical forge — the real host the base URL defaults to.
pub const DEFAULT_BASE_URL: &str = "https://git.halogenos.org";

/// The default request timeout. A construction parameter so tests construct
/// short bounds instead of waiting production ones.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The authority this tool requires — the member floor registration
/// enforces (decision 0043's closure).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// How much of a commit subject the compact result carries.
pub const SUBJECT_LIMIT: usize = 200;

/// How many characters of the hash the compact result shows — enough to be
/// unambiguous in any repository the project runs, short enough to read.
pub const SHORT_HASH: usize = 12;

/// The commit lookup tool.
pub struct CommitLookup {
    client: reqwest::Client,
    base_url: String,
}

impl CommitLookup {
    /// Construct against a base URL — the real host by default, a loopback
    /// server in tests — with the given request timeout. The client follows
    /// no redirects, per the one-bounded-GET contract: a redirect answer is
    /// a tool error, never a second request.
    ///
    /// # Panics
    ///
    /// If the HTTP client cannot be built — a broken TLS stack at
    /// construction, not a runtime condition.
    #[must_use]
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Self {
        Self {
            client: lookup_client(timeout),
            base_url: base_url.into(),
        }
    }
}

impl ToolHandler<CoreEvent> for CommitLookup {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: format!(
                "Look up one commit in a halogenOS repository on the project's canonical \
                 forge. Give the repository's name within the {ORGANIZATION} organization \
                 and a commit hash, branch or tag; the answer carries the commit's \
                 subject, author, date and link."
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "repository": {
                        "type": "string",
                        "description": "The repository name within the project organization, \
                                        for example android_manifest."
                    },
                    "reference": {
                        "type": "string",
                        "description": "A commit hash, branch name or tag."
                    }
                },
                "required": ["repository", "reference"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        input: &'a str,
        _ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            let arguments: Value = match serde_json::from_str(input) {
                Ok(arguments) => arguments,
                Err(_) => {
                    return ToolOutcome::Error(
                        "the input is not a JSON object with `repository` and `reference`".into(),
                    );
                }
            };
            let Some(repository) = arguments["repository"].as_str() else {
                return ToolOutcome::Error("the input names no `repository`".into());
            };
            let Some(reference) = arguments["reference"].as_str() else {
                return ToolOutcome::Error("the input names no `reference`".into());
            };
            if !valid_repository(repository) {
                return ToolOutcome::Error(
                    "the repository must be a plain name within the project organization".into(),
                );
            }
            if !valid_reference(reference) {
                return ToolOutcome::Error(
                    "the reference must be a commit hash, branch or tag name".into(),
                );
            }

            let url = format!(
                "{}/api/v1/repos/{ORGANIZATION}/{repository}/git/commits/{reference}",
                self.base_url
            );
            let answer = match bounded_get(&self.client, &url, &[], "the forge").await {
                Ok(answer) => answer,
                Err(error) => return ToolOutcome::Error(error),
            };
            match decode(repository, &answer) {
                Ok(compact) => ToolOutcome::Done(compact),
                Err(error) => ToolOutcome::Error(error),
            }
        })
    }
}

/// The Forgejo dialect's decode into the compact result. Every field the
/// compact form needs must be present; a missing one is the forge answering
/// in a shape this decoder does not speak, reported as the tool error.
fn decode(repository: &str, answer: &Value) -> Result<String, String> {
    let sha = read_string(answer, "/sha", "the forge")?;
    let link = read_string(answer, "/html_url", "the forge")?;
    let message = read_string(answer, "/commit/message", "the forge")?;
    let author = read_string(answer, "/commit/author/name", "the forge")?;
    let date = read_string(answer, "/commit/author/date", "the forge")?;
    let subject = truncated(message.lines().next().unwrap_or_default(), SUBJECT_LIMIT);
    // Bounded by characters, not bytes: the value is whatever the forge
    // answered, and a byte slice across a multi-byte character would panic
    // where every other shape refusal is a sentence.
    let short: String = sha.chars().take(SHORT_HASH).collect();
    Ok(format!(
        "Commit {short} in {ORGANIZATION}/{repository}\n\
         Subject: {subject}\n\
         Author: {author}\n\
         Date: {date}\n\
         Link: {link}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_decode_produces_the_compact_result() {
        let answer = json!({
            "sha": "9b6526c3663f3ad4e3974ef713faedd71ee8ec1e",
            "html_url": "https://example.invalid/commit/9b6526c",
            "commit": {
                "message": "Track the manifest\n\nA body the compact form drops.\n",
                "author": { "name": "A. Committer", "date": "2026-08-17T01:23:26+02:00" }
            }
        });
        let compact = decode("android_manifest", &answer).expect("the shape decodes");
        assert_eq!(
            compact,
            "Commit 9b6526c3663f in halogenOS/android_manifest\n\
             Subject: Track the manifest\n\
             Author: A. Committer\n\
             Date: 2026-08-17T01:23:26+02:00\n\
             Link: https://example.invalid/commit/9b6526c"
        );
    }

    #[test]
    fn a_missing_field_is_a_named_error_not_a_panic() {
        let error = decode("r", &json!({ "sha": "abc" })).expect_err("the shape is refused");
        assert!(
            error.contains("html_url"),
            "the error names the field: {error}"
        );
    }

    #[test]
    fn a_multi_byte_hash_is_bounded_by_characters_not_bytes() {
        // The hash is remote-supplied: a value whose twelfth character is
        // multi-byte must shorten cleanly, never panic on a byte boundary.
        let answer = json!({
            "sha": "abcdefghijk……rest",
            "html_url": "https://example.invalid/c",
            "commit": { "message": "m", "author": { "name": "A", "date": "D" } }
        });
        let compact = decode("r", &answer).expect("the shape decodes");
        assert!(
            compact.starts_with("Commit abcdefghijk… in"),
            "the short hash takes whole characters: {compact}"
        );
    }

    #[test]
    fn an_over_limit_subject_is_truncated() {
        let long = "s".repeat(SUBJECT_LIMIT + 50);
        let answer = json!({
            "sha": "abcdef0123456789",
            "html_url": "https://example.invalid/c",
            "commit": { "message": long, "author": { "name": "A", "date": "D" } }
        });
        let compact = decode("r", &answer).expect("the shape decodes");
        let subject = compact
            .lines()
            .find_map(|line| line.strip_prefix("Subject: "))
            .expect("the subject line exists");
        assert!(
            subject.chars().count() <= SUBJECT_LIMIT + 1,
            "bounded plus the ellipsis"
        );
        assert!(subject.ends_with('…'));
    }
}
