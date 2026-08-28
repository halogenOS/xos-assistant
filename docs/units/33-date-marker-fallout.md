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

**The marker, as the framework ships it.** `ensure_date_marker` runs inside the
consumer-append seam: the first user-voiced append of a day inserts one `date_marker`
before the appended block; same-day appends insert none (change detection against the
latest marker). The marker and the block that tripped it are written in one transaction
at every framework seam (`store/descriptors.rs:1143`, `store/messages.rs:770`,
`store/drafts.rs:225`, `store/conversations.rs:419` and `:443`), so no ledger ever holds
a marker whose message failed to land; the one shape that leaves a marker as a
conversation's last block is a fork whose junction copy inherits it
(`copy_junction_before`). The render golden
(`agent-ledger providers/render/tests.rs:521-563`) pins the dated line's format; the
format is the framework's decision, pinned there and nowhere else. The framework's
public surface names the kind only through the `DateMarker` leaf's `LeafKind`
declaration (`agency/date_marker.rs:41`) — there is no pub string constant. The stamped
test seam (`append_consumer_block_stamped`) is `pub(crate)` — a consumer test cannot
drive the date and always receives a marker stamped from now.

**How the marker reaches a consumer request.** The consumer's ledger head is
`[system_prompt, tool_palette, date_marker, chat_message, …]` — the palette is
reconciled before the append (`assembly.rs:671`). The `tool_palette` block projects no
role (`tools/palette.rs:108`), which splits the system run, so the dated line renders as
its OWN system message after the prompt's message — never joined to the composed
prompt. Request-side prompt assertions in both suites already use substring checks
(`support::carries`), and the whole-equality prompt assertions
(`spine/addressing.rs:113`, `spine/group_context.rs:1618`, `spine/report.rs:1820`)
compare the STORED `system_prompt` block, which the marker never touches. Prompt
assertions therefore need no change in this unit.

**The production defect.** `Assistant::owing_tail_debt`
(`core/src/assembly.rs:1536-1573`) finds the debt a conversation's tail still owes: if
the tail is transparent — a kind in `DEBT_READ_THROUGH` (`assembly.rs:59`: context
note, tool palette, report) or an erased chat row — it reads past the whole transparent
run (`kind::newest_block_id_past_erased`, `kind.rs:845`) and judges the block behind. A
`date_marker` inside that run is neither in the caller's read-through list nor an
erased chat row, so the read stops ON the marker, the marker maps to
`AssistantKind::Core`, and the walk answers "no debt" — an owed answer sitting behind
an erasure-plus-note run is silently dropped. The core lib test
(`kind::tests::the_read_answers_past_kind_runs_and_erased_rows_alike`, `kind.rs:1520`)
already pins the correct behavior — calling the read with its own literal
`&[CONTEXT_NOTE_KIND]` list — and is the failure that exposed both the defect and where
the fix must live: a caller-supplied list can never carry the marker's transparency,
because every caller would have to know it.

**The test fallout, two shapes.** The spine and adapter suites break mechanically, not
semantically:

- Count-gated settledness: `settled(len)` (`core/tests/spine/support.rs`) accepts a
  ledger of exactly `len` blocks; the marker makes every such ledger one longer, so the
  gate never accepts and the test times out. This is the bulk of the 60.
- Kind-sequence awaits: the adapter suite's ledger-shape await
  (`adapters/telegram/tests/adapter/tools.rs:142`) compares an explicit kind list; the
  marker appears third and the list never matches.

**What already holds.** The marker carries no personal data and no consumer columns;
erasure does not touch it. The scripted provider derives answers from the newest
projected user message, which the marker (a system contribution) never is.

## Decisions taken with this unit

- **Date markers are transparent to the past-erased read itself, for every caller,
  2026-08-28.** `kind::newest_block_id_past_erased` skips `date_marker` rows
  unconditionally, exactly as it already skips erased chat rows: a framework date
  record is never an answerable block, for any caller, ever. `DEBT_READ_THROUGH` stays
  what it is — the walk's policy over CONSUMER kinds. The walk's tail test learns the
  marker's transparency through the same single recording as the read (one shared
  predicate or constant, cited by the implementer), never through a second list entry.
  With this shape the core lib test passes as written, its caller-supplied list
  untouched. *Rejected:* adding the marker to `DEBT_READ_THROUGH` — the lib test's own
  literal list proves every other caller of the read would be wrong one by one, and the
  marker is not a consumer policy choice; *rejected:* filtering markers at each call
  site — the same decision recorded per caller.
- **A marker tail and an interior marker are one decision, 2026-08-28.** The
  fork-inherited marker tail is the one reachable stranded shape, and it is covered by
  the same single recording of transparency the read and the tail test share — the
  implementer cites the shared site in flags; no separate tail fixture is built.
  *Rejected:* a dedicated bare-marker-tail test — no public seam in this workspace
  constructs that ledger (marker and message share one transaction at every seam, and
  the framework's fork machinery is not driveable from the consumer suite at test
  granularity), so the fixture would need an invented construction the spec refuses.
