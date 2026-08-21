# 0006 — Sender identity crosses the boundary; the principal id does not

Date: 2026-08-21

## Context

Decision 0003 keeps personal data out of the ledger: identity lives in its own tables,
blocks carry a principal id. Someone has to turn a platform sender into that principal
id, and the question is on which side of the adapter boundary that happens.

## Decision

The inbound message carries what the identity store needs — the sender's opaque external
id and display fields — and the core's entry point resolves or creates the principal
from them, refreshing the display fields on later messages. Only the principal id enters
the ledger.

## Rejected alternatives

- **The adapter carrying a principal id.** It would need identity-store access, which the
  edge contract forbids: an adapter translates, it does not resolve.
- **A separate registration call before ingestion.** Two calls that must agree, and a
  message from an unseen sender would still need the create-on-first-contact fallback, so
  the second call buys nothing.
