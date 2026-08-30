# 0145 — Her own message ids need no privacy-document change

Date: 2026-08-30, with unit 38.

## Context

The unit stores something new: one row per message the assistant sent, holding the
platform's id for that message and the key of the send it belonged to. Every unit that
stores something new asks whether the published privacy documents still describe what
the assistant does, because assuming the answer is how a processing description quietly
stops matching the software.

## Decision

The stored ids name messages the assistant's own software wrote. They carry nothing of
anybody: no person, no message content of anyone's, no identifier of a member, and no
new recipient — the rows never leave the process and reach no model. The operator said
so in the same breath as the order to build this, and the reading is checked against the
published documents rather than accepted from this paragraph: no published sentence
contradicts the new rows, so none of the documents changes.

The rows are structure, not personal data, and their lifetime follows from that: a
person's erasure leaves them, and the conversation's own deletion removes them through
the block cascade, with no cleanup pass of their own.

## Rejected alternatives

- **Assuming it silently.** A unit that stores something new and records nothing about
  privacy leaves the next reader to re-derive the reasoning, and the reader after that
  to skip it.
- **Amending the documents anyway, to be safe.** A processing description that lists
  things it does not process is as wrong as one that omits things it does, and it trains
  readers to skim.
- **Treating her message ids as personal data because a member's are.** A member's
  message id names something a person wrote and is one half of a reference between two
  people; hers names a message her software wrote, and there is no person on either end.
