# 0174 — the comparison is one store read, defined exactly

Date: 2026-08-31, with the editing unit.

## Context

The check above runs on every edit update the platform delivers, including
the ones nobody asked for.

## Decision

A new read on the message kind — the only place the column names live —
returns the newest recorded version of a named message in one conversation:
the origin column OR the revision column matching, within the conversation
junction, ordered by the ledger's own append order descending, one row. It
carries that version's text, which the incoming one is compared against byte
for byte, and the principal that wrote it, which the reviser is compared
against — one row answers both questions, and a second read would be a
second chance for them to disagree. Append order, so "newest" means the last
version recorded and never a clock a platform supplied.

Matching either column is what makes the read correct on a platform where a
revision carries its own distinct origin; on this one the two coincide,
because an edit arrives under the original's message id, so the id an edit
update names is the key every version of that message stores. On a platform
delivering an id per revision the disjunction reaches every version from the
ORIGINAL's id and one row from a later version's — which is why the adapter
there owes the root-resolution step decision 0171 states, and why this read
needs no chain walk on either platform.

One bounded statement, not a conversation load: reading the whole ledger per
edit would be a different cost class, and the new column is indexed in the
same appended migration step that adds it.

It is a store READ, so it fails closed: the failure propagates, ingestion
refuses, and the adapter's batch discipline retries — the same choice
decisions 0041 and 0052 record for every other admission read. Recording
anyway on a failed read would write a duplicate row and, in helpful mode,
spend a model turn on it.

## Rejected alternatives

- **Reading the whole conversation and scanning it in memory.** A full
  ledger load per link preview.
- **Ordering by the stored send time.** A platform-supplied clock, and two
  edits within one second are then unordered.
- **Matching on the origin alone.** Correct here, a silent no-op on the
  second platform this design exists for.
