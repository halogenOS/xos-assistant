# 0075 — Plain language reaches the same rights mechanisms through one tool

Date: 2026-08-23

## Context

A person should not need command syntax to exercise a right: "please stop
collecting my messages" must work. The model can read that ask, but the
model must never perform a privacy change itself — the system enforces the
change, and the one failure this design must never have is acting on a
guessed person.

## Decision

One privacy tool — member authority, palette-governed, actions `opt_out`
and `request_deletion`, any other or absent action answered with the fixed
invalid-action result — acts on the turn's origin set resolved to
PRINCIPALS: the own-debt-takers of the debt origin walk, mapped to their
stored principal ids. Exactly one distinct principal in the set: the tool
acts on it. Several (the absorbed co-summoner shape), none, or an erased
row whose principal no longer resolves: the tool DECLINES with the fixed
ambiguity result naming the commands, because the commands are always
unambiguous.

`opt_out` writes the flag through a protected consumer surface holding the
erasure fence for reading; the unit-5 no-write amendment gains its dated
second clause — a tool may write the consumer's own identity-table fact
when the write IS the honored right. `request_deletion` files the same
principal-keyed pending state the command files and returns the fixed
result carrying the literal confirm token for the model to relay; the
prompt orders the relay verbatim, the pinned fact is the token in the tool
result, and a model garbling it costs one retry via the command path — the
stated residual. The tool's writes failing return the transient result in
the report tool's established wording style. A second stated residual: a
call from a person whose reply window is exhausted answers that same
transient result — the fixed line stays true (nothing took effect, the
commands remain the direct path), at the cost of not naming the bound as
the reason.

## Rejected alternatives

- **Acting on "the newest" of several co-summoners.** The wrong person,
  structurally: recency is not consent.
- **A target parameter.** Forgeable — the model could be talked into
  naming someone else, and the system could not tell.
