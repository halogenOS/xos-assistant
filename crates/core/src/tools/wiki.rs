//! The wiki lookup: bounded GETs against the project wiki (decided
//! 2026-08-23; decision 0038 held the tool while the project SITE had no
//! wiki — the manifest repository's wiki on the mirror forge is a
//! different backend, and it is real).
//!
//! The page fetch takes a page name — the title with spaces as dashes,
//! parentheses literal — and reads the page's raw text from a configured
//! base address defaulting to the real raw host. The body is decoded
//! lossily as UTF-8 and bounded by [`RESULT_LIMIT`] with a truncation
//! marker: a truncated wiki page is a degraded answer, not a changed
//! meaning, unlike rules.
//!
//! The page enumeration (2026-08-24) is the discovery path: called with no
//! page name, the tool reads the wiki's rendered index from a second
//! configured base address defaulting to the real forge host — the raw
//! host serves pages but no index — extracts the content page names by the
//! forge's stable page-link shape (`…/wiki/<PageName>`), drops the
//! service's reserved pages (underscore-prefixed names and the
//! history/edit variants), de-duplicates and sorts. No page list lives in
//! code or configuration — the names come from the wiki itself, from the
//! index that actually lists them.
//!
//! Neither host publishes a rate-limit contract, so the tool keeps a
//! per-process response cache — the lookup layer's shared cache shape — keyed by
//! the full request address, the index answer under its own address beside
//! the pages, with a named TTL matching the raw host's own cache header
//! (five minutes), pages and missing-page answers cached alike — negative
//! caching bounds a model guessing page names — and a named entry cap
//! cleared whole when hit. Transport failures are never cached: a passing
//! condition must not silence five minutes of retries.

use std::collections::BTreeSet;
use std::time::Duration;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{Admission, Block, CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::{Value, json};

use crate::message::Authority;
use crate::tools::lookup::{
    MemoryCache, ORGANIZATION, TextAnswer, bounded_get_text, lookup_client,
};

/// The registered name the model calls the tool by.
pub const NAME: &str = "lookup_wiki";

/// The real raw host the base URL defaults to — the mirror forge's raw
/// service, which serves wiki pages as plain text, unauthenticated, with
/// no redirect.
pub const DEFAULT_BASE_URL: &str = "https://raw.githubusercontent.com";

/// The real forge host the index base URL defaults to — where the wiki's
/// rendered index lists every content page in one unauthenticated GET.
/// A second base on purpose: the raw host serves page content but no
/// index, and deriving one host from the other by string surgery was
/// rejected with the unit.
pub const DEFAULT_INDEX_BASE_URL: &str = "https://github.com";

/// The repository whose wiki the lookup reads — the project's manifest
/// repository on the mirror organization.
pub const WIKI_REPOSITORY: &str = "android_manifest";

/// The default request timeout. A construction parameter so tests
/// construct short bounds instead of waiting production ones.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The authority this tool requires — the bar the admission hook's
/// provenance reading is compared against at every call (decision 0043).
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

/// One character a page name may carry — also what bounds a name cut out
/// of the rendered index, so the enumeration and the predicate cannot
/// disagree about the page-name shape.
fn page_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '(' | ')')
}

/// A page name safe to place as one URL path segment: the title with
/// spaces as dashes and parentheses literal. Letters, digits, dash,
/// underscore, dot and parentheses; no dot-only segment, non-empty. Its
/// own predicate on purpose — the repository predicate rejects the
/// parentheses real page titles carry.
#[must_use]
pub fn valid_page_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && name.chars().all(page_name_char)
}

/// One cached answer: what the model-facing side of a fetch came to — the
/// bounded page text, or the missing-page error.
type CachedAnswer = Result<String, String>;

/// The wiki lookup tool.
pub struct WikiLookup {
    client: reqwest::Client,
    /// The raw host's base address — where page content is read from.
    base_url: String,
    /// The forge host's base address — where the rendered index the page
    /// enumeration reads is served.
    index_base_url: String,
    /// The per-process response cache, keyed by the full request address,
    /// over [`CACHE_TTL`] and [`CACHE_CAP`].
    cache: MemoryCache<CachedAnswer>,
}

