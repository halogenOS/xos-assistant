# 0136 — The teaching and the description name the new questions

Date: 2026-08-30, with unit 37.

## Context

A tool that answers a question the model never routes to it is dead weight. The prompt's
identity sentence and the tool's own description are the two places that route a
question, and both listed only the model, the version, the uptime and the clock.

## Decision

Both grow the new questions, in the same shape the clock questions already have. The
identity sentence names asking which operating system or architecture the assistant runs
on, and asking what it is built on, and sends all of them to the same tool with the same
prohibition: never from memory, never from what the conversation said earlier. The
description enumerates the operating system, the architecture and the public
repositories, and lists the phrasings a member actually uses.

The verbatim pin over the identity sentence moves with the sentence, so the prompt the
suites check is the prompt the deployment records.

## Rejected alternatives

- **Growing only the description.** The identity sentence is what a model reads before
  it decides whether any tool is relevant. Left as it was, it would keep answering the
  operating-system question from training data, which is the failure the tool exists to
  end.
- **A second teaching paragraph for the host questions.** The routing decision would
  then be written in two places, and the two would drift.
