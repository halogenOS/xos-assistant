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

## Amended 2026-08-31 — the assistant deletes its own messages, and nobody else's

The message-retraction unit ships one deletion capability, so the proof
sentence above is no longer true as written and the framing narrows with it.

What stands: the assistant deletes NO member's message, ever, and no message
leaves a chat on the assistant's own reading. What changes: on a group
administrator's reply deletion command the assistant takes back a message it
sent ITSELF, through the platform's plural deletion method, named by the wire
client and nowhere else.

Decision 0070 is untouched by that. The administrator's command is the human
decision the mechanism requires, the act touches no person's words, and the
bound lives in the code and not in a granted right: the identifiers the core
may name for deletion come only from its own recorded deliveries, so the
assistant cannot reach a member's message even where the platform would allow
it. The bot is still never made an administrator and is never granted the
delete-any-message right.

The self-edit half is unchanged and unaffected: no edit method is called, and
a delivered answer is never rewritten under readers who already read it.

## Rejected alternatives

- **Editing without recording.** The ledger would carry an answer nobody can
  read in the chat any more.
- **Streaming the answer into a message edited as it grows.** The group rate
  limits, the moving chunk boundaries, and an answer that reads as finished
  several times before it is — and the platform's own streaming methods are
  private-chat-only and thirty seconds long.
