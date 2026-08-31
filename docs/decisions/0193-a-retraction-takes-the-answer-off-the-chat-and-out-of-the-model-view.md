# 0193 — A retraction takes the answer off the chat and out of the model's view

Date: 2026-08-31, with the message-retraction unit.

## Context

A group administrator replies to one of the assistant's own messages with the
moderation bot's deletion command. Until this unit that reply mirrored nothing:
the assistant's own words are no person's row, so the deletion mirror declined
it and the message recorded as ordinary.

Two things had to be settled. What happens in the CHAT, which is the
capability the unit's title names. And what happens in what the assistant
READS, which the earlier draft of this design left alone on the reasoning that
the ledger appends and never rewrites — the retracted answer would have kept
its text, its place and its projection, and the assistant would have gone on
speaking from an answer an administrator had taken back.

## Decision

**One command, two effects, decided by what the reply names.** The trigger
stays the moderation bot's own token under the same three conditions the
mirror already applies: a group channel, the reported invoked command, and a
sender at or above the administrator floor. A reply naming a person's message
erases that stored row, unchanged. A reply naming one of the assistant's own
messages retracts the delivery that message belonged to. Administrators learn
one command, and the existing bounds carry over untouched — a token aimed at
another bot by name is no invocation here and does nothing.

**The retraction is keyed on the delivery, and a repeat records one fact.** A
retraction block records one delivery key, so an administrator replying to the
third message of a chunked answer and one replying to the fifth are asking for
the same thing. A delivery that already carries a retraction gets no second
one: the recorded fact is that an administrator asked for this delivery to go,
and asking twice is one ask. The wire call is re-issued on every repeat,
because the first may have failed and an administrator who sees the message
still standing is saying so.

**A delivery goes whole, through the plural deletion method, in batches of at
most a hundred.** An answer past the platform's message cap is already several
messages, and taking back only the replied-to one would leave the group reading
the remainder of a retracted answer. One method serves every size: its stated
range starts at one, and it skips an identifier it cannot find while the
single-message method refuses with a client error — and the commonest reason an
identifier is missing is the moderation bot deleting the same message on the
same command a moment earlier.

**The retracted answer leaves the model's view, through a fork.** Nothing is
rewritten: the channel's session is forked without the retracted answer's own
blocks and without every quote derived from them — a member's reply quote, and
the deletion command's own reply quote, which every retraction creates by
construction. The delivery receipts stay, because a receipt records what the
platform took; a receipt whose answer block is gone resolves no quote, which is
the correct reading of a message that was taken back. The command row and the
retraction fact ride into the fork: the lawful record goes forward. The blocks
the fork left behind are held by the retired conversation alone and go to the
existing orphan collector.

**An answer below a compaction boundary takes the digest scrub.** There the
answer survives only as prose a model wrote about it, which cannot be edited
free of one message the way a junction row is dropped. The chain is rebuilt
from a clone of the ancestor without the answer, each digest regenerated from
the clone beneath it — the same mechanism, and the same per-hop model-turn
cost, that an erasure already accepts.

**The fork runs outside the ingestion's holds, and settles the turn first.**
Recognition and the retraction fact land inside them, where the ledger is
serialized; the fork runs once they release, because it re-takes both for its
own swap. The serving conversation's open turn is interrupted and confirmed
settled before the swap, exactly as a session reset does, so a command arriving
mid-answer cuts that answer short.

**The strip is unconditional, and the platform's refusal does not undo it.**
When the chat deletion is refused — the platform's own 48-hour window, or a
message already gone — the refusal is logged and dropped while the retraction
stands and the fork has run. The group then keeps text the assistant has
dropped from its own reading. That is the honest cost of recording the ASK
and never the outcome, and the published privacy documents state it.

**An edited message that becomes the deletion command retracts nothing.** The
reasoning is the mirror's own: nothing establishes that the moderation bot acts
on an edited command, and a retraction acts on the chat and on the assistant's
reading on the strength of it. The command is still recognized, so the row
records silently and takes no turn.

**The retraction shows the model nothing.** A line could not name the answer it
retracted without projecting the assistant's own message identifiers, and a
line appended at the tail would read as a retraction of whatever answer
happened to be newest. Since the retracted answer is out of the view anyway,
there is nothing to say about it.

## Rejected alternatives

- **Leaving the retracted answer in the model's view.** The ledger's
  append-only rule was the argument, and the fork honours it: nothing is
  rewritten, the blocks are dropped from a copy, and the source keeps every one
  of them until the collector runs. Left in the view, the assistant would go on
  answering from prose an administrator removed from the chat, which is the
  divergence between store and chat the deletion mirror exists to prevent.
- **Nulling the retracted answer's stored text the way the mirror nulls a
  person's row.** The nulling path exists for personal data in a table of its
  own, which is the carve-out that lets a person's rights coexist with
  append-only storage. The assistant's own prose is not that, and borrowing the
  erasure path for it would blur the one distinction the storage design rests
  on.
- **Leaving the turn in flight alone.** An answer being written from a
  conversation the swap is about to unmap lands where nothing delivers it, and
  the streaming tail it had already written would ride into the fork only to be
  swept away. Cutting it short is what every other session replacement already
  does.
- **Running the fork inside the ingestion's holds.** The fork re-takes the
  ingestion lock for its swap, and the compacted case re-takes it through the
  digest scrub, so it would deadlock on the lock the rest of the ingestion is
  already holding.
- **Appending the retraction only on success.** The ledger would then record
  the administrator's ask as never having happened, and the outcome is known
  only later, in the adapter, which would need a second receipt path for a fact
  nothing else wants.
- **The single-message deletion method for a one-message delivery.** A branch
  that buys nothing and splits one act along an accident of length, and it
  turns the commonest case — a message the moderation bot already deleted —
  from a silent success into a logged failure.
- **A separate command owned by the assistant.** A second vocabulary for one
  human intent, and a command the administrators would have to be taught
  separately from the one they already use.
