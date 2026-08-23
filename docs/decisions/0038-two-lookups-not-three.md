# 0038 — Two lookups, not three; the wiki tool waits for a wiki

Date: 2026-08-22

## Context

The feature list named three lookups: commits, releases, and the project docs.
The project site turned out to be a static single-page application: no wiki, no
server-side search, nothing a bounded GET can query.

## Decision

This unit ships two lookups — commits and releases — and drops the third rather
than pointing it at a backend that does not exist. When the project stands up a
searchable docs backend, a wiki tool follows as its own small unit.

## Rejected alternatives

- **Scraping the site bundle.** Client-side assets, hash-named, no stable
  contract; the tool would break on every site deploy.
- **Fetch-and-search over a fixed URL list.** A hand-kept index that drifts,
  answering worse than the two real lookups.

---

Amended 2026-08-23: the wait ends with decision 0058. This record ruled the tool
out while the project SITE had no wiki; the manifest repository's wiki on the
mirror forge is a different backend, and it is real — the wiki lookup ships
against its raw pages.
