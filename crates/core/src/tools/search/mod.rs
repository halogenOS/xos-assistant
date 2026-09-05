//! The web search: one tool that answers questions about the world with a
//! page of ranked results (unit 27, 2026-08-25).
//!
//! It searches and nothing else. Titles, links, snippets and a host-derived
//! source hint are what a result carries; the tool opens no page, follows
//! no link and fetches no document, and that boundary is what keeps it
//! small enough to be safe.
//!
//! Four disciplines sit on the way to the vendor, in this order:
//!
//! 1. **The bounds.** A query over [`QUERY_LIMIT`] characters is refused
//!    whole with the limit named — never truncated, because a cut query is
//!    a different question — and a page outside [`FIRST_PAGE`]…[`LAST_PAGE`]
//!    is refused with the range named.
//! 2. **The person guard** (the `guard` module): a query carrying a deliberate
//!    handle-form token is refused whole and nothing is sent. The refusal
//!    teaches the rule and never echoes the token — a tool result is a
//!    framework record erasure cannot reach, and a guard that writes the
//!    identifier it refused into permanent storage protects nothing.
//! 3. **The same-query cache**: the query as written, case-folded and
//!    whitespace-collapsed, plus the page, live for [`CACHE_TTL`]. A cache
//!    hit costs no vendor spend and is therefore served even on a spent
//!    budget — the budget's stated basis is metered spend and nothing
//!    else, and cache freshness is its own question. The key is
//!    deliberately NOT the guard's normalisation, which exists to find one
//!    token and would merge queries a member wrote differently.
//! 4. **The per-person budget**: [`SEARCH_BUDGET_CAP`] searches per person
//!    per [`SEARCH_BUDGET_WINDOW`], in the shape of the reply bound the
//!    command family already uses. The person is the principal over the
//!    turn's debt-origin set, resolved exactly as the rights tool resolves
//!    it; a turn holding zero or several distinct principals declines the
//!    spend, because the per-person guarantee must never fold several
//!    people into one bucket. A failed request hands its grant straight
//!    back: the budget bounds what is billed, and a refused key bills
//!    nothing.
//!
//! Every refusal and every failure is a taught result the model reads and
//! the chat never sees (decision 0044), each distinguishable from the
//! others and from an honest empty page — and none of them carries a bare
//! status number, which is why the shared lookup layer's own status wording
//! is never used here.
//!
//! The envelope promises only what the vendor can keep: the query as sent,
//! the page number, the count returned, and the rows themselves. There is
//! no total and no more-pages flag, because the vendor answers neither and
//! a stubbed number would pass the pins while lying in production. An empty
//! first page and an exhausted later page read differently, and a later
//! page identical to the previous one reads as exhausted too — a vendor
//! past its real limit may repeat the last page instead of sending an empty
//! one.

pub(crate) mod guard;
pub(crate) mod vendor;

use std::fmt::Write as _;
use std::future::Future;
use std::time::Duration;

use agent_ledger::providers::{BoxFuture, ToolDefinition};
use agent_ledger::{CoreEvent, ToolContext, ToolHandler, ToolOutcome};
use serde_json::{Value, json};

use crate::message::Authority;
use crate::tools::lookup::{MemoryCache, ORGANIZATION, truncated};
use crate::tools::provenance::sole_principal;
use crate::window::{Change, ReplyWindow, SEARCH_BUDGET_CAP, SEARCH_BUDGET_WINDOW};

/// The registered name the model calls the tool by.
pub const NAME: &str = "search_web";

/// The real host the base address defaults to — the search vendor's own
/// endpoint.
pub const DEFAULT_BASE_URL: &str = "https://google.serper.dev";

/// The default request timeout — the lookup layer's own default bound.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The authority this tool requires: member, because the tool exists to
/// answer members' questions.
pub const REQUIRED_AUTHORITY: Authority = Authority::Member;

/// The longest query the tool sends, in characters. A longer one is refused
/// whole with this number named — never truncated.
pub const QUERY_LIMIT: usize = 400;

/// The first page the tool will ask for.
pub const FIRST_PAGE: u32 = 1;

/// The last page the tool will ask for. Past this the model is fishing, and
/// the budget is better spent on a reworded query.
pub const LAST_PAGE: u32 = 5;

/// How many characters of one result title the envelope carries.
pub const TITLE_LIMIT: usize = 200;

/// How many characters of one result snippet the envelope carries.
pub const SNIPPET_LIMIT: usize = 400;

/// How long one cached page serves: ten minutes, the length a repeated
/// question inside one conversation falls within. Its own number, because
/// cache freshness is a question about how fast the web changes and the
/// budget window is a question about metered spend — one answering the
/// other would tie a freshness change to a spending change.
pub const CACHE_TTL: Duration = Duration::from_mins(10);

/// How many pages the cache holds. At the cap it is cleared whole, the
/// established memory-cap shape: losing the cache costs one billed request
/// per query, while an unbounded map would grow with every query a model
/// ever wrote.
pub const CACHE_CAP: usize = 64;

// ─── The source hint (the table, inlined) ────────────────────────────────

/// A wikipedia host.
const ENCYCLOPEDIA: &str = "encyclopedia";
/// A government or education host.
const OFFICIAL: &str = "official";
/// A known blog host.
const BLOG: &str = "blog";
/// Any other host.
const WEBSITE: &str = "website";
/// A row whose link carries no host at all.
const UNKNOWN: &str = "unknown";

