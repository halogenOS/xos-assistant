# 0190 — A digest fed by erased words is regenerated, and the lineage is replaced

Date: 2026-08-31, with unit 48.

## Context

A compaction message is prose a model wrote about a stretch of conversation. When somebody
in that stretch is erased, the stored columns of their messages are nulled as always — but
prose cannot be mechanically stripped of one voice, and the digest keeps whatever it said
about them.

The decided shape (2026-08-31): the digest is scrubbed by cloning the conversation,
stripping the erased words out of the history and deleting the old one, with blocks cloned
only if they changed.

## Decision

Clone, strip, regenerate, swap, delete — per compacted lineage, copy-on-write, and the
lineage is the WHOLE ancestry.

A compacted thread can itself be compacted: `/compact` is a command anyone with the floor
may repeat, and the threshold door re-arms on the successor. The thread that comes out of
the second round carries a digest written from the first thread's older half — a half that
HOLDS the first digest. So prose about an erased person's words survives one generation on
as prose about that prose, and a chain read only one hop deep finds nothing to scrub in
either of the two newest conversations while exactly that prose keeps serving.

The ancestor-reference chain is therefore walked to its ROOT and rebuilt upward. The root is
cloned minus the erased person's blocks; then each thread standing on it gets a clone whose
digest is regenerated from the clone beneath it and whose reference names that clone;
the serving thread's clone is the last of them and is the one that takes the channel.
Everything else, every verbatim half included, is SHARED through the junction. Only what
changed is written: two blocks per hop.

The regeneration's span is PINNED, not re-derived, at every hop: exactly the clone beneath's
blocks that the thread above never inherited. The boundary needs no stored position, because
it is the complement of what that thread holds — nothing silently drops out of the serving
view and nothing is reported twice beside the verbatim half. Those blocks are a PREFIX of
the clone's ledger, because a thread's own opening appends sit at the front of it and
everything it inherited follows.

Ordering is capture-first. Nothing is swapped and nothing established is deleted until every
regenerated summary is in hand, so a failed or empty capture at any depth leaves the whole
lineage standing exactly as it was — the clones built so far go with the failure. Only past
a VERIFIED channel swap are the originals deleted, junction-only, all of them: the root and
every thread that stood on it — which is what leaves the old digests unreferenced for the
collector, and deleting that prose IS the erasure.

A thread's own ancestor reference is read by BLOCK ID, never by ledger position. A
compaction's appends are the newest blocks it writes while the rows it inherits are older,
so a thread's opening carries the highest ids in its ledger and sits at the front of it —
ids descend at that seam, and the greatest id is the one reference that is this thread's
own.

The scrub runs past the erasure fence and past the whole data erasure, for two reasons that
point the same way: it drives a model turn, and no model call may be made under the one
hold every ingestion takes; and the stored personal data is erased immediately — the scrub
completes the erasure of a DIGEST and never delays the erasure of the data.

Two readings are stated for the record: a scrub of a model-written digest means
REGENERATION and never an edit; and a scrub named in the singular, for one conversation, is
the one mechanism applied to EVERY conversation in the affected lineage, however deep it
runs.

One residual is stated rather than hidden. A scrub whose regeneration fails leaves the old
digest standing, logged with the lineage it could not rewrite; a repeat erasure of the same
principal runs it again. Making the retry automatic would mean holding the erasure fence
across a model call, which trades a logged residual for a stalled assistant.

## Rejected alternatives

- **Editing the digest in place.** Block storage is append-only, and there is no honest
  edit of prose that removes one participant from a summary of a conversation.
- **Deleting the digest and leaving the thread digest-less.** The thread would silently lose
  everything the summarized half held, for every member, because one of them left.
- **Running the scrub inside the erasure fence so a failure keeps the identity row.** The
  fence would then be held across a model call, and an ingestion stalled for a summary's
  latency is a stalled assistant. The residual above is the accepted cost of not doing it.
- **Walking one hop and calling the lineage two conversations.** It is two only until a
  thread is compacted a second time, which nothing forbids and the threshold door reaches by
  itself. A person whose words sat in the root's summarized half and nowhere else then has
  no block in either of the two newest conversations, so a one-hop reading skips the lineage
  entirely and the digest derived from their words keeps serving. The cost of the full walk
  is one model turn per hop, paid once, inside an erasure.
