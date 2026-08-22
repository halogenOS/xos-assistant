# 0032 — The counts join the framework's tables, sanctioned and recorded

Date: 2026-08-22

## Context

The budget counts need the conversation id and the receipt time. The kind's content
table carries neither: both live in the framework's own tables — the
conversation-to-block junction and the block header.

## Decision

The budget queries join those two tables by name from the kind's module. Framework
vocabulary is not platform vocabulary: the no-vocabulary invariant binds the core
against platforms, not against the library it consumes. The coupling is real and
carries a recorded risk — the framework does not contract those table names, so a
framework rename breaks the counts at run time, not at compile time. Surfaced to
the framework's improvements list: exported schema-name constants or a counting
read seam, so consumers stop naming internals.

Extended 2026-08-22, at the unit's close: core test support names the framework's
block-header table in its receipt-aging and encoding helpers, and the framework's
migration bookkeeping table in its schema-inspection helpers — suite-side only, for
the same reason and under the same risk — ratified as part of this coupling rather
than separate ones, and gone the same day the framework offers the read seam.

## Rejected alternatives

- **A conversation column on the content table.** A second record of the
  junction's fact, drifting on fork.
- **Loading whole ledgers through the public read path per stamp.** The exact
  full materialization the outbound edge was already cured of.
