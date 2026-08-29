# 0110 — The web search searches, and opens nothing

Date: 2026-08-25, with unit 27.

## Context

The assistant answers from the project's own sources and from what the model knows;
asked anything else it says it cannot check. Giving it the web is one capability in
name and two in fact: searching, which returns titles, links and snippets, and
fetching, which opens a page and reads it. Fetching brings its own apparatus —
robots handling, an origin-refusal memory, consent rules, a content bound over
arbitrary markup — and none of it is needed to answer "what does this error code
mean".

## Decision

This unit ships the search alone. The tool returns a page of ranked results, each
with its title, link, snippet where the result has one, and a source hint derived
from the host; it opens no page, follows no link and reads no document. Fetching is
a unit of its own, with its own assessment.

That boundary is what keeps this unit small enough to be safe: the widest thing it
can do with a member's words is send them to one search vendor, and the widest thing
it can bring back is a paragraph the vendor already published.

## Rejected alternatives

- **Shipping search and fetch together.** The fetch apparatus rides in on a search
  box: one unit, two risk surfaces, and the second one is where the robots rules,
  the consent questions and the arbitrary-content handling live.
- **A "browse" tool that decides for itself whether to open a page.** The same thing
  with the boundary hidden inside a model's judgment.
