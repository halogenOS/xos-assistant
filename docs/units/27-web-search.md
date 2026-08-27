# Unit 27 — the assistant can search the web

Date: 2026-08-25. Revision 2, 2026-08-27, rebuilt against a cold probe that verified the
vendor live and ran the spec's claims against the tree. The largest corrections, named up
front so they are not re-made:

- The guard's display-name half had no data to match: display names are deliberately not
  stored (decision 0077, `identity.rs:8-10`) and the adapter never even translates them
  (`translate.rs:743-745`). A guard that collects more personal data in order to protect it
  is absurd; that half is dropped and the reason recorded.
- The vendor returns **no total-results field** — the envelope's `total` and `has_more`
  were promises a stub would fake and production would break. Both are gone.
- Revision 1 decided the unconfigured-key case twice, in opposite ways. One decision
  remains.
- The privacy claim this unit touches lives in six documents, including the published
  policy's closed recipient table; revision 1 named two.

## What this unit is

The assistant answers from the project's own sources and from what the model knows; asked
anything else it says it cannot check. This unit gives it one tool: a web search returning
ranked results with their snippets. It opens no page — fetch is unit 29's, with its robots
handling, origin-refusal memory and consent rules — and that boundary is what keeps this
unit small enough to be safe.

## Grounding

**The tool seam exists whole.** `ToolHandler<CoreEvent>` with `definition()`/`execute()`
(`core/src/tools/wiki.rs:312`), admitted at a required authority (`core/src/tools/mod.rs:91`,
`ToolSet::admit` at `:114`), palette recorded per conversation with reconciliation on first
activity per process (`assembly.rs:1463`, both directions correct — verified). Conditional
admission has a precedent: the report tool is admitted only when its handle is configured,
and the SAME predicate gates its teaching, "so the prompt never teaches a tool the palette
does not carry" (`assembly.rs:436-449`). Optional secrets have a precedent:
`mirror_token: Option<SecretRef>` (`config.rs:545`, resolved in `main.rs:211-218`).

**The vendor, verified live.** `POST https://google.serper.dev/search`, `X-API-KEY` header,
JSON body. `page` is a real request parameter (echoed in `searchParameters` in every
published sample). Auth failure is **403** — for a missing and for a bad key alike, with
the distinction only in the JSON `message` body. There is **no total-results field** in any
response, and no public API documentation without an account: the citation for response
shapes is a live probe at integration time, and the tests pin against a stub built from
recorded real responses. Sample pages run short (8 organic rows for a 10-row request);
rows can lack `snippet` and can carry `position`, `date`, `sitelinks`, `attributes`.
Requests default to `autocorrect: true` and `gl: "us"`, `hl: "en"`. Serper's terms are
UK-law (adequacy-covered), and its privacy policy states that where personal data is
processed the customer is controller and Serper processor.

**The shared lookup layer is GET-only and its failure wording is the bare status this unit
forbids.** Every lookup path is `client.get(url)` (`lookup.rs:101`) and the shared error
renders "answered HTTP 403" (`lookup.rs:125`). Decision 0044 (failures reach the model,
never the chat) holds and is unchanged.

**The sourcing teaching routes all substantive claims to lookups.** `teaching.rs:143-159`:
"Your lookup tools are the only source of substantive claims: any claim about the project…
must come from a lookup you made in this turn." Registering a web search inside "your
lookup tools" without a carve-out would let a random web page back a *project* claim.

**What the identity tables can and cannot answer.** Stored per message: the speaker's
public username. Not stored anywhere: display names (decision 0077). The principals table
is adapter-scoped, not conversation-scoped.

**The tool-record erasure gap, and who accepted it on what grounds.** A tool call and its
result live on framework tables erasure does not reach; decision 0045 accepted that gap
because "the group tools are project lookups whose inputs are overwhelmingly technical."
A model-written query derived from conversation is not that, and this unit must not
quietly widen a decision whose stated ground it removes.

## Decisions taken with this unit

- **One tool, search only, 2026-08-25.** Titles, links, snippets; it opens nothing.
  *Rejected:* shipping search and fetch together (fetch's whole apparatus rides in on a
  search box).
- **The vendor sits behind a trait, 2026-08-25.** `SearchProvider`, one implementation.
  The tool owns the envelope, the guard and the teaching surface; the implementation owns
  the endpoint, the key and the response shape. *Rejected:* one concrete client.
