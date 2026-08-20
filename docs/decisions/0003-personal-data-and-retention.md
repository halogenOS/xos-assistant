# 0003 — Personal data lives apart from the ledger; history is kept

Date: 2026-08-20

## Context

The assistant reads a public community group. The people in it are real, they are covered
by European data protection law, and some of them will ask for their messages to be
removed — or will simply delete a message in the chat client and expect it to be gone.

That pulls against the storage model in decision 0002, where a block is permanent and
immutable once written. Append-only storage and a request for erasure cannot both be
honored if the personal data sits inside the blocks.

## Decision

**Personal data is stored in its own tables, never inline in the ledger.** A block holds
its position, its kind, its links and the facts the machinery needs to work; the message
text and the details identifying a person are rows in separate tables, referenced by key.

Erasure then deletes rows in those tables and nothing else. The ledger keeps its shape:
positions stay, references stay, conversations still read in order, and the append-only
rule is never broken. A block whose personal data is gone reports itself as erased and
contributes nothing to what the model sees.

**History is kept as long as it can be.** There is no scheduled expiry of message
content. The assistant is more useful the further back it can look, and privacy here is
served by separation and by honoring erasure, not by a timer that deletes everyone's
history to reach the small part somebody actually wanted removed.

## Rejected alternatives

- **A fixed retention window** (delete all content after some number of days). Rejected:
  it destroys the long memory that makes the assistant able to answer a question about a
  discussion from months ago, and it does not remove the need for erasure on request, so
  the harder mechanism would have to exist anyway.
- **Erasure by deleting blocks.** Rejected: it breaks the append-only ledger, leaves
  holes in conversations, and strands references from other blocks that pointed at the
  deleted one.
- **Encrypting each person's content under its own key and destroying the key on
  request.** Rejected for now: it reaches the same result as deleting the rows while
  adding key management and making every read decrypt. It stays available if a future
  requirement calls for erasure that survives an existing backup.
