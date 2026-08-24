# Unit 16 — grounded-answer discipline (and the literal-addressed fact)

Date: 2026-08-24. Revision 2, rewritten after a cold probe found that revision 1
would have broken two just-merged units. The operator's rule (msg 574): "No
guessing. Even if the model says dont know, they are not allowed to give any
information from trained knowledge. The lookup must happen and the reaction is
either nothing or dont know." The live failure: asked "How do I use sandboxed
play in xos", the assistant looked up (a guessed page name that 404'd) and a
commit, found nothing, and answered from training — "As far as I know, sandboxed
Google Play is primarily a GrapheneOS feature…".

Revision 1 tried to key the miss outcome on a per-message "addressed" fact the
MODEL would read, and to re-point the report tool and the disclosure fold onto
that literal fact. The probe showed both are wrong: the model cannot see whether
a message addressed it (a reply to the assistant leaves no mark in the
projection at all), and the report tool and disclosure fold are deliberately
built on the SUMMONS (`co_summoners` / `own_debt_taken`), so re-pointing them
would make an absorbed unaddressed message unreportable (breaking unit 15) and
would deny the AI-disclosure line to a person whose first interaction was
unaddressed-but-summoned (breaking decision 0078). Revision 2 keeps every
existing summons reader untouched, restores the literal fact for ONE new
mechanism consumer, and moves the miss decision out of the model entirely.

## Decisions taken with this unit

- **The literal addressed fact is restored ALONGSIDE the summons, read by one
  new consumer only, 2026-08-24.** Unit 14's implementation stores the summons
  fact (`addressed || helpful`) in `COLUMN_ADDRESSED`, and the whole debt spine
  reads it there on purpose (the budget counts — including the raw SQL
  `COUNTED_DEBT_SQL` and the `idx_..._principal_addressed` index — the unlatch,
  the co-summoner rule, the disclosure fold, the report tool's `co_summoners`
  scoping). All of that STAYS exactly as it is: the recast column keeps meaning
  summons, and no summons reader is touched. This unit ADDS a second, separate
  per-message fact — the literal "the user addressed the assistant"
  (`message.addressed` as the adapter recorded it, before the mode folded in) —
  stored beside the summons, and read by exactly ONE new consumer: the outbound
  miss-routing below. Because the recast column is not renamed, the raw SQL and
  the index (probe Gap B) need no change; because no existing reader moves
  (probe Gap A), unit 15's report scoping and unit 12's disclosure keep their
  meaning. Rejected: renaming `addressed` to `summoned` and re-pointing readers
  (revision 1 — breaks the report scoping and the disclosure duty, and silently
  misses the raw-SQL/index readers); deriving the literal fact from `summoned &&
  !helpful` (helpful is mutable and an addressed helpful-mode message is
  indistinguishable — it must be stored).

- **Substantive answers come only from tool lookups; trained knowledge is never
  a source, and a lookup that does not actually answer is a miss, 2026-08-24.**
  The prompt teaches: any claim about halogenOS/XOS — a feature, a procedure, a
  project fact — must come from a tool lookup made in the turn (wiki, commit,
  release), never from the model's training, and the lookup happens before the
  answer. Crucially, "grounded" means the lookup result actually answers the
  question: a result that is empty, off-topic, or does not contain the specific
  claim is a MISS, not a licence to fill the gap (probe Gap E — this is one step
  from the live 404 failure). A hedged guess ("as far as I know", "probably",
  "unless it's changed") about anything the lookup did not confirm is forbidden;
  in a support group a plausible wrong answer costs the reader more than silence.
  A compound answer grounds every halogenOS-specific claim in it or drops that
  claim (probe Gap G). Enforced by teaching plus the deterministic miss handling
  below, not by a mechanical "no answer without a preceding tool call" gate (that
  cannot tell a greeting from a question and is trivially satisfied by an
  irrelevant lookup). Rejected: a mechanical lookup gate; a softer "prefer
  lookups" wording (the operator's rule is absolute).

