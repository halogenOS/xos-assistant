# 0067 — The prompt may address people by the shown handle

Date: 2026-08-23

## Context

The mention capability is the reason decision 0056 sends the handle at all;
the model has to be told it may use what it now sees — and told the bound,
because a model that invents handles pings strangers.

## Decision

The system prompt's teaching gains one line: the model may mention a person
by the handle shown with their message, and must never guess a handle it
was not shown. The privacy documents were updated with decision 0056 before
this unit; the DPIA's transmitted-identifier line carries a dated note that
the projection now runs — a draft is amendable in place.

## Rejected alternatives

- **No teaching.** The capability was bought with a transmitted identifier;
  a model left to infer it may guess handles from prose, which is exactly
  the ping-a-stranger failure the bound exists to prevent.
- **Teaching the prefix format itself.** The model needs the permission and
  the bound, not the storage encoding; format prose would drift the moment
  the projection changes.
