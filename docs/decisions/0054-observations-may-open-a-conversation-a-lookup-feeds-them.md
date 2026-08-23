# 0054 — Observations may open a conversation; a lookup feeds them; authorization precedes both

Date: 2026-08-23

## Context

A group's title and rules should exist on the ledger before anyone speaks,
and the adapter must learn them from the platform without a call per
message.

## Decision

An observation for an authorized, unmapped group channel runs the same
winner-only creation path a message does — system prompt and palette
included. The observation path holds the same two locks ingestion holds:
the erasure fence (an observation must not create a mapping mid-erasure)
and the stamp lock (the on-delta read-then-append must be serialized, or
two equal observations both append). The adapter observes lazily: once per
channel per process, on first contact with an authorized group — the
assistant being added, or the first message seen — it looks the channel up
(title and the exposed pinned announcement) and reports what it finds; it
reports every pin event it sees thereafter. A membership observation from
an add is reported before the add's lookup observations, so authorization
is judged first. A failed lookup is best-effort: logged, retried on the
next first-contact (the once-per-process memory is not set on failure),
never halting the update batch — group facts are enrichment, not
authority, unlike the admin fetch.

## Rejected alternatives

- **A core-to-adapter query surface.** A new boundary for what a push
  solves.
- **Observing on every message.** A platform call per message.
- **Suppressing the lookup in the adapter for unauthorized channels.** An
  adapter decision; the core's refusal answers it, and a wasted lookup
  against a stranger group costs one call.

Refined 2026-08-23, at the unit's close. Four shipped mechanisms sharpen the
stated rules. A failed first-contact lookup rests per chat for one minute
(`LOOKUP_RETRY_REST`) instead of retrying on every message — the stated
retry-on-next-first-contact amplified a permanent failure into a platform
call per message. The withdraw rests the same way (`WITHDRAW_RETRY_REST`): a
refused chat's flood draws one leave per window, and while the rest stands
the adapter delivers authority unresolved without the administrator fetch —
the core refuses that chat before reading authority. A lookup that answered
is remembered even when the core withdrew, voided by a later admission; the
answered memory is capped (`ANSWERED_MEMORY_CAP`), so "once per process" is
precisely "once per cap epoch". A pin event's first-contact lookup reports
the title only: the event carries the authoritative text, and the lookup's
by-sending-date pin would otherwise land stale rules first. And authority
resolution is deferred to the core's need — the adapter delivers a group
message with authority unresolved when the fetch fails, the core refuses an
unadmitted group before reading authority, and an admitted message without
it draws the typed transient refusal the driver halts on; nothing is ever
recorded with a defaulted authority. Configuration grew one refusal beyond
the spec's letter, recorded here: an operators key naming an adapter the
binary does not assemble refuses the start — a misspelled key would
otherwise silently refuse every add.
