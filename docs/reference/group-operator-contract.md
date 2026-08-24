# The group operator's contract

Date: 2026-08-23. What a group operator does to run the assistant in a
group, and the exact rules the assistant reads by. The mechanisms behind it
are recorded in decisions 0047 through 0054, and the report setup in 0060
and 0062.

## Group privacy mode must be off

Before the assistant can see the group's messages, its platform privacy mode
must be disabled: with it on (the platform default) the assistant receives
only messages that mention it, reply to it, or are commands, and cannot read
the ordinary conversation it exists to help with or moderate. Disable it in
the platform's bot settings, then remove the assistant from the group and add
it again — the platform fixes privacy mode at join time, so the change only
takes effect on a fresh join. Keeping the assistant a non-administrator (see
the report setup) is unaffected: privacy mode and admin status are separate
switches.

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
with a short confirmation in its own voice, generated from the new rules
text. When that generation fails or times out, the deterministic fallback
line delivers instead: "Rules noted. The assistant follows the pinned
rules of this group." Every real change is confirmed one way or the other,
however quickly the changes follow each other; re-pinning the same text
says nothing, because nothing changed.

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

## The report setup

When a member replies to an offending message and asks the assistant for a
report, the assistant files one with the group's moderation bot: it sends
the fixed `/report@<handle>` line as a reply to the reported message, at
most once per group within the report window, and confirms the filing in
its answer. The handle comes from the `moderation_handle` configuration
key; with the key absent, the report tool does not exist: a report ask is
answered as ordinary conversation and nothing is ever filed. What the
assistant says in that answer is the model's own prose — no mechanism
scripts a "cannot report" line.

Changed 2026-08-24, with the autonomous-moderation unit: the report is now
the assistant's own assessment. It judges each group message against the
group's pinned rules and files the same `/report@<handle>` reply when a
message clearly violates them — nobody asks, and member-initiated
reporting is removed. Each message is reported at most once, ever, in
place of the earlier per-group report window. The capability exists only
when the `moderation_handle` key is set AND the `answering` key is
`helpful` (the default): in `addressed` mode the assistant does not read
the messages it would judge, so the tool does not exist there either. The
group's administrators judge every report, exactly as before; the
assistant takes no action of its own (decision 0070).

Three platform-side switches make the filing reach the moderation bot, and
all three are the operator's to set:

1. **Bot-to-bot communication is enabled for the assistant** in the
   platform's bot management surface, so the assistant's messages reach
   another bot at all.
2. **The moderation bot's bot-to-bot setting is opened to all bots**, so it
   reads the assistant's report command.
3. **The assistant is NOT a group administrator.** The moderation bot
   ignores administrators' reports, so an administrator assistant files
   into silence. Keep the assistant an ordinary member and turn its privacy
   mode off instead of promoting it.

One unknown, stated plainly: whether the moderation bot honors a report
filed by a bot at all is undocumented on the platform. The first live
filing settles it; until then, treat the report path as unproven in
production and verify the moderation bot's reaction after the first real
report.

## The deletion mirror

Added 2026-08-23. When a group administrator replies to a message with the
moderation bot's own deletion command — a reply `/del` — both bots read the
same command: the moderation bot deletes the message in the chat, and the
assistant erases its stored copy of that message, silently. The assistant
adds no second answer, because the administrator addressed the moderation
bot; only administrators' commands count, exactly as the moderation bot
ignores everyone else's. Nothing to configure: the mirror rides the
moderation bot's own command, with or without the report setup above — no
switch exists, and the bounds below are its only limits.

The constraint, stated plainly: the assistant must SEE the command, so only
deletions issued as a reply `/del` reach it, and only the bare token: a
`/del@...` suffixed with the moderation bot's handle is aimed at that bot
by name, reads to the assistant as another bot's command, and mirrors
nothing — the assistant strips only its own handle from a command. The
moderation bot's other
forms — bulk purges, and direct removals through the platform's own
interface — produce nothing the assistant can read and leave the stored
copy in place. For those, the person-wide deletion commands of the privacy
route remain the way to clear the store.

## The privacy command

`/privacy` — bare, or suffixed with the assistant's own handle — answers
with the configured privacy policy address, deterministically and without a
model turn. The answer goes out at most once per group within the same
window the rules acknowledgment uses; repeats inside the window are read
and left unanswered. The platform additionally offers a bot-level privacy policy
field in its bot management surface; filling it is deployment wiring, kept
in the deployment notes, and does not replace the command.
