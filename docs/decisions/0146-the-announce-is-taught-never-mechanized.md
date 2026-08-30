# 0146 — The heads-up line is taught, never mechanized

Date: 2026-08-30, with unit 40.

## Context

A question that needs a web search leaves the chat silent while the call runs: the
composing cue goes dark for the whole tool window, so nothing tells the member that work
is happening. The assistant should say, concisely, that something is about to take a
while.

The delivery mechanism for that already exists whole and is proven. A provider round's
text ahead of a tool call finalizes as its own committed answer block; the end of the
stream fires once per round, for a tool-use stop exactly as for an end of turn; and the
outbound edge wakes on it and delivers every committed answer above its cursor, with no
filter that would hold a mid-turn text back. An adapter pin already walks the operator's
example end to end: narration delivered, tool run, answer delivered, in order and
threaded. What was missing was that nothing asked the model to write the line.

## Decision

One sentence joins the web search teaching: before running a search, say in one short
line what you are about to look up, then search, then answer. The unit's whole surface is
that prose, its pins, and one grown test fixture — no new code in the core, the adapter or
the framework, and no new tool.

Stated honestly, because the difference matters to anyone reading a failure: a taught
behavior is probabilistic. The mechanism guarantees that whatever text precedes a call is
delivered; it does not guarantee that the model writes one. A turn that searches without
announcing is the model's own miss, not a broken mechanism.

## Rejected alternatives

- **A framework early-flush boundary.** The per-round end of stream already IS the
  pre-call flush. A second boundary would write the same decision twice, and the two
  would drift.
- **Code in the core or the adapter.** Nothing there decides what the model says; a
  mechanism that injected a line would be the assistant speaking words no model wrote,
  in a place the conduct prose cannot reach.
- **Leaving it undone because the mechanism already works.** The mechanism delivers a
  line nobody asks for. Without the teaching the member sees the same silence.
