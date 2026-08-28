# Unit 33 — the consumer absorbs the framework's date markers

Date: 2026-08-28. Framework slice 13 (agent-ledger 02b79a9) makes every user-voiced
consumer append write a `date_marker` block when the conversation's date changed — a
role-less framework record, ordered before the append that tripped it, rendered as a
dated system line (`Current date: 2026-07-12 (Sunday)…`). The consumer was written
against marker-less ledgers: 63 tests fail (60 spine, 2 adapter, 1 core lib), and one of
those failures is a production defect, not an expectation drift. This unit makes the
consumer correct and green against framework master. Everything else in the queue merges
behind it, and the deployment repository's framework pin (out of this tree) must not
move before it lands — the deployment breaks the same way.

## Grounding

**The marker, as the framework ships it.** `ensure_date_marker` runs inside every
user-voiced append seam, in the same transaction, immediately before the blocks that
ride it: the first user-voiced append of a day inserts one `date_marker`; same-day
appends insert none (change detection against the conversation's latest marker). A
marker can be a conversation's LAST block through real, public paths:
`Store::insert_user_blocks(conv, vec![])` runs the marker seam with no blocks behind it
(`store/messages.rs:744-783`, no empty-vec guard), a `Continuation::NewThread` fork
deep-copies a role-less group ending in a marker (pinned by the framework's own test,
`store/descriptors.rs:3327-3381`), and `copy_junction_up_to` copies through such a
group. The dated line's format is pinned in the framework (the render golden at
`providers/render/tests.rs:521-563` and the kind's unit tests at
`agency/date_marker.rs:104-230`) and never in the consumer — the consumer tree holds
zero occurrences of the line or the kind's name today. The kind is nameable without a
string literal: `BlockKind::DateMarker` is root-exported and matchable; the leaf's
`KINDS` declaration lives on `agency::DateMarker` (not re-exported at the crate root).
The date-driving seams
(`DateStamp`, `insert_user_blocks_dated`, `append_consumer_block_stamped`) are
`pub(crate)` — a consumer test cannot cross a date boundary, so a single consumer run
yields at most one marker per conversation, placed below its first user-voiced row.

**How the marker reaches a consumer request.** The consumer's ledger head is
`[system_prompt, tool_palette, date_marker, chat_message, …]` — the palette is
reconciled before the append (`assembly.rs:671`). The `tool_palette` block projects no
role (`tools/palette.rs:108`), which splits the system run, so the dated line renders
as its OWN system message after the prompt's message. This differs from the framework's
render golden, which pins the prompt-joined line only for ADJACENT prompt/marker
blocks — a shape the consumer never produces. Request-side prompt assertions in both
suites use substring checks (`support::carries`), and the whole-equality prompt
assertions (`spine/addressing.rs:113`, `spine/group_context.rs:1618`,
`spine/report.rs:1820`) compare the STORED `system_prompt` block, which the marker
never touches. Prompt assertions therefore need no change in this unit.

**The production defect, at its two sites.** `Assistant::owing_tail_debt`
(`core/src/assembly.rs:1521-1573`) finds the debt a conversation's tail still owes, and
judges marker blindness twice:

- The tail condition (`assembly.rs:1543-1548`): transparent means a kind in
  `DEBT_READ_THROUGH` (`assembly.rs:59`: context note, tool palette, report) or an
  erased chat row. A `date_marker` tail is neither — it maps to `AssistantKind::Core`
  and the walk answers "no debt" without ever reading behind it.
- The past-erased read (`kind::newest_block_id_past_erased`, `kind.rs:845`), called
  with a caller-supplied read-through list (the walk passes `DEBT_READ_THROUGH`; the
  core lib test passes its own `&[CONTEXT_NOTE_KIND]`, `kind.rs:1456`): an interposed
  marker stops the read, and the walk judges the marker instead of the block behind.

The core lib test (`kind::tests::the_read_answers_past_kind_runs_and_erased_rows_alike`,
`kind.rs:1520`) already pins the read's correct behavior — its own append trips a
marker into the run, and the read must return the block behind — and currently fails
returning the marker's id. The walk's spine pins live in
`spine/addressing.rs:269-460`; `assembly.rs` itself contains no test module.

**Why the tail half carries no fixture.** A bare marker tail is observationally
identical under the broken and the fixed tail condition — both answer "no debt", one by
misjudging the marker, one by reading past it to nothing — so a test on that shape
passes either way and pins nothing. The shape that would distinguish, a debt BEHIND a
marker tail, needs a date boundary between two appends, which only the framework's
`pub(crate)` seams can drive; and it is unreachable in today's consumer anyway — every
consumer append brings its message in the marker's own transaction, and the consumer
never calls the empty-vec insert. On top of that, `reconcile_palette` runs before the
walk on every ingest (`assembly.rs:671`) and appends a palette to any conversation not
in its per-process memory, so a store-constructed marker tail is re-tailed by a palette
before the walk ever sees it. The tail half of the fix is structural consistency,
verified by the recording's shape, not by a fixture; a behavioral pin at date-crossing
granularity belongs to the framework's stamped seam and is recorded as a framework
follow-up, not this unit's test.

