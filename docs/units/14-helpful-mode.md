# Unit 14 — helpful mode, the name, and the configurable disclosure

Date: 2026-08-23. Revision 2, rewritten after the cold probe found the design's
real shape: every downstream mechanism — the unlatch that dispatches a turn,
the per-person and per-channel budgets, the disclosure's once-per-person fold,
the flood counting — keys on a message having OPENED A DEBT (addressed and not
limited). Revision 1 tried to make helpful mode summon turns by a separate
mode check beside addressing, which left all of those keyed on the old
predicate and broke each one. Revision 2 makes helpful mode change the ONE
thing they all read: an unlimited group message opens an answer debt exactly as
an addressed one does, and flows through every existing mechanism unchanged.
The only genuinely new pieces are the abstention sentinel and the mode gate.

## Decisions taken with this unit

- **Helpful mode makes an unaddressed question open a debt, 2026-08-23.** The
  configuration key `answering` takes `helpful` (default) or `addressed`. The
  debt-opening predicate — today `addressed && !limited` (`Stamp::compose`) —
  becomes `(addressed || helpful_mode) && !limited` for a group message. That
  single change routes an unaddressed group message through the whole
  machinery as if it had been addressed: it takes the answer budget at
  ingestion (so a rate-limited member's message is stamped limited and opens
  no turn — the zero-cost quiet, now actually reached because the budget is
  consulted); it emits the unlatch (so a fresh or error-latched conversation
  dispatches it, not just addressed traffic); it becomes a co-summoner (so the
  disclosure's once-per-person fold works); and it counts toward the channel
  budget (so the flood of unaddressed messages is bounded by the same per-
  channel bound that bounds addressed answering). `addressed` keeps its
  literal meaning — the adapter still records whether the user addressed the
  assistant — and every path that genuinely needs "did the user address me"
  (the report tool, the name trigger) reads the flag, not the debt. Rejected:
  a separate helpful-turn trigger beside the debt (revision 1 — breaks unlatch,
  budget, disclosure and counting, each proven by the probe); making
  `addressed` itself true for unaddressed helpful messages (it would corrupt
  the report path and the disclosure's meaning of "addressed").
- **The channel budget is the flood bound and the cost bound, 2026-08-23.** In
  helpful mode every unlimited group message opens a debt and so takes a
  channel-budget slot at ingestion, before the model runs; when the channel
  budget is spent the message is stamped limited and opens no turn — no model
  call. So the per-channel budget bounds model CALLS in helpful mode, which is
  exactly the flood protection an adversarial burst needs, and the operator
  sizes it. Recorded as the operator's accepted economics: at community
  traffic the window is generous and every real question is answered; a burst
  beyond it degrades to silence, not to unbounded spend. The per-person budget
  bounds one member the same way. Rejected: an unbounded call rate (the probe's
  flood surface); a separate call budget distinct from the answer budget (one
  bound, reused — the debt IS the call in helpful mode).
- **The model abstains through a fixed sentinel, recognized before anything
  else, 2026-08-23.** The prompt teaches: answer only when you can genuinely
  help — a real question you can answer, or one that otherwise warrants a
  reply; stay silent for members talking among themselves, for anything you
  have no information on, and when a lookup returns empty rather than guessing.
  To stay silent the model emits the fixed abstention sentinel (a named
  constant) as its ENTIRE answer. The outbound edge recognizes the sentinel on
  the RAW finalized answer content, BEFORE the disclosure prepend and before
  any delivery: a recognized abstention delivers nothing, prepends no
  disclosure line, and introduces nobody (so the once-per-person disclosure is
  untouched — an abstained answer is not a first answer). The sentinel must be
  the whole answer; an answer containing the sentinel's words as prose is a
  normal answer and is delivered. Recognition is exact on the trimmed content.
  Rejected: recognizing after the disclosure prepend (would mutate the
  swallowed block and mark the asker introduced — the probe's hole); a tool
  call to abstain (a round trip for silence).
- **The abstention block is not projected as the assistant's speech,
  2026-08-23.** A stored sentinel would otherwise reach the model on the next
  turn as its own prior message. The abstention is recorded as a turn-closure
  fact, not an assistant text block the projection reads: the answer block
  carries the sentinel but projects as nothing (the projection skips a
  recognized abstention, the way it skips other non-spoken kinds), so the
  model never sees a wall of its own past sentinels. The ledger still holds the
  turn's closure (the turn happened, the read was spent) — only the empty
  content is kept out of the model's history. Rejected: not storing the turn
  (the closure is a real fact the budget accounting and the actor rely on);
  projecting the sentinel verbatim (pollutes the context and teaches the model
  to abstain by imitation).
- **Mid-turn questions are already handled by absorption, 2026-08-23.** A
  question arriving while a turn's tool call runs is absorbed into that turn
  (the dispatch-identity mechanism) and reaches the model when the turn
  resolves, so the model sees whether a member already answered it and can
  abstain or defer. Helpful mode inherits this unchanged; recorded because the
  operator named it a requirement.
- **The name is one configuration key with three effects, resolved at startup,
  2026-08-23.** The `name` key sets the identity the prompt teaches, the
  default fill for the disclosure line, and — in `addressed` mode only — an
  additional wake trigger beside the mention and the reply. Unset, it defaults
  to the assistant's platform display name, which the binary fetches once at
  startup (the platform exposes it beside the id and username; the adapter
  reads it and hands it to the assembly as ordinary startup data, no core
  platform vocabulary, no adapter behavior — the same shape the operator id
  already crosses). The name reaches the prompt and the disclosure through the
  assembly config; the trigger word reaches the adapter as one translated
  configuration value. The trigger match is whole-word and case-insensitive
  over the message text; a name that is not a single clean word (contains
  whitespace or non-word characters) is not used as a trigger and the assistant
  falls back to mention-and-reply, logged — the mention and reply always work.
  Rejected: hardcoding the name; resolving the display-name default at config
  load (the platform value is not known until the startup fetch, so the key is
  validated at load and the default filled at startup).
- **The disclosure line is configurable, with a single composition rule,
  2026-08-23.** The `disclosure` key overrides the first-interaction line.
  Unset, the line is composed from the resolved name by a fixed template
  (`Hi, I'm <name>, ...`); the shipped default remains the operator's exact
  Xenia copy when the name resolves to Xenia, and follows the name otherwise —
  one rule, no ambiguity, and the docs test pins the template against the
  resolved name rather than a fixed spelling. The unit-12 mechanism is
  unchanged — stored into the first spoken answer, per person, mechanical;
  only the text is a value, and it is never empty (unset means the composed
  default). Rejected: dropping the line when unset (the Act duty is not
  optional).

## The unit's contract

Three optional keys — `answering` (closed enum helpful|addressed, decoding is
validation), `name` (trimmed, empty rejected), `disclosure` (trimmed, empty
rejected) — refused-unknown-keys. The debt-opening predicate gains the mode:
an unlimited group message opens a debt when addressed OR the mode is helpful,
routed through the existing unlatch, budget, limited-stamp, co-summoner and
channel-counting unchanged. The abstention sentinel constant; its exact
recognition on raw finalized content at the outbound edge, before the
disclosure prepend, delivering nothing and introducing nobody; its projection
skip so the model never reads it. The startup display-name fetch and its flow
to the assembly; the name's three effects; the trigger-word predicate in the
adapter for addressed mode. The disclosure template composed from the name.
The prompt's helpful/abstain teaching and its name identity. The compliance
page notes the disclosure holds under every mode (the first SPOKEN answer
carries the line, and helpful answers are spoken answers that open real
debts). The policy's processing description gains one sentence: the assistant
reads group messages to offer help, under the same legitimate interest.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency; a previous-unit store
  upgrades cleanly (schema unchanged — verify; the mode is behavioral, old
  unaddressed rows keep their stamp and are not retroactively answered, stated).
- **AC2** Helpful mode, the debt spine: an unaddressed group question opens a
  debt, summons a turn on a FRESH (boot-latched) conversation and on an
  ERROR-latched one (the unlatch fires), and the answer reaches the chat; a
  rate-limited member's unaddressed message is stamped limited and opens NO
  turn (zero model call); the channel budget bounds calls (an over-budget
  channel's next unaddressed message opens no turn) — pinned end to end.
- **AC3** The once-per-person disclosure survives helpful mode: the first
  spoken answer to an unaddressed new person carries the line, their SECOND
  spoken answer does not, a returning deleted person gets it again — pinned
  (the hole revision 1 would have shipped).
- **AC4** Addressed mode: an unaddressed message summons no turn; a mention, a
  reply, and a name-mention each summon one — pinned over the wire, the
  name-trigger whole-word and case-insensitive, a non-word name falling back to
  mention-and-reply.
- **AC5** The abstention sentinel: recognized exactly on the raw content,
  delivered as nothing, the turn closed, no disclosure prepended, nobody
  introduced, no window slot beyond the debt already opened; an answer with the
  sentinel's words as prose is delivered; a stored abstention does not project
  into the next turn's model history — pinned.
- **AC6** The name and disclosure: the configured name reaches the prompt
  identity and the disclosure template; the startup display-name default
  applies when unset; the configured disclosure overrides; the composed line is
  never empty — pinned including the docs test for the compliance note and the
  policy sentence.
- **AC7** Absorption under helpful mode: a question absorbed into a running
  turn reaches the model and a member's intervening answer is visible to it —
  pinned.
