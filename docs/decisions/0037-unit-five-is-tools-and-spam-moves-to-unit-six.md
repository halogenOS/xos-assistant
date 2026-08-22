# 0037 — Unit five is the tools; spam moves to unit six

Date: 2026-08-22

## Context

Decision 0029 sliced unit five as "the tools with admission and the spam
reporting". The spec review for that unit found the two halves separable: the tool
layer is capability wiring, the spam policy is product behavior on top of it.

## Decision

The stage is re-sliced once more: unit five ships the tool layer — the two lookup
tools, the palette, the admission wrapper — and unit six ships spam detection and
reporting as the last build unit. The report tool then inherits an admission gate
that already exists and is already pinned, instead of arriving beside it.

## Rejected alternatives

- **Keeping the bundle.** One unit carrying both a new capability layer and the
  first policy built on it — twice the review surface, and a spam-policy dispute
  would block the tools it does not depend on.
- **Spam first.** The report tool is the one tool with an authority requirement
  above member today; shipping it before the admission gate exists would ship it
  ungated.