/// The blog hosts the table knows by name. Generic by instruction: this is
/// a shape hint for the model, never a judgment of authority, and a host
/// missing from it simply reads as a website.
const BLOG_HOSTS: [&str; 8] = [
    "blogspot.com",
    "dev.to",
    "ghost.io",
    "hashnode.dev",
    "medium.com",
    "substack.com",
    "tumblr.com",
    "wordpress.com",
];

// ─── The fixed results ───────────────────────────────────────────────────

/// The person guard's refusal: the rule, the fix, and no echo of what was
/// matched — the matched token is exactly what must not be written into a
/// record erasure cannot reach.
pub const PERSON_REFERENCE_RESULT: &str = "No web search was made. A search query must never carry a person \
     reference — an at sign followed by a name. Search for the subject \
     itself, with that token removed. Do not retry the same query with the \
     token spelled differently.";

/// The decline when the turn resolves to no single person: the budget is
/// per person, and folding several people into one bucket would break that
/// guarantee.
pub const NO_SINGLE_PERSON_RESULT: &str = "No web search was made: this turn does not resolve to one person, and \
     the search budget is per person. Answer from the project lookups or \
     from what the conversation already shows.";

/// The decline when a read the tool depends on did not stand: nothing was
/// searched and nothing was recorded.
pub const TRANSIENT_RESULT: &str = "No web search was made: the conversation could not be read just now. \
     Nothing was searched. Answer from what you already have.";

/// The refused-key failure: a deployment problem the model cannot fix by
/// rewording.
pub const REFUSED_KEY_RESULT: &str = "No web search was made: the search service refused this deployment's \
     key. Nothing you can change in the query fixes it — answer without the \
     web search and do not call this tool again in this turn.";

/// The other-refusal failure at the same status: the service refused the
/// request for a reason of its own.
pub const REFUSED_REQUEST_RESULT: &str = "No web search was made: the search service refused the request. Answer \
     without the web search and do not call this tool again in this turn.";

/// The rate-limit failure.
pub const RATE_LIMITED_RESULT: &str = "No web search was made: the search service is rate-limiting this \
     deployment right now. Answer without the web search.";

/// The unreachable-host failure: nothing was answered at all.
pub const UNREACHABLE_RESULT: &str = "No web search was made: the search service could not be reached. \
     Answer without the web search.";

/// The unusable-answer failure: the service was reached and answered, and
/// the answer did not arrive in a state this tool can use — cut off partway,
/// or longer than the size bound every lookup reads under. Told apart from
/// the unreachable host on purpose: the request was made, so a retry costs
/// another one.
pub const UNUSABLE_ANSWER_RESULT: &str = "No web search was made: the search service answered, but the answer was \
     unusable — it ended partway or ran past the size this tool reads. \
     Answer without the web search.";

/// The timeout failure.
pub const TIMEOUT_RESULT: &str = "No web search was made: the search service did not answer within the \
     time bound. Answer without the web search.";

/// The unusable-answer failure: the service answered something this tool
/// has no reading for.
pub const UNREADABLE_RESULT: &str = "No web search was made: the search service answered something this \
     tool cannot read. Answer without the web search.";

/// The malformed-call teaching: the closed input shape, stated once.
pub const INVALID_INPUT_RESULT: &str = "The web search takes a JSON object with a `query` string, and \
     optionally a `page` number. Nothing was searched.";

/// The over-long query's refusal, naming the limit.
#[must_use]
pub fn over_long_result() -> String {
    format!(
        "No web search was made: a query is at most {QUERY_LIMIT} characters and this one is \
         longer. It was not shortened for you — a cut query asks a different question. Search \
         for the subject in fewer words."
    )
}

/// The out-of-range page's refusal, naming the range.
#[must_use]
pub fn page_out_of_range_result() -> String {
    format!(
        "No web search was made: pages run from {FIRST_PAGE} to {LAST_PAGE}. Reword the query \
         instead of asking for a later page."
    )
}

/// The spent-budget decline, naming the bound and when it reopens.
#[must_use]
pub fn budget_spent_result() -> String {
    format!(
        "No web search was made: this person has used all {SEARCH_BUDGET_CAP} web searches for \
         the moment. The bound is {SEARCH_BUDGET_CAP} searches per person per {minutes} minutes, \
         so a search can be made again once {minutes} minutes have passed since the first of \
         them. Answer from the project lookups or from what you already have.",
        minutes = SEARCH_BUDGET_WINDOW.as_secs() / 60,
    )
}

/// The empty first page: an honest nothing, not a failure.
#[must_use]
pub fn no_results_result(query: &str) -> String {
    format!(
        "The web search for `{query}` returned no results. Search different words, or answer \
         without it."
    )
}

/// The exhausted later page: the results ended at the page before it. Also
/// what a later page identical to the previous page reads as.
#[must_use]
pub fn exhausted_result(query: &str, page: u32) -> String {
    format!(
        "The web search for `{query}` has no page {page}: the results ended at page {previous}. \
         Search different words instead of asking for a later page.",
        previous = page.saturating_sub(1),
    )
}

/// One page of results, rendered as the stated lines: the query as sent,
/// the page, the count, and each row with its link, its source hint and its
/// snippet where the row carries one.
#[must_use]
pub fn page_result(query: &str, page: u32, rows: &[SearchRow]) -> String {
    let mut rendered = format!(
        "Web search results for: {query}\nPage: {page}\nResults: {count}",
        count = rows.len()
    );
    for (position, row) in rows.iter().enumerate() {
        let _ = write!(
            rendered,
            "\n\n{number}. {title}\nLink: {link}\nSource: {source}",
            number = position + 1,
            title = truncated(&row.title, TITLE_LIMIT),
            link = row.link,
            source = source_hint(&row.link),
        );
        if let Some(snippet) = &row.snippet {
            let _ = write!(
                rendered,
                "\nSnippet: {snippet}",
                snippet = truncated(snippet, SNIPPET_LIMIT)
            );
        }
    }
    rendered
}

