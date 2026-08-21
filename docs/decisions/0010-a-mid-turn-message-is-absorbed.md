# 0010 — A mid-turn message is absorbed, not individually answered

Date: 2026-08-21

## Context

A message can arrive while an answer is still streaming. The framework fires a turn only
when the newest block awaits the model, so once the streaming answer finalizes, that
message sits behind the answer and no turn is owed for it.

## Decision

The absorbed message draws no turn of its own; it joins the context of the next turn
instead. The unit pins this behavior with a test that states the observed order.

OPEN, surfaced to the framework's improvements batch: a conversation whose newest block
is a finalized answer never reconsiders buried messages until the next append, which can
leave a trailing message unanswered indefinitely in a quiet channel. The follow-up
decision — a post-finalize reconsideration or similar — belongs to the framework, not to
this project.

## Rejected alternatives

- **Re-driving a turn from this side after finalization.** The one-turn condition is the
  framework's scheduling law; working around it in a consumer would fork the semantics
  the two projects are supposed to share.
