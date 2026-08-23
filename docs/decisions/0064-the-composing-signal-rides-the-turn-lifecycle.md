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
