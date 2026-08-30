# Unit 40 — a heads-up line before slow work

Date: 2026-08-30. The operator's instruction, verbatim: "The bot should announce
concisely if it's going to do something that takes a while." Example given: a
question needing a web search draws "Let me look that up for you real quick.", then
the search runs, then the real answer arrives.

## Grounding

**The delivery mechanism exists whole and is pinned; the missing piece is the
teaching.** A provider round's text ahead of a tool call finalizes as its own
committed answer block (`finalize_streamed_text_tail`, framework
`ingestion.rs:877-923`); `StreamDone` fires PER ROUND — for a tool-use stop exactly
like an end-turn (`ingestion.rs:1002-1011`) — and the outbound edge wakes on it
(`outbound.rs:215-230`) and delivers every committed answer block above its cursor
with no mid-turn filter (`outbound.rs:344-420`, `deliverable_of` :543-555). The
adapter pin `narration_before_the_call_delivers_both_texts_to_the_chat`
(`adapter/tools.rs:209-268`) proves the operator's exact example shape: narration
delivered, tool runs, answer delivered, in order, threaded. The production SSE
decoder produces the same ingestion sequence (`providers/chat/sse.rs:242, 333-383`).
The composing cue goes dark during the tool window (`composing.rs:139-188`), so the
announce is the only activity signal there — the operator's point. Nothing in any
prompt asks the model to write the line; that is this unit.

**The announce's home is the search, as a product call.** The operator's example
was the search, and only the search reaches the open web; the forge, release and
wiki lookups are HTTP reads too, against configured project hosts with the same
10-second timeout, and they stay un-announced — a scoping decided by the operator's
example, not by a locality claim.
The search teaching (`SEARCH_TEACHING`, `core/src/teaching.rs:109-116`) already
composes if and only if the tool is admitted — the capability-gated pattern this
unit extends (`teaching.rs:98-99`, pins at :526-572).

## Decisions taken with this unit

- **The announce is taught, never mechanized, 2026-08-30.** One sentence joins the
  search teaching: before running a search, say in one short line what you are about
  to look up, then search, then answer. The mechanism delivers whatever text
  precedes the call — that is already shipped and pinned — so this unit's
  surface is prose, pins, and one grown test fixture (the loopback SSE server's
  tool round), and the spec states honestly that a taught behavior is
  probabilistic, never a mechanism guarantee. *Rejected:* a framework early-flush
  boundary — the per-round StreamDone already IS the pre-tool flush, and a second
  boundary would duplicate it; *rejected:* any adapter or core code.
- **The announce is scoped to slow work, which today means the search, 2026-08-30.**
  The sentence lives inside the capability-gated search teaching, so it composes
  only when the search is admitted and never teaches announcing before the
  project-host lookups — and never before a report, whose own teaching wants the thinking done
  quietly (decision 0070's flow). A future slow tool brings its own announce
  sentence with it. *Rejected:* a general announce-before-any-tool rule — chatter
  before sub-second lookups.
- **The announce coexists with the no-filler rules by being scoped and bounded,
  2026-08-30.** The conduct prompt's length rule and the silence teaching's "no
  placeholder" stand: the teaching words the announce as a real, one-line statement
  of what is being looked up — not a stand-in for an answer, not restating the
  member's words — and explicitly at most one line. The sentence is worded to dodge
  a literal reading against "end your turn without writing any text": that rule
  governs a turn with NOTHING to say; an announcing turn has a search to run.
- **The announce changes nothing about the budgets, verified, 2026-08-30.** An
  earlier reading claimed an announced-then-failed turn spends its debt where a
  silent failed one does not; checked against the counted-debt mechanism and its
  pins, no such delta exists — a failed turn's counting is identical with or
  without a preceding announce, and only a COMPLETED silent turn's empty answer is
  excluded. The announce is budget-inert. Stated so nobody re-derives the false
  contrast.
- **Two pins close the composition gap, both in the core spine suite,
  2026-08-30.** Every existing narration DELIVERY pin scripts the event-native
  shape while production speaks SSE (one search fixture does carry a narration —
  spine/search.rs:530-536 — but no pin asserts its delivery ORDER). This unit adds:
  in `spine/search.rs`, a pin that a narration ahead of the search delivers before
  the search's result exists with the answer following; and in
  `spine/chat_completions.rs`, a production-wire pin — the loopback SSE server
  there grows a two-round script (text deltas, tool-call fragments, a tool_calls
  finish, then the closing text) with a registered tool, real fixture work stated
  as such. The adapter suite is NOT a home for either (its fixture has no search
  wiring and its provider is event-native). No new mechanism — the pins prove the
  shipped one under the production composition.

## The unit's contract

When the assistant is about to run a web search, she first says in one short line
what she is about to look up; the line delivers before the search runs, the search
runs, and the answer follows — all inside the one turn, threaded as today. Nothing
else changes: no new mechanism in the core, the adapter or the framework (the core
change is teaching prose and its pins), no new tool,
no budget change beyond the recorded announce-then-fail acceptance, no
privacy-document change (the announce is assistant prose riding the existing
conversation).

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under
  denied warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The announce sentence composes if and only if the search capability is
  admitted — a new contains-pin beside the existing capability-gate pins
  (`teaching.rs:526+`), and the pin names the facts the wording decision lists
  (one line, what is being looked up, no placeholder, no restating), the way the
  search teaching's own facts are pinned; the existing contains-pins stay green
  untouched.
- **AC3** The operator's example shape is pinned by its two deterministic facts,
  in `spine/search.rs` with the existing searching fixture shape: the LEDGER order
  (the narration text block precedes the tool call, which precedes the tool
  result, via the settle-shape read) and the CHAT arrival order (the narration
  reply is received before the answer reply, via two reply receives). Together
  they prove announce → search → answer; a wall-clock delivered-before-the-result-
  existed assertion is NOT pinned — it would race two bus subscribers and flake —
  and this criterion says so rather than asking for it.
- **AC4** The production-wire composition is pinned in `spine/chat_completions.rs`:
  a two-round SSE script (text deltas, tool-call fragments, a tool_calls finish,
  then the closing text) drives the real provider module over the loopback server
  with the RUNTIME-FACTS tool registered — the no-network tool, so no second
  vendor server enters the wire test — and the consumer-visible ledger shows the
  narration-then-call-then-answer composition.
- **AC5** The decision records land numbered from the highest shipped, dated, with
  rejected alternatives.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-announce`, branch `unit/announce`, from
  `main` (`e4222a7`). Build's first step: `git rebase main`.
- Sites: `core/src/teaching.rs` (the sentence + its pin), `crates/core/tests/spine/
  search.rs` and `spine/chat_completions.rs` (the two composition pins; the
  loopback SSE server there grows the tool round), `docs/decisions`.
- This is a SMALL unit with a predictable seat count; it may run in the small lane
  beside a big build.
- The quality bar from the operator, verbatim scope for the reviewers: "The code
  must always be better and cleaner afterwards than it was before. If you had to
  add a snowflake if somewhere, it's a smell."
