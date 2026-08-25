# Unit 27 — the assistant can search the web

Date: 2026-08-25. The assistant answers from the project's own sources — the wiki, the
forge, the release mirror — and from what the model already knows. Asked anything the
project has not written down, it says it cannot check. That is honest and it is a wall a
community assistant hits constantly, because most questions in a support group are about
the world the project sits in rather than the project itself.

This unit gives it one tool: a web search returning ranked results with their snippets.
It does not give it the ability to open a page; that is unit 29, and the boundary is the
whole reason this unit is small enough to be safe.

## Grounding

**The tool seam already exists and this unit invents nothing.** A lookup implements
`ToolHandler<CoreEvent>` with `definition()` and `execute()` (`core/src/tools/wiki.rs:312`),
is admitted at a required authority into the palette
(`core/src/tools/mod.rs:91-110`), and is registered per conversation. The three shipped
lookups all take an endpoint and a timeout at construction, which is how a test points them
at a local server. A failure reaches the model and never the chat (decision 0044).

**The provider is one small adapter behind a trait, already written elsewhere.** The
sibling project's Serper adapter is 97 lines: `POST https://google.serper.dev/search` with
an `X-API-KEY` header and a JSON body of `q` and `num`, decoding an `organic` array of
`title`, `link` and `snippet` into a neutral result. It sits behind a `SearchProvider`
trait with a registry, so the vendor is one implementation rather than the shape. That
split is worth carrying over: this unit specifies the trait and one implementation.

**What the operator's own design doc asks for**, beyond "call an API":
- **Pagination is mandatory**, ten results a page, with `query`, `results`, `total`,
  `page`, `page_size` and `has_more` in the envelope, so the model spends tokens
  deliberately and asks for more only when it wants more.
- **A source hint per result**, computed from the host alone — encyclopedia, official,
  blog, website, unknown — so the model can weigh sources without opening them. The
  doc is explicit that this stays generic: no curated authority list, because a
  domain-specific taxonomy belongs to whoever needs it and not to a generic tool.
- **A failed read must teach.** A bare status code "is true and useless": it says what
  failed, not what it means or what to do instead.

**A search query is personal data leaving for a new recipient.** The record of processing
enumerates who receives what (`docs/privacy/records-of-processing.md`, section 6): the
model provider, its sub-processors, public project sources, the chat platform, the group's
administrators. A search provider is none of them. The query is written by the model from
the conversation, so it can carry a member's words.

## Decisions taken with this unit

- **One tool, search only, 2026-08-25.** It returns what the search API returns: titles,
  links, snippets. It opens nothing. A snippet is a hint and the tool's own description
  says so, so the model does not present a snippet as a checked fact.
  *Rejected:* shipping search and fetch together — fetch carries robots handling, an
  origin-refusal memory, content extraction and a consent rule about which links may be
  opened, and none of that should ride in on the back of a search box.
- **The vendor sits behind a trait, 2026-08-25.** A `SearchProvider` with one
  implementation today. The tool holds the pagination, the source hint and the envelope;
  the implementation holds the endpoint, the key and the response shape. *Rejected:* one
  concrete client (the second provider would rewrite the tool rather than register beside
  it).
- **Ten results a page, and the model asks for the next, 2026-08-25.** With `has_more`
  stated in the envelope and in the tool's description. *Rejected:* returning everything
  (a search answer is the largest thing this assistant would ever put in a turn, and most
  of it is never read).
- **The source hint is computed from the host and nothing else, 2026-08-25.** Exactly the
  operator's table. *Rejected:* a project-specific ranking, per the doc's own instruction.
- **A refusal is answered with what it means, 2026-08-25.** The provider's own errors are
  reported to the model with the meaning and the next step, not as a status number. A key
  the deployment has not configured is a structured refusal naming the missing
  configuration, not a crash and not a silent empty result. *Rejected:* returning zero
  results on a failure — indistinguishable from a genuine miss, and it teaches the model
  that the web has nothing on the subject.
