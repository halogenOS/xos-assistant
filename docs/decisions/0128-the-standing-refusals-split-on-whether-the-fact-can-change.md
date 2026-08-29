# 0128 — The standing refusals split on whether the fact can change

Date: 2026-08-25, with unit 29; corrected and completed 2026-08-29.

## Context

This repository already has a rule for refusal wording, and it is a split, not a
uniform close. A refusal whose fact holds for the whole turn ends with the shared
no-retry line, so a model offered a tool that will refuse it does not loop. A refusal
whose fact may not hold beyond this failure carries no such line, because a later call
may work; the admission check and the report tool both state it that way, and the
report tool's transient error is pinned to exactly that shape.

## Decision

Five PERMANENT refusals, each its own fixed string closing with the shared no-retry
line: a handle the conversation never showed; a handle shown only by a join; a call
outside a group; a stored standing that does not parse; and a malformed call, meaning
a missing or non-string handle — the framework validates no arguments, so the handler
answers that shape itself.

One TRANSIENT refusal, for a read that did not stand, carrying no no-retry line and
naming the moment instead.

Every one of the six states nothing about anybody. An unreadable standing, in
particular, answers no standing at all instead of falling back to member: a broken row
is not evidence that somebody is an ordinary member.

## Rejected alternatives

- **Leaving the wording to the implementer.** An earlier revision of the unit did, and
  shipped two pinned strings and a third nobody wrote.
- **A no-retry line on the transient refusal.** It contradicts the documented, pinned
  convention and would teach the model never to retry a call that would have worked.
- **Folding the join-only case into the never-shown refusal.** The model can see the
  join line; a refusal it can see is false teaches it to distrust the tool.
