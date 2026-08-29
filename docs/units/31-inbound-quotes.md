# Unit 31 — a reply arrives as a quote of what it replied to

Date: 2026-08-28. When a member replies to a message, the assistant sees only the reply's
own text. The relationship is stored — the chat message records the replied-to origin —
but nothing of it reaches the model, so the model reads "The text font tiring my eyes" as
a free-standing sentence and answers a question nobody asked. The operator's instruction
of 2026-08-27, after exactly that failure in the live group: use the framework's existing
quote mechanism, no inferior custom rendering.

This unit lands an inbound reply as a framework `quote` block referencing the replied-to
message, ordered before the reply's own chat message. The model then reads the quoted text
itself, `> `-prefixed, above the member's words — the same thing a human sees.

## Grounding

**The framework mechanism, verified against the merged tree.** A `quote` block stores a
span reference — `start_block_id`, `start_pos`, `end_block_id`, `end_pos` — resolved to
text at store-read time in **character** offsets (`resolve_quote_text` in
`agent-ledger store/blocks.rs`, `chars().skip/take` on both paths), projected as
`> `-prefixed lines with an empty resolution rendering nothing. A user-voiced quote owes
a model turn (`agency/quote.rs`, `Awaiting::Model`). `Store::insert_user_blocks` is
public and accepts `InputBlock::Quote`. The consumer's composing enum already delegates
every framework kind, so the quote projects with no consumer projection code.

**Slice 14 landed 2026-08-29 (framework merge `f2bf250`), so consumer text is reachable
and this unit is unblocked.** Its final shape, which this unit builds against:
`ContentDescriptor::quoted_text_column: Option<&'static str>` declares which column
speaks, validated at open (`validate_quotable_column` in `store/descriptors.rs`): the
column must be declared, must not be the role column, must be `ColumnType::Text` by
VARIANT (a `Json` column is refused despite text affinity), and the kind must not be
ephemeral. Span membership is one SQL decision (`span_members`): a `block_text` row or a
declaring kind; the declared members' text fills through the domain gate
(`fill_declared_text`), and absence stays empty — no declaration, an erased or NULL row,
a dangling reference, a closed gate — the resolver swallows everything to `""`. A
single-block quote (this unit's every case: `start == end`) takes the membership-free
substring path resolution has always had, still gate-consulted for its text. The fork's
deep copy collects quote targets through the same span walk, clones every declared
column of a covered consumer row as a DETACHED clone, and consults the gate before
writing anything. This unit's half of the contract is one field: the chat-message
descriptor declares `COLUMN_TEXT`, which is `ColumnType::Text` (`core/src/kind.rs:584`)
on a non-ephemeral kind, so the open-time validation passes as the tree stands.

**What the consumer already stores.** The chat message records the replied-to message's
ORIGIN string (`COLUMN_REPLY_TARGET`) and a reply-to-assistant flag
(`COLUMN_REPLY_TO_ASSISTANT`), both in the descriptor (`core/src/kind.rs:595-596`), used
to wake the assistant and by erasure's target-keyed pass. An origin-keyed lookup
precedent exists in the deletion path (the doc at `kind.rs:795` names where a reply's
origin is stored). Erasure runs three passes — `erase_principal_content`
(`kind.rs:691`), `erase_message_named` (`kind.rs:736`), `erase_reply_targets_naming`
(`kind.rs:827`) — nulling a message's text and origin, and a quote of an erased row
resolves to the empty string by slice 14's construction — no special case.

**What the platform supplies.** A reply carries the replied-to message; a manual quote
additionally carries the quoted text and a UTF-16 position (`text_quote`, `is_manual`).
UTF-16 offsets against our stored UTF-8 text are exactly the class of conversion this
project's history warns about; the design below avoids the conversion entirely.

**Skipped messages are not in the ledger.** A no-text message (a bare photo album, a
sticker) is skipped at translation (decision 0017), so a reply to one has no block to
reference. The operator's screenshot case — a reply to a captionless album — is therefore
NOT closed by this unit alone; it needs the images unit. Stated so nobody reads this unit
as that promise.

## Decisions taken with this unit

- **A reply lands as a quote block before its message, resolved by reference,
  2026-08-28.** At ingest, when the inbound message replies to an origin whose chat-message
  block exists in the conversation, a `quote` block referencing that block is appended
  first, then the chat message as today. The quote is user-voiced — the member chose to
  attach that context. *Rejected:* rendering the reply link into the projection (unit 26's
  probe killed it: ids leak erased origins and invite wrong-target reports; the quote
  renders text, never an id); *rejected:* copying the quoted text into the quote block
  (a second copy of a member's words that erasure would have to know about — the reference
  resolves at read time, so erasure keeps working for free).
- **The whole message is the default span; a manual quote narrows it by text search,
  2026-08-28.** A plain reply quotes the full stored text (positions `0..char_count`). A
  manual quote's text is searched for in the stored target text: found, the span is that
  character range; not found (edits drifted, media caption mismatch), the whole message.
  *Rejected:* converting the platform's UTF-16 position to a stored offset — the
  arithmetic this repository's history warns about, performed to save a string search;
  *rejected:* trusting the platform's quoted text verbatim into the block (the copy
  problem above).
- **No target, no quote, and nothing is invented, 2026-08-28.** A reply to a message
  outside the ledger — before the assistant joined, skipped as no-text, in another
  conversation, or already erased with its origin nulled — lands the chat message exactly
  as today, quoteless. The stored reply-target fact is unchanged either way. *Rejected:* a
  placeholder quote ("replied to an unavailable message") — content the member never
  wrote, in the member's voice.
