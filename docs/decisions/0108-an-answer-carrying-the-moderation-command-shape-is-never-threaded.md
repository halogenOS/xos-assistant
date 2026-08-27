# 0108 — An answer carrying the moderation command shape is never threaded

Date: 2026-08-24

## Context

The moderation bot files a report when it sees a REPLY carrying the report command
shape. Decision 0046 accepted one residual when the report tool shipped: a disobedient
model could type that command into an answer anyway. The reasoning was that the
assistant's answers are unthreaded, so stray prose is noise and files nothing, and
outbound prose sanitation was rejected as the alternative.

Decision 0106 makes answers threaded. A threaded answer whose prose carries the
command shape — a member asking what the command does, a model slip, an injected line
— would be a real filed report against whatever message it threaded onto, bypassing
every check the report tool performs: the target validation against the turn's
assessed messages, the per-origin deduplication, the self-report refusal.

## Decision

An answer whose text carries the report command lead is delivered as a plain message.
The hazard exists only when the answer is a reply, so the reply is what is dropped.
This preserves decision 0046's reasoning instead of overturning it: stray prose stays
noise.

It is not the prose sanitation decision 0046 rejected. Nothing is rewritten, stripped,
refused or withheld — the text goes out exactly as the model wrote it, byte for byte,
and only the routing changes. The reading is one containment check against the same
constant the report line is built from, so the check and the shape it protects against
can never drift apart.

## Rejected alternatives

- **Sanitizing the answer's prose.** Rejected once already by decision 0046, and for
  the same reason: censoring the assistant's own speech on a pattern is the bolted-on
  conditional the structure rule forbids, and it makes the assistant unable to talk
  about moderation at all.
- **Withholding such an answer.** A person asked a question; the answer is theirs.
  Silence to protect a mechanism is a worse trade than a plain message.
- **Trusting the report tool's own checks to catch it.** They never run: the
  moderation bot reads the chat, not this process, and a filed report from a reply
  reaches it without any of them.

## Amended 2026-08-27: the guard covers every reply-acted command shape

The decision above named one shape, the report lead, because the report was the
hazard the unit analysed. The core records a second command acted on from a reply:
the moderation bot's deletion command (`mirror.rs`), which an administrator invokes
by replying with it. A threaded answer repeating that shape ends the same way one
repeating the report lead does — a real command against the message it threaded onto
— only in a deletion instead of a report, and only while the assistant itself holds
administrator standing, which no repository should have to assume about a deployment.

The guard now reads the one list in `reply_commands.rs`, which names every
reply-acted shape; each shape's definition stays with the module that owns its
behaviour. A future reply-acted command is added to that list or the guard goes
blind to it — the list is the record of this decision's scope, so the decision and
the code cannot drift apart. Everything else above stands: routing only, never
sanitation.
