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
the model spent a round and got a standing no, and a model looping on the ask reaches the
forced end of its turn on the framework's own count. The refused ask costs the person none of
their shared privacy-reply grants — it changed nothing and recorded nothing, so the reply
window hands the grant back, on the command surface exactly as on the tool's.

The refused confirm spends its grant. It consumes the pending it found, and consuming it is a
state change: the person's request is gone from this process, which is what the stopping line
tells them. A confirm that finds no pending spends its grant too, its own line being an answer
the bound is there to bound.

## Rejected alternatives

- **Aborting the erasure like the other two.** Rejected: it breaks a promise already spoken.
  The person was told the deletion had started, and the cost of keeping it is a wait measured
  in one short run over local tables.
- **Keeping a pending across a restart.** Rejected: the pending is process memory on purpose,
  the flow where forgetting errs safe. Persisting it would store a person's deletion intent to
  survive a restart — a new personal record for a state whose loss already has a correct
  answer, the nothing-pending line and a fresh ask.
- **Letting the refused ask spend a reply grant like any answered ask.** Rejected: one looping
  turn during a shutdown would drain the person's bound on refusals alone and then withhold
  their own `/privacyout`. The bound exists against a flood of lines that carried a change,
  and the refused ask changes nothing. The refused confirm is the other case and spends its
  grant, since it consumes the pending it found.
- **Waiting for the compaction as well.** Rejected: a compaction sits inside a model turn, so
  a shutdown that waited for it would be as slow as the provider. What it leaves behind is a
  few blocks served to nobody, which the sweep reclaims.
