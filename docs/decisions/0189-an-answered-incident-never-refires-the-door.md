# 0189 — A forced turn end is answered once, inside the thread it happened in

Date: 2026-08-31, with unit 48. Amends decision 0165.

## Context

The unattended door is level-read from durable state: a status row carrying the framework's
forced-end key makes a mapped conversation eligible. That was self-consuming under unit 45,
because the tail keep never carried the marker across — the fork could not re-fire.

This mechanism carries the second half of the ledger forward verbatim, and the marker sits
at the end of the turn it ended, which is usually inside that half. An unscoped read would
find it in the successor thread, compact that, find it again in ITS successor, and burn a
model turn per round until the ledger ran out.

## Decision

The eligibility reads a status recorded inside THIS thread's own life: newer than the
thread's ancestor-reference block, which is the first thing a compaction writes. Block ids
ascend with insertion, so every inherited block is older than that reference and every
later incident is newer. A thread carrying no reference has never been compacted, and its
whole ledger is its own life.

The other half of the self-limit is unchanged: a compacted source is unmapped from the
moment its successor claims the channel, so however many late appends wake the driver, it
is never compacted again.

## Rejected alternatives

- **Cutting the marker out of the carried half.** The second half rides across VERBATIM;
  editing it to suit one reader is exactly the shape this mechanism replaced.
- **Remembering answered incidents in process memory.** A restart would forget them and
  re-fire on every one, which is the failure the level read exists to avoid.
- **A cooldown between unattended compactions.** One was declined for unit 45, and a bound
  nobody asked for is a decision that is not this unit's to make.
