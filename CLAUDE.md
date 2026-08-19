# halogenOS Group Assistant

A community assistant for [halogenOS](https://halogenos.org), the performance-focused,
open-source Android distribution. The assistant answers community questions and helps with
recurring group tasks, reachable through the project's chat channels.

## Architecture: shared core, thin adapters

The assistant serves several chat platforms at once — Telegram and Matrix first. All
behavior lives in a platform-neutral core: conversation handling, knowledge lookup, command
semantics, rate and abuse protection. Each platform connects through an adapter that only
translates between the platform's API and the core's message model.

Two invariants hold at every commit:

- An adapter contains no behavior.
- The core contains no platform vocabulary.

Adding a platform means writing one adapter and registering it — never touching the core.
If a change seems to need platform-specific branches in the core, the structure is wrong:
refactor until the change fits naturally, then make it.

## Engineering standards

- Modular structure, robust failure handling, clear separation of concerns, well-chosen
  abstractions. Features that need bolted-on conditionals signal a refactor, not an if.
- Every unit of work runs in a git worktree through the implement-review-verify workflow,
  merges back on completion, and the worktree is deleted.
- Documented decisions carry their date and the rejected alternatives.
- Commit messages follow the repository style: lowercase scope prefix plus plain imperative,
  a body written from zero, and a `Test:` footer stating a past fact.

## Operating notes

- `TODO.md` and `LOCAL.md` are untracked working files: `TODO.md` holds project state per
  its header conventions; `LOCAL.md` holds deployment-specific wiring that stays out of the
  repository.
- This repository is public-ready: no secrets, no personal data, no deployment internals in
  any tracked file or commit.

## Decisions

- Implementation language: Rust, decided 2026-08-19 — see `docs/decisions`.
