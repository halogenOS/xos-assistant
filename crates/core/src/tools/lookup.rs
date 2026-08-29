//! What the lookups share: the project organization, the one-bounded-GET
//! contract with the client that enforces it, the bounded POST seam beside
//! it, the decode helpers, the per-process response cache and the
//! path-safety checks.
//!
//! Transport verdicts are typed here and worded by the caller
//! ([`WireFailure`], 2026-08-29): the GET paths word them with the shared
//! sentences they always answered, while a caller that maps failures to
//! taught results of its own — the web search, whose unit forbids the bare
//! "answered HTTP {status}" wording — reads the verdict and the answer's
//! status itself. One place decides what the wire did; each caller owns
//! what it says about it.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// The project organization every lookup addresses — one org on each host,
/// same name everywhere.
pub(crate) const ORGANIZATION: &str = "halogenOS";

/// The largest answer body a lookup reads before refusing, in bytes —
/// shared by every fetch: the two JSON dialects answering one object
/// each, and the wiki's plain-text page and rendered-index reads. A body
/// past this bound is not an answer this unit has a reading for.
pub(crate) const MAX_BODY_BYTES: usize = 1024 * 1024;

/// The client every lookup performs its one bounded GET with: the given
/// request timeout, and no redirect following — one GET means one, so a
/// redirect answer surfaces as the tool error [`bounded_get`] names instead
/// of becoming a second request to wherever the host points.
///
/// # Panics
///
/// If the HTTP client cannot be built — a broken TLS stack at construction,
/// not a runtime condition.
#[must_use]
pub(crate) fn lookup_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("the HTTP client builds")
}

/// What the wire did when it produced no answer the caller can read: the
/// transport's verdict, typed instead of worded. The GET paths render it
/// through [`WireFailure::worded`] into the sentences they have always
/// answered; a caller with taught results of its own words it there
/// instead, so no shared wording leaks into a surface that forbids it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireFailure {
    /// The request did not answer within the client's time bound.
    Timeout,
    /// The host could not be reached at all.
    Unreachable,
    /// The answer body ran past [`MAX_BODY_BYTES`].
    OverBound,
    /// The connection ended mid-body.
    Truncated,
    /// The body arrived whole and is not the JSON the caller asked for.
    Unreadable,
}

impl WireFailure {
    /// The shared wording naming `who` — the one spelling of each transport
    /// failure the GET lookups report to the model.
    pub(crate) fn worded(self, who: &str) -> String {
        match self {
            Self::Timeout => format!("{who} did not answer within the time bound"),
            Self::Unreachable => format!("{who} could not be reached"),
            Self::OverBound => format!("{who} answered more than the size bound"),
            Self::Truncated => format!("the answer from {who} ended mid-body"),
            Self::Unreadable => format!("{who} answered something that is not JSON"),
        }
    }
}

/// The verdict one transport error carries: a timeout is its own case, and
/// everything else is an unreachable host — a raw transport error's own
/// rendering is not this module's to bound, and never reaches a caller.
fn wire_failure(error: &reqwest::Error) -> WireFailure {
    if error.is_timeout() {
        WireFailure::Timeout
    } else {
        WireFailure::Unreachable
    }
}

/// What a bounded POST answered when the wire itself worked.
pub(crate) enum JsonAnswer {
    /// A success body, decoded as JSON.
    Body(Value),
    /// A non-success answer: the status as a NUMBER and the decoded body
    /// where one arrived. Deliberately unworded — the caller that maps
    /// statuses to its own taught results reads both and says its own
    /// thing, and a redirect arrives here like any other non-success,
    /// because the shared client follows none.
    Refused { status: u16, body: Option<Value> },
}

