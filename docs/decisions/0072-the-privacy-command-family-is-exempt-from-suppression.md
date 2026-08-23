# 0072 — The privacy command family is exempt from suppression, and identity stays frozen

Date: 2026-08-23

## Context

Decision 0071 drops an opted-out person's messages at ingestion. Dropped
without exception, the person's own `/unblockprivacy` would be dropped too
— a one-way door — and their `/privacy` and deletion commands would go
unanswered while the policy promises them.

## Decision

The deterministic privacy command family — `/privacy`, `/privacyout`,
`/privacydelete`, `/confirmdelete`, `/unblockprivacy` — is exempt from
suppression, and the exemption covers exactly that family. An opted-out
person's `/unblockprivacy` works, so the door reopens from inside; their
`/privacy` keeps answering.

An exempted command message is recorded — the request itself is the lawful
processing of honoring it — with the command stamp, but through the
READ-ONLY identity path: the display fields are not refreshed. The freeze
the stub promises holds even across the person's own commands, and after a
deletion no command re-materializes the emptied fields.

## Rejected alternatives

- **Full suppression.** A one-way door: the person could never opt back in
  or reach deletion through the chat again.
- **Refreshing identity on exempt commands.** The freeze would leak — every
  command would re-collect the display fields the stub exists to stop
  collecting, and a deletion's emptied stub would silently refill.
