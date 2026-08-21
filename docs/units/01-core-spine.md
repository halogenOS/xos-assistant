# Unit 1 — the core spine

Date: 2026-08-21. Revision 2, rewritten after an unbriefed review by two probing
reviewers returned two blockers and a set of gaps, each verified against the framework's
real tree. Status: settled for implementation.

The framework library this project decided to extract is complete and stands alone:
generic runtime machinery, a content-descriptor system for consumer block kinds, a derive
that composes them, and a proven out-of-crate path from a stored row to an answered turn.
This unit makes the assistant its first consumer.

## The stage, planned

Units follow one at a time, each through the full workflow. Later units get their own
spec when they start; the list below is the order and the reason, not their contract.

1. **The core spine** (this unit): assistant-core consumes the framework; a
   platform-neutral inbound message becomes a ledger block, takes a turn against a
   scripted provider, and yields an outbound reply on a subscription edge.
2. **The Telegram adapter**: long polling, translation both ways, nothing else.
3. **The live model**: the OpenRouter provider, system prompt in its own files, the
   acting policy (record every group message, answer the addressed ones), and
   user-facing failure behavior on the outbound edge.
4. **Protection and enforcement**: rate and abuse protection, the one-turn-one-authority
   queue, then the feature tools and spam reporting.

## Decisions taken with this unit

- **Framework dependency by sibling checkout, 2026-08-21.** The library has no public
  home yet; publishing it is the maintainer's call and has not been made. Until then the
  manifest names a relative path dependency and the README states the expected layout.
  Rejected: a git URL (no remote exists to name); vendoring (a fork of our own fresh
  fork, two trees to keep aligned for zero benefit).
- **The message model is the core's own vocabulary, 2026-08-21.** An inbound message
  carries a channel key, the sender's identity, an authority level, the text, an
  optional origin reference and a timestamp. Adapters translate their platform's types
  into this model at the boundary and never past it. Rejected: reusing a platform SDK's
  update type in the core (breaks the no-platform-vocabulary invariant on day one); a
  trait the core calls back into (inverts the dependency and gives adapters a behavior
  surface).
- **Sender identity crosses the boundary; the principal id does not, 2026-08-21.** The
  inbound message carries what the identity store needs — the sender's opaque external
  id and display fields — and the entry point resolves or creates the principal from
  them; only the principal id enters the ledger. Rejected: the adapter carrying a
  principal id (it would need identity-store access, which the edge contract forbids);
  a separate registration call before ingestion (two calls that must agree, and a
  message from an unseen sender would still need the fallback).
- **Every recorded message awaits the model in this unit, 2026-08-21.** The acting
  policy — record all, answer some — is behavior on the block kind and arrives with the
  live-model unit, where the addressing rules exist to express it. Wiring a placeholder
  policy now would be a second decision site to unwind later. Rejected: a stub
  mention-check in the core (platform addressing rules are adapter knowledge; a wrong
  boundary is worse than a late one).
- **Authority is recorded on the message block as text, 2026-08-21.** Enforcement of
  the one-authority turn comes with the protection unit, but the ledger must already
  carry the fact, or the enforcement unit would inherit a history it cannot classify.
  The stored encoding is a text column with the fixed vocabulary `member`, `moderator`,
  `admin`; ordering lives in code. Rejected: deriving authority at read time from the
  identity store (authority is resolved live by the adapter at receipt; re-deriving
  later reads today's role into yesterday's message); an integer encoding (opaque in
  the stored row, and the vocabulary is closed anyway).
- **The unit writes its own scripted provider, 2026-08-21.** The framework ships no
  reusable scripted provider — its own out-of-crate test defines one privately against
  the public provider traits, which are sufficient. The unit does the same in its test
  code; no framework change is expected. Rejected: extracting a shared test-support
  feature into the framework (a new public surface for one consumer's tests, before a
  second consumer exists to shape it).
