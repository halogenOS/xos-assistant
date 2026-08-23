# Unit 13 — the deletion mirror

Date: 2026-08-23. Revision 1. The operator's settled design: when a group
administrator replies to a message with the moderation bot's own deletion
command, both bots receive that command independently — the moderation bot
deletes the message in the chat, and the assistant erases its stored copy,
silently. One command, both stores clean, and the assistant stays a non-admin
bystander: the admin asked the moderation bot; the assistant's part is
bookkeeping.

## Decisions taken with this unit

- **The mirror rides the moderation bot's command, 2026-08-23.** A group
  message whose reported invoked command is the deletion token (`/del`, the
  moderation bot's own, a named constant) and whose reply target names a
  stored message triggers the mirror when the SENDER is a group
  administrator — the authority the adapter already resolves per message.
  The command message itself is recorded like any command (the request is
  the lawful record), the reply target resolves through the stored origin
  exactly as the report tool's target does, and the named message's row is
  erased: text, origin, send time, reply reference and speaker nulled — the
  same nulls the person's own erasure applies, scoped to one row. The
  placeholder stays, projection reads the erased marker, nothing else moves.
  SILENT: no reply, no acknowledgment — the admin addressed the moderation
  bot, and a second bot answering a command meant for the first is noise.
  Rejected: a reply from the assistant (noise, and the admin asked Rose);
  admin-gating through the tool authority walk (this is a deterministic
  command, not a model tool — the sender's resolved authority is the gate,
  and the 0043 interrupt blocker stays untripped); acting on the model's
  judgment anywhere (decision 0070 — the admin IS the human decision).
- **Non-admin senders are ignored, 2026-08-23.** A member's `/del` is
  recorded as an ordinary message and mirrors nothing — the moderation bot
  ignores them too. A `/del` without a reply target mirrors nothing. A
  target already erased mirrors nothing, idempotently. An admin's `/del`
  whose target the store never held mirrors nothing. All silent.
- **The erasure fence and the stamp hold, 2026-08-23.** The one-row erasure
  runs under the erasure fence's read guard within ingestion's existing
  write path — one row's nulls, not the person-wide operation, so no spawn
  is needed and no deadlock shape exists. The command stamp keeps `/del`
  out of the answer machinery like every command.
- **The documents move, 2026-08-23.** The policy's deletion section gains
  one sentence: a message deleted by the group's administrators is removed
  from the assistant's store as well. The DPIA's moderation paragraph notes
  the mirror as reactive bookkeeping of an admin's own act. The operator
  reference explains the piggyback and its constraint: the assistant must
  see the command, so the moderation bot's deletions must arrive as a
  REPLY `/del` (the moderation bot's other forms — bulk purges, direct
  deletions — do not reach the assistant and leave the store copy, named
  plainly as the bound; the person-wide erasure commands remain for those).

## The unit's contract

The deletion token constant; the mirror check in ingestion's command
handling (admin sender, reply target, group channel); the one-row erasure
beside the existing passes; the silence; the document updates. No adapter
change, no new table, no configuration.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** An admin's reply `/del` nulls the target row (text, origin, send
  time, reply reference, speaker) AND, when the target was present, scrubs
  the reply references naming its origin on every other row in the
  conversation (decision 0085, so erasure stays reachable) while the `/del`
  command row keeps its own reference; the placeholder projects erased, no
  reply is sent, the command row is recorded — pinned block by block and
  over the wire.
- **AC3** The silent no-ops pinned: non-admin sender, no reply target,
  unknown target, already-erased target — the reply-reference pass is
  withheld when the target was not freshly present, so nothing is nulled
  beyond the standing state and nothing is sent.
- **AC4** Interactions pinned: the mirror inside a turn's absorption window
  does not disturb the turn; a mirrored message that carried an unanswered
  debt leaves the conversation liveness intact (the walk's read-through
  covers the erased row); the suppression and DM gates precede the mirror.
- **AC5** The three document updates ship — pinned in the docs test.
