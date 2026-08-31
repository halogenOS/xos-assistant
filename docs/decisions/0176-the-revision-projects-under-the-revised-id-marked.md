# 0176 — the revision projects under the revised id, marked, and the earlier version keeps its line

Date: 2026-08-31, with the editing unit.

## Context

The model has to be able to tell which version it is reading, and the report
tool validates the bracketed id the projection shows.

## Decision

A revision projects as the revised message's bracketed id, the speaker
prefix, then a fixed edited marker and the text. The bracketed id stays
exactly the shape the report teaching names and the report tool validates,
and the fixed word sits at the head of the text where it reads as what the
room sees. The earlier version keeps projecting its own words: the
projection reads one block with no ledger access, so hiding it is not
something a per-block reading can do, and rewriting it is not something an
append-only ledger does.

The marker is prose and a member can type it, exactly as a member can type a
bracketed id. The bound is that nothing mechanical reads it — no tool, no
stamp and no erasure pass consults the marker, so a forgery can mislead the
model's reading and reach nothing else, and the report tool's co-summoner
validation still bounds where any forgery can aim.

## Rejected alternatives

- **Folding the marker into the bracket.** It corrupts the one token the
  model is taught to name a message by.
- **Deriving the marker at read time in a way any mechanism could act on.**
  It would make forged prose actionable, which the id mark is careful not to
  be.
- **Suppressing the superseded version through a write-time stamp on the
  older row.** A mutation of a fact already read.
- **Waiting for the framework's superseding-block compaction.** The
  follow-ups file records it as unbuilt.
