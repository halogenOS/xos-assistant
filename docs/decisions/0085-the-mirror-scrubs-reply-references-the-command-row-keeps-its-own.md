# 0085 — The mirror scrubs reply references; the command row keeps its own

Date: 2026-08-23

## Context

A reply reference stores the replied-to message's platform identifier
verbatim. The person-wide erasure reaches such copies through its
target-keyed pass, which joins repliers to the erased person's rows on the
stored origin — the very column the deletion mirror nulls. A mirror that
nulled only the target row would therefore strand a copy of the deleted
message's identifier on every row that replied to it, unreachable by any
later erasure of that message's author.

## Decision

The mirror's erasure nulls, in the same operation, the reply reference of
every row in the conversation that names the target's origin — exactly when
the target row was present, so an unknown target stays the full no-op the
silent-no-op contract pins. Both passes key on the origin the command
handed in, not the target row's column, so the scrub cannot miss by
ordering.

The deletion command row itself keeps its reply reference: it appends after
the scrub ran, and it stays kept on purpose. The reference is the lawful
record of what the request acted on; it names a message the group's
administrators removed — an act of group moderation, resolvable to nothing
in the store once the mirror ran — and it is not permanent: the
administrator's own erasure nulls it like every reply reference on their
rows. A repeated deletion of the same target matches zero target rows,
skips the scrub, and so leaves earlier command rows' records intact.

## Rejected alternatives

- **Scrubbing unconditionally, target present or not.** A `/del` naming a
  message the store never held would then null references to it — repliers
  to a pre-assistant message, say — widening the pinned no-op into a write.
- **Nulling the command row's reference too.** The record of the request
  would then name nothing, and nothing else says what was deleted; the copy
  already dies with its administrator's erasure.
- **Leaving the copies to the person-wide pass.** It joins on the origin
  the mirror nulls; after a mirror it can never reach them again.