impl WikiLookup {
    /// Construct against the two base URLs — the real raw and forge hosts
    /// by default, loopback servers in tests — with the given request
    /// timeout. The client follows no redirects, per the one-bounded-GET
    /// contract.
    ///
    /// # Panics
    ///
    /// If the HTTP client cannot be built — a broken TLS stack at
    /// construction, not a runtime condition.
    #[must_use]
    pub fn new(
        base_url: impl Into<String>,
        index_base_url: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            client: lookup_client(timeout),
            base_url: base_url.into(),
            index_base_url: index_base_url.into(),
            cache: MemoryCache::new(CACHE_TTL, CACHE_CAP),
        }
    }

    /// The one cached bounded-GET path both reads take: the cache first,
    /// then one bounded GET of `url` named `who`, its success body shaped
    /// by `transform` and its 404 worded by `missing`. The shaped answer,
    /// body or missing alike, is remembered; a wire failure is returned
    /// uncached — a passing condition must not answer for the next five
    /// minutes. `Err` is the tool error the model reads.
    async fn cached_text_get(
        &self,
        url: String,
        who: &str,
        transform: impl FnOnce(&str) -> Result<String, String>,
        missing: impl FnOnce() -> String,
    ) -> Result<String, String> {
        if let Some(cached) = self.cache.cached(&url).await {
            return cached;
        }
        let answer = match bounded_get_text(&self.client, &url, who).await {
            Ok(TextAnswer::Body(body)) => transform(&body),
            Ok(TextAnswer::Missing) => Err(missing()),
            Err(error) => return Err(error),
        };
        self.cache.remember(url, answer.clone()).await;
        answer
    }

    /// One page's model-facing answer: the bounded page text, or the
    /// missing-page error.
    async fn look_up(&self, page: &str) -> Result<String, String> {
        let url = format!(
            "{}/wiki/{ORGANIZATION}/{WIKI_REPOSITORY}/{page}.md",
            self.base_url
        );
        self.cached_text_get(
            url,
            "the wiki",
            |body| Ok(bounded_result(body)),
            || missing_page(page),
        )
        .await
    }

    /// The page list's model-facing answer: the content page names read out
    /// of the rendered index, or the missing-index error.
    async fn list_pages(&self) -> Result<String, String> {
        let url = format!(
            "{}/{ORGANIZATION}/{WIKI_REPOSITORY}/wiki",
            self.index_base_url
        );
        self.cached_text_get(
            url,
            "the wiki index",
            |body| {
                let names = page_names(body);
                if names.is_empty() {
                    // A 200 index with no page links is a markup the scan no
                    // longer recognizes, not an empty wiki. Answering an empty
                    // list would read as "the wiki has no pages" and, under the
                    // grounded-answer discipline, silence every question the
                    // wiki could answer — so this is a loud tool error, not a
                    // successful empty answer.
                    return Err(UNREADABLE_INDEX.to_owned());
                }
                Ok(bounded_result(&names.join("\n")))
            },
            || MISSING_INDEX.to_owned(),
        )
        .await
    }
}

/// The tool error a not-found index answers — clean and cacheable, unlike
/// a wire failure.
const MISSING_INDEX: &str =
    "the wiki index answered: not found — the page list cannot be read right now";

/// The tool error a present-but-unparseable index answers: the request
/// succeeded but no page link was found in the markup, so the page list
/// could not be read. A loud error rather than a silent empty list.
const UNREADABLE_INDEX: &str = "the wiki index was read but no page links were found in it — the page list \
     cannot be read right now";

