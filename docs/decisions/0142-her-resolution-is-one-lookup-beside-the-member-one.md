# 0142 — Her resolution is one lookup beside the member one

Date: 2026-08-30, with unit 38.

## Context

A member's reply resolves its target by matching the origin stored on the replied-to
message's own row. One of the assistant's messages has no such row: what names it is the
delivery receipt that recorded which chat message her block became. Everything past that
resolution — the span decision, the hand-selected excerpt's narrowing, the tail-skip,
the user-voiced append — is target-agnostic and reused unchanged.

## Decision

One conversation-scoped query beside the member one, answering an origin with the newest
recorded delivery of that origin that carried a quotable block: the block a span points
at, and the stored text a span is measured against. Both come back together because the
one consumer runs its span decision against the text.

The text is read from the framework's own text table beside the receipt row — the one
place her stored prose lives, under the recorded coupling to the framework's tables. The
disclosure line is written into the stored block before the send, so her stored text is
what the channel saw, and quoting the block is honest about what she said.

Newest, and junction-joined like every origin reader: platform message ids are opaque
and unique only per channel, so a bare match would reach a stranger conversation's row.

An origin the conversation never recorded, and a reply the platform carried no id for,
resolve to nothing and land quoteless exactly as before this unit — nothing is invented.
The same holds for the race the report loses: a member's reply can land before the send's
report appends the row, and that reply's quoteless landing is permanent. The quote
resolves against what the ledger holds when the reply lands, and nothing heals backwards.

Each message of one send carries the same answer block, so a reply to any chunk of a
long answer quotes the whole stored answer.

## Rejected alternatives

- **Per-chunk span narrowing.** The chunks are a transport artifact of the platform's
  message cap; her message is the block, and a member replying to the third chunk is
  replying to what she said.
- **Healing an early reply once the receipt lands.** A quote that changes after the model
  has already read the reply rewrites the conversation's history to fix a race, which
  costs more truth than the quote is worth.
- **Copying her text into the quote block.** The quote stores a span and the store
  resolves it at read time, so an erased target renders nothing with no erasure pass
  needing to know quotes exist. A copy would be a second place her words live.
