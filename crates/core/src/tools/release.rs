//! The release lookup: one bounded GET against the mirror's builds
//! repository.
//!
//! Releases exist only on the mirror organization's builds repository — the
//! data the project site itself reads — and that mirror speaks the GitHub v3
//! API. The tool reads one release by tag, defaulting to the latest, and
//! returns the compact form: version, date, link, and the per-device assets
//! summarized. It writes nowhere.
//!
//! An optional API token raises the mirror's rate limit from sixty to five
//! thousand requests per hour. It is a secret: held in memory, sent only as
//! the authorization header, never stored, never logged, never part of an
//! error string — which is why this type derives no `Debug`.

use std::time::Duration;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::{Value, json};

use crate::message::Authority;
use crate::tools::lookup::{
    ORGANIZATION, bounded_get, lookup_client, read_string, truncated, valid_reference,
};

/// The registered name the model calls the tool by.
pub const NAME: &str = "lookup_release";

/// The mirror's API — the real host the base URL defaults to.
pub const DEFAULT_BASE_URL: &str = "https://api.github.com";

/// The builds repository on the mirror organization: the only home of
/// releases.
pub const BUILDS_REPOSITORY: &str = "builds";

/// The default request timeout. A construction parameter so tests construct
/// short bounds instead of waiting production ones.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The tool's own rate window, bound at the assembly: this many calls may
/// run in any trailing [`WINDOW_SECONDS`], and the next is refused with
/// the framework's per-tool rate-limit text. The numbers are the
/// operator's (2026-08-30), set after a session ground this lookup in a
/// loop at a pace the conversation-wide window never noticed.
pub const WINDOW_CALLS: usize = 6;

/// The trailing span [`WINDOW_CALLS`] is counted over, in seconds.
pub const WINDOW_SECONDS: i64 = 60;

/// The authority this tool requires — the bar the admission hook's
/// provenance reading is compared against at every call (decision 0043).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// How many assets the compact result lists before summarizing the rest as
/// a count.
pub const ASSETS_LISTED: usize = 12;

/// How much of a release title or an asset name one line carries.
pub const LINE_LIMIT: usize = 120;

/// The release lookup tool.
pub struct ReleaseLookup {
    client: reqwest::Client,
    base_url: String,
    /// The optional mirror token, held in memory for the header and nothing
    /// else.
    token: Option<String>,
}

impl ReleaseLookup {
    /// Construct against a base URL — the real host by default, a loopback
    /// server in tests — with the optional mirror token and the given
    /// request timeout. An absent token sends no authorization header. The
    /// client follows no redirects, per the one-bounded-GET contract: a
    /// redirect answer is a tool error, never a second request.
    ///
    /// # Panics
    ///
    /// If the HTTP client cannot be built — a broken TLS stack at
    /// construction, not a runtime condition.
    #[must_use]
    pub fn new(base_url: impl Into<String>, token: Option<String>, timeout: Duration) -> Self {
        Self {
            client: lookup_client(timeout),
            base_url: base_url.into(),
            token,
        }
    }
}

impl ToolHandler<CoreEvent> for ReleaseLookup {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: format!(
                "Look up a halogenOS release in the project's {ORGANIZATION}/{BUILDS_REPOSITORY} \
                 repository — the home of every build. Give a release tag, or omit it for \
                 the latest release; the answer carries the version, date, link and the \
                 per-device assets."
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tag": {
                        "type": "string",
                        "description": "The release tag. Omit it for the latest release."
                    }
                },
                "required": []
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(NAME, REQUIRED_AUTHORITY);

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
                        "the input is not a JSON object; pass an empty object for the \
                         latest release"
                            .into(),
                    );
                }
            };
            let tag = match &arguments["tag"] {
                Value::String(tag) if !tag.is_empty() => Some(tag.as_str()),
                _ => None,
            };
            if let Some(tag) = tag
                && !valid_reference(tag)
            {
                return ToolOutcome::Error("the tag must be a plain release tag name".into());
            }

            let repository = format!("repos/{ORGANIZATION}/{BUILDS_REPOSITORY}/releases");
            let url = match tag {
                Some(tag) => format!("{}/{repository}/tags/{tag}", self.base_url),
                None => format!("{}/{repository}/latest", self.base_url),
            };
            // The mirror requires a user agent; the token, when configured,
            // rides only here, as the authorization header.
            let mut headers: Vec<(&str, String)> = vec![
                ("user-agent", "halogenos-assistant".into()),
                ("accept", "application/vnd.github+json".into()),
            ];
            if let Some(token) = &self.token {
                headers.push(("authorization", format!("Bearer {token}")));
            }
            let answer = match bounded_get(&self.client, &url, &headers, "the mirror").await {
                Ok(answer) => answer,
                Err(error) => return ToolOutcome::Error(error),
            };
            match decode(&answer) {
                Ok(compact) => ToolOutcome::Done(compact),
                Err(error) => ToolOutcome::Error(error),
            }
        })
    }
}

