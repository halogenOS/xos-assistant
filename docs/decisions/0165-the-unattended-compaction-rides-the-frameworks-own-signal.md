# 0165 — The unattended compaction rides the framework's own signal, level-read

Date: 2026-08-30, with unit 45.

## Context

When a run of tool-call window refusals ends a turn between rounds, the framework records a
status row with its own machine key. That is the exact case a compaction exists for: the
model kept calling into a spent window, the history it is reading has gone bad, and every
further round buys another refusal. The framework's own launch notes assign the consumer
half of that hook to this unit.

## Decision

One operation, two triggers: the command, and the signal.

A watcher task spawned beside the stream observer, on the same broadcast, folds a
conversation's stored status rows on every block change. When the fold finds the forced-end
key AND the conversation is currently mapped to a channel, the same compaction the command
runs, runs on it. It answers nothing in chat — nobody invoked anything, and an unasked-for
line in a group is noise — and records itself in the log.

It fires wherever a mapped conversation exhausts its tool-call window, DIRECT chats included,
while the two commands are fenced to groups (decision 0161). The two conditions are about
different things and are deliberately not one: the commands' fence is about authority, and a
moderator floor states nothing in a room that has no moderators, while this healing is about
a conversation whose history has gone bad — which happens in any room the assistant is
mapped into, with nobody there who could ask for the repair.

The trigger is LEVEL-read from durable state, never from the event: the bus is deliberately
lossy, so a dropped or lagged event costs nothing and the next change on that conversation
reads the same standing fact. Edge-triggering on the event alone would drop exactly the
incident the watcher exists for.

It is self-consuming from both sides. The marker is never in the kept set, so the fork
carries none and cannot re-fire. And the mapped-only condition makes the swept SOURCE
ineligible from the moment its fork claims the channel, however many late appends wake its
fold. An unmapped conversation is never compacted unattended.

The whole operation — checking and acting, on both triggers — runs under one hold: the global
ingestion lock the ingest path already takes, plus the erasure fence shared, in that order.
The command path is already inside an ingestion and holds both. The unattended path takes
them itself and re-reads the marker and the mapping INSIDE them, so a wake that lost the race
finds its conversation unmapped and stands down. A cheap read outside the holds decides
whether to queue at all: the wake arrives on every block change in every conversation, and
taking the ingestion lock that often would stall the chat.

The unattended path carries no repetition bound, by the operator's ruling (2026-08-30): a
channel that keeps landing forced turn ends is compacted again on the next standing trigger.
Wakes never overlap — the watcher is one task awaiting each run — and the eligibility
re-read keeps a swept source from ever firing twice; what remains unbounded is repetition
across the fork chain, and the operator declined a cooldown on it. The command path stays
bounded by its own per-person reply window.

The re-claim takes the winner check the first-contact path already owns. A claim lost to a
concurrent racer deletes the just-created fork — junction rows alone, every block living on
in the source — logs at warn, and leaves the winner's state governing, so no fork mapped
nowhere is left owing turns nobody can deliver. A claim-lost COMMAND made nothing of its own,
so it answers SILENCE and fires no reset directive — on both commands. The session the
channel ends up with is the racer's doing, and a done line would report a replacement this
command did not make, while a directive would have the adapter forget its lookups for a
session that never arrived.

No write path resolves a mapping before the stamp lock: the ingest, like the observe path,
reads the mapping inside the lock, so a swap and a queued message serialize and the message
lands in the surviving session — pinned by the racing test.

## Rejected alternatives

- **A second, differently shaped automatic compaction.** One decision, recorded once; two
  shapes would drift the moment either was tuned.
- **An in-flight set keyed by conversation.** It would guard against concurrent wakes on
  one conversation, and there are none: the watcher awaits each run before reading the next
  event, so the set could never refuse an entry. Machinery for a concurrency that cannot
  happen reads as a bound while bounding nothing.
- **A per-channel cooldown on the unattended path.** Proposed as the repetition bound and
  ruled out by the operator (2026-08-30): the healing runs whenever its trigger stands.
- **Edge-triggering on the bus event alone.** The channel is lossy by design, and the event
  it would drop is the incident this exists for.
- **Answering the group when it fires.** Nobody asked, and a line explaining a maintenance
  action to a room mid-conversation is noise.
