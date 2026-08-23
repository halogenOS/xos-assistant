# 0046 — The system prompt is the maintainer's, with gated sections

Date: 2026-08-22

## Context

The maintainer authored the assistant's system prompt and handed it over with
instructions: light adjustments and additions are welcome, and two kinds of content
are marked to stay OUT of the shipped prompt until their mechanisms exist. The prompt
replaces the placeholder written with the live-model unit.

## Decision

**The prompt files carry the maintainer's text; edits stay light.** The shipped prompt
is the handed-over text with the gated sections removed and nothing else changed
beyond formatting. Larger changes go back through the maintainer.

**Gated sections, each with its re-entry trigger.** The following lines are part of
the intended prompt but are held out until the named mechanism ships; when it does,
the line returns to the prompt verbatim:

1. Under rule enforcement, after the light-reminder bullet:
   `* /warn command when replying to the flagged user message.`
   Trigger: the warn path is implemented with its permission checks.
2. Under rule enforcement, after the light-reminder bullet (and after the /warn
   bullet once that returns):
   `* /report when it needs human judgment`
   Trigger: unit 6b ships the report tool. Moved into this set 2026-08-22: the
   prompt can now reach production before the tool, so the bullet waits with the
   others instead of instructing the model toward a command that does not exist yet.
3. Under rule enforcement, after the light-reminder bullet (and after the /warn
   and /report bullets once those return):
   `* /dban for permanent ban for clear violations of our no-tolerance policies.`
   Trigger: the ban path is implemented with its permission checks.
4. At the end of the tool-failure paragraph:
   `If you're genuinely encountering a situation you can't fix and believe it is a
   harness bug, please submit a model report through the feedback tools.`
   Trigger: the feedback tools exist; the maintainer offered their rough shape on
   request.

The /report bullet stays in the prompt: the reporting tool is the next unit's named
scope, and the prompt reaches production no earlier than that unit.

> Superseded 2026-08-22, and restored the same day with this note: the paragraph
> above was this record's original text, and the move of the /report bullet into the
> gated set (item 2, with its dated note) was written by deleting it in place — an
> edit the append-only convention for decision records forbids. The standing rule is
> item 2's; this paragraph stands as the history it always was.

## Rejected alternatives

- **Shipping the gated lines and letting the missing tools fail.** The model would
  instruct itself toward commands it cannot issue correctly; a moderation command
  sent without its mechanism (an unthreaded report, a warn without permission
  checks) acts on real people in the group.
- **Keeping the placeholder prompt.** The maintainer's text is the product's actual
  voice; the placeholder existed only so the prompt seam could be built and tested.
