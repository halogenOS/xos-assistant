# 0030 — Protection limits answering, never recording

Date: 2026-08-22

## Context

A public group exposes the assistant to floods. The record-all policy is the product
— the group's memory — so what a flood can exhaust is the answering path: model
spend and group attention.

## Decision

No protection mechanism may drop a message from the ledger. The limits act at the
write, on the stamp: an over-limit addressed message is recorded with its addressed
fact true, a limited fact naming the refusing budget, and an answer-due fact
composed as

    answer-due = (addressed and not limited) or tail-owes

The budget refuses only the debt the message itself would open, never a debt it
propagates: a flooder can be refused their own answer but can never cancel someone
else's. The limited fact reads as "this message's own debt was refused"; a true
answer-due beside it is a propagated debt, not a contradiction.

## Rejected alternatives

- **Dropping over-limit messages at the adapter.** Breaks record-all and pushes
  policy into the adapter, against the no-behavior invariant.
- **Limiting at the provider.** Spends the turn the limit exists to save.
- **A flat answer-due false on limited messages.** Cancels a propagated debt — the
  lost answer decision 0021 forbids: an innocent sender's owed answer would vanish
  because a flooder wrote behind it.
