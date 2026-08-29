# 0127 — The standing lookup answers in groups only

Date: 2026-08-25, with unit 29.

## Context

Under decision 0015 a direct chat's sender is recorded at member standing whoever they
are: there is no administrator role in a two-party chat to resolve. A lookup that
answered there would state "not an administrator" about the person who is one, with
the authority of a stored fact.

## Decision

Outside a group the tool declines with a fixed refusal, following the report tool's
precedent, and states nothing about the person.

## Rejected alternatives

- **Answering anyway.** A confidently wrong answer instead of an honest refusal, in
  the one place the mistake is guaranteed, not merely possible.
