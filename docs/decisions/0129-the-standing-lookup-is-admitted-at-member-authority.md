# 0129 — The standing lookup is admitted at member authority

Date: 2026-08-25, with unit 29.

## Context

The tool reads a stored standing and states it to the model, so admitting it at
administrator authority looks careful at first glance.

## Decision

Member authority. What the tool answers is visible in the group's own member list to
anybody who opens it, so there is nothing here that an ordinary member may not know.

## Rejected alternatives

- **Admitting it at admin.** It would answer only for people who already know the
  answer, and the question is asked precisely about the turns an ordinary member
  summoned. It would also be the first registration above the provenance floor, waking
  a refusal path that has never fired — a real change to the admission behaviour,
  smuggled in behind a tool that did not need it.
