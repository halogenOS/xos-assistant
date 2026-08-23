# 0052 — Group membership is authorized, persistently, fail-closed

Date: 2026-08-23

## Context

Anyone can add a bot to a group. The assistant serves the project's own
groups, admitted by the operator — and the admission must survive lost
events, failed leave calls and restarts.

## Decision

The operator's invitation is a durable fact, not a fleeting event. A new
authorization table (its migration step appended per the schema discipline)
records which group channels the operator admitted. It is written exactly
one way: a membership observation — the assistant added to a group, with
the acting principal — whose adder matches the configured operator.
Everything else fails closed: a group message, or any observation, for a
group channel with no authorization row is refused without touching the
ledger, and the refusal carries the withdraw directive; a membership
observation with a foreign adder, with no operator configured, or with no
adder named returns the directive too and records nothing. The check needs
no delivery guarantee: a failed or lost leave call is healed by the next
contact from that group, which is refused and re-directed all over again,
and a restart changes nothing because the authorization is a table row, not
process memory. Existing group mappings at migration time are backfilled as
authorized — they were admitted under the old regime by the operator's own
hand. Direct channels are untouched.

The membership transition the adapter reports is judged by membership, not
by a status pair: from outside the group to inside it, in any member shape
the platform grants (member, administrator, restricted-but-in), and only
for group-kind chats — the platform fires the same update for private
blocks and unblocks, which are nobody's invitation.

## Rejected alternatives

- **A fleeting check on the add event alone.** Fails open on a lost event,
  a failed leave, or a restart.
- **An adapter-side allowlist.** Behavior in the adapter.
- **Startup membership reconciliation.** No platform surface enumerates the
  bot's groups; the fail-closed refusal covers the gap without one.
