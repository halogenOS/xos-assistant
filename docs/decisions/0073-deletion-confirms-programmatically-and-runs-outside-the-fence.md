# 0073 — Deletion confirms programmatically and runs outside the fence

Date: 2026-08-23

## Context

A person deletes their data through two commands: the ask and the confirm.
The ingestion path that answers the confirm holds the erasure fence for
reading, while the erasure takes the same fence for writing — running the
erasure inline from the confirm would deadlock on the caller's own hold,
the exact defect the unit's first revision shipped and its unbriefed probe
caught.

## Decision

`/privacydelete` answers the confirm instruction and files the pending
confirmation, keyed BY PRINCIPAL — a deletion asked in one chat confirms in
any, since the person is the subject, not the room. The memory is
process-held with a named window constant (`CONFIRM_WINDOW`, five minutes),
a named cap swept like every peer structure, and forgotten on restart —
deletion is the flow where forgetting errs safe.

`/confirmdelete` inside the window consumes the pending state, answers the
fixed started line, and SPAWNS the erasure as its own task after the
ingestion returns: the spawned run takes the fence for writing once the
ingestion's read hold releases. The started line promises what the
mechanism delivers — the deletion is underway, not instantaneously done; a
failure is logged and leaves the data standing, and re-asking works — the
copy never claims completion the spawn cannot see.

With nothing pending, or past the window, the fixed nothing-pending line
answers — one line covers both, because a lapsed pending IS nothing
pending. A second confirm after a completed run answers the same line. The
receipt a confirm returns names no erased rows.

## Rejected alternatives

- **Erasure inline in the confirm's ingestion.** Self-deadlock on the
  erasure fence, found by the spec's unbriefed probe of revision one.
- **A completion callback into the chat.** A new outbound path for one
  line, when the deterministic return already answered the person.
- **A durable pending store.** A restart would re-arm a half-asked
  deletion; forgetting is the safe direction for a destructive flow.
