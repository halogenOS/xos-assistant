# 0023 — The system prompt lives in its own files, pinned per conversation

Date: 2026-08-22

## Context

The assistant's voice needs a durable home, and the framework records a
conversation's system prompt through its own system-prompt kind.

## Decision

The prompt is prose in prompt files in the repository, under the prompts directory;
the binary loads them at start (every file, joined in file-name order) and the
assembly records the result through the framework's system-prompt kind at each
conversation's creation — the mapping winner only. The framework records a
conversation's prompt exactly once, so an edited prompt reaches new conversations
only.

OPEN, surfaced to the framework's improvements list: a long-lived group conversation
never receives a prompt update; the superseding-prompt block is framework work.

## Rejected alternatives

- **A constant in code.** Prose in code, and a prompt edit becomes a code change.
- **Deployment-supplied prompt files.** The assistant's voice belongs to its public
  repository.
