# 0148 — The heads-up line coexists with the no-filler rules by being scoped and bounded

Date: 2026-08-30, with unit 40.

## Context

Two standing rules read, at first glance, as forbidding the new sentence. The conduct
prose matches length to the message's weight and says that restating someone's own words
back at them adds nothing. The silence teaching says that a turn with nothing to say ends
without writing any text — no placeholder.

Neither rule is being relaxed. The new sentence has to be worded so that it cannot be
read as permission for filler.

## Decision

The teaching words the line as a real statement of what is being looked up, with its
bounds written into the sentence itself: one line and no more, stating the thing the
assistant is going to look for, never a placeholder standing in for an answer, and never
a restatement of the words the member just wrote.

The last clause settles the silence teaching's reading in the prompt, where the model
will meet it: ending a turn with no text is for a turn with nothing to say, and a turn
with a search to run has something to say. The two rules stand exactly as written; the
line fits inside them because it carries information the member does not have — that a
search is happening, and what it is for.

## Rejected alternatives

- **Amending the silence teaching to carve out an exception.** The rule is about a turn
  with nothing to say. Carving into it would weaken the strongest sentence in the
  answering teaching to solve a wording problem in another one.
- **A fixed line the model repeats.** A stock phrase before every search is the
  placeholder the rules forbid, and it would tell the member nothing about what is being
  looked up.
- **Leaving the bounds to the conduct prose alone.** The prose is general; a sentence
  that invites a line before slow work has to carry its own limit, next to the invitation,
  or the limit is one indirection away from where it is needed.
