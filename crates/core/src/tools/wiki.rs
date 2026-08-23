//! The wiki lookup: one bounded GET against the project wiki's raw pages
//! (decided 2026-08-23; decision 0038 held the tool while the project SITE
//! had no wiki — the manifest repository's wiki on the mirror forge is a
//! different backend, and it is real).
//!
//! The lookup takes a page name — the title with spaces as dashes,
//! parentheses literal — and reads the page's raw text from a configured
//! base address defaulting to the real raw host. The body is decoded
//! lossily as UTF-8 and bounded by [`RESULT_LIMIT`] with a truncation
//! marker: a truncated wiki page is a degraded answer, not a changed
//! meaning, unlike rules. No page list lives in code or configuration —
//! the model learns the names from the wiki itself, starting at the entry
//! page or the sidebar.
//!
//! The raw host publishes no rate-limit contract, so the tool keeps a
//! per-process response cache: keyed by the full request address, a named
//! TTL matching the host's own cache header (five minutes), pages and
//! missing-page answers cached alike — negative caching bounds a model
//! guessing page names — and a named entry cap cleared whole when hit, the
//! established memory-cap shape. Transport failures are never cached: a
//! passing condition must not silence five minutes of retries.

use std::collections::HashMap;
use std::time::Duration;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::message::Authority;
use crate::tools::lookup::{ORGANIZATION, TextAnswer, bounded_get_text, lookup_client};

/// The registered name the model calls the tool by.
pub const NAME: &str = "lookup_wiki";

/// The real raw host the base URL defaults to — the mirror forge's raw
/// service, which serves wiki pages as plain text, unauthenticated, with
/// no redirect.
pub const DEFAULT_BASE_URL: &str = "https://raw.githubusercontent.com";

/// The repository whose wiki the lookup reads — the project's manifest
/// repository on the mirror organization.
pub const WIKI_REPOSITORY: &str = "android_manifest";

/// The wiki's entry page — where the model starts when it does not know a
/// page's name.
pub const ENTRY_PAGE: &str = "Home";

/// The wiki's sidebar page — the other ordinary fetch that lists pages.
pub const SIDEBAR_PAGE: &str = "_Sidebar";

/// The default request timeout. A construction parameter so tests
/// construct short bounds instead of waiting production ones.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The authority this tool requires — the bar the admission wrapper's
/// provenance gate compares each call's reading against (decision 0043).
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// How many characters of a page the model-facing result carries before
/// the truncation marker.
pub const RESULT_LIMIT: usize = 10_000;

/// What marks a cut page: appended where [`RESULT_LIMIT`] cut the text, so
/// the model knows it read a bounded head, not the whole page.
pub const TRUNCATION_MARKER: &str = "\n… [the page continues past this result's bound]";

/// How long one cached answer serves — the raw host's own cache header
/// says five minutes, and the tool's memory mirrors it.
pub const CACHE_TTL: Duration = Duration::from_mins(5);

/// How many answers the cache holds. At the cap it is cleared whole — the
/// established memory-cap shape: losing the cache costs one refetch per
/// page, while an unbounded map would grow with every name a model ever
/// guessed.
pub const CACHE_CAP: usize = 64;

/// A page name safe to place as one URL path segment: the title with
/// spaces as dashes and parentheses literal. Letters, digits, dash,
/// underscore, dot and parentheses; no dot-only segment, non-empty. Its
/// own predicate on purpose — the repository predicate rejects the
/// parentheses real page titles carry.
#[must_use]
pub fn valid_page_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '(' | ')'))
}

/// One cached answer: what the model-facing side of a fetch came to — the
/// bounded page text, or the missing-page error.
type CachedAnswer = Result<String, String>;

/// The per-process response cache, keyed by the full request address: live
/// entries serve for [`CACHE_TTL`], expired ones are swept on every read,
/// and the whole map is cleared when an insert meets [`CACHE_CAP`]. Its
/// own struct so the TTL and the cap are pinned under paused time without
/// a wire — a paused runtime and real sockets do not mix.
struct AnswerCache {
    entries: Mutex<HashMap<String, (Instant, CachedAnswer)>>,
}

