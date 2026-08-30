# 0162 — A wipe drops the mapping and claims a brand-new conversation

Date: 2026-08-30, with unit 45.

## Context

"A brand new session" has to mean something exact in an append-only store. Two ways of
producing a new conversation id already existed: the first-contact path, which creates a
conversation with the current prompt and palette and claims the channel for it, and the
retire walk, which forks with full inherited history. The wipe needs the first, and needs it
to be the same code rather than a copy of it.

## Decision

`/wipe` deletes the group's mapping row and runs the first-contact path for the same
channel: a fresh conversation, the current composed prompt, the current tool palette, the
mapping claim with its winner check. The group gets exactly what a newly admitted group
gets, with nothing inherited. The next member message speaks into an empty session.

The winner check is read, not discarded: a claim lost to a concurrent racer leaves this wipe
having made nothing, so it answers SILENCE and fires no reset directive (decision 0165 states
the same for both commands), and the fresh conversation it created is deleted before anything
referenced it.

The old conversation is not touched. It stays whole, readable, exportable and reachable by
erasure — the retire walk's own recorded promise, and the reason a wipe is not a retention
change: nothing is deleted, and the one deletion of a conversation outside erasure remains
what it always was, a just-created conversation that lost its mapping claim before anything
referenced it.

The channel's standing observations — its title, its pinned rules — come back because the
outcome of the command carries a CHANNEL-RESET directive beside its answer. The core
decides; the adapter translates by forgetting whatever it looked up for the old session, so
the channel's next contact runs its first-contact lookup again and enriches the fresh
conversation. That is the withdraw directive's exact shape, and it adds no decision to the
adapter.

Two costs are stated instead of hidden.

Every debt the old conversation still owed is cut with it. The reset was asked for, and the
old conversation still shows any unanswered message to a human reader.

An answer or an outbound item in flight at the moment of the swap resolves its channel from
the mapping when it is delivered, finds none, and is dropped. That drop was silent before
this unit and is now logged at warn on both of the edge's unmapped branches. It is accepted
openly: a session being reset owes its unsent products to the record, not to the chat. The
runtime may still spend one turn on a debt of the retired conversation, since nothing marks
a source retired to the scheduler; its answer lands in the dead conversation and drops at the
same logged branch — the retire machinery's own recorded leftover, unchanged.

## Rejected alternatives

- **The framework's new-thread continuation.** It deep-copies the trailing user group into
  the fresh thread, so the message that asked for the wipe would be carried into the session
  that is supposed to be empty. That is not a brand new session.
- **Deleting the old conversation.** A retention change the privacy record forbids without
  its own revision, for no gain: the channel already moves on from it.
- **Letting the adapter notice the new conversation by itself.** Any rule it could use would
  be the adapter deciding what a reset means, and behaviour in an adapter is the invariant
  this repository does not bend.
