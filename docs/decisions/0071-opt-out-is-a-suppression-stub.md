# 0071 — Opt-out is a suppression stub, the one lawful remnant

Date: 2026-08-23

## Context

The privacy-self-service unit lets a person opt out of collection. Honoring
the objection going forward needs the system to remember WHO objected —
forgetting the identifier would silently resume collection on the person's
next message — while erasure promises the identity data gone. The two
promises meet on one row.

## Decision

The identity row survives an opt-out as a stub carrying the opt-out flag —
the suppression-list shape, lawful because storing the identifier is what
honoring the objection takes. The flag is a boolean column on the identity
table, added by an appended migration step (`INTEGER NOT NULL DEFAULT 0`; a
boolean, no frozen vocabulary — the schema's own precedent), and
adapter-scoped like the identity it hangs on: opting out on one platform is
opting out there, and the fixed copy says "on this platform". Erasure
leaves the flag standing.

From the moment the flag stands, the person's inbound messages are DROPPED
at ingestion — the full no-write claim: no message row, no identity
refresh, no principal write, no conversation creation, no palette append,
no mapping. The outcome reuses the disregarded variant, whose reading
widens to "refused without effect at the person's own ask or the operator's
switch"; the adapter acknowledges and the offset advances, exactly as for
the direct-chat refusal.

Opt-out is not deletion and does not reach backward, and the fixed copy
states all three edges plainly: what was stored before stands until
deletion, it keeps being projected to the model with later turns, and a
pre-flag unanswered question may still draw its one answer.

## Rejected alternatives

- **Dropping the identity row.** Collection would silently resume on the
  person's next message — the objection forgotten by the very mechanism
  meant to honor it.
- **Suppressing the stored history's projection.** A second projection
  switch for content whose deletion already exists on request; deletion is
  the right tool for the stored past.
- **Cancelling pre-flag debts.** A turn mid-flight cannot be recalled
  anyway; the flag stops collection, deletion stops history, and blurring
  the two would promise more than either delivers.
