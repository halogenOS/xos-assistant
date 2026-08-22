# 0034 — Over-limit is silent in the chat, and only a taken debt re-engages

Date: 2026-08-22

## Context

A refused answer could be announced, and decision 0022 made every addressed
message a re-engagement of an error-latched conversation. Both surfaces are in a
hostile sender's hands during a flood.

## Decision

A rate-limited addressed message draws no answer and no notice: a notice per flood
message is a flood amplifier a hostile sender controls. The limited fact in the
ledger is the audit trail; the behavior is documented in the repository. The
unlatch intent follows the same line: only a message whose own debt is taken —
addressed, not limited — is re-engagement per decision 0022. A refused debt
neither answers nor unlatches, so a limited flood cannot wake an error-latched
conversation. This refines decision 0022's "every addressed message" to "every
addressed message the budgets admit"; unaddressed messages stay silent as before.

## Rejected alternatives

- **A notice per limited message.** Hands the flooder the assistant's voice.
- **A one-per-window notice.** Still triggerable on schedule.
- **Limited messages unlatching.** Re-engagement by a message the budget just
  refused.
