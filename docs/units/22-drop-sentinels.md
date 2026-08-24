# Unit 22 — drop the abstention sentinels; the empty turn speaks for itself

Date: 2026-08-24. The framework now records an empty completed turn as a real empty
assistant text block (agent-ledger master bfcc6c7) and raises a `responding` signal when
real text starts flowing. That makes the consumer's `[[abstain]]`/`[[miss]]` sentinels a
bolted-on workaround: a magic answer string invented to represent an outcome the mechanism
now expresses natively. This unit removes them and lets the empty turn — and the model's own
words — carry the meaning. It also delivers the typing cue the framework signal was built for.
Grounded by a cold probe of both repos; the file:line receipts below are its.

## Grounding (what the probe found)

- **`responding` reaches the consumer bus.** `CoreEvent::StreamStatus{label}` is emitted on the
  same `Arc<EventBus>` the composing edge subscribes to (`event.rs:116`, framework
  `ingestion.rs:600`, shared bus `actor.rs:84`); the edge drops it today (`composing.rs:173`
  `Ok(_) => {}`). The `RESPONDING` key (`event::stream_status::RESPONDING`) fires once per turn
  at the first non-empty text delta — never at text-block-start, never for thinking, never for a
  turn that finalizes empty.
- **The empty block reaches outbound as an empty `Answer`.** `deliverable_of` turns any finalized
  assistant text block into `Deliverable::Reply{text, kind: Answer}` regardless of emptiness
  (`outbound.rs:583-593`). Unhandled, an empty answer either delivers a lone introduction line to
  a first-time asker (`disclosure.rs:116-131`, no empty guard) or sends an empty message to
  Telegram, which the Bot API rejects.
- **The miss machinery is fully self-contained.** `DONT_KNOW_ANSWER`, `store_dont_know`,
  `anchor_literally_addressed` and the `is_miss` routing (`outbound.rs:171,395-409,514-559`) are
  used only by the sentinel mechanism and its tests; nothing else consumes them.
- **The budget refund keys on the sentinel string.** `COUNTED_DEBT_SQL` (`kind.rs:927-939`)
  excludes a debt from the rate-limit count when an assistant block anchored on the debt's
  message trims to `ABSTENTION_SENTINEL` (`?3`, bound at `kind.rs:963,995`) — the "silence is
  refunded" behaviour. The answer text is the joinable column `block_text.content` (`at.content`),
  and the framework writes the empty block's content as `''`.
- **`projects_raw_sentinel`** (`kind.rs:1081-1090`) makes sentinel blocks invisible to the model
  projection; with sentinels gone it is dead, and the framework WANTS the empty block projected
  (the model reads its own empty message back). The acknowledgment sentinel check
  (`acknowledgment.rs:129-131`) is dead too; its empty-guard above it already covers the
  degenerate case.
