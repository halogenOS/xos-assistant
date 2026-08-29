//! The search vendor behind the trait (decided 2026-08-25): one
//! implementation, owning the endpoint, the key and the response shape.
//! The tool above owns the envelope, the guard, the budget and the teaching
//! surface, and knows none of what is written here.
//!
//! The vendor takes a JSON POST at `/search` with the key in an `X-API-KEY`
//! header, and answers a JSON object whose `organic` array carries the
//! ranked rows. Recorded from the live probe of 2026-08-27, which is the
//! only citation there is — the vendor publishes no open API
//! documentation:
//!
//! - a page runs SHORT of what was asked for (eight rows for a ten-row
//!   request in the vendor's own samples), so nothing pins a row count;
//! - a row can arrive without `snippet`, and can carry fields this tool
//!   does not read (`position`, `date`, `sitelinks`, `attributes`), which
//!   are ignored rather than refused;
//! - there is NO total-results field anywhere in the answer, which is why
//!   the envelope promises no total and no more-pages flag;
//! - an authentication failure is 403 for a missing and for a refused key
//!   alike, with the distinction only in the JSON `message` body.
//!
//! The request sends `autocorrect: false` — an answer to a silently
//! corrected query would break the unit's own rule that what is sent, and
//! what is answered, is the query as written — and sends `gl` and `hl` only
//! where the deployment configured them, so an international group's
//! results are a deployment choice instead of a vendor default nobody
//! chose.

use std::time::Duration;

use agent_ledger::providers::BoxFuture;
use serde_json::{Value, json};

use crate::tools::lookup::{JsonAnswer, WireFailure, bounded_post_json, lookup_client};
use crate::tools::search::{SearchConfig, SearchFailure, SearchProvider, SearchRow};

/// The path the search is posted to, under the configured base address.
const SEARCH_PATH: &str = "/search";

/// The header the key travels in. The value is never logged, never
/// rendered and never carried into a tool result.
const KEY_HEADER: &str = "X-API-KEY";

/// How many results one request asks for. What arrives is what is
/// rendered: a short page is the vendor's normal answer, not an error.
const REQUESTED_RESULTS: u32 = 10;

/// The status that carries an authentication refusal, for a missing and a
/// refused key alike — the body's `message` is what tells them from any
/// other refusal at the same status.
const REFUSED_STATUS: u16 = 403;

/// The status that carries a rate limit.
const RATE_LIMITED_STATUS: u16 = 429;

/// What a 403's message must carry to read as a key refusal rather than
/// some other refusal at the same status: the marks the vendor's own
/// authentication messages use, matched case-folded on the `message` field
/// alone. Every mark names the AUTHENTICATION — the bare reason phrase
/// `forbidden` is deliberately not one of them, because it is what the
/// status itself already says and a region or plan refusal carries it just
/// as readily as a bad key.
const KEY_REFUSAL_MARKS: [&str; 3] = ["unauthorized", "unauthorised", "api key"];

/// The one search provider: the vendor's endpoint, its key and its response
/// shape, and nothing about the tool that calls it.
pub(crate) struct VendorSearch {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    /// The country code sent as `gl`, absent when the deployment set none.
    country: Option<String>,
    /// The language code sent as `hl`, absent when the deployment set none.
    language: Option<String>,
}

impl VendorSearch {
    /// Construct against the configured endpoint and key, with the given
    /// request timeout — the shared lookup client, so the timeout, the
    /// redirect refusal and the body cap are the ones every lookup runs
    /// under.
    pub(crate) fn new(config: SearchConfig, timeout: Duration) -> Self {
        Self {
            client: lookup_client(timeout),
            base_url: config.base_url,
            api_key: config.api_key,
            country: config.country,
            language: config.language,
        }
    }

    /// The request body for one query and page.
    fn body(&self, query: &str, page: u32) -> Value {
        let mut body = json!({
            "q": query,
            "page": page,
            "num": REQUESTED_RESULTS,
            "autocorrect": false,
        });
        let object = body.as_object_mut().expect("the body is a JSON object");
        if let Some(country) = &self.country {
            object.insert("gl".into(), json!(country));
        }
        if let Some(language) = &self.language {
            object.insert("hl".into(), json!(language));
        }
        body
    }
}

impl SearchProvider for VendorSearch {
    fn search<'a>(
        &'a self,
        query: &'a str,
        page: u32,
    ) -> BoxFuture<'a, Result<Vec<SearchRow>, SearchFailure>> {
        Box::pin(async move {
            let url = format!("{}{SEARCH_PATH}", self.base_url);
            let headers = [(KEY_HEADER, self.api_key.clone())];
            let answer = bounded_post_json(&self.client, &url, &headers, &self.body(query, page))
                .await
                .map_err(transport_failure)?;
            match answer {
                JsonAnswer::Body(body) => Ok(rows(&body)),
                JsonAnswer::Refused { status, body } => Err(refusal(status, body.as_ref())),
            }
        })
    }
}

/// What one transport verdict means to the tool above.
fn transport_failure(failure: WireFailure) -> SearchFailure {
    match failure {
        WireFailure::Timeout => SearchFailure::Timeout,
        WireFailure::Unreachable | WireFailure::Truncated => SearchFailure::Unreachable,
        WireFailure::OverBound | WireFailure::Unreadable => SearchFailure::Unreadable,
    }
}

