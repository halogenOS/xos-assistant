# 0011 — Erasure covers direct channels

Date: 2026-08-21

## Context

Decision 0003 placed personal data — the identifying details and the message text —
in tables apart from the ledger, and had erasure delete rows in those tables while
every block keeps its place. The core spine delivers the identity-row side of that:
blocks store the principal id only, and erasure removes the identity rows it points
at. Making an erased person's stored message text report itself as erased, as 0003
calls for, was first left open here; decision 0012 settled it on 2026-08-21 — the
text column is nullable, erasure nulls it, and a direct conversation of the erased
principal is removed entirely.

The channel-to-conversation mapping adds a second place a person can be identified: a
group channel's key names the group, but a direct channel's key names the person and
is personal data.

## Decision

The mapping records the channel kind at creation. Erasing a principal removes, besides
the identity rows, the direct-channel mappings whose conversations contain that
principal's messages — found by reading the ledger, never by writing it — in one call.

## Rejected alternatives

- **Erasure of identity rows alone.** Leaves a personal identifier sitting in the
  mapping table, which defeats the erasure.
- **The caller supplying channel keys to unmap.** Pushes a data-protection obligation
  onto every caller and turns the one operation into two that must agree.
