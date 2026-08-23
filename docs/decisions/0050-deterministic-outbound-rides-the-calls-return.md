# 0050 — Deterministic outbound rides the call's return, not the event edge

Date: 2026-08-23

## Context

The group-context unit sends three deterministic items: the rules
acknowledgment, the privacy answer, and the withdraw directive. The
existing outbound path is the event edge — the model's answers and the
failure notice — with at-least-once delivery from stored state.

## Decision

Message ingestion is already a direct call whose result the adapter driver
classifies; the observation surface follows the same shape, and everything
deterministic this unit sends is returned from that call as a value the
driver translates — a text to send, a leave to perform. The event edge
stays exactly what it is. No consumer event type, no bus rework, no
mapping-row dependency for the withdraw; redelivery semantics come out
right by construction: a replayed update re-returns an idempotent
directive, and the on-delta rule keeps a replayed acknowledgment from
repeating.

## Rejected alternatives

- **A consumer event composed over the framework bus.** Reshapes the event
  type across the assembly, the streams, the tool bounds and the edge, to
  deliver three deterministic lines.
- **Emitting the acknowledgment from the stored note via the edge cursor.**
  At-least-once against a courtesy line's at-most-once intent.
