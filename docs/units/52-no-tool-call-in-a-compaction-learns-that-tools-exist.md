# Unit 52 — no tool call during a compaction learns that tools exist

A conversation compacting itself is offered no tool definitions, and its palette names no
tool. What the model is never told is that this is so. Two paths carry a tool call it makes
anyway, and today both of them answer badly.

**A call naming a registered tool** draws the assistant's generic refusal, "the tool 'x' is
not in this conversation's tool palette", which describes an internal record the model has
never seen and cannot act on.

**A call naming a tool that does not exist** — the likelier case, because the model was shown
no schemas and is inventing — never reaches the assistant at all. The framework resolves the
name itself and answers with the registry's whole sorted list of tool names, as an ordinary
failure that counts toward no bound. So a compaction that hallucinates one tool call is
handed the names of every real tool in the process, and can spend the rest of the turn
calling them. That is the exact behavior this unit exists to end.

This unit has a framework half and an assistant half.

## The framework half

**An unknown-tool answer names what the TURN was offered, not what the process registered.**
The dispatch already decides what a turn is offered, by asking the turn's asking block's own
kind; that same fact decides the listing. A turn offered the registry's tools is answered
exactly as it is today, with those names. A turn offered nothing is answered that no tools
are available to it, and no name is listed. The two components read one fact through one
shared reading — the runner never re-derives what the dispatch already decided.

The existing special answer for a process with an empty registry folds into this rule: an
empty registry offers nothing, so it is the same branch, and one sentence covers both.

**That answer is a refusal, not an ordinary failure.** Today's unknown-tool answer is
deliberately an ordinary failure, because it hands the model the names that would resolve and
the next round can therefore succeed. When the turn was offered nothing, no round of it can
resolve any name, so the reasoning does not hold and the standing-no classification of
decision 0196 applies: the outcome is a refusal, and a run of refusals reaches the framework's
forced turn end. This is what bounds a compaction that keeps inventing names.

Neither change knows anything about compaction. The framework has no compaction vocabulary
and gains none.

## The assistant half

**A stored refusal on the palette kind.** The palette's content table gains a second,
nullable column holding the sentence a refusal speaks when this palette turns a tool away.
The palette value gains the matching optional field. A stored value that is absent,
unreadable, empty, or only whitespace reads as no refusal recorded — the palette's existing
fail-closed reading, extended to the new column: absence never becomes an empty sentence.

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
sentence when it does not. Either way it closes with the unchanged no-retry line, because the
fact holds for the whole turn. The refusal for a conversation carrying no palette block at
all is unchanged: there is no block there to carry a sentence. The authority refusal is
unchanged: it is not the palette turning the tool away.

The admission wrapper does not learn about compaction. It has one rule for every conversation
— the palette decides, and a palette that turns a tool away supplies the sentence. Compaction
vocabulary stays in the compaction module, which is where the instructions text already
lives. The core as a whole knows compaction perfectly well; the one component kept ignorant
of it is the rule every tool call passes through, because a general rule that secretly knows
one concrete case is the shape this project refactors away.

**A schema step.** The domain's schema list gains one appended step adding the column to the
existing content table. Rows written before it read as no refusal recorded, so an upgraded
database keeps the wording it has today until a compaction writes a palette under the new
step.

## The sentences, exactly

Today's three, unchanged by this unit and recorded here so "unchanged" is checkable:

    declined: this conversation has no tool palette recorded, and a conversation without one
    admits no tools. Do not call this tool again this turn; answer from what you already
    have.

    declined: the tool 'NAME' is not in this conversation's tool palette. Do not call this
    tool again this turn; answer from what you already have.

    declined: the tool 'NAME' needs REQUIRED authority and this turn's provenance reads
    READING — the minimum over everyone who summoned it. Do not call this tool again this
    turn; answer from what you already have.

The new one, composed from the palette's stored sentence and the unchanged no-retry line. For
a compaction it reads, in full:

    declined: tools are not available during compaction. Do not call this tool again this
    turn; answer from what you already have.

The framework's answer to a call in a turn offered no tools names the attempted name and
lists nothing. Its exact bytes are the framework half's own choice, subject to one rule: no
tool name other than the attempted one appears in it.

## What stays as it is

- The tool definitions offered to a compaction's turn: none, decided by the block kind of the
  instructions message. This unit changes nothing there; it makes the rest of the system
  agree with it.
- The spent-window refusals, their wording and their ordering ahead of name resolution.
- The palette supersession on delta: it compares tool lists and nothing else, and it writes no
  refusal. The conversations that carry a refusal are temporary summary forks, which no
  channel serves and the supersession therefore never reaches. A reviewer confirms this
  against the code instead of taking it from this document.
- The no-retry line, its spelling, and the split between whole-turn refusals and the transient
  one.

