# Unit 14 — helpful mode, the name, and the configurable disclosure

Date: 2026-08-23. Revision 2. The operator's design across several messages: the
assistant should help with a question asked into the group even when nobody
mentioned it, abstaining freely when it has nothing to add; its name and its
disclosure line should be configurable; and it should also wake to its name.
The economics are the operator's stated call — a cheap cached read per message,
acceptable at the community's traffic — so helpful behavior is the default and
the quiet "addressed only" mode is the option.

Revision 2 (2026-08-23) recasts the mechanism onto the debt spine, changing
no decision: an unlimited group message OPENS A DEBT when it is addressed or
the mode is helpful — the summons resolved once at the entry point and
stamped at the write, never a second trigger beside the addressed one. The
stamp's readers — the unlatch emission, the budget counts, the co-summoner
rule, the disclosure fold, mid-turn absorption — then work unchanged and
mode-free, which is the recast's whole point.

## Decisions taken with this unit

- **Answering is a mode, helpful by default, 2026-08-23.** A configuration key
  `answering` takes `helpful` (default) or `addressed`. In `addressed` mode a
  group message summons a turn only when it addresses the assistant — the
  current behavior. In `helpful` mode every group message summons a turn, and
  the model decides whether to speak. The economics are recorded as the
  operator's: with prompt caching the marginal read is cheap and the
  community's volume makes it acceptable; a deployment that wants the quiet
  shape sets `addressed`. Rejected: helpful with no off switch (a different
  community may want quiet); a per-message heuristic in the core deciding
  answerability (that judgment is the model's, and a keyword gate would both
  miss real questions and waste the model's own abstain).
- **The model abstains through a fixed sentinel, swallowed before the chat,
  2026-08-23.** The prompt teaches: answer only when you can genuinely help —
  a real question you can answer from the sources or your knowledge, or one
  that otherwise warrants a reply; stay silent for members talking among
  themselves, for anything you have no information on, and when a lookup comes
  back empty rather than guessing. When the model chooses silence it emits the
  fixed abstention sentinel (a named constant) as its whole answer; the
  outbound edge recognizes the sentinel and delivers nothing, records the turn
  closed. The turn still counts against the model-call economics (the read
  happened) but speaks nothing and — crucially — an abstained turn spends no
  answer-window slot, since the window bounds what the assistant SAYS.
  Rejected: a tool the model calls to abstain (a round trip for silence); a
  confidence threshold in the core (the model's judgment, not a number).
- **The per-person budget is the free quiet, checked before the model runs,
  2026-08-23.** A member over their answer-window budget already has their
  message stamped limited at ingestion, which opens no turn — so in helpful
  mode a rate-limited member's message costs ZERO: no model call, no read,
  silent by the existing mechanism. Helpful mode changes only the UNLIMITED
  message's path (address-independent turn); the limited path is untouched.
  This is the output-cost limiter the operator named, already built.
- **Mid-turn questions are already handled by absorption, 2026-08-23.** A
  question arriving while a turn's tool call runs is absorbed into that turn
  (the dispatch-identity mechanism) and reaches the model when the turn
  resolves, so the model sees whether a member already answered it and can
  abstain or defer to them. Helpful mode inherits this with no new work; the
  spec records it because the operator named it as a requirement.
- **The name is one configuration key with three effects, 2026-08-23.** A
  `name` key (defaulting to the platform display name the assistant reads from
  the platform at startup, an explicit value overriding) sets: the identity
  the prompt teaches (so the model knows what it is called and answers the
  are-you-a-bot question about that name), the default fill for the disclosure
  line, and — in `addressed` mode only — an additional wake trigger: a group
  message naming the assistant addresses it, beside the mention and the reply.
  The name match is whole-word and case-insensitive, translated in the adapter
  beside the mention check (the adapter already owns addressing translation);
  a name with characters that cannot form a clean trigger word falls back to
  mention-and-reply only, logged. In `helpful` mode the name-trigger is moot
  (every message is evaluated) but the prompt identity and disclosure fill
  still apply. Rejected: hardcoding the name (the whole point is
  configurability); the display name as the trigger without an override
  (display names punctuate badly).
- **The disclosure line is configurable, 2026-08-23.** The `disclosure` key
  overrides the fixed first-interaction line; unset, it is composed from the
  name (`I am <name>, an AI assistant. ...` or the operator's stored default).
  The mechanism from unit 12 is unchanged — stored into the first answer,
  per person, mechanical; only the text is now a value. Rejected: dropping the
  line when unset (the Act duty is not optional; unset means the default text,
  never no text).

## The unit's contract

The `answering` mode key, the `name` key, the `disclosure` key — all optional,
refused-unknown-keys, validated (empty rejected, name trimmed). The helpful
debt-opening in the core keyed on the mode at exactly one place: an unlimited
group message opens a debt when addressed or when the mode is helpful, the
summons stamped at the write (revision 2). The abstention
sentinel constant and its recognition at the outbound edge, delivering nothing
and spending no window; the recognized abstention kept out of the projection.
The prompt's helpful/abstain teaching and its name
identity, composed at assembly from the config. The adapter's name-trigger in
`addressed` mode, whole-word case-insensitive, beside the mention. The
disclosure fill from the name. Decisions recorded; the compliance page notes
that the disclosure duty holds under every mode (the first spoken answer still
carries the line); the policy's processing description gains one sentence that
the assistant reads group messages to offer help, under the same basis.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency; a previous-unit store
  upgrades cleanly (no schema change expected — verify).
- **AC2** Helpful mode: an unaddressed group question summons a turn and the
  answer reaches the chat (carrying the first-interaction line for a new
  person); an unaddressed message the model abstains on delivers nothing and
  spends no answer-window slot; a rate-limited member's message opens no turn
  at all (zero model call) — pinned end to end.
- **AC3** Addressed mode: an unaddressed message summons no turn; a mention, a
  reply, and a name-mention each summon one — pinned over the wire, the
  name-trigger whole-word and case-insensitive, a punctuated name falling back.
- **AC4** The abstention sentinel: recognized exactly, delivered as nothing,
  the turn closed, the window unspent; a normal answer containing the
  sentinel's words as prose is NOT swallowed (the sentinel is the whole answer
  or nothing) — pinned.
- **AC5** The name and disclosure: the configured name reaches the prompt
  identity and the disclosure fill; the display-name default applies when
  unset; the configured disclosure overrides; unset composes from the name and
  is never empty — pinned including the docs test for the compliance note.
- **AC6** Absorption under helpful mode: a question absorbed into a running
  turn reaches the model and a member's intervening answer is visible to it —
  pinned.
