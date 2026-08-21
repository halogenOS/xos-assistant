# 0019 — An over-cap reply goes out in chunks

Date: 2026-08-21

## Context

The platform caps one message's text at 4096 UTF-16 code units; a longer `sendMessage`
call is refused. The core finalizes answers with no length bound, so a reply can carry
more text than one message accepts. Decision 0018 sends the reply's text plainly and
unthreaded; this decision states what happens when that text does not fit one message.

## Decision

The client splits the text into consecutive chunks, each within the cap, split on
character boundaries, and sends them in order — one core answer can reach the chat as
several messages. A chunk that fails ends the reply at the last delivered chunk:
sending the tail after a lost middle would deliver a spliced statement, so the rest is
dropped with the failed chunk, the same logged-and-dropped outcome the send-failure
rule accepts for a whole reply.

## Rejected alternatives

- **Treating an over-cap reply as a failed send: log and drop the whole reply.** The
  assistant would fall silent exactly when it has the most to say, and neither the
  chat nor the ledger would show why. The send-failure rule covers sends the platform
  or the network refuses, not text the client can deliver by honoring a documented
  cap.
- **Truncating to the cap.** Discards the answer's tail while presenting the rest as
  complete — a silent misquote of the core's finalized answer.
- **Splitting on sentence or paragraph boundaries.** Kinder split points, but the
  rule needs language-aware boundary detection plus a character-boundary fallback for
  a single over-long paragraph anyway; the plain character-boundary split is exact,
  small and testable, and a nicer splitter can replace it inside the client without
  touching any contract.
