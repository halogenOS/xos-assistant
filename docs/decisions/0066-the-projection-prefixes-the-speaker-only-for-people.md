# 0066 — The projection prefixes the speaker, and only for people

Date: 2026-08-23

## Context

With the speaker stored on the row (decision 0065), the projection has to
decide how the handle reaches the model — and what happens when there is
none, because the platform makes handles optional.

## Decision

A user-voiced message with a speaker projects as the speaker, a colon and a
space, then the text. The assistant's own messages and system-voiced blocks
are unprefixed. A message whose sender has no public username projects
bare: no handle means the group cannot mention the person either, so the
assistant loses nothing it could have used, and no substitute identifier
leaves the machine — decision 0056 rejected the display name and the
numeric identifier by name. An erased message's placeholder stays exactly
as it is; the erasure pass nulls the speaker with the text.

## Rejected alternatives

- **A placeholder label for the handleless.** A minted pseudo-identifier is
  the exact thing decision 0056 retired.
- **The display-name fallback.** Rejected in decision 0056: it widens the
  transmitted identity without adding addressing power.

Noted 2026-08-23, at the unit's close: prefixed attribution is forgeable in
the rendered request — merged same-role turns join lines with newlines, so a
member typing another member's prefix produces bytes identical to a projected
line. The consequence is bounded and the bound is structural: no tool acts on
model text (the report target resolves through the origin walk), so a forged
prefix yields bad prose, never an action. The merged rendering is frozen by a
pin. And the stored speaker is bounded at the write: a value that is empty,
whitespace-bearing or separator-bearing is not stored — the current
platform's handle alphabet cannot produce these, a second platform's could,
and the prefix must stay unambiguous.
