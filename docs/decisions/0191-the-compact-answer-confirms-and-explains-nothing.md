# 0191 — The `/compact` answer confirms the act and explains nothing

Date: 2026-08-31.

## Context

Unit 45 shipped `/compact` with an answer that described the mechanism of that era: the
recent messages stay, the old context is set aside. Unit 48 replaced the mechanism — the
older half becomes a model-written summary that rides along — and the shipped line went
quietly wrong about what the command does. The replacement is user-facing copy, so the fix
pass that found the gap recorded it and left the wording to the operator.

## Decision

The answer is `Compaction finished` — the operator's copy, verbatim. The line confirms
that the command took effect and says nothing about what compaction does to the history;
that explanation lives in the group operator contract, which already carries it in full.
A confirmation that re-explains its mechanism goes stale every time the mechanism moves,
and this one already had.

## Rejected alternatives

- **Keeping the unit 45 line.** It described a mechanism the assistant no longer has.
- **A line describing the new mechanism** ("the older half is now a summary, the recent
  half stays word for word"). Rejected by the operator as too verbose — and it would
  inherit the same staleness the old line died of.
