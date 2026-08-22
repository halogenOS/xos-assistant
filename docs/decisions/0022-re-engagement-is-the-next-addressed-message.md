# 0022 — Re-engagement is the next addressed message

Date: 2026-08-22

## Context

A stream error latches the conversation — the framework's rule — and nothing in the
earlier units ever unlatched an error-latched conversation. The unit-1 stopgap kept
a per-process set of already-unlatched conversations, with its own bookkeeping
against conversation-id reuse.

## Decision

The recovery surface is the ingestion path itself: an addressed message always emits
the unlatch intent — a person addressing the assistant IS the deliberate
re-engagement — and an unaddressed message never does. This also retires the
per-process unlatched set entirely, and with it the id-reuse bookkeeping: the intent
is idempotent, so emitting it on every addressed write is one decision in one place.
Against an empty wallet each addressed message costs one refused pre-stream attempt,
which spends nothing.

## Rejected alternatives

- **An operator unpause surface.** Right for the later toolset unit, wrong as the
  only recovery.
- **Unlatch at restart only.** Turns every transient provider error into an outage
  until someone restarts the process.
