# 0105 — The fixed line is the acknowledgment's deterministic fallback

Date: 2026-08-24

## Context

The fixed acknowledgment of decision 0051 delivered on every real delta,
without exception. A model call can fail, time out, return empty, run past
any sensible length, or answer with the abstention or miss sentinel — and a
rules change that draws silence would be a regression against the guarantee
the fixed wording carried.

## Decision

The fixed line survives as the fallback, not the primary. When the bounded
completion of decision 0104 fails, times out, closes without its terminal
done, crosses the output cap, or returns only whitespace or a machinery
sentinel, the deterministic line delivers instead — so a real rules delta
ALWAYS draws a visible acknowledgment. The model call improves the wording;
it never weakens the guarantee. Every fallback is recorded in the log with
its cause.

## Rejected alternatives

- **No fallback.** A real rules change could produce no acknowledgment at
  all — the silent-swallow regression the probe found in the withdrawn turn
  design, reproduced by other means.
- **Delivering a truncated or sentinel result.** A cut-off sentence or a
  machinery token in the chat is worse than the fixed line.
