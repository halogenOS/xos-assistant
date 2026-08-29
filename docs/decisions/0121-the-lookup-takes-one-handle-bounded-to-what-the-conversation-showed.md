# 0121 — The lookup takes one handle, bounded to what the conversation showed

Date: 2026-08-25, with unit 29; widened for join notices 2026-08-29.

## Context

The tool needs a subject. The privacy tool takes none and acts on whoever spoke,
because it WRITES and acting on a guessed person is unforgivable there. This tool
writes nothing and has to answer "is @someone an administrator", which the turn's
own origin set cannot express at all.

## Decision

The tool is registered as `member_standing` and takes one parameter, `handle`. The
handle must be one this conversation SHOWED: a message's stored speaker, or a stored
joiner since unit 36. Nothing else is a source, and message TEXT above all is not:
read from text, a member typing another member's handle would make it askable, and
the tool becomes a queryable directory of who holds power over whom.

A handle shown only by a join has no message and therefore no stored standing. It
answers its own refusal — joined, has not spoken, no standing on record — because
refusing it as never shown would read as false beside the join line the model was
just shown.

## Rejected alternatives

- **Any handle at all.** The directory above, built by accident.
- **No parameter, resolving the subject from the turn's origins.** Copies a
  constraint from a tool that writes, cannot answer the question the unit exists for,
  and makes the answer depend on turn assembly instead of on what was asked.
- **Answering a join-only handle as a member.** A standing nobody recorded, stated
  as a fact.
