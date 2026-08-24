# 0098 — Silence is the default in helpful mode

Date: 2026-08-24

## Context

Helpful mode folds every group message into the model's view, and the live test
showed the assistant replying to messages that asked nothing — a statement, a
message setting up group content, members talking among themselves.

## Decision

The teaching leads with silence as the default and frames the grounded,
genuinely-helpful answer as the exception that clears a bar, not a reflex a
question-shaped string triggers. A message that warrants no reply draws the
abstention sentinel.

## Rejected alternatives

- **A reply-rate limiter.** The turn budget already bounds volume; this is a
  matter of judgment, not rate, and a limiter would suppress a genuinely helpful
  answer while still letting a poor one through when the budget is fresh.
