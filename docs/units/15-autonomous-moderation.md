# Unit 15 — autonomous moderation

Date: 2026-08-24. Revision 2, rewritten after the cold probe found the parameterless
target undefined under helpful-mode absorption. The assistant assesses each group
message against the pinned rules and reports a violation on its own initiative;
member-initiated reporting is removed as redundant. The assistant assesses, the
moderation bot's administrators decide (decision 0070). The reasoning quality is
load-bearing and is sized by the reasoning-effort key.

## Decisions taken with this unit

- **The tool names its target, bounded to the turn's own assessment set,
  2026-08-24.** Helpful mode folds several messages into one turn by absorption,
  so "the message being assessed" is not singular — the probe's deepest finding.
  The report tool therefore takes ONE parameter: the offending message, named by
  its stored origin (the platform message id the model sees in the projected
  conversation). The resolution VALIDATES that origin against the current turn's
  co-summoner set (`co_summoners` over the turn's dispatch anchor): the named
  origin must belong to a message the model is actually assessing this turn. A
  named origin outside that set — an old message, an arbitrary id, another
  channel — is refused. So the model gains the precision to pick the one
  violator among several absorbed messages, and cannot aim the report at
  anything it is not currently assessing. The member-initiated reply-target
  resolution (unit 8's `newest_co_summoner_reply` / `stored_reply` /
  `StoredReply`) is removed; `co_summoners` STAYS — it is shared with the
  privacy tool, and now also validates the report target. Rejected: a
  parameterless "the assessed message" (undefined under absorption); an
  unvalidated origin parameter (the model could aim at any message); reporting
  every violating co-summoner automatically (the model's judgment picks, one
  report per call).
- **A message is reported at most once, by per-origin dedup, 2026-08-24.** A
  message that dies unanswered re-co-summons the next turn (the marker-aware
  walk), so without a bound it could be re-assessed and re-reported. The report
  block already stores its target origin; the tool checks for an existing report
  of the named origin in the conversation and declines a duplicate. This replaces
  unit 8's per-channel `REPORT_WINDOW` — a five-minute channel-wide limit that
  would suppress DISTINCT violations from different members in a bad hour, the
  exact harm to avoid. Per-origin dedup is precise: each violating message is
  reported once, however many turns re-assess it, and a busy hour of genuine
  violations is not throttled. Rejected: the per-channel time window (suppresses
  distinct genuine reports); no dedup (double-reports on the die-after-filing
  re-summon path the probe found).
- **The assessment is the model's, taught only where it can act, 2026-08-24.**
  The prompt teaches the model to judge each message against the group's pinned
  rules and report a clear violation, thinking first; borderline, rule-absent,
  and no-rules cases are not reported. The moderation teaching is composed into
  the prompt ONLY when a moderation handle is configured AND the mode is helpful
  — the two conditions autonomous assessment needs (the tool registers only with
  a handle; only helpful mode shows the model every message). A deployment
  without a handle, or in addressed mode, teaches no moderation and the tool is
  absent — no instruction to use a capability that is not there. The judgment is
  reasoned: the reasoning-effort key sizes the thinking (default low, raised for
  sharper moderation), and the teaching instructs the model to reason before it
  reports. This is decision 0070: the assistant ASSESSES, the administrators
  DECIDE; it never bans, mutes or removes. Rejected: a keyword engine in the core
  (rules are natural language, judgment is contextual — a member quoting a banned
  phrase to ask about it is not a violation); teaching moderation unconditionally
  (a tool that isn't registered).
- **The rules are guaranteed in the model's context at assessment, 2026-08-24.**
  The model can only judge against rules it can see. The pinned rules reach the
  model as a context note; this unit requires that the newest rules note is
  present in every projected request while a rules note exists — the projection
  keeps it, the way it keeps the system prompt, rather than letting it fall out
  of a windowed history. If the projection does not already guarantee this, the
  unit makes it so (the rules note projects like the system prompt, not like
  ordinary history), and an acceptance criterion pins the newest rules note in
  the request the model assesses on. Rejected: assessing against rules that may
  have scrolled out of context (silent moderation failure).
- **The base prose carries no rules and no stale engagement or report copy,
  2026-08-24.** The embedder's base prose (`prompts/assistant.md`) held a
  hardcoded "Community rules, applies to everyone" list and a member-initiated
  report instruction — both leaked in the live test: asked for the group's
  rules, the model recited the base file's placeholder five instead of the
  three the operator had pinned, and under this unit it would have MODERATED
  against those placeholders, not the group's real rules. A group's rules are
  runtime data, never prompt prose. So the base prose is reconciled to hold
  only what is invariant across group and mode — the persona, the honest-AI
  stance, the privacy-tool mechanics, the work ethic — and the group's rules
  reach the model through exactly ONE channel: the pinned rules note
  (`RULES_NOTE_LEAD`, "The group's rules are now:"), guaranteed in context by
  this unit. The base prose loses the hardcoded rules list entirely; loses the
  member-initiated report paragraph (superseded by this unit's composed
  moderation teaching and the report tool's own reframed description); and
  loses the addressed-only "stay quiet during regular conversations" framing
  that contradicts unit 14's composed answering teaching, which alone owns when
  the assistant speaks. The composed moderation teaching states that the only
  rules are the pinned note's, and that with no rules note present there are no
  rules — the model says so plainly, invents none, and reports nothing.
  Rejected: leaving the hardcoded list (the leak itself, and moderation against
  the wrong rules); a second rules source beside the pinned note (two truths,
  the exact drift this unit exists to close); scrubbing the leak in a separate
  unit (it is this unit's own subject — the rules the model assesses on).
- **The tool's wording and guards are reframed for assessment, 2026-08-24.** The
  tool's description and result lines are rewritten from "a member's reply to the
  offending message" to "the message you are assessing that violates the rules,
  named by its id." The guards: the named origin must be in the turn's
  co-summoner set (else decline, "that message is not one you are assessing");
  the named message must have a stored origin and a reported principal (else
  decline, an unrecorded message names nobody erasure can reach); the resolved
  principal must not be the assistant's own (the self-report guard, now reachable
  because the model could in principle name the assistant's own message id — it
  declines). Each decline teaches no-retry. The exact copy of every line ships as
  named constants, pinned.
- **A report and an answer are independent; false positives are bounded and
  recorded, 2026-08-24.** A turn may both answer and report, or report and
  abstain from speaking — the report block delivers regardless of whether the
  answer is spoken or swallowed (unit 14's abstention touches only the answer).
  A wrong report pings the administrators, who decide and can ignore it; the
  false-positive exposure to the reported member is recorded in the impact
  assessment as the accepted residual of an assessment-only capability, bounded
  by the human decision and the reasoning level, and the reported member's
  erasure reaches the report block (the stored principal, unit 8's two-ended
  erasure). Rejected: an autonomous consequence beyond the report (the invariant
  forbids it); hiding the false-positive residual (recorded, per the honest-
  documentation discipline).
- **The documents move, 2026-08-24.** The policy's moderation sentence becomes
  the assistant's own assessment: it reads group messages, judges them against
  the pinned rules, and reports a violation to the moderation bot for the
  administrators to decide, taking no action itself. The impact assessment
  records autonomous assessment as a processing purpose under the same
  legitimate interest, with the human-decides bound, the reasoning dependency,
  and the false-positive residual. The compliance page states the assessment is
  not an Article-22 automated decision with legal effect: the output is a report
  to humans who decide, not an effect on the member; it does not overclaim a
  reasoning-audit trail the artifact does not keep — it states what is stored
  (the report names its target and principal) and that the decision is the
  administrators'. The AI-Act standing-capability trigger is answered here.

## The unit's contract

The report tool gains a validated origin parameter; the reply-target resolution
(`newest_co_summoner_reply`/`stored_reply`/`StoredReply`) is removed and
`co_summoners` retained (shared, now the validator). Per-origin dedup replaces
`REPORT_WINDOW` (removed from the report path; the notice window is untouched).
The moderation prompt teaching composed only when a handle is configured and the
mode is helpful. The newest rules note guaranteed in the projected request while
one exists. The tool description, result lines and guards reframed for
assessment with exact copy. The enumerated document updates. The report stays
member authority (it files to the moderation bot, it does not moderate),
palette-governed, group-only. No autonomous action beyond the report.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** Autonomous report end to end over the adapter: a group message
  violating a pinned rule opens a helpful-mode turn, the model assesses and
  calls the tool naming that message's origin, the origin validates against the
  turn's co-summoner set, and `/report@<handle>` is filed as a reply to that
  message against the scripted wire — pinned block by block; the turn may also
  answer or abstain and the report delivers regardless.
- **AC3** Target validation: a named origin NOT in the turn's co-summoner set is
  declined (the anti-aiming guard); with several messages absorbed, the model
  names the specific violator and only that one is reported (the multi-
  co-summoner shape the probe raised) — pinned.
- **AC4** No report when there is nothing to report: a rule-compliant message
  not reported; no rules in session, not reported; a message already reported is
  not reported again on re-assessment (per-origin dedup, the die-after-filing
  re-summon path) — pinned.
- **AC5** The guards: the named message resolving to the assistant's own
  principal declines (self-report, now reachable); an unrecorded / origin-less
  target declines; each decline teaches no-retry with the exact reframed copy —
  pinned. The member-initiated reply-target resolution is gone and its helpers
  have no caller (co_summoners excepted) — pinned.
- **AC6** The rules reach the model: the newest rules note is present in the
  projected request the model assesses on, while a rules note exists — pinned.
- **AC7** Gating: the moderation teaching is in the prompt only with a handle
  configured and helpful mode; absent either, no moderation teaching and no
  registered tool — pinned. The documents ship: the policy assessment sentence,
  the DPIA purpose + false-positive residual, the compliance Article-22 note,
  the removed-member-report decision — pinned in the docs test.
- **AC8** The base prose carries no rules and no stale copy: the shipped
  `prompts/assistant.md`, and the composed system prompt built from it, contain
  no hardcoded community-rules list and no member-initiated report instruction
  — pinned by a content assertion. With a pinned rules note in session, a
  rules-question turn answers from the note's text and cites nothing else; with
  no rules note in session, the model states there are no rules set, invents
  none, and moderation reports nothing (the no-rules path of AC4) — pinned. The
  base prose no longer instructs "stay quiet during regular conversations"; the
  composed answering teaching alone governs when the assistant speaks — pinned.
