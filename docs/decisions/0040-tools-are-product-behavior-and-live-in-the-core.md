# 0040 — Tools are product behavior and live in the core

Date: 2026-08-22

## Context

The tools had to live somewhere: the core, the adapter, or a crate of their own.
The core's invariant bans platform vocabulary — chat-platform vocabulary.

## Decision

The tools sit in the core's tool module tree. The project's own forge and
releases are the product, not a chat platform, so naming them in the core keeps
the invariant intact. The core gains its first network dependency for them: the
HTTP client at the framework's own major version — one HTTP stack, one TLS story
across the workspace — recorded in the dependency review before the manifest
named it.

## Rejected alternatives

- **A separate tools crate.** A boundary with no consumer behind it: nothing but
  the core would ever depend on it.
- **Tools in the adapter.** The adapter translates, it does not act; a tool in an
  adapter would be behavior outside the core, the exact thing the architecture
  forbids.
