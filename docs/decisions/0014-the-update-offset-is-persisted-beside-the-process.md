# 0014 — The update offset is persisted beside the process

Date: 2026-08-21

## Context

`getUpdates` acknowledges by offset: the next poll's offset confirms everything before
it. Where that offset lives decides what a crash does to in-flight updates.

## Decision

The state file holds the next offset to send — the highest acknowledged update id plus
one — at a path the embedder supplies, written after the batch's messages are ingested.
A crash between ingest and write therefore redelivers (and duplicates) rather than
drops. An absent, empty or malformed state file is treated as absent, logged, and the
redelivered updates are the accepted duplicates.

## Rejected alternatives

- **Offset only in memory.** The wire itself confirms on the next poll, so the loss
  window is a crash between ingest and that poll — similar in size, but implicit; the
  file makes the redelivery window explicit and testable.
- **Deduplicating in the core.** The core has no platform vocabulary and no uniqueness
  contract on origin.
