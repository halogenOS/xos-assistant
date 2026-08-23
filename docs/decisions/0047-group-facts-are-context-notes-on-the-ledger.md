# 0047 — Group facts are context notes on the ledger

Date: 2026-08-23

## Context

The assistant meets its first real group knowing nothing about it: no name, no
rules. The group's title and rules are observed facts read from the group
itself, not configuration, and they must reach a live conversation when they
change.

## Decision

A new consumer block kind carries them — the context note: a topic and a
text, agency-inert, frontier-transparent, projected to the model in the
system voice, appended only when the observed text differs from the newest
stored note of the same topic. The system-voice projection follows the
framework's date marker; providers join system lines, so a note never erases
the system prompt. Because a note is appended by an independent path at an
arbitrary moment, the kind answers the framework's frontier-transparency
hook and the entry point's own owing-tail read walks past notes the same
way, so a note on top of an unanswered message leaves the turn owed and the
debt propagation intact. Notes accumulate in stream order and the projection
wording makes the newest authoritative; a framework superseding-block is the
future compaction, already on the improvements list, not a blocker.

## Rejected alternatives

- **Rendering rules into the system prompt block.** The prompt is written
  once at creation; an edit never reaches a live conversation.
- **A mutable rules row.** Blocks are the only content unit and storage is
  append-only.
- **Deferring the append until no debt is open.** A queue and a second
  delivery problem, when transparency answers the ordering outright.
