# 0188 — The compaction's capture runs outside the ingestion holds

Date: 2026-08-31, with unit 48. Amends decision 0165.

## Context

Unit 45's compaction was a read and a fork: cheap enough to run whole under the two holds
an ingestion takes, and unit 0165 recorded it running there. This one drives a model turn.
Holding the single ingestion lock across a model call stalls every conversation this
process serves for that call's whole latency — the reasoning the rules acknowledgment
already recorded when it released both holds before its own bounded generation.

## Decision

A compaction is two phases, and only the second takes the holds.

The CAPTURE — the cut, the temporary conversation, its turn and its answer — runs with
neither hold. The SWAP — opening the thread and re-pointing the channel — takes the erasure
fence shared and the stamp lock, in that order, and re-reads the channel's mapping inside
them. A capture whose channel moved on while the summary was being written stands down and
leaves everything as it found it.

Inside those holds, and before anything is copied, the SOURCE'S OWN STREAM IS SETTLED — the
same interrupt-and-confirm the erasure runs ahead of its deletions. The holds order the
swap against ingestion; they say nothing about a turn already in flight, and a turn left
running lands its answer in the source AFTER the second half was copied, in a conversation
the swap has just unmapped and the outbound edge delivers nothing from. The member's
question rides across into the thread and its answer simply vanishes. The streaming tail
that turn had already written is worse: it rides across as shared junction rows, and the
source's own finalization deletes those blocks by id, cascading the thread's rows away
underneath its born cursor. The settle is what makes the copy a copy of something that has
stopped moving, and cutting the in-flight answer is what the group operator contract already
states a reset does. The identical settle runs ahead of the erasure scrub's serving clone,
for the identical reason.

`/compact` therefore cannot be answered from inside the ingestion that recognized it. The
entry point releases both holds after the message row stands and runs the mechanism then,
so the line it answers still reports what happened rather than what was requested. The
person waits for the summary; the rest of the assistant does not.

The unattended doors are one driver task, and the compactions it runs are awaited inline:
two captures for one conversation would spend two model turns to produce one summary, and
the second would find the channel already moved.

## Rejected alternatives

- **Answering `/compact` immediately and compacting afterwards.** The shipped line says the
  session WAS compacted. Speaking it before the work is a line that is false at the moment
  it is read.
- **Holding the locks across the capture.** Bounded at three minutes, and three minutes of
  stalled ingestion is a stalled assistant.
- **Spawning the command's compaction and forgetting it.** The reply window grants exactly
  with the change, so a spawned change could not hand its grant back on failure.
- **Standing down whenever the source is mid-stream, and retrying later.** Free for the two
  unattended doors, but `/compact` is a person asking for it now, and a busy channel over a
  warm cache is exactly where the threshold door is told to go anyway — a stand-down rule
  would make the mechanism answer differently through different doors, which is the one
  thing "one mechanism, three doors" forbids. The settle stops the turn instead, which is
  the behaviour already written down for a reset.
