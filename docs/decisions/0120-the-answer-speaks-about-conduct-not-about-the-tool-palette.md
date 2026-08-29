# 0120 — The answer speaks about conduct, not about the tool palette

Date: 2026-08-25, with unit 29.

## Context

The assistant already has a standing mechanism: every tool is admitted at a required
authority, and the admission check compares the turn's provenance against it
(decision 0043). The standing lookup answers a question that sounds like the same
one, and its answer can differ from what that check would decide — a Moderator
answers true here, while a tool admitted at Admin would refuse that same turn.

## Decision

The lookup answers "may this person tell the assistant how to behave". The admission
check answers "which tools may this turn reach". They are two questions, decided in
two places, and they are allowed to differ.

The divergence is deliberate and is recorded here because a later reader finding two
different answers to what looks like one question should find the reason beside
them. Today nothing diverges in practice: every registered tool sits at the member
floor, so the admission comparison never refuses anybody.

## Rejected alternatives

- **Deriving the answer from the palette.** It would tie a sentence about a person's
  standing in their group to an internal admission table, and make both change
  together for no reason — a general mechanism made to know about one tool's
  sentence.
