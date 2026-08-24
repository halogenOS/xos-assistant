# 0092 — A message is reported at most once, per origin

Date: 2026-08-24

## Context

A message that dies unanswered re-co-summons the next turn through the
marker-aware walk, so without a bound the same violation could be re-assessed
and re-reported turn after turn. The member-initiated report carried a
per-channel five-minute window against a different threat — a member asking
repeatedly.

## Decision

Per-origin dedup replaces the report window on the report path: the report
block already stores its target origin, and the filing scans the
conversation's ledger for an existing report of the named origin, declining
a duplicate with its own pinned no-retry copy. Each violating message is
reported exactly once, however many turns re-assess it — the
die-after-filing re-summon path included — and a busy hour of DISTINCT
genuine violations is never throttled, which the channel-wide window would
have done: suppressing separate members' violations is the exact harm to
avoid. A transiently failed append filed nothing, so the dedup finds
nothing and a later assessment files cleanly. The privacy notice's own
window is untouched.

## Rejected alternatives

- **The per-channel time window.** Suppresses distinct genuine reports in a
  bad hour — the moment the capability matters most is the moment it would
  go quiet.
- **No dedup.** Double-reports on the die-after-filing re-summon path the
  probe found; every re-assessment would ping the administrators again.
- **A process-memory dedup.** The ledger already stores the fact; reading it
  survives restarts and costs nothing the filing does not already load.