/// One bounded POST of a JSON body (decided 2026-08-27, with the web
/// search): the same client discipline as [`bounded_get`] — its timeout,
/// its refusal to follow redirects, its body cap — with the status handed
/// back instead of worded, because the caller's unit forbids the shared
/// status sentence. Headers are sent as given and never echoed anywhere.
pub(crate) async fn bounded_post_json(
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, String)],
    body: &Value,
) -> Result<JsonAnswer, WireFailure> {
    let mut request = client.post(url).json(body);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    let response = request.send().await.map_err(|error| wire_failure(&error))?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        // A refused answer's body is read under the same cap and decoded
        // best-effort: it is what disambiguates one refusal from another,
        // and a body that is missing, over-bound or not JSON simply leaves
        // the caller with the status alone.
        let body = bounded_body(response)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        return Ok(JsonAnswer::Refused { status, body });
    }
    let bytes = bounded_body(response).await?;
    serde_json::from_slice(&bytes)
        .map(JsonAnswer::Body)
        .map_err(|_| WireFailure::Unreadable)
}

/// One bounded GET: send, check the status, read the body up to
/// [`MAX_BODY_BYTES`], decode JSON. Every failure is a plain sentence naming
/// `who` — never a raw transport error, whose rendering is not this module's
/// to bound, and never any header value.
pub(crate) async fn bounded_get(
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, String)],
    who: &str,
) -> Result<Value, String> {
    let response = send_get(client, url, headers, who).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "{who} answered: not found — check the name and the reference"
        ));
    }
    checked_success(&response, who)?;
    let body = read_body(response, who).await?;
    serde_json::from_slice(&body).map_err(|_| WireFailure::Unreadable.worded(who))
}

/// What the text-body GET answered when the wire itself worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextAnswer {
    /// A success body, decoded lossily as UTF-8 — a host that says plain
    /// text today may answer HTML on a 200 tomorrow, and such a body
    /// passes through as text and reads as what it is.
    Body(String),
    /// The host answered 404: no such resource. A named answer instead of
    /// an error, because the wiki lookup caches it — negative caching
    /// bounds a model guessing page names.
    Missing,
}

/// The text-body sibling of [`bounded_get`] (decided 2026-08-23): one
/// bounded GET whose success body is decoded lossily as UTF-8 instead of
/// as JSON, and whose 404 is a typed [`TextAnswer::Missing`] instead of a
/// worded error — the caller owns the missing-resource wording and the
/// negative cache. Every other failure is the same plain sentence naming
/// `who`.
pub(crate) async fn bounded_get_text(
    client: &reqwest::Client,
    url: &str,
    who: &str,
) -> Result<TextAnswer, String> {
    let response = send_get(client, url, &[], who).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(TextAnswer::Missing);
    }
    checked_success(&response, who)?;
    let body = read_body(response, who).await?;
    Ok(TextAnswer::Body(
        String::from_utf8_lossy(&body).into_owned(),
    ))
}

/// Send one GET under the shared failure wording: a timeout and an
/// unreachable host each become the plain sentence naming `who`.
async fn send_get(
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, String)],
    who: &str,
) -> Result<reqwest::Response, String> {
    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    request
        .send()
        .await
        .map_err(|error| wire_failure(&error).worded(who))
}

/// The shared status discipline past the caller's own 404 handling: a
/// redirect is a refusal — one GET means one — and so is any non-success
/// status.
fn checked_success(response: &reqwest::Response, who: &str) -> Result<(), String> {
    let status = response.status();
    if status.is_redirection() {
        return Err(format!(
            "{who} answered with a redirect, which this lookup does not follow"
        ));
    }
    if !status.is_success() {
        return Err(format!("{who} answered HTTP {}", status.as_u16()));
    }
    Ok(())
}

/// Read one success body up to [`MAX_BODY_BYTES`], under the shared
/// failure wording.
async fn read_body(response: reqwest::Response, who: &str) -> Result<Vec<u8>, String> {
    bounded_body(response)
        .await
        .map_err(|failure| failure.worded(who))
}

