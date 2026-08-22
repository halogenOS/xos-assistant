# 0036 — The turn's authority is stamped at the write, minimum rule

Date: 2026-08-22

## Context

Every message already records its sender's authority (decision 0008). The tool
unit's admission will need the authority of the debt that summoned a turn — a
turn can be summoned by several senders through debt propagation, and the
per-block hook that summons it cannot fold history (decision 0021).

## Decision

This unit adds the debt's authority as a write-time stamp beside answer-due. A
message with no owing tail opens its debt — if addressed and not limited — at
its own authority. Whenever the tail owes, the minimum rule applies regardless
of the incoming message's own addressed fact: the carried debt authority is the
minimum of the tail's debt authority and the incoming sender's authority. The
frontier message's debt authority is therefore the lowest authority that
contributed to summoning the turn, recorded before the turn exists — provenance
stamped at insert, policy read live at admission, in the access-model tradition.
A pre-migration owing tail carries a null debt authority; the fold reads that as
the tail's own stored sender authority, which every pre-migration row carries. A
message carrying no debt stamps null.

## Rejected alternatives

- **Deriving the turn's authority at admission by folding history.** The
  per-block hook cannot fold; admission reads one stamped fact.
- **The maximum rule.** A member's question riding an admin's debt would gain
  admin standing — the escalation the access model forbids.
