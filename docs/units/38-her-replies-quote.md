# Unit 38 — a reply to the assistant quotes her words

Date: 2026-08-30. Unit 31 landed replies as quotes but left the assistant's side
quoteless: nothing stored which platform message any of her blocks became, so a reply
to her could not be matched to the message it answers. The operator's order,
2026-08-29, verbatim: "Why not? Please fix it" — her sent messages record their ids,
and replies to her quote her words like everything else. Her own message ids are
nobody's personal data (the operator, same day): no privacy document changes, and the
reviewers only confirm no published sentence contradicts that.

## Grounding

**The send path sees the id today and drops it.** The Bot API's answer to a send IS
the sent message with its id; `send_body` (`adapters/telegram/src/client.rs:654-676`)
decodes it into `let _sent` and returns `Ok(())`. Both send paths discard it — the
reply consumer (`driver.rs:868-896` through `send_message`/`send_chunk_threaded`/
`send_chunk`) and the deterministic items (`send_item`, `driver.rs:739-743`). One
answer may become several platform messages (`chunks_within_cap`, `client.rs:814-831`;
only the first chunk threads, decision 0019).

**No one knows her block's id at write time; the outbound edge knows it at delivery
time.** The framework finalizes her answer blocks at four sites, each discarding the
returned block id. But the consumer's outbound edge iterates loaded blocks WITH ids
(`core/src/outbound.rs:342-346`) when it builds `OutboundReply` (`message.rs:483-503`)
— and the disclosure line is written into the stored block BEFORE the send (decision
0079), so stored text equals sent text, which is what makes quoting her stored block
honest.

**The seam is already designed, in a committed and unbuilt spec.** Unit T4
(`docs/units/telegram/04-deleting-messages.md`) designs the delivery receipt whole: a
core entry point in the shape of ingest and observe taking the handle and the reported
origins; the wire client returning the delivered ids instead of discarding them;
one `Delivered` block per platform message that actually reached the chat, holding the
origin and the delivery key, per chunk, across BOTH send paths; a new
`core/src/delivery.rs` owning the kinds and the bounded conversation-scoped queries;
two appended schema migrations. T4 also scheduled the vocabulary amendment this unit
performs: `ReplyTarget::AssistantMessage` gains `{ origin: Option<String> }` and its
doc loses the no-origin sentence, while `COLUMN_REPLY_TARGET`'s storage documentation
stays true and unedited — the variant's origin is never stored on the chat message.

**What T4's shape lacks for quoting, exactly one value.** A `Delivered` row maps
origin to delivery key, not to her answer's ledger BLOCK id — and a quote span's
endpoint is a block id. T4 declined that association because deterministic items have
no answer block. This unit adds it as a third, nullable value on the `Delivered` row,
filled for answer deliveries (the id is in hand at `outbound.rs:342-346`), absent for
notices, items and reports.

**Unit 31's machinery is target-agnostic past the lookup.** `land_reply_quote`
(`core/src/quoting.rs:152-181`) matches only `ReplyTarget::Message`; everything after
the resolution — the span decision, the manual-excerpt narrowing (already translated
for replies to her, `translate.rs:589-591`), the tail-skip, the user-voiced append —
reuses unchanged. Her blocks are natively quotable through `block_text` (slice 14's
single-block path), the quote draws no turn whichever block it references
(`NEVER_ANSWERABLE` and the awaiting override hold), and her wake is decided at
translation, untouched by any of this.

**The adapter already decodes what it needs.** `RepliedTo` carries `message_id`
(`client.rs:330-334`); `reply_target_of` (`translate.rs:571-579`) discards it in
translation only. No new wire field.

## Decisions taken with this unit

- **The delivery receipt is built to T4's recorded shape, answers carrying their
  block id, 2026-08-30.** The wire client returns the delivered ids; after each
  successful send the adapter reports the platform fact through a new core entry
  point beside ingest and observe; the core appends one `Delivered` block per
  delivered platform message — origin, delivery key, and the answer's block id where
  the delivery was an answer, NULL for deterministic items and notices — per chunk,
  both send paths, only for chunks that actually reached the chat. `OutboundReply`
  gains the answer's block id from its one construction site, NULL at the notice
  site. The kinds and the bounded conversation-scoped lookups live in a new
  `core/src/delivery.rs` per T4. *Rejected:* an answers-only subset seam — T4's
  coverage is total and building half of a recorded design leaves the other half
  colliding later; *rejected:* a side table — T4's own recorded rejection: blocks
  cascade with a deleted conversation and need no cleanup pass;
  *rejected:* building T4 whole here — the deletion mirror's consumption is its own
  unit and waits.
- **The origin rides the reply-target variant and is never stored on the chat
  message, 2026-08-30 — T4's scheduled amendment, performed here on the operator's
  order.** `ReplyTarget::AssistantMessage` gains `{ origin: Option<String> }`;
  `reply_target_of` fills it from the already-decoded id; the variant's doc loses the
  no-origin sentence; `stored_fields`' arm keeps storing NULL, so
  `COLUMN_REPLY_TARGET`'s documentation stays true unedited. The no-origin decision
  family (message.rs, kind.rs, translate.rs, mirror.rs, decision 0059) is amended in
  its RIDES half only; mirror.rs's `AssistantMessage => None` arm stays for T4 to
  hook. *Rejected:* storing her origin in the reply-target column — erasure's naming
  pass and the reports read that column as member-message references.