/// The GitHub v3 dialect's decode into the compact result.
fn decode(answer: &Value) -> Result<String, String> {
    // Writing into a String cannot fail, which is why the write results
    // below are discarded.
    use std::fmt::Write as _;

    let tag = read_string(answer, "/tag_name", "the mirror")?;
    let link = read_string(answer, "/html_url", "the mirror")?;
    let date = read_string(answer, "/published_at", "the mirror")?;
    let title = answer["name"].as_str().unwrap_or_default();
    let mut compact = format!("Release {tag}", tag = truncated(&tag, LINE_LIMIT));
    if !title.is_empty() {
        let _ = write!(compact, " — {}", truncated(title, LINE_LIMIT));
    }
    let _ = write!(compact, "\nPublished: {date}\nLink: {link}");

    let assets = answer["assets"].as_array().cloned().unwrap_or_default();
    let _ = write!(compact, "\nAssets: {}", assets.len());
    for asset in assets.iter().take(ASSETS_LISTED) {
        let name = asset["name"].as_str().unwrap_or("(unnamed)");
        let _ = write!(compact, "\n- {}", truncated(name, LINE_LIMIT));
        if let Some(size) = asset["size"].as_u64() {
            let _ = write!(compact, " ({size} bytes)");
        }
    }
    if assets.len() > ASSETS_LISTED {
        let _ = write!(compact, "\n… and {} more", assets.len() - ASSETS_LISTED);
    }
    Ok(compact)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operator's numbers, pinned where they are defined: six calls in
    /// any trailing sixty seconds (decision 0169). A drift here is a
    /// different pacing decision and must be one on purpose.
    #[test]
    fn the_rate_window_is_the_operators_six_per_sixty() {
        assert_eq!(WINDOW_CALLS, 6);
        assert_eq!(WINDOW_SECONDS, 60);
    }

    #[test]
    fn the_decode_produces_the_compact_result() {
        let answer = json!({
            "tag_name": "20260707.2230.36-rb",
            "name": "[release build] XOS-16.2 for a device",
            "published_at": "2026-07-07T20:43:15Z",
            "html_url": "https://example.invalid/releases/tag/20260707.2230.36-rb",
            "assets": [
                { "name": "boot.img", "size": 100_663_296 },
                { "name": "halogenOS_Device-16.2.zip", "size": 1_186_987_330 }
            ]
        });
        let compact = decode(&answer).expect("the shape decodes");
        assert_eq!(
            compact,
            "Release 20260707.2230.36-rb — [release build] XOS-16.2 for a device\n\
             Published: 2026-07-07T20:43:15Z\n\
             Link: https://example.invalid/releases/tag/20260707.2230.36-rb\n\
             Assets: 2\n\
             - boot.img (100663296 bytes)\n\
             - halogenOS_Device-16.2.zip (1186987330 bytes)"
        );
    }

    #[test]
    fn an_over_limit_asset_list_is_summarized() {
        let assets: Vec<Value> = (0..ASSETS_LISTED + 3)
            .map(|n| json!({ "name": format!("asset-{n}.img"), "size": 1 }))
            .collect();
        let answer = json!({
            "tag_name": "t",
            "published_at": "d",
            "html_url": "l",
            "assets": assets
        });
        let compact = decode(&answer).expect("the shape decodes");
        assert!(compact.contains(&format!("Assets: {}", ASSETS_LISTED + 3)));
        assert!(compact.ends_with("… and 3 more"));
        assert_eq!(
            compact.matches("\n- ").count(),
            ASSETS_LISTED,
            "only the listed bound of assets gets its own line"
        );
    }

    #[test]
    fn a_missing_field_is_a_named_error_not_a_panic() {
        let error = decode(&json!({ "tag_name": "t" })).expect_err("the shape is refused");
        assert!(
            error.contains("html_url"),
            "the error names the field: {error}"
        );
    }
}
