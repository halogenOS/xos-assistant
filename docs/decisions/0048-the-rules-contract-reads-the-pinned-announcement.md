# 0048 — The rules contract reads the pinned announcement

Date: 2026-08-23

## Context

Both target platforms share the concept of pinning a message; the platform
delivers a pin as an event, and its lookup exposes exactly one pinned
message, chosen by the pinned messages' sending dates — not by pin recency.
The group needs a way to state its rules that the assistant can read from
the group itself.

## Decision

The adapter reports two platform-neutral observations — the channel's title
and the text of a pinned announcement (the one the lookup exposes, or the
one a pin event names) — and the core owns the contract: a pinned text whose
first line is exactly the rules prefix `Rules:` followed by a newline —
case-sensitive, a carriage return before the newline tolerated, nothing
before the prefix — is the group's rules; the prefix line is stripped and
the remainder becomes the rules note. A remainder that is empty after
trimming is refused with a log line. A pinned text without the prefix is not
rules and supersedes nothing. A pin whose content the platform withholds
(the inaccessible form) yields no observation. The rules text is bounded by
a named byte constant; an over-bound text is refused whole with a log line,
never truncated — a cut rule is a different rule.

Two operational facts are recorded plainly in the operator reference: the
lookup selects by sending date, so an old rules pin can sit invisibly behind
a newer announcement — the operator posts a fresh rules message and pins it,
and the acknowledgment confirms the pickup — and rules removal has no event
on the platform, so a stale note stands until the next rules pin: replace
rules, never merely unpin them.

## Rejected alternatives

- **Fetching pin history.** The platform exposes no enumeration.
- **Treating every pin as rules.** Groups pin announcements too.
- **Adapter-side prefix parsing.** The contract is product behavior; the
  adapter only translates.
- **Truncating over-bound rules.** Meaning-changing.
