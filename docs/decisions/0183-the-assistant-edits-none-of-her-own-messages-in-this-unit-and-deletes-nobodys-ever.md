# 0183 — the assistant edits none of her own messages in this unit, and deletes nobody's ever

Date: 2026-08-31, with the editing unit.

## Context

Two different things share one name. A member edits a message; the assistant
edits its own. The platform allows the second without the time limit
everyone assumes — the 48-hour sentence in its documentation binds business
messages the bot did not send.

## Decision

Two separate statements, for two separate reasons.

The self-edit half is a scope statement, not a capability judgement. What
this unit does refuse is re-editing a delivered answer when its question is
edited: the answer would silently change under readers who already read it,
and the stored answer block would stop being what the channel saw, which is
the equality decision 0079 exists to keep. Any future capability appends its
superseding record before the edit call, on that same rule.

The deletion half is a settled refusal. The platform grants an administrator
bot the power to delete any message in the group, and helpful-mode
deployments run the bot as an administrator, so the capability is live and
unused on purpose. Decision 0070 places a human at every moderation effect:
the assistant assesses and administrators act. A message the assistant
removed from the chat on its own reading would be a moderation effect with
no human in it, and no unit may add one. The deletion mirror is not a
counter-example — there the administrator's own command is the human
decision, and the assistant only erases its stored copy.

The proof is that no request builder in the adapter names a message-editing,
message-deleting or message-draft method at all.

## Rejected alternatives

- **Editing without recording.** The ledger would carry an answer nobody can
  read in the chat any more.
- **Streaming the answer into a message edited as it grows.** The group rate
  limits, the moving chunk boundaries, and an answer that reads as finished
  several times before it is — and the platform's own streaming methods are
  private-chat-only and thirty seconds long.
