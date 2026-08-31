# 0178 — the model is taught which version is meant

Date: 2026-08-31, with the editing unit.

## Context

Two versions of one message stand in the ledger, both projected, under one
id.

## Decision

A shared rules section in the teaching, composed into both answering modes
beside the sourcing and audience rules, states: a message may appear again
marked as edited under the same id; the edited version is what the person
now means, so answer that one; when the earlier wording was already answered
and the edit does not change what was asked, end the turn with no text. The
last clause needs no new mechanism — an empty turn already delivers as
nothing.

## Rejected alternatives

- **A mechanical suppression of a second answer to the same id.** The
  machine deciding what a person meant by their own edit, and the assistant
  then falling silent on a genuinely rewritten question.
