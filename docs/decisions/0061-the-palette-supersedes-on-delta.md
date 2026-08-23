# 0061 — The palette supersedes on delta, and existing conversations gain the new tools

Date: 2026-08-23. Pulled forward from the mid-session palette plan: unit 5's
no-pre-existing-store assumption expired when unit 7 deployed.

## Context

The palette block is written once at a conversation's creation, and the
admission wrapper already reads the NEWEST stored palette. A deployed store
now predates this unit's tools, so without a supersession mechanism every
live conversation would admit yesterday's set forever.

## Decision

Ingestion and observation, under the stamp lock, compare the newest stored
palette against the registered tool set on each conversation's first
activity per process, and append a fresh palette block on delta — the same
on-delta shape as the context note, one write per real change. The delta
append lands ahead of the activity's own message, so the very turn that
activity summons admits the current set. Conversations created before this
unit therefore admit the wiki and report tools on their next activity; a
future palette change reaches live conversations the same way — removal
included: a report handle taken out of the configuration removes the report
tool from every conversation's palette on its next activity. A stored
palette that never parsed reads as a delta, because it admits nothing.

The superseding palette is appended at an arbitrary point in a
conversation's history, so the palette kind becomes frontier-transparent
and joins the owing-tail walk's read-through set beside the context note
and the report: a palette appended over an unanswered message buries no
debt. The once-per-process memory is bounded by a named cap and cleared
whole at it, the established memory-cap shape, and it is marked only after
the append stands, so a transient failure retries on the redelivered
activity.

The palette block stays inert and invisible; nothing summons the model
about it. The model-visible palette-delta note is the next unit's concern,
recorded there.

## Rejected alternatives

- **A migration backfill.** A one-shot fix that leaves the next change
  stranded again.
- **Per-conversation registration state.** The ledger's newest palette IS
  that state.
