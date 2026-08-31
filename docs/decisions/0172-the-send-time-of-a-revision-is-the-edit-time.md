# 0172 — the send time of a revision is the edit time

Date: 2026-08-31, with the editing unit.

## Context

A version that came into being hours after the original would otherwise be
stored claiming the original's send time.

## Decision

The adapter translates the platform's edit time into the neutral timestamp
and falls back to the original send time when the platform sends none. It
is read on the edited-message branch alone: the update TYPE decides that a
message is a revision, never the presence of a field on the shared inbound
shape, so an ordinary message carrying an edit time is still an ordinary
message. Whichever timestamp is chosen passes the existing
representable-range guard, and a value outside it is the same named skip it
is today. The block header keeps the store's own receipt time, so the
ledger still holds both times it always held.

## Rejected alternatives

- **Keeping the original send time.** The row would claim a version existed
  hours before it did.
- **Deciding "this is a revision" from a non-null edit time.** A forwarded
  message can carry one, and the fact would then be inferred rather than
  reported.
