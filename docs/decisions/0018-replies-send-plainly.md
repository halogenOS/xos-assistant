# 0018 — Replies send plainly

Date: 2026-08-21

## Context

The core's outbound reply carries a channel key and text — no origin reference yet,
though the ledger stores the origin column for exactly that future use. The platform
could thread a reply onto the message it answers.

## Decision

The adapter sends the reply's text to the chat, unthreaded. Reply threading is deferred
until the outbound edge carries the origin; wiring a guess now would thread every reply
onto the newest message, which is wrong in a busy group.

## Rejected alternatives

- **Extending the outbound edge in this unit.** A core change the adapter invariant
  says an adapter must never need.
