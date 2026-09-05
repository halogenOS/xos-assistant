# 0200 — The shutdown abandons the cheap work and keeps the promises

Date: 2026-09-04, with unit 55.

## Context

The assembly holds unattended tasks nothing waits on: the compaction driver, the retention
sweep, and every confirmed erasure still running. Until now a stopping process simply exited
under them. Two of those tasks cost a later sweep when they are cut mid-work; the third is a
promise made to a person in words, since the confirm answers that the deletion is underway
before its task has removed a single row.

The deletion flow reaches the same state through two surfaces, the `/privacydelete` command
and the privacy tool, and both of them file a pending confirmation this process alone holds
in memory.

## Decision

The shutdown treats a task by what its half-done state costs.

The compaction driver and the retention sweep are ABORTED wherever they stand. A driver cut
mid-compaction leaves a temporary conversation nothing is mapped to, which is exactly what a
killed process leaves behind; under a configured retention span the sweep names that
conversation like any other whose span has passed — the rule reads a conversation's newest
block and asks nothing about a mapping — and deletes it whole a span later.

A confirmed erasure is AWAITED to its end. It writes across several tables, and cut midway it
leaves some of a person's rows emptied and others standing after that person was told the
deletion had started. It is short, it asks no provider, and it is idempotent on a re-ask.

Both halves of the deletion flow are REFUSED once the shutdown has begun, with one fixed
line: the ask files no pending, and a confirm arriving past that point consumes the pending it
finds, starts nothing, and does not refile it. The line is this one, and this document is the
record of it:

> The assistant is shutting down, so the deletion was not started. Your request is not kept.
> Once the assistant is back, ask again with /privacydelete and confirm with /confirmdelete.

It sends the person back through `/privacydelete` and `/confirmdelete` once the assistant is
up again. The refusal lives in the pending memory itself, which holds the stopping fact: a
pending this process would refuse to confirm cannot be built, whichever surface asks.

On the tool surface the refused ask is the framework's typed refusal, not a completed call:
the model spent a round and got a standing no.

Every answer the privacy reply window delivers spends a grant, on the command surface and the
tool's alike, the refused ask included. The framework ends a turn on a trailing run of
refusals among the open turn's own outcomes (decision 0196). Model text resets nothing, and
any other outcome of that turn in between starts the count over, so the count is no spend
ceiling on the window. The window is the bound on both surfaces, which is what it is for: it
bounds a person's own commands and what the tool does on that person's behalf alike, as it
has since 2026-08-23 (decision 0076), and either one repeated during a shutdown drain reaches
its silence like any other flood.

## Rejected alternatives

- **Aborting the erasure like the other two.** Rejected: it breaks a promise already spoken.
  The person was told the deletion had started, and the cost of keeping it is a wait measured
  in one short run over local tables.
- **Keeping a pending across a restart.** Rejected: the pending is process memory on purpose,
  the flow where forgetting errs safe. Persisting it would store a person's deletion intent to
  survive a restart — a new personal record for a state whose loss already has a correct
  answer, the nothing-pending line and a fresh ask.
- **Handing the reply grant back on the refused ask.** Rejected: which answers are free would
  become a per-arm judgment with no rule behind it. A confirm that finds nothing, an opt-out
  already standing and this refusal all deliver a line and change nothing, and only one of
  them would have been free. A delivered line is what the bound bounds.
- **Waiting for the compaction as well.** Rejected: a compaction sits inside a model turn, so
  a shutdown that waited for it would be as slow as the provider. What it leaves behind is a
  few blocks served to nobody, which the sweep reclaims.
