# Unit 44 — the bait bar rises to certainty

Date: 2026-08-30. The operator's instruction, verbatim, after the live assistant
reported a joiner named "PurePerson" as promotional bait ("that name smells like
promotional bait"): "Waaaaay too sensitive. It must be actual bait like 100% obvious.
Not just smelling." So: the join-name report fires only on a name that is
unmistakably bait, and suspicion — however strong it smells — means doing nothing.

## Grounding

The join rule rides inside `MODERATION_TEACHING`
(`crates/core/src/teaching.rs:88-96`, unit 36): "When a joiner's shown name is
itself promotional bait — an advertisement, a solicitation or a come-on carried in
place of a name — that name is the violation before the account has said anything,
and you report the join on sight". The message half of the same teaching already
carries a threshold sentence ("Report only clear violations: do not report
borderline calls…", `teaching.rs:81-83`); the join half carries none, and the live
model filled the gap with a smell test. The teaching is pinned as verbatim facts at
`teaching.rs:668-677` (the join-rule fact array; its doc comment sits at
`:662-665`) and composed under
`moderation_taught` (`teaching.rs:50,155-157`). This unit is one sentence-level
teaching change plus its pins and the two doc comments that restate the rule;
no mechanism moves.

## Decisions taken with this unit

- **The join report requires certainty, in the teaching's own words, 2026-08-30.**
  The join sentences are amended so the quoted span reads:
  `When a joiner's shown name is itself unmistakably promotional bait — an advertisement, a solicitation or a come-on carried in place of a name, obvious at a glance to anyone — that name is the violation before the account has said anything, and you report the join on sight, naming it by its bracketed id exactly as you would name a violating message. A name that merely sounds promotional, or that you suspect but cannot be certain of, is not bait: report only what is beyond doubt, and when in doubt, do nothing. Filing the report is the whole action: you never ban, kick, or reply to the joiner, and a join you do not report needs no comment.`
  The rest of the teaching is byte-identical. The threshold mirrors the message
  half's clear-violations-only rule, so the two halves of the teaching now carry
  the same evidentiary bar. The two doc comments that restate the trigger move
  with it, each to an exact replacement — the const's module doc sentence
  (`teaching.rs:66-73`, today "a shown name that is itself promotional
  bait is the violation before the account has spoken") becomes
  `a shown name that is itself unmistakably promotional bait — obvious at a glance to anyone — is the violation before the account has spoken`,
  and the fact-pin test's doc (`teaching.rs:662-665`) says
  `unmistakably promotional bait` wherever it says `promotional bait` today —
  so the shipped rustdoc never contradicts the
  teaching it documents (the unit-40 convention). No document outside the
  teaching moves: the raised bar narrows what is REPORTED, not what is
  processed, so decision 0070's amendment, the D10 purpose text, the privacy
  policy and the assessments all stay true as trigger statements and dated
  records — stated here so nobody hunts for a privacy delta. *Rejected:* a mechanism (a name classifier, a pattern
  list) — the judgment is the model's job and the operator asked for a bar, not a
  filter; *rejected:* removing the join report — the operator wants obvious bait
  still reported on sight.

## Acceptance criteria

- **AC1** The amended span is pinned byte for byte; of the seven join fact pins
  (`teaching.rs:669-676`), exactly the one asserting the old bar (line 671,
  today `a joiner's shown name is itself promotional bait`) is rewritten to
  `a joiner's shown name is itself unmistakably promotional bait` and the
  other six plus the composed
  report-on-sight check (`teaching.rs:683-688`) pass unchanged; the two doc
  comments carry the exact replacements the decision quotes; no other teaching
  text changes (the composed
  prompt diff is exactly the amended span).
- **AC2** The checks pass: fmt, clippy with warnings denied, the full suite, the
  doc build, exit codes read bare.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-bait`, branch `unit/bait-bar`, from
  `main` (`9ecd0b1`). Build first step: `git rebase main`.
- One decision record at merge, numbered from the highest shipped.
- Deploy-relevant: rides the next approved deploy alongside the other pending
  behavior fixes (their specs live in sibling worktrees, not on this branch —
  no pin coordination is needed; nothing else touches the teaching).
