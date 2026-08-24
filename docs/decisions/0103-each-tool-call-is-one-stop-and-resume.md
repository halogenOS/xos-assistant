# 0103 — Each tool call is one stop and one resume

Date: 2026-08-24

## Context

Keying the composing cue on the turn phase means a turn with tool calls yields
more transitions than the old all-turn signal did.

## Decision

The cue begins when the model starts, stops when a tool call goes out, resumes
when its result returns the turn to the model's thinking and streaming, and
stops for good when the answer commits — one begin/stop pair around each
tool-execution window, exactly mirroring what is happening. This is the intended
behavior, not flicker to suppress. Everything else the edge does is unchanged:
the once-per-transition dedup, the lost-stop lifetime deadline and its re-begin,
the lag answer that stops every open signal, the channel resolution and its
swallowed read errors. Only the boolean the edge computes from each event
changes.

## Rejected alternatives

- **Collapsing a tool call's stop-then-resume into one continuous signal.** That
  is the all-turn behavior this unit exists to end.
