# 0199 — A refused statement is fatal, and a refused compaction ends the process

Date: 2026-09-02, with unit 56.

## Context

Decision 0195 put the classification of a store failure at one chokepoint and left the
judgement to the caller. This app then judged two classes fatal, the unusable database and the
stopped store actor, and left a refused statement in the passing classes: a caller met it,
failed what it was doing, and the next attempt ran.

An afternoon in production showed what that costs where no caller exists. A prompt edit moved
every live group onto a conversation whose system prompt sat at the END of the ledger, and the
compaction of such a conversation opens a thread that must carry its own prompt, which the
one-prompt rule refuses. The refusal counted as passing, so the unattended compaction forked a
temporary conversation, paid for a summary turn over half the history, met the refusal, and
started over on the next wake, every thirty seconds and on every block change.

The operator's standing words are the measure: "There is a difference between catching an
expectable query error and a serious db failure", "a database error should hard crash the
application, not leave it running in a corrupted state", and "Failures are failures and
failures just fail the compaction."

## Decision

A refused statement is fatal. `StoreError::Rejected` classifies `FailureKind::Fatal` beside
the unusable database and the stopped actor: the database applied a rule this code violated,
the ledger is in a shape this code cannot continue from, and the same statement is refused the
same way every time. `StoreError::Contended` stays `FailureKind::Transient`, and so does the
plain storage class: a race with another writer is about what one message asked for, and the
next attempt can win it. A rule is not a race.

An unattended compaction has no caller to fail, so it states the failure out of band. On a
fatal class the compaction driver raises the assembly's exit signal and stops watching, with
no retry and no backoff; the binary ends the run, and the supervisor starts a replacement over
the same durable state, where the startup walk repairs the ledger before anything is served.
Every other class leaves the conversation standing and the next wake tries again.

Refines decision 0195, which stands otherwise: one classifier, one chokepoint, the judgement
with the caller that knows the blast radius. What moves is which classes this app judges
fatal, and that a background path with no caller can now end the process itself.

## Rejected alternatives

- **Stopping the retry for the lifetime of the process and carrying on.** The first draft of
  the fix. Rejected: a refused statement means the ledger is in a shape the code cannot
  continue from, and the answer to that is to end loudly, not to serve around it while one
  conversation quietly stops being compacted.
- **Retrying a refusal with a backoff.** Rejected on the operator's standing words above. A
  backoff on a rule refusal only slows the burn: the statement is refused identically on every
  attempt, and each attempt costs a summary turn.
- **Narrowing the fatal class to the positional prompt rule alone.** Rejected: it would put
  the reading of which constraint fired back at the call sites, which is exactly what decision
  0195 removed. Every refusal names a rule this code violated, and none of them passes on a
  retry.
- **Leaving the compaction driver to log the fatal class and keep watching.** Rejected: it
  reproduces the incident with one line of prose added. A path with no caller either states
  the failure where the process can act on it or swallows it.
