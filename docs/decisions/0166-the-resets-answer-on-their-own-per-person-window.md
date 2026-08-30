# 0166 — The session resets answer on their own per-person window

Date: 2026-08-30, with unit 45.

## Context

The privacy family's four rights commands answer through a per-principal window that grants
a reply and applies the state change as ONE operation: a withheld reply withholds the change,
and a change that fails hands its grant back. The session resets need the same protocol —
they change state and they are triggered by a person — and they need a bound that a flood of
one family cannot spend on the other's behalf.

## Decision

`/wipe` and `/compact` share ONE per-principal window, the privacy family's
one-window-per-family shape, with its own constants set to the same values as that family's.
Each bound carries its own constant, as everything bounded here does, because the two bound
different things and either may move alone. The window is budget-exempt: the flood budgets
bound answering, and a moderator command is not the flood they exist for.

The reset is applied exactly with the granted reply. Past the cap the reply is recorded
silence and the reset it would have made is withheld with it, so no session is ever replaced
into a silence nobody can see. A reset that fails answers silence too, with a warn log — and
the log says the reset failed PARTWAY and points at what stands, because the alternative
wording would promise an atomicity the swap does not have.

The remaining window is narrow and named. The sweep itself is one transaction now, so a fork
is never half-swept; what is left is the fork-then-claim shape the creation race already
records, and a failure or a crash between the fork and the claim leaves a fork nothing points
at — harmless, never cleaned — or a channel with no mapping, which the adapter's redelivery
of the unacknowledged update converges on at the next attempt.

A reset that lost its mapping claim answers silence as well, for a third reason: it made
nothing at all, and the session the channel ends up with belongs to the racer that won
(decision 0165).

The three fixed lines are core copy, because wording is behaviour, and they are pinned byte
for byte beside the catalogue:

- the wipe, applied: `Done. This group starts a fresh session; the old one stays on record.`
- the compact, applied: `Done. This session was compacted: recent messages stay, old context
  is set aside.`
- the compact, nothing to cut: `This session is already compact. Nothing changed.`

## Rejected alternatives

- **Sharing the privacy family's window instance.** A flood of session resets would then
  silence somebody else's data-rights command, which is the one thing a rights bound must
  never do.
- **A window per command.** A moderator who wipes and then compacts is one person doing one
  thing to one session; two counters would bound neither honestly.
- **Applying the reset before the grant is decided.** A reset applied into recorded silence
  is a group whose session changed with nothing said about it.
