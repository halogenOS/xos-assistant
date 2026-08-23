# 0076 — The rights replies are bounded per person, and never budget-silenced

Date: 2026-08-23

## Context

Every fixed line a member can trigger is bounded — the flood-amplifier
discipline of the protection unit. The existing bound is channel-keyed,
which fits a courtesy line but not a right: one neighbor's `/privacy`
flood must not starve another person's deletion confirm, and the answer
budgets that silence a flooder must not silence that same person's opt-out.

## Decision

The four self-service commands and the privacy tool's deterministic
replies are bounded by their own window keyed by PRINCIPAL
(`PRIVACY_REPLY_WINDOW`, the acknowledgment length, with a named per-window
cap), so one person's flood bounds that person alone. The answer-budget
check does not gate the family at all: a rights request is answered even
from a sender the flood budgets have silenced — the per-person window is
the whole bound. The state change (flag, pending, confirm) applies exactly
when its reply is granted, never silently. `/privacy` keeps its existing
channel-keyed bound; it is a notice pointer, not a state change.

## Rejected alternatives

- **State change on recorded silence.** A destructive action with no
  receipt — the person would learn of their own deletion by its silence.
- **The shared channel window.** Cross-person starvation of a right: one
  member's flood would close the window a neighbor's confirm needs.
