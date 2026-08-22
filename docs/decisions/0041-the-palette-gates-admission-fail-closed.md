# 0041 — The palette is a consumer block kind, and it gates admission, not exposure

Date: 2026-08-22 (recording the 2026-08-20 fail-closed settlement against the
framework as probed)

## Context

The framework has no palette surface: tool definitions go to the model
registry-wide on every turn, and nothing per-conversation filters them. Which
tools a conversation admits therefore has to be the assistant's own recorded
fact.

## Decision

The palette is the assistant's own leaf kind: one durable block naming the
admitted tools, written at every conversation's creation — direct and group
alike — beside the system prompt, under the same winner-only rule the creation
race already has. It projects nothing to the model and awaits nothing. No
palette block means no tools, and so does a palette whose stored list does not
parse: fail closed, because a public group is a different threat model from an
operator session. What the palette cannot do today is stated plainly: the model
may still be OFFERED a tool the palette will decline, so the decline wording
teaches the model not to retry; the per-conversation definitions filter joins
the framework improvements list. Conversations created before this unit have no
palette and admit nothing; no backfill — no production store predates
deployment, so the case is a test fixture, not an operational path.

## Rejected alternatives

- **Fail-open with run-level checks.** The source system's model, correct for
  operator-facing sessions where run-level checks are the real enforcement; a
  public group has neither the operator nor the checks.
- **A registry-side filter in the assistant.** The registry is framework
  machinery; the fact belongs on the ledger, where a replay can read it.
