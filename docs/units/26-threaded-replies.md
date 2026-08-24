# Unit 26 — the assistant replies to the message it is answering

Date: 2026-08-24. In a busy group the assistant's answer arrives as a loose message with
nothing tying it to the question, and the model reading the conversation back cannot tell
which message replied to which either. Both directions of the same relationship are
missing, and the platform carries it in both.

## Grounding (what exists, and what the record says)

**Outbound: the plumbing is built and the decision was deferred, not refused.**
`OutboundReply` already carries `reply_target: Option<String>` (`core/src/message.rs:389`),
the Telegram adapter already translates it into the platform's reply parameters with
send-without-reply tolerance and threads only the first chunk (`adapters/telegram/src/
client.rs:419`), and the report's delivery already sets one. `deliverable_of`
(`core/src/outbound.rs:479-500`) sets `reply_target: None` for an answer, with the comment
"The answer stays unthreaded on purpose — decision 0018's judgment stands".

**What decision 0018 actually decided.** Its original reason (2026-08-21) was that the
outbound edge carried no origin, so "wiring a guess now would thread every reply onto the
newest message, which is wrong in a busy group". Its 2026-08-23 amendment records the
deferral falling due for reports. The objection was to GUESSING the target — not to
threading.

**The target need not be guessed.** Every block a turn writes carries the turn's
`dispatch_anchor`, which is the id of the message that summoned it, and a chat message
stores its platform `origin` (`core/src/kind.rs`, `COLUMN_ORIGIN`). So the message an
answer is answering is a lookup, not an inference.

**Inbound: the relationship is stored and never shown.** A message records
`reply_target` and `reply_to_assistant` (`core/src/kind.rs:151,158,462-467`), and the
system uses them — a reply to the assistant counts as addressing it, which is what wakes
it. But `projected_text` (`core/src/kind.rs:556-569`) renders `[origin] speaker: text`
and nothing more, so the model never learns that one message answers another. In a quiet
group it infers the link from ordering; in a busy one it cannot.

**Erasure nulls both.** The erasure pass nulls the origin and the reply target with the
text, and `erase_reply_targets_naming` nulls a target that names an erased message
(`core/src/kind.rs:140`). Anything built here reads what may already be absent.

## Decisions taken with this unit

- **An answer threads onto the message that summoned its turn, 2026-08-24.** The operator
  asked for it; the original objection is spent. The target is the platform origin of the
  turn's dispatch anchor — a stored fact, not the newest message and not a guess. This
  **supersedes decision 0018's answer clause**, which is amended in place with its date and
  this reason rather than quietly contradicted.
- **A target that cannot be resolved degrades to a plain send, 2026-08-24.** No anchor, an
  anchor that is not a member message, an origin nulled by erasure, or a target the platform
  refuses: the answer still goes out, unthreaded. An answer is never withheld for want of a
  thread — the reply is the point and the thread is the courtesy. This differs from the
  report's undeliverable path, which is accounted delivered and dropped, and differs on
  purpose: a report names a specific message and means nothing without it.
- **Only the answer's first chunk threads, 2026-08-24.** The adapter's existing behaviour,
  inherited rather than re-decided.
- **The projection shows the reply relationship, 2026-08-24.** A message that replies to
  another renders its target's id beside its own, so the model can follow the link it
  already has ids for. It reads as an addition to the existing `[origin] speaker: text`
  shape rather than a new vocabulary, and it appears only when a usable target is stored —
  an erased or absent target renders exactly as today. The implementer settles the exact
  mark against the existing projection prose; it must not collide with the origin mark's
  meaning, because the model already uses that id to name a message when it reports.
  *Rejected:* rendering the quoted text of the replied-to message (it is already in the
  conversation the model reads; quoting it again spends context to repeat what is there and
  doubles the erasure surface).
- **The assistant's own messages carry no reply link in the projection beyond what is
  stored, 2026-08-24.** This unit adds no new stored fact. It renders what the ledger
  already holds and threads what the ledger already anchors.

## The unit's contract

An answer is delivered as a reply to the message that summoned its turn, resolved from
that turn's anchor, threading the first chunk only; where the target cannot be resolved or
the platform refuses it, the answer is sent plainly rather than withheld. A message that
replies to another shows that link in what the model reads, when a usable target is
stored. Reports keep their existing threading and their existing undeliverable handling.
No new stored fact, no new configuration, no new dependency, and no change to when the
assistant answers or stays silent.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** An answer threads onto its summoning message: a turn summoned by a member's
  message delivers with that message's origin as the reply target — pinned, including that
  it is the SUMMONING message rather than the newest message in the channel, proven by a
  history where those differ.
- **AC3** Degradation is graceful and the answer still goes: no anchor, a non-message
  anchor, and an anchor whose origin was nulled by erasure each deliver the answer with no
  target — pinned for all three, and no case yields silence.
- **AC4** Reports are untouched: a report still threads onto its reported message and a
  report whose target is nulled is still accounted delivered and dropped — existing pins
  pass unchanged.
- **AC5** The projection shows a reply link: a message replying to another renders its
  target beside its own id, a message replying to nothing renders exactly as today, and a
  message whose target was erased renders exactly as today — pinned, against the composed
  projection rather than the column.
- **AC6** Erasure is not weakened: after erasure the erased message's own projection is
  still only the marker, and no reply link anywhere reveals its text or origin — pinned.
- **AC7** Decision 0018 carries its second amendment with this unit's date and reason, and
  the outbound comment asserting answers stay unthreaded is gone — checked, since a record
  contradicting the code is what this unit is correcting.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-replies`, branch
  `unit/threaded-replies`). Sites: `core/src/outbound.rs` (`deliverable_of` ~479, the
  delivery loop, the report's target path), `core/src/kind.rs` (`projected_text` ~556,
  `COLUMN_ORIGIN`, `COLUMN_REPLY_TARGET` ~151), `core/src/message.rs` (`OutboundReply`
  ~373), and the adapter's existing reply parameters (`client.rs` ~419) which need no
  change.
- The anchor-to-origin lookup is the piece to write: an answer block's `dispatch_anchor`
  names a block id, and that block's stored origin is the target. A similar anchor lookup
  existed for the removed miss routing; read the report's target path for the shape the
  edge already uses.
- Do not add a stored column for this. Everything needed is already recorded.