- **A mid-turn message is absorbed, not individually answered, 2026-08-21.** The
  framework fires a turn only when the newest block awaits the model, so a message that
  arrives while an answer is streaming sits behind that answer once it finalizes and
  draws no turn of its own; it joins the context of the next turn instead. The unit
  pins this with a test. OPEN, surfaced to the framework's improvements batch: a
  conversation whose newest block is a finalized answer never reconsiders buried
  messages until the next append, which can leave a trailing message unanswered
  indefinitely in a quiet channel; the follow-up decision (a post-finalize
  reconsideration or similar) belongs to the framework, not this unit.
- **Erasure covers direct channels, 2026-08-21.** A group channel's key names the
  group, not a person; a direct channel's key names the person and is personal data.
  The mapping records the channel kind at creation, and erasing a principal removes,
  besides the identity rows, the direct-channel mappings whose conversations contain
  that principal's messages — found by reading the ledger, never by writing it.
  Rejected: erasure of identity rows alone (leaves a personal identifier in the
  mapping); the caller supplying channel keys to unmap (pushes a data-protection
  obligation onto every caller and makes the one operation two).
- **Erasure reaches the prose — reconciled with decision 0003, 2026-08-21.** This
  spec's first revision scoped erasure to identity rows and mappings and left the
  message text in place; the unit's correctness review flagged that against decision
  0003, which requires the text itself to be erasable, and 0003 wins. The
  reconciliation: under the framework's block model the kind's content table is
  exactly 0003's separate personal-data table referenced by key — the block header
  row is the immutable ledger entry, the content row carries the personal payload.
  The personal columns — the text, the origin reference and the platform send
  time — are therefore nullable; erasure nulls them for the erased principal in
  every conversation, an erased message projects nothing to the model, and a
  direct conversation of the erased principal is removed entirely, since
  conversation-level removal of a two-party chat leaves no holes and strands no
  references — the two failure modes 0003's rejected block deletion was rejected
  for — and an erased skeleton's remaining metadata would itself be the person's
  data. Rejected: an out-of-row prose table (the framework's projection reads
  descriptor fields only; a second-table load path does not exist, and inventing
  one would duplicate the content table's own mechanism); keeping direct
  conversations as erased skeletons (metadata that serves nobody and still
  identifies the person). OPEN, recorded in the decision record: a group
  conversation's derived title may have been shaped by since-erased prose; title
  regeneration on erasure is later work.

## The unit's contract

### The message model

Core types for one inbound message and one outbound reply. The inbound message carries:
a channel key, the channel kind (direct or group), the sender identity, the authority
level (`member` < `moderator` < `admin`), the message text, an optional origin
reference (the platform's own message id, opaque, kept for later reply threading), and
a timestamp. The channel key is an opaque pair — adapter name plus the adapter's own
conversation identifier — compared only for equality. The sender identity is the
sender's opaque external id plus display fields (display name, optional username);
it is the input to principal resolution and never enters the ledger itself.

### The block kind

One consumer block kind, ChatMessage, composed with the framework's kinds through the
derive. Its descriptor declares one content table with typed columns for the role
(text, written from the append's role argument and read back into the block's role),
the message text, the principal id, the authority level (text, vocabulary above) and
the optional origin reference. The kind's projection renders the message for the model;
its agency hooks make an appended message await a turn per the decision above.

### Identity, separated

Sender identity — external platform id, display name, username — lives in tables of
its own, keyed by principal id, created by domain migrations, never in ledger content.
The entry point creates the principal on first contact and refreshes the display fields
on later messages. Ledger blocks store the principal id only.

### Conversation mapping

A channel key maps to a ledger conversation in its own table, together with the channel
kind, created on first message. The mapping is the only place a channel key is stored.

### Erasure

A public core operation, separate from the adapter edges: given a principal id, one
call runs three idempotent steps — the personal columns of the principal's
messages (text, origin reference, platform send time) are nulled in every
conversation, the principal's direct conversations and their mappings are removed
entirely, and the identity rows are deleted. No block header row is ever mutated;
the text lives in the kind's content table, which is the separate personal-data
table of decision 0003. An erased message projects nothing to the model. Erasing an
unknown principal reports that plainly instead of succeeding idly. This is the
operation AC3 exercises; wiring it to an operator surface is a later unit's work.

### Core assembly

The core is constructed with its runtime wiring: the store (opened with the descriptor
and the domain migrations), the bus, the provider registry with one registered provider
instance, and the model binding — provider instance id, external model id, display
name, vendor — under which first-message conversation creation happens. The entry
point draws this binding from the assembly, never from the message. In this unit the
registered provider is the scripted one; the live-model unit replaces the assembly's
provider configuration, nothing else.

### The ingestion edge

One public core entry point receives an inbound message: it resolves or creates the
principal, maps the channel (creating the conversation under the assembly's binding on
first message and releasing the conversation's boot latch with the explicit unlatch
intent — without it no turn ever fires), appends the ChatMessage block through the
framework's consumer write path, and returns. A stream error re-latches a conversation;
this unit does not unlatch again on error (failure behavior is the live-model unit's
decision, noted there).

### The outbound edge

One public subscription edge yields outbound replies. It is built on the framework's
event subscription as the wake signal only: events carry no answer text, so on a
completed stream the edge reads the answer block from the ledger — the framework's own
re-derive discipline — and maps the conversation id back to the channel key through
the mapping table. Delivery is at-least-once from stored state: a lagging subscriber
recovers by re-reading, never by replaying events. On a stream error the edge yields
nothing for that turn (see the ingestion edge's note). The title derivation the
framework's metadata worker runs on young conversations is not a reply and never
reaches the edge.

### The scripted provider and the title stream

The unit's test provider implements the framework's public provider traits. The
metadata worker sends a title-derivation request for every young conversation on the
same binding; with no tools registered in this unit, the provider discriminates on the
request content — the title request carries the framework's title instruction, a turn
does not — and answers each kind deterministically, so turn-count and block-by-block
assertions stay exact.

### Scope fence

No Telegram code changes, no live provider, no tool registration, no acting policy, no
rate protection, no framework changes. The adapter crate keeps its skeleton.

## Acceptance criteria

- **AC1** The workspace builds with the framework dependency; the test suite runs
  without the network and passes in parallel and under a single thread identically.
- **AC2** The composed kind opens: the store opens with the descriptor and the domain
  migrations, validation passes, and a reopened file-backed store proves the durable
  registry path.
- **AC3** The erasure test (amended 2026-08-21 with the 0003 reconciliation): a group
  conversation carries messages from two principals, and each principal has a direct
  conversation. Erasing one principal succeeds in one call; the erased principal's
  identity rows, direct conversation and direct mapping are gone while the other
  principal's remain; the group conversation's block count is unchanged and every
  block still loads; the erased principal's group messages carry no stored text,
  origin reference or send time and project nothing to the model; the other
  principal's messages are untouched.
  Erasing a principal id that matches nothing returns the not-found outcome.
- **AC4** The end-to-end test: an inbound message through the public entry point wakes
  the runtime, the scripted provider streams an answer, and the outbound edge yields
  that answer bound to the correct channel key — asserted on the resulting ledger
  block by block, with the title derivation accounted for and excluded from the edge.
- **AC5** Two messages on different channel keys produce two conversations with no
  cross-talk, proven by the same assertions.
- **AC6** A mid-turn arrival is pinned: a second message appended while the scripted
  stream is open draws no turn of its own and appears in the next turn's projected
  context; the test states the observed order.
- **AC7** The platform-vocabulary scan: a committed word list (at minimum the platform
  names and their SDK crate names) and a committed test that greps the core crate's
  code and comments against it find nothing; the list is the file adapters grow.
- **AC8** Clippy with warnings denied, fmt, and doc builds are clean across the
  workspace.
- **AC9** Any new external dependency is recorded in this repository's dependency
  review document — current version checked from its registry, advisory database
  consulted — before a manifest names it. The framework itself is recorded there as
  reviewed in its own repository.
- **AC10** The decisions above appear in the repository's decision records, dated,
  with their rejected alternatives.
