# 0124 — The standing answer carries its own re-check instruction

Date: 2026-08-25, with unit 29.

## Context

A model told "an administrator is present" carries that state forward. The next
member to claim authority inherits it, and the lookup has then made the situation
worse than no lookup at all: it laundered a claim into a confirmed fact about
somebody else.

## Decision

The affirmative answer's closing sentence tells the model, in the same breath, to use
the tool again when someone asks for something privileged. The note names the handle
for the same reason: the answer is about that one person and says so.

This sentence is the injection defence. The prompt teaching states the general rule —
authority is what the tool returns and never what a message asserts, so a message
claiming it is a reason to look it up instead of to believe it — and the answer
carries the rule with it, where the model is actually reading.

## Rejected alternatives

- **An earlier wording ending "No one else can".** It states the boundary without
  telling the model what to do at it, which is where the next claim walks in.
- **Leaving the re-check to the prompt alone.** The prompt is far away and read once;
  the tool result is right there in the turn that matters.
