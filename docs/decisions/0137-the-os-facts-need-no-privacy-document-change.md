# 0137 — The OS facts need no privacy-document change

Date: 2026-08-30, with unit 37.

## Context

Every unit that puts something new into a result asks whether the published privacy
documents still describe what the assistant does. Assuming the answer is how a
processing description quietly stops matching the software.

## Decision

The three facts describe the software and the distribution it runs on: no person, no
message content, no identifier of anybody, and no new recipient. The result rides the
conversation that already exists, to the model processor that already receives it. The
published documents describe that path already, so none of them changes.

This is checked against the published documents themselves, not accepted from this
paragraph — the convention the runtime-facts unit set when it made the same claim.

## Rejected alternatives

- **Assuming it silently.** A unit that adds to what leaves the process and records
  nothing about privacy leaves the next reader to re-derive the reasoning, and the next
  reader after that to skip it.
- **Amending the documents anyway, to be safe.** A processing description that lists
  things it does not process is as wrong as one that omits things it does, and it
  trains readers to skim.
