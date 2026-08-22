# 0021 — Addressing is translated by the adapter and decided at the write

Date: 2026-08-22

## Context

Unit 1 shipped a stopgap: every recorded message awaited the model (decision 0007).
The live model needs the real acting policy — record all, answer some — and a seam
that says which messages get answered.

## Decision

What "addressed" means on a platform — a direct chat, a mention of the bot's
username, a reply to one of the assistant's messages — is platform knowledge; the
adapter resolves it (bot identity from the identity call, fetched before the first
poll with the poll's own backoff, no message translated before it succeeds) and the
inbound message carries the neutral fact. The kind stores it.

Because the framework owes a turn from the newest block alone, the stored fact that
summons the turn is stamped by the entry point at the write: a message's answer-due
fact is true when the message is addressed or when the block behind it carries an
unanswered answer-due — so an unaddressed message arriving on the heels of an
addressed one propagates the debt instead of cancelling it. The stamp is a decision
recorded once at insert, in the access-model tradition of provenance stamps; it is
not a derivable-fact column, because the per-block hook that consumes it cannot fold
history. Both columns are structure, not personal data — erasure leaves them.

Refined 2026-08-22 and closed the same day: a machinery-walking tail read was
implemented so absorbed addressed messages would chain the summoner's debt for
tool provenance, and was then refuted by the unit's second verification —
stored shape cannot tell a turn's narration from a turn boundary, and the
walk's cap invented debts for bystanders. The stamp keeps the tail-only read:
an addressed message absorbed mid-turn opens a fresh debt at its own
authority, which is correct for ANSWERING (it is answered by the next turn)
and is no longer load-bearing for provenance, since tool registration floors
required authority at member until the framework's dispatch anchor ships
(decision 0043's closure).

The stamp's reading is shared with the awaiting hook, and an erased tail owes
nothing: a message whose text was erased (decision 0012) carries no debt to
propagate, so erasure cancels a pending answer instead of summoning a turn over an
erasure marker.

This replaces the unit-1 stopgap (decision 0007).

## Rejected alternatives

- **The core parsing mentions.** Platform vocabulary in the core, against the
  invariant.
- **The adapter dropping unaddressed messages.** Record all, answer some — the
  group's memory is the product.
- **A framework change to fold history at the frontier.** The write-time stamp
  expresses the policy in the seams that exist.
