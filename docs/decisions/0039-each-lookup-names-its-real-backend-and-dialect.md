# 0039 — Each lookup names its real backend and dialect

Date: 2026-08-22

## Context

The project runs two forges with different API dialects: the canonical
self-hosted one, and a mirror that is nonetheless the only home of releases —
the data the project site itself reads.

## Decision

Commit lookup speaks the Forgejo v1 API of the canonical self-hosted forge,
unauthenticated — it answers so today, and commits are public data; its
parameters are the repository name within the project organization and a commit
hash or reference. Release lookup speaks the GitHub v3 API against the project's
builds repository on the mirror organization; its parameter is an optional tag,
defaulting to the latest release. Two base URLs live in configuration with the
real hosts as defaults; one HTTP client, one decoder per dialect. An optional
mirror API token — a secret, referenced indirectly like the others — raises the
mirror's rate limit from sixty to five thousand requests per hour; the canonical
forge needs none.

## Rejected alternatives

- **One forge for both tools.** Releases exist only on the mirror; the canonical
  forge is the truth for code. Either single choice answers one lookup with the
  wrong backend.
- **Shelling out to clones.** The APIs answer; a clone is state to manage and a
  process boundary to secure, for no better data.
