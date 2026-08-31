# 0177 — an edit that leaves no text records nothing, and the words already recorded stay

Date: 2026-08-31, with the editing unit.

## Context

A member who deletes a message's caption produces an update with neither
text nor caption, which translation already reports as the textless skip.

## Decision

It stays a skip. The alternative would be a recorded message with empty
text, and the whole erased-marker reading rests on absent text meaning
erasure and on nothing else ever producing it. A message row that projects
as neither words nor the erasure marker has no honest reading.

The consequence is stated rather than engineered away: the earlier wording
stays in the ledger, and the route to removing it is the one the product
already offers — the person's own deletion command, or an administrator's
reply deletion. It goes into the impact assessment's addendum as a residual.

## Rejected alternatives

- **A recorded empty message.** It breaks the erasure marker's exactness.
- **A fixed retraction text.** Invented words attributed to a person, and
  forgeable.
- **Treating a text-emptying edit as an erasure of the original.** The
  assistant would erase a person's recorded words on an act that is not a
  deletion request, and nothing distinguishes a caption deleted by mistake
  from one deleted on purpose.
