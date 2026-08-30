# 0153 — A bot sender is never summoned by mode, only by address

Date: 2026-08-30, with unit 42.

## Context

The summons resolution is the one place the answering mode enters the machinery: a message
summons the assistant when it addressed it, or when helpful answering evaluates every
message. Helpful answering is the mode the live group runs in, so every message from every
member — including every automated one — summoned a turn. The teaching text forbade
welcoming a joiner, and the model welcomed one anyway: teaching is probabilistic where a
mechanism is not.

## Decision

In the summons resolution, a message whose sender is a bot is summoned if and only if it is
addressed. The helpful-mode clause applies to non-bot senders alone. Everything past the
resolution stays exactly as built and reads the resolved summons as before.

Recorded history is untouched. A bot's messages land in the ledger and reach the model's
context precisely as today — the deletion mirror and the group's visible memory both depend
on that — they simply open no turn and no debt of their own. Direct channels are unaffected
in practice and the rule still reads coherently there: a direct message is addressed by
definition, so a bot in a direct channel is summoned.

The fence is MODEL turns alone. A recognized programmatic command is handled
deterministically — no model, no turn, no request — so a bot invoking one is not the model
being triggered, and the operator confirmed that reading when a stricter one was offered.
Command recognition, the fixed answers and the deletion mirror stay sender-blind: a bot's
privacy command answers exactly as a member's does, under the same per-channel window that
already bounds any flood of it.

The teaching text is untouched, on the operator's explicit shape for this unit. The helpful
arm still tells the assistant that every message reaches it and it decides whether to
speak, which grows slightly loose for bot senders, whose messages reach the context but
never bring the model in. Recorded here as a later teaching unit's candidate, never this
one's.

## Rejected alternatives

- **Filtering bot messages out of ingestion.** The model must keep seeing them: the
  deletion mirror rides a moderation bot's own command, and a group's memory with the
  automated half deleted is a memory of half a room. They must trigger nothing, not vanish.
- **Teaching the model harder.** Already tried, in the exact sentence the incident
  violated. A mechanism decides this, or nothing does.
- **Refusing programmatic commands from bots too.** It answers a question nobody asked —
  the incident was a model turn — and it would deny an automated account the rights
  commands the privacy documents promise every sender.
