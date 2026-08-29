# 0017 — Text is what this unit records

Date: 2026-08-21

## Context

Updates carry more than plain text: media with captions, media without, edits, service
messages, non-message update kinds. The message kind the core records holds text.

## Decision

An update's message text, or its caption when the message is media with a caption,
becomes the inbound text. Updates with neither, and non-message updates, are skipped.
Edited messages are skipped too: the recorded ledger keeps the message as first seen,
and an edit kind — appending the revision as its own block — is a later unit's decision,
taken when the acting policy exists to read it.

## Amended 2026-08-29 — a join is not a message, and it is recorded

Unit 36 records one platform fact this decision skipped: the service message
announcing that people joined a group. It does not widen what a MESSAGE is —
the message kind still records text, and every other service shape, a
departure and a chat's creation included, is still skipped, now under a
named skip of its own instead of the generic no-text one. A join lands as a
block of its own kind through the observation surface, carrying the name the
platform showed, because a joining account's displayed name can be the whole
of an advertisement and the assistant cannot assess what it never recorded.

## Rejected alternatives

- **Recording edits as fresh messages.** Two blocks claiming to be the person's one
  statement, with no marking.
- **Letting a join in through the message kind (2026-08-29).** A join carries no
  text, no sender and no ask; widening the message invariants to admit one
  would carve out every one of them.
