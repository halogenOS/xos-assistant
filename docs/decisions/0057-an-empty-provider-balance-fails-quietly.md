# 0057 — An empty provider balance fails quietly

Date: 2026-08-23

## Context

Decision 0025 gave every failed turn one notice in the chat. One failure
class does not fit that shape: the provider refusing for lack of balance.
It is not a passing fault. It holds until someone tops the account up, so
every mention in the meantime fails identically, and every one of them
draws the same line — a notice per mention, about a condition nobody in the
chat can act on. The operator asked for silence there.

The consumer can tell the class apart without new machinery. The framework
renders a non-success provider response as `api error {status}: {body}` —
its own `Display` for that error — and hands the rendered text to the
consumer on the stream-error event. The status is therefore readable at the
outbound edge, from a rendering the framework owns.

## Decision

A named predicate sits beside the failure notice constant and answers one
question: does this failure pass without a word. It holds for the
payment-class rendering — the `api error 402:` prefix — and for nothing
else. The outbound edge's stream-error arm reads the event's error text,
consults the predicate, and on a quiet failure writes one info line naming
the conversation and the class instead of delivering. The log keeps the
record; the chat learns nothing.

Everything else is untouched. The latch still closes the conversation, so
spending still stops. The next addressed message still re-engages. Every
other status, and every failure that is not a rendered provider status at
all, still yields the one notice, with its wording unchanged.

## Rejected alternatives

- **Notifying anyway.** The honest-looking option, and the one the operator
  decided against by name: while the balance is empty every mention produces
  the same notice, so the chat fills with a line that repeats a condition
  its readers cannot fix.
- **A failure kind in the framework.** A framework change to carry a
  classification exactly one consumer wants, on an event whose rendered text
  already carries the status. The `Display` prefix is the framework's own
  stable contract; reading it needs no release.

Two clarifications, recorded the same day. Decision 0025 rejected "a notice
classified by string-matching provider prose" — that rejection was about
varying the notice's WORDING by parsed class, and it stands; this record
varies whether the line goes out at all, keyed on the framework's own stable
Display contract for a provider status, not on free prose. And the quiet
class is exactly the payment status of the one registered provider wire,
where it means an empty balance; a provider whose quota refusals arrive as
rate-limit renderings still speaks, which is correct — a rate limit passes,
an empty balance does not.

---

Amended 2026-08-31: superseded by decision 0192. Silence is now universal, so the class
this record singled out needs no rule of its own and the predicate is gone.
