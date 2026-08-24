# 0101 — Discovery guidance points to the page list

Date: 2026-08-24

## Context

The tool's description and its unknown-page error told the model to learn page
names from the entry page or the sidebar — the two pages that do not carry the
list, the live dead end.

## Decision

The description and the unknown-page error now name the enumeration as the way
to discover pages. A model that does not know a page's name lists the pages,
finds the one it needs, and fetches it, the way a person scans a wiki's index.
The grounded-answer teaching is unchanged; this only makes the source reachable.

## Rejected alternatives

- **Leaving the sidebar and entry-page guidance.** They name discovery pages
  that do not list content — the dead end the live test walked into.
