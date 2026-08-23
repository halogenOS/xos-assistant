# 0074 — Erasure keeps the stub when the flag stands

Date: 2026-08-23

## Context

Decision 0071's suppression flag lives on the identity row, and decision
0012's erasure deletes identity rows. Deleting a flagged person's row would
delete the objection with the data: their next message would create a fresh
principal and collection would silently resume — the failure the stub
exists to prevent.

## Decision

The erasure operation gains one conditional: an identity row whose opt-out
flag stands is EMPTIED — the display name to the empty string under the
schema's non-null contract, the username to its typed absence — instead of
deleted, so the flag survives its own person's deletion. For an unflagged
person nothing changes.

The operation's documented idempotency changes with it, recorded as a
dated refinement on decision 0012: for a flagged person, a repeat erasure
re-runs over emptiness and reports completion rather than not-found —
honest, harmless, stated.

## Rejected alternatives

- **The flag in its own table keyed by external id.** A second identity
  surface to keep consistent, when the row already exists precisely
  because the stub must.