- **A miss is signalled by its own sentinel and routed by the mechanism, not the
  model, 2026-08-24.** The model cannot reliably tell whether a message addressed
  it (probe Gap D), so it does not decide the miss outcome. Instead there are TWO
  sentinels: the existing abstention sentinel (unit 14) for social silence —
  members talking among themselves, nothing to add — which always delivers
  nothing; and a NEW unresolved-lookup sentinel the model emits as its whole
  answer when it looked and could not ground an answer. The outbound edge
  recognizes the miss sentinel on the raw answer (like the abstention one, before
  any disclosure prepend) and routes it by the LITERAL addressed fact of the
  message that summoned the turn (the dispatch anchor's message): unaddressed →
  deliver nothing (silence, exactly like an abstention); addressed → deliver a
  fixed, plain "don't know" line (a named constant, no trained-knowledge tail).
  So the model's job is only to be honest that it found nothing; the machine,
  which holds the fact, decides whether the asker is owed a reply. The
  addressed-miss "don't know" is a delivered first answer like any other: it
  carries the once-per-person disclosure line when it is that person's first
  spoken answer (the disclosure fold, unchanged, still summons-keyed). An
  unaddressed miss delivers nothing and — like an abstention — introduces nobody
  and spends no disclosure. Rejected: the model choosing silence vs don't-know
  (it cannot see the reply-addressed channel — Gap D); marking the addressed fact
  into the projection for the model to read (leaks the internal flag and still
  rests on model discipline for the choice); one sentinel for both social-silence
  and miss (the mechanism could not tell a "nothing to add" from a "found
  nothing", and would wrongly answer "don't know" to an addressed "lol").

- **Silence is the default in helpful mode; the grounded answer earns its way
  out, 2026-08-24.** Restating unit 14's intent with the test's force (owner msg
  564: not a reply to every message). A statement that asks nothing, a message
  setting up group content, members talking among themselves — none warrant a
  reply; the assistant stays silent (the abstention sentinel). The teaching leads
  with silence as the default and frames the grounded answer as the exception
  that clears a bar. Rejected: a reply-rate limiter (the turn budget already
  bounds volume; this is judgment).

## The unit's contract

The core stores a second per-message fact, the literal addressed flag, beside
the unchanged summons column; NO existing summons reader moves — the budgets
(including the raw `COUNTED_DEBT_SQL` and its index), the unlatch, `co_summoners`
/ `own_debt_taken`, the report tool's scoping, and the disclosure fold all keep
reading the recast column as summons. The store migration adds the literal
column; historical rows take a safe default and are never read for their literal
value (only the current turn's dispatch-anchor message is read, at outbound
time). The composed helpful teaching (`teaching.rs`, `AnsweringMode::Helpful`)
rewritten: the tool as the only source of substantive claims; the
lookup-before-answer rule; a lookup that does not answer is a miss; the two
sentinels (social-silence vs unresolved-lookup) named exactly; silence the
default. A new miss sentinel constant beside the abstention sentinel
(`abstention.rs` or a sibling), recognized at the outbound edge on the raw answer
and routed by the dispatch anchor's literal addressed fact to nothing
(unaddressed) or the fixed "don't know" constant (addressed), the latter flowing
through the disclosure fold like any first answer. Addressed-mode teaching gains
only the no-guessing rule (its ungrounded answer is likewise the fixed
"don't know"; in addressed mode every summoning message is literally addressed,
so the miss always delivers the line). No configuration change, no new
dependency.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency. A previous-unit store
  upgrades cleanly: the migration adds the literal column with its safe default,
  and NO existing pin that asserts the recast `addressed`(=summons) changes
  meaning (the summons readers are untouched) — verified and stated.
- **AC2** The literal fact is stored beside the summons without disturbing it:
  an unaddressed helpful-mode message stores summons=true (opens its debt, counts
  its budget, joins `co_summoners`) AND literal-addressed=false; an addressed one
  stores both true; the report scoping and the disclosure fold behave exactly as
  units 15/12 pinned them (the relevant prior pins pass unchanged) — pinned.
- **AC3** Unaddressed miss → silence: a turn summoned by an UNADDRESSED message
  whose model answer is the miss sentinel delivers nothing, prepends no
  disclosure, introduces nobody — pinned, deterministically (the routing reads
  the stored literal fact, not model text).
- **AC4** Addressed miss → a fixed "don't know" line delivered, carrying the
  once-per-person disclosure when it is that person's first spoken answer, and
  carrying no trained-knowledge tail — pinned; the addressed/unaddressed split
  proven by the two mirrored cases over the same miss sentinel. Addressed mode:
  the miss always delivers the line.
- **AC5** Social-silence stays silent regardless of addressing: an ADDRESSED
  message whose model answer is the abstention (not miss) sentinel delivers
  nothing and is not converted to "don't know" — pinned (the two sentinels are
  distinct and routed differently).
- **AC6** Grounded → answer: a question whose in-turn lookup actually answers it
  is answered from that result in both modes; a lookup that returns something
  empty or off-topic is treated as a miss (the model emits the miss sentinel),
  not answered from it — the sufficiency rule pinned by a scripted turn whose
  tool result does not contain the claim.
- **AC7** The teaching text carries, verbatim-pinned: the tool as the only source
  of substantive claims; lookup-before-answer; the sufficiency rule (an
  unanswering lookup is a miss); silence as the default; both sentinels named
  with their distinct meanings; the no-guessing / no-hedged-knowledge
  prohibition. Addressed-mode teaching's no-guessing addition pinned. The fixed
  "don't know" copy pinned as a named constant.
- **AC8** No mechanism regressed: the abstention recognition, the disclosure fold
  (summons-keyed), the report tool's `co_summoners` scoping, the budgets and the
  absorption path behave as units 14/15 pinned them — the relevant prior pins
  still pass, unchanged.

## Notes for launch

- Branches from main (unit 15 merged, HEAD ee87184). Overlaps `teaching.rs`,
  `assembly.rs`, `kind.rs`, `abstention.rs`/outbound recognition, the store
  schema/migration.
- Probe residual named, not blocking: no acceptance criterion replays the exact
  live prompt against a recorded model transcript (probe Gap F). With the miss
  routing now DETERMINISTIC in the core (the model only signals a miss; the
  machine decides silence vs don't-know), the model-dependent surface shrinks to
  "does the model emit the miss sentinel instead of guessing", which the teaching
  governs and a scripted turn pins; a live-transcript replay would add
  confidence but is not required for correctness. Stated as the accepted residual.
