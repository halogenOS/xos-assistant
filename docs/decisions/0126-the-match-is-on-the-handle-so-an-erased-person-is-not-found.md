# 0126 — The match is on the handle, so an erased person is not found

Date: 2026-08-25, with unit 29.

## Context

Erasure nulls a message's text, origin, send time, reply reference and speaker, and
leaves the standing and the principal id standing: the standing is structure, not
personal data. Which key this lookup matches on therefore decides an erasure outcome,
and cannot be left to whoever writes the code.

## Decision

The match is on the stored handle. An erased person's rows keep a standing and have no
handle, so nothing matches them and the lookup answers the unshown refusal. That is
the correct outcome: their erasure was honoured, and this tool is not a way back to
them.

The tool takes the erasure fence at registration, as both non-lookup peers do, so a
lookup cannot answer from a row an erasure is in the middle of clearing.

## Rejected alternatives

- **Matching through the principal id.** It would report the surviving standing of
  somebody whose erasure was honoured — the exact trap this decision exists to name.
- **Handle to principal to that principal's latest row.** The same defect, and it
  breaks a second way: a released username reassigned to a new person would answer
  for two different people under one handle.
