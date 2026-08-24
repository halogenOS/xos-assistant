# Unit 18 — the composing cue tracks the model, not the whole turn

Date: 2026-08-24. Revision 1. The composing (typing) cue should show while the
assistant is thinking or streaming an answer, and stop while a tool call runs —
a lookup against an external service is not the assistant composing, and showing
"typing…" through a multi-second HTTP fetch misrepresents what is happening. The
cue currently spans the whole turn: it is derived as `work_due && !latched`, and
`work_due` stays true for the entire turn — through every tool call — until the
answer commits. So the cue holds through tool execution, the exact case to fix.

## Grounding

The framework already carries the phase the cue needs. The conversation-state
event the composing edge reads (`CoreEvent::ConversationState`) carries an
`awaiting` field beside `work_due` and `latched`, and the state broadcaster
re-emits the event whenever the scheduler's outcome changes — including when
`awaiting` changes mid-turn — so the transitions are observable on the same bus
the edge already subscribes to. The `Awaiting` values are the phase: `System` is
"an unresolved tool call, a pending request" — the tool-executing window;
`Model` is "a model turn is warranted" — the thinking window ahead of the
stream; the streaming tail itself awaits nobody, so `awaiting` is `None` once
the provider's first delta lands; `User` / `OutOfBand` are a human owing a reply
or an approval. So the two phases "typing" should cover — thinking and streaming
— are `Some(Model)` and `None`, and the two it should not — a tool call and a
human wait — are `Some(System)` and `Some(User)`/`Some(OutOfBand)`. The edge
today discards `awaiting` entirely.

## Decisions taken with this unit

- **The composing cue is on while the model is composing — thinking and
  streaming — and off for the not-the-model windows, 2026-08-24.** The
  derivation gains the phase: the cue is on when the turn is owed and unlatched
  AND the frontier is not in a not-the-model window, so it holds through the
  thinking window (`awaiting == Model`) and the streaming tail (`awaiting ==
  None`, the stream awaits nobody), and drops while a tool call is unresolved
  (`awaiting == System`) and while a human is owed a reply or an approval
  (`awaiting == User` / `OutOfBand`). Concretely the derived predicate becomes
  `work_due && !latched && !matches!(awaiting, Some(System | User | OutOfBand))`,
  replacing `work_due && !latched` — a strict narrowing of the old all-turn
  behavior that removes exactly the tool-execution and human-wait windows and
  leaves thinking and streaming on as before. This keys off the true framework
  mapping, verified against it: the thinking window publishes `Model`, a pending
  tool call publishes `System`, and the streaming tail publishes `None` — so
  keying on `== Some(Model)` alone would leave the cue dark through the whole
  streamed answer, the exact phase "typing" must cover. Rejected: `awaiting ==
  Some(Awaiting::Model)` (drops the streaming tail, which awaits nobody — the cue
  goes dark while the answer streams); excluding only `awaiting == System` and
  leaving the cue on for `User`/`OutOfBand` ("on while a human is owed" is the
  same misrepresentation as "on during a tool call"); a per-tool timer or a
  debounce in the adapter (the phase is a framework fact already on the bus —
  deriving the cue from it is exact, a timer is a guess).
- **Each tool call is one stop and one resume, and the existing edge machinery
  is untouched, 2026-08-24.** Keying on `awaiting` means a turn with tool calls
  now yields more transitions: the cue begins when the model starts, stops when
  a tool call goes out, resumes when its result returns and the model runs
  again, and stops for good when the answer commits — one begin/stop pair around
  each tool-execution window, exactly mirroring what is happening. This is the
  intended behavior, not flicker to suppress. Everything else the edge does is
  unchanged: the once-per-transition dedup (begin only when not already open,
  stop only when open), the lost-stop lifetime deadline and its re-begin, the
  lag answer that stops every open signal, the channel resolution and its
  swallowed read errors, and the adapter's refresh bound. Only the boolean the
  edge computes from each event changes. Rejected: collapsing a tool call's
  stop-then-resume into one continuous signal to avoid transitions (that is the
  current all-turn behavior this unit exists to end).

## The unit's contract

The composing edge's derived predicate changes from `work_due && !latched` to
`work_due && !latched && !matches!(awaiting, Some(System | User | OutOfBand))`,
so the cue tracks the model's thinking (`Model`) and streaming (`None`) and
drops during a tool call (`System`) and during a human wait (`User` /
`OutOfBand`). The `awaiting` field is already on the `ConversationState` event; no
framework change is needed and none is made. The edge's dedup, lifetime
deadline, lag handling, channel resolution and error swallowing are unchanged.
No configuration change, no new dependency, no adapter behavior change (the
adapter still only translates a begin/stop into the platform's typing action).

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** A tool-bearing turn yields the stop-and-resume shape: model-warranted
  → begin; a pending tool call (`awaiting == System`) → stop; the tool result
  and the model running again (`awaiting == Model`) → begin; the answer commit
  → stop — pinned on the composing edge as the ordered transition sequence for a
  scripted state stream carrying the awaiting phases.
- **AC3** A plain turn with no tool call is unchanged: one begin at the start and
  one stop at commit, no extra transitions — pinned (the prior single-begin/
  single-stop pin holds under the new derivation).
- **AC3b** The streaming tail holds the cue: a state with `work_due` true,
  unlatched, and `awaiting == None` (the stream awaiting nobody) keeps the cue
  on — proven by a turn that thinks (begin), calls a tool (stop), then streams
  its answer with `awaiting == None` (RESUME, not left dark), then commits (stop)
  — the ordered begin/stop/begin/stop pins that the streaming `None` state turns
  the cue back on, which `awaiting == Some(Model)` alone would not.
- **AC4** The cue is off during a human wait: an `awaiting == User` or
  `OutOfBand` state with `work_due` true and unlatched yields no begin (or a
  stop if one was open) — pinned.
- **AC5** Every existing composing invariant still holds under the new
  derivation: the once-per-transition dedup, the lost-stop lifetime expiry and
  its re-begin, the lag-stops-everything answer, and the latched/foreign
  exclusions — the prior pins pass, extended only where they now must carry an
  `awaiting` value on their state events.

## Notes for launch

- Branches from main (units 15–17 merged, HEAD 4a89d9b). The edge is
  crates/core/src/composing.rs; the `Awaiting` type is in the framework
  (agent_ledger, re-exported) and the `ConversationState` event already carries
  it. The edge's test helper `state_event(conversation_id, work_due, latched)`
  gains an `awaiting` argument so the new pins can drive the phases.
- Verify against the framework (agent_ledger actor's state broadcaster) that a
  streaming answer holds `awaiting == Model` and a pending tool call publishes
  `awaiting == System`, and build to the true mapping if it differs.