impl AnswerCache {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// The live cached answer for one address, expired entries swept.
    async fn cached(&self, url: &str) -> Option<CachedAnswer> {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();
        entries.retain(|_, (at, _)| now.duration_since(*at) < CACHE_TTL);
        entries.get(url).map(|(_, answer)| answer.clone())
    }

    /// Record one answer, clearing the whole map first when the cap is
    /// hit.
    async fn remember(&self, url: String, answer: CachedAnswer) {
        let mut entries = self.entries.lock().await;
        if entries.len() >= CACHE_CAP {
            tracing::debug!("the wiki cache reached its cap and was cleared");
            entries.clear();
        }
        entries.insert(url, (Instant::now(), answer));
    }
}

/// The wiki lookup tool.
pub struct WikiLookup {
    client: reqwest::Client,
    base_url: String,
    /// The per-process response cache, keyed by the full request address.
    cache: AnswerCache,
}

impl WikiLookup {
    /// Construct against a base URL — the real raw host by default, a
    /// loopback server in tests — with the given request timeout. The
    /// client follows no redirects, per the one-bounded-GET contract.
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
            cache: AnswerCache::new(),
        }
    }

    /// One page's model-facing answer: the cache first, then one bounded
    /// GET. `Err` is the tool error the model reads.
    async fn look_up(&self, page: &str) -> Result<String, String> {
        let url = format!(
            "{}/wiki/{ORGANIZATION}/{WIKI_REPOSITORY}/{page}.md",
            self.base_url
        );
        if let Some(cached) = self.cache.cached(&url).await {
            return cached;
        }
        let answer = match bounded_get_text(&self.client, &url, "the wiki").await {
            Ok(TextAnswer::Body(body)) => Ok(bounded_result(&body)),
            Ok(TextAnswer::Missing) => Err(missing_page(page)),
            // A wire failure is returned uncached: a passing condition
            // must not answer for the next five minutes.
            Err(error) => return Err(error),
        };
        self.cache.remember(url, answer.clone()).await;
        answer
    }
}

/// The page text within [`RESULT_LIMIT`] characters, the truncation marker
/// naming the cut.
fn bounded_result(body: &str) -> String {
    if body.chars().count() <= RESULT_LIMIT {
        return body.to_owned();
    }
    let mut bounded: String = body.chars().take(RESULT_LIMIT).collect();
    bounded.push_str(TRUNCATION_MARKER);
    bounded
}

/// The missing-page error, naming the page-name shape so the model can
/// correct a guess.
fn missing_page(page: &str) -> String {
    format!(
        "the wiki has no page named `{page}` — a page is named by its title with spaces \
         as dashes, parentheses literal; fetch the {ENTRY_PAGE} page or the \
         {SIDEBAR_PAGE} page to see the page names"
    )
}

