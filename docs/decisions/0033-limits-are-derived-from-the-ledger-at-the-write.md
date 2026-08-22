# 0033 — Limits are derived from the ledger at the write

Date: 2026-08-22

## Context

Enforcing a budget needs a count of recent answered debt. A count can be stored and
maintained, or derived when it is needed.

## Decision

No counter table, no in-memory tally: the entry point runs two bounded counts
inside the existing stamp serialization — the same serialization that already
orders the answer-due stamp, so two racing messages cannot both take the last
budget slot. The counted predicate is messages that opened debt — addressed, not
limited — younger than the window by receipt time, counted by principal globally
(spend is global, so heavy direct-chat use and group use share one budget) and by
conversation. Propagated stamps are not counted, and a multi-message turn still
counts each opened debt: a debt opened is a spend intent, and the over-count
against absorbed turns is accepted and stated here. The appended migration adds
the index the principal count runs on (principal id, addressed); the channel
count rides the framework's existing junction index. The stamp serialization is
in-process; single-process deployment is already the assembly's stated
assumption.

## Rejected alternatives

- **A counter table.** A second record of a derivable fact, drifting on every
  erasure.
- **In-memory counters.** Reset on restart, and unreadable in audits.
