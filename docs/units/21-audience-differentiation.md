# Unit 21 — the assistant reads its audience and asks before it assumes

Date: 2026-08-24. Revision 2, tightened after a cold probe. A community question
can come from an END USER who wants to USE the ROM, or from a BUILDER/DEVELOPER
who wants to IMPLEMENT or integrate something — different questions, different
answers. Shown "How do I use sandboxed play in xos", the assistant found the
right wiki page but answered it as if the asker were building a ROM ("If you're
looking to integrate Sandboxed Google Play into your build… use the corresponding
branch… merge or cherry-pick the commits…"), locking into the developer reading
of an end-user question. This unit teaches the assistant to notice the ambiguity
and ask ONE short clarifying question instead of committing to an assumption.

## Grounding (what the probe settled)

- This is a TEACHING change only. The whole answering register — silence, the
  sourcing/lookup discipline, the sentinels — lives in `crates/core/src/teaching.rs`
  (`answering_section`, `sourcing_rules`, `sentinel_rules`), NOT in the base prose:
  `prompts/assistant.md` is persona / rule-enforcement / tool-usage prose only and
  carries no answering-mode content. So the change lands in `teaching.rs`; the base
  prose is not touched.
- The mechanical risk does NOT exist: a clarifying question is ordinary model text,
  stored and delivered as `ReplyKind::Answer` (there is no "question" kind). The
  outbound edge special-routes only an answer whose WHOLE trimmed text is exactly
  `[[abstain]]` or `[[miss]]` (`abstention.rs` exact match, `outbound.rs`); any other
  text falls through to ordinary delivery, disclosure fold included. So nothing can
  swallow a clarifying question as a miss or abstention — no mechanism change is
  needed. The real risk is prompt-level: teaching the model to emit the question
  (prose) rather than reach for `[[miss]]` on a turn where it made no lookup.

## Decisions taken with this unit

- **The assistant distinguishes using from building, and asks ONE question when the
  message does not say which, 2026-08-24.** The composed teaching gains the
  distinction: many questions read one way to an end user (use it on my phone) and
  another to a developer (integrate it into a build), and the right answer differs
  sharply. On a genuinely ambiguous question the assistant asks ONE brief clarifying
  question — "are you asking how to use it on your device, or how to build it into a
  ROM?" — and stops there. When the intent IS clear from the message or the context,
  it answers directly; the clarifying question is for real ambiguity, not a reflex.
  Rejected: always answering the most technical reading (the live failure); asking a
  clarifying question on every message (annoying — most questions are not ambiguous).
- **A clarifying question is a warranted answer, exempt from the lookup-backing rule
  without weakening it, 2026-08-24.** Unit 16's Helpful teaching states an answer
  "must be one you can back with a lookup" (the `AnsweringMode::Helpful` branch of
  `answering_section`). A clarifying question is an answer that asks WHICH question
  is being asked — it makes no substantive claim about the project, so it needs no
  lookup. This unit reconciles that specific sentence, not "the teaching" in the
  abstract: the lookup-backing rule binds a SUBSTANTIVE CLAIM (a fact about the ROM);
  asking the member to disambiguate is not a substantive claim and is a warranted
  reply that neither abstains nor emits the miss sentinel. The grounding rule is
  unweakened — the moment the disambiguated real answer is given, it needs its lookup
  exactly as before. The wording carves the exception narrowly (a question back to
  the member), so it cannot become a licence to answer a substantive question without
  a lookup. Rejected: leaving the "must be lookup-backed" sentence unreconciled (the
  probe's finding — the model would face a literal contradiction and resolve it on
  its own); broadening the exception to any non-lookup answer (would gut unit 16).
- **At most one clarifying question per thread; the disambiguation is answered, not
  re-interrogated, 2026-08-24.** After the assistant asks its one clarifying
  question, the member's reply arrives as an ordinary later message and the assistant
  ANSWERS the now-disambiguated question (applying the lookup discipline to that real
  answer). The teaching says plainly: do not chain clarifying questions — if the
  reply is still not perfectly clear, make the best grounded answer for the most
  likely reading rather than asking again, so the exchange cannot loop into repeated
  interrogation. Rejected: leaving each turn to independently re-ask (the probe's
  repeat-clarification-loop — the same annoyance the unit exists to avoid, one turn
  later).
- **The audience teaching reads the message, never profiles the person, and applies
  in both modes with the addressed-mode reach noted, 2026-08-24.** The assistant
  reads the audience from what the message and the conversation show — the words, the
  level, prior turns — not from guesses about who the person is; it recognizes when
  the QUESTION is ambiguous between use and build, and asks. The teaching goes in the
  shared rules, so it applies in both answering modes. One reach note: in helpful
  mode every group message opens a turn, so the member's disambiguating reply is seen
  and answered; in ADDRESSED mode only a message that addresses the assistant (a
  mention, a reply to it, its name) opens a turn, so a plain unaddressed follow-up is
  not seen — the clarifying question there gently invites the member to reply to it,
  and the operator contract notes that addressed-mode follow-ups must address the
  assistant. This is a documentation note, not a mechanism change. Rejected: profiling
  the member's expertise (the same ungrounded-assertion failure the project avoids).

## The unit's contract

The change is entirely in `crates/core/src/teaching.rs`: the shared answering
teaching gains the use-versus-build distinction, the ask-ONE-clarifying-question-on-
ambiguity rule, the answer-directly-when-clear rule, the do-not-chain-clarifying-
questions rule, and the reconciliation of the "an answer must be lookup-backed"
sentence so a clarifying question is a warranted, lookup-exempt reply. `prompts/
assistant.md` is not touched. No mechanism change — the sentinels, the miss routing,
the disclosure fold, the report path, and the lookup discipline for substantive
claims are all unchanged; a clarifying question is delivered as an ordinary answer
and composes with every one of them. The operator contract gains the addressed-mode
follow-up note. No configuration, no new dependency.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** The teaching text carries, verbatim-pinned in the teaching test (in BOTH
  modes, since the rules are shared): the use-versus-build distinction; the
  ask-one-clarifying-question-on-ambiguity rule; the answer-directly-when-clear rule;
  the do-not-chain rule; and the reconciled statement that a clarifying question is a
  warranted reply needing no lookup while a substantive claim still needs one.
- **AC3** An ambiguous question draws a clarifying question, delivered as an ordinary
  answer (NOT swallowed by the miss/abstention routing): a scripted model turn on a
  genuinely ambiguous use/build question delivers the clarifying question to the chat
  — pinned. This is verified as ORDINARY delivery through the sentinel checks, not via
  the mid-turn absorption harness (the disambiguating reply is a later, separate turn,
  not a within-turn absorption).
- **AC4** A clear question is answered directly, not interrogated: a scripted turn on
  an unambiguous question delivers the answer with no clarifying question — pinned.
- **AC5** The disambiguation is a normal two-turn exchange: after the clarifying
  question, a following disambiguating message opens its own turn and the assistant
  answers the now-clear question (the prior clarifying question visible in the turn's
  projected context) — pinned as two sequential turns, not one absorbed turn.
- **AC6** A clarifying question, as a new person's first delivered answer, carries the
  once-per-person disclosure line (delivery is content-agnostic past the sentinel
  checks) — pinned, matching unit 16's precedent of pinning a new answer shape's
  disclosure composition.
- **AC7** No mechanism regressed: the sentinels, the miss routing, the disclosure fold
  and the report/co-summoner machinery behave as units 12/14/16/15 pinned them — the
  relevant prior pins pass unchanged; `abstention.rs`, `outbound.rs`, the schema and
  `assembly.rs` are untouched.

## Notes for launch

- Branches from main (units 15-20 merged). Teaching-only change in `teaching.rs`.
- Accepted residual (stated plainly, per unit 16's precedent): the scripted-provider
  harness proves the MECHANICAL property — a clarifying question is delivered as an
  ordinary answer, never swallowed (already true today) — but cannot prove a LIVE
  model reliably tells "genuinely ambiguous" from "clear" and asks only when it
  should. That judgment is the unit's point and rests on the teaching; a live-model
  check against the original failing prompt would raise confidence but is not required
  for correctness. Not a blocker.