// ─── The vendor seam ─────────────────────────────────────────────────────

/// What the deployment points the search at: the vendor's base address, the
/// resolved key and the locale. One carrier, so the assembly, the binary
/// and the tool all name the same four facts.
///
/// `Debug` is written by hand and redacts the key: [`crate::AssemblyConfig`]
/// derives `Debug`, so a derived one here would print the secret through
/// every debug rendering of the assembly's own configuration.
#[derive(Clone)]
pub struct SearchConfig {
    /// The vendor's base address; [`DEFAULT_BASE_URL`] in production, a
    /// loopback server under test.
    pub base_url: String,
    /// The vendor's API key, resolved from its indirection by the embedder.
    pub api_key: String,
    /// The country code sent as the vendor's `gl`, absent when the
    /// deployment configured none.
    pub country: Option<String>,
    /// The language code sent as the vendor's `hl`. Always present: the
    /// embedder names one for every deployment, its own default included,
    /// so there is no unconfigured-language case for this type to carry.
    pub language: String,
}

/// What a redacted key renders as wherever the configuration is debugged.
pub const REDACTED_KEY: &str = "<redacted>";

impl std::fmt::Debug for SearchConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &REDACTED_KEY)
            .field("country", &self.country)
            .field("language", &self.language)
            .finish()
    }
}

/// One ranked result, as the envelope needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRow {
    /// The result's title.
    pub title: String,
    /// The result's link, rendered whole: a truncated link is a broken one.
    pub link: String,
    /// The result's snippet, absent where the row carried none.
    pub snippet: Option<String>,
}

/// Why a search produced no page. Each variant is one taught result the
/// model can tell from every other, and from an honest empty page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchFailure {
    /// The service refused the key this deployment sent.
    RefusedKey,
    /// The service refused the request for a reason that is not the key.
    Refused,
    /// The service is rate-limiting this deployment.
    RateLimited,
    /// The service could not be reached.
    Unreachable,
    /// The service did not answer within the time bound.
    Timeout,
    /// The service answered and the answer was unusable: it ended partway,
    /// or ran past the size bound the lookup layer reads under.
    UnusableAnswer,
    /// The service answered something this tool has no reading for.
    Unreadable,
}

impl SearchFailure {
    /// The taught result this failure answers with.
    fn taught(self) -> &'static str {
        match self {
            Self::RefusedKey => REFUSED_KEY_RESULT,
            Self::Refused => REFUSED_REQUEST_RESULT,
            Self::RateLimited => RATE_LIMITED_RESULT,
            Self::Unreachable => UNREACHABLE_RESULT,
            Self::Timeout => TIMEOUT_RESULT,
            Self::UnusableAnswer => UNUSABLE_ANSWER_RESULT,
            Self::Unreadable => UNREADABLE_RESULT,
        }
    }
}

/// The vendor seam (decided 2026-08-25): the tool owns the envelope, the
/// guard, the budget and the teaching surface; the implementation owns the
/// endpoint, the key and the response shape, and the tool names none of
/// them.
pub(crate) trait SearchProvider: Send + Sync {
    /// One page of results for one query, or the failure it came to.
    fn search<'a>(
        &'a self,
        query: &'a str,
        page: u32,
    ) -> BoxFuture<'a, Result<Vec<SearchRow>, SearchFailure>>;
}

// ─── The tool ────────────────────────────────────────────────────────────

/// The web search tool.
pub struct WebSearch {
    provider: Box<dyn SearchProvider>,
    /// The per-person spend bound, the tool's own: nothing else draws on
    /// it, so it is constructed here instead of injected.
    budget: ReplyWindow,
    /// The same-query page cache, keyed by [`cache_key`].
    cache: MemoryCache<Vec<SearchRow>>,
}

impl WebSearch {
    /// Construct against the configured vendor with the given request
    /// timeout — the production shape.
    #[must_use]
    pub fn new(config: SearchConfig, timeout: Duration) -> Self {
        Self::with_provider(Box::new(vendor::VendorSearch::new(config, timeout)))
    }

    /// Construct over a given provider — how the crate's own tests exercise
    /// the envelope, the guard, the cache and the budget without a wire.
    pub(crate) fn with_provider(provider: Box<dyn SearchProvider>) -> Self {
        Self {
            provider,
            budget: ReplyWindow::new(SEARCH_BUDGET_WINDOW, SEARCH_BUDGET_CAP),
            cache: MemoryCache::new(CACHE_TTL, CACHE_CAP),
        }
    }

