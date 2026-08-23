# Unit 15 — autonomous moderation

Date: 2026-08-24. Revision 1. The operator's design, stated from the first day and
sharpened during the test: the assistant assesses each group message against the
pinned rules and reports a violation on its own initiative. Member-initiated
reporting was redundant — anyone can invoke the moderation bot's report command
directly — so the assistant earns its place by being the one reading everything
and catching what nobody flagged. It assesses; the moderation bot and the human
administrators decide. The reasoning quality is load-bearing and named as a
requirement.

## Decisions taken with this unit

- **The report tool reports the message being assessed, 2026-08-24.** Helpful
  mode (unit 14) makes the model evaluate every group message in a turn. The
  report tool's target resolution gains that path: when the model calls it, the
  target is the SUMMONING message of the turn — the message the model is
  assessing — resolved through the dispatch anchor to that message's own origin,
  not a member's reply target. So a turn opened by a rule-violating message,
  which the model judges against the rules and decides to report, files
  `/report@<moderation bot>` as a reply to that very message. The member-
  initiated resolution (the reply-target walk from unit 8) is REMOVED: it
  duplicated a capability every member already has directly, and carrying two
  resolution paths is dead weight. The self-report and unrecorded-target guards
  stay — the summoning message is a member's, never the assistant's, so the
  self-report guard is inert here but kept for the assistant-message edge. A
  turn with no summoning member message (a deterministic command, an
  observation) offers nothing to report and the tool declines. Rejected:
  keeping both resolutions (redundant member path); a target parameter (the
  model would aim it; the assessed message is ground truth).
- **The assessment is the model's, against the rules, taught in the prompt,
  2026-08-24.** The prompt teaches: judge each message against the group's
  pinned rules held in the session; when a message clearly violates a rule that
  calls for a report, report it; when it is borderline, or the rule does not
  call for a report, or there are no rules, do not. The judgment is the model's,
  reasoned — the reasoning-effort key (defaulting low) sizes the thinking, and a
  deployment that wants sharper moderation raises it; the operator's "get the
  thinking exactly right" is recorded as the reason the level is tunable and the
  moderation teaching instructs the model to think before it reports. This is
  decision 0070 exactly: the assistant ASSESSES, the moderation bot's human
  administrators DECIDE. The assistant never bans, mutes or removes; it raises
  a hand. Rejected: a keyword/regex rule engine in the core (the rules are
  natural language and the judgment is contextual — a member quoting a banned
  phrase to ask about the rule is not a violation, which only the model sees);
  autonomous action beyond the report (the invariant forbids it).
- **A report is bounded and not repeated, 2026-08-24.** The assessment runs
  inside the message's own turn, which is already budget-bounded (helpful
  mode's per-channel and per-person budgets bound how often the model runs at
  all). A message is assessed once — its turn — so a violation is reported once;
  a later turn does not re-assess an old message. The moderation bot's own
  deduplication and the admins' review bound the downstream. A report and an
  answer are not exclusive: a turn may both answer a question and report a
  separate violation it noticed, or report and abstain from speaking — the
  report block delivers independently of whether the answer is spoken or
  swallowed (the unit-14 abstention path). Rejected: a per-channel report rate
  limit distinct from the turn budget (the turn budget already bounds
  assessment frequency; a second limit would suppress genuine violations in a
  bad hour).
- **The documents move, 2026-08-24.** The policy's moderation sentence changes
  from "when a member asks" to the assistant's own assessment: it reads group
  messages, judges them against the pinned rules, and reports a violation to the
  group's moderation bot for the administrators to decide; it takes no action
  itself. The impact assessment records autonomous assessment as a new
  processing purpose under the same legitimate interest, with the human-decides
  bound and the reasoning dependency named; its review trigger for a
  standing-touching capability fires and is answered here. The compliance page
  notes the assessment is not an automated decision with legal effect (Article
  22): the effect is a report to humans who decide, and the reasoning is
  logged. The AI-Act standing-capability trigger (recorded in the DPIA) is
  addressed: this is assessment producing a report a human judges, not an
  effect.

## The unit's contract

The report tool's target resolution replaced: the summoning message's own
origin through the dispatch anchor, the member-reply-target walk removed with
its now-dead provenance helper. The prompt's moderation teaching: assess
against the pinned rules, report a clear violation, think first, never act.
The report tool stays member authority (it needs no elevation — it files to
the moderation bot, it does not moderate), palette-governed, group-only, the
self-report and unrecorded-target guards retained. The report-and-answer
independence per unit 14. The document updates enumerated. No new
configuration beyond the existing reasoning key. No autonomous action of any
kind beyond filing the report.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** Autonomous report end to end over the adapter: a group message that
  violates a pinned rule opens a turn (helpful mode), the model assesses and
  calls the report tool, the tool resolves the target to that message's own
  origin, and `/report@<handle>` is filed as a reply to it against the scripted
  wire — pinned block by block; the model may also answer or abstain in the
  same turn and the report delivers regardless.
- **AC3** No report when there is nothing to report: a rule-compliant message
  assessed and not reported; a message with no rules in session not reported; a
  turn with no summoning member message (deterministic command) declines — the
  model's judgment pinned via a scripted decision, the tool's decline pinned
  structurally.
- **AC4** The member-initiated resolution is gone: the report tool no longer
  reports a member's reply target; the removed provenance helper has no
  caller; a member replying to a message and asking gets an answer, not a
  reflexive report (the model assesses the message it is in a turn for) —
  pinned.
- **AC5** The guards hold: the summoning message resolving to the assistant's
  own message declines (self-report); an unrecorded target declines; the report
  is filed once per assessing turn — pinned.
- **AC6** The documents ship: the policy's assessment sentence, the DPIA
  purpose and the compliance Article-22 note, the prompt teaching, the removed
  member-report decision recorded — pinned in the docs test.
