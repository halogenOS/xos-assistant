# 0147 — The heads-up line is scoped to slow work, which today means the search

Date: 2026-08-30, with unit 40.

## Context

Several tools read over HTTP: the forge, release and wiki lookups against configured
project hosts, and the web search against the open web. All four share the same
ten-second timeout, so no locality argument separates them. The question is which of them
earns a line ahead of the call.

## Decision

The sentence lives inside the web search teaching, which composes if and only if the
search capability is admitted. So the line is taught for the search and for nothing else:
not for the project lookups, and not for a report, whose own teaching wants the judgment
made quietly and whose whole action is the report itself.

The scope is a product decision taken from the operator's own example — a question needing
a web search — not a claim that the project lookups are always fast. A future slow tool
brings its own sentence with it, in its own teaching, where the same capability predicate
already decides whether that tool exists at all.

## Rejected alternatives

- **Announce before any tool call.** Chatter ahead of sub-second lookups, in a product
  whose conduct prose tells it to match its length to the message's weight.
- **A shared announce sentence composed beside every tool teaching.** One decision, two
  homes: the sentence would have to name which tools are slow, and that list would live
  apart from the teachings that admit them.
- **A timeout-based rule taught to the model.** The model cannot know in advance how long
  a host will take, so the rule would be an invitation to speculate.
