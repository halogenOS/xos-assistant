# 0107 — Helpful-mode answers are never threaded

Date: 2026-08-24

## Context

Under helpful answering the assistant evaluates every message in a channel and
answers what it can, including messages nobody addressed to it. Decision 0106 makes
an answer a reply to the one message that addressed the assistant. In helpful mode
that message usually does not exist.

## Decision

An answer to a message nobody addressed the assistant with is delivered plainly. It
follows from decision 0106 by construction — an unaddressed message stores a false
literal-addressed fact, so the rule finds no target — and no mode is consulted
anywhere in the delivery. It is recorded here because it is a product judgment as
much as a mechanical outcome: answering an unaddressed message is a courtesy, and
quote-replying someone who never asked, in front of the group, is not.

Helpful mode does not exempt a channel from threading. A member who does address the
assistant in a helpful-mode channel is answered as a reply, by the same rule.

## Rejected alternatives

- **A mode check in the delivery path.** The delivery would then read configuration
  that the write-time stamp already resolved, in the one place unit 16 made mode-free;
  a second reading of the same decision is a second place for it to be wrong.
- **Threading helpful-mode answers onto the message being answered.** Quote-replying
  a person who never addressed the assistant advertises to the group that they were
  being read, which is not the texture the courtesy is for.
