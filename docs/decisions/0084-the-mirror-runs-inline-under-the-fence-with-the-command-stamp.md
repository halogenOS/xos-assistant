# 0084 — The mirror runs inline under the fence, with the command stamp

Date: 2026-08-23

## Context

The person-wide erasure takes the erasure fence exclusively and must be
spawned outside ingestion (decision 0073); the mirror is an erasure too,
but of one row, requested by the very message being ingested. And a
deleted message may be the conversation's owing tail: what happens to its
unanswered debt has to be decided, not left to ordering accident.

## Decision

The one-row erasure runs inline within ingestion's existing write path,
under the erasure fence's read hold that ingestion already carries — one
row's nulls, not the person-wide operation, so no spawn is needed and no
deadlock shape exists. It runs after the suppression drop and the channel
admissions — an opted-out administrator's command is dropped whole, and a
refused channel never reaches it — and BEFORE the tail read, so the
command row's stamp is decided against the post-mirror world: a debt the
deleted message itself owed dies with its text, through the same shared
owes-answer reading that already cancels an erased row's debt, while a
debt any other row carries still propagates through the command row.
Conversation liveness is untouched either way — later traffic stamps and
answers exactly as before. The command stamp keeps `/del` out of the
answer machinery like every command: no debt of its own, no budget count,
no unlatch, no reply window.

## Rejected alternatives

- **Spawning the one-row erasure like the person-wide one.** The spawn
  shape exists for the exclusive fence hold; one row under the read hold
  needs none of it, and a spawn would let the command return before its
  erasure ran.
- **Erasing after the append.** The stamp would then read the pre-mirror
  tail and propagate a deleted message's debt into the command row — the
  assistant would answer the ghost of a message an administrator just
  removed.
- **Recording the mirror's `/del` as an ordinary message.** An addressed
  one would take a debt and summon a turn over a command meant for
  another bot.
