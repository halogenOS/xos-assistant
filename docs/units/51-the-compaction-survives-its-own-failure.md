# Unit 51 — the compaction survives its own failure

Date: 2026-09-01. Driven by a production incident of the same morning: the unattended
compaction of two long conversations failed on every attempt, retried on every wake with the
full first half as the model payload (single requests of 3158 and 6279 messages), and drained
the provider balance; after that, an endless 402 loop. The journal carried
`FOREIGN KEY constraint failed`, `Query returned no rows`, `tool call insert failed`, and
`scheduler: ratchet drive failed` repeating for the life of the process.

The repositories: the app (this repo, incident HEAD `bbeb859`) and the framework
(`ronna-core`, checkout `~/projects/agent-ledger`, HEAD `0b7cdd8`). Framework paths below are
that repo's, rooted at `crates/agent-ledger/`. Both repos change in this unit, framework
first; the app consumes the framework by path from the sibling checkout, unchanged by this
unit.

## Grounding — the verified failure chain

Every claim here was read from the two trees at the stated heads.

1. **The capture concludes before the turn does.** `capture_summary`
   (`crates/core/src/session.rs:606-638`) waits on `streams::await_stream_end`
   (`crates/core/src/streams.rs:339-366`), which answers on the FIRST `StreamDone` — but the
   framework emits `StreamDone` at message end (`src/ingestion.rs:1004-1013`) and a
   `ToolUse` stop's tool lifecycles arrive AFTER it, the turn continuing
   (`src/ingestion.rs:395`, `src/actor.rs:778-788`). It also answers `true` on
   `RecvError::Lagged` (`streams.rs:362`) — and the driver's own log proves this process lags
   its bus — and on the three-minute `SUMMARY_BOUND` expiry the caller proceeds anyway
   (`session.rs:617-622`).
2. **Retire deletes the conversation under the running turn.** `compact` calls
   `retire(temporary)` immediately (`session.rs:575`); `retire` (`session.rs:271-278`) runs
   `delete_conversation` (`src/store/conversations.rs:228-248`) with no interrupt and no
   settle. The still-running turn's every subsequent write funnels through `insert_block`
   (`src/store/messages.rs:168-184`), whose junction insert (`messages.rs:181`) references
   the deleted `conversations` row (`src/store/migrations.rs:112`, `foreign_keys=ON` at
   `src/store/mod.rs:287`): `FOREIGN KEY constraint failed`, surfacing as
   `tool call insert failed` (`src/ingestion.rs:1231-1236`, `src/actor.rs:677`).
3. **The dead conversation's actor set is immortal.** `route_event` spawns actor sets on
   demand (`src/actor.rs:1934-1948`) and nothing ends them when their conversation ceases to
   exist. The leaked scheduler's `Store::cursor` (`src/store/conversations.rs:259-268`, a
   bare `query_row`) answers `Query returned no rows` on every store-wide change
   notification (`src/store/mod.rs:200-202`) forever: `scheduler: ratchet drive failed`
   (`src/actor.rs:1525`), once per leaked set per store change.
4. **The fork's own writes refire the door, mis-attributed.** The forced-turn-end door
   level-reads on EVERY `BlocksChanged` (`session.rs:1383-1404`); the `TOOL_CALLS_EXHAUSTED`
   marker is only outrun by a successful compaction's `AncestorReference`, so failure leaves
   it standing — the ruled and intended shape. What is NOT intended: `copy_junction_up_to`
   (`src/store/conversations.rs:746-765`) inserts one junction row per first-half block, and
   the change watcher throws away the junction row it just read and re-derives the
   conversation by LOOKUP — `conversation_for_block` (`src/store/messages.rs:958-969`), a
   `LIMIT 1` with no `ORDER BY` served by an index ordered `(block_id, rowid)` — so the
   SOURCE's older junction row wins and the flood lands on the source id, re-passing its door
   test with no genuine activity anywhere. The driver's 256-slot subscription drops most of
   the flood as `Lagged` (`src/bus.rs:34`); the survivors refire the door at once. This is
   what made the loop self-sustaining.
5. **A model may attempt a tool in the toolless compaction turn.** The compaction turn runs
   with an empty palette by the operator's requirement, and the model can emit a tool call
   anyway. The app's palette decline is app-authored prose (`crates/core/src/tools/admission.rs:69-71`),
   while the framework's trailing refusal run counts only errors whose text opens with
   `ToolError::RATE_LIMIT_PREFIX` (`src/agency/tool_error.rs:42,117-120`, folded at
   `src/agency/tool_call.rs:309-323`). So a decline does not shorten the turn. It is not
   unbounded — every recorded call, declines included, counts toward the conversation's
   tool-call window (`src/agency/tool_call.rs:215-219`), 60 calls per 60 seconds by default
   (`src/tools/runner.rs:96-97`) — but the bound is the wrong one: it costs up to sixty paid
   model rounds per turn where the five-refusal forced end would cost five.
