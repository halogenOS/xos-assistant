# 0064 — The composing signal rides the turn lifecycle, always on

Date: 2026-08-23

## Context

The operator asked for a typing indicator while the assistant works on an
answer. The invariants bind as everywhere: the adapter contains no behavior,
the core contains no platform vocabulary. The core already broadcasts its
turn lifecycle — the conversation-state event carries whether a turn is owed
and being worked (`work_due`) and whether the conversation is latched — and
the outbound edge shows what a per-adapter subscription looks like.

## Decision

The core exposes a second per-adapter subscription, the composing edge: one
transition when a channel's turn begins, one when it ends. Composing is
derived as `work_due && !latched` from the conversation-state event — the
dispatch's beginning, ahead of any provider traffic — and both ends of a
turn close it through the same derivation: a completed turn clears the owed
work, a failed one latches. A deterministic reply never signals by
construction, because a command-stamped or unaddressed message opens no
debt and therefore no turn. The signal is live-only: nothing stored, no
history seeded, nothing owed across a restart; a lag stops every open
signal and a live turn re-marks itself. The adapter translates the
transitions into the platform's typing action, refreshed on a named
interval just under the platform's own expiry, stopping when the signal
ends or the answer sends; a failed action send is logged and swallowed.
There is no configuration: the indicator is presentation with no privacy
and no cost weight, and always on.

## Rejected alternatives

- **An indicator on deterministic replies.** They are answered from the
  ingestion call's return and send instantly; nothing is being composed,
  and an indicator would announce work that does not exist.
- **A configuration toggle.** Configuration carries decisions with privacy,
  cost or deployment weight; a presentation cue carries none, and a knob
  would be surface without a decision behind it.
- **Deriving from the streaming-plane events.** The first stream event
  arrives only once the provider speaks, so the queueing and dispatch wait
  — exactly the silence the indicator exists to cover — would show
  nothing, and the shape would lean on per-provider status emission the
  framework does not promise.
- **Delivering the signal over the outbound reply edge.** That edge is
  at-least-once from stored state with a cursor; a presence cue is
  live-only and must never be replayed — a stored "composing" re-delivered
  after a restart would show typing with no turn running.
- **Rendering the indicator in the driver from ingest outcomes.** The
  adapter would be deciding when the assistant is working — behavior in an
  adapter, and wrong the moment a turn is summoned by anything but the
  message the driver just ingested.

## Refinement (2026-08-23): every signal carries a lifetime bound

A live orphan proved the residual real: a turn ended, its stop was lost —
the stop travels at most once end-to-end, and the ended conversation then
idled, so no later state change could heal it — and the adapter's refresh
loop kept re-sending the typing action for three quarters of an hour on an
idle chat. The refinement removes the dependency on a stop arriving at all,
in two layers:

- **The core edge bounds the signal.** Every begin carries a deadline, the
  signal lifetime constant (five minutes, generous against any real turn);
  a signal still open at its deadline is stopped on the edge's own clock,
  and the expiry clears the edge's entry, so the next genuine begin is not
  swallowed by stale bookkeeping. A turn genuinely running past the
  deadline re-begins on its next state change, with a fresh deadline.
- **The adapter's refresh loop bounds itself.** The loop runs at most the
  signal lifetime in refresh periods, plus slack, then ends unconditionally
  — a bound that needs no message delivered by anyone. The lifetime is the
  core's exported constant: how long the cue may live stays the core's
  decision, and the loop only obeys it.

Rejected alternatives:

- **Re-deriving the owed turn from the ledger before each re-send.** The
  owed-turn derivation is the scheduler's alone — a second reader would
  either duplicate it or run the ratchet from outside its single driver —
  and placing the check in the adapter would put a decision in a component
  that must hold none.
- **Only healing on the next state event.** The edge already stops an open
  signal when a later state change shows the turn ended; the orphan proved
  the case where no later event ever comes, which no event-driven heal can
  cover.
- **Only the adapter-side bound.** It ends the orphaned loop but leaves the
  edge's own entry stale, silently swallowing the conversation's next
  begin; and a second adapter would have to reinvent the bound.
