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
writing anything. This unit's half of the contract is one field
flipped: main already carries `quoted_text_column: None` on all five descriptors (the
integration commit `a4db173` that made the workspace compile against the framework
tip), and this unit turns the chat-message descriptor's to `Some(COLUMN_TEXT)` —
`ColumnType::Text` (`core/src/kind.rs`) on a non-ephemeral kind, so the open-time
validation passes as the tree stands. The other four stay `None`.

**What the consumer already stores.** The chat message records the replied-to message's
ORIGIN string (`COLUMN_REPLY_TARGET`) and a reply-to-assistant flag
(`COLUMN_REPLY_TO_ASSISTANT`), both in the descriptor (`core/src/kind.rs:595-596`).
Waking is decided at translation from the live message (`replies_to_bot` feeding the
addressed flag in `translate.rs`); the stored flag has no production reader today, and
the reply-target column is read by erasure's target-keyed pass. No production code maps
an origin to a block id — the erasure passes NULL by origin without ever selecting the
id — so the quote's origin-to-block resolution is new code with no existing lookup to
adapt. Erasure's nulling passes include `erase_principal_content`
(`kind.rs:692`) and `erase_message_named` (`kind.rs:737`), each nulling a message's
text and origin, and `erase_reply_targets_naming` (`kind.rs:828`), which nulls only
the reply-target reference column on other rows; person-wide erasure composes these
with the join, report and reported-origin passes. A quote of an erased row resolves
to the empty string by slice 14's construction — no special case.

**What the platform supplies, and where it must be carried.** A reply carries the
replied-to message; a manual quote additionally arrives in the message's `quote` field
(a `TextQuote`: `text`, `position` in UTF-16 code units, `is_manual`). UTF-16 offsets
against our stored UTF-8 text are exactly the class of conversion this project's
history warns about; the design below reads only `text` and `is_manual` and never the
position. Nothing carries these fields today, so the unit threads one neutral fact
through four named sites: the adapter's decoder gains the `quote` field
(`crates/adapters/telegram/src/client.rs`, beside `RepliedTo`), translation surfaces
it, the intake's pending build copies it (`crates/adapters/telegram/src/driver.rs`),
and the core's platform-neutral inbound message (`crates/core/src/message.rs`) gains
the quoted-text fact — text and the manual flag, nothing else. The adapter translates
vocabulary and carries the fact; every decision about it stays in the core.

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
  manual quote's text is searched for in the stored target text: found, the span is the
  FIRST occurrence's character range — deterministic, and any occurrence carries the
  same words; not found (edits drifted, media caption mismatch), the whole message.
  *Rejected:* disambiguating repeated occurrences with the platform's UTF-16 position —
  the conversion this decision exists to avoid, spent on choosing between identical
  strings.
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
- **The quote never answers and never asks, in this consumer, 2026-08-29.** Two
  mechanisms, one principle: a quote is context a member attached, not a voice of
  its own here. FIRST, the debt walk: a quote at the tail — the exact state a crash
  between the two appends leaves, and what the tail-skip then preserves on retry —
  must not settle a debt it merely sits on. The quote kind therefore joins
  `NEVER_ANSWERABLE` (`core/src/kind.rs:855`, today the date-marker kinds), the list
  whose recorded meaning is that the fact holds for every reader — NOT
  `DEBT_READ_THROUGH`, which is recorded as the consumer's policy about its OWN
  kinds and would be contradicted by a framework kind in it. The list is read by
  exactly the owing-tail walk's two sites, and that is the whole reach; nothing
  else anchors on a newest message. SECOND, the turn: the framework serves a model
  turn for any user-voiced frontier, so an orphaned quote (its message refused on
  retry, or sitting bare at restart before the redelivery lands) would be answered
  on its own. In this consumer the turn duty lives on the chat message's answer_due
  stamp alone, so the override goes where the consumer already speaks for framework
  kinds: `FrameworkKind`'s hand-written `Agency` delegation (`core/src/kind.rs:1099`)
  answers no awaiting for a quote and delegates everything else untouched — one
  match, at the one boundary that is already the consumer's recorded policy seam
  over the framework's kinds. Storage, rendering and resolution never move. A lone
  quote asks for nothing; its message, when it lands, owes exactly what it owes
  today. *Rejected:* a new leaf arm claiming the quote string — the framework's
  derive refuses a leaf whose claim overlaps the delegate's (its coherence
  assertion fails compilation), and that refusal is correct;
  *rejected:* narrowing `FrameworkKind::CLAIMED_KINDS` by a const filter plus a
  delegating newtype leaf — buildable, but a filtered claims list and a whole leaf
  to express one line of policy;
  *rejected:* a framework derive attribute for delegate interception — a new derive
  surface for a need one consumer line serves; revisit if a second consumer needs it;
  *rejected:* `DEBT_READ_THROUGH` — see its recorded meaning above.
