# 0087 — Answering is a mode, and the summons is stamped at the write

Date: 2026-08-23

## Context

The operator wants the assistant to help with a question asked into the
group even when nobody mentioned it, abstaining freely when it has nothing
to add — and named the economics: with prompt caching the marginal read is
cheap at the community's traffic. The core's answering machinery is a debt
spine: the entry point stamps every message once at the write, and the
budgets, the unlatch emission, the co-summoner rule of decision 0043 and
the disclosure fold of decision 0078 all read the stored stamp.

## Decision

A configuration key `answering` takes `helpful` (the default) or
`addressed`. In `addressed` mode a group message summons a turn only when
it addresses the assistant — the prior behavior. In `helpful` mode every
group message summons a turn, and the model decides whether to speak,
abstaining through the sentinel of decision 0088.

The mode enters the machinery at exactly one place: the entry point's
summons resolution ahead of the write-time stamp. The stored stamp's
addressed column is recast as the SUMMONS fact — the adapter's addressed
resolution, or helpful answering's every-message evaluation — decided once
at the write, exactly as the column's own contract already demands.
Everything behind the stamp is therefore mode-free and unchanged: the
budget counts count summoned unlimited messages, the unlatch fires on a
taken debt, an unaddressed helpful message is a co-summoner and draws the
first-interaction line, and mid-turn absorption inherits all of it. A
rate-limited member's message is stamped limited at the write and opens no
turn — under helpful answering that member's flood costs zero model calls,
the free quiet of the protection unit's existing mechanism.

The mode is configuration read at start; a changed mode reaches new writes
only, and stored rows keep the summons their write decided — no reader
ever re-derives a stamp against a mode that may have changed since.

## Rejected alternatives

- **Helpful with no off switch.** A different community may want the quiet
  shape; the key costs one closed word.
- **A per-message answerability heuristic in the core.** That judgment is
  the model's; a keyword check would both miss real questions and waste
  the model's own abstain.
- **A mode parameter threaded through the stamp's readers.** The budgets'
  SQL, the provenance walk, the disclosure fold and the report and privacy
  tools would each grow a mode argument, and rows written under one mode
  would be re-read under another — the write-once stamp keeps history
  meaning what it meant when it was written.
- **A separate helpful trigger beside the addressed one.** A second
  summoning path would need its own budget consultation, unlatch emission
  and provenance treatment; folding the mode into the one predicate keeps
  one spine.
