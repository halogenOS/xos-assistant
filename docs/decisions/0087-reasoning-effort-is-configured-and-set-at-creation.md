# 0087 — Reasoning effort is configured and set at creation

Date: 2026-08-23

## Context

Nothing set a conversation's reasoning level: creation left the store's
column null, the framework read null as "defer to the provider", and the
provider request went out without a `reasoning_effort` — the model thought
unboundedly on every turn, part of the answering latency, with the
moderation assessments riding on that unbounded thinking. The framework's
path for the level already existed end to end — the stored key is parsed at
request build, carried on the provider request, and translated onto the
chat wire's `reasoning_effort` field — so the one missing link was the
consumer never writing the key.

## Decision

The configuration file gains an optional `reasoning` key, defaulting to
`low`, decoded as a closed word list spelling exactly the framework's
level keys — decoding is the validation, mirroring the direct-chat key, and
tests hold the two vocabularies equal. The assembly carries the resolved
level and sets it on the winning conversation right after the mapping
claim, so every new conversation — direct and group alike — stores the
configured key and every provider request carries it. Conversations created
before the key shipped keep their deferring null: no backfill, because a
redeploy's fresh conversations get the level and no production store
predates it.

The provider traffic stays on the chat-completions endpoint.

## Rejected alternatives

- **Switching to the responses endpoint for the reasoning control.** The
  framework's responses path sends the reasoning parameter for its own
  vendor's model slugs only, so the deployment's model would lose the level
  through it — the chat wire's `reasoning_effort` is the path that carries
  it for every model.
- **Backfilling existing conversations' null levels.** A stored null
  already reads as the provider's default, the store is append-only in
  spirit and young in fact, and a redeploy's fresh conversations pick the
  level up at creation.
- **A framework change setting the level inside `create_conversation`.**
  The default is deployment policy, not machinery; the store's existing
  mutator is the framework's offered surface, and the consumer owns the
  choice.