    /// The whole call: the bounds, the guard, the cache, the person, the
    /// budget and the request, in that order. `Err` carries the taught
    /// result the model reads.
    ///
    /// The person arrives as a LAZY future, and the order above is what
    /// that buys: a cache hit returns before the future is ever polled, so
    /// it reads no ledger, resolves nobody and spends nothing. It is also
    /// the seam the crate's own tests exercise the cache and the budget
    /// through, since a real turn's dispatch anchor — what the co-summoner
    /// walk reads — is not something any public write surface can forge;
    /// the resolution itself is pinned end to end in the integration suite.
    async fn answer(
        &self,
        input: &str,
        person: impl Future<Output = Result<i64, String>>,
    ) -> Result<String, String> {
        let ask = parse_ask(input)?;
        if ask.query.chars().count() > QUERY_LIMIT {
            return Err(over_long_result());
        }
        if !(FIRST_PAGE..=LAST_PAGE).contains(&ask.page) {
            return Err(page_out_of_range_result());
        }
        if guard::carries_person_reference(&ask.query) {
            return Err(PERSON_REFERENCE_RESULT.to_owned());
        }
        let key = cache_key(&ask.query, ask.page);
        if let Some(rows) = self.cache.cached(&key).await {
            // A cache hit spends nothing, so it neither needs a person nor
            // consults the budget: the bound exists for metered spend.
            return Ok(self.rendered(&ask, &rows).await);
        }
        let principal_id = person.await?;
        let search = async {
            self.provider
                .search(&ask.query, ask.page)
                .await
                .map(Change::Applied)
        };
        let rows = match self.budget.grant_with(principal_id, search).await {
            None => return Err(budget_spent_result()),
            // The grant was handed back before this: a failed request bills
            // nothing, so it must not cost the person a search.
            Some(Err(failure)) => return Err(failure.taught().to_owned()),
            // A search that answered spent the provider call, so this
            // producer builds only [`Change::Applied`] and the answer read
            // out here is that one.
            Some(Ok(change)) => change.answer(),
        };
        self.cache.remember(key, rows.clone()).await;
        Ok(self.rendered(&ask, &rows).await)
    }

    /// What one answered page renders as: the page itself, an honest empty
    /// first page, or an exhausted later page — the last of which is also
    /// what a page identical to the one before it reads as. Without the
    /// previous page in memory there is no evidence of repetition, and the
    /// page renders as the ordinary page it is.
    async fn rendered(&self, ask: &Ask, rows: &[SearchRow]) -> String {
        if rows.is_empty() {
            return if ask.page == FIRST_PAGE {
                no_results_result(&ask.query)
            } else {
                exhausted_result(&ask.query, ask.page)
            };
        }
        if ask.page > FIRST_PAGE {
            let previous = self
                .cache
                .cached(&cache_key(&ask.query, ask.page - 1))
                .await;
            if previous.is_some_and(|previous| previous == rows) {
                return exhausted_result(&ask.query, ask.page);
            }
        }
        page_result(&ask.query, ask.page, rows)
    }
}

/// One call's ask: the query as written and the page it wants.
struct Ask {
    query: String,
    page: u32,
}

/// The ask one call's input names, or the malformed-input teaching. The
/// query is taken exactly as written — the trim is only what decides
/// whether anything was asked at all.
fn parse_ask(input: &str) -> Result<Ask, String> {
    let parsed: Value = serde_json::from_str(input).map_err(|_| INVALID_INPUT_RESULT.to_owned())?;
    let query = parsed
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| INVALID_INPUT_RESULT.to_owned())?;
    if query.trim().is_empty() {
        return Err(INVALID_INPUT_RESULT.to_owned());
    }
    let page = match parsed.get("page") {
        None | Some(Value::Null) => FIRST_PAGE,
        Some(page) => u32::try_from(
            page.as_u64()
                .ok_or_else(|| INVALID_INPUT_RESULT.to_owned())?,
        )
        .map_err(|_| page_out_of_range_result())?,
    };
    Ok(Ask {
        query: query.to_owned(),
        page,
    })
}

/// The cache key: the query as written, case-folded and whitespace-
/// collapsed, plus the page. Deliberately not the guard's normalisation —
/// that exists to find one token, and a key stripped its way would merge
/// queries a member wrote differently.
fn cache_key(query: &str, page: u32) -> String {
    let folded = query.to_lowercase();
    let collapsed: Vec<&str> = folded.split_whitespace().collect();
    format!("{page}\u{1f}{}", collapsed.join(" "))
}

/// The person one turn's spend is booked to: the one person behind the
/// turn, resolved once in the provenance reading — the same resolution the
/// rights tool takes its subject from. Zero principals, several, or a
/// taker whose principal is unreadable all read as no single person, and
/// decline the spend instead of picking a bucket to book it to.
async fn principal(ctx: &ToolContext<'_, CoreEvent>) -> Result<i64, String> {
    let conversation_id = ctx.agency.conversation_id;
    let ledger = match ctx.agency.store.list_blocks(conversation_id).await {
        Ok(ledger) => ledger,
        Err(error) => {
            tracing::warn!(conversation_id, %error, "the web search's ledger read failed");
            return Err(TRANSIENT_RESULT.to_owned());
        }
    };
    sole_principal(&ledger, ctx.block_id).ok_or_else(|| NO_SINGLE_PERSON_RESULT.to_owned())
}

/// The host one link names, or `None` when the link names none. The URL
/// grammar is the one the HTTP client already parses every address with, so
/// the userinfo, the port and the path are dropped by the parser instead of
/// by a second reading of the same grammar written here. A link that does
/// not parse — a relative path, or prose a vendor put in the field — carries
/// no host, which is the `unknown` reading.
fn host(link: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(link).ok()?;
    // Already lowercased by the parser for a named host; the trailing root
    // dot is the one thing it keeps, and `example.com.` is `example.com`.
    let host = parsed.host_str()?.trim_end_matches('.').to_owned();
    (!host.is_empty()).then_some(host)
}