/// What one refused answer means: the key refusal is told from any other
/// refusal at the same status by the vendor's own `message`, a rate limit
/// by its status, and everything else — a redirect included, which the
/// shared client never follows — is the unusable answer.
fn refusal(status: u16, body: Option<&Value>) -> SearchFailure {
    let message = body
        .and_then(|body| body.get("message"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    match status {
        RATE_LIMITED_STATUS => SearchFailure::RateLimited,
        REFUSED_STATUS if KEY_REFUSAL_MARKS.iter().any(|mark| message.contains(mark)) => {
            SearchFailure::RefusedKey
        }
        REFUSED_STATUS => SearchFailure::Refused,
        _ => SearchFailure::Unreadable,
    }
}

/// The ranked rows in one answer: every entry of `organic` carrying a title
/// and a link, its snippet where the row has one, and every other field
/// ignored. A row missing a title or a link is dropped rather than
/// rendered half — there is nothing to show for it.
fn rows(body: &Value) -> Vec<SearchRow> {
    body.get("organic")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some(SearchRow {
                        title: row.get("title")?.as_str()?.to_owned(),
                        link: row.get("link")?.as_str()?.to_owned(),
                        snippet: row
                            .get("snippet")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> SearchConfig {
        SearchConfig {
            base_url: "http://127.0.0.1:1".into(),
            api_key: "FAKE-SEARCH-KEY".into(),
            country: Some("de".into()),
            language: Some("en".into()),
        }
    }

    /// The request the vendor is sent: the query as written, the page and
    /// the requested count, autocorrect explicitly off, and the configured
    /// locale — never a vendor default nobody chose.
    #[test]
    fn the_request_body_sends_the_query_as_written_with_autocorrect_off() {
        let provider = VendorSearch::new(config(), Duration::from_secs(5));
        let body = provider.body("  Spaced   Query  ", 3);
        assert_eq!(body["q"], json!("  Spaced   Query  "));
        assert_eq!(body["page"], json!(3));
        assert_eq!(body["num"], json!(REQUESTED_RESULTS));
        assert_eq!(body["autocorrect"], json!(false));
        assert_eq!(body["gl"], json!("de"));
        assert_eq!(body["hl"], json!("en"));
    }

    /// An unconfigured locale sends no locale key at all, rather than a
    /// guess at one.
    #[test]
    fn an_unconfigured_locale_sends_no_locale_keys() {
        let provider = VendorSearch::new(
            SearchConfig {
                country: None,
                language: None,
                ..config()
            },
            Duration::from_secs(5),
        );
        let body = provider.body("kernel", 1);
        assert!(body.get("gl").is_none(), "no country, no gl");
        assert!(body.get("hl").is_none(), "no language, no hl");
    }

    /// The response shapes the grounding recorded: a short page, a row
    /// without a snippet, unknown fields ignored, and a half row dropped.
    #[test]
    fn the_rows_read_the_recorded_answer_shape() {
        let body = json!({
            "searchParameters": { "q": "kernel", "page": 2 },
            "organic": [
                {
                    "title": "A page",
                    "link": "https://en.wikipedia.org/wiki/Kernel",
                    "snippet": "A snippet.",
                    "position": 1,
                    "date": "2026-01-01",
                    "sitelinks": [{ "title": "x", "link": "y" }],
                    "attributes": { "k": "v" }
                },
                { "title": "No snippet here", "link": "https://example.invalid/page" },
                { "title": "Half a row" }
            ]
        });
        let rows = rows(&body);
        assert_eq!(rows.len(), 2, "the half row is dropped, not rendered half");
        assert_eq!(rows[0].snippet.as_deref(), Some("A snippet."));
        assert_eq!(rows[1].snippet, None, "a row can arrive without a snippet");
        assert_eq!(rows[1].title, "No snippet here");
    }

    /// An answer with no organic array — and one with none of the fields —
    /// reads as an empty page, which the envelope words honestly, rather
    /// than as a failure.
    #[test]
    fn an_answer_without_rows_reads_as_an_empty_page() {
        assert!(rows(&json!({ "organic": [] })).is_empty());
        assert!(rows(&json!({})).is_empty());
    }

    /// The refusals, told apart exactly as the grounding says they can be:
    /// the key by the 403's own message, the rate limit by its status, and
    /// anything else by neither.
    #[test]
    fn the_refusals_are_distinguished_by_status_and_message() {
        assert_eq!(
            refusal(403, Some(&json!({ "message": "Unauthorized." }))),
            SearchFailure::RefusedKey
        );
        assert_eq!(
            refusal(403, Some(&json!({ "message": "Invalid API key" }))),
            SearchFailure::RefusedKey
        );
        assert_eq!(
            refusal(403, Some(&json!({ "message": "Not enough credits" }))),
            SearchFailure::Refused,
            "a 403 that is not about the key is its own refusal"
        );
        assert_eq!(
            refusal(403, Some(&json!({ "message": "Forbidden" }))),
            SearchFailure::Refused,
            "the status's own reason phrase names no key and reads as the \
             other refusal"
        );
        assert_eq!(refusal(403, None), SearchFailure::Refused);
        assert_eq!(refusal(429, None), SearchFailure::RateLimited);
        assert_eq!(refusal(500, None), SearchFailure::Unreadable);
        assert_eq!(
            refusal(302, None),
            SearchFailure::Unreadable,
            "a redirect is never followed and never a page"
        );
    }
}
