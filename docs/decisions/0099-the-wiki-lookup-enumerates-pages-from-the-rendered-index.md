# 0099 — The wiki lookup enumerates pages from the rendered index

Date: 2026-08-24

## Context

The wiki lookup could fetch a page by its exact name but could not list the
wiki's pages, and the pages it sent the model to for discovery — the entry page
and the hand-written sidebar — carry navigation, not the page list. So a real
content page unlinked from those two was unreachable by name, and the
grounded-answer discipline turned that gap into an honest "I don't know" for
content the wiki actually held.

## Decision

A new enumeration capability lists the wiki's content pages: one bounded GET of
the wiki's rendered index, from which the page names are extracted by the
forge's stable page-link shape, the reserved pages (underscore-prefixed names,
the history and edit variants) dropped, then de-duplicated and sorted into the
tool's page-name shape. The extraction is a documented tolerance of the forge's
markup — the stable path shape is matched wherever it appears, the page is never
parsed as HTML. A 200 index the scan finds no page links in is a loud tool
error, not a silent empty list, so a markup shift that broke the scan surfaces
rather than reading as "the wiki has no pages".

## Rejected alternatives

- **A full git clone or fetch of the wiki repository.** Heavy, a new
  process/dependency surface, far more than a discovery list needs.
- **The forge contents API or archive service.** Both return not-found for a
  wiki backend — verified by testing the endpoints.
- **Trusting the hand-written sidebar or the entry page as the index.** The
  exact failure this decision exists to fix: they carry navigation, not the
  page list.