- **The quoter's erasure leaves the quote, because the quote holds nothing of the
  quoter, 2026-08-29.** A framework user block carries no principal — the store's
  append takes only the conversation and the blocks — and the quote block stores a
  span reference and a voice, no handle, no author fact, none of the quoter's words.
  When the reply's author erases, the principal pass nulls their message's text and
  origin as today, and the quote block survives as an unattributed reference to text
  whose owner has not asked for erasure; if the target erases too, the resolution
  empties on its own. Pinned rather than assumed: after the quoter's erasure the
  message is nulled, the quote still resolves, and nothing anywhere ties the quote to
  the erased person. *Rejected:* deleting the quote on the quoter's erasure — a pass
  would have to FIND it, and the only route to it would be recording who appended it,
  which stores a new fact about the quoter for the sole purpose of erasing it.
- **The reply-target column and the quote block are two facts, not one decision twice,
  2026-08-28.** The column stores the PLATFORM fact (which origin was replied to) and
  serves waking, reports and erasure's naming pass. The quote block is derived projection
  content: it carries no origin, only a span. Erasure nulls the column and empties the
  resolution independently. Recorded because the duplication reviewer must be answered
  rather than dodged: the two records answer different questions and neither can be
  derived from the other at its read site.
- **A reply to the assistant lands quoteless in this unit, 2026-08-29.** The adapter
  discards the platform id of her messages by recorded decision (2026-08-23, "no
  origin rides it"), nothing stores which platform message any of her blocks became,
  and so no stored fact identifies WHICH of her `text` blocks a reply answers.
  Inventing a selection rule (newest assistant block, nearest answer) reproduces the
  exact misattribution this unit exists to kill, and carrying her platform ids is a
  new stored fact with privacy-document movement — its own unit, if the operator wants
  it. She still wakes exactly as today (that decision is made at translation from the
  live message). Her blocks stay quotable natively through `block_text` the day a
  reference can be built. *Rejected:* quoting her via a guessed selection rule;
  *rejected:* storing her message ids as a side effect of this unit.
- **Ordering within the ingest, and the crash window, named, 2026-08-28.** The quote
  lands through the framework's public user-block append, then the chat message through
  the consumer append, both INSIDE the ingest's stamp-locked stretch — one feeder runs
  per process and the lock serializes ingestions, so no other INGESTION slides a
  block in between; the quote append must be placed inside that stretch, not before
  it. A framework turn-close finishing inside that narrow window can still write
  between the two — harmless: the quote still precedes its own message, which is
  the invariant the resolution and the projection need. Delivery is
  at-least-once — a halted step is redelivered, and no origin dedupe exists in the
  ingest — so a crash between the two appends means the retried update would land its
  quote AGAIN before its message: the append therefore skips the quote when the
  conversation's last block is already a quote of the same span, which is exactly the
  crash-retry signature and one read of the tail. A fully redelivered update doubling
  its chat message is today's behavior, unchanged and out of scope here; the quote
  rides whatever the message does. And because duplicate origins can exist for that
  reason, the origin-to-block resolution picks the NEWEST matching chat-message block
  — the latest stored version of that origin is what the member replied to.
  *Rejected:* claiming an idempotency model bounds the window — none exists in either
  tree, and the earlier revision cited one; the skip rule above is what actually
  closes the doubling. Accepted and stated; making the two atomic would
  need a mixed framework-and-consumer append the store deliberately does not have.
- **The date-marker interaction is known and harmless, 2026-08-28; restated
  2026-08-29 for the day boundary.** Each append runs the date-marker seam
  independently, so on the same day the pair inserts at most one marker, ordered
  before the quote — and across a midnight boundary a second marker may land BETWEEN
  the quote and its message. Both shapes are harmless to the projection, and the
  tail-skip survives the seam because a marker and its quote commit in one
  transaction: after a crash between the appends the tail is always the quote, never
  a bare marker. Nothing to build; stated so the reviewer finds it decided.
- **Erasure parity across the refresh fork, 2026-08-29 — the pin slice 14 assigned
  to this consumer, anchored on what the walk actually does.** The startup walk that
  moves channels onto an edited prompt or a swapped model forks with
  `fork_conversation`, which SHARES blocks — it copies junction rows and clones
  nothing (the framework's own pin is named `fork_conversation_shares_blocks`). The
  deep-copy machinery with detached clones exists only in `fork_continuation`'s
  new-thread arm, which no consumer code calls. Parity therefore holds because source
  and fork read the SAME chat-message row, and `erase_principal_content` sweeps that
  table by principal, junction-free; this unit pins it rather than assuming it: fork a
  conversation holding a quote of a member's message through the refresh walk, erase
  the member, and both the source's and the fork's projections must render no quoted
  text. *Rejected:* pinning detached-clone erasure — no consumer path creates a
  detached clone, and a pin of an unreachable state proves nothing about this
  product.
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
chat-message descriptor declares its quotable text column. A reply to the assistant
lands quoteless — no stored fact identifies which of her blocks was answered, and
nothing is guessed. The reply-target column, waking, reports and erasure
are unchanged. A quote settles no debt and draws no turn of its own: the debt walk
reads through it, and only the chat message's stamp asks the model to speak. No new stored fact beyond the quote blocks themselves, no new dependency,
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
- **AC6** A reply to the assistant lands quoteless and her waking behaviour is
  unchanged — pinned; the quoteless landing is this unit's recorded decision, not an
  accident.
- **AC7** The ingest still answers: the quote path adds no new refusal and no silence —
  the reply's turn happens exactly as it would have, pinned in both answering modes.
- **AC8** The descriptor declaration is the one required by slice 14 and is validated at
  open — pinned by the framework's own open-time check, driven from this workspace.
- **AC9** The manual-quote fact survives the adapter thread: a decoded update
  carrying the platform's quote field reaches the core's inbound message with its
  text and manual flag intact. Two pins, because the existing struct-built seam
  convention cannot catch a decoder regression: one RAW-JSON decode through serde
  proving the message-level quote field parses (a new, stated convention for the
  decoder), and one at the translate/intake seam proving the fact is carried and
  copied.
- **AC10** Quoter-side erasure holds the recorded decision: after the reply author's
  erasure their message is nulled as today, the quote block survives, still resolves
  the target's text, and stores no fact tying it to the erased person — pinned by
  running the real principal erasure.
- **AC11** The crash state keeps its debt: with a quote appended, its message
  withheld (the crash shape), and an unanswered member message behind the quote,
  the retried message's landing still opens the debt the tail owed — one pin
  driving the debt walk's read-through and the tail-skip together — and a lone
  quote at the tail draws no model turn.
- **AC12** Erasure parity across the refresh fork: after a conversation holding a
  quote of a member's message forks through the startup refresh walk, erasing that
  member leaves BOTH the source's and the fork's projections without the quoted text —
  pinned by running the real fork and the real erasure against the shared row.

## Notes for launch

- Unblocked: slice 14 merged to the framework master 2026-08-29 (`f2bf250`), and this
  workspace path-depends on that master. The branch sits on the consumer `main`
  tip of its launch day (unit 29's merge included); the build's first step rebases
  again regardless.
- The two crash-and-fork pins drive public surfaces: AC11's crash shape is built
  with `Store::insert_user_blocks` on the spine fixture's store handle (the date
  marker commits in the same transaction, as decided above), and AC12's fork runs
  `Assistant::retire_stale_channels` (`core/src/assembly.rs:949`, public).
- Worktree `~/projects/halogenos-assistant-quotes`, branch `unit/inbound-quotes`. Sites: the ingest edge in `core/src/assembly.rs` (the append
  sequence, around the existing chat-message landing), the new origin-to-block
  resolution (no existing lookup maps an origin to a block id; the erasure passes
  around `core/src/kind.rs:692-828` only NULL by origin), the manual-quote thread
  through `adapters/telegram/src/client.rs`, `translate.rs`, `driver.rs` and
  `core/src/message.rs`, the debt walk and agency surfaces (`NEVER_ANSWERABLE` in
  `core/src/kind.rs` and the assistant kind tree's new quote arm), the chat-message
  descriptor declaring the quotable column, and the spine and adapter tests.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell." If the ingest edge needs a bolted-on conditional
  to fit the quote in, the structure is wrong — restructure the landing sequence until
  the quote is a natural step.