- **Her resolution is one lookup beside the member one, 2026-08-30.** A new
  conversation-scoped query in `delivery.rs`: origin string to the NEWEST delivered
  answer block id, junction-joined like every origin reader (platform ids are unique
  only per channel). `land_reply_quote` gains the `AssistantMessage { origin: Some }`
  arm calling it; `origin: None` (a reply to her from before this unit records ids,
  or a delivery that was never recorded) lands quoteless exactly as today — nothing
  invented. Multi-chunk answers: each chunk's `Delivered` row carries the same answer
  block id, so a reply to ANY chunk quotes the whole stored answer — the stored block
  is the one truth of what she said. *Rejected:* per-chunk span narrowing — the
  chunks are a transport artifact; her message is the block.
- **A reply to a deterministic item lands quoteless, stated, 2026-08-30.** Items and
  notices deliver with no quotable block (the notice is not stored; the report
  block's descriptor declares no quotable column), their `Delivered` rows carry NULL,
  and the resolution answers nothing. *Rejected:* quoting the report block — its
  descriptor decision is unit 29-era and not this unit's to reopen.
- **Unit 31's quoteless-her decision is superseded, its pin moves, 2026-08-30.** The
  operator's order overturns the recorded decision; AC6's pin
  (`a_reply_to_the_assistant_lands_quoteless_and_still_wakes_her`) becomes the
  quoted-her pin plus the still-wakes assertion unchanged; the translate pin on the
  bare variant moves with the widening. Unit 31's doc stays unedited — this record is
  the amendment, the one-voice rule's way.
- **No privacy-document change, on the operator's word, 2026-08-30.** Her own ids
  describe her software's messages, no person; review confirms no published sentence
  contradicts the new stored rows and changes nothing.

## The unit's contract

Every message the assistant successfully sends is recorded as a `Delivered` block —
origin, delivery key, and the answer block's id where one exists — through a core
entry point the adapter reports into, both send paths, per chunk. A member's reply to
one of her recorded messages lands, in ledger order, as a user-voiced quote of her
stored answer block before the member's chat message — whole answer by default, a
manual excerpt narrowed by the same first-occurrence search — and the model reads her
words `> `-prefixed above the member's. A reply to an unrecorded message of hers, to
a deterministic item, or to anything predating this unit lands quoteless exactly as
today. Her wake, the turn duty, the tail-skip, erasure and the debt walk are
untouched; the chat message still stores no reply-target for her. No new dependency,
no configuration, no privacy-document change.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under
  denied warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The receipt records: a delivered answer yields one `Delivered` block per
  platform message with origin, delivery key and the answer block id; a
  deterministic item's rows carry no answer id; a failed chunk yields no row — pinned
  through the adapter's scripted Bot API server and the core's entry point.
- **AC3** A reply to her quotes her words end to end: ingest a member reply carrying
  her recorded origin and the model-bound projection shows her stored answer
  `> `-prefixed above the member's message — pinned through the real projection fold,
  including the disclosure-bearing first answer (stored text equals sent text).
- **AC4** A manual excerpt of her message narrows to its first occurrence across a
  multibyte boundary; a drifted excerpt quotes her whole answer — both pinned.
- **AC5** A reply to a chunk of a multi-chunk answer quotes the whole stored answer —
  pinned with a scripted answer crossing the chunk cap.
- **AC6** Quoteless stays quoteless: a reply to her unrecorded (pre-unit) message, to
  a deterministic item, and to a notice each land exactly as today — pinned per case,
  and she still wakes on every reply to her.
- **AC7** The quote of her block neither answers nor asks: it settles no debt and
  draws no turn — pinned by the crash shape with her block as the target.
- **AC8** The variant widening changes no storage: her reply-target column stays
  NULL, erasure's naming pass and the report resolution see exactly what they saw —
  pinned by the existing suites passing untouched plus one explicit NULL assertion.
- **AC9** The decision records land numbered after unit 37's, each dated, each with
  rejected alternatives; T4's spec is NOT edited (its build refreshes it).

## Notes for launch

- Worktree `~/projects/halogenos-assistant-herquotes`, branch `unit/her-reply-quotes`,
  from `main` (`cf26e9a`, unit 31 merged). The build's first step is
  `git rebase main` — unit 37 (OS info) may merge first.
- Sites: `adapters/telegram/src/client.rs` (send family returns delivered ids),
  `driver.rs` (both send-path consumers report), `translate.rs` (variant fill + pin),
  new `core/src/delivery.rs` (kinds, migrations appended per T4's shape, lookups),
  `core/src/outbound.rs` + `message.rs` (`OutboundReply` block id;
  `ReplyTarget::AssistantMessage { origin }`), `core/src/quoting.rs` (the new arm),
  `core/src/kind.rs` (delivered kind registration in the assistant kind tree —
  agency-inert, frontier-transparent, the join-notice precedent), spine and adapter
  tests, `docs/decisions`.
- T4 (`docs/units/telegram/04-deleting-messages.md`) is the seam's recorded design:
  the implementer reads it alongside this spec; where the two state the same
  mechanism, T4's wording of the Delivered shape governs, and this unit's additions
  are the block-id value and the quoting consumption.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell."