- **Member authority, and a per-person search budget, 2026-08-27.** Admitted at
  `Authority::Member` — the tool exists to answer members' questions. Because each call is
  a paid request to a metered vendor and the model chooses when to call, the tool takes a
  per-person windowed budget in the shape of the reply bound the command family already
  uses (`window.rs`): a spent budget declines with a fixed result naming when to try
  again, and nothing is sent. *Rejected:* no budget (a single member can drive unbounded
  spend); *rejected:* a global budget alone (one member exhausts it for the group).
- **A small same-query cache, 2026-08-27.** Same normalised query and page within the
  cache window answers from memory, mirroring the wiki lookup's cache precedent
  (`wiki.rs:24-31`) — here the reason is spend, not rate limits. *Rejected:* no cache
  (identical retries within a turn or a conversation each bill).
- **The envelope promises only what the vendor can keep, 2026-08-27.** A page of results,
  each with title, link, snippet where present, and a host-derived source hint; the
  envelope states the query as sent, the page number and the count returned. There is no
  `total` and no `has_more` — the vendor answers neither, and a stubbed number would pass
  the pins while lying in production. An empty page one renders "no results"; an empty
  later page renders that the results ended at the previous page — the two are
  distinguishable by the page number the model itself supplied, closing the
  empty-versus-exhausted ambiguity honestly. The tool's description says a next page may
  be requested and may turn out empty. *Rejected:* `has_more` computed as "the page came
  back full" (a guess wearing a field name); *rejected:* revision 1's `total`.
