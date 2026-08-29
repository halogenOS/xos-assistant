# 0135 — The three rows' bytes, exactly

Date: 2026-08-30, with unit 37.

## Context

The fact list is prose a model reads and quotes. Its rows have a settled order and a
settled shape, and anything vague about the new ones would be settled differently by
the next reader of the code.

## Decision

The three rows append after the time row, in the order `os:`, `arch:`, `source:`. The
source row states both repositories, joined by one comma and one space, the assistant's
own first and the framework's second.

The distribution and the architecture arrive at the rendering as arguments, like every
other value that varies from build to build or from call to call, so nothing in the
rendering can go stale behind a caller's back. The two homes vary on no build, so they
are written in the rendering beside the row labels, which do not vary either.

The rendering is pinned character for character, with the homes appearing in the pin as
the literal text a model would read.

## Rejected alternatives

- **Two source rows, one per repository.** Where the software lives is one fact; two
  rows invite the model to state one of them and drop the other.
- **The framework's repository first.** A member asking where the assistant's source
  lives should read the assistant's own repository first; the framework is the answer
  to the follow-up question.
- **Reading the distribution inside the rendering.** The rendering would then reach the
  host, which is the one place a byte pin cannot control, and the rows a caller passes
  would no longer be the rows it gets.
