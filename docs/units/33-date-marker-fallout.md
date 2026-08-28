# Unit 33 — the consumer absorbs the framework's date markers

Date: 2026-08-28. Framework slice 13 (agent-ledger 02b79a9) makes every user-voiced
consumer append write a `date_marker` block when the conversation's date changed — a
role-less framework record, ordered before the append that tripped it, rendered as a
dated system line (`Current date: 2026-07-12 (Sunday)…`) that joins an adjacent system
message. The consumer was written against marker-less ledgers: 64 tests fail (61 spine,
2 adapter, 1 core lib), and one of those failures is a production defect, not an
expectation drift. This unit makes the consumer correct and green against framework
master. Everything else in the queue merges behind it, and the ronna-core pin must not
bump before it lands — the deployment breaks the same way.

## Grounding

**The marker, as the framework ships it.** `ensure_date_marker` runs inside the
consumer-append seam: the first user-voiced append of a day inserts one `date_marker`
before the appended block; same-day appends insert none (change detection against the
latest marker). The render golden
(`agent-ledger providers/render/tests.rs:521-563`) pins the line format and the
system-message join; the format is the framework's decision, pinned there and nowhere
else. The stamped test seam (`append_consumer_block_stamped`) is `pub(crate)` — a
consumer test cannot drive the date and always receives a marker stamped from now.

**The production defect.** `Assistant::owing_tail_debt` (`core/src/assembly.rs:1536-1573`)
finds the debt a conversation's tail still owes: if the tail is transparent — a kind in
`DEBT_READ_THROUGH` (`assembly.rs:59`: context note, tool palette, report) or an erased
chat row — it reads past the whole transparent run
(`kind::newest_block_id_past_erased`, `kind.rs:845`) and judges the block behind. A
`date_marker` inside that run is neither in the read-through set nor an erased chat row,
so the read stops ON the marker, the marker maps to `AssistantKind::Core`, and the walk
answers "no debt" — an owed answer sitting behind an erasure-plus-note run is silently
dropped. The core lib test
(`kind::tests::the_read_answers_past_kind_runs_and_erased_rows_alike`, `kind.rs:1520`)
already pins the correct behavior and is the failure that exposed this.

**The test fallout, three shapes.** The spine and adapter suites break mechanically, not
semantically:

- Count-gated settledness: `settled(len)` (`core/tests/spine/support.rs`) accepts a
  ledger of exactly `len` blocks; the marker makes every such ledger one longer, so the
  gate never accepts and the test times out. This is the bulk of the 61.
- Kind-sequence awaits: the adapter suite's ledger-shape await
  (`adapters/telegram/tests/adapter/tools.rs:142`) compares an explicit kind list; the
  marker appears third and the list never matches.
- Prompt-recording assertions: the recorded request's system message is now the composed
  prompt, a blank line, then the dated line; assertions comparing it whole to
  `composed_prompt()` fail once the count gates stop masking them.

**What already holds.** The marker carries no personal data and no consumer columns;
erasure does not touch it. The scripted provider derives answers from the newest
projected user message, which the marker (a system contribution) never is. The framework
exports the marker kind's name; the implementer grounds the exact constant and never
writes the string twice.

## Decisions taken with this unit

- **The date marker is transparent to the owing-tail walk, 2026-08-28.** The marker
  joins the kinds the walk reads through, in both places the walk judges transparency:
  the tail test and the past-erased read (`DEBT_READ_THROUGH` feeds both). A marker is a
  framework record about the calendar, appended by the framework at its own moment — the
  exact description the read-through set's comment gives for why a kind must not settle
  a debt behind it. The walk's doc comment is updated to say so. *Rejected:* treating a
  marker tail as settled while reading through interior markers — two transparency
  answers for one kind, and a marker stranded as tail by the crash window between the
  marker write and its message would then hide the debt state behind it; *rejected:*
  filtering markers inside `newest_block_id_past_erased` itself — that read serves the
  walk through the same set, and a second, kind-specific exclusion would be the same
  decision recorded twice.
- **Each suite's support owns one consumer-view filter; no per-test arithmetic,
  2026-08-28.** The spine support and the adapter support each gain a single view —
  the ledger without `date_marker` blocks — and every count gate (`settled`),
  kind-sequence await, and index-based block assertion reads through it. No test adds
  one to a length or splices a marker into an expected kind list; a future framework
  record kind is absorbed by widening the view in one place per suite. *Rejected:*
  updating the 63 expectations in place — it smears the marker's existence over every
  test and breaks again on the next record kind; *rejected:* a shared test-support crate
  so the view exists once across suites — a new published crate for one predicate,
  coupling the adapter suite to the core suite's internals; the two supports already
  hold their own await helpers by the same reasoning.
