# 0201 — The compaction answers under a schema and carries the rules

Date: 2026-09-13, with unit 63. Extends decision 0185.

## Context

The compaction's temporary turn used to answer in prose, and the capture took the newest
assistant text past the instructions as the summary. Six stored summaries showed that this
admits anything the model writes: a tool call spelled out as text, a greeting, an answer to
the wrong question. The design also asks that the summary include the group's rules and
that the rules lookup read recursively through the compacted lineage; neither was built,
so a compacted thread whose rules note sat in the summarized half found every rule new and
announced it to the group again.

The framework now passes a JSON Schema through to the provider, which holds the model to
it, and reads the answer back as JSON.

## Decision

The temporary fork carries the schema of one object with one list per logical unit of
the summary, the units being the instructions' own: the topics in the order they came up,
the questions asked with the answers they got, the decisions and conclusions reached, the
facts established about people, versions, settings and links, the corrections made, and
what was left open or unfinished. Every list is present and the model leaves a list empty
when the first half holds nothing for it; nothing else is allowed in the object. The
instructions say what the lists are for; the provider enforces the shape. The capture reads
the object the framework hands back and renders it as the compaction message: each unit
that holds anything under its own heading as a list, in that order, items trimmed, empty
units omitted. An answer that is not that object, or whose every unit is empty, is the
provider failing the contract it was handed, and it fails the compaction the way every
other failure does: the capture ends at once with no summary, nothing is swapped, nothing
is deleted, and the next trigger re-derives the whole compaction. No shape is repaired,
no retry runs without the schema, and no prose fallback exists.

The stored compaction message is the captured summary followed, where the lineage holds a
rules note, by two newlines and the rules line the note itself projects. The rules bytes
are the note's own, never the model's retelling. The rules note is found by one reading,
nearest first: the serving conversation's newest rules note, else the nearest ancestor's,
and so on up the chain the compaction leaves behind. The same reading serves the pinned-
rules observation, so a compacted thread that inherited no note still knows the rules its
ancestor read and announces nothing when they are re-pinned unchanged.

## Rejected alternatives

- **Validating the schema and the answer locally in the framework.** The provider already
  enforces the schema; a second validator is a second answer to the same question, and it
  was the shape that grew a parallel storage model nobody had decided on.
- **A block kind of its own for the schema.** The tool choice record already states what
  a turn is offered; the shape it answers in is the same decision about the same turn, and
  the newest record speaking for both is what stops a schema from outliving its turn.
- **Asking the model to echo the rules in a second field.** The rules would then be the
  model's bytes. The note holds the authoritative text, and the message carries it as is.
- **Copying the rules note into the successor thread as its own block.** The lineage
  lookup answers the same question without a second copy of the note. Retention may retire
  an ancestor and take its note with it; the summary the successor opens with still states
  the rules, so the model keeps them, and a later pin appends a note of the thread's own.
- **Salvaging prose at the bound.** A turn interrupted at the bound is read the same way
  as any other; text outside the schema is no summary there either.
