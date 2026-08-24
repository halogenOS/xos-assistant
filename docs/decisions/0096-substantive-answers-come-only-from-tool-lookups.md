# 0096 — Substantive answers come only from tool lookups

Date: 2026-08-24

## Context

A live test showed the assistant, after a lookup that found nothing, filling the
gap with a hedged guess drawn from its training. In a support group a plausible
wrong answer costs the reader more than silence.

## Decision

The prompt teaches that any substantive claim about the project must come from a
tool lookup made in the turn, never from trained knowledge, and the lookup
happens before the answer. "Grounded" means sufficiency, not mere presence: a
lookup result that is empty, off-topic, or missing the specific claim is a miss,
not a licence to fill the gap. A hedged guess about anything the lookup did not
confirm is forbidden, and a compound answer grounds every project-specific claim
in it or drops that claim. Enforced by teaching plus the deterministic miss
handling, not by a mechanical gate.

## Rejected alternatives

- **A mechanical "no answer without a preceding tool call" gate.** It cannot
  tell a greeting from a question and is trivially satisfied by an irrelevant
  lookup — the sufficiency problem it is meant to solve.
- **A softer "prefer lookups" wording.** The rule is absolute: trained knowledge
  is not a permitted source for a substantive claim.
