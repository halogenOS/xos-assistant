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

## Rejected alternatives

- **Recording edits as fresh messages.** Two blocks claiming to be the person's one
  statement, with no marking.
