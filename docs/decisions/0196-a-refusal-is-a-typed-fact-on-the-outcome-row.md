# 0196 — A refusal is a typed fact on the outcome row

Date: 2026-09-01, with unit 51.

## Context

The compaction turn is offered no tools at all — the operator's requirement, verbatim:
"Compactions dont have tools, that was part of my requirements. But the model might try to
use one anyway." A model that reaches for one anyway is declined by this app's admission
wrapper, which records its own prose as the outcome the model reads.

The framework already bounds that loop: a run of five refusals ends the turn. But it
recognized a refusal by matching the start of the recorded error text against its own
rate-limit prefix, and the framework's recorded position was that the prefix IS the machine
key. This app's decline never starts with that prefix, so a declined call counted as an
ordinary failure and the turn ran on. The only remaining bound was the conversation's
tool-call window, sixty calls a minute — sixty paid model rounds where five would do.

## Decision

Whether an outcome is a refusal is a typed fact stored on the outcome row, set by the pass
that made the decision, read back by the framework's fold. A refusal says the model spent a
round and was handed only the reason.

This app's palette decline sets that fact through the framework's typed surface —
`ToolOutcome::Refused` — and keeps its own wording. The words stay this app's; the fact is
the framework's. A decline loop in a toolless turn now ends at the five-refusal forced end.

The prefix match retires, and the position that the prefix is the machine key is superseded
here: it held while one producer of refusals existed inside the framework, and a second
producer outside it needs the same fact.

## Rejected alternatives

- **Matching this app's `declined:` prose from the framework.** It hardcodes consumer
  vocabulary in the framework, and every consumer that ever writes a different sentence
  silently stops being bounded.
- **Making this app write the framework's rate-limit prefix.** It ships a sentence that lies
  to the model — nothing was rate limited — and it breaks the recorded wording this app
  tests for.
- **A refusal counter this app keeps itself, ending the turn from outside.** The framework
  owns the turn's end and already counts the run; a second counter is a second decision path
  over the same fact, and it would disagree the first time either side changed.
- **Leaving the decline as an ordinary failure and shrinking the tool-call window.** The
  window bounds a different thing, and shrinking it would cut short every legitimate
  tool-using turn to bound a turn that has no tools at all.
