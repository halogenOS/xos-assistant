# 0180 — the deletion mirror ignores revisions

Date: 2026-08-31, with the editing unit.

## Context

The mirror's whole premise is that both bots receive the same command
independently: the moderation bot deletes the message in the chat, and the
assistant erases its stored copy (decision 0082).

## Decision

A message that revises another one mirrors nothing. Nothing establishes that
the moderation bot acts on edited commands, and an assistant that erased its
stored copy of a message still visible to everyone would produce precisely
the divergence the mirror exists to prevent.

The privacy self-service commands are the opposite case and stay reachable
through an edit: only the author can edit their own message, so an edited
deletion request is that person's own ask about their own data — with the
honest limit that under platform privacy mode a command first appearing in
an edit may never arrive at all.

## Rejected alternatives

- **Mirroring anyway.** An invisible one-sided deletion.
- **Refusing every command that arrives through an edit.** It would silently
  swallow a person's own rights request.
