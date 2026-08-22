# 0035 — Budgets live in the configuration file

Date: 2026-08-22

## Context

The budget numbers are product knobs. Decision 0024 made the configuration one
file with unknown keys refused.

## Decision

The configuration file gains a protection table with four fields and per-field
defaults: principal, 6 answers per 600 seconds; channel, 20 answers per 600
seconds. A window of zero disables that budget explicitly; an answer count of
zero is refused at parse, naming the field — an assistant configured to answer no
one is a misconfiguration, not a policy. A partial table takes per-field
defaults; unknown keys are refused like everywhere else in the file. In the
core's own configuration type the count is nonzero by construction, so the
refusal is structural, not a convention.

## Rejected alternatives

- **Hardcoded budgets.** A product knob in code.
- **Per-channel-kind budgets.** Direct chats end at the principal budget anyway.
- **Count-zero as disable.** Inverts the natural reading; the window is the
  disable knob.
