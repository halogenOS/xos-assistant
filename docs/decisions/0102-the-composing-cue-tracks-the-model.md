# 0102 — The composing cue tracks the model, not the whole turn

Date: 2026-08-24

## Context

The composing (typing) cue was derived as `work_due && !latched`, and
`work_due` stays true for the whole turn — through every tool call — until the
answer commits. So the cue held through tool execution, showing "typing…"
across a multi-second lookup against an external service, which is not the
assistant composing.

## Decision

The cue is on while the model is composing — its thinking and its streaming —
and off for the windows that are not the model composing. The framework's
conversation-state event already carries the phase in its `awaiting` field, and
the state broadcaster re-emits on every outcome change, so the transitions are
on the bus the edge reads. The thinking window is `awaiting == Model`; the
streaming tail awaits nobody, so `awaiting == None`; a pending tool call is
`awaiting == System`; a human owing a reply or approval is `User` / `OutOfBand`.
The derived predicate becomes `work_due && !latched && !matches!(awaiting,
Some(System | User | OutOfBand))` — a strict narrowing of the all-turn behavior
that removes exactly the tool-execution and human-wait windows and leaves
thinking and streaming on as before.

## Rejected alternatives

- **`awaiting == Some(Model)` alone.** The streaming tail awaits nobody
  (`awaiting == None`), so this leaves the cue dark through the whole streamed
  answer — the exact phase "typing" must cover.
- **Excluding only `System` and leaving the cue on for `User` / `OutOfBand`.**
  "On while a human is owed" is the same misrepresentation as "on during a tool
  call"; both are not the model composing.
- **A per-tool timer or an adapter-side debounce.** The phase is a framework
  fact already on the bus; deriving the cue from it is exact, a timer guesses.