- **Each suite's support owns one consumer-view filter; no per-test arithmetic,
  2026-08-28.** The spine support and the adapter support each gain a single view —
  the ledger without `date_marker` blocks — and every count gate (`settled`),
  kind-sequence await, and index-based block assertion reads through it. No test adds
  one to a length or splices a marker into an expected kind list; a future framework
  record kind is absorbed by widening the view in one place per suite. The filter names
  the kind through the framework's `DateMarker` leaf declaration or a typed match —
  never a consumer string literal. *Rejected:* updating the 62 expectations in place —
  it smears the marker's existence over every test and breaks again on the next record
  kind; *rejected:* a shared test-support crate so the view exists once across suites —
  a new published crate for one predicate, coupling the adapter suite to the core
  suite's internals; the two supports already hold their own await helpers by the same
  reasoning.
- **The marker fact gets one consumer pin, midnight-safe, 2026-08-28.** One new spine
  test pins what the consumer now relies on: after a fresh conversation's first ingest,
  exactly one `date_marker`, ordered before that ingest's chat message; after a second
  ingest, no second marker carrying the SAME stored date as the first — the pin reads
  the markers' stored dates instead of assuming the wall clock, so a run that crosses
  midnight still asserts exactly one marker per distinct date. The pin also asserts the
  dated line reaches the recorded request as its own system message after the prompt's
  system message, matching it only by its stable lead (`Current date: `) — the exact
  format stays pinned in the framework's render golden. Cardinality is pinned here
  precisely because every other test reads through the filter and would mask a double
  marker. *Rejected:* recomputing today's expected line in the consumer suite — it
  re-records the framework's format decision and races midnight.
- **The scope is the fallout, 2026-08-28.** The unit changes the read and the walk's
  shared transparency recording, the two suites' supports, the tests the two fallout
  shapes name, and the one new pin. It does not widen the framework seam, does not add
  a consumer projection for markers, and does not touch the deployment repository's
  framework pin — that bump is a separate, owner-approved step.

## The unit's contract

The consumer is green against agent-ledger master 02b79a9. `newest_block_id_past_erased`
skips date markers for every caller, the owing-tail walk's tail test shares that one
recording, and so a debt behind any run of markers, erased rows, notes, palettes and
reports is still found — pinned by the existing core lib test passing as written.
Both test suites assert consumer behavior through a support-owned view of the ledger
that excludes framework date records. Exactly one new test pins the marker's presence,
ordering, per-date cardinality, and its dated line arriving as its own system message.
Production behavior is otherwise unchanged: no new stored fact, no new dependency, no
privacy-document change, no change to when the assistant answers or stays silent beyond
restoring the debt the marker was swallowing.

## Acceptance criteria

- **AC1** The full workspace suite is green in both answering modes against framework
  master; clippy, fmt, doc under denied warnings; vocabulary and secret scans clean; no
  new dependency.
- **AC2** The read is transparent to markers for every caller: the core lib test at
  `kind.rs:1520` passes as written — its assertion and its caller-supplied list
  unedited — and a spine case proves an owed answer behind an erased-row-plus-note run
  with an interposed marker still draws the answering turn. The tail test and the read
  share one recording of the marker's transparency, cited file:line.
- **AC3** The marker pin passes as specified: one marker on first ingest, ordered
  before that ingest's chat message; no second marker with the same stored date on the
  second ingest, judged by the markers' stored dates; the dated line present in the
  recorded request as its own system message after the prompt's, matched only by its
  stable lead.
- **AC4** No test outside the two support modules and the new pin names the marker
  kind or adjusts a count for it — checked mechanically by counting the use sites of
  the framework's `DateMarker` kind name in each suite.
- **AC5** The dated line's format is never re-recorded in the consumer: the only
  consumer knowledge of it is the pin's stable lead.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-markers`, branch
  `unit/date-marker-fallout`). Sites: `core/src/kind.rs` (the read, its transparency
  recording, and the lib test left as written), `core/src/assembly.rs` (the walk's tail
  test and its comment), `core/tests/spine/support.rs` and the failing spine modules,
  `adapters/telegram/tests/adapter/` support and its two tests, one new spine test.
- The failure inventory as measured on 2026-08-28 (60 spine by module: tools 19,
  report 8, end_to_end 4, helpful 4, privacy_rights 4, speaker 4, audience 2,
  disclosure 2, erasure 2, mirror 2, protection 2, sourcing 2, storage 2, addressing 1,
  erasure_streams 1, projection 1; adapter 2; core lib 1) is a snapshot for
  orientation, not a bound — the criterion is the whole suite green.
- The implementer sweeps every production ledger walk for the same marker-blindness
  before touching tests: candidates are `erasure.rs`, `streams.rs`, `outbound.rs`,
  `tools/admission.rs`, and unit 26's co-summoner threading — each either unaffected
  with the reason stated, or fixed under the same transparency decision.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell." A per-test count adjustment is exactly such a
  snowflake.
