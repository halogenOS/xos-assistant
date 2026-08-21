# 0009 — The unit writes its own scripted provider

Date: 2026-08-21

## Context

The core spine's tests need a deterministic model: fixed answers, exact turn counts. The
framework ships no reusable scripted provider — its own out-of-crate test defines one
privately against the public provider traits, which are sufficient.

## Decision

The unit does the same: its test code implements the framework's public provider traits
with a scripted provider of its own. No framework change is expected or requested.

## Rejected alternatives

- **Extracting a shared test-support feature into the framework.** A new public surface
  for one consumer's tests, before a second consumer exists to shape it.
