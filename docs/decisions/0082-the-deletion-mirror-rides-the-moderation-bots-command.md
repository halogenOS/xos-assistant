# 0082 — The deletion mirror rides the moderation bot's command

Date: 2026-08-23

## Context

The group's administrators delete offending messages by replying to them
with the moderation bot's deletion command. The chat copy goes, the
assistant's stored copy stays — two stores, one act. Both bots receive the
same reply independently, so the assistant can keep its store honest
without owning a command, asking anyone, or saying anything.

## Decision

A group message whose reported invoked command is the deletion token
(`/del`, the moderation bot's own, a named constant) and whose reply target
names a stored message triggers the mirror when the sender is a group
administrator — the authority the adapter already resolves per message,
where decision 0015 maps the platform's administrator set to the moderator
and admin standings, so the mirror's floor is moderator. The command
message itself is recorded like any command: the request is the lawful
record, its reply reference included. The reply target resolves through the
stored origin within the conversation, exactly as the report tool's target
does, and the named message's row is erased — text, origin, send time,
reply reference and speaker nulled, the same nulls the person's own erasure
applies, scoped to one row. The placeholder stays, projection reads the
erased marker, nothing else moves: the target-keyed reply pass stays a
person-wide operation, so a copy of the deleted message's identifier on the
command row and on other replies remains — the residual class decision
0063's refinements already record.

SILENT: no reply, no acknowledgment. The administrator addressed the
moderation bot, and a second bot answering a command meant for the first is
noise.

## Rejected alternatives

- **A reply from the assistant.** Noise — the admin asked the moderation
  bot, and got its answer there.
- **Admin-checking through the tool authority walk.** This is a
  deterministic command, not a model tool: the sender's resolved authority
  decides, and the decision-0043 interrupt blocker stays untripped.
- **Acting on the model's judgment anywhere.** Decision 0070 routes
  assessment to humans — and here the administrator IS the human decision;
  the assistant's part is bookkeeping.
- **An assistant-owned deletion command.** A second command for the same
  act would drift from the moderation bot's, and admins already speak one.
