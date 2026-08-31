# 0173 — an unchanged revision, and one naming a message the store does not hold, record nothing

Date: 2026-08-31, with the editing unit.

## Context

The platform documents that an edit update "may at times be triggered by
changes to message fields that are either unavailable or not actively used
by your bot" — a link preview attaching, hours later. And erasure nulls a
row's origin along with its text, so an erased message matches no origin
lookup at all.

## Decision

Under the stamp lock, ahead of the palette reconciliation, ingestion asks
the store for the newest recorded version of the named message in that
conversation, and disregards the update in two cases: when the incoming text
is identical to that version, and when the store holds no version of the
message at all. Nothing is written and the update is acknowledged. The
privacy command family is exempt from both, in the one condition it is
already exempt from the suppression re-read: a rights command is answered
whatever the store holds.

The identical-text drop is not a protection mechanism and decision 0030 is
untouched: what is dropped is a redelivery of content the ledger already
holds, byte for byte, under that same message — no statement a person made
goes unrecorded, and a genuinely different edit always records, however many
times the person makes one.

**The idempotency it buys, stated exactly.** The comparison reads the NEWEST
recorded version and nothing else, so redelivery is idempotent for the TAIL
version alone: an update redelivered after a halted batch, with no other
version recorded in between, records once. A redelivered update that a later
version has already superseded is a different case — its text differs from
the tail, and nothing in the row distinguishes it from a member returning to
their earlier wording, so it records. That is the residual, and it is
accepted in this direction on purpose: the alternative is comparing against
the whole history, which would silently swallow a person's genuine return to
an earlier wording. A redelivered NEW message still duplicates, exactly as
it did before this unit.

**Two further residuals of the unheld-target drop.** An edit whose original
the store has not recorded — a delivery reordered ahead of it, or an
original the ingestion never recorded at all — hits the drop, and the
corrected words are not recorded. And after a session reset or a compaction,
which fork the conversation and point the channel at the fork, an edit of a
message the serving conversation no longer holds hits the same drop: the
read is scoped to the conversation being served, by design, since a match
across conversations would reach a stranger's row. Both are named in the
impact assessment's addendum beside the two the unit already carried.

The unheld-target drop is the erasure guard. Recording such a revision as a
fresh statement would write a person's erased words, and their erased
identifier, back into the ledger with no human act anywhere in the path.
What is given up is the case where an edit adds text to a message the store
never held — a caption typed onto a photo that arrived without one. That
message is not in the ledger, nobody has read it, and nothing about the
group's memory silently changes; an erased message resurrecting itself is a
defect against a published promise.

## Rejected alternatives

- **Comparing in the adapter.** An adapter decides nothing, and it holds no
  store.
- **A new outcome variant for "nothing changed".** Every adapter would have
  to match a case it must treat exactly like the existing disregard.
- **Recording every update faithfully.** A duplicate of the same sentence in
  the model's context, and a turn for every link preview.
- **Bounding the volume by a budget instead.** A budget refuses an ANSWER,
  never a row (decision 0030), so it cannot bound rows at all.
- **Recording an unheld target as an ordinary new statement.** The erasure
  defect above.
- **A stored marker distinguishing "erased" from "never held".** Erasure
  exists to leave nothing that points at a person's removed message, and a
  per-message tombstone is that pointer under another name.
- **Distinguishing a person-generated edit from a platform-generated one by
  the edit time.** The platform documents no such guarantee, and building on
  it would be building on folklore.
