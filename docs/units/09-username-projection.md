# Unit 9 — the username travels with the message

Date: 2026-08-23. Revision 1. Decision 0056 is the operator's: the assistant should
address people the way the group does, by their handle, so the public username
travels with the message to the model provider. This unit makes the recorded
decision true — the published policy already describes it.

## Decisions taken with this unit

- **The speaker is a column on the message row, written at receipt, 2026-08-23.**
  The projection reads one block with no ledger access, so the username the model
  sees must live on the row. The chat-message row gains a nullable speaker column
  (its own appended migration step, frozen-list discipline): the sender's public
  username as the platform delivered it at receipt — the handle as it was when
  the person spoke, which is the historically honest value. The identity tables
  keep owning who is who; the column is a projection fact, not an identity
  fact. The author-keyed erasure pass nulls it beside text and origin, so
  deletion keeps its promise. Rejected: projecting through a ledger join (the
  projection trait reads one block; a context-bearing projection is a framework
  change for one column); reading the identity table's CURRENT handle (a
  renamed person would be retroactively re-labeled through their whole
  history, and the projection would need the join anyway).
- **The projection prefixes the speaker, and only for people, 2026-08-23.** A
  user-role message with a speaker projects as the speaker, a colon and a
  space, then the text; the assistant's own messages and system-voiced blocks
  are unprefixed. A message whose sender has no public username — the platform
  makes handles optional — projects bare, unprefixed: no handle means the
  group cannot mention the person either, so the assistant loses nothing it
  could have used, and no substitute identifier leaves the machine (decision
  0056 rejected the display name and the numeric identifier by name). An
  erased message's placeholder stays exactly as it is — the erasure pass
  nulls the speaker with the text. Rejected: a placeholder label for the
  handleless (a minted pseudo-identifier, the exact thing 0056 retired); the
  display-name fallback (rejected in 0056).
- **The prompt may address people; the policy already says so, 2026-08-23.**
  The system prompt's teaching gains one line: the model may mention a person
  by the handle shown with their message, and must never guess a handle it
  was not shown. The privacy documents were updated with decision 0056
  before this unit; the DPIA's transmitted-identifier line stops saying "not
  yet built" — a dated note, drafts are amendable in place.

## The unit's contract

The speaker column by appended migration step; the write path fills it from
the resolved sender; the author-keyed erasure pass extends to it; the
projection prefix per the rule above; the prompt line; the DPIA note. The
adapter already delivers the username in the sender identity — no adapter
change beyond what translation already carries. No configuration.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency; a previous-unit store
  upgrades through the appended step alone, existing rows projecting bare —
  pinned.
- **AC2** End to end over the scripted provider: a group message from a
  handled sender reaches the model as `handle: text`; a handleless sender's
  message arrives bare; the assistant's own answers carry no prefix — pinned
  on the outbound request.
- **AC3** Erasure: the speaker nulls with the author's pass; the erased
  placeholder projects unchanged; a post-erasure message from the same person
  carries the handle again — pinned.
- **AC4** The prompt line ships; the DPIA note is dated; the decision records
  this unit's closure of 0056's implementation debt — pinned in the docs test.

## Ratified at the unit's close, 2026-08-23

The write path stores the speaker BOUNDED, beyond the contract's letter: an
empty, whitespace-bearing or separator-bearing value is not stored and the
message projects bare. The projection prefix must be unambiguous, and the
bound is the core's to own rather than an assumption about one platform's
handle alphabet.
