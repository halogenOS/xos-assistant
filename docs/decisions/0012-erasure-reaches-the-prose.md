# 0012 — Erasure reaches the prose

Date: 2026-08-21

## Context

The core-spine unit first scoped erasure to identity rows and channel mappings, leaving a
person's message text stored in the block kind's content table and still projected to the
model. The unit's correctness review flagged that against decision 0003, which requires
the text itself to be erasable: the two documents contradicted each other, and 0003 is
the older, settled authority. This record resolves the contradiction on 0003's side and
states the mechanism.

## Decision

**The kind's content table is the separate personal-data table 0003 calls for.** Under
the framework's block model a block is the immutable header row — position, kind, links —
and its content lives in a per-kind table referenced by the block's key. That is
precisely the separation 0003 describes, so no second prose table is needed: the text
column in the content table is nullable, and erasure nulls it.

Erasing a principal is one call running three idempotent steps:

1. The principal's message text is set to null in every conversation. The block header
   is never touched; positions, references and conversation order all keep their shape.
   An erased message projects nothing to the model.
2. The principal's direct conversations are removed entirely, mappings included. A
   two-party chat that lost its human is metadata that still identifies the person —
   title, timestamps, its very existence — and serves nobody. Conversation-level
   removal leaves no holes inside a shared history and strands no cross-references,
   which are the two failure modes 0003 rejected block deletion for.
3. The principal's identity rows are deleted.

Erasing a principal id that matches nothing reports the not-found outcome rather than
succeeding idly.

The nulled set is every personal column the kind stores — the text, the origin
reference and the platform send time. What remains on an erased message is structure:
role, authority level and a principal id that no longer resolves to anything.

OPEN: a group conversation's derived title may have been shaped by prose that is now
erased. Regenerating titles on erasure is later work; it is surfaced here rather than
silently accepted.

OPEN (2026-08-21, from the closing verification): an erased message is
boundary-invisible in the projection — it projects nothing but still ends the
contiguous run of its neighbours, so a conversation with an erased message in the
middle can project two same-role messages in a row, and one erased at the front can
open with the model's own voice. Live providers that demand strict alternation reject
such requests; the live-model unit must either normalize the shape at its encoder or
settle it in the framework's fold before it ships.

OPEN (2026-08-21, from the closing verification): erasure is ordered against
ingestion by the assembly's fence but not against a conversation's open stream; a
direct conversation could be deleted mid-stream. The observed failure direction is a
loud error on the stream's finalizing write, not silent retention. Settling the
ordering belongs with the unit that first runs live streams.

## Rejected alternatives

- **An out-of-row prose table with the content table holding only a key.** The
  framework's projection reads a kind's declared descriptor fields; there is no load
  path that enriches a block from a second table, and building one would duplicate the
  content table's own mechanism to arrive at the same separation one join further away.
- **Keeping direct conversations as erased skeletons.** Consistent with step 1 alone,
  but the surviving metadata is itself the person's data, and a skeleton conversation
  has no reader.
- **Leaving the text and recording the gap as open work.** That was the state this
  record replaces; a data-protection promise with its central case missing is not a
  smaller version of the promise.
