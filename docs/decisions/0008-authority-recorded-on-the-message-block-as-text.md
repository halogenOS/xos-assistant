# 0008 — Authority is recorded on the message block as text

Date: 2026-08-21

## Context

The protection unit will enforce a one-authority turn, but enforcement can only
classify history the ledger already carries: if the fact is not recorded now,
enforcement inherits messages it cannot classify.

## Decision

The message block records the sender's authority level at receipt, in a text column
with the fixed vocabulary `member`, `moderator`, `admin`. The ordering
(`member` < `moderator` < `admin`) lives in code, not in the storage.

## Rejected alternatives

- **Deriving authority at read time from the identity store.** Authority is resolved
  live by the adapter at receipt; re-deriving later reads today's role into yesterday's
  message.
- **An integer encoding.** Opaque in the stored row, and the vocabulary is closed
  anyway, so text costs nothing and reads plainly.
