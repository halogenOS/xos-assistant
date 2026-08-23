# 0065 — The speaker is a column on the message row, written at receipt

Date: 2026-08-23. This closes decision 0056's implementation debt: the
username-projection unit makes the recorded decision true, and the published
policy stops over-describing the transmission.

## Context

Decision 0056 decided that the public username travels with the message to
the model provider, and recorded that the projection change follows as its
own unit. The projection reads one block with no ledger access, so whatever
the model is to see of the sender must live on the message row itself.

## Decision

The chat-message row gains a nullable speaker column, its own appended
migration step under the frozen-list discipline (the step quotes no enum,
so it freezes no list): the sender's public username as the platform
delivered it at receipt — the handle as it was when the person spoke, which
is the historically honest value. The identity tables keep owning who is
who; the column is a projection fact, not an identity fact. It is personal
data, so the author-keyed erasure pass nulls it beside the text and the
origin, and deletion keeps its promise. Every pre-migration row reads NULL
and projects bare.

## Rejected alternatives

- **Projecting through a ledger join.** The projection trait reads one
  block; a context-bearing projection is a framework change for one column.
- **Reading the identity table's CURRENT handle.** A renamed person would
  be retroactively re-labeled through their whole history — and the
  projection would need the join anyway.

Noted 2026-08-23, at the unit's close: the column promise is scoped and kept —
the pass nulls the speaker beside the text. What deletion still does not reach
is the assistant's own answer blocks, which can quote an erased person's words
and now repeat their handle; that gap predates this unit, is widened by one
field here, and is recorded in the impact assessment's deletion risk.
