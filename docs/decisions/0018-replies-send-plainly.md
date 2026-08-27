# 0018 — Replies send plainly

Date: 2026-08-21

## Context

The core's outbound reply carries a channel key and text — no origin reference yet,
though the ledger stores the origin column for exactly that future use. The platform
could thread a reply onto the message it answers.

## Decision

The adapter sends the reply's text to the chat, unthreaded. Reply threading is deferred
until the outbound edge carries the origin; wiring a guess now would thread every reply
onto the newest message, which is wrong in a busy group.

## Rejected alternatives

- **Extending the outbound edge in this unit.** A core change the adapter invariant
  says an adapter must never need.

---

Amended 2026-08-23: the deferral falls due with decision 0059 — the outbound edge
now carries an optional reply target and the adapter threads onto it with
send-without-reply tolerance, first chunk only. The judgment of this record stands
for the model's ANSWERS: they still send plainly, and only the report's delivery
threads.

Amended 2026-08-24: the answer clause above is superseded by decision 0106. An answer
threads onto the one message the turn absorbed that literally addressed the assistant,
and only when exactly one did; every other case still sends plainly. The reason this
record refused threading is intact and is what shapes the new rule: threading onto the
newest message, or onto the summoning frontier, quote-replies a bystander in a busy
group. Neither is what the new rule reads — it reads the addressed fact stored on each
absorbed message, and a turn with no single addressed message keeps this record's plain
send.
