# 0113 — The envelope promises only what the vendor can keep

Date: 2026-08-27; the source-hint table inlined 2026-08-29, with unit 27.

## Context

A search result page is the one place a tool in this repository could invent facts
without noticing: a total-results count, a has-more flag, a promise of ten results.
The vendor was probed live before this was written. It answers no total-results
field anywhere, its own samples return eight rows for a ten-row request, and a row
can arrive without a snippet.

## Decision

The envelope states the query as sent, the page number, the count returned, and each
returned row: its title, its link, its snippet where the row carries one, and a
source hint computed from the host and nothing else. It is prose, like every shipped
tool's result — the repository has never chosen a serialization for a tool result and
this unit does not decide one as a side effect. Titles and snippets are bounded
through the shared truncation; a link is never cut, because a truncated link is a
broken one.

There is no total and no more-pages flag. Ten results are requested; whatever arrives
is rendered. An empty first page renders "no results"; an empty LATER page renders
that the results ended at the previous page — the two are told apart by the page
number the model itself supplied. A later page whose rows repeat the previous page's
reads as exhausted too, because a vendor past its real limit may repeat the last page
instead of sending an empty one.

The request sends `autocorrect: false`, because results answering a silently
corrected query would break this unit's own rule that what is sent — and what is
answered — is the query as written. The locale comes from configuration, with the
language defaulting to English and the country sent only where a deployment chose
one.

The source-hint table, inlined here so it lives where a reviewer can reach it: a
wikipedia host reads `encyclopedia`; a host carrying a government or education label
reads `official`; a known blog host reads `blog`; any other host reads `website`; a
row without a host reads `unknown`.

## Rejected alternatives

- **`has_more`, computed as "the page came back full".** A guess wearing a field
  name. The vendor cannot answer it and a stub would fake it convincingly.
- **A `total` field.** The first revision of this unit promised one; the vendor
  sends none, so it would have passed every pin and lied in production.
- **Pinning "ten results".** The vendor's own samples return eight.
- **A JSON tool result.** A serialization decided as a side effect of a search unit.
- **A curated authority list behind the source hint.** The hint is a shape cue for
  the model, not a judgment about which source to believe.
- **The vendor's locale defaults.** US-English answers for a community that is
  neither, and corrected queries presented as uncorrected.
