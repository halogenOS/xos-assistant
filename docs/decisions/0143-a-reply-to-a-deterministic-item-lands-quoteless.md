# 0143 — A reply to a deterministic item lands quoteless, stated

Date: 2026-08-30, with unit 38.

## Context

Not everything the assistant sends is one of her blocks. The rules acknowledgment and
the privacy commands' answers are the core's own prose, returned by a call and never
stored. The failure notice is derived from an event and never stored either. A report's
line is stored, but its block declares no quotable text column.

## Decision

Their delivery receipts record what reached the chat, like every other send, and name no
block. The resolution answers nothing for them, so a reply to one of them lands
quoteless — recorded, answered, and simply carrying no quote. Stated here rather than
left for a reader to find out, because a reply to an acknowledgment is an ordinary thing
for a member to do.

## Rejected alternatives

- **Quoting the report block anyway.** Its descriptor's declaration is another unit's
  decision and not this one's to reopen, and a report line quoted back at the model
  would put a filing in front of it as if it were something said in the room.
- **Skipping their receipts, since nothing quotes them.** The record exists for more than
  quoting, and a class of her messages left unrecorded is the omission the receipt's
  design was corrected once to remove.
- **Inventing a quote from the fixed prose.** The quote's endpoint is a block, and prose
  that is not a block would have to be copied somewhere to be quoted — a second place
  the same words live.
