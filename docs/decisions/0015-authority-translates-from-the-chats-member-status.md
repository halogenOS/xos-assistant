# 0015 — Authority translates from the chat's member status

Date: 2026-08-21

## Context

Decision 0008 records the sender's authority on the message block, resolved live by the
adapter at receipt. The platform expresses standing as a member status, and the API that
answers it is rate-limited.

## Decision

The platform's `creator` maps to admin, `administrator` to moderator, everything else to
member; a direct chat's sender is a member. The adapter resolves status from a per-chat
administrator list fetched via `getChatAdministrators` and cached with a short
time-to-live — which is what "resolved live at receipt" can mean against a rate-limited
API. A failed list fetch fails that message's ingest transiently; authority is never
silently defaulted into the ledger.

## Rejected alternatives

- **`getChatMember` per message.** One API round-trip per group message, against the
  same authority data.
- **Trusting a status carried on the update.** Updates do not carry it.
