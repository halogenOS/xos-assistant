# 0088 — The model abstains through a fixed sentinel, swallowed at the edge

Date: 2026-08-23

## Context

Under helpful answering (decision 0087) every group message summons a
turn, so the model needs a way to choose silence: members talking among
themselves, questions it has no information on, lookups that come back
empty. The turn still costs its read — the operator's accepted economics —
but must speak nothing, and must not spend an answer-window slot, because
the window bounds what the assistant SAYS.

## Decision

The prompt teaches the model to answer only when it can genuinely help and
to stay silent by emitting the fixed abstention sentinel — a named
constant — as its WHOLE answer. Recognition is exact on the raw trimmed
finalized content, shared by three readers:

- **The outbound edge** delivers a recognized abstention as nothing: no
  send, no first-interaction introduction (a spoken-to-nobody answer
  introduces nobody), the block accounted delivered. The recognition runs
  BEFORE the disclosure resolution — a prepended line would both
  un-recognize the sentinel and record an introduction nobody received.
  The turn is already closed by its own committed answer, so an abstained
  debt propagates nothing.
- **The projection** skips a recognized abstention: the framework's kinds
  reach the composed kind through a wrapping delegate whose one judgment
  is that such an answer is invisible to the model — the same kind-level
  seam decision 0027 used. The skip is boundary-invisible, so the user
  runs around it project as two same-role messages; unlike 0027's erased
  runs, an abstention is this product's own design and its answering runs
  over one vendor wire that accepts same-role adjacency, and the
  alternative — a non-empty placeholder — would feed the model its own
  machinery token as prose. The residual is recorded here: a strict
  same-role-rejecting vendor binding would need 0027's marker shape
  instead.
- **The budget counts** exclude a debt whose anchored answer is the
  sentinel: the count's SQL subtracts every summoned unlimited message
  whose stored answer — matched through the answer's dispatch anchor, the
  summoning frontier id every block a turn writes carries — is exactly the
  sentinel. Two recorded residuals: the anchor names the frontier alone,
  so a co-summoner absorbed into an abstained turn keeps its own row's
  slot spent; and the SQL trims ASCII whitespace where the edge trims the
  full class, so a sentinel wrapped in exotic whitespace is swallowed but
  still counted — both err toward limiting, never toward flooding.

An ordinary answer that merely quotes the sentinel's words as prose is
never swallowed: the sentinel is the whole answer or it is no abstention.

## Rejected alternatives

- **A tool the model calls to abstain.** A round trip for silence.
- **A confidence threshold in the core.** The judgment is the model's, not
  a number's.
- **Rewriting or deleting the stored abstention block.** The ledger stays
  the honest record of the turn; the projection judgment reads it without
  touching it.
- **Projecting the abstention as a fixed marker (0027's shape).** Keeps
  strict-vendor alternation but reads a machinery token to the model as
  its own past prose; taken only if a strict binding ever ships.
- **Counting answers instead of debts for the window.** A redesign of the
  protection unit's derived-from-the-ledger counts; the one exclusion
  keeps the shipped counting.
