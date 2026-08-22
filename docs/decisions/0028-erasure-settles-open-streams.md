# 0028 — Erasure settles open streams before deleting

Date: 2026-08-22

## Context

Decision 0012 left OPEN that erasure was ordered against ingestion by the assembly's
fence but not against a conversation's open stream: a direct conversation could be
deleted mid-stream, failing loudly on the stream's finalizing write.

## Decision

The assembly tracks per-conversation streaming state from the bus events it already
consumes, backed by a stored-state read that covers the observation's lossy edge.
One window stays open and is accepted (noted 2026-08-22, from the unit's final
review): between the actor dispatching a turn and the provider's first connected
event, nothing marks the conversation as streaming, so an erasure in that window
deletes the conversation under a stream about to write. The failure direction is a
loud error on that stream's write, possibly an orphaned answer block — never silent
retention, because the prose is nulled by principal id before any deletion.
Erasing a principal whose direct conversation shows an open stream emits the
interrupt, awaits the stream's end signal for that conversation — the turn's done,
its error, or the stream's close, the same terminal set the observation closes on,
because an errored turn emits no close at all — then confirms settle with a bounded
re-read: no streaming tail remains and the interrupt's own status append is in,
counted, not id-compared, because deleting the streaming tail frees ids for reuse —
before deleting. A conversation neither observed open nor holding a stored streaming
tail is erased directly and pays no wait. A stored tail with no observed stream — a
crash's residue in a durable store — still gets the interrupt and the stored-state
settle, skipping only the end-signal wait no gone runtime can ever satisfy. The
bound is a named constant; past it the erasure fails loudly and deletes nothing.

Pinned by a test that erases during a held scripted stream, one that proves the
timeout path deletes nothing, one that erases an idle principal without paying any
wait, and one that settles a crash-left tail from stored state.

This closes the second OPEN item of decision 0012.

## Rejected alternatives

- **Deleting under the open stream and accepting the loud store error.** The state
  this record replaces; the person's erasure call fails on a race they cannot see.
- **A ledger-fact streaming flag instead of the bus observation.** The stored
  streaming tail is already that fact; the observation only saves the read, and the
  settle re-read decides from stored state either way.
