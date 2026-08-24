# 0104 — The rules acknowledgment is a bounded one-shot completion, not a turn

Date: 2026-08-24

## Context

A real rules delta answered with a fixed line (decision 0051), and the
operator, seeing it live, wants the acknowledgment in the assistant's own
voice. Making it a model TURN is architecturally blocked, proven by a probe
of the tree: only the message kind carries an answer debt, a context note
is frontier-transparent by a tested invariant (decision 0094), and a
member-less turn breaks the disclosure fold, the budgets and the abstention
routing it would borrow — the answer machinery is for member answers, and a
rules acknowledgment is a service event.

## Decision

The observation path performs one bounded model completion on a real
`NoteTopic::Rules` delta: the new rules text goes in verbatim beside a
short instruction naming the assistant, and the collected output is
delivered exactly as the acknowledgment always was — on the observation's
return value, stored nowhere, never a block. The call is bounded three
ways: a request timeout, an output cap enforced while the stream is
collected, and the assembly's configured reasoning level. It opens no
debt, no turn, no disclosure fold, no budget row, no co-summoner chain —
none of the answer machinery runs. The provider is the registered module
the answer machinery already uses, bound once for the completion and torn
down with the collected result; the collection mirrors the framework's own
collected reading (deltas accumulate, a final restates, a restart discards)
instead of re-implementing the turn loop. The admission is untouched: the
on-delta comparison stays the whole check, an identical re-pin appends
nothing and calls nothing, and a title change acknowledges nothing.

## Rejected alternatives

- **The member-less turn.** Blocked at every layer by the probe above; the
  revision that proposed it was withdrawn.
- **A template with slots.** Still canned; still drifts from the
  assistant's voice.
- **A rate limit on the acknowledgment call.** Pinning is admin-only and
  the delta check already bounds the spend; an unused limiter is
  complexity for a threat the rights model already contains.