6. **Database failures are swallowed.** The write failures in (2) and the scheduler failures
   in (3) are logged at error/warn and the process runs on, in whatever state the failed
   write left implied. `StoreError::Sqlite` (`src/store/mod.rs:71-74`) is one opaque variant
   carrying a foreign-key violation, a busy timeout and a disk error alike, so no caller can
   tell impossible state from ordinary contention.

**Corrected from this spec's first draft:** `anchor_of` (`src/store/messages.rs:192-198`)
does NOT mistreat a legal absence. Its signature is already `Result<Option<i64>>` and that
`Option` is the nullable column. The missing-row case is an error, and every caller
(`src/store/tool_calls.rs:120,157`, `src/store/approvals.rs:66,188`) passes a block id it
read inside the same transaction — so an absent row there is impossible state, exactly what
F4 aborts on. Nothing to change; the first draft's `.optional()` requirement was wrong and is
withdrawn.

**Also corrected:** an earlier revision of F1 said the block watcher's filter admits
`conversations` as a wake. It does not — that filter (`src/actor.rs:1867-1872`) tests for
`blocks`, `conversation_blocks` and the content tables only. The claim named the wrong
subscription: the scheduler reacts through the table-blind `ctx.store.changes.watcher()`
(`src/actor.rs:1563`), which is why the deletion wakes it. F1 now says that.

## The design

**Framework (ronna-core), landed first:**

- **F1 — an actor set lives exactly as long as its conversation.** The set's existence
  DERIVES from its conversation's existence, read from the store it already reads. The seam
  is the cursor: `Store::cursor` stops being a bare `query_row` and answers `Option<i64>`,
  where `None` means the conversation is gone. The scheduler already performs that read on
  every drive; `None` ends the set instead of logging a failure, so the leak, its error
  spam and its death signal are one fact read in one place. `delete_conversation` commands
  nothing about actors — the operator's ruling, verbatim: "this smells like an architecture
  issue. You are not allowed to bolt an imperative thing on it. Refactor cleanly if needed."
  The deletion is itself an observed change so an idle store does not defer the end: the
  `conversations` table is already in the hook's allowlist (`src/store/descriptors.rs:289-295`),
  so the delete emits a change, and the scheduler's own subscription is table-blind — it
  reacts through `ctx.store.changes.watcher()` (`src/actor.rs:1563`, `db_changes.react()`),
  which wakes on any change at all. The block watcher's filter
  (`src/actor.rs:1867-1872`) is a different subscription and admits only `blocks`,
  `conversation_blocks` and the content tables; it is not on this path and this unit does
  not widen it.
- **F2 — a change event's conversation comes from the row the change names.** Per branch of
  the watcher (`src/actor.rs:1873-1895`), because the three branches have different rows:
  - a `conversation_blocks` change already reads its own row to get `block_id`; it reads
    `conversation_id` from that same row and the second lookup disappears. A row that is
    gone by read time (a delete, a rolled-back insert) attributes nothing and emits nothing —
    the store's own position, `src/store/mod.rs:369-380`: an event is a wakeup, and truth is
    what the consumer re-reads.
  - a `blocks` or content-table change has no junction row, so the block's conversations
    must be looked up — and the defect is the arbitrary pick, not the lookup. The block is
    announced to EVERY conversation joined to it; `conversation_for_block`'s `LIMIT 1`
    without an order dies.
- **F3 — a refusal is a typed fact, not a prefix.** The tool outcome row records whether it
  is a refusal that should count toward the trailing run, as a stored fact the framework
  owns and reads back; the rate-limit refusals set it, and a consumer's own decline (the
  app's palette decline) sets it through the same typed surface. The starts-with test on
  `RATE_LIMIT_PREFIX` retires, and with it the recorded position that the prefix IS the
  machine key (`src/agency/tool_error.rs:42`) — superseded here, with the supersession
  recorded: a second consumer now needs the same fact, and matching a consumer's prose from
  the framework is the second decision path this spec rejects below. A toolless turn's
  decline loop then ends at the existing five-refusal forced end.