- **The rendered form is prose, like every shipped tool, 2026-08-27.** `ToolOutcome::Done`
  carries a human-readable result (the repo's convention: `commit.rs:157`); the envelope's
  facts are stated lines, pinned as lines. Result titles and snippets are bounded through
  the existing `lookup::truncated`. *Rejected:* a JSON body (a serialization the repo has
  never chosen, decided as a side effect of a search unit).
- **Ten results requested, whatever arrives rendered, 2026-08-27.** `num` is sent with the
  page; a short page renders what came, a missing snippet renders the title and link
  without one, and unknown row fields are ignored. *Rejected:* pinning "ten results" (the
  vendor's own samples return eight).
- **Autocorrect off, locale explicit, 2026-08-27.** The request sends
  `autocorrect: false`, because results answering a silently corrected query would break
  this unit's own rule that what is sent — and what is answered — is the query as written.
  `gl` and `hl` come from configuration with the defaults stated in the config's own
  documentation (`hl` defaulting to English, `gl` unset), so an international group's
  results are a deployment choice rather than a vendor default nobody chose. *Rejected:*
  the vendor defaults (US-English answers for a non-US community, and corrected queries
  presented as uncorrected).
- **The source hint is computed from the host and nothing else, 2026-08-25.** Exactly the
  operator's table; generic by instruction. *Rejected:* a curated authority list.
- **The lookup layer grows a POST seam rather than being bypassed, 2026-08-27.** The
  shared timeout, redirect and body-cap discipline is the reason `lookup.rs` exists; the
  search provider posts through it. The search tool maps the vendor's failures to taught
  results itself — the shared "answered HTTP {status}" wording is exactly what this unit
  forbids, so the tool never surfaces it. The 403 case reads the vendor's JSON `message`
  to distinguish a missing or refused key from anything else, and each taught failure —
  refused key, rate limit, unreachable host — is distinguishable from the others and from
  an honest empty page. *Rejected:* a private client beside the lookup layer (loses the
  shared discipline and records the same decision twice).
- **Availability is configuration, and absence means not admitted, 2026-08-27.** With no
  key configured the tool is not in the palette and its teaching is not composed — one
  predicate for both, the report tool's exact precedent. Revision 1 also specified a
  structured refusal for a missing key; that sentence decided the same case the opposite
  way and is deleted — there is no call path on which an unconfigured tool can answer.
  *Rejected:* admitting it and failing on use.
- **The key is a secret reference, never a value, 2026-08-25.** `Option<SecretRef>`, the
  mirror token's exact shape.
- **The PII guard is the handle-form rule, matched on a normalised query, and it needs no
  member list, 2026-08-27.** The tool refuses a query containing a handle-form token: an
  at sign starting a token, followed by name characters. The definition is platform-
  neutral by construction — it covers a bare platform handle and a federated one alike,
  and no platform vocabulary enters the core. Exceptions, each pinned: an at sign preceded
  by a word character (an email address), an at-name followed by a slash (a scoped package),
  and an at sign followed by digits in a version context. Matching runs on a normalised
  view — zero-width and formatting characters removed, case folded, and single separators
  between the letters of a candidate token collapsed, so `@ h a n d l e`, `@h.a.n.d.l.e`
  and `@handle` are one token — while what is sent to the vendor is always the query as
  written, or nothing. Normalisation is applied to FIND the token, never to the whole
  query as one string, so word boundaries survive and single ordinary words are never
  matched — the operator's rule of 2026-08-25 verbatim: a search for a word somebody
  happens to be called is chance, not a submission of personal data.
  *Rejected:* the display-name half (no data — decision 0077 dropped display names, the
  adapter never translates them, and re-collecting them to protect them would falsify four
  pinned document statements and breach the assessment's own safeguard);
  *rejected:* a member-list check on the handle (the principals table is adapter-scoped
  and only holds people who spoke, so the check would miss exactly the mentioned
  bystanders it exists for — the FORM is refused regardless of whose it is);
  *rejected:* confusable folding (a UTS-39 table is a new dependency guarding against an
  adversary this guard does not face: the query author is our own model, the failure mode
  is carelessness, and no lexical guard stops a model that paraphrases a person instead —
  the guard is a discipline device, recorded as such).
- **The refusal teaches without echoing, 2026-08-27.** A refused query answers with the
  rule and the fix — remove the handle-form token — and does NOT echo the matched token,
  because a tool result is a framework record erasure cannot reach, and a guard that
  writes the identifier it refused into permanent storage protects nothing. Fixed wording,
  no-retry-with-other-words line, the repo's convention. *Rejected:* naming what was
  matched (revision 1 — see decision 0045's grounds).
- **Bounds, named, 2026-08-27.** Query maximum 400 characters, refused whole with the
  limit named, never truncated. Request timeout: the lookup layer's existing default. Page
  bounds: 1 through 5 — past that the model is fishing, and the budget is better spent on
  a reworded query.
- **The model is taught what a search is and is not, 2026-08-27.** Three sentences of
  teaching, gated on the same predicate as admission: a snippet is a hint and an answer
  built on one says where it came from; a snippet that does not contain the claim is a
  miss, exactly as the existing lookup rule states; and **project facts still come only
  from the project lookups** — the web tool answers questions about the world and is
  never the source for a claim about the project. *Rejected:* leaving the sourcing rule
  unamended (the moment search registers, "your lookup tools" includes a web proxy and
  the project-claim rule silently authorises it).
- **Decision 0045 is amended, not silently widened, 2026-08-27.** Its acceptance of the
  tool-record erasure gap rested on lookup inputs being "overwhelmingly technical". A
  model-written query from conversation removes that ground for this tool, so the record
  gains a dated amendment stating the widened surface and the two mitigations that answer
  it: the guard (no deliberate member identifier leaves or is recorded) and the
  no-echo refusal.
- **The privacy documents move with this unit — all six sites, 2026-08-27.** A
  member-derived query to a new third party touches: the **published policy** (the closed
  recipient table gains the search vendor, the "data leaves the EU/EEA in three places"
  sentence becomes four with the UK adequacy basis, and the sourcing sentence is
  re-checked), the **record of processing** (recipient row with role — processor, per its
  own terms — and transfer basis; §7 transfer list; the two §9 minimisation rows; §10
  gains the missing processor agreement as an open dependency; §11's review trigger is
  acknowledged as fired; §3's purpose P1, scoped to questions "about the project", gains
  the general-questions purpose this unit exists for), the **impact assessment**
  (recipient list, transfer list, a new risk-register row for member words reaching a
  third party, the residual re-rating, and an addendum per its own trigger), and the
  **legitimate-interests assessment** (safeguards 2 and 3 are procedural obligations this
  unit trips: the re-weigh is performed and recorded with this unit's date). Every edit
  carries a dated note. *Rejected:* the two-document list of revision 1 — shipping it
  would have made the published policy false, the exact defect class the unit itself
  names.
- **The unit's decisions are recorded as decision records, 2026-08-27.** The dated
  decisions above land in `docs/decisions` per the repository's convention, and the
  acceptance criteria check it — fourteen inline decisions shipping unrecorded was
  revision 1's omission.

## The unit's contract

With a key configured, the assistant can search the web through one member-authority tool
under a per-person windowed budget; with no key, the tool is not in the palette and its
teaching is not composed. A search sends the query exactly as written — autocorrect off,
locale from configuration — or sends nothing: a query over 400 characters, or one
containing a handle-form token outside the pinned exceptions, is refused whole with a
fixed result that names the rule and never echoes the token. A result page renders as
prose: each returned row's title, link, snippet where present and host-derived source
hint, with the query, page number and count stated; no total and no more-pages promise is
made; an empty first page and an exhausted later page read differently. Vendor failures
reach the model as taught results — refused key, rate limit, unreachable host, each
distinguishable — and never the chat, and no bare status number ever surfaces. Project
facts still come only from the project lookups. Nothing is stored beyond the ordinary
tool record, decision 0045 carries its amendment, and all six privacy documents carry the
new recipient with dated notes before this ships.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; any new dependency named in
  `docs/dependency-review.md` with its web-checked version and registry history before
  the manifest names it.
- **AC2** The envelope renders what arrived: a full page, a short page (fewer rows than
  requested), and a row without a snippet each render their stated lines — pinned against
  a stub built from recorded real vendor responses, including the short-page sample.
- **AC3** The source hint matches the operator's table per row, including the no-host
  case — pinned per row.
- **AC4** Failures teach and are distinguishable: refused key (403 with the vendor's
  message read), rate limit, unreachable host, and the two empty-page readings — five
  pinned results, none carrying a bare status number, none reaching the chat.
- **AC5** Unconfigured means absent: with no key, the tool is not in the palette AND the
  teaching is not in the composed prompt — both pinned on one predicate, and nothing
  fails at startup.
- **AC6** The key never appears: rendered configuration, logs, error paths — the existing
  secret scan plus a direct assertion on the failure path.
- **AC7** The bounds hold: a 401-character query is refused naming the limit with nothing
  sent (stub asserts no request); page 6 is refused; the budget declines when spent and
  recovers with the window — each pinned.
- **AC7b** The guard refuses the handle form: a bare handle token, one spaced out, one
  dotted, one carrying zero-width characters, one in mixed case — each refused, stub
  asserting no request arrived, and the refusal text pinned to contain no fragment of the
  matched token.
- **AC7d** The guard passes ordinary searches, with equal weight to AC7b: a single common
  word that happens to be a member's handle (member present in the conversation), an
  email address, a scoped package name, a version string — each sent untouched, pinned.
- **AC8** The documents move, all six sites: policy, record of processing (including §3,
  §7, §9, §10, §11), impact assessment (including the risk row and addendum),
  legitimate-interests re-weigh, decision 0045's amendment — each with a dated note,
  checked per file and pinned by the documentation suite.
- **AC9** The decisions are recorded: the unit's dated decisions exist in
  `docs/decisions`, and the teaching carve-out (project facts from project lookups only)
  is pinned in the composed prompt.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-search`, branch
  `unit/web-search`). Sites: a new tool module beside `core/src/tools/wiki.rs`; the POST
  seam in `core/src/tools/lookup.rs`; conditional admission and teaching on one predicate
  (the report tool's shape, `assembly.rs:436-449`); the budget beside the reply bound's
  mechanism (`window.rs`); configuration and secret in `crates/assistant/src/config.rs`
  and `main.rs` (the mirror token's shape); the teaching in `core/src/teaching.rs` with
  the sourcing carve-out; `docs/decisions` (new records + the 0045 amendment); and the
  four privacy documents.
- Read `wiki.rs` end to end first — the closest tool: external HTTP, bounded result,
  cache, a description that teaches when to call. Then `rights.rs` for fixed-result
  refusals with no-retry lines.
- The operator's design reference is at
  `~/.local/state/halogenos-assistant/search-design-reference.md`; the envelope and
  refusal discipline in this revision deliberately diverge from it where the live vendor
  cannot honour it (`total`, structured unconfigured-key errors) — this spec wins.
- The vendor publishes no open API docs; the response-shape citation is recorded real
  responses (fixtures checked in beside the stub), refreshed at integration time when the
  operator's key exists.
- The prompt change means the prompt-refresh mechanism forks live conversations on the
  next deploy start — existing behaviour, nothing to build, stated so nobody rediscovers
  it as a surprise.
