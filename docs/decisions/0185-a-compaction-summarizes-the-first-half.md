# 0185 — A compaction summarizes the first half and carries the second verbatim

Date: 2026-08-31, with unit 48. Supersedes decision 0163.

## Context

The model reads the WHOLE conversation every turn. Unit 45 answered that with a tail keep:
a fork holding the trailing chat rows and nothing else, with everything older set aside.
That answer threw the older conversation away — a group's history simply stopped existing
for the assistant at the cut — and it had no way to shorten a long conversation that was
all chat and no tool traffic.

The design this unit builds replaces it: the older half is not discarded, it is
SUMMARIZED, and the recent half is carried forward word for word.

## Decision

One mechanism, and the same one behind every door into it.

The ledger is cut in half by the framework's own deterministic rule: the block at half the
ledger BY BLOCK COUNT, resolved to the message group containing it, whose LAST block ends
the first half — then extended forward while any tool call inside the first half has its
outcome beyond it. Two properties fall out and everything downstream rests on them: no
message group is ever split, and no tool lifecycle is ever split, so the second half can
never open on an orphaned outcome.

The first half is forked into a TEMPORARY conversation, which records an empty tool palette
and then, last, the compaction instructions. The order is the mechanism: the instructions
are a system-voiced harness message, and appending one is what summons a turn, so
everything that turn must be governed by has to be in the ledger before it lands. That
turn's answer is the summary. The temporary conversation is retired junction-only the
moment its answer is read.

A new thread then opens with the current prompt, a block naming the conversation it
continues, the summary, and the second half of the source's ledger — the same blocks,
shared through the junction, never copies. The channel moves to it through the claim's own
winner check, and it is served like any other conversation.

Nothing is deleted. The source keeps every block it had; the thread simply holds fewer of
them.

Two readings are STATED rather than derived, because the design does not decide them
and the record should show what was chosen:

- **Half is measured by BLOCK COUNT**, not by tokens or by characters.
- **A group straddling the half point lands whole in the SUMMARIZED half.** The opposite
  side is equally consistent with taking half the ledger; this side is chosen so a
  straddling exchange is summarized whole instead of having its opening split from its
  summary. The near side is the FALLBACK, and only that: when the far side would leave no
  second half at all — one group running from the half point to the tail — the group rides
  across verbatim instead, so a conversation that has to be compacted is never one that
  cannot be.
- **The compaction message is appended in the SYSTEM voice.** The harness is stating what
  the earlier history held; the model is not recalling it. Every wire this deployment
  reaches folds system messages into its system parameter, so the digest reads as context
  ahead of the verbatim half rather than as somebody's turn.

## Rejected alternatives

- **Keeping the tail keep beside this.** Two compaction shapes is two answers to one
  question, and the second one would be wrong eventually. The tail-keep plan is gone whole.
- **Deep-copying the second half instead of sharing it.** Copying a conversation's blocks
  to move them is the thing the junction table exists to avoid, and it would break the
  outbound edge's inherited boundary — every carried answer would look freshly authored and
  go out to the chat a second time.
- **Fusing the ancestor reference and the summary into one block.** They say different
  things: one is where the history came from, the other is what it said. The design orders
  two appends and two appends is what the code does.
