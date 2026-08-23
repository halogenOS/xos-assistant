# The group operator's contract

Date: 2026-08-23. What a group operator does to run the assistant in a
group, and the exact rules the assistant reads by. The mechanisms behind it
are recorded in decisions 0047 through 0054.

## Admitting the assistant into a group

The assistant serves only groups the configured operator added it to. The
operator's platform id is named in the deployment configuration (the
operators table); when that account adds the assistant to a group, the
admission is recorded durably and survives restarts. An add by anyone else
— or an add while no operator is configured — makes the assistant leave the
group, and every later contact from an unadmitted group is refused the same
way, so a missed leave heals itself.

## The rules pin, verbatim

The assistant reads the group's rules from the pinned announcement, under
this exact contract:

- A pinned message whose **first line is exactly `Rules:`** — case
  sensitive, nothing before it, followed by a newline (a carriage return
  before the newline is tolerated) — is the group's rules.
- The `Rules:` line is stripped; the remainder is the rules text the
  assistant follows and quotes to the model.
- A remainder that is empty after trimming is refused; the pin is not
  rules.
- A pinned message without the prefix is an ordinary announcement: it is
  not rules and does not replace any earlier rules.
- The rules text is bounded at 4096 bytes. An over-bound text is refused
  whole, never cut short — write shorter rules instead.

When the assistant picks up new or changed rules, it says so in the chat
with one fixed line: "Rules noted. The assistant follows the pinned rules
of this group." At most one such line goes out per group within the
acknowledgment window; further changes inside the window are picked up
silently.

## First setup: post fresh, then pin

The platform's lookup exposes exactly one pinned message, chosen by the
pinned messages' **sending dates** — not by which was pinned most recently.
An old rules message can therefore sit invisibly behind a newer pinned
announcement. To make the current rules the ones the assistant sees, post a
**fresh** rules message and pin it; the acknowledgment line confirms the
pickup.

## Replace rules, never merely unpin them

Unpinning produces no event the assistant can read, so rules removed by
unpinning would silently stand until the next rules pin. To change or
retire rules, pin a new `Rules:` message that states the current rules —
replacing, not merely unpinning.

## The trust boundary, stated plainly

Whoever holds the group's pin right can steer the assistant: a rules note
is written into the assistant's system voice. That is the feature — the
group governs its assistant — and the boundary is the group's own
administrator set, which controls pinning. The byte bound above caps the
surface; there is no second control beyond the group's own admin
discipline.

## The privacy command

`/privacy` — bare, or suffixed with the assistant's own handle — answers
with the configured privacy policy address, deterministically and without a
model turn. The answer goes out at most once per group within the same
window the rules acknowledgment uses; repeats inside the window are read
and left unanswered. The platform additionally offers a bot-level privacy policy
field in its bot management surface; filling it is deployment wiring, kept
in the deployment notes, and does not replace the command.
