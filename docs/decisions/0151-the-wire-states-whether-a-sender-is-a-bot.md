# 0151 — The wire states whether the sender is a bot, and states it nowhere else

Date: 2026-08-30, with unit 42.

## Context

The assistant welcomed a joiner in the live group because a moderation bot's captcha
prompt drew a turn: an ordinary inbound message from a bot member, summoned like anyone's,
with the join notice sitting in the same context. The operator decided the fix's shape —
a bot may not trigger this assistant at all unless it mentions the assistant.

Deciding that in code needs one fact nothing carried: whether the sending account is
automated. The platform states it on every sender object and on every joiner of a join
notice; the adapter's decode types skipped the field, so the fact never crossed the
boundary.

## Decision

The sender identity that crosses the adapter boundary carries three facts, not two: the
opaque external id, the username, and whether the account is a bot. The adapter's sender
and joiner decode types read the platform's own flag, absent decoding as false — the
wire's own meaning, since only the platform asserts that an account is automated. All
three places the adapter builds an identity fill it from their own account's flag: a
message's sender, the acting principal of the assistant's own entry into a group, and each
joiner of a join notice, where the flag is that joiner's.

The fact is stored NOWHERE. No column, no migration, no erasure pass, no privacy document
change: it is read fresh off every update, consumed by the two readings unit 42 decides —
the adapter's addressing and the core's summons resolution — and dropped. It is
platform-neutral by construction: every platform this assistant will meet either marks
automated accounts or leaves the flag false, which is what "no automated account is known
here" means.

This widens decision 0077, openly. That decision left the crossing identity at exactly two
fields and its assertion said so; the assertion now names three, the identity's own
documentation names the third, and 0077 carries a dated amendment so no sentence of it
contradicts the tree.

## Rejected alternatives

- **A field on the message.** The fact belongs to the account, not to one message of it. A
  per-message copy would invite a caller to believe two messages of one account could
  differ.
- **Persisting it on message rows.** Nothing reads it after the stamp is composed, and a
  stored copy would drift from the account's current state the moment a platform reissued
  it. A column also means a migration, an erasure question, and a published sentence about
  data held for nothing.
- **Deriving it from the username's shape.** Handle conventions are a naming habit, not a
  fact; the platform already states the fact.
