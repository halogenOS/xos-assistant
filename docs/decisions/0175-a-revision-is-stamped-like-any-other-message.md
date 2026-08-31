# 0175 — a revision is stamped like any other message

Date: 2026-08-31, with the editing unit.

## Context

A person who edits a question into shape is asking it.

## Decision

The same summons resolution, the same budgets, the same debt propagation,
the same absorption rule for a message arriving mid-turn (decision 0010). A
person who edits five times spends five of their own budget slots, which is
what the budgets are for. A revision arriving while its own original is
being answered is absorbed exactly like any mid-turn message, so the answer
in flight may answer the pre-edit wording and the next turn sees both
versions — the framework's scheduling law, unchanged here. In a direct
channel every message is addressed by the channel's nature, so every
revision there summons, exactly as every message there does.

## Rejected alternatives

- **A revision that never summons.** A member fixing a typo in their
  question would never be answered, and under platform privacy mode the edit
  route is the only one that stays open for a message the bot already knew.
- **A separate edit budget.** A second counter measuring the same thing.
