# 0025 — A failed turn tells the chat once, at most once

Date: 2026-08-22

## Context

Before this unit a failed turn was silent: the conversation latched and the chat saw
nothing. The stage plan owed the user-facing failure behavior.

## Decision

On a stream error the conversation latches and the outbound edge yields a failure
notice — marked as a notice, one per failed turn, no model prose — and the adapter
sends one short plain line. The notice is derived from the bus event, and the bus is
lossy: the notice is at-most-once by construction, stated plainly — a lagged edge
may drop it, and a late error from a torn-down predecessor stream may produce a
spurious one; both are accepted for a courtesy line. The durable record of failed
turns is framework work, already on the improvements list as recording dead turns.

One uniform notice text: no distinct budget wording, because the wire flattens the
refusal to prose before the core sees it — the latch already stops spend, which is
the substance.

## Rejected alternatives

- **Silent failure.** The status quo, now user-visible.
- **Blind automatic retry.** Spends without consent and can loop.
- **A notice classified by string-matching provider prose.** A coupling to wording
  nobody owns.
