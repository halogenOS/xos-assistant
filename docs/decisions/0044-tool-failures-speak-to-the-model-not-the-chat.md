# 0044 — Tool failures speak to the model, not the chat

Date: 2026-08-22

## Context

A lookup can fail — network, rate limit, not found, timeout — and something has
to hear about it. The chat and the model are different audiences.

## Decision

A failed lookup returns its error as the tool result the model sees, and the
model answers with what it has; the chat never receives a raw error. A turn
where the model narrates before calling a tool sends both texts to the chat —
both are the assistant speaking, and the platform already receives
multi-message replies under the chunking rule; accepted and pinned. Timeouts
are per-tool construction parameters with named-constant defaults, so tests
construct short ones instead of waiting production bounds.

## Rejected alternatives

- **Tool errors as failure notices.** The notice is for a failed turn; a turn
  whose tool failed still completes with the model's own answer.
- **Suppressing pre-tool narration.** An outbound rule against the assistant's
  own words — a filter on the assistant's voice the product has no reading for.
