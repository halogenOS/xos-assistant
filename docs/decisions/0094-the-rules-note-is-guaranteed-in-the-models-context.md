# 0094 — The rules note is guaranteed in the model's context

Date: 2026-08-24

## Context

The model can only judge against rules it can see. The pinned rules reach it
as a context note in the conversation's stream; if a projection ever trimmed
history to a window, the rules could scroll out of the very request that
assesses a message against them — a silent moderation failure.

## Decision

The newest rules note is present in every projected request while a rules
note exists, the way the system prompt is. Verified rather than built: the
framework's projection folds the conversation's whole loaded ledger — no
window trims history — and a note is a durable block, so every stored rules
note rides every later request in stream order with the newest authoritative
under the supersession wording. The guarantee is stated on the note kind's
projection and pinned by a test that reads the note out of the request the
model assesses on; a future windowed projection must keep the pin green by
pinning the rules note the way it pins the system prompt.

## Rejected alternatives

- **Assessing against rules that may have scrolled out of context.** The
  silent failure this decision exists to exclude: the model would judge from
  memory of rules it no longer sees, or not judge at all.
- **Re-rendering the rules into the system prompt block.** Rejected when the
  note kind was decided (2026-08-23) and still: the prompt is recorded once
  at creation, and rules change while a conversation lives.
