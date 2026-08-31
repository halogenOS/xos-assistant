# 0170 — a member's edit is a new message naming what it revises

Date: 2026-08-31, with the editing unit.

## Context

Decision 0017 deferred this exactly: "an edit kind — appending the revision
as its own block — is a later unit's decision, taken when the acting policy
exists to read it." The acting policy exists. Until now an edit was fetched
and thrown away, so the assistant could answer a question its author had
already withdrawn, and the stored record of what someone said was knowingly
stale.

## Decision

The adapter stops skipping the edit update and translates it exactly like a
fresh message, with one extra fact: the origin of the message being
revised. The core appends an ordinary message block carrying that fact in a
new nullable column. Nothing is rewritten, the earlier version keeps its row
and its place, and the ledger reads as what it is: a person said one thing
and then said it differently.

## Rejected alternatives

- **Keeping the skip.** The assistant answers a question its author has
  already withdrawn, and the stored record of what someone said is knowingly
  stale — which the accuracy principle in Article 5(1)(d) argues against.
- **Updating the stored row in place.** The append-only rule, and the
  earlier version was already read, already answered and possibly already
  reported.
- **The framework's conversation fork.** It creates a second conversation,
  and the channel mapping holds one conversation per channel with a UNIQUE
  constraint. The fork exists for a composer re-running one user turn, not
  for a room where twenty people are talking. (The unit's design named the
  fork primitive as it stood before decisions 0162 and 0163 gave the session
  resets their own fork paths; those paths are a moderator's deliberate
  reset of the whole session, and neither is what a typo correction asks
  for, so the rejection stands on the same ground.)
