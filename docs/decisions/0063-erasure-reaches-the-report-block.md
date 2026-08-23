# 0063 — Erasure reaches the report block

Date: 2026-08-23. This narrows the 0045 lineage instead of joining it: the
report block is a consumer kind in a consumer table, so the framework seam
0045 waits on is not needed here.

## Context

The report block exists to carry an identifier — the reported message's
platform origin — and decision 0003's rule is that no stored identifier may
sit out of erasure's reach.

## Decision

The report block stores the reported message's principal id precisely so
erasure can reach it: the reported person's erasure nulls the block's
target origin, keyed by that principal, and the line goes undeliverable —
the edge skips a targetless report. The reporter's erasure nulls the
reply-target column on their own message rows through the existing
author-keyed pass. The tool's append holds the erasure fence, so a report
cannot re-materialize an origin an erasure just nulled — and a fresh ask
replying to an erased message resolves no recorded principal and is
refused, for the same reason. The block's line text stays: it names nobody.

## Rejected alternatives

- **The OPEN-set shrug.** The block exists to carry an identifier; shipping
  it unreachable would be the exact gap decision 0003 exists to prevent.

## Refinement: the reply-target column is reached from both ends, 2026-08-23

The first review round found the half this record left unstated: the
reply-target column stores the replied-to person's message identifier on the
REPLIER's row, so the author-keyed pass alone nulls the erased person's own
rows while leaving a verbatim copy on every row that replied to them — the
same unreachable-identifier shape the decision above refuses for the report
block. Erasure therefore gains a target-keyed pass over the chat-message
table: every reply-target naming a message of the erased principal, matched
through the origin column within the same conversation (platform message ids
are unique only per channel), is nulled. The pass runs before the
author-keyed pass, because it joins on the very origins that pass nulls.
Rejected: recording the residual as an OPEN item — the same shrug the
decision above already rejects.

## Refinement: the retry window keeps one residual copy, 2026-08-23

The second review round qualified the refinement above. The target-keyed
pass finds a replier's row by matching its stored reply-target against the
erased person's own message references — the very references the
author-keyed pass empties next. A completed erasure leaves nothing behind;
a failed one can: when a step after the reference-emptying fails, ingestion
reopens while the identity rows still exist, and a reply recorded in that
window stores the erased person's message identifier with nothing left for
the retried pass to match it against. That copy stays, unlinked inside the
store but stored. The decided follow-up is a reach key that does not depend
on data the operation itself removes: resolve the replied-to sender to a
recorded principal when the reply is recorded and key the pass on that —
the same shape the report record already uses with its reported principal.
That change crosses the message model, the adapters' translation and the
schema, so it ships as its own unit; until it does, this refinement is the
record of the residual. The unit's erasure contract as specified names the
author-keyed step alone — the target-keyed pass of the refinement above is
wider than the specified reach, and aligning the unit specification with
both refinements is part of the same follow-up.

Rejected alternatives:

- **Emptying the person's message references in the same write that
  deletes the identity rows.** Closing the window by atomicity reorders
  and couples the steps decision 0012 records as separately owned — a
  reshape of the whole operation to serve one qualified residual the
  ingestion-time reach key removes at its root.
- **Leaving the refinement above unqualified.** Its reach claim would
  overstate what a retried erasure can do.

## Refinement: the residual is the join's reach, not the retry window, 2026-08-23

A later review widened the refinement above: the retry window is one way a
stored reply target outlives the pass's reach, not the boundary of the
residual. The target-keyed pass matches a replier's stored value against
the erased person's recorded origins, so every reply whose value matches
none of them keeps its copy on the ordinary path too. A reply recorded
after the person's erasure completed resolves to nothing — the identity
rows are gone, and the person's next appearance is a new principal whose
origins do not include the old message. A reply to a message the assistant
never recorded stores an identifier no pass will ever match. Each such
copy sits unlinked inside the store, but stored. The decided follow-up is
unchanged and closes every case at the root: the ingestion-time reach key
of the refinement above resolves the target when the reply is recorded,
and what to store for a target that resolves to nothing is that unit's
explicit decision.

Rejected alternatives:

- **Refusing the reply-target write at ingestion now.** Storing no target
  when it resolves to no recorded principal closes every case at the
  source, but it makes the report tool's no-reply error claim the member
  never replied when they did, drops the stored reply fact for genuine
  replies to unrecorded messages, and ships the follow-up unit's central
  decision piecemeal ahead of that unit.
- **Leaving the second refinement's window wording as the boundary.** It
  would understate what stays stored.
