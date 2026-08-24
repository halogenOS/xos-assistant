# Unit 20 — the rules acknowledgment is the assistant's own words

Date: 2026-08-24. Revision 2, rewritten after a cold probe proved revision 1
unbuildable. When the group's pinned rules change, the assistant posts a fixed
line — "Rules noted. The assistant follows the pinned rules of this group." — a
deterministic product line (decided 2026-08-23) so the wording could not drift.
The operator, seeing it live, wants the acknowledgment in the assistant's own
voice: a short, natural confirmation that reads like the assistant, not a canned
string. This unit generates the acknowledgment with the model, and retires the
fixed line — WITHOUT making it a member answer, because it is not one.

## Grounding (why revision 1 was wrong)

Revision 1 proposed opening a model TURN from the rules delta, reusing the answer
machinery. The probe proved that is architecturally blocked, at every layer:
- `answer_due`/`debt_authority` are columns on `block_chat_message` alone,
  written only by the message ingest path (`kind.rs`, `assembly.rs:697`). No
  other block kind carries a debt; a `context_note` cannot open one without new
  debt machinery built from scratch.
- `ContextNote::frontier_transparent()` returns `true` unconditionally
  (`note.rs:263`) — a load-bearing, tested invariant (decision 0094: "a note
  appended over an unanswered message buries nothing") — so a rules-note block
  can never become the dispatch frontier. The "stamp the note block" option is a
  dead end unless that invariant is broken.
- A member-less turn breaks the answer machinery it would reuse: the disclosure
  fold's `co_summoners` is always empty for a non-message frontier, so
  `first_answer_to_someone` returns `true` EVERY time — every acknowledgment
  would be prefixed with the full AI-disclosure line, forever. The budgets JOIN
  `block_chat_message`, so a member-less turn is invisible to them — the reused
  "budget accounting" simply does not apply. And a member-less turn is
  "unaddressed", so if the model emitted the miss/abstention sentinel the
  acknowledgment would be silently swallowed — a delivery the retired fixed line
  guaranteed.

The lesson: the answer machinery is for MEMBER ANSWERS, and a rules
acknowledgment is a service event, not a member answer. It must not borrow the
turn machinery; it needs its own small, bounded generation.

## Decisions taken with this unit

- **The rules delta generates its acknowledgment with a bounded, one-shot model
  call, not a turn, 2026-08-24.** The observation path that today returns
  `DeliveryItem::Acknowledgment(RULES_ACKNOWLEDGMENT)` on a real `NoteTopic::Rules`
  delta (`assembly.rs` ~892) instead performs one bounded model completion —
  given the new rules text and a short instruction to acknowledge them in the
  assistant's voice — and delivers its output as the acknowledgment. This opens
  no debt, no turn, no disclosure fold, no budget row, no co-summoner chain: none
  of the answer machinery is touched, because the acknowledgment is not a member
  answer. The call is bounded — a request timeout, a small output cap, the
  configured reasoning level — and produces plain text. Rejected: the member-less
  turn (revision 1 — architecturally blocked at every layer, proven by the
  probe); a template with slots (still canned, still drifts from "the assistant's
  voice").
- **A deterministic fallback preserves the delivery guarantee, 2026-08-24.** The
  retired fixed line delivered 100% of the time; a model call can fail, time out,
  return empty, or return something unusable. So the fixed line is kept as a
  FALLBACK, not the primary: when the bounded call fails, times out, returns
  empty, or returns only whitespace/an abstention-or-miss sentinel, the
  deterministic line is delivered instead — so a real rules change ALWAYS draws a
  visible acknowledgment, never silence. The model call improves the wording; it
  never removes the guarantee. Rejected: no fallback (revision 1's silent-swallow
  regression — a real rules change could produce no acknowledgment at all).
- **The admission and the spend bound are unchanged in spirit, 2026-08-24.** The
  on-delta admission still gates everything: an identical re-pin appends nothing
  and calls nothing; only a genuine rules change runs the call. Pinning is
  admin-only, so only an administrator can trigger a call, and only by actually
  changing the rules — the same bound the deterministic line already relied on.
  The one new cost — an admin who cycles the rules text now triggers a model call
  per real change rather than a free string — is admin-only, bounded by the delta
  check, and accepted; if it ever needs tightening, a per-channel acknowledgment
  window is the lever, noted but not built. `NoteTopic::Title` still acknowledges
  nothing. Rejected: a rate limit on the acknowledgment now (the delta check plus
  admin-only pinning already bounds it; an unused limiter is complexity for a
  threat the rights model already contains).
- **The acknowledgment's ledger treatment matches the retired line's,
  2026-08-24.** Whatever the deterministic acknowledgment does today with respect
  to being stored as a block versus delivered ephemerally, the model-generated
  acknowledgment does the same — this unit changes only WHERE the text comes from
  (a bounded model call with a deterministic fallback), not how it is delivered or
  recorded. The implementer verifies the current storage/delivery of the
  acknowledgment and keeps it identical. Rejected: newly recording the
  acknowledgment as an assistant turn block if it is not recorded today (that
  would reintroduce the projection/history questions this design exists to avoid).

## The unit's contract

The rules-delta path replaces the fixed `RULES_ACKNOWLEDGMENT` string with a
bounded one-shot model completion (new rules in, short in-voice acknowledgment
out), delivered exactly as the acknowledgment is delivered today; the fixed line
becomes the fallback for a failed, timed-out, empty, or unusable call, so a real
delta always delivers something. No debt, no turn, no disclosure/budget/
co-summoner/abstention interaction. The on-delta admission (real change only,
identical re-pin silent), the title-acknowledges-nothing rule, and the storage/
delivery of the acknowledgment are unchanged. The bounded call carries a timeout,
an output cap, and the configured reasoning level. No new configuration; no new
dependency; the provider is the one the answer machinery already uses.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency. The fixed line survives
  only as the fallback constant.
- **AC2** A real rules change delivers a model-generated acknowledgment: on a
  `NoteTopic::Rules` delta the bounded call runs with the new rules in its
  request, and its output is delivered to the chat as the acknowledgment — pinned
  against a scripted model completion, the request provably carrying the new
  rules text.
- **AC3** The delivery guarantee holds: when the bounded call fails, times out,
  returns empty/whitespace, or returns an abstention/miss sentinel, the
  deterministic fallback line is delivered instead — pinned for each failure mode;
  a real delta NEVER results in silence.
- **AC4** The admission is unchanged: an identical re-pin runs no call and
  delivers nothing; a title change runs no acknowledgment call — pinned. Exactly
  one acknowledgment per real delta.
- **AC5** No answer machinery is touched: the acknowledgment path opens no debt,
  no unlatch, no disclosure fold, and no budget row — the acknowledgment carries
  no AI-disclosure prefix (it is not a member answer), and the disclosure,
  co-summoner, budget and abstention pins of units 12/14/16 are unchanged and
  pass — pinned, including an assertion that the acknowledgment text has no
  disclosure line prepended.
- **AC6** Storage/delivery unchanged: the model-generated acknowledgment is
  stored and delivered exactly as the deterministic line was (verified against
  the current behavior), and the bounded call is timeout- and output-capped —
  pinned.
- **AC7** The documents: any doc naming the fixed acknowledgment line as fixed
  product behavior is updated to the model-generated acknowledgment with its
  deterministic fallback — pinned in the docs test where such a line exists.

## Notes for launch

- Branches from main (units 15-18 merged, HEAD 1891fcd).
- The one real mechanism question for the implementer: how to make a bounded
  one-shot model completion from the observation path (the provider the answer
  machinery uses is registered in the core; the actor drives streaming turns —
  the acknowledgment needs a collected one-shot call, bounded). Settle it against
  the framework's provider interface; if a clean one-shot entry does not exist,
  the smallest addition that gives one — not a reimplementation of the turn loop.
- Keep the deterministic line verbatim as the fallback constant so the guarantee
  is unchanged when the model is unavailable.
