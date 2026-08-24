# Unit 16 — grounded-answer discipline (and the literal-addressed fix)

Date: 2026-08-24. Revision 1, from the live test. Supersedes the earlier
`16-abstention-discipline` draft, which this absorbs. The operator's rule,
stated plainly (msg 574): "No guessing. Even if the model says dont know, they
are not allowed to give any information from trained knowledge. The lookup must
happen and the reaction is either nothing or dont know." The live failure that
prompted it: asked "How do I use sandboxed play in xos", the assistant called
the wiki lookup (on a guessed page name that 404'd) and a commit lookup, found
nothing, and then answered from its training — "As far as I know, sandboxed
Google Play is primarily a GrapheneOS feature, unless it's been recently
integrated into XOS!" The store's tool_call/tool_result blocks confirm the
lookup happened before the answer; the defect is the trained-knowledge fill on a
miss, and the guess earlier in the turn.

Ahead of the discipline sits a regression the same test surfaced: the assistant
can no longer tell whether a message ADDRESSED it, because unit 14's
implementation recast the `addressed` column to mean "summoned" (addressed OR
helpful) and kept no separate record of the literal fact. The discipline's
addressed/unaddressed branch cannot work until that is restored — so it is the
first decision here.

## Decisions taken with this unit

- **The literal "did the user address me" fact is restored, distinct from the
  summons, 2026-08-24.** Unit 14's decision required `addressed` to keep its
  literal meaning — "the adapter still records whether the user addressed the
  assistant — and every path that genuinely needs 'did the user address me' (the
  report tool, the name trigger) reads the flag, not the debt." Its
  implementation did the opposite: `assembly.rs` computes `summoned =
  message.addressed || helpful` and `kind.rs`'s `Stamp` stores that under
  `COLUMN_ADDRESSED` (`addressed: summoned`), so in helpful mode every stored
  message reads back `addressed = true` and the literal fact is gone (the store
  confirms: "What are the rules", "How do I use sandboxed play" both
  `addressed=1`, no reply, no mention). This unit separates the two facts: the
  message carries BOTH the literal addressed fact (the adapter's
  `message.addressed`, untouched) AND the summons fact (addressed or helpful).
  The debt machinery — `own_debt_taken`, the unlatch, the budgets, `answer_due`
  — reads the SUMMONS; the report tool, the disclosure fold, the name trigger,
  and this unit's grounded-answer branch read the LITERAL addressed. The
  implementer chooses the cleanest representation (rename the recast column to
  `summoned` and restore `addressed` to literal, updating each reader to the
  fact it means; or add a distinct literal column beside the recast one) — the
  binding invariant is that after this unit the literal per-message "the user
  addressed the assistant" is recoverable and is NOT true merely because helpful
  mode picked the message up. A store migration carries existing rows: an old
  helpful-mode row cannot recover a literal fact it never stored, and defaults to
  not-addressed (the conservative reading — silence over a wrong "don't know"),
  stated. Rejected: deriving literal addressed as `summoned && !helpful` at read
  time (helpful is a mutable config, the message was stamped under a mode, and an
  addressed message in helpful mode is indistinguishable from an unaddressed one
  — the fact must be stored); leaving the conflation and keying the discipline on
  the summons (would make every helpful-mode miss a "don't know", never silence —
  the operator's exact complaint).

- **Substantive answers come only from tool lookups; trained knowledge is never
  a source, 2026-08-24.** The prompt teaches: any claim about halogenOS/XOS — a
  feature, a procedure, a fact about the project — must come from a tool lookup
  made in the turn (the wiki, the commit, the release lookups), never from the
  model's own training. The lookup happens BEFORE the answer. The assistant does
  not pad, hedge, or "as far as I know" its way past a gap: a plausible-sounding
  guess in a support group is worse than silence, because wrong ROM guidance
  costs the reader. This is enforced by teaching, not a mechanical gate: the
  model is told the tool result is its only ground, and the abstention/`don't
  know` outcomes below catch the miss. Rejected: a mechanical "no answer without
  a preceding tool call in the turn" gate (cannot classify a greeting or a social
  reply as needing no lookup, and would either silence "hello" or be trivially
  satisfied by an irrelevant lookup — the discipline is the model's, taught); a
  softer "prefer lookups" wording (the operator's rule is absolute — trained
  knowledge is not a permitted source).

- **A miss is silence when unaddressed, a plain "don't know" when addressed,
  2026-08-24.** When the lookups do not ground an answer, the outcome is keyed on
  the restored literal addressed fact: an UNADDRESSED message (no mention, no
  reply to the assistant, not its name, not a direct chat) draws SILENCE — the
  abstention sentinel, nothing reaches the chat; an ADDRESSED message draws a
  plain "I don't know" (or "I can't find that documented"), because a member who
  asked the assistant directly is owed a reply and silence would read as
  ignoring them. Neither outcome carries a shred of trained-knowledge content: a
  "don't know" states the absence and stops, with no "but as far as I know…"
  tail. "I don't know" is therefore INVALID on an unaddressed message (the
  operator's rule, msg 571) and valid on an addressed one. Rejected: "don't know"
  everywhere (noise on unaddressed messages — the complaint); silence everywhere
  (rude to someone who addressed the assistant directly).

- **Silence is the default in helpful mode; the grounded answer earns its way
  out, 2026-08-24.** Restating unit 14's intent with the test's force (owner msg
  564: the assistant "is not supposed to reply to every message"). A statement
  that asks nothing, a message setting up group content (the operator preparing a
  pin), members talking among themselves, an aside — none warrant a reply, and
  the assistant stays silent. The teaching leads with silence as the default and
  frames the grounded, genuinely-helpful answer as the exception that clears a
  bar, not a reflex a question-shaped string triggers. Rejected: a reply-rate
  limiter (the turn budget already bounds volume; this is judgment, not rate).

## The unit's contract

The core separates the literal addressed fact from the summons fact and each
reader reads the one it means (debt readers -> summons; report/disclosure/name-
trigger/grounded-answer -> literal addressed), with a store migration for
existing rows defaulting the unknown literal to not-addressed. The composed
helpful teaching (`answering_section`, `AnsweringMode::Helpful`) rewritten to
carry: the tool as the only source of substantive information; the lookup-before-
answer rule; the miss outcomes keyed on literal addressed (unaddressed ->
sentinel, addressed -> plain "don't know", zero trained-knowledge content in
either); silence as the default. The abstention sentinel mechanism, the outbound
recognition, the disclosure fold, the budgets, and the absorption path are
unchanged. Addressed-mode teaching unchanged except that its ungrounded answer is
likewise a plain "don't know" with no guessing. No configuration change, no new
dependency.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency; a previous-unit store
  upgrades cleanly (the migration for the literal-addressed split verified; old
  helpful-mode rows default to not-addressed, stated and pinned).
- **AC2** The literal addressed fact is restored: an unaddressed group message in
  helpful mode stores literal-addressed = false while still opening a debt
  (summoned = true, answer_due = true); an addressed one stores literal-addressed
  = true; the debt machinery (unlatch, budgets, answer_due) reads the SUMMONS and
  is unchanged — pinned over the wire and at the stamp.
- **AC3** Unaddressed + ungrounded -> silence: an unaddressed question the
  lookups do not ground produces the abstention sentinel and nothing reaches the
  chat — pinned against a scripted model turn that would otherwise emit an "I
  don't know" or a hedged guess.
- **AC4** Addressed + ungrounded -> a plain "don't know" is delivered (not
  swallowed), carrying no trained-knowledge tail — pinned, the addressed/
  unaddressed distinction proven by the two mirrored cases.
- **AC5** Grounded -> answer: a question the lookups DO ground (a real wiki/commit
  result in the turn) is answered from that result, in both modes — pinned.
- **AC6** The teaching text carries, verbatim-pinned in a teaching test: the tool
  as the only source of substantive claims; the lookup-before-answer rule;
  silence as the default; the miss outcomes keyed on literal addressed; the named
  no-guessing / no-hedged-knowledge prohibition. Addressed-mode teaching's
  no-guessing addition pinned.
- **AC7** No mechanism regressed: the sentinel recognition, the once-per-person
  disclosure (now reading literal addressed), the report tool's guards (reading
  literal addressed), the budgets and the absorption path behave as unit 14/15
  pinned them — the relevant prior pins still pass, updated only where they
  asserted the recast `addressed`.

## Notes for launch

- Branches from main AFTER unit 15 merges (overlaps `teaching.rs`,
  `assembly.rs`, `kind.rs`, the store schema/migrations, and the report/
  disclosure readers). Cold gap-finder probe on this spec before the build.
- The report tool (unit 15) and the disclosure (unit 12) currently read the
  recast `addressed`; this unit must update them to the literal fact and re-pin,
  since in helpful mode the recast made them see every message as addressed.
  Verify the unit-15 report guards still hold under the literal reading.