**The test fallout, three mechanical shapes.** Roughly 31 failures gate on
`settle_shape`'s exact kind sequence, roughly 25 on `settled(len)`'s exact count, and a
handful assert directly on the raw ledger or the projected messages
(`storage.rs:232-235` asserts length and kind on the raw ledger;
`projection.rs:209` counts projected messages, where the marker is its own system
message). The adapter suite's kind-sequence await is `await_shape`, module-local at
`adapters/telegram/tests/adapter/tools.rs:130` — not yet in the adapter support
module. All three shapes are the same blindness: expectations enumerate consumer
content and the framework record shifted it.

**Sweep, already verified.** These production ledger readers are unaffected, checked
on this tree: `tools/provenance.rs` (`chain_step`'s catch-all `_ => Extends` reads a
marker through), `outbound.rs` (`deliverable_of`'s `_ => None` skips it),
`streams.rs:281` (prefix match), the note/palette newest-row reads (kind-filtered
joins), `erasure.rs:160-186` (per-block `ChatMessage` matches),
`retire_stale_prompts` (`assembly.rs:852-925`, `find_map` on `SystemPrompt`),
`window.rs` and `disclosure` (in-memory), `mirror` (inbound-only), and
`acknowledgment` (kind-filtered). The implementer confirms rather than re-derives.

## Decisions taken with this unit

- **Marker transparency is one recorded fact with two readers, 2026-08-28.** The
  consumer records once — one named constant or predicate beside `DEBT_READ_THROUGH`,
  with the doc comment carrying the decision — that a framework date record can never
  settle a debt walk and is never an answerable block. Both blind sites consume that
  one recording: the past-erased read excludes marker rows in its SQL for every caller,
  and the tail condition treats a marker tail as transparent. `DEBT_READ_THROUGH`
  itself stays a list of consumer kinds only, and the core lib test's caller-supplied
  list stays untouched. The marker kind is named through the framework's `DateMarker`
  leaf declaration or a typed match, never a consumer string literal. *Rejected:*
  adding the marker to `DEBT_READ_THROUGH` — the list is the walk's consumer policy,
  and the lib test's literal-list call would then fail until edited, weakening the pin;
  *rejected:* fixing only the read — the tail condition runs first and a marker tail
  would still swallow the walk (found by the second cold round); *rejected:* per-caller
  filtering — the same decision recorded per caller.
- **The tail half is verified by the recording's shape, and no fixture is built,
  2026-08-28.** The tail condition's marker transparency is enforced by consuming the
  same single recording as the read, and checked mechanically — the recording has
  exactly two consuming sites, cited file:line — plus the walk's contract comment
  stating the rule. No spine fixture exists for it, deliberately: a bare marker tail
  answers "no debt" under broken and fixed code alike, so a test on the one
  publicly-constructible shape pins nothing (and `reconcile_palette` re-tails such a
  conversation with a palette before the walk runs anyway); the distinguishing
  debt-behind shape needs a framework-private date crossing and is unreachable in
  today's consumer. A vacuous test labeled a pin fails the operator's quality bar
  harder than an honestly-stated structural argument. This closed after three cold
  rounds relocated the fixture question (none possible → one call exists → that call
  is vacuous); per the machine's practice rules, two relocations mean cut. *Rejected:*
  the bare-tail spine case (vacuous, above); *rejected:* a fixture invented around the
  `pub(crate)` seams; *rejected:* widening the framework seam for a consumer test —
  the behavioral pin at date-crossing granularity is the framework's follow-up.
- **The interior pin is the core lib test, and no spine twin is built, 2026-08-28.**
  An interior marker above a LIVE owing message needs two same-run markers — a date
  crossing — so no consumer spine fixture can build it. The lib test's ledger already
  holds the interior marker (its own append trips one) and its assertion, unedited, is
  the read's pin. *Rejected:* an earlier revision's spine case for the same fact —
  unsatisfiable as written, both cold rounds agree.
- **Each suite's support owns one consumer-view filter; no per-test arithmetic,
  2026-08-28.** The spine support and the adapter support each gain a single view —
  the ledger without `date_marker` blocks — and every count gate (`settled`), kind
  sequence (`settle_shape`, the adapter's `await_shape`, which moves into the adapter
  support), index assertion, and projected-message count IN THE FAILING SET reads
  through it. The radius is exactly the failing tests: a currently-green raw assertion
  stays raw even where it is marker-sensitive (`spine/group_context.rs:1607-1618`
  asserts an exact raw kind sequence and passes because its fixture makes no
  user-voiced append) — it moves through the view when and if it breaks. No test adds
  one to a length or splices a marker into an expected kind list. *Rejected:*
  updating expectations in place — it smears the marker over every test and breaks
  again on the next record kind; *rejected:* routing green tests through the view too
  — it widens the diff past the fallout for no behavioral gain; *rejected:* a shared
  test-support crate — a new crate for one predicate, coupling the suites.
- **The marker fact gets one consumer pin, midnight-safe, 2026-08-28.** One new spine
  test pins what the consumer relies on: after a fresh conversation's first ingest,
  exactly one `date_marker`, ordered before that ingest's chat message; after a second
  ingest, no second marker carrying the SAME stored date as the first — judged by the
  markers' stored dates, never the wall clock, so a midnight crossing cannot fail it;
  and the dated line reaching the recorded request as its own system message after the
  prompt's, matched only by its stable lead (`Current date: `). Cardinality is pinned
  here precisely because every other test reads through the filter and would mask a
  double marker. *Rejected:* recomputing the dated line in the consumer — it
  re-records the framework's format decision and races midnight.
- **The scope is the fallout, 2026-08-28.** The unit changes the shared transparency
  recording with its two readers, the two suites' supports, the tests the three
  fallout shapes name, and the one new pin (the marker fact). It does not
  widen the framework seam, does not add a consumer projection for markers, and does
  not touch the deployment repository's framework pin — that bump is a separate,
  owner-approved step.

## The unit's contract

The consumer is green against agent-ledger master 02b79a9. One recorded fact — a
framework date record never settles a debt walk — feeds both the tail condition and
the past-erased read, so a debt behind any run of markers, erased rows, notes,
palettes and reports is still found, and a stranded marker tail neither hides a debt
nor breaks the walk. The core lib test passes as written and is the read's pin; the
tail half is verified by the recording's two-site shape and its stated comments. Both
test suites assert
consumer behavior through a support-owned view of the ledger that excludes framework
date records. Exactly one new test pins the marker's presence, ordering, per-date
cardinality, and its dated line arriving as its own system message. Production
behavior is otherwise unchanged: no new stored fact, no new dependency, no
privacy-document change, no change to when the assistant answers or stays silent
beyond restoring the debt the marker was swallowing.

## Acceptance criteria

- **AC1** The full workspace suite is green in both answering modes against framework
  master; clippy, fmt, doc under denied warnings; vocabulary and secret scans clean;
  no new dependency.
- **AC2** The read is transparent to markers for every caller: the core lib test at
  `kind.rs:1520` passes as written — its assertion and its caller-supplied list
  unedited.
- **AC3** The tail condition is transparent to markers through the SAME single
  recording as the read — checked mechanically: the recording has exactly two
  consuming sites (the read's SQL exclusion and the tail condition), cited file:line,
  and the walk's contract comment plus the read's doc comment (`kind.rs:838-841`,
  whose "past erased rows alone" sentence the change falsifies) state the rule. No
  fixture exists for the tail half, per the decision recording why.
- **AC4** The marker pin passes as specified: one marker on first ingest, ordered
  before that ingest's chat message; no second marker with the same stored date on the
  second ingest, judged by stored dates; the dated line present in the recorded
  request as its own system message after the prompt's, matched only by its stable
  lead.
- **AC5** No test outside the two support modules and the pin names the marker kind
  or adjusts a count for it — checked mechanically by counting the use sites of the
  framework's `DateMarker` kind name (constant and raw string) in each suite.
- **AC6** The dated line's format is never re-recorded in the consumer: the only
  consumer knowledge of it is the pin's stable lead.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-markers`, branch
  `unit/date-marker-fallout`). Sites: `core/src/kind.rs` (the read, its doc comment's
  empty-slice sentence updated, the lib test left as written),
  `core/src/assembly.rs:59` and `:1521-1573` (the transparency recording beside
  `DEBT_READ_THROUGH`, the tail condition, the walk's contract comment),
  `core/tests/spine/support.rs` and the failing spine modules (the walk's existing
  spine pins are in `spine/addressing.rs:269-460`),
  `adapters/telegram/tests/adapter/` (its support module gains the view;
  `await_shape` moves there from `tools.rs:130`), and the marker-fact pin in a new
  spine module `core/tests/spine/date_marker.rs`, registered in the suite's
  `main.rs`.
- The failure inventory as measured on 2026-08-28 (60 spine by module: tools 19,
  report 8, end_to_end 4, helpful 4, privacy_rights 4, speaker 4, audience 2,
  disclosure 2, erasure 2, mirror 2, protection 2, sourcing 2, storage 2,
  addressing 1, erasure_streams 1, projection 1; adapter 2; core lib 1) is a snapshot
  for orientation, not a bound — the criterion is the whole suite green.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell." A per-test count adjustment is exactly such a
  snowflake.