- **The reply-target column and the quote block are two facts, not one decision twice,
  2026-08-28.** The column stores the PLATFORM fact (which origin was replied to) and
  serves waking, reports and erasure's naming pass. The quote block is derived projection
  content: it carries no origin, only a span. Erasure nulls the column and empties the
  resolution independently. Recorded because the duplication reviewer must be answered
  rather than dodged: the two records answer different questions and neither can be
  derived from the other at its read site.
- **Replies to the assistant's own messages quote them too, and need no declaration,
  settled 2026-08-29.** The reply-to-assistant flag continues to wake her; the quote
  gives the model the exact words of hers being answered, which is precisely the
  misattribution case the operator hit. Her answers are framework `text` blocks
  (`insert_final_text_block` with `Role::Assistant`) carrying `block_text`, so they are
  quotable natively — the one declaration this unit makes is the chat-message
  descriptor's. *Rejected:* a second declaration for her side — there is no consumer
  table holding her words to declare.
- **Ordering within the ingest, and the crash window, named, 2026-08-28.** The quote
  lands through the framework's public user-block append, then the chat message through
  the consumer append, sequentially under the ingest's existing serialization. A crash
  between the two leaves a user-voiced quote owing a turn with no message — the same
  class of window the ingest's other multi-write sequences already carry, bounded by the
  store's idempotency-and-retry model. Accepted and stated; making the two atomic would
  need a mixed framework-and-consumer append the store deliberately does not have.
- **The date-marker interaction is known and harmless, 2026-08-28.** The framework
  user-block append runs the date-marker seam (slice 13); the consumer append that
  follows runs it too; the same-day dedupe makes the pair insert at most one marker,
  ordered before the quote. Nothing to build; stated so the reviewer finds it decided.
- **A fork's detached clones stay inside erasure's reach, 2026-08-29 — the pin slice
  14 assigned to this consumer.** When a served channel forks (the startup walk that
  moves channels onto an edited prompt or a swapped model), quote targets deep-copy as
  detached clones, chat-message rows included, every declared column copied — principal,
  origin and text ride along. Erasure's passes sweep the chat-message TABLE by what the
  row stores, not by junction reachability, so a detached clone is erased with its
  original; this unit pins that parity rather than assuming it: fork a conversation
  holding a quote of a member's message, erase the member, and both the source and the
  fork must project no quoted text afterwards. *Rejected:* trusting the table-sweep
  argument without the pin — the clone path is new enough that the claim must be
  executable.
- **No new stored fact, no privacy-document change, 2026-08-28.** The quote block stores
  a span reference over content already stored and already sent to the model in the same
  conversation; no new category of data is collected, stored, or reaches any recipient.
  Deliberate claim, to be checked, not assumed: the reviewer verifies no published
  statement enumerates "what a request carries" in a way a quote line falsifies.

## The unit's contract

A member's reply to a message whose block the conversation holds is preceded, in ledger
order, by a user-voiced quote block referencing that message — the whole text, or the
manually quoted span found by text search — and the model reads the quoted text as
`> `-prefixed lines above the member's words. A reply to anything the ledger does not
hold lands exactly as today. An erased target resolves to nothing and renders nothing. The
chat-message descriptor declares its quotable text column, and replies to the assistant's
own messages quote her stored words. The reply-target column, waking, reports and erasure
are unchanged. No new stored fact beyond the quote blocks themselves, no new dependency,
no privacy-document change, and no change to when the assistant answers or stays silent.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** A reply is quoted end to end: a member replies to an earlier member message and
  the model-bound projection carries the quoted text `> `-prefixed before the reply —
  pinned through the real projection fold on a real ingest, not by inspecting the block.
- **AC3** A manual quote narrows: a reply whose quoted text is a substring of the target
  projects exactly that substring; one whose quoted text no longer matches projects the
  whole message — both pinned, the first across a multibyte character boundary.
- **AC4** No target, no quote: replies to a pre-join origin, to a skipped no-text message
  and to an origin in another conversation each land quoteless and identical to today —
  pinned per case.
- **AC5** Erasure empties the quote: after the target message is erased, the same
  conversation's projection renders no quoted text and no marker of it — pinned by
  running a real erasure, and proving the quote block still exists while contributing
  nothing.
- **AC6** A reply to the assistant quotes her words — pinned, including that her waking
  behaviour is unchanged.
- **AC7** The ingest still answers: the quote path adds no new refusal and no silence —
  the reply's turn happens exactly as it would have, pinned in both answering modes.
- **AC8** The descriptor declaration is the one required by slice 14 and is validated at
  open — pinned by the framework's own open-time check, driven from this workspace.
- **AC9** Erasure parity over clones: after a conversation holding a quote of a member's
  message forks through the startup refresh walk, erasing that member leaves BOTH the
  source's and the fork's projections without the quoted text — pinned by running the
  real fork and the real erasure, and proving the detached clone row itself no longer
  carries the text.

## Notes for launch

- Unblocked: slice 14 merged to the framework master 2026-08-29 (`f2bf250`), and this
  workspace path-depends on that master. The branch was rebased onto the consumer
  `main` tip (`4f0f281`) the same day.
- Worktree `~/projects/halogenos-assistant-quotes`, branch `unit/inbound-quotes`. Sites: the ingest edge in `core/src/assembly.rs` (the append
  sequence, around the existing chat-message landing), the origin-to-block resolution
  beside the deletion path's precedent (the origin-keyed lookup named at
  `core/src/kind.rs:795`), the chat-message
  descriptor (and whichever descriptor holds the assistant's own text) declaring the
  quotable column, and the spine tests.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell." If the ingest edge needs a bolted-on conditional
  to fit the quote in, the structure is wrong — restructure the landing sequence until
  the quote is a natural step.
