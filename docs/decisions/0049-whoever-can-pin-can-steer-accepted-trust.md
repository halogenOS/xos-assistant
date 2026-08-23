# 0049 — Whoever can pin can steer the assistant — accepted trust

Date: 2026-08-23

## Context

A rules note is a system-voiced line written by whoever holds the group's
pin right. Feeding admin-authored text into the model's system voice is a
real surface.

## Decision

Accepted, with its reasoning recorded: the group governing its assistant is
the point of the feature, and pinning is an administrator right in the
target groups. The byte bound on the rules text caps the surface; the trust
boundary is the group's own admin set, and the operator reference states it.

## Rejected alternatives

- **An operator-only rules source.** The operator IS a group admin; a second
  check adds a knob, not safety.

Refined 2026-08-23, at the unit's close. The title path had no bound of its
own — the platform's title cap was silently load-bearing for a system-voice
surface the core claims to own. The core now carries its own byte bound,
`TITLE_TEXT_MAX_BYTES` (512), beside the rules bound: an over-bound observed
title is refused whole with a log line, never truncated.