- **F4 — a database failure is classified where it is still typed, and propagated to the
  code competent to judge it.** Classification happens at ONE chokepoint, inside the store
  actor (`src/store/mod.rs`) where the rusqlite error is still typed and its extended code
  still readable, never at a call site. The classifier reads the primary code
  (`extended_code & 0xff`) and answers a class: `Rejected` for a constraint violation
  (the incident's foreign-key failure among them), `Contended` for busy and locked,
  `Unusable` for corruption, not-a-database and misuse, plain `Sqlite` for everything else,
  and `ActorStopped` when the store's own thread is gone. Nothing ends the process inside a
  query. The class travels to the caller, and the codepath that knows the blast radius
  decides what it means: the app states `Unusable` and `ActorStopped` as
  `FailureKind::Fatal`, and each intake ends its run so the supervisor starts a replacement
  process with the message still unacknowledged. Because the classifier sits under every
  caller, no caller can swallow what it raises. The user's words, verbatim: "There is a
  difference between catching an expectable query error and a serious db failure. Something
  failing on a foreign key constraint is an error while a race with another writer is
  expected and can be retried if it makes sense. […] You aren't meant to panic inside a db
  query but instead wrap and propagate the error properly so a codepath competent to handle
  it can decide what to do about it." This supersedes F4's first position — that the
  chokepoint itself aborts on the impossible-state classes, from "a database error should
  hard crash the application, not leave it running in a corrupted state" — because an abort
  under every caller takes the decision away from the only code that can size the damage,
  and it makes the whole path untestable in-process. The hard crash survives as the app's
  answer to the fatal class, one level up. A panic on the store's own thread is caught
  there, logged, and left unresumed: the thread finishes, its channel closes, and every
  later call answers `ActorStopped` — which is the fatal class, so the same end follows.
- **F5 — the framework answers whether a turn is durably over.** A predicate read off the
  ledger: no streaming tail remains AND no tool outcome is awaiting a continuation
  (`ToolCall::unanswered_outcome_anchor`). It is the framework's question — the app cannot
  see the second half — and it exists because a turn-end STATUS row is not available to ask:
  `settle_turn_identity` (`src/actor.rs:1067-1072`) writes one only when the turn ends over
  an unanswered outcome, so a toolless prose turn, which is every successful compaction,
  never records one.

**App, on the new framework:**

- **A1 — the capture awaits the turn's durable end.** `capture_summary` concludes when F5's
  predicate reads true, level-read from the store on block changes — the driver's own
  lossy-bus-safe pattern — never on `StreamDone`, never on a lagged subscription.
  `SUMMARY_BOUND` remains the outer bound; on expiry the temporary turn is interrupted and
  the interrupt's own settle awaited before anything else happens.

  A turn that ends WITHOUT prose ends the capture too, and it needs a second fact to say so
  (added 2026-09-01, after three readers found the same hole). The predicate alone cannot
  tell "the turn ran and wrote nothing" from "the turn has not started": a freshly forked
  temporary is all durable and owes no outcome, so the predicate reads true over it before
  anything runs. The second fact is that a stream terminal for this conversation has been
  seen since the unlatch — the turn demonstrably ran. Only all three together answer `None`
  at once: the predicate true, no prose in the ledger, and the turn seen to have ended. This
  does not concede A1's rule. `StreamDone` still concludes nothing by itself — over a
  tool-use stop the predicate is false and over a turn with prose the ledger answers — and a
  terminal a lagged subscription drops costs only the bound, which is what the capture would
  have waited anyway. Without this the incident's own shape regresses: a provider error ends
  the turn writing nothing, and the capture sits out the full three minutes inside the one
  shared compaction driver, stalling every other conversation's door for the same three
  minutes, where the code this unit replaces failed in milliseconds.
- **A2 — retire settles first, and a settle that fails stops the deletion.** `retire`
  interrupts the temporary conversation and awaits its settle before `delete_conversation`.
  If the settle fails (`streams::confirm_settled` at its deadline,
  `crates/core/src/streams.rs:371-390`), the conversation is NOT deleted and the compaction
  fails: deleting anyway would reopen the exact race A2 exists to close. After this unit, no
  write ever targets a retired id.
- **A3 — a failure just fails.** No backoff, no cooldown, no stand-down state of any kind —
  the operator's ruling, verbatim: "Failures are failures and failures just fail the
  compaction." A failed attempt logs and returns; the door stands; the next attempt happens
  when the next genuine block change arrives, which — once F2 kills the mis-attribution — is
  real conversation activity and nothing else. The unbounded ruling of decision 0165 stands
  untouched.
- **A4 — the record tells the truth.** The self-limiting claim at `session.rs:1303-1310` is
  rewritten to name the failure case; a new decision records the store-failure policy —
  classify at the chokepoint, propagate the class, and let the intake end the run on the
  fatal one — naming the abort-inside-the-store-actor shape it beat; a second records the
  typed refusal superseding the prefix key; decision 0165 is NOT amended (no bound is
  added).

## Acceptance criteria

1. The compaction capture concludes on the temporary turn's durable end as F5 defines it —
   no streaming tail, no outcome awaiting a continuation — or on the unit's own awaited
   interrupt after `SUMMARY_BOUND`; never on `StreamDone`, a lagged bus, or a bare timeout.
   A successful toolless compaction concludes as soon as its prose lands, not at the bound.
   A turn that ends with NO prose concludes at once as well, answering `None`, on the three
   facts A1 names: the predicate true, the ledger holding no prose, and a stream terminal
   for this conversation seen since the unlatch. Parking such a turn to the bound is a
   failure of this criterion, not a slow pass — the capture runs inline in the one shared
   compaction driver.
2. `retire` settles (interrupt + awaited end) before deletion; a settle that fails leaves the
   conversation undeleted and fails the compaction. After a failed compaction no framework
   write targets a deleted id — no FK violation, no scheduler `Query returned no rows`.
3. `Store::cursor` answers `Option`; an actor set whose cursor reads `None` ends itself, and
   the deletion itself wakes that read rather than the next unrelated change.
   `delete_conversation` commands nothing about actors, and no store-change logging from
   retired ids survives.
4. A junction change event names the conversation of its own row, read in the same query
   that reads its `block_id`; a junction row absent at read time emits nothing. A block or
   content change is announced to every conversation joined to that block. A fork's copy
   emits nothing attributed to the source, and the source's door does not refire from a
   fork's writes.
5. A refusal is recorded as a typed fact on the outcome row; the app's palette decline sets
   it through that surface, the framework matches no consumer prose, and a decline loop in a
   toolless turn ends at the five-refusal forced end.
6. A failed unattended compaction adds no retry machinery: no backoff, no cooldown, no
   stand-down; the next attempt rides the next genuine block change only.
7. Store-error classification lives at the single store-actor chokepoint and reads off the
   SQLite primary code: constraint answers `Rejected`, busy and locked answer `Contended`,
   corruption, not-a-database and misuse answer `Unusable`, everything else stays plain
   `Sqlite`, and a store thread that is gone answers `ActorStopped`. No call site
   classifies, and nothing ends the process inside a query. The app states `Unusable` and
   `ActorStopped` as `FailureKind::Fatal` and every other class as it did before; on the
   fatal class each intake ends its run with the message unacknowledged.
8. Every behaviour above is covered by a test: early capture on a `ToolUse` turn, the
   lossy-bus capture path, a successful toolless capture concluding before the bound, the
   `SUMMARY_BOUND` interrupt-and-settle path, a settle failure leaving the conversation
   undeleted, the retire race, actor end-of-life on deletion, junction attribution including
   the absent-row and multi-conversation cases, the decline loop ending at the forced end,
   and store-error classification (every class, and the operational ones proven NOT to be
   fatal). The operator's ruling, verbatim: "Thats what tests are for you need to test
   every scenario." Framework-side, the classifier is unit-tested against each primary code.
   App-side, `failure_kind` is tested over all five store classes, and both intakes —
   long-poll and webhook — are tested end to end against a killed store writer: the run ends
   stating the core cannot serve, and the offset stays on the update the core could not take.
9. Review items, not tests: no retry machinery exists anywhere in the diff (AC6); the stale
   self-limiting prose is gone; the crash-policy and typed-refusal decisions exist; 0165 is
   untouched.

## Rejected alternatives

- **A failure backoff on the unattended doors** (base-and-cap doubling) — proposed in this
  unit's first draft and rejected by the operator, 2026-09-01, verbatim: "I dont want the
  backoff. Failures are failures and failures just fail the compaction." The loop's actual
  fuel was the mis-attributed event flood (F2); with attribution fixed, retries ride genuine
  activity only.
- **A payment-class stand-down and a typed error class across the boundary** — dropped with
  the backoff under the same ruling: a 402'd attempt is refused before it costs anything,
  and with F2 in place such attempts fire only on genuine activity. The framework's
  `LlmError` classification stays where it is; nothing consumes it across the boundary in
  this unit.
- **Imperative actor teardown invoked by `delete_conversation`** — rejected by the
  operator's architecture ruling; existence derives from the conversation, it is not
  managed by the deleter.
- **Matching the app's `declined:` prose from the framework, or making the app write the
  framework's rate-limit prefix** — the first hardcodes consumer vocabulary in the framework,
  the second ships a sentence that lies to the model and breaks the pinned-wording test at
  `crates/core/src/tools/rights.rs:377-381`. F3's typed fact replaces both.
- **Parsing the 402 out of the rendered error string in the app** — the class exists typed
  in the framework; re-deriving it from prose is a second decision path. (Moot with the
  stand-down dropped, recorded so nobody re-proposes it.)
- **Adding `.optional()` to `anchor_of`** — the first draft's requirement, withdrawn: the
  absent row there is impossible state, and turning it into a legal `None` would hand every
  caller a silence where an error belongs. See the correction above.
- **Crashing on every `StoreError::Sqlite` indiscriminately** — that variant carries busy
  timeouts and disk errors as well as constraint violations, so a wholesale sweep would end
  the bot on ordinary write contention. The primary code is the discriminator.
- **Ending the process at the store-actor chokepoint itself** — F4's first position,
  superseded 2026-09-01 by the user's words quoted in F4. Classifying and aborting in the
  same place reads as tidy and is not: the store actor knows a constraint failed, and
  nothing about whether that constraint belongs to one refused message or to the whole
  process. Deciding there takes the judgement away from the code that has the context, and
  it puts an `abort()` under every test, which is why the first position needed a subprocess
  harness to be exercised at all. Classification stays at the chokepoint; the decision moves
  out to the caller that can size the damage.
- **Auditing every log site for swallowed integrity failures** — the first draft's AC7 asked
  for it across 210 sites in two repos with no decision procedure. The chokepoint makes every
  caller compliant by construction instead.

## Known consequences, out of scope

- A process end between `fork_temporary` and `retire` leaves the temporary conversation
  latched and unreapable — its blocks are joined, so `gc_orphan_blocks` cannot reach it.
  Inert, one per mid-compaction process end. A reaper for latched temporaries is its own
  unit.
- A2's settle failure leaves one behind the same way, and more often than the line above
  accounts for: once per settle-failed attempt, not once per process end. It is not inert
  either — the temporary stands unmapped but unlatched, so a provider deaf to the interrupt
  keeps writing into it and its block changes keep waking the driver. The reaper unit above
  covers the deletion; what it cannot cover is the writing, and that is the price A2 accepts
  for never deleting a conversation still being written to.
- F5's predicate is not monotone across a tool-use turn, so A1's capture can still conclude
  early as a RACE. The framework trails a tool-use stop's call blocks after the message end
  (`src/ingestion.rs`, the drain phase), and in the window between the text finalization's
  commit and that insert the ledger reads durably over with prose present. The wake the
  finalization emits is exactly the one the capture reads on, so the read races the insert.
  It is a narrow window against the certainty this unit removes — the code it replaces
  concluded at every message end, always — but it is real, and closing it needs a
  framework-side fact the app cannot see. Its own unit.

## Rulings appendix (operator, verbatim)

- 2026-09-01, msg 1552: "So you added a bug that wasted money? Damn. Why are your tests not
  covering this kind of thing? Loops are the worst kind of bug. It either wastes money,
  wastes resources or spams people. Fix it please"
- 2026-09-01, msg 1555: "Also a database error should hard crash the application, not leave
  it running in a corrupted state"
- 2026-09-01, msg 1556: "Thats what tests are for you need to test every scenario"
- 2026-09-01, msg 1559: "Compactions dont have tools, that was part of my requirements. But
  the model might try to use one anyway" and "Yes compaction is unbounded but that was under
  the assumption that the code is correct and that db errors dont spiral out of control"
- 2026-09-01, msg 1566: "On deletion keeps them running: this smells like an architecture
  issue. You are not allowed to bolt an imperative thing on it. Refactor cleanly if needed."
  / "I dont want the backoff. Failures are failures and failures just fail the compaction."
  / "Retry's own db writes get wrongly counted → sounds like an architecture problem again.
  Fix properly instead of bolton."
- 2026-09-01, msg 1604, superseding msg 1555's placement of the crash: "There is a
  difference between catching an expectable query error and a serious db failure. Something
  failing on a foreign key constraint is an error while a race with another writer is
  expected and can be retried if it makes sense." / "Instead of throwing around questions
  like these and circling around your yapping, how about you design the architecture
  properly and implement error handling the right way? You aren't meant to panic inside a db
  query but instead wrap and propagate the error properly so a codepath competent to handle
  it can decide what to do about it."
- Standing from decision 0165 (2026-08-30), untouched by this unit: "The unattended path
  carries no repetition bound, by the operator's ruling".
