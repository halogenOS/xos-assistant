# 0016 — Messages on behalf of a chat are skipped

Date: 2026-08-21

## Context

A group message can carry `sender_chat`: an anonymous administrator posting as the
group, or a linked channel's auto-forward. Its nominal sender is not a person — the
platform substitutes a stand-in and deliberately withholds the real author.

## Decision

Such messages are skipped, as a named case beside the channel-broadcast skip. Recording
one would mint a shared principal aggregating several real people — wrong identity,
wrong erasure scope — at member authority, which is the wrong standing too.

## Rejected alternatives

- **Recording the stand-in sender as-is.** A principal that is not a person corrupts
  both the identity model and erasure.
- **Resolving the real author.** The platform deliberately withholds it.
