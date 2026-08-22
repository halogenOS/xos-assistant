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
2. Under rule enforcement, after the /report bullet:
   `* /dban for permanent ban for clear violations of our no-tolerance policies.`
   Trigger: the ban path is implemented with its permission checks.
3. At the end of the tool-failure paragraph:
   `If you're genuinely encountering a situation you can't fix and believe it is a
   harness bug, please submit a model report through the feedback tools.`
   Trigger: the feedback tools exist; the maintainer offered their rough shape on
   request.

The /report bullet stays in the prompt: the reporting tool is the next unit's named
scope, and the prompt reaches production no earlier than that unit.

## Rejected alternatives

- **Shipping the gated lines and letting the missing tools fail.** The model would
  instruct itself toward commands it cannot issue correctly; a moderation command
  sent without its mechanism (an unthreaded report, a warn without permission
  checks) acts on real people in the group.
- **Keeping the placeholder prompt.** The maintainer's text is the product's actual
  voice; the placeholder existed only so the prompt seam could be built and tested.
