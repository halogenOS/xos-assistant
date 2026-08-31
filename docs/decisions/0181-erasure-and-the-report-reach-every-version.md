# 0181 — erasure and the report reach every version

Date: 2026-08-31, with the editing unit.

## Context

One message can now stand in the ledger as several rows.

## Decision

The revision reference is personal data of its author, so the author-keyed
pass nulls it with the other five columns. The mirror's named erasure
matches the origin OR the revision reference and nulls the revision
reference too, so "delete this message" reaches every recorded version of
it. Because every version stores the original's id, one match on that id
reaches a chain of any length — and on this platform every id a deletion
can name IS that id, since an edit arrives under the original's message id
and an administrator's reply carries it whichever version they were reading.
On a platform delivering an id per revision the same match reaches the whole
chain from the original's id and the one row a later version's id names,
which is why an adapter there owes the root-resolution step decision 0171
states.

The report tool's resolution matches the named id against either column.
Which version it resolves is defined rather than left open: the first match
in turn order, the earliest version present in the turn's assessment set.
Nothing depends on the choice — the only facts the tool reads from that row
are the role and the principal id, and both are identical across versions of
one message — and the report block carries the id, not the text. Decision
0092 stands unchanged: one report per message, not per version.

**The author fact is enforced, not assumed.** That the versions of one
message share an author holds on this platform because only a message's own
author can edit it, and the two shapes that would break it never reach the
core. It is not left resting on that: the ingestion's own newest-version
read carries the recorded author beside the text, and a revision whose
reviser is a different principal records as an ordinary new message with no
reference at all. The words are never refused — only the link falls away —
so no platform reporting an implausible relation can put one person's
identifier into another person's row, which is what would otherwise owe a
target-keyed erasure pass for this column.

## Rejected alternatives

- **One report per version.** An assessment per keystroke, and an
  edit-spammer could manufacture a report flood aimed at their own message.
- **Resolving deliberately to the newest version.** An extra ordering read
  that buys nothing, because no fact the tool reads differs between the rows.
- **Storing the assessed text in the report block so the evidence is
  fixed.** It would send message content to a recipient that receives none
  today, which is a change of what a recipient receives, not a bug fix.
