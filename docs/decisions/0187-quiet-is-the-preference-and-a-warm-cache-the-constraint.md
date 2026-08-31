# 0187 — Quiet is the preference, a warm prompt cache the constraint

Date: 2026-08-31, with unit 48.

## Context

The design asks for the compaction trigger to happen in a quiet window while avoiding an
expired prompt cache, so the compaction's own dispatch is not paid for at full price. The
two pull in opposite directions on a busy channel: quiet may not arrive before the cache
expires.

## Decision

One rule, whichever threshold arm armed the trigger, reading the CURRENT cache state rather
than the state that armed it.

- A quiet moment — no inbound message for the quiet window — is always the moment to go.
- While the cache is WARM and quiet has not come, the cache's edge is the deadline: at a
  named margin before it the compaction goes anyway, so its own dispatch still lands on the
  warm prefix.
- While the cache is already expired and nothing has re-warmed it, there is no warm window
  left to protect, so waiting for quiet costs nothing and the rule waits.

Every dispatch re-warms the cache and restarts the rule from the new edge, so an armed
trigger under continuing traffic never knowingly dispatches full-price into an expired
cache.

The cache's lifetime is an ESTIMATE of an external fact and is named as one. No provider
reports it, and time since the last dispatch is the only reading of expiry this process can
take. The estimate errs toward treating a cache as cold, which costs a compaction that
could have waited and never a full-price dispatch nobody expected.

The rule is a pure function over injected durations, so what it decides is pinned without a
clock anywhere near it.

## Rejected alternatives

- **Two rules, one per arm.** The arms differ in what makes a compaction NEEDED, not in
  when it is cheapest to run one. Two rules would have to be kept in agreement forever.
- **Going immediately on an armed threshold.** The design asks for a quiet window, and a
  summary written in the middle of a conversation interrupts the very turn the members are
  waiting on.
- **Waiting for quiet unconditionally.** A busy channel would then always pay full price
  for the compaction, which is the cost the design's second clause exists to avoid.
