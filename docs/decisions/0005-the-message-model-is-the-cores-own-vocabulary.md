# 0005 — The message model is the core's own vocabulary

Date: 2026-08-21

## Context

Adapters and the core need a shared shape for one message crossing the boundary, and the
architecture's two invariants — an adapter contains no behavior, the core contains no
platform vocabulary — decide where that shape may live and what it may say.

## Decision

The core defines its own message model. An inbound message carries a channel key, the
channel kind (direct or group), the sender's identity, an authority level, the message
text, an optional origin reference and a timestamp. The channel key is an opaque pair —
adapter name plus the adapter's own conversation identifier — compared only for
equality. Adapters translate their platform's types into this model at the boundary and
never past it.

## Rejected alternatives

- **Reusing a platform SDK's update type in the core.** Breaks the no-platform-vocabulary
  invariant on day one and welds the core to one SDK's shape.
- **A trait the core calls back into.** Inverts the dependency and gives adapters a
  behavior surface, against the first invariant.
