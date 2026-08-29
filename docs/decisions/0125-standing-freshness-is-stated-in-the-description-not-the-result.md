# 0125 — Standing freshness is stated in the description, not the result

Date: 2026-08-25, with unit 29.

## Context

The ledger holds the standing each person had when they last spoke. That is what the
answer is as of, and a reader who does not know it could take the answer for a live
reading of the group.

## Decision

The answer is computed from the person's most recent stored message, and the limit is
stated in the tool's description, which the model reads before it chooses to call:
the standing is as of that person's most recent message here, not a live reading. The
two result strings are the operator's and have no room for a clause; a note about a
tool's reach belongs where the tool is chosen anyway.

## Rejected alternatives

- **Naming the message the answer speaks for, inside the result.** Demanded by an
  earlier revision of the unit; the strings are pinned and are not up for paraphrase.
- **Asking the platform for a live answer.** Platform behaviour belongs in the
  adapter, it puts a network round trip inside a turn, and the answer is stale again
  by the time the model reads it.
