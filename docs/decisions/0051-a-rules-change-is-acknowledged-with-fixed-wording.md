# 0051 — A rules change is acknowledged in the chat, with fixed wording

Date: 2026-08-23

## Context

The operator asked for visible confirmation that a rules change was picked
up, and the pickup must be reliable without becoming a flood amplifier a
pin-toggling admin can play.

## Decision

When an observation appends a rules note — new or changed — the returned
value carries the acknowledgment: the fixed line `Rules noted. The
assistant follows the pinned rules of this group.`, a named constant beside
the failure notice. Deterministic product behavior, not a model answer: no
turn, no budget slot, no wording drift, and the note itself stays inert. At
most one acknowledgment per channel per acknowledgment window (a
named-constant cooldown): within the window a further delta still appends
its note, silently — the flood-amplifier discipline the protection unit
recorded for notices applies to any bot line a non-operator can trigger.
Title changes are not acknowledged.

## Rejected alternatives

- **A model-generated acknowledgment.** A turn for a confirmation line.
- **Unbounded acknowledgments.** A pin-toggling admin makes the bot spam
  the chat.
- **Silence.** The operator asked for visible confirmation.

Refined 2026-08-23, at the unit's close. The acknowledgment window bounded
the chat line and left the ledger unbounded: a pin toggler appended a
system-voiced note per toggle, each projected on every later turn. Note
appends of one topic are now capped at `NOTE_TOPIC_APPEND_CAP` (3) per
conversation within the same window. A capped delta is not queued: the next
observation after the window re-reads the newest note, and the still-standing
difference appends through the on-delta rule itself.
