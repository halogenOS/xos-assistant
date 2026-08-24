# Unit 20 — the rules acknowledgment is the assistant's own words

Date: 2026-08-24. Revision 1. When the group's pinned rules change, the assistant
posts a fixed line — "Rules noted. The assistant follows the pinned rules of this
group." — a deterministic product line (decided 2026-08-23) so the wording could
not drift. The operator, seeing it live, wants the acknowledgment in the
assistant's own voice: a short, natural confirmation that reads like the
assistant, not a canned string. This unit makes the rules delta produce a
model-generated acknowledgment, and retires the fixed line.

## Grounding

- The fixed line is `RULES_ACKNOWLEDGMENT` (`crates/core/src/outbound.rs`), emitted
  at `crates/core/src/assembly.rs` (~line 893): on a real rules-note delta the
  observation path appends the note block and returns
  `DeliveryItem::Acknowledgment(RULES_ACKNOWLEDGMENT)` — a deterministic reply
  delivered straight to the chat, NOT a model turn. `NoteTopic::Title` acknowledges
  nothing.
- The delta check is the whole admission: an identical re-pin appends nothing and
  says nothing (`newest == text` short-circuits). Only a genuine change acknowledges.
- The rules note is guaranteed in the model's projected context while one exists
  (unit 15, decision 0094), so a turn triggered by a rules change already sees the
  new rules.
- The debt spine (unit 14): a turn dispatches when a message OPENS A DEBT
  (`answer_due`). A pin is a service observation, not a member message, and today
  opens no debt — which is why the acknowledgment is a deterministic reply rather
  than a turn. Making it a model turn is a debt-spine interaction and is the unit's
  central design question (below), to be settled against the real mechanism.

## Decisions taken with this unit

- **A real rules change draws a model-generated acknowledgment, 2026-08-24.** The
  deterministic `RULES_ACKNOWLEDGMENT` line is retired. On a genuine rules-note
  delta the assistant runs a turn whose answer is a short acknowledgment in its own
  voice that it has read the new rules and will hold the group to them — natural,
  brief, not a canned string, and varying with the rules. The on-delta admission is
  unchanged: an identical re-pin still appends nothing and acknowledges nothing;
  only a real change runs the turn. `NoteTopic::Title` still acknowledges nothing.
  Rejected: keeping the fixed line (the operator's own request retires it);
  generating the line from a template with slots (still canned, still drifts from
  "the assistant's voice").
- **The acknowledgment turn reuses the answer machinery, not a parallel path,
  2026-08-24.** The acknowledgment is produced by the same turn machinery every
  answer uses — the projection carrying the new rules note, the model call, the
  outbound delivery, the once-per-person disclosure fold on a first spoken answer —
  so it inherits abstention-free delivery (an acknowledgment is warranted, it does
  not abstain), the reasoning level, and the budget accounting, with no second
  answer path to keep in step. The mechanism that opens the turn is the unit's to
  settle against the debt spine: the rules delta must open an answer-due turn (the
  acknowledgment intent) the way an addressed message opens one, carrying a
  per-turn intent the model reads ("the group's rules just changed to the pinned
  note; acknowledge the change briefly in your own voice, and do not restate the
  rules verbatim"). The implementer settles whether that intent rides as a
  synthetic summoning fact, a one-turn context note, or a stamp on the note block,
  binding: it opens exactly one turn per real delta, it is not a member message (no
  member principal is invented), and it flows through the existing unlatch/budget/
  disclosure readers unchanged. Rejected: a bespoke "generate an ack" model call
  outside the turn machinery (a parallel answer path — the exact duplication the
  ledger architecture forbids); opening the turn as if a member sent a message
  (invents a principal and corrupts the addressed/co-summoner facts).
- **The acknowledgment is taught, and grounded-answer discipline does not swallow
  it, 2026-08-24.** The prompt teaches the shape of a rules acknowledgment: brief,
  in the assistant's voice, confirming it will hold the group to the new rules,
  without reciting them. Because unit 16 makes an ungrounded/no-help turn abstain,
  the teaching is explicit that a rules-change turn is warranted and answered — the
  acknowledgment is not a substantive claim needing a lookup, it is the assistant
  confirming a group fact it was just handed, so it neither abstains nor emits the
  miss sentinel. Rejected: leaving the acknowledgment to the generic answer
  teaching (it would risk abstention, since no member asked anything — the turn has
  no question to answer, only a fact to acknowledge).

## The unit's contract

The deterministic `RULES_ACKNOWLEDGMENT` constant and its delivery
(`DeliveryItem::Acknowledgment` on the rules delta) are retired in favor of a
model turn: the rules-note delta opens exactly one answer-due turn carrying an
acknowledgment intent, the model produces a brief in-voice acknowledgment, and it
delivers through the ordinary answer path (disclosure fold included). The on-delta
admission (real change only, identical re-pin silent), the title-acknowledges-
nothing rule, and the rules-note-in-context guarantee are unchanged. The prompt
gains the acknowledgment teaching; the grounded-answer/abstention discipline is
taught not to swallow a rules-change turn. No member principal is invented for the
turn. No new configuration; no new dependency.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency. The retired constant has no
  remaining caller.
- **AC2** A real rules change runs one model turn and delivers its acknowledgment:
  a rules-note delta opens exactly one answer-due turn carrying the acknowledgment
  intent, the scripted model answer is delivered to the chat as the acknowledgment,
  and the new rules note is present in the turn's projected request — pinned over
  the wire, block by block.
- **AC3** The admission is unchanged: an identical re-pin opens no turn and delivers
  nothing; a title change opens no acknowledgment turn — pinned. Exactly one turn
  per real delta, never two.
- **AC4** No member principal is invented and the debt readers are undisturbed: the
  acknowledgment turn opens its debt without a member message, and the co-summoner
  set, the addressed/literal-addressed facts, the report scoping and the budgets
  behave as units 14-18 pinned them — the relevant prior pins pass unchanged.
- **AC5** The acknowledgment is not swallowed: the rules-change turn delivers its
  answer (it does not abstain and does not emit the miss sentinel), and it carries
  the once-per-person disclosure when it is that person... no member: the disclosure
  behaves per its rule for a turn with no summoning member — pinned, the teaching
  for the acknowledgment shape pinned verbatim.
- **AC6** The documents: if any doc names the fixed acknowledgment line as product
  behavior, it is updated to the model-generated acknowledgment — pinned in the docs
  test where such a line exists.

## Notes for launch

- Branches from main (units 15-18 merged, HEAD 1891fcd; the framework media wire is
  in agent-ledger master, not needed here).
- COLD-PROBE FIRST: the debt-spine interaction (opening a turn for a service event
  with no member message) is the risk. The probe verifies whether the note-append
  path can open an answer-due turn cleanly, how the intent reaches the model, and
  whether the disclosure/co-summoner/budget readers tolerate a member-less turn —
  before the build.
