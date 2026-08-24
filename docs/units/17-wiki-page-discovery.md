# Unit 17 — wiki page discovery

Date: 2026-08-24. Revision 1, from the live test. The wiki lookup can fetch one
page by its exact name, and tells the model to learn page names from the wiki's
entry page or its sidebar. But this wiki's sidebar is a hand-written navigation
menu — Donate, Website, Changelog, Building, Code Review, Contact, Code of
Conduct — and its Home page is a one-line welcome; neither lists the wiki's
actual content pages. So a real page the model needs is unreachable: asked how to
use a documented feature, the assistant fetched Home and the sidebar, guessed a
page name that did not exist, and concluded the wiki had nothing — while the page
existed the whole time, simply unlinked from the two discovery pages the tool
sends the model to. The tool can fetch any page but cannot enumerate the pages,
and the grounded-answer discipline (unit 16) makes that gap load-bearing: with no
way to find the page, an answer the wiki holds becomes an honest "I don't know".

This unit gives the tool the missing capability: the full list of the wiki's
pages, so the model can find any documented page by name.

## Grounding

The enumeration source was chosen by testing, not assumed. For this wiki backend
(a forge wiki served as a git repository), the single-request options were
checked directly: the raw-content host serves individual pages but no index; the
forge's contents API and its archive/tarball service both return not-found for a
wiki (a wiki is not the repository's own tree); only a full git fetch or the
rendered wiki index list every page. The rendered wiki index — the wiki's own
landing HTML — was confirmed to contain a link to every content page, the
unlisted feature page included, in one unauthenticated GET. That is the source
this unit reads: one bounded request, the same shape as a page fetch.

## Decisions taken with this unit

- **The wiki lookup gains page enumeration, read from the rendered wiki index,
  2026-08-24.** A new capability lists the wiki's content pages: one bounded GET
  of the wiki's rendered index, from which the page names are extracted by the
  forge's own stable page-link shape (`…/wiki/<PageName>`), dropping the
  service's reserved pages (the ones whose names begin with an underscore, and
  the history/edit variants) and de-duplicating. The result is the sorted list
  of page names in the tool's page-name shape (title with spaces as dashes,
  parentheses literal) — exactly what the fetch capability takes. It is a bounded
  request with the established bounded-GET contract, its output capped with a
  truncation marker, and cached under the same per-process TTL cache as page
  fetches, keyed by its own request address. Rejected: a full git clone or fetch
  of the wiki repository (heavy, a new process/dependency surface, far more than
  a discovery list needs); the forge contents API or archive service (both
  return not-found for a wiki backend — verified); trusting the hand-written
  sidebar or the entry page as the index (the exact failure this unit exists to
  fix — they carry navigation, not the page list).
- **The enumeration source is a second configured base, defaulting to the
  forge's own host, 2026-08-24.** Page CONTENT is read from the raw host (a page
  is plain text); the page LIST is read from the rendered index on the forge
  host. These are two hosts, so the lookup gains a second base address for the
  index, defaulting to the real forge host, overridable the way the raw base is —
  so a test points it at a loopback server serving a captured index, and a
  deployment never hard-codes a host. No page-list is ever baked into code or
  configuration; the names come from the wiki itself, now from the index that
  actually lists them. Rejected: deriving the index host from the raw host by
  string surgery (brittle and undiscoverable); a single base for both (the raw
  host does not serve the index).
- **Discovery guidance now points to the page list, 2026-08-24.** The tool's
  description and its unknown-page error stop sending the model to the entry page
  or the sidebar to learn names — those do not carry the list — and instead name
  the enumeration capability as the way to discover pages. So a model that does
  not know a page's name lists the pages, finds the one it needs, and fetches it,
  the way a person scans a wiki's index. The teaching that the tool is the only
  source of substantive claims (unit 16) is unchanged; this only makes the
  source actually reachable. Rejected: leaving the sidebar/entry-page guidance
  (it names discovery pages that do not list content — the live dead end).

## The unit's contract

The wiki lookup gains a page-enumeration capability (a distinct operation the
model can call with no page name) that fetches the rendered wiki index over one
bounded GET from a new, configurable index base (defaulting to the real forge
host), extracts the content page names by the forge's stable page-link shape,
drops reserved/underscore/history pages, de-duplicates, sorts, bounds the output
with the truncation marker, and caches the answer under the existing TTL cache.
The tool description and the unknown-page error are rewritten to send the model
to the enumeration for discovery instead of the entry page or sidebar. The
page-fetch capability, its raw base, the page-name predicate, the result bound,
and the cache are otherwise unchanged. The tool stays member authority,
palette-governed. No new dependency; the fetch uses the existing bounded-GET
client.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** Enumeration end to end against a scripted index server: the tool's
  list capability GETs the index once, returns the full set of content page
  names in the page-name shape — including a page that the hand-written sidebar
  and the entry page do NOT link (the live gap) — with reserved/underscore and
  history/edit links dropped and duplicates removed, sorted — pinned against a
  captured real index fixture.
- **AC3** The extracted names are exactly what the fetch takes: a name returned
  by the list passes the page-name predicate and, fed back to the fetch
  capability against a scripted page server, resolves — pinned (list-then-fetch
  round trip, no name mangling).
- **AC4** Discovery guidance: the tool description and the unknown-page error
  name the enumeration as the way to find page names and no longer send the
  model to the entry page or the sidebar — pinned verbatim on the shipped copy.
- **AC5** The bounded-GET contract holds: the index request is bounded, its
  result capped with the truncation marker when over the bound, a transport
  failure is surfaced and not cached, a missing index is a clean tool error, and
  a served index is cached under the TTL like a page — pinned, including the
  cache behavior under paused time in the established shape.
- **AC6** The index base is configurable and defaults to the real forge host;
  the raw base for page content is unchanged; a test drives both against
  loopback servers with no real host reached — pinned.

## Notes for launch

- Branches from main (units 15, 16 merged, HEAD 55e4437). The tool is
  crates/core/src/tools/wiki.rs; the base addresses live in
  crates/core/src/tools/mod.rs (the Bases struct, which currently carries the
  wiki raw base).
- The enumeration parses the rendered index HTML by the stable page-link shape
  `…/wiki/<PageName>`; this is a documented tolerance of the forge's markup, not
  a full HTML parse — extract the links, do not interpret the page. A captured
  real index is the fixture so the pin rides real markup, not an invented one.