- **Teaching carries the sentinel instructions** in `sentinel_rules()`, `sourcing_rules()` (the
  `[[miss]]` sentence, `teaching.rs:146-148`), `audience_rules()` (the "not an abstention and not
  a miss" reconciliation, `teaching.rs:171-174`) and `answering_section()`.

## Decisions taken with this unit

- **Delete the sentinels, 2026-08-24.** Remove `crates/core/src/abstention.rs` entirely and its
  `lib.rs:74` re-export. Every `is_abstention`/`is_miss`/`ABSTENTION_SENTINEL`/`MISS_SENTINEL`
  use site goes with it. *Rejected:* keeping the constants "just in case" — dead vocabulary that
  re-invites the workaround.
- **An empty answer is delivered as nothing, 2026-08-24.** In the outbound delivery loop, an
  `Answer` whose text is empty (after trim) is swallowed exactly where the abstention swallow sits
  today (`outbound.rs:375-383`, before the disclosure fold at `:416`): advance the cursor and
  `continue` (accounted as delivered, yields nothing). The model saying nothing produces nothing
  on the wire — natively. *Rejected:* returning `None`/`Skipped` from `deliverable_of` — it
  either fails to advance the cursor (re-scans the empty block every wake) or logs the wrong
  report-specific line (probe Q2(i)).
- **The model's own words carry "I don't know", 2026-08-24.** Remove the whole miss routing and
  its machinery (`outbound.rs:171,395-409,514-559`). When addressed and unable to back an answer
  with a lookup, the model says "I don't know" as ordinary text (delivered by the plain answer
  path); when not addressed with nothing to add, it ends its turn empty. The addressed-vs-silent
  choice moves from a machine reading `literal_addressed` to the model's own judgment — the
  intended direction. *Rejected:* preserving the fixed `DONT_KNOW_ANSWER` wording as a machine
  rewrite — that is the machine deciding again; the teaching instead forbids guessing from memory.
- **The budget refund keys on the empty answer, not the sentinel, 2026-08-24.** In
  `COUNTED_DEBT_SQL` change the final predicate from `= ?3` to `= ''` and drop the
  `ABSTENTION_SENTINEL` binding (`kind.rs:963,995`); keep the `trim(...)` (defensive, `trim('')==''`).
  A debt whose turn produced an empty answer (no-text / thinking-only / elided-move — all commit
  the empty block) stays refunded; a turn with real text stays counted. Behaviour preserved,
  re-keyed onto the framework's native record.
- **`FrameworkKind`'s projection becomes a pure delegate, 2026-08-24.** Delete
  `projects_raw_sentinel` and its three call sites (`kind.rs:1142,1149,1156`) so the projection
  hooks plainly delegate; the empty block projects to the model as its own empty message (the
  framework's intent). Delete the acknowledgment sentinel check (`acknowledgment.rs:129-131`),
  leaving the empty-guard.
- **Re-teach silence and "I don't know", 2026-08-24.** In `teaching.rs`: delete `sentinel_rules()`
  and its interpolation; reword `answering_section()`'s silence lines to "when you have nothing to
  add, end your turn without writing any text — no placeholder"; rewrite the `sourcing_rules()`
  miss sentence (`:146-148`) to "when addressed and a lookup cannot back the answer, say you don't
  know plainly — never guess from memory or offer a hedged recollection; when not addressed with
  nothing to add, end the turn with no text"; reword only the sentinel nouns in `audience_rules()`
  (`:171-174`) keeping "a clarifying question makes no substantive claim, so it needs no lookup".
  KEEP verbatim the unit-16 sentence "an answer that makes a substantive claim must be one you can
  back with a lookup" (`teaching.rs:112-114`) — the new rule completes it, does not contradict it.
- **The typing cue lights on real text only, 2026-08-24.** The composing edge (`composing.rs`)
  adds a `CoreEvent::StreamStatus` arm: when `label == RESPONDING` it begins the cue for that
  conversation; it clears on the stream terminal set — `StreamDone | StreamError | StreamClosed`.
  The cue no longer lights during the pre-text thinking window or for a turn that says nothing (no
  `responding` → no cue) — the operator's complaint resolved. The cue stays keyed per
  conversation, and the existing 5-minute lost-stop lifetime deadline (`composing.rs:104-124`)
  bounds any missed clear. *Rejected:* deriving the begin from `ConversationState` alone (it
  cannot tell thinking from real text — the whole reason `responding` exists); a wall-clock grace
  delay (operator rejected it).
  **AMENDED 2026-08-24, after implementation (orchestrator disposition).** This decision first
  said the cue clears on `ConversationState work_due→false`. That premise was FALSE and the
  implementer verified it against the tree, reproducing the failure live before building to the
  true state. Grounded: `work_due = outcome.owes_turn || outcome.parked` (framework
  `actor.rs:1358`), and `owes_turn` is a fact about the frontier BLOCK. A `Streaming` block takes
  the default `awaiting() → None` and is not frontier-transparent (`agency/records.rs:82-96`), so
  the instant it is inserted — lazily at the FIRST TEXT DELTA, which is exactly when `responding`
  fires — the frontier stops owing a turn and `work_due` drops to false. Clearing on that edge
  would stop the cue the moment it lit (a visible flash, reproduced in the adapter tests). The
  stream terminal set is the framework's real turn-end vocabulary here and is what the existing
  stream observer and the outbound edge already key on. Consequence, accepted: a tool-bearing turn
  raises one begin/stop pair per text-bearing stream — correct behaviour, since each pair marks
  real text actually flowing. **Turn-id keying: pending the operator's decision (asked
  2026-08-24).** The persisted-in-DB turn id already exists as the framework's `dispatch_anchor`;
  the grounded finding is the cue does not need to key on it (per-conversation state + the
  deadline already prevent cross-turn leak, a conversation being serial). If the operator wants
  the edge keyed on it explicitly, the edge reads `latest_block(conv).dispatch_anchor` from the
  `responding` event onward (store access exists, `composing.rs:213`); the base design here does
  not depend on that choice.

## The unit's contract

The `[[abstain]]`/`[[miss]]` sentinels and all their machinery are gone. A turn that says
nothing produces an empty assistant block (framework) that the outbound edge delivers as
nothing — no empty send, no stray introduction line. When addressed and unable to answer, the
model says "I don't know" as ordinary text; the addressed-vs-silent choice is the model's own.
The budget still refunds a silent turn, keyed on the empty answer. The model projection shows
the empty turn (a pure delegate, no suppression). The teaching instructs no-text-for-nothing and
plain "I don't know" without guessing, keeping the lookup-backed discipline intact. The typing
cue lights only once real user-visible text starts flowing and clears at turn end. No new
dependency.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc under denied warnings; vocabulary and
  secret scans clean; no new dependency. `abstention.rs` is deleted and no symbol references it.
- **AC2** An empty answer delivers nothing: a turn whose committed answer block is empty sends no
  Telegram message and prints no introduction line — pinned, including the first-time-asker path
  (no stray disclosure) and the returning-asker path (no empty send).
- **AC3** A real answer still delivers normally, and an addressed "I don't know" authored by the
  model is delivered as ordinary text (no special routing) — pinned.
- **AC4** The budget refund holds on the new key: an addressed turn that produced an empty answer
  is NOT counted against the principal/channel budget (silence still refunded), while a turn with
  real text IS counted — pinned against `COUNTED_DEBT_SQL` behaviour, and the sentinel parameter
  is gone.
- **AC5** The empty turn projects to the model as its own empty message (the projection is a pure
  delegate; no sentinel suppression remains) — pinned.
- **AC6** The teaching no longer contains any sentinel vocabulary and instructs end-empty /
  say-"I don't know"-plainly / model's-own-judgment; the unit-16 lookup-backed sentence is
  retained verbatim — pinned (prompt-composition tests rewritten off the sentinels).
- **AC7** The typing cue lights on `responding` and clears on the stream terminal (amended
  2026-08-24, see the decision): a turn that produces real text raises the cue once text starts
  (not during pre-text thinking) and stops it at the stream's end; a turn that says nothing raises
  NO cue; a clarifying-question reply (real text) raises the cue — pinned. Unit-18 composing pins
  rewritten to the `responding`-begin derivation.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-noresponse`, branch
  `unit/drop-sentinels`); builds against agent-ledger master bfcc6c7 (the empty block + the
  `responding` signal). Confirmed to compile against it.
- The mechanism is grounded (receipts above) — do NOT re-cold-probe; verify against the tree and
  build. The one open product choice (turn-id keying) is flagged in the decisions and does not
  block the base cue design.
- Sites, from the probe: delete `abstention.rs` + `lib.rs:74`; outbound swallow at `:375` shape +
  remove `:171,395-409,514-559`; `kind.rs` SQL `:936` + params `:963,995`, delete
  `projects_raw_sentinel` `:1081-1090` + call sites `:1142,1149,1156`; `acknowledgment.rs:129-131`;
  `teaching.rs` `:15,100-127,146-148,171-174,184-193` + tests `:203-263,288-290,325-350`;
  `composing.rs` add the `StreamStatus`/`RESPONDING` arm at the `:127` match, clear unchanged.
