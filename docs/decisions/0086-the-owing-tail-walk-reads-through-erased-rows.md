# 0086 — The owing-tail walk reads through erased rows

Date: 2026-08-23

## Context

The owing-tail walk read through the consumer's independent mid-history
kinds by kind string. An erased chat row keeps the live rows' kind, so the
walk evaluated it directly, and its cancelled debt settled the tail — which
was right for the erased row's own ask, and wrong for a debt the row merely
carried: deleting the tail that propagated a third party's unanswered ask
silently dropped that ask, the mirror's carried-tail case above all.

## Decision

Erased chat rows are transparent to the walk, exactly like the read-through
kinds: the tail read resolves past the whole transparent run in one bounded
query and evaluates the first block that still speaks. The erased row's own
debt still dies through the shared owes-answer reading, unchanged; a live
ask a third party's row still owes behind the run propagates onto the next
stamp — someone else's deletion erases one row, not the standing question
behind it. The query is owned by the chat kind, because only the kind knows
an erased row's shape, and it absorbs the module that held the kind-agnostic
read: with every caller needing the kind-aware form, the kind-agnostic one
had no consumer left.

## Rejected alternatives

- **Walking row by row past erased rows.** A person-wide erasure can null a
  run of any length; the walk sits on ingestion's hot path, and a
  row-per-read loop degrades into a conversation hydration.
- **Dropping the liveness criterion and recording the loss.** An
  administrator deleting one bystander message would then kill another
  person's pending answer — a silent cross-person effect no one asked for.
- **Widening the read-through to the framework's turn-closure markers.** A
  closed turn is a settled tail; reading debt through its marker would
  resurrect failed turns' debts.
