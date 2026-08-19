# 0001 — Implementation language: Rust

Date: 2026-08-19

## Context

The assistant is a small, always-on service that talks to several chat platforms at once,
starting with Telegram and Matrix. All behavior lives in a platform-neutral core; each
platform connects through a thin adapter. The language had to be settled before the first
line of code, because it decides which platform client libraries are available, how the
service is packaged and deployed, and how much of the plumbing the project has to write
and maintain itself.

## Decision

The assistant is written in Rust.

Deciding factors:

- `matrix-rust-sdk` is a maintained Matrix client library and keeps end-to-end-encrypted
  Matrix reachable later without a rewrite.
- `teloxide` covers the Telegram Bot API.
- The result is a single static binary, which packages cleanly with Nix.
- A long-running service benefits from the strictness of the type system and from explicit
  failure handling.

The toolchain version is pinned in exactly one place, the Nix flake, so the recorded truth
exists once.

## Rejected alternatives

- **Kotlin** — the Matrix SDK story is weaker, the JVM footprint is large for a small
  always-on service, and Gradle packaging adds friction under Nix.
- **Gleam** — the ecosystem is young: no mature Telegram or Matrix client exists, so both
  would have to be written and maintained inside the project before any feature work could
  start.
