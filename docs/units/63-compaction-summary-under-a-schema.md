# Unit 63 — the compaction summary under a schema, with the rules

Decided 2026-09-13 and 2026-09-14. The decision record is 0201; this document is the
unit's contract and its acceptance criteria. It was written after the branch's two
commits, from the decisions that preceded them, and is what the branch is reviewed
against.

## The contract

The compaction's temporary turn is held to a JSON Schema by the provider, through the
framework's response schema (framework slice 21). The schema is one object with one list
per logical unit of the summary, every list required, no other field:

- `topics`: strings, in the order the topics came up;
- `questions_answered`: objects of `question` and `answer`, both strings;
- `decisions`, `facts`, `corrections`, `open_items`: strings.

The instructions name those lists and ask for plain prose in each item. The capture reads
the answer object through the framework's typed answer reader and renders it as the
compaction message: each unit that holds a non-blank item, under its own fixed heading,
as a list, in the order above, items trimmed; units with nothing are omitted. An answer
that is not that object, or whose every unit is empty, ends the capture at once with no
summary, and the compaction fails the way every other failure does. No shape is repaired,
no prose fallback exists, no retry runs without the schema.

The stored compaction message is the rendered summary followed, where the serving
lineage holds a rules note, by two newlines and the rules line the note itself projects.
The rules note is found nearest first: the serving conversation's newest rules note, else
the nearest ancestor's, and so on up the lineage; absence is absence. The pinned-rules
observation compares against the same lineage reading, so a compacted thread does not
re-announce rules its ancestor read. Once retention has retired the ancestor holding the
note, the thread has no rules note in reach: the next pin of the same rules is
acknowledged once and recorded as the thread's own note.

The scripted test providers answer a compaction with the object only when the request
carries the schema, and with prose otherwise, so a compaction dispatched without its
schema fails the suite.

## Acceptance criteria

1. Both temporary forks the session opens (the compaction and the regenerated digest)
   carry the compaction schema, and the schema is exactly the document stated above.
2. The instructions name the six lists and are byte-asserted.
3. A full answer renders every unit under its heading in the stated order with items
   trimmed; a unit with no non-blank item renders nothing; an object with every unit
   empty is refused; a missing list, a list of the wrong element type, a question item
   missing a field, and a non-object are each refused with a reason.
4. During capture, an answer outside the schema ends the capture with no summary without
   waiting out the bound; the same reading applies at the bound after the interrupt.
5. The compaction message carries the rules from a note in the summarized half, from a
   note past the cut, and from two hops back after a second compaction, each in the
   note's own bytes under the rendered summary.
6. Re-pinning unchanged rules on a compacted thread delivers no acknowledgment in each of
   those three cases; after the ancestor holding the note is deleted, the same pin is
   acknowledged once and the thread records its own note.
7. The lineage reading is the one used by both the compaction message and the
   observation; no second rules lookup exists.
8. The erasure scrub's regenerated digests go through the same capture and the same
   rendering.
9. `cargo test --workspace`, `cargo clippy --workspace --all-targets` (only the
   pre-existing adapter test-length warning) and `cargo fmt --all --check` are clean.
