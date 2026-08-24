# 0097 — A miss has its own sentinel, routed by the mechanism

Date: 2026-08-24

## Context

On a lookup miss the reaction must be silence when the message did not address
the assistant and a plain admission of not knowing when it did. But the model
cannot see whether a message addressed it — a reply to the assistant leaves no
mark in the projection — so the model cannot make that choice.

## Decision

Two sentinels: the existing abstention sentinel for social silence, which always
delivers nothing, and a new unresolved-lookup sentinel the model emits as its
whole answer when it looked and could not ground an answer. The outbound edge
recognizes the miss sentinel on the raw answer, before any disclosure prepend,
and routes it by the literal addressed fact of the message that summoned the
turn: unaddressed delivers nothing; addressed delivers a fixed plain
don't-know line, which flows through the disclosure fold like any first answer.
The model's only job is to be honest that it found nothing; the machine, which
holds the fact, decides whether the asker is owed a reply. Both sentinels are
suppressed from the projection so neither pollutes a later request.

## Rejected alternatives

- **The model choosing silence versus don't-know.** It cannot see the
  reply-addressed channel, so it would misclassify that whole channel.
- **Marking the addressed fact into the projection for the model to read.** It
  leaks the internal flag and still rests on model discipline for the choice.
- **One sentinel for both social silence and miss.** The mechanism could not
  tell "nothing to add" from "found nothing", and would wrongly answer
  don't-know to an addressed social remark.