- **Prompt assertions compare the prompt; the dated line is the framework's,
  2026-08-28.** One support helper splits a recorded system message into the composed
  prompt and the framework's trailing dated line; prompt-recording assertions compare
  the prompt half against `composed_prompt()` exactly as before. The dated line's
  presence is pinned once, in the new marker test; its exact text stays pinned in the
  framework's render golden and is asserted here only by its stable lead
  (`Current date: `). *Rejected:* recomputing today's expected line in the consumer
  suite — it re-records the framework's format decision and races midnight.
- **The marker fact gets one consumer pin, 2026-08-28.** One new spine test pins what
  the consumer now relies on: a fresh conversation's first ingest of the day lands
  exactly one `date_marker` ordered before the chat message, a second same-day ingest
  lands none, and the recorded request's system message carries the dated line after the
  composed prompt. Cardinality is pinned here precisely because every other test reads
  through the filter and would mask a double marker.
- **The scope is the fallout, 2026-08-28.** The unit changes the walk's transparency
  set, the two suites' supports, the tests the three shapes name, and the one new pin.
  It does not widen the framework seam, does not add a consumer projection for markers,
  and does not touch the deployment pin — the pin bump is a separate, owner-approved
  step.

## The unit's contract

The consumer is green against agent-ledger master 02b79a9. The owing-tail walk treats a
date marker as transparent, so a debt behind any run of markers, erased rows, notes,
palettes and reports is still found — pinned by the existing core lib test passing with
a marker interposed. Both test suites assert consumer behavior through a support-owned
view of the ledger that excludes framework date records, and assert the composed prompt
without re-recording the framework's dated-line format. Exactly one new test pins the
marker's presence, ordering, same-day cardinality, and its dated line reaching the
request. Production behavior is otherwise unchanged: no new stored fact, no new
dependency, no privacy-document change, no change to when the assistant answers or
stays silent beyond restoring the debt the marker was swallowing.

## Acceptance criteria

- **AC1** The full workspace suite is green in both answering modes against framework
  master; clippy, fmt, doc under denied warnings; vocabulary and secret scans clean; no
  new dependency.
- **AC2** The owing-tail walk reads through markers: the core lib test at `kind.rs:1520`
  passes as written against the marker-bearing ledger, and a spine case proves an owed
  answer behind an erased-row-plus-note run still draws the answering turn.
- **AC3** A marker stranded as the ledger tail does not settle the conversation: with
  the tail being a bare `date_marker`, the walk reads through it and judges the block
  behind — pinned at the walk's own granularity.
- **AC4** The new marker pin passes: one marker per conversation per day, ordered before
  the day's first chat message, none on the second same-day ingest, and the dated line
  present in the recorded request's system message after the composed prompt.
- **AC5** No test outside the two support modules and the new pin names the marker kind
  or adjusts a count for it — enforced by review, and mechanically: the marker kind's
  name appears in each suite exactly where the view and the pin live.
- **AC6** Prompt-recording assertions still compare against `composed_prompt()`
  verbatim for the prompt half, with the split helper carrying the only knowledge of the
  dated line's lead.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-markers`, branch
  `unit/date-marker-fallout`). Sites: `core/src/assembly.rs` (`DEBT_READ_THROUGH` and
  the walk's comment), `core/src/kind.rs` (the lib test, unchanged in its assertion),
  `core/tests/spine/support.rs` and the failing spine modules,
  `adapters/telegram/tests/adapter/` support and its two tests, one new spine test.
- The failure inventory as measured on 2026-08-28 (61 spine by module: tools 19,
  report 8, end_to_end 4, helpful 4, privacy_rights 4, speaker 4, audience 2,
  disclosure 2, erasure 2, mirror 2, protection 2, sourcing 2, storage 2, addressing 1,
  erasure_streams 1, projection 1; adapter 2; core lib 1) is a snapshot for orientation,
  not a bound — the criterion is the whole suite green.
- The implementer sweeps every production ledger walk for the same marker-blindness
  before touching tests: candidates are `erasure.rs`, `streams.rs`, `outbound.rs`,
  `tools/admission.rs`, and unit 26's co-summoner threading — each either unaffected
  with the reason stated, or fixed under the same transparency decision.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell." A per-test count adjustment is exactly such a
  snowflake.
