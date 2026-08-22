# 0020 — The OpenRouter key lives in memory

Date: 2026-08-22

## Context

The live model speaks through the framework's OpenRouter provider module. That module
persists provider configuration — the API key included — into the store, and the
project's secrets rule forbids a secret in the store: the store file is long-lived,
backed up, and outlives any key rotation.

## Decision

The assembly registers a thin provider wrapper whose configuration lives in process
memory and whose persistence hooks are inert, delegating the wire entirely to the
framework's OpenRouter binding. This is not a bespoke provider: no wire code is
duplicated, only the configuration's residence changes. The acceptance scan asserts
the key is absent from the store file after a turn that provably used it.

Surfaced to the framework's improvements list: an in-memory provider-configuration
seam, so consumers stop needing wrappers like this one.

## Rejected alternatives

- **Relaxing the secrets rule to admit the store.** The store file's lifetime is the
  problem itself; a rotated key would survive in every backup.
- **A bespoke provider speaking the wire directly.** Duplicates request shapes,
  streaming and error handling the framework already owns, and drifts from them.
