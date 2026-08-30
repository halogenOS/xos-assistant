# 0161 — The session resets are a group's moderator commands, and nothing else's

Date: 2026-08-30, with unit 45.

## Context

`/wipe` and `/compact` change what the model reads for a whole group. Who may invoke them,
and where, is the first question the catalogue has to answer for them — and the answer has
to be one fact, read both by whoever decides the reply and by whoever ever publishes a menu.

## Decision

Both commands are offered in a GROUP, to a moderator and above, and nowhere else. Direct
chats are fenced out: this deployment does not serve group sessions there, and there is no
other session to reset.

The fence is these two COMMANDS', and it says who may ask — nothing about where a session may
be repaired. The unattended compaction runs on direct chats too, deliberately, and decision
0165 records why the two conditions differ.

The floor is the lower edge of the group's administrator set — the same edge the deletion
mirror already reads, where decision 0015 puts the group's owner at admin and its
administrators at moderator. It is checked against the authority the ingest path resolved,
never a claim in the text.

The five privacy commands' row is stated beside it rather than derived from it: they stay
offered to every member in BOTH kinds of channel, exactly as they behave today. The
direct-chat fence belongs to these two commands, not to the catalogue, and a direct-chat
`/privacy` still answers.

An invocation below the floor, or in a direct chat, is recognized, takes the command stamp
— no debt, no model turn, no unlatch — and answers SILENCE. No refusal line goes out,
because a refusal line advertises a surface the person cannot use.

The reading is monotone in standing by construction and by test: whatever a lower standing
is offered, every higher standing is offered too, in both kinds of channel. A caller passes
the LOWEST standing of the audience it is asking about, so a non-monotone row would offer a
moderator something an administrator is refused.

## Rejected alternatives

- **An admin-only floor.** The deletion mirror already trusts a moderator with removing a
  member's stored message, which is heavier and irreversible; resetting a context is not the
  place to be stricter.
- **A refusal line for an invocation below the floor.** It tells everyone in a public group
  that a moderator command exists and invites the flood the silence avoids, and it would be
  the assistant's only unsolicited line about its own moderation surface.
- **Fencing direct chats catalogue-wide.** The privacy family's five commands are data
  rights; making one unreachable in the one place a person can ask privately would be a
  retention-adjacent change made as a side effect of a moderation feature.
