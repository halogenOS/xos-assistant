# 0095 — The literal addressed fact is restored beside the summons

Date: 2026-08-24

## Context

Helpful mode's implementation stores the summons fact (addressed OR helpful)
in the addressed column, and the whole debt spine — the budget counts including
the raw counting SQL and its index, the unlatch, own_debt_taken, the co-summoner
rule, the report tool's scoping, the disclosure fold — reads it there on
purpose. That recast destroyed the literal "the user addressed the assistant"
fact: in helpful mode every stored message reads back addressed, so a mechanism
that needs to know whether a person spoke TO the assistant has nothing to read.

## Decision

Add a second per-message fact, the literal addressed flag (the adapter's raw
fact before the mode folds in), stored beside the unchanged summons column and
read by exactly one new consumer: the outbound miss routing. The recast column
keeps meaning summons and no summons reader moves, so the raw counting SQL and
its index need no change and the report scoping and disclosure duty keep their
meaning. The migration adds the nullable column; historical rows take a safe
default and are never read for their literal value.

## Rejected alternatives

- **Renaming the column to summoned and re-pointing readers to a literal fact.**
  A cold probe showed this breaks the report tool (an absorbed unaddressed
  message becomes unreportable) and the disclosure duty (a person whose first
  interaction was unaddressed-but-summoned would never get the required
  disclosure line), and silently misses the raw-SQL and index readers.
- **Deriving the literal fact from summoned and the mode.** The mode is mutable
  and an addressed helpful-mode message is indistinguishable from an unaddressed
  one, so the fact must be stored, not derived.