/// The source hint for one link, computed from the host and nothing else
/// (decided 2026-08-25, the table inlined with the unit): a wikipedia host
/// reads `encyclopedia`, a host carrying a government or education label
/// reads `official`, a known blog host reads `blog`, any other host reads
/// `website`, and a row without a host reads `unknown`. Generic by
/// instruction — a curated authority list was rejected with the unit.
#[must_use]
pub fn source_hint(link: &str) -> &'static str {
    let Some(host) = host(link) else {
        return UNKNOWN;
    };
    if host == "wikipedia.org" || host.ends_with(".wikipedia.org") {
        return ENCYCLOPEDIA;
    }
    // A government or education host, read by LABEL and not by suffix,
    // so `nasa.gov` and `www.gov.uk` both read official while `mygov.uk`
    // and a `gov.example.com` do not: the first label is a name, and only
    // the labels behind it say what kind of host this is.
    if host
        .split('.')
        .skip(1)
        .any(|label| label == "gov" || label == "edu")
    {
        return OFFICIAL;
    }
    if BLOG_HOSTS
        .iter()
        .any(|blog| host == *blog || host.ends_with(&format!(".{blog}")))
    {
        return BLOG;
    }
    WEBSITE
}

impl ToolHandler<CoreEvent> for WebSearch {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: NAME.into(),
            // The page range is written from the constants, so the copy the
            // model reads cannot drift from the bound the call enforces.
            description: format!(
                "Search the web and read the ranked results: each one's title, link, source \
                 hint and, where it has one, its snippet. Use it for questions about the \
                 world; facts about {ORGANIZATION} itself come from the project lookups and \
                 never from here. It opens no page — a snippet is all you get of a result. A \
                 later page of the same search may be requested with `page`, which runs from \
                 {FIRST_PAGE} to {LAST_PAGE} and may come back empty. Never put a person's \
                 handle in a query."
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to search for, in words. It is sent exactly as \
                                        written and is never corrected or shortened."
                    },
                    "page": {
                        "type": "integer",
                        "description": format!(
                            "Which page of results to read, from {FIRST_PAGE} to {LAST_PAGE}. \
                             Omit it for the first page."
                        )
                    }
                },
                "required": ["query"]
            }),
        }
    }

    crate::tools::admission::admits_at_required_authority!(NAME, REQUIRED_AUTHORITY);

    fn execute<'a>(
        &'a self,
        input: &'a str,
        ctx: ToolContext<'a, CoreEvent>,
    ) -> BoxFuture<'a, ToolOutcome> {
        Box::pin(async move {
            match self.answer(input, principal(&ctx)).await {
                Ok(page) => ToolOutcome::Done(page),
                Err(taught) => ToolOutcome::Error(taught),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// The person every unit test books its spend to — the resolution
    /// itself is the integration suite's, since a turn's dispatch anchor is
    /// not forgeable through any public write surface.
    const SOMEBODY: i64 = 7;

    /// A scripted provider: answers the queued pages in order and records
    /// every request, so a pin can prove what was sent — and that nothing
    /// was.
    struct ScriptedProvider {
        pages: Mutex<Vec<Result<Vec<SearchRow>, SearchFailure>>>,
        seen: Mutex<Vec<(String, u32)>>,
    }

    impl ScriptedProvider {
        fn new(pages: Vec<Result<Vec<SearchRow>, SearchFailure>>) -> Arc<Self> {
            Arc::new(Self {
                pages: Mutex::new(pages),
                seen: Mutex::new(Vec::new()),
            })
        }

        /// Every request the provider was asked to make, in order.
        fn requests(&self) -> Vec<(String, u32)> {
            self.seen.lock().expect("the request log locks").clone()
        }
    }

    impl SearchProvider for Arc<ScriptedProvider> {
        fn search<'a>(
            &'a self,
            query: &'a str,
            page: u32,
        ) -> BoxFuture<'a, Result<Vec<SearchRow>, SearchFailure>> {
            Box::pin(async move {
                self.seen
                    .lock()
                    .expect("the request log locks")
                    .push((query.to_owned(), page));
                let mut pages = self.pages.lock().expect("the script locks");
                if pages.is_empty() {
                    return Ok(Vec::new());
                }
                pages.remove(0)
            })
        }
    }

    /// One tool over the given script, with the script kept for reading.
    fn fixture(
        pages: Vec<Result<Vec<SearchRow>, SearchFailure>>,
    ) -> (WebSearch, Arc<ScriptedProvider>) {
        let provider = ScriptedProvider::new(pages);
        (
            WebSearch::with_provider(Box::new(Arc::clone(&provider))),
            provider,
        )
    }

    /// One row with a snippet.
    fn row(title: &str, link: &str) -> SearchRow {
        SearchRow {
            title: title.into(),
            link: link.into(),
            snippet: Some(format!("What {title} says.")),
        }
    }

    /// One call, booked to [`SOMEBODY`].
    async fn call(tool: &WebSearch, input: &str) -> Result<String, String> {
        tool.answer(input, async { Ok(SOMEBODY) }).await
    }

    /// One call booked to the given person.
    async fn call_for(tool: &WebSearch, input: &str, principal_id: i64) -> Result<String, String> {
        tool.answer(input, async move { Ok(principal_id) }).await
    }

    /// The taught text of a declined call, or a panic naming what came
    /// instead.
    fn declined(outcome: Result<String, String>) -> String {
        match outcome {
            Err(taught) => taught,
            Ok(page) => panic!("expected a decline, got a page: {page}"),
        }
    }

    // ─── AC7b: the guard refuses before anything is sent ─────────────────

    #[tokio::test]
    async fn a_person_reference_is_refused_and_nothing_is_sent() {
        let (tool, provider) = fixture(vec![Ok(vec![row("A page", "https://a.invalid/p")])]);
        for query in [
            "@handle",
            "@ h a n d l e",
            "@h.a.n.d.l.e",
            "@han\u{200b}dle",
            "@HaNdLe",
            "word\u{200b}@handle",
        ] {
            let input = json!({ "query": query }).to_string();
            let taught = declined(call(&tool, &input).await);
            assert_eq!(
                taught, PERSON_REFERENCE_RESULT,
                "the query {query:?} draws the fixed refusal"
            );
            assert!(
                !taught.to_lowercase().contains("handle"),
                "the refusal never echoes the matched token: {taught}"
            );
        }
        assert!(
            provider.requests().is_empty(),
            "a refused query reaches no vendor at all"
        );
    }

    /// `AC7d` at the tool's own edge: the pinned exceptions are sent
    /// untouched — the query the vendor receives is the query as written.
    #[tokio::test]
    async fn the_pinned_exceptions_are_sent_exactly_as_written() {
        let (tool, provider) = fixture(Vec::new());
        for (position, query) in [
            "duffy",
            "a.duffy@example.com",
            "@scope/package",
            "package@1.2.3",
        ]
        .into_iter()
        .enumerate()
        {
            let input = json!({ "query": query }).to_string();
            assert!(
                call_for(
                    &tool,
                    &input,
                    i64::try_from(position).expect("a small index") + 1
                )
                .await
                .is_ok(),
                "the ordinary query {query:?} is searched"
            );
            assert_eq!(
                provider.requests()[position],
                (query.to_owned(), FIRST_PAGE),
                "the vendor receives the query as written"
            );
        }
    }

    /// The refusal's copy, pinned verbatim: the rule, the fix, the
    /// no-retry-with-other-words line, and no echo.
    #[test]
    fn the_person_refusal_is_pinned_verbatim() {
        assert_eq!(
            PERSON_REFERENCE_RESULT,
            "No web search was made. A search query must never carry a person reference — an \
             at sign followed by a name. Search for the subject itself, with that token \
             removed. Do not retry the same query with the token spelled differently."
        );
    }

    // ─── AC7: the bounds ─────────────────────────────────────────────────

    #[tokio::test]
    async fn an_over_long_query_and_an_out_of_range_page_are_refused_with_their_numbers() {
        let (tool, provider) = fixture(Vec::new());

        let over = "x".repeat(QUERY_LIMIT + 1);
        let taught = declined(call(&tool, &json!({ "query": over }).to_string()).await);
        assert_eq!(taught, over_long_result());
        assert!(
            taught.contains(&QUERY_LIMIT.to_string()),
            "the refusal names the limit: {taught}"
        );

        let exact = "x".repeat(QUERY_LIMIT);
        assert!(
            call(&tool, &json!({ "query": exact }).to_string())
                .await
                .is_ok(),
            "the limit itself is inside the bound"
        );

        for page in [0, 6, 99] {
            let input = json!({ "query": "kernel", "page": page }).to_string();
            assert_eq!(
                declined(call(&tool, &input).await),
                page_out_of_range_result(),
                "page {page} is outside the range"
            );
        }
        assert_eq!(
            provider.requests().len(),
            1,
            "only the query at the bound was ever sent"
        );
    }

    /// A malformed call is its own teaching, judged before anything else.
    #[tokio::test]
    async fn a_malformed_call_answers_the_input_teaching() {
        let (tool, provider) = fixture(Vec::new());
        for input in [
            "not json",
            "{}",
            r#"{"query": 7}"#,
            r#"{"query": "   "}"#,
            r#"{"query":"kernel","page":"two"}"#,
        ] {
            assert_eq!(
                declined(call(&tool, input).await),
                INVALID_INPUT_RESULT,
                "the input {input:?} answers the input teaching"
            );
        }
        assert!(
            provider.requests().is_empty(),
            "a malformed call reaches no vendor"
        );
    }

    // ─── AC3: the source hint, per row ───────────────────────────────────

    #[test]
    fn the_source_hint_follows_the_table_per_row() {
        for (link, expected) in [
            ("https://en.wikipedia.org/wiki/Kernel", ENCYCLOPEDIA),
            ("https://wikipedia.org/wiki/Kernel", ENCYCLOPEDIA),
            ("https://www.nasa.gov/mission", OFFICIAL),
            ("https://cs.stanford.edu/page", OFFICIAL),
            ("https://www.gov.uk/guidance", OFFICIAL),
            ("https://example.medium.com/post", BLOG),
            ("https://medium.com/@writer/post", BLOG),
            ("https://dev.to/someone/post", BLOG),
            ("https://example.invalid/page", WEBSITE),
            ("http://user:pw@example.invalid:8443/page?q=1", WEBSITE),
            ("not a link at all", UNKNOWN),
            ("", UNKNOWN),
            ("/relative/path", UNKNOWN),
        ] {
            assert_eq!(
                source_hint(link),
                expected,
                "the hint for {link:?} follows the table"
            );
        }
    }

    // ─── AC2: the envelope renders what arrived ──────────────────────────

    #[test]
    fn the_envelope_states_the_query_the_page_and_the_count() {
        let rows = vec![
            SearchRow {
                title: "A page".into(),
                link: "https://en.wikipedia.org/wiki/Kernel".into(),
                snippet: Some("A snippet.".into()),
            },
            SearchRow {
                title: "No snippet here".into(),
                link: "https://example.invalid/page".into(),
                snippet: None,
            },
        ];
        assert_eq!(
            page_result("linux kernel", 2, &rows),
            "Web search results for: linux kernel\n\
             Page: 2\n\
             Results: 2\n\
             \n\
             1. A page\n\
             Link: https://en.wikipedia.org/wiki/Kernel\n\
             Source: encyclopedia\n\
             Snippet: A snippet.\n\
             \n\
             2. No snippet here\n\
             Link: https://example.invalid/page\n\
             Source: website"
        );
    }

    /// A full page renders every row it carries, numbered from one, and
    /// states the count it actually returned — the envelope's count is what
    /// arrived, never what was requested.
    #[test]
    fn a_full_page_renders_every_row_it_carries() {
        let rows: Vec<SearchRow> = (1..=10)
            .map(|n| {
                row(
                    &format!("Result {n}"),
                    &format!("https://example.invalid/{n}"),
                )
            })
            .collect();
        let rendered = page_result("kernel", 1, &rows);
        assert!(rendered.contains("Results: 10"));
        for n in 1..=10 {
            assert!(
                rendered.contains(&format!("{n}. Result {n}")),
                "row {n} renders under its own number"
            );
        }
    }

    /// The bounds on the rendered fields: a long title and a long snippet
    /// are cut through the shared truncation, and the link is never cut —
    /// a truncated link is a broken one.
    #[test]
    fn a_long_title_and_snippet_are_bounded_and_the_link_is_not() {
        let link = format!("https://example.invalid/{}", "p".repeat(500));
        let rows = vec![SearchRow {
            title: "t".repeat(TITLE_LIMIT + 10),
            link: link.clone(),
            snippet: Some("s".repeat(SNIPPET_LIMIT + 10)),
        }];
        let rendered = page_result("q", 1, &rows);
        assert!(
            rendered.contains(&format!("1. {}…", "t".repeat(TITLE_LIMIT))),
            "the title is bounded with the shared marker"
        );
        assert!(
            rendered.contains(&format!("Snippet: {}…", "s".repeat(SNIPPET_LIMIT))),
            "the snippet is bounded with the shared marker"
        );
        assert!(rendered.contains(&link), "the link renders whole");
    }

    // ─── AC4: the empty readings ─────────────────────────────────────────

    #[test]
    fn an_empty_first_page_and_an_exhausted_later_page_read_differently() {
        let empty = no_results_result("kernel");
        let exhausted = exhausted_result("kernel", 3);
        assert_ne!(empty, exhausted);
        assert!(empty.contains("no results"), "{empty}");
        assert!(
            exhausted.contains("ended at page 2"),
            "the exhausted reading names the last page that answered: {exhausted}"
        );
    }

    #[tokio::test]
    async fn an_empty_page_answers_its_reading_rather_than_a_failure() {
        let (tool, _) = fixture(vec![Ok(Vec::new()), Ok(Vec::new())]);
        assert_eq!(
            call(&tool, r#"{"query":"kernel"}"#).await,
            Ok(no_results_result("kernel")),
            "an empty first page is an honest nothing"
        );
        assert_eq!(
            call(&tool, r#"{"query":"kernel","page":3}"#).await,
            Ok(exhausted_result("kernel", 3)),
            "an empty later page ended at the page before it"
        );
    }

    #[tokio::test]
    async fn a_later_page_repeating_the_previous_one_reads_as_exhausted() {
        let page = vec![row("A page", "https://a.invalid/p")];
        let (tool, _) = fixture(vec![Ok(page.clone()), Ok(page)]);
        // Page one is answered and cached; page two repeats it, which a
        // vendor past its real limit does instead of sending an empty page.
        let first = call(&tool, r#"{"query":"kernel","page":1}"#).await;
        assert!(
            first.as_deref().is_ok_and(|page| page.contains("Page: 1")),
            "page one answers as a page: {first:?}"
        );
        assert_eq!(
            call(&tool, r#"{"query":"kernel","page":2}"#).await,
            Ok(exhausted_result("kernel", 2)),
            "a repeated page reads as exhausted"
        );
    }

    // ─── AC4: the failures, each distinguishable ─────────────────────────

    #[test]
    fn every_failure_teaches_its_own_result_and_none_carries_a_status() {
        let failures = [
            SearchFailure::RefusedKey,
            SearchFailure::Refused,
            SearchFailure::RateLimited,
            SearchFailure::Unreachable,
            SearchFailure::Timeout,
            SearchFailure::UnusableAnswer,
            SearchFailure::Unreadable,
        ];
        let taught: Vec<&str> = failures.iter().map(|failure| failure.taught()).collect();
        for (position, one) in taught.iter().enumerate() {
            for other in &taught[position + 1..] {
                assert_ne!(one, other, "two failures teach the same result");
            }
        }
        for result in taught
            .iter()
            .copied()
            .chain([no_results_result("q").as_str(), TRANSIENT_RESULT])
        {
            for status in ["403", "429", "500", "302", "HTTP"] {
                assert!(
                    !result.contains(status),
                    "a taught result carries a bare status: {result}"
                );
            }
        }
    }

    // ─── AC6: the key never appears ──────────────────────────────────────

    #[test]
    fn the_rendered_configuration_carries_no_fragment_of_the_key() {
        let key = "sk-search-0123456789abcdef";
        let config = SearchConfig {
            base_url: "https://example.invalid".into(),
            api_key: key.into(),
            country: Some("de".into()),
            language: "en".into(),
        };
        let rendered = format!("{config:?}");
        assert!(
            rendered.contains(REDACTED_KEY),
            "the debug rendering marks the key redacted: {rendered}"
        );
        for fragment in ["sk-search", "0123456789", "abcdef", key] {
            assert!(
                !rendered.contains(fragment),
                "the rendered configuration carries a fragment of the key: {rendered}"
            );
        }
    }

    #[test]
    fn no_failure_path_text_carries_a_fragment_of_a_key() {
        let key = "sk-search-0123456789abcdef";
        for result in [
            REFUSED_KEY_RESULT,
            REFUSED_REQUEST_RESULT,
            RATE_LIMITED_RESULT,
            UNREACHABLE_RESULT,
            TIMEOUT_RESULT,
            UNUSABLE_ANSWER_RESULT,
            UNREADABLE_RESULT,
            TRANSIENT_RESULT,
            PERSON_REFERENCE_RESULT,
            NO_SINGLE_PERSON_RESULT,
            INVALID_INPUT_RESULT,
        ] {
            for fragment in ["sk-search", "0123456789", "abcdef", key, "X-API-KEY"] {
                assert!(
                    !result.contains(fragment),
                    "a failure path names a key fragment: {result}"
                );
            }
        }
    }

    // ─── The cache key ───────────────────────────────────────────────────

    #[test]
    fn the_cache_key_folds_case_and_collapses_whitespace_and_keeps_the_page() {
        assert_eq!(cache_key("Linux Kernel", 1), cache_key("linux   kernel", 1));
        assert_eq!(
            cache_key("  linux kernel ", 1),
            cache_key("linux kernel", 1)
        );
        assert_ne!(cache_key("linux kernel", 1), cache_key("linux kernel", 2));
        assert_ne!(
            cache_key("linuxkernel", 1),
            cache_key("linux kernel", 1),
            "the key is not the guard's normalisation: a space is a word boundary"
        );
    }

    // ─── The cache and the budget (AC7) ──────────────────────────────────

    /// A repeated query inside the cache window is answered from memory:
    /// the vendor is asked exactly once, whatever the case and spacing the
    /// second call wrote.
    #[tokio::test]
    async fn a_repeated_query_costs_no_second_request() {
        let (tool, provider) = fixture(vec![Ok(vec![row("A page", "https://a.invalid/p")])]);
        let first = call(&tool, r#"{"query":"Linux Kernel"}"#).await;
        let second = call(&tool, r#"{"query":"linux   kernel"}"#).await;
        assert_eq!(
            first,
            Ok(page_result(
                "Linux Kernel",
                FIRST_PAGE,
                &[row("A page", "https://a.invalid/p")]
            ))
        );
        assert_eq!(
            second,
            Ok(page_result(
                "linux   kernel",
                FIRST_PAGE,
                &[row("A page", "https://a.invalid/p")]
            )),
            "the second call is answered from the first's rows, stated \
             under the query its own caller wrote"
        );
        assert_eq!(
            provider.requests(),
            vec![("Linux Kernel".to_owned(), FIRST_PAGE)],
            "the repeat cost no second request"
        );
    }

    /// The budget's cap, per person: the sixth distinct query inside the
    /// window declines, a second person's searches are bounded on their
    /// own, and a cache hit is served on the spent budget because it spends
    /// nothing. The window's expiry is pinned under paused time on the
    /// bound itself, in `crate::window` — a paused clock and this tool's
    /// own awaits do not mix.
    #[tokio::test]
    async fn the_budget_caps_per_person_and_a_cache_hit_is_served_past_it() {
        let (tool, provider) = fixture(Vec::new());
        let queries: Vec<String> = (0..SEARCH_BUDGET_CAP)
            .map(|n| json!({ "query": format!("query {n}") }).to_string())
            .collect();
        for input in &queries {
            assert!(
                call(&tool, input).await.is_ok(),
                "a search inside the cap is made"
            );
        }
        assert_eq!(
            provider.requests().len(),
            SEARCH_BUDGET_CAP as usize,
            "every search inside the cap reached the vendor"
        );

        let over = json!({ "query": "one search too many" }).to_string();
        assert_eq!(
            declined(call(&tool, &over).await),
            budget_spent_result(),
            "the search past the cap declines"
        );
        assert_eq!(
            provider.requests().len(),
            SEARCH_BUDGET_CAP as usize,
            "the declined search reached no vendor"
        );

        assert!(
            call(&tool, &queries[0]).await.is_ok(),
            "a cache hit is served even on a spent budget: it spends nothing"
        );
        assert_eq!(
            provider.requests().len(),
            SEARCH_BUDGET_CAP as usize,
            "and it still reached no vendor"
        );

        assert!(
            call_for(&tool, &over, SOMEBODY + 1).await.is_ok(),
            "another person's budget is their own"
        );
    }

    /// A failed request hands its grant straight back: the bound exists for
    /// metered spend, and a refused key bills nothing, so a person whose
    /// searches all failed has spent none of them.
    #[tokio::test]
    async fn a_failed_request_costs_the_person_nothing() {
        let failures: Vec<Result<Vec<SearchRow>, SearchFailure>> = (0..=SEARCH_BUDGET_CAP)
            .map(|_| Err(SearchFailure::RateLimited))
            .collect();
        let (tool, provider) = fixture(failures);
        for n in 0..=SEARCH_BUDGET_CAP {
            let input = json!({ "query": format!("query {n}") }).to_string();
            assert_eq!(
                declined(call(&tool, &input).await),
                RATE_LIMITED_RESULT,
                "the failure teaches its own result, past the cap as within it"
            );
        }
        assert_eq!(
            provider.requests().len(),
            SEARCH_BUDGET_CAP as usize + 1,
            "every attempt reached the vendor: none of them spent a grant"
        );
    }
}