## Acceptance criteria

**Framework half.**

1. A tool call naming an unregistered tool, made in a turn that was offered the registry's
   tools, is answered exactly as it is today: the attempted name, then the registered names in
   sorted order, as an ordinary failure.
2. A tool call naming an unregistered tool, made in a turn that was offered no tools, is
   answered with a sentence that names the attempted name and no other tool name, and the
   outcome is recorded as a refusal.
3. A process with an empty registry answers criterion 2's sentence, from criterion 2's branch;
   no separate empty-registry answer survives.
4. What a turn was offered is read in exactly one place and used by both the dispatch and the
   unknown-tool answer. A test proves the two agree by exercising both against a turn whose
   asking block offers no tools.
5. A run of refusals from criterion 2 reaches the framework's forced turn end, so a turn that
   keeps inventing tool names is bounded. A test makes that run and reads the turn ending.

**Assistant half.**

6. A palette recorded with a refusal sentence turns away a tool it does not name with exactly
   `declined: tools are not available during compaction. Do not call this tool again this
   turn; answer from what you already have.` — stating neither the tool's name nor the word
   palette.
7. A palette recorded without a refusal sentence turns away a tool it does not name with
   exactly the second sentence recorded above, byte for byte.
8. A conversation carrying no palette block draws exactly the first sentence recorded above,
   and a tool whose authority is too low draws exactly the third, both byte for byte.
9. A stored refusal that is absent, unreadable, empty, or whitespace-only reads as no refusal
   recorded, and criterion 7's sentence is what such a palette speaks. The unreadable case is
   proved at the parse, because the framework's write refuses a non-text value for a text
   column and no stored row can carry one.
10. The compaction module's refusal text is exactly `tools are not available during
    compaction`, asserted verbatim by a test.
11. Both temporary-fork sites record their empty palette carrying that text; a test reaches
    the recorded block through the fork's own door instead of rebuilding the fork.
12. A test makes a real tool call against a forked compaction conversation and reads back the
    framework's refusal outcome carrying the compaction sentence, proving the whole path from
    the fork's record to the words the model receives.
13. A store opened on a database written before this unit answers criteria 7 and 8 unchanged,
    and accepts a palette written with a refusal after the schema step applies.
14. The palette's stored-fields constructor is the only encoder of a palette's fields, and the
    palette module is the only decoder. No call site assembles either column by hand.

**Both.**

15. The four checks pass with no warnings in each repository: format, clippy across the
    workspace with all targets and features, the workspace test run, and the documentation
    build.

## Rejected alternatives

- **The assistant half alone.** The original shape of this unit: give the palette a sentence
  and stop. Rejected once the framework path was read — a compaction's likeliest tool call
  names a tool that does not exist, never reaches the assistant, and is answered with the
  whole registry. The half that matters most would have been missing.
- **A compaction branch in the admission wrapper.** The wrapper would ask whether the
  conversation is a compaction and pick its wording. Rejected: it puts one concrete case
  inside the one rule every tool call passes through, and the next case that needs its own
  words adds a second branch beside it.
- **Refusing every tool call in a turn offered no tools, in the framework, ahead of name
  resolution.** Simpler, and it would cover both paths in one place. Rejected: the sentence
  the model reads would then always be the framework's, in framework words, and the assistant
  could never say why its own conversation admits nothing. The framework answers for what it
  knows — the turn was offered nothing — and the assistant answers for what it knows.
- **A framework hook letting the assistant supply the unknown-tool sentence.** Rejected: a
  consumer-facing seam carried by the framework for one case, and the framework would be
  handing out words it cannot check.
- **A separate block kind for a refusing palette.** Two kinds, one of which admits nothing
  with words. Rejected: the two would share their whole reading and differ in one field, and
  the admission wrapper would have to know both.
- **Saying it in the compaction instructions only.** The instructions already say the model is
  compacting; adding "you have no tools" there is text the model may ignore, and it produces
  nothing at all when a tool is called anyway. The refusal has to speak at the call.
- **A refusal composed from the generic sentence plus the palette's.** Rejected: the generic
  sentence describes an internal record, which is exactly the part the model cannot act on.
  The palette's sentence replaces it.

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

> Loops are the worst kind of bug. It either wastes money, wastes resources or spams people

> You are not allowed to bolt an imperative thing on it. Refactor cleanly if needed.

> a database error should hard crash the application, not leave it running in a corrupted
> state

> There is a difference between catching an expectable query error and a serious db failure.
> Something failing on a foreign key constraint is an error while a race with another writer
> is expected and can be retried if it makes sense. [...] You aren't meant to panic inside a
> db query but instead wrap and propagate the error properly so a codepath competent to
> handle it can decide what to do about it.
