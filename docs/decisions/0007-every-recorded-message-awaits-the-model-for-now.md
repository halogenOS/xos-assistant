# 0007 — Every recorded message awaits the model in the core spine

Date: 2026-08-21

Replaced 2026-08-22 by decision 0021: the acting policy is now record all, answer
some, with the adapter resolving the addressed fact and the entry point stamping
answer-due at the write.

## Context

The planned acting policy is: record every group message, answer the addressed ones.
Whether a message is addressed to the assistant depends on platform addressing rules
that arrive with the live-model unit. The core spine ships before those rules exist.

## Decision

In the core spine, every recorded message awaits the model — the block kind's agency
hook says so unconditionally. The acting policy is behavior on the block kind and
arrives with the live-model unit, where the addressing rules exist to express it.
Wiring a placeholder policy now would be a second decision site to unwind later.

## Rejected alternatives

- **A stub mention-check in the core.** Platform addressing rules are adapter knowledge;
  a wrong boundary is worse than a late one.
