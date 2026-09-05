# halogenOS Group Assistant

A community assistant for [halogenOS](https://halogenos.org), the performance-focused,
open-source Android distribution. The assistant answers community questions and helps with
recurring group tasks, reachable through the project's chat channels.

## Architecture: shared core, thin adapters

The assistant serves several chat platforms at once — Telegram and Matrix first. All
behavior lives in a platform-neutral core: conversation handling, knowledge lookup, command
semantics, rate and abuse protection. Each platform connects through an adapter that only
translates between the platform's API and the core's message model.

Three invariants hold at every commit:

- An adapter contains no behavior.
- The core contains no platform vocabulary.
- The assistant assesses, a human decides: no moderation effect without a human
  decision point in the mechanism (decision 0070).

Adding a platform means writing one adapter and registering it — never touching the core.
If a change seems to need platform-specific branches in the core, the structure is wrong:
refactor until the change fits naturally, then make it.

## Engineering standards

- Modular structure, robust failure handling, clear separation of concerns, well-chosen
  abstractions. Features that need bolted-on conditionals signal a refactor, not an if.
- Every unit of work runs in a git worktree through the implement-review-verify workflow,
  merges back on completion, and the worktree is deleted.
- Documented decisions carry their date and the rejected alternatives.
- A unit's acceptance criteria run the feature through the existing lifecycles, not only its
  own path: a restart with a changed prompt, a compaction, an erasure, a take-back, a
  retirement. A mechanism proven alone and never in sequence with the others is the shape
  that reached production on 2026-09-02 (the prompt-last fork that broke every compaction).
- A shape the code must never produce is refused by the store, never merely avoided by the
  callers: the rule lives where the rows are written, so the wrong shape fails loudly the
  first time anyone builds it.
- Commit messages follow the repository style: lowercase scope prefix plus plain imperative,
  a body written from zero, and a `Test:` footer stating a past fact.

## Operating notes

- `TODO.md` and `LOCAL.md` are untracked working files: `TODO.md` holds project state per
  its header conventions; `LOCAL.md` holds deployment-specific wiring that stays out of the
  repository.
- This repository is public-ready: no secrets, no personal data, no deployment internals in
  any tracked file or commit.

## Decisions

Each one is recorded in `docs/decisions` with its date and the alternatives it beat.

- Implementation language: Rust, decided 2026-08-19.
- Storage and orchestration: the block ledger architecture adopted from ronna-lightspeed —
  blocks are the only content unit, storage is append-only, conversation state is derived
  from the blocks, and behavior lives on the block kind instead of in the machinery.
  Decided 2026-08-20.
- Personal data lives in tables of its own, separate from the ledger, so erasure never
  breaks the append-only rule. Message history is kept without a retention timer. Decided
  2026-08-20.
- License: GNU General Public License v3.0, following the adopted code. Decided
  2026-08-20.
