# 0031 — The budget clock is the receipt time

Date: 2026-08-22

## Context

A budget window needs one clock. Every recorded message carries two times: the
platform's send time on the content row, and the store's insertion time on the
block header.

## Decision

The window counts against the block header's creation time — assigned by the store
at the write, unforgeable, never null, uniform across adapters — anchored at the
stamp's own wall clock. With receipt time, a backlog replayed after downtime meets
the budgets like live traffic instead of arriving pre-aged.

## Rejected alternatives

- **The platform send time.** Platform-asserted: unforgeable where the platform's
  server assigns it, but not guaranteed unforgeable on every future adapter — a
  federated platform's origin timestamps are peer-asserted. It is also nullable
  under erasure, and a backlog replayed after downtime carries old send times that
  would open the window exactly when the queue is longest.
- **A hybrid of both.** Two clocks is two answers to one question.
