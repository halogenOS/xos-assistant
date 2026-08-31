# 0179 — recognising a deletion command and acting on it are two readings

Date: 2026-08-31, with the editing unit.

## Context

The moderation bot's deletion token is not a catalogue command, so the
command stamp reaches it through the mirror alone: one predicate answered
both "does this message name the moderation bot's deletion command" and
"does the mirror act", and the ingestion's stamp read the second to decide
the first.

## Decision

They are split, both halves in the mirror module. The recognition keeps the
existing body — the deletion token as the reported command, a group channel,
a reply naming a stored message, a sender at or above the administrator
floor — and the mirror's action is that recognition gated on the message
revising nothing. The ingestion reads the recognition ONCE: the stamp reads
it, and the action is derived from it. So a deletion command arriving as an
edit is marked as a command, takes no debt, spends no budget slot and stays
silent, which is what an administrator addressing the other bot should get
either way.

## Rejected alternatives

- **Leaving the two joined.** An edited deletion command would become an
  ordinary summoned message — a model turn on a command aimed at another
  bot, in a group where the assistant is meant to be invisible for these.
- **Adding the deletion token to the privacy command family to recover the
  stamp.** The family is a rights mechanism with its own answer windows and
  its own suppression exemption, and a moderation command is neither.
