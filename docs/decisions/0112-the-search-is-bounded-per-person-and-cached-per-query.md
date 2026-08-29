# 0112 — The search is bounded per person and cached per query

Date: 2026-08-27; keyed, sized and its cache key defined 2026-08-29, with unit 27.

## Context

Each search is a paid request to a metered vendor, and the model — not the member —
chooses when to make one. Nothing else in this assistant spends money per call: the
answering budgets bound how often the assistant answers, and the rights window bounds
courtesy lines, but neither bounds a vendor's bill. A member who asks one question can
draw a run of searches from a model that decides to reword and page through results.

## Decision

The search is admitted at member authority — it exists to answer members' questions —
and takes a per-person windowed budget in the shape of the reply bound the command
family already uses, with its own named constants: five searches per person per ten
minutes.

The PERSON is the principal over the turn's debt-origin set, resolved exactly as the
rights commands resolve it. A turn holding zero or several distinct principals
declines the spend with a taught result, the rights precedent's shape: the per-person
guarantee never folds several people into one bucket. A spent budget declines with a
fixed result naming the bound and when it reopens, and nothing is sent.

Beside it sits a same-query cache: the query as written, case-folded and
whitespace-collapsed, plus the page. A cache hit costs no vendor spend, so it is
served even on a spent budget and resolves no person at all — the budget's stated
ground is metered spend and nothing else. A failed request hands its grant straight
back, because a refused key bills nothing.

## Rejected alternatives

- **No budget.** One member's curiosity, amplified by a model that likes to page,
  drives unbounded spend.
- **A global budget alone.** One member exhausts it for the whole group.
- **Keying the spend on the tool call's context when the origin set is plural.**
  Several people's spend in one bucket under one person's name, which is the one
  thing a per-person bound must not do.
- **Reusing the guard's normalisation as the cache key.** Two jobs on one mechanism:
  the guard's normalisation exists to find one token, and a key stripped its way
  would merge queries a member deliberately wrote differently.
- **No cache.** Identical retries inside one turn or one conversation each bill.
