# halogenOS Group Assistant

A community assistant for [halogenOS](https://halogenos.org), the performance-focused,
open-source Android distribution. It answers community questions and helps with recurring
group tasks across the project's chat platforms, Telegram first, through a shared
platform-neutral core and one thin adapter per platform.

**Status: pre-alpha.** The repository holds the project scaffold only — a Rust workspace
with empty crates, a Nix development shell, and the decision records. There is no behavior
yet.

## Development

Cargo runs inside the Nix development shell, which provides the toolchain:

    nix develop -c cargo test --workspace
    nix develop -c cargo clippy --workspace --all-targets -- -D warnings
    nix develop -c cargo fmt --check

## Layout

- `crates/core` — the platform-neutral core: conversation handling, knowledge lookup,
  command semantics, rate and abuse protection.
- `crates/adapters/telegram` — the Telegram adapter: translation between the Telegram Bot
  API and the core's message model.
- `docs/decisions` — decision records, numbered in order.

The architecture rules that bind every change, including the two invariants that separate
the core from the adapters, are written down in `CLAUDE.md`.