/// The body read itself: at most [`MAX_BODY_BYTES`], with the transport's
/// verdict typed. A body ending mid-stream is told apart from one that ran
/// past the cap, and from a timeout inside the read.
async fn bounded_body(mut response: reqwest::Response) -> Result<Vec<u8>, WireFailure> {
    let mut body: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > MAX_BODY_BYTES {
                    return Err(WireFailure::OverBound);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(error) if error.is_timeout() => return Err(WireFailure::Timeout),
            Err(_) => return Err(WireFailure::Truncated),
        }
    }
    Ok(body)
}

/// The per-process response cache every cached lookup keeps (hoisted here
/// 2026-08-29, with the web search): live entries serve for the TTL it is
/// constructed with, expired ones are swept on every read, and the whole
/// map is cleared when an insert meets the cap — losing a cache costs one
/// refetch per key, while an unbounded map would grow with every address or
/// query anything ever asked for. One writing of that shape, because two
/// caches deciding it separately is two places to get it wrong; the TTL,
/// the cap and the key are each caller's own.
pub(crate) struct MemoryCache<T> {
    ttl: Duration,
    cap: usize,
    entries: Mutex<HashMap<String, (Instant, T)>>,
}

impl<T: Clone> MemoryCache<T> {
    pub(crate) fn new(ttl: Duration, cap: usize) -> Self {
        Self {
            ttl,
            cap,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The live cached value for one key, expired entries swept.
    pub(crate) async fn cached(&self, key: &str) -> Option<T> {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();
        entries.retain(|_, (at, _)| now.duration_since(*at) < self.ttl);
        entries.get(key).map(|(_, value)| value.clone())
    }

    /// Record one value, clearing the whole map first when the cap is hit.
    pub(crate) async fn remember(&self, key: String, value: T) {
        let mut entries = self.entries.lock().await;
        if entries.len() >= self.cap {
            tracing::debug!("a lookup cache reached its cap and was cleared");
            entries.clear();
        }
        entries.insert(key, (Instant::now(), value));
    }
}

/// One required string field out of a decoded answer, by JSON pointer. A
/// missing or non-string field is the named refusal the decoders report.
pub(crate) fn read_string(answer: &Value, pointer: &str, who: &str) -> Result<String, String> {
    answer
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            format!(
                "{who} answered without `{}`",
                pointer.trim_start_matches('/').replace('/', ".")
            )
        })
}

/// A field bounded to `limit` characters, an ellipsis marking the cut.
pub(crate) fn truncated(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut bounded: String = text.chars().take(limit).collect();
    bounded.push('…');
    bounded
}

/// A repository name safe to place as one URL path segment: letters, digits,
/// dot, dash and underscore, no dot-only segment, non-empty.
pub(crate) fn valid_repository(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// A git reference safe to place in a URL path: the repository characters
/// plus the slash a branch name may carry, with no empty or dot-only
/// segment — which is what keeps a traversal out of the path.
pub(crate) fn valid_reference(reference: &str) -> bool {
    !reference.is_empty() && reference.split('/').all(valid_repository)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_names_reject_separators_and_traversal() {
        assert!(valid_repository("android_manifest"));
        assert!(valid_repository("device-oneplus.sdm845"));
        assert!(!valid_repository(""));
        assert!(!valid_repository("a/b"));
        assert!(!valid_repository(".."));
        assert!(!valid_repository("a b"));
        assert!(!valid_repository("a?b=c"));
    }

    #[test]
    fn references_allow_branch_slashes_but_never_traversal() {
        assert!(valid_reference("9b6526c3663f"));
        assert!(valid_reference("XOS-16.2"));
        assert!(valid_reference("feature/lookup"));
        assert!(!valid_reference("feature//lookup"));
        assert!(!valid_reference("../../../etc"));
        assert!(!valid_reference("a/.."));
        assert!(!valid_reference(""));
    }

    #[test]
    fn truncation_bounds_by_characters_and_marks_the_cut() {
        assert_eq!(truncated("short", 10), "short");
        assert_eq!(truncated("exactly-10", 10), "exactly-10");
        assert_eq!(truncated("longer than ten", 10), "longer tha…");
    }
}