impl ToolHandler<CoreEvent> for WikiLookup {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: format!(
                "Look up one page of the halogenOS wiki and read its raw text. A page is \
                 named by its title with spaces as dashes and parentheses kept literal, \
                 for example Frequently-Asked-Questions-(FAQ). When you do not know a \
                 page's name, fetch the {ENTRY_PAGE} page or the {SIDEBAR_PAGE} page \
                 first — they list the wiki's pages."
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "page": {
                        "type": "string",
                        "description": "The page name: the title with spaces as dashes, \
                                        parentheses literal."
                    }
                },
                "required": ["page"]
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
                    return ToolOutcome::Error("the input is not a JSON object with `page`".into());
                }
            };
            let Some(page) = arguments["page"].as_str() else {
                return ToolOutcome::Error("the input names no `page`".into());
            };
            if !valid_page_name(page) {
                return ToolOutcome::Error(
                    "the page must be a plain page name: the title with spaces as dashes, \
                     parentheses literal"
                        .into(),
                );
            }
            match self.look_up(page).await {
                Ok(text) => ToolOutcome::Done(text),
                Err(error) => ToolOutcome::Error(error),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── The page-name predicate (AC3) ───────────────────────────────────

    #[test]
    fn page_names_allow_dashes_and_parentheses() {
        assert!(valid_page_name("Home"));
        assert!(valid_page_name("_Sidebar"));
        assert!(valid_page_name("Building-halogenOS"));
        assert!(valid_page_name("Frequently-Asked-Questions-(FAQ)"));
        assert!(valid_page_name("device_oneplus.sdm845"));
    }

    #[test]
    fn page_names_reject_separators_and_empties() {
        assert!(!valid_page_name(""));
        assert!(!valid_page_name("a/b"));
        assert!(!valid_page_name("a\\b"));
        assert!(!valid_page_name("."));
        assert!(!valid_page_name(".."));
        assert!(!valid_page_name("a b"));
        assert!(!valid_page_name("a?b=c"));
        assert!(!valid_page_name("a#anchor"));
        assert!(!valid_page_name("a%2fb"));
    }

    // ─── The result bound ────────────────────────────────────────────────

    #[test]
    fn the_result_bound_truncates_with_the_marker() {
        let short = "a short page";
        assert_eq!(
            bounded_result(short),
            short,
            "a page within the bound is whole"
        );
        let exactly = "x".repeat(RESULT_LIMIT);
        assert_eq!(
            bounded_result(&exactly),
            exactly,
            "the bound itself is whole"
        );
        let over = "x".repeat(RESULT_LIMIT + 5);
        let bounded = bounded_result(&over);
        assert!(
            bounded.ends_with(TRUNCATION_MARKER),
            "the marker names the cut"
        );
        assert_eq!(
            bounded.chars().count(),
            RESULT_LIMIT + TRUNCATION_MARKER.chars().count(),
            "the bound counts characters, the marker rides after it"
        );
    }

    // ─── The cache's clock, under paused time (AC2) ──────────────────────
    //
    // The TTL and the cap are pinned here, on the cache struct itself,
    // because a paused runtime and real sockets do not mix: the paused
    // clock auto-advances past every timeout while a task awaits the
    // wire. The one-request-per-live-entry behavior over a real wire is
    // pinned in the integration suite, where time runs.

    #[tokio::test(start_paused = true)]
    async fn a_cached_answer_serves_inside_the_ttl_and_expires_past_it() {
        let cache = AnswerCache::new();
        cache.remember("u1".into(), Ok("the page".into())).await;
        assert_eq!(
            cache.cached("u1").await,
            Some(Ok("the page".into())),
            "a live entry serves"
        );
        tokio::time::advance(
            CACHE_TTL
                .checked_sub(Duration::from_secs(1))
                .expect("the TTL is longer than a second"),
        )
        .await;
        assert!(
            cache.cached("u1").await.is_some(),
            "still inside the TTL, still served"
        );
        tokio::time::advance(Duration::from_secs(2)).await;
        assert_eq!(
            cache.cached("u1").await,
            None,
            "past the TTL, the entry expired and was swept"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_missing_page_answer_is_cached_like_a_body() {
        let cache = AnswerCache::new();
        cache
            .remember("u404".into(), Err(missing_page("Guessed")))
            .await;
        assert_eq!(
            cache.cached("u404").await,
            Some(Err(missing_page("Guessed"))),
            "negative caching bounds a model guessing page names"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_cache_clears_whole_at_its_cap() {
        let cache = AnswerCache::new();
        for n in 0..CACHE_CAP {
            cache.remember(format!("u{n}"), Ok("a page".into())).await;
        }
        assert!(
            cache.cached("u0").await.is_some(),
            "the cap itself still holds every entry"
        );
        cache.remember("one-more".into(), Ok("a page".into())).await;
        assert_eq!(
            cache.cached("u0").await,
            None,
            "the insert at the cap cleared the map whole"
        );
        assert!(
            cache.cached("one-more").await.is_some(),
            "the clearing insert itself stands"
        );
    }

    #[test]
    fn a_missing_page_error_names_the_page_name_shape() {
        let error = missing_page("Guessed-Page");
        assert_eq!(
            error,
            "the wiki has no page named `Guessed-Page` — a page is named by its title \
             with spaces as dashes, parentheses literal; fetch the Home page or the \
             _Sidebar page to see the page names"
        );
    }
}
