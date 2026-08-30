# 0139 — The delivery receipt is bookkeeping and no reader ever meets it

Date: 2026-08-30, with unit 38.

## Context

The receipt is a real block in a real conversation, appended by an independent path at
an arbitrary moment — the moment the platform took a message. Every walk over the
ledger therefore meets it: the projection that builds the model's context, the owing
tail that decides whether a question still stands, and the dispatch frontier that
decides whether a turn is owed.

## Decision

The receipt projects nothing to the model, summons nothing, is transparent on the
dispatch frontier, and joins the owing-tail walk's read-through list — the consumer's
own list for the consumer's own kinds.

The read-through membership is load-bearing on day one, not a precaution for later. A
failed turn's failure notice records its own delivery at the tail, directly over the
question that failed; an opaque tail there would answer the debt walk with a settled
reading and bury the standing question behind it.

One consequence is stated rather than discovered. A block that says nothing to the model
still ends the contiguous run of same-voiced blocks before it — the framework's own
contract for every record kind, the report's included. So a receipt landing between two
of one person's messages leaves the model reading two messages where it would have read
one merged. The text is identical and its order is identical; only the grouping differs.
That is the accepted cost of recording the delivery on the ledger where it cascades with
its conversation.

## Rejected alternatives

- **The silent-but-opaque group, beside the report kind.** That arm is exactly what
  buries a standing debt behind a receipt, and the failure notice puts one there on the
  first day.
- **Deferring the membership to the unit that consumes the record.** The row lands the
  moment this unit ships; a walk it breaks is broken from that moment.
- **Projecting a line about the delivery.** A truthful line would have to name the
  message it recorded, which means projecting the assistant's own message origins — a
  change with its own privacy reading, and no reader wants it.
