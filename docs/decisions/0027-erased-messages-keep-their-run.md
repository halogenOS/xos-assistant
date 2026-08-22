# 0027 — Erased messages keep their run

Date: 2026-08-22

## Context

Decision 0012 left OPEN that an erased message was boundary-invisible in the
projection: it projected nothing but still ended the contiguous run of its
neighbours, so a conversation with an erased message in the middle could project two
same-role messages in a row, and one erased at the front could open with the model's
own voice. Live providers that demand strict alternation reject such requests. The
unit spec preferred a kind-level shape and authorized a minimal framework fold
amendment only if no kind-level shape could express it.

## Decision

Closed at the kind level; the framework's fold needed no amendment. The kind's
projection keeps the stored role in the grouping pass whether the text is erased or
not: an erased message holds its place in its run under its own voice while
contributing a fixed erasure marker in place of its nulled prose, so no two
same-role messages split apart and a request never opens with the assistant's
voice. Where an erased run stands alone between two other-role neighbours — a
position the alternation requires a message in — it projects one marker-only
message. The marker refines decision 0012's projection clause without reversing
it: none of the person's words ever reach the model; what projects is a constant.
It is non-empty on purpose, because the same strict vendors that reject same-role
adjacency reject a message whose content is empty.

The probe of the framework's fold that confirmed the seam: the central grouping pass
owns only structure — contiguous-role grouping — and asks each block for its
grouping role and its contribution, so a kind answering with its stored role
preserves run continuity on its own. Pinned by tests on the projected request over
the two erased shapes, deterministically, model-independently and network-free.

This closes the first OPEN item of decision 0012.

## Rejected alternatives

- **A framework fold amendment.** Authorized as the fallback, not needed: the
  grouping seam already lets the kind express the shape.
- **A live probe against real models.** Wire acceptance is a per-model fact and
  cannot evidence a configurable binding; the unit spec dropped this branch.
- **An empty separator message where a whole run is erased.** Keeps 0012's
  "projects nothing" wording to the letter but trades the alternation defect for
  an empty-content one on the same strict vendors.
- **Dropping an all-erased group from the projection.** Wherever such a group
  appears after run-continuity grouping, it separates other-role neighbours;
  dropping it recreates the adjacency the closure exists to prevent.