- **Availability is configuration, and its absence is not an error, 2026-08-25.** With no
  key configured the tool is not admitted to the palette at all, so the model is never
  told about a capability it does not have. *Rejected:* admitting it and failing on use.
- **The key is a secret reference, never a value in configuration, 2026-08-25.** The same
  indirection the bot token and the model key already use: a name the process resolves at
  startup, so the value is never in the repository, never in the rendered configuration and
  never in a log.
- **The privacy documents move with this unit, 2026-08-25.** The record of processing gains
  the search provider as a recipient, naming what it receives — a query written by the
  model, which may contain a member's words — and the impact assessment gains the transfer
  and its purpose. This is not a follow-up: shipping the tool without it makes a published
  statement false. *Rejected:* deferring the documents to a later pass.
- **The model is taught what a search result is worth, 2026-08-25.** The prompt's sourcing
  discipline already binds it to back a substantive claim with a lookup; this extends that
  to say a snippet is a hint, that a result's title is not evidence, and that it should say
  where an answer came from when the answer came from a search.
- **The query is the model's, and it is bounded, 2026-08-25.** The tool takes a query
  string with a stated maximum length, refusing an over-long one with a message that says
  so rather than truncating a member's words into something they did not say.

## The unit's contract

The assistant can search the web through one registered tool when a deployment has
configured a search key, and cannot when it has not, in which case the tool does not exist
as far as the model is concerned. A search returns one page of ten ranked results, each
with its title, link, snippet and a host-derived source hint, in an envelope stating the
total and whether more remain. A provider failure reaches the model as a stated meaning
with a next step, never as a bare status and never as an empty result; a failure never
reaches the chat. The query is bounded and refused whole when it is too long. No page is
fetched, no link is followed, and nothing is stored beyond the ordinary record of the tool
call and its result. The record of processing and the impact assessment name the search
provider as a recipient before this ships.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; any new dependency justified in the report
  and checked against its registry for a known compromised release before it is added.
- **AC2** A search returns the envelope: ten results with title, link, snippet and source
  hint, and `query`, `total`, `page`, `page_size`, `has_more` — pinned against a local
  stub server, with page two proving the pagination rather than the first page proving the
  shape.
- **AC3** The source hint matches the stated table for each row of it, including the
  no-host case — pinned per row.
- **AC4** A provider failure teaches: an error from the search API reaches the model with a
  meaning and a next step, and never reaches the chat — pinned for a refused key, a
  rate-limited response and an unreachable host, each distinguishable from the others and
  from an honest zero-result search.
- **AC5** Unconfigured means absent: with no key, the tool is not in the palette and the
  model is not told of it — pinned, including that nothing fails at startup.
- **AC6** The key never appears: not in the rendered configuration, not in a log line, not
  in an error — pinned by the existing secret scan plus a direct assertion on a failure
  path, since that is where a key is most often printed.
- **AC7** The query is bounded: an over-long query is refused with a message naming the
  limit, and no truncated query is sent — pinned.
- **AC8** The documents move: the record of processing carries the search provider as a
  recipient with what it receives, and the impact assessment carries the transfer —
  checked, and pinned by the documentation suite the repository already runs.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-search`, branch
  `unit/web-search`). Sites: a new tool module beside `core/src/tools/wiki.rs`, its
  admission in `core/src/tools/mod.rs:91`, the configuration and secret wiring in
  `crates/assistant/src/config.rs` and `main.rs` (follow how the model key is resolved),
  the sourcing teaching in `core/src/teaching.rs`, and the two privacy documents.
- Read `core/src/tools/wiki.rs` end to end first. It is the closest existing tool: an
  external HTTP lookup with a timeout, a bounded result, a cache, and a description written
  to teach the model when to call it. Match its shape rather than inventing a second one.
- The operator's full design for search and fetch is kept outside the repository at
  `~/.local/state/halogenos-assistant/search-design-reference.md`. Read it for the parts
  this unit implements — the envelope, the source-hint table, the refusal discipline — and
  ignore the parts belonging to fetch.
- The vendor's endpoint and response shape must be verified against its current
  documentation before they are written down. A sibling project's adapter is a starting
  point, not a citation.
