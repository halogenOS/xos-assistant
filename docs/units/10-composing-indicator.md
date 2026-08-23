# Unit 10 — the composing indicator

Date: 2026-08-23. Revision 1. The operator asked for a typing indicator while the
assistant works on an answer. Decision 0064 carries the design and its rejected
alternatives; this unit ships it. No configuration: the indicator is presentation
with no privacy or cost weight, always on.

## The unit's contract

1. **The core exposes the composing edge**, a second per-adapter subscription
   beside the outbound one: one transition when a channel's turn begins, one when
   it ends, derived from the framework's conversation-state event as
   `work_due && !latched` — the dispatch's beginning, closed by completion and
   failure alike, because a completed turn clears the owed work and a failed one
   latches. A deterministic reply never signals: a command-stamped or unaddressed
   message opens no debt, so no turn is owed for it. The signal is live-only —
   no history seeded, nothing stored, nothing owed across a restart; a lag stops
   every open signal and a live turn re-marks itself on its next state change.
   No failure path leaves the edge: a mapping read that fails is logged and that
   transition dropped, because a presence cue must never disturb answering.
2. **The adapter translates the transitions** into the platform's typing action:
   a begin starts the chat's refresh loop — the action re-sent on a named
   interval just under the platform's roughly-five-second expiry — and a stop
   ends it. A delivered answer stops its chat's refresher too, so a stop lost on
   the lossy cue can never outlive the answer it was about. A failed action send
   is logged and swallowed; the loop keeps its cadence and the next tick
   retries. Dropping the run future aborts every refresher with it.
3. **The binary needs no wiring change**: the adapter's run entry takes the
   composing edge beside the outbound edge itself.

## Acceptance criteria

- **AC1** Over the scripted wire: an addressed message that summons a model turn
  records at least one typing action before the answer — proven with the
  provider's turn hold, not a scheduling bet — and none after it, proven by a
  recorded barrier message.
- **AC2** A deterministic reply (the privacy command) records no typing action.
- **AC3** A failing action send leaves the answer's delivery untouched.
- **AC4** The core edge's own contract is pinned in module tests: one begin and
  one stop per turn with repeats deduplicated, latched and foreign-adapter
  conversations yield nothing, and a lag stops every open signal.
- **AC5** Full battery green: build, tests parallel and single-threaded with all
  features, clippy denied, fmt, doc denied, vocabulary and secret scans; no new
  dependency.

## Revision 2 (2026-08-23): the lifetime bound

A live orphan showed the stop transition is at-most-once end-to-end: a lost
stop on an idle conversation left the refresh loop firing for three quarters
of an hour. The refinement to decision 0064 bounds the signal in two layers —
the core edge stops any signal still open at the signal-lifetime deadline on
its own clock (and clears its entry, so the next begin is not swallowed), and
the adapter's refresh loop ends itself after the lifetime in refresh periods
plus slack, with no delivered message required. Contract additions:

- **AC6** Core pin: after a begin with every later event withheld, the stop
  arrives on the edge's own clock inside a bounded await, and a following
  state change re-begins the signal.
- **AC7** Adapter pin: a refresh loop with every stop withheld ends on its
  own cycle bound inside a bounded await, on a paused clock.
