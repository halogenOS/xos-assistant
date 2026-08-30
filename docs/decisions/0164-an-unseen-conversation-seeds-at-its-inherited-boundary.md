# 0164 — The outbound edge seeds an unseen conversation at its inherited boundary

Date: 2026-08-30, with unit 45.

## Context

The outbound edge keeps one cursor per conversation: the highest block id it has accounted
for. When the edge is taken it seeds every mapped conversation at its newest stored block, so
nothing already stored is ever delivered. A conversation the edge meets for the first time
afterwards had no cursor and started at zero, on a premise stated in the module: all of its
blocks postdate the edge.

Forks break that premise. A compacted fork inherits its source's history through the
junction, so its oldest blocks are answers the edge already delivered from the source.
Seeded at zero, the edge would send every kept answer to the group a second time, and the
first-delivery disclosure resolution would write its line INTO those blocks — an edit through
a fork, reaching a block the source still holds, which is the one thing detaching exists to
avoid.

## Decision

A conversation with no cursor seeds at its INHERITED BOUNDARY: the newest of its blocks that
another conversation also holds, or zero when it holds none.

A junction row is what makes a block part of a conversation, so a block two conversations
hold is a block this one was forked with; ids ascend along junction order, so every inherited
block sits at or below the newest shared one and every block the conversation authored for
itself sits above it. The partition is exact, it needs no new state and no moment anyone has
to catch, and it makes the module's premise true again instead of narrowing it. A
conversation created fresh shares nothing with anybody and seeds at zero, so a wiped
channel's first answer delivers normally.

The framework's durable ratchet cursor is deliberately NOT the seed, and the reason is worth
recording because it reads like the obvious candidate: at the instant of a fork it holds
exactly the inherited boundary, capped at what the source had confirmed. But it is the
frontier of what the model has been driven through, it advances with every turn, and by the
time a completed stream wakes this edge it already stands past the very answer the wake is
about. Reading it at delivery time would swallow that answer.

This also repairs a latent case the retire walk had: a channel retired at startup after the
edge was taken forked into a conversation the edge had never seen, and would have re-sent its
inherited answers on the fork's first wake.

## Rejected alternatives

- **The durable ratchet cursor, read when the edge first meets the conversation.** Correct
  only at the instant of the fork; by first delivery it has moved past the answer being
  delivered, and the edge would go silent.
- **Seeding an unseen conversation at its newest stored block.** That is what the edge does
  at startup, where everything stored IS history — but a conversation the edge meets on a
  wake is being woken BY a new block, and this would drop it.
- **Recording the boundary on a block of its own at fork time.** A new durable kind for a
  fact the junction already states exactly, and one more thing to keep true.
