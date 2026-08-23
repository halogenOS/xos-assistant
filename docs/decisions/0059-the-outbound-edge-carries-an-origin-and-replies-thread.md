# 0059 — The outbound edge carries an origin, and replies thread

Date: 2026-08-23. The deferral of decision 0018 falls due.

## Context

Decision 0018 sent replies plainly and deferred threading until the outbound
edge carries the origin of the message a reply answers. The report needs
exactly that: its fixed line acts on the moderation bot only as a reply to
the reported message.

## Decision

The outbound reply gains an optional reply target carrying the platform
origin of the message it answers. The adapter translates it into the
platform's reply parameters — the current primitive; the old reply field was
replaced two platform versions ago — with send-without-reply tolerance, so a
deleted target degrades to a plain send. The chunking rule threads only the
first chunk.

On the inbound side, the message gains the reply target's origin as a
translated field beside the addressed flag — a reply to another person's
message carries that message's id as the opaque origin, a reply to one of
the assistant's own messages carries that fact instead, and a reply without
a usable id carries nothing. Both facts are stored on the chat-message row
by an appended migration step: the origin under the same author-keyed
erasure null as the existing origin column, the assistant-reply fact as
structure erasure leaves.

**The model's answers stay unthreaded in this unit** — decision 0018's
judgment stands; the field exists for the report's delivery and whatever
answer-threading decision comes later.

## Rejected alternatives

- **Threading answers now.** A product-texture change smuggled into a
  plumbing unit; the operator's call.
