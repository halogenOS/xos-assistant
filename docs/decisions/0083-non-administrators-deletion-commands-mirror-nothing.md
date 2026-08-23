# 0083 — Non-administrators' deletion commands mirror nothing

Date: 2026-08-23

## Context

The deletion token is public vocabulary: any member can type it, with or
without a reply, at any target. The mirror must not turn a member's typing
into an erasure, and must stay quiet in every case the moderation bot
itself would ignore.

## Decision

A member's `/del` is recorded as an ordinary message and mirrors nothing —
the moderation bot ignores them too. A `/del` without a reply target
mirrors nothing, and a reply to the assistant's own message carries no
target origin and mirrors nothing the same way; both record as ordinary
messages. An administrator's `/del` whose target the store never held
mirrors nothing, and a target already erased mirrors nothing, idempotently
— these two are still the recognized command and record with the command
stamp, because the trigger reads the message alone and never the store.
All of it silent: nothing nulled beyond the standing state, nothing sent.

## Rejected alternatives

- **Answering the no-ops with an error line.** The silence decision of
  0082 covers every case: nobody asked the assistant anything.
- **Treating a member's `/del` as a report ask.** The report path is the
  member's own tool with its own consent shape; inventing intent from a
  token aimed at another bot would act on a guess.
- **Recognizing the command only when the store holds the target.** The
  stamp would then depend on store state, and two identical requests would
  record as different kinds.
