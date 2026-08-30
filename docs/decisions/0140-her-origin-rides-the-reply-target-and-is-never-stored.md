# 0140 — Her origin rides the reply-target variant and is never stored

Date: 2026-08-30, with unit 38.

## Context

The platform reports which message a reply points at whoever wrote it, and the adapter
already decodes that id. Translation dropped it for one case only: a reply to one of the
assistant's own messages, which crossed into the core as a bare fact with no origin,
because nothing downstream had a use for one. Quoting her words is that use.

The column the other variant is stored in carries its own documentation: it is NULL for
a reply to one of the assistant's own messages, and its values are two people's personal
data. The erasure pass that scrubs references to an erased person's messages joins
against it.

## Decision

The reply-to-assistant variant carries the origin the platform reported, optional
because the wire field is. Translation fills it from the id it already decodes. The core
consumes it during ingestion — it resolves which of her recorded messages the reply
points at — and the chat message's stored fields keep writing exactly what they wrote
before: the reply-to-assistant flag, and no reply target.

So the column's documentation stays true and unedited, which is the point of not storing
the value. The no-origin decision family is amended in its rides half only; the
deletion mirror's own reading of this variant is untouched and stays for the unit that
consumes the delivery record.

## Rejected alternatives

- **Storing her origin in the reply-target column.** Its documentation states verbatim
  that the column is NULL for this case and classifies its values as two people's
  personal data, and erasure joins against it as member-message references. A column
  that sometimes holds the assistant's own id makes both statements false.
- **A third column for it.** A stored fact nothing reads.
- **A parallel field on the inbound message, beside the empty variant.** Two ways to say
  one thing, and the second one drifts.
- **Keeping the variant empty and resolving her message by recency.** The wrong message,
  silently, the moment a second answer exists.
