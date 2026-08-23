# 0062 — The moderation handle and the report line

Date: 2026-08-23

## Context

The report line is a platform command aimed at one bot: `/report@` plus the
moderation bot's handle. Somebody has to name that handle, and the platform
side has switches without which a bot's report goes nowhere.

## Decision

The handle is an optional configuration key — trimmed, a leading `@`
stripped, refused empty after trimming; absent means the report tool does
not register, and the palette-delta mechanism (decision 0061) removes it
from conversations that had it. One global handle: one deployment serves
one community; per-group handles are rejected until a second community
exists. The line's wording is a named core constant. The wiki base address
is trimmed and refused empty the same way.

The platform-side setup is operational and recorded in the operator
reference document: bot-to-bot communication enabled for the assistant, the
moderation bot's bot-to-bot setting opened to all bots, and the assistant
NOT a group administrator — the moderation bot ignores administrators'
reports, so an administrator assistant files into silence. Whether the
moderation bot honors a bot's report at all is undocumented; the first live
filing settles it, and the reference document says so plainly.

## Rejected alternatives

- **Per-group handles.** Configuration surface for a second community that
  does not exist.
- **The handle as a core constant.** The moderation bot is deployment
  wiring, not product truth; a public repository names no deployment
  internals.
