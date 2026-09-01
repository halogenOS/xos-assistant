# Unit 52 — a palette says why it admits nothing

A conversation compacting itself is offered no tools and admits none, and the model is told
so twice over: the compaction's turn carries no tool definitions, and the compaction's
palette names no tool. What the model is never told is **why**. A tool call made anyway —
one the model invented, or one carried over from a habit of the conversation above — draws
the assistant's generic refusal, "the tool 'x' is not in this conversation's tool palette",
which describes a record the model has never seen and cannot act on.

This unit gives the palette a second stored fact: the words a refusal speaks when this
palette turns a tool away. The compaction writes its own words onto the palette it records.
The admission wrapper reads them back and says them.

## What this is not

The admission wrapper does not learn about compaction. It has one rule for every
conversation — the palette decides, and a palette that turns a tool away supplies the
sentence. Compaction vocabulary stays in the compaction module, which is where the
instructions text already lives. The core as a whole knows compaction perfectly well; the
one component kept ignorant of it is the rule every tool call passes through, because a
general rule that secretly knows one concrete case is the shape this project refactors away.

## The change

**A stored refusal on the palette kind.** The palette's content table gains a second,
nullable column holding the refusal sentence. The palette value gains the matching optional
field. A stored value that is absent, unreadable, empty, or only whitespace reads as no
refusal recorded — the palette's existing fail-closed reading, extended to the new column:
absence never becomes an empty sentence.

**One encoder.** The palette's stored-fields constructor takes the refusal beside the tool
names. There is no second constructor and no second encoding path; every writer passes the
refusal or passes nothing.

**The compaction module owns its words.** The compaction module holds the refusal text as a
constant beside the compaction instructions, asserted verbatim by a test the same way the
instructions are, so an edit to it is deliberate. The text is, exactly:

    tools are not available during compaction

Both places that fork a temporary conversation for a summary — the compaction of a
conversation's first half, and the digest a session regeneration takes over the span its
successor did not inherit — record their empty palette with that refusal on it. Every other
palette writer records none.

**The admission wrapper reads it back.** The refusal for a tool the palette does not name is
built from the palette's own sentence when it carries one, and is the existing generic
sentence when it does not. Either way the sentence closes with the unchanged no-retry line,
because the fact holds for the whole turn. The refusal for a conversation carrying no palette
block at all is unchanged: there is no block there to carry a sentence. The authority refusal
is unchanged: it is not the palette turning the tool away.

**A schema step.** The domain's schema list gains one appended step adding the column to the
existing content table. Rows written before it read as no refusal recorded, so an upgraded
database keeps the wording it has today until a compaction writes a palette under the new
step.

## What stays as it is

- The tool definitions offered to a compaction's turn: none, decided by the block kind of
  the instructions message, in the framework. This unit adds nothing there.
- The palette supersession on delta: it compares tool lists and nothing else, and it writes
  no refusal. The conversations that carry a refusal are temporary summary forks, which no
  channel serves and the supersession therefore never reaches. A reviewer confirms this
  against the code instead of taking it from this document.
- The no-retry line, its spelling, and the split between whole-turn refusals and the
  transient one.
- The refusal is a typed refusal on the outcome row, per decision 0196. Unchanged.

## Acceptance criteria

1. A palette recorded with a refusal sentence turns away a tool it does not name with a
   message stating that sentence and closing with the no-retry line, and stating neither the
   tool's name nor the word palette.
2. A palette recorded without a refusal sentence turns away a tool it does not name with
   exactly today's message, byte for byte.
3. A conversation carrying no palette block at all draws exactly today's no-palette message,
   byte for byte, and a tool whose authority is too low draws exactly today's authority
   message, byte for byte.
4. A stored refusal that is absent, unreadable, empty, or whitespace-only reads as no refusal
   recorded, and criterion 2's message is what such a palette speaks.
5. The compaction module's refusal text is exactly `tools are not available during
   compaction`, asserted verbatim by a test.
6. Both temporary-fork sites record their empty palette carrying that text; a test reaches
   the recorded block through the fork's own door instead of rebuilding the fork.
7. A test makes a real tool call against a forked compaction conversation and reads back the
   framework's refusal outcome carrying the compaction sentence, proving the whole path from
   the fork's record to the words the model receives.
8. A store opened on a database written before this unit answers criterion 3 unchanged, and
   accepts a palette written with a refusal after the schema step applies.
9. The palette's stored-fields constructor is the only encoder of a palette's fields, and the
   palette module is the only decoder. No call site assembles either column by hand.
10. The four checks pass with no warnings: format, clippy across the workspace with all
    targets and features, the workspace test run, and the documentation build.

## Rejected alternatives

- **A compaction branch in the admission wrapper.** The wrapper would ask whether the
  conversation is a compaction and pick its wording. Rejected: it puts one concrete case
  inside the one rule every tool call passes through, and the next case that needs its own
  words adds a second branch beside it.
- **A separate block kind for a refusing palette.** Two kinds, one of which admits nothing
  with words. Rejected: the two would share their whole reading and differ in one field, and
  the admission wrapper would have to know both.
- **Saying it in the compaction instructions only.** The instructions already say the model
  is compacting; adding "you have no tools" there is text the model may ignore, and it
  produces nothing at all when a tool is called anyway. The refusal has to speak at the call.
- **A refusal sentence composed from the generic one plus the palette's.** Rejected: the
  generic sentence describes an internal record, which is exactly the part the model cannot
  act on. The palette's sentence replaces it.

## Decisions appendix

The user's words, verbatim.

2026-08-31, on what a compaction's turn must do about tools:

> It simply doesn't have tool schemas and any tool run returns an error that tools are not
> available during compaction. that prevents the model from endlessly hallucinating tool
> calls and outputs and invent its own session history

> Implement what's needed in the framework then add this

2026-09-01, correcting a claim that the core would not learn compaction:

> Why does the core not learn compaction? The core should support compaction.

Standing, from earlier units and unchanged here:

> Thats what tests are for you need to test every scenario

> You are not allowed to bolt an imperative thing on it. Refactor cleanly if needed.

> a database error should hard crash the application, not leave it running in a corrupted
> state

> There is a difference between catching an expectable query error and a serious db failure.
> Something failing on a foreign key constraint is an error while a race with another writer
> is expected and can be retried if it makes sense. [...] You aren't meant to panic inside a
> db query but instead wrap and propagate the error properly so a codepath competent to
> handle it can decide what to do about it.
