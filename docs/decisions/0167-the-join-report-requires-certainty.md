# 0167 — The join report requires certainty, in the teaching's own words

Date: 2026-08-30, with unit 44.

## Context

Unit 36 taught the assistant to report a join whose shown name is itself
promotional bait, and taught it with no threshold. The assistant read that as
an invitation to judge by feel and reported a joiner whose ordinary name
merely sounded promotional. The operator ruled the bar wrong: only actual
bait, obvious at a glance, may be reported — a smell is not a report.

The message half of the same teaching has carried a threshold since unit 15
— "Report only clear violations: do not report borderline calls, messages no
rule covers, or anything when no rules are pinned" — and the join half
carried none. A model filled the gap with the lowest bar available to it.

## Decision

The join sentences state the bar in their own words, quoted here exactly as
shipped: the trigger reads "itself unmistakably promotional bait", the aside
adds "obvious at a glance to anyone", and a sentence of its own settles the
suspicion case: "A name that merely sounds promotional, or that you suspect
but cannot be certain of, is not bait: report only what is beyond doubt, and
when in doubt, do nothing." The rest of the teaching is byte-identical. The
two halves of the moderation teaching now carry the same principle — clear
violations only — with the join half deliberately worded stricter, because
that is the calibration the operator ordered after the misfire.

The rules-less case needs no new sentence: the message half's "do not report
... anything when no rules are pinned" is global, and it governs joins too —
a group with no pinned rules gets no join reports.

No mechanism moves. The judgment stays the model's, exactly where unit 36 put
it, and the report stays the whole effect — decision 0070 is untouched.

Three rustdoc homes restate the trigger and move with it — the teaching
constant's doc, the join pin test's doc, and the report tool's module doc —
so no shipped documentation states a bar the teaching no longer has. Dated
unit records keep their original wording, as history.

The raised bar narrows what is REPORTED, not what is processed: every join
notice is still seen, projected and stored exactly as before. Decision 0070's
amendment, the D10 purpose text, the privacy policy and the assessments stay
true as trigger statements and as dated records, and none of them moves with
this unit.

## Rejected alternatives

- **A mechanism — a name classifier or a pattern list.** The operator asked
  for a higher bar, not a filter, and the judgment of whether a name is an
  advertisement is the model's job. A list would be wrong in both directions
  at once: blind to the bait it does not enumerate, confident about the
  innocent name that matches a word.
- **Removing the join report.** The operator wants obvious bait still
  reported on sight; what was wrong was the sensitivity, not the capability.
