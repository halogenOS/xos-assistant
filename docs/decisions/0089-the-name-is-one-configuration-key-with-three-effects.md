# 0089 — The name is one configuration key with three effects

Date: 2026-08-23

## Context

The assistant's name was baked into the disclosure copy (decision 0079)
and nowhere else; the operator wants it configurable, and wants the
assistant to wake to its name in a group beside the mention and the reply.

## Decision

A `name` key names the assistant. Absent, the name defaults to the display
name the process reads from the platform at startup — one identity call,
performed only when the key is unset; a failed read refuses the start
loudly, naming both remedies (retry, or configure the key), because a
nameless assembly would compose a broken identity and disclosure silently.
An explicit value overrides and skips the platform read. Empty values are
refused at the load; a surviving name resolves trimmed.

The resolved name has three effects:

- **The prompt identity.** The assembly composes an identity section into
  every new conversation's system prompt: the model knows what it is
  called and answers the are-you-a-bot question about that name honestly,
  extending decision 0080's teaching.
- **The disclosure fill.** An unset `disclosure` key composes the
  first-interaction line from the name (decision 0090).
- **The wake trigger.** A group message naming the assistant addresses it,
  beside the mention and the reply. The match is whole-word and
  case-insensitive, translated in the adapter beside the mention check —
  the adapter owns addressing translation, and the trigger is one more
  input the embedder hands it, not behavior of its own. A name whose
  characters cannot bound one clean trigger word — spaces, punctuation —
  falls back to mention-and-reply, logged once at start. Under helpful
  answering the trigger is moot for summoning (every message is
  evaluated), but the addressing fact it resolves stays truthful either
  way.

## Rejected alternatives

- **Hardcoding the name.** The whole point is configurability; the old
  copy survives as a configured value.
- **The display name as the trigger without an override.** Display names
  punctuate badly; the override plus the clean-word fallback keeps the
  trigger sound.
- **Matching the name in the core.** The spec places addressing
  translation in the adapter, where the mention and reply checks already
  live; a second addressing site in the core would split the one rule.
- **Retrying the startup identity read forever.** A start that hangs
  silently is worse than one that refuses naming the remedy; the service
  manager restarts a refused start.