/// The content page names in one rendered index: every occurrence of the
/// forge's stable page-link shape `/{organization}/{repository}/wiki/<PageName>`
/// yields one name, the service's reserved links dropped — an
/// underscore-prefixed name, and a continued path (the `/_history` and
/// `/_edit` variants) — then de-duplicated and sorted. This is a deliberate
/// tolerance of the forge's markup, not an HTML parse: the byte stream is
/// scanned for the stable path shape wherever it appears, and the page is
/// never interpreted. A name whose title carries a character outside the
/// page-name shape (a slash-titled page) is not represented here — the fetch
/// capability cannot address one either, so the two agree.
fn page_names(index_html: &str) -> Vec<String> {
    let link_prefix = format!("/{ORGANIZATION}/{WIKI_REPOSITORY}/wiki/");
    let mut names = BTreeSet::new();
    let mut rest = index_html;
    while let Some(found) = rest.find(&link_prefix) {
        let after = &rest[found + link_prefix.len()..];
        let end = after.find(|c| !page_name_char(c)).unwrap_or(after.len());
        let (name, tail) = after.split_at(end);
        // A continued path is a service variant of the page, not a page
        // link; an underscore-prefixed name is a reserved page.
        if !tail.starts_with('/') && !name.starts_with('_') && valid_page_name(name) {
            names.insert(name.to_owned());
        }
        rest = tail;
    }
    names.into_iter().collect()
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

/// The missing-page error, naming the page-name shape and the enumeration
/// so the model can correct a guess by reading the real list.
fn missing_page(page: &str) -> String {
    format!(
        "the wiki has no page named `{page}` — a page is named by its title with spaces \
         as dashes, parentheses literal; call this tool with no page to list the wiki's \
         page names"
    )
}

impl ToolHandler<CoreEvent> for WikiLookup {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            description: "Look up one page of the halogenOS wiki and read its raw text. A \
                          page is named by its title with spaces as dashes and parentheses \
                          kept literal, for example Frequently-Asked-Questions-(FAQ). \
                          Called with no page, the tool lists every page name instead: \
                          when you do not know a page's name, list the pages first, then \
                          fetch the one you need."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "page": {
                        "type": "string",
                        "description": "The page name: the title with spaces as dashes, \
                                        parentheses literal. Omit it to list the wiki's \
                                        pages."
                    }
                },
                "required": []
            }),
        }
    }

    /// The authority a call of this tool requires (decision 0043), answered
    /// through the framework's admission hook over the ledger snapshot the
    /// runner's admission pass already loaded.
    fn admit<'a>(
        &'a self,
        ctx: &'a ToolContext<'a, CoreEvent>,
        ledger: &'a [Block],
    ) -> BoxFuture<'a, Admission> {
        crate::tools::admission::at_required_authority(NAME, REQUIRED_AUTHORITY, ctx, ledger)
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
                        "the input is not a JSON object; pass an empty object to list \
                         the wiki's pages, or one with `page` to fetch a page"
                            .into(),
                    );
                }
            };
            let Some(arguments) = arguments.as_object() else {
                return ToolOutcome::Error(
                    "the input is not a JSON object; pass an empty object to list \
                     the wiki's pages, or one with `page` to fetch a page"
                        .into(),
                );
            };
            // No page named is the enumeration — the discovery path.
            let page = match arguments.get("page") {
                None | Some(Value::Null) => None,
                Some(Value::String(page)) => Some(page.as_str()),
                Some(_) => {
                    return ToolOutcome::Error(
                        "the `page` must be a string: the title with spaces as dashes, \
                         parentheses literal"
                            .into(),
                    );
                }
            };
            let answer = match page {
                Some(page) if !valid_page_name(page) => {
                    return ToolOutcome::Error(
                        "the page must be a plain page name: the title with spaces as \
                         dashes, parentheses literal"
                            .into(),
                    );
                }
                Some(page) => self.look_up(page).await,
                None => self.list_pages().await,
            };
            match answer {
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
    // The TTL and the cap are pinned here, on the shared cache under this
    // tool's own constants, because a paused runtime and real sockets do
    // not mix: the paused clock auto-advances past every timeout while a
    // task awaits the wire. The one-request-per-live-entry behavior over a
    // real wire is pinned in the integration suite, where time runs.

    #[tokio::test(start_paused = true)]
    async fn a_cached_answer_serves_inside_the_ttl_and_expires_past_it() {
        let cache: MemoryCache<CachedAnswer> = MemoryCache::new(CACHE_TTL, CACHE_CAP);
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
        let cache: MemoryCache<CachedAnswer> = MemoryCache::new(CACHE_TTL, CACHE_CAP);
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
        let cache: MemoryCache<CachedAnswer> = MemoryCache::new(CACHE_TTL, CACHE_CAP);
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

    // ─── Discovery guidance (AC4) ────────────────────────────────────────

    #[test]
    fn a_missing_page_error_sends_the_model_to_the_enumeration() {
        let error = missing_page("Guessed-Page");
        assert_eq!(
            error,
            "the wiki has no page named `Guessed-Page` — a page is named by its title \
             with spaces as dashes, parentheses literal; call this tool with no page to \
             list the wiki's page names"
        );
    }

    #[test]
    fn the_description_sends_the_model_to_the_enumeration() {
        let tool = WikiLookup::new("http://127.0.0.1:1", "http://127.0.0.1:1", DEFAULT_TIMEOUT);
        let definition = tool.definition();
        assert_eq!(
            definition.description,
            "Look up one page of the halogenOS wiki and read its raw text. A page is \
             named by its title with spaces as dashes and parentheses kept literal, for \
             example Frequently-Asked-Questions-(FAQ). Called with no page, the tool \
             lists every page name instead: when you do not know a page's name, list \
             the pages first, then fetch the one you need."
        );
        assert_eq!(
            definition.parameters["required"],
            serde_json::json!([]),
            "the page is optional — omitting it is how the enumeration is called"
        );
        for copy in [definition.description, missing_page("Guessed-Page")] {
            assert!(
                !copy.contains("Home") && !copy.contains("Sidebar"),
                "the shipped copy no longer names the entry page or the sidebar as \
                 the discovery path: {copy}"
            );
        }
    }

    // ─── The index extraction (AC2, AC3) ─────────────────────────────────

    /// The captured real index: the wiki's own landing HTML, every content
    /// page linked, the hand-written sidebar's navigation beside it.
    const CAPTURED_INDEX: &str = include_str!("../../tests/fixtures/wiki-index.html");

    /// Every content page the captured index links, in the tool's
    /// page-name shape, byte-sorted — the unlisted feature page included.
    const CAPTURED_PAGES: [&str; 15] = [
        "AIDL-HALs",
        "Button-Backlight-Control",
        "Code-of-Conduct",
        "Configurable-Dark-Mode-Tones",
        "Contact-and-maintainership",
        "Encryption-auto-detection",
        "Fixing-errors",
        "Fixing-runtime-errors",
        "Font-System",
        "Home",
        "Integrating-Sandboxed-Google-Play-(16.2)",
        "Porting-from-other-ROMs",
        "Porting-from-other-ROMs-(Legacy)",
        "Project-Standards",
        "System-AIDL-Services",
    ];

    #[test]
    fn the_extraction_reads_every_content_page_out_of_the_captured_index() {
        let names = page_names(CAPTURED_INDEX);
        assert_eq!(
            names, CAPTURED_PAGES,
            "the captured real index yields exactly the content pages, sorted"
        );
        for name in &names {
            assert!(
                valid_page_name(name),
                "a listed name is exactly what the fetch takes: {name}"
            );
        }
    }

    #[test]
    fn the_extraction_drops_reserved_links_and_duplicates() {
        let markup = r#"<a href="/halogenOS/android_manifest/wiki/Alpha">a</a>
            <a href="/halogenOS/android_manifest/wiki/_Sidebar">reserved</a>
            <a href="/halogenOS/android_manifest/wiki/Alpha/_history">history</a>
            <a href="/halogenOS/android_manifest/wiki/Beta/_edit">edit variant only</a>
            <a href="/halogenOS/android_manifest/wiki">the bare index</a>
            <a href="/halogenOS/android_manifest/wiki/Alpha">a again</a>"#;
        assert_eq!(
            page_names(markup),
            vec!["Alpha".to_owned()],
            "reserved pages, service variants and duplicates never reach the list"
        );
    }

    #[test]
    fn an_over_bound_page_list_is_truncated_with_the_marker() {
        use std::fmt::Write as _;
        let mut markup = String::new();
        for n in 0..2_000 {
            let _ = writeln!(
                markup,
                "<a href=\"/halogenOS/android_manifest/wiki/Page-{n:04}\">x</a>"
            );
        }
        let list = page_names(&markup).join("\n");
        assert!(
            list.chars().count() > RESULT_LIMIT,
            "the probe list overflows the bound"
        );
        let bounded = bounded_result(&list);
        assert!(
            bounded.ends_with(TRUNCATION_MARKER),
            "the marker names the cut"
        );
        assert_eq!(
            bounded.chars().count(),
            RESULT_LIMIT + TRUNCATION_MARKER.chars().count(),
            "the list rides under the same bound as a page"
        );
    }
}
