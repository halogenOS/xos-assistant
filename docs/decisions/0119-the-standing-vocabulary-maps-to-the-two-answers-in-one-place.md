# 0119 — The standing vocabulary maps to the two answers in one place

Date: 2026-08-25, with unit 29.

## Context

Three stored standings, two answers. The translation between them is a privilege
judgment: it decides who the model will treat as able to override its instructions.

## Decision

The mapping is written once, in a named function inside the standing tool, and read
from nowhere else. Its match over the three values is exhaustive, so a fourth
standing added later fails the build at the one place that must decide it.

A second place deciding the same thing is how a privilege check becomes a privilege
escalation: the two drift, the looser one wins, and nobody reading either can see
the other. One decision, recorded once, in code as much as in these files.

## Rejected alternatives

- **A third result string naming the standing found.** The two result strings are
  the operator's own words and are pinned byte for byte; a third has nowhere to
  live, and the distinction it would draw is one the answer does not need.
- **Deriving the mapping at each call site from a comparison against Moderator.**
  An ordering comparison spelled out twice is two decisions that look like one.
