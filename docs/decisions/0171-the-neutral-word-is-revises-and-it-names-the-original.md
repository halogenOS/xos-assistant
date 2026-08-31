# 0171 — the neutral word is `revises`, and it names the original

Date: 2026-08-31, with the editing unit.

## Context

The core carries no platform vocabulary, and a second platform will not
deliver an edit the way this one does. Whatever the inbound message reports
has to be true on both.

## Decision

The inbound message gains `revises`: the opaque origin of the message this
one supersedes, beside the existing origin, which stays this version's own
identifier. The value is the origin of the message as FIRST known, not of
the version immediately superseded — a third edit of one message names the
same identifier as the first, so every version shares one key and a single
match on that key reaches them all.

**What this platform guarantees, exactly.** An edit arrives under the
original's own message id, so the two identifiers are equal and every id
Telegram can name for a message — in a reply an administrator deletes, in a
report the model files, in an edit update itself — IS the shared key. On
this platform, therefore, one match reaches every version of a message,
whichever version the person naming it was looking at.

**What a platform delivering an id per revision owes.** There the two
identifiers differ, and a match on the id of a LATER version reaches that
one row while a match on the original's reaches the whole chain. So the
value this field carries has to be the chain's root, and resolving it is
that adapter's own step, owed before it reports a revision at all: it holds
the fact this one does not — which event superseded which — and the core
must not be made to walk a chain it cannot see. Nothing in the core changes
for such a platform, and the core never learns which platform it is talking
to.

## Rejected alternatives

- **A boolean "this is an edit".** It cannot say WHAT was edited, so the
  second platform needs a second field and the core grows a platform branch.
- **Deriving the relation in the core from two rows sharing an origin.**
  True here, false elsewhere, and the core would be reasoning about a
  platform's id scheme.
- **Letting the reference name the immediately-preceding version.** Erasure
  and the report would each need a recursive walk up the chain, and one
  erased link in the middle — whose origin is nulled — orphans everything
  behind it.
