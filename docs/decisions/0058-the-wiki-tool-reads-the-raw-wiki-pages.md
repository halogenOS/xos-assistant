# 0058 — The wiki tool reads the project wiki's raw pages

Date: 2026-08-23

## Context

Decision 0038 ruled the wiki tool out while the project SITE had no wiki. The
project's manifest repository wiki on the mirror forge is a different backend,
and it is real: sixteen pages, served raw as plain text at a stable
unauthenticated address with no redirect, under the host's own five-minute
cache header.

## Decision

The lookup takes a page name — the title with spaces as dashes, parentheses
literal — performs one bounded GET against a configured base address
defaulting to the real raw host, and returns the page text decoded lossily as
UTF-8. The model-facing result is bounded by its own named constant with a
truncation marker: a truncated wiki page is a degraded answer, not a changed
meaning, unlike rules. A missing page is a tool error naming the page-name
shape. Page-name validation gets its own predicate — the existing repository
predicate rejects the parentheses real page titles carry.

The raw host publishes no rate-limit contract, so the tool keeps a
per-process response cache: keyed by the full request address, a named TTL
matching the host's own cache header (five minutes), page bodies and
missing-page answers cached alike — negative caching bounds a model guessing
page names — and a named entry cap cleared whole when hit, the established
memory-cap shape. Transport failures are never cached.

**The model learns the page names from the wiki itself.** The tool's
description teaches the name shape and names the entry page; the model starts
at the entry page or the sidebar when it does not know a page, both ordinary
fetches. No page list lives in code or configuration.

## Rejected alternatives

- **Enumerating pages via the wiki's git transport.** A clone for a page
  list — the shape unit 5 already rejected.
- **The page-index scrape.** An HTML page with no stable contract.
- **Content-type enforcement.** The host says plain text today; an HTML body
  on a 200 passes through as text and reads as what it is.
- **Waiting again.** The backend exists.
- **A configured page list.** Drifts.
- **Embedding today's page names in the prompt.** Drifts identically, and the
  prompt is the maintainer's document (decision 0046).
