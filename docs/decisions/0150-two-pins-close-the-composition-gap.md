# 0150 — Two pins close the composition gap, both in the core spine suite

Date: 2026-08-30, with unit 40.

## Context

The delivery of a text written ahead of a tool call was already proven, but only over
fixtures that speak the framework's neutral events directly. Production speaks a
server-sent stream that is decoded into those events, and one search fixture did carry a
narration without any pin asserting the order it arrives in. So two things were unproven:
the announce-then-search-then-answer composition over the search itself, and the same
composition as the real decoder produces it.

## Decision

Two pins, both in the core's spine suite.

The first, beside the searching fixtures, asserts the operator's example by its two
deterministic facts: the ledger order — the heads-up text stands before the call block,
which stands before the result — read from the settled shape, and the chat arrival order —
the heads-up reply received before the answer reply — read from two receives on one
outbound edge, with the first-interaction line telling the two deliveries apart.

The second drives the framework's real chat-completions module over the suite's loopback
server, whose script grows a second round: text deltas, tool-call fragments folded by
index, a tool-calls finish, then the closing text once the call is answered. The call
names the runtime-facts tool, the one that reaches no network, so the wire test keeps
exactly one server in it. Rounds are told apart by the request body, so a redispatched
turn draws the round its ledger is at.

What is deliberately NOT pinned: that the heads-up line was delivered before the search's
result existed. Asserting that races the outbound edge against the vendor stub across two
subscribers, and a flaky pin proves less than no pin. The two facts above bracket the same
claim without a stopwatch.

## Rejected alternatives

- **The adapter suite as the home for either pin.** Its fixture has no search wiring and
  its provider speaks the neutral events, so neither pin would prove what it claims there.
- **A wall-clock assertion that the line beat the result.** A race dressed as a guarantee.
- **Trusting the existing narration delivery pins.** They script the neutral events
  directly; the production decoder's own ordering — the end of the turn released before
  the drained calls — is exactly what makes the composition work, and nothing in this
  repository exercised it.
- **A second vendor server for a search call on the production wire.** Two scripted
  servers in one wire test to prove an ordering that the no-network tool proves alone.
