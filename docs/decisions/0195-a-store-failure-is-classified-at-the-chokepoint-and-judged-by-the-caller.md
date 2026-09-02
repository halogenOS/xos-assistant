# 0195 — A store failure is classified at the chokepoint and judged by the caller

Date: 2026-09-01, with unit 51.

## Context

A production morning ran on a corrupted premise. A compaction deleted its temporary
conversation under a turn that was still writing, every later write of that turn hit
`FOREIGN KEY constraint failed`, and the process logged each one at error level and carried
on. The failures were real damage reports and nothing read them as such, because the store
answered every one of them with a single opaque variant: a foreign-key violation, a busy
timeout and a disk error arrived through the same `StoreError::Sqlite`, so no caller could
tell impossible state from ordinary write contention.

Two answers were on the table and the difference between them is where the JUDGEMENT sits.
The operator settled it, verbatim: "There is a difference between catching an expectable
query error and a serious db failure. Something failing on a foreign key constraint is an
error while a race with another writer is expected and can be retried if it makes sense. […]
You aren't meant to panic inside a db query but instead wrap and propagate the error properly
so a codepath competent to handle it can decide what to do about it."

## Decision

Classification happens at ONE chokepoint, inside the store actor, where the rusqlite error is
still typed and its extended code still readable. The classifier reads the primary code and
answers a class: `Rejected` for a constraint violation, `Contended` for busy and locked,
`Unusable` for corruption, not-a-database and misuse, plain `Sqlite` for everything else, and
`ActorStopped` when the store's own thread is gone. Nothing ends the process inside a query.

The class travels to the caller, and the codepath that knows the blast radius decides what it
means. This app states `Unusable` and `ActorStopped` as `FailureKind::Fatal`, and each intake
ends its run on that class with the message still unacknowledged, so the supervisor starts a
replacement process and the platform redelivers what was never taken. Every other class stays
what it was: a refused request, a failed compaction, a logged retry.

Because the classifier sits under every caller, no caller can swallow what it raises, and the
whole path is exercisable in-process — the classes are values, so a test can produce every
one of them and read what each intake does with it.

## Rejected alternatives

- **Ending the process at the store-actor chokepoint itself.** This unit's first position,
  superseded the same day by the words quoted above. Classifying and deciding in one place
  reads as tidy and is not: the store actor knows a constraint failed and knows nothing about
  whether that constraint belongs to one refused message or to the whole process. Deciding
  there takes the judgement away from the only code with the context, and it puts a process
  end under every test, which is why that position needed a subprocess harness to be
  exercised at all.
- **Crashing on every `StoreError::Sqlite` indiscriminately.** That variant carries busy
  timeouts and disk errors as well as constraint violations, so a wholesale sweep would end
  the assistant on ordinary write contention. The primary code is the discriminator.
- **Classifying at the call sites.** Every call site would need the same reading of the same
  codes, and the ones that forgot would be the ones that mattered. One reading, one place.
- **Auditing every log site for swallowed integrity failures.** Proposed as a sweep across
  hundreds of sites in two repositories with no procedure for deciding any of them. The
  chokepoint makes every caller compliant by construction instead.

---

Amended 2026-09-02: the class list above is refined by decision 0199. A refused statement is
fatal now, beside the two classes this record names, and an unattended compaction that meets
one ends the process instead of retrying. Contention and the plain storage class stay what
this record states, as does everything else in it.
