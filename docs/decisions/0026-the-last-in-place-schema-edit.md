# 0026 — The last in-place schema edit

Date: 2026-08-22

## Context

The addressing seam adds two columns to the message kind's content table, and this
unit ships the first deployable process.

## Decision

No durable store predates this unit's binary, so the shipped CREATE TABLE gains the
two addressing columns by direct edit one final time. From this unit on, every
schema change is an appended, versioned migration step.

## Rejected alternatives

- **Starting the append-only migration discipline one unit early.** A ceremony for
  stores that cannot exist.
