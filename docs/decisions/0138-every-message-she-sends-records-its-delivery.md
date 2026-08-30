# 0138 — Every message she sends records its delivery, answers naming their block

Date: 2026-08-30, with unit 38.

## Context

The platform answers a send with the sent message, ids included, and both send paths
threw that answer away. So the ledger knew what the assistant said and never which chat
message she said it as, and a member replying to one of her messages could not be
matched to the words they replied to. Their reply landed as a free-standing sentence.

The delivery receipt was already designed whole, in a committed and unbuilt spec: one
block per platform message that reached the chat, holding the origin and the delivery
key, across both send paths. What that shape lacks for quoting is exactly one value —
a receipt maps an origin to a delivery key, and a quote's endpoint is a block id.

## Decision

The wire client returns the ids the platform gave the messages it took, on the whole
send and on the error of a cut-short one. After each send the adapter reports the
platform fact through a core entry point beside ingestion and observation, and the core
appends one delivery receipt per delivered message: the origin, the delivery key — the
send's own first id, minting no new identity — and the block a reply to that message
quotes, where the send carried one of her blocks. Deterministic items, the failure
notice and a report's line name no block, and their receipts hold nothing there.

One send reports at most once by construction, and the argument is recorded here rather
than indexed. An outbound reply is consumed once: the cursor advances on the send and
reseeds at the newest block on restart, so nothing re-sends. A redelivered update's
dedupe yields no second delivery. A crash between the send and the report loses the
row, which is the accepted quoteless case. The indexes are therefore not unique, and
the newest-row resolution tolerates a duplicate if one ever appears.

## Rejected alternatives

- **An answers-only subset of the seam.** The recorded design's coverage is total, and
  building half of it leaves the other half colliding with this one later.
- **A side table beside the ledger.** Blocks cascade with a deleted conversation and
  need no cleanup pass of their own; a table keyed on the answer's block id does not,
  and deterministic items have no answer block to key on.
- **Building the whole recorded design here.** Consuming the record — taking one of her
  messages back from the chat — is its own unit and waits for it.
- **An adapter-local map from an answer to its ids.** Lost on restart, and it makes the
  adapter hold state it would then have to reason about, which is behaviour in an
  adapter.
