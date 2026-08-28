# Unit 34 — the runtime tool states the current date and time

Date: 2026-08-29. The operator asked for it after watching the assistant answer "what
time is it" from the day marker's written-at stamp — right that once, stale the rest
of the day. The runtime-facts tool (unit 32) already states per-call facts about the
running assistant; the current date and time are exactly such facts.

## Grounding

The framework's date markers give the model the DATE once per day; they carry a
written-at minute that is the marker's label, never the present time. Framework
slice 15 exports one public clock reading — date, weekday, zone abbreviation and IANA
zone name (each optionally NULL, never guessed), wall-clock `HH:MM` — read at one
instant from the same private source the markers stamp from, so the tool and the
markers can never disagree. The consumer holds no clock of its own and gains no
dependency for one.

## Decisions taken with this unit

- **The clock joins the runtime-facts tool; no second tool, 2026-08-29.** The
  operator named both options and the facts tool is the fit: it exists to state
  per-call runtime facts, and a second tool would teach the model two names for one
  kind of question. *Rejected:* a separate datetime tool.
- **The tool renders the framework's reading at execute time, 2026-08-29.** Two fact
  lines in the tool's existing rendering convention: the date with its weekday, and
  the time with the zone name when the reading carries one — absent zone parts render
  gracefully absent, never guessed. The framework's marker LINE format is not
  re-recorded: the tool renders its own lines from the reading's parts. *Rejected:*
  re-deriving local time in the consumer (records the clock decision twice and can
  drift from the markers); *rejected:* echoing the marker's rendered line (its format
  is the framework's pinned decision).

## Acceptance criteria

- **AC1** Workspace green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary scan clean; no new consumer dependency (checked: no clock or
  timezone crate enters any consumer manifest).
- **AC2** The tool's result carries the date-with-weekday and time facts read at
  execute time through the framework's public reading — pinned at the tool's existing
  test granularity, including that two calls in one process can answer different
  times (the reading is per call, not cached).
- **AC3** Absent zone parts render gracefully — pinned with a reading whose zone is
  NULL, however the tool's tests already drive such shapes.
- **AC4** No consumer re-derivation: the consumer names no clock source but the
  framework's reading — checked mechanically over the consumer crates.

## Notes for launch

Branches from `main` (worktree `~/projects/halogenos-assistant-clock`, branch
`unit/runtime-clock`). Site: `core/src/tools/runtime.rs` and its spine tests. Blocked
on framework slice 15 merging to the framework master this workspace path-depends on.
