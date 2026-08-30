# Unit 42 — a bot triggers nothing without a mention

Date: 2026-08-30. The operator's instruction, verbatim, after the assistant welcomed a
joiner in the live group when the moderation bot's captcha prompt drew a turn: "The bot
shouldnt react like this. Not when rose announces and not on join messages. The joins
are for moderation only." — and, deciding the fix's shape after two other options were
offered: "Neither just implement that bots can't trigger this bot at all unless they
@mention our bot." So: exactly one rule, in code — a message from a bot account never
opens a turn for the assistant, unless that message @mentions the assistant.

## Grounding

**How the incident happened.** In helpful mode every message summons
(`resolved_summons`, `crates/core/src/assembly.rs:1586-1591`: summoned = addressed OR
Helpful). The moderation bot's captcha prompt is an ordinary inbound message from a bot
member, so it summoned a turn; the join notice sat in context; the model produced a
welcome despite a taught sentence forbidding it — the teaching was probabilistic where
a mechanism is not.

**What the wire carries today.** The adapter's translation reads the platform sender
(`from`) and builds `SenderIdentity { external_id, username }`
(`crates/adapters/telegram/src/translate.rs:197,222-226`;
`crates/core/src/message.rs:73-78`); the platform's sender object states whether the
account is a bot, and the translation currently drops that fact. The addressed flag
for a group message is a union of three forms: an @mention of the assistant's handle,
a reply to the assistant, or the configured wake name
(`translate.rs:206-213`), pinned with byte-level cases at `translate.rs:752-774`. A
join report's `by:` site builds a `SenderIdentity` too (`translate.rs:253`).

**What is decided before summoning.** The deletion mirror is decided at
`assembly.rs:788-792`, before the summons resolution at `:794`, and takes
`LimitedBy::Command` regardless of summoning — the moderation bot's `/del`
bookkeeping never depends on whether anything summons. A non-summoned helpful-mode
message is recorded as history and opens no debt of its own; the counted-debt
machinery counts summoned, un-limited debts only
(`crates/core/src/kind.rs:1069-1081`, `COLUMN_ADDRESSED` storing the summons at
`kind.rs:339,485`).

**Turn-opening is more than the summons.** `answer_due` composes as own debt OR the
conversation's owing tail (`kind.rs:342`), the tail is read before the summons
resolution (`assembly.rs:793`), and the watcher fires a turn for any newest block
with `answer_due` (`kind.rs:669`). So a message that summons nothing can still open
a turn by CARRYING an earlier message's unanswered debt — without a rule for it, a
bot's message appended while the tail owes would open the exact turn the operator
forbade. This unit decides that case below.

**The wire structs do not decode the bot fact today.** The adapter's own serde
types `User` (`client.rs:360-363`) and `Joiner` (`client.rs:247-253`) carry id and
username only and skip unknown keys; the platform sends `is_bot` on both. There are
THREE production sites building a `SenderIdentity`: the message path
(`translate.rs:222`), the membership observation of the assistant's own entry
(`translate.rs:253`), and the join report's per-joiner identity
(`joined_member`, `translate.rs:315`).

## Decisions taken with this unit

- **The wire states whether the sender is a bot, and states it nowhere else,
  2026-08-30.** `SenderIdentity` gains a `bot: bool`, filled by the adapter from
  the platform sender's own bot fact; the adapter's `User` and `Joiner` decode
  structs gain `is_bot` (absent decodes as false). All three construction sites
  fill it from their own sender's fact — the message path, the membership
  observation, and the join report's per-joiner identity, where the fact is the
  JOINER's own flag. The fact is consumed at the adapter's addressing and the
  core's summons resolution and STORED NOWHERE: no schema column, no migration, no
  erasure change — it is a property of the account read fresh off every update,
  platform-neutral (every platform this assistant will meet marks automated
  accounts or leaves the flag false). *Rejected:* a field on the message — the
  fact belongs to the account, not to one message of it; *rejected:* persisting it
  — nothing reads it after the stamp, and a stored copy would only drift from the
  account's current state.
- **For a bot sender, only an @mention addresses the assistant, 2026-08-30.** In the
  adapter, where the platform's addressing forms are translated: a bot sender's
  group message is addressed if and only if `mentions_bot` holds — a reply to the
  assistant does not address it, and the wake name does not address it. The operator
  named the @mention as the one opening; a bot replying to the assistant or speaking
  its name stays un-triggering. Non-bot senders keep the three-form union untouched.
  This decision lives in the adapter because which platform forms count as
  addressing is translation, exactly where the three forms live today.
  *Rejected:* keeping the union for bots — a moderation bot that quotes or replies
  to the assistant's message would re-open the exact hole being closed.
- **A bot sender is never summoned by mode, only by address, 2026-08-30.** In the
  core's summons resolution: a message whose sender is a bot is summoned if and only
  if it is addressed; the helpful-mode clause applies to non-bot senders alone.
  Recorded history is untouched — the bot's messages land in context exactly as
  today, they open no turn and no debt of their own. Direct channels are unaffected
  in practice (the platform does not deliver bot-to-bot private messages) and the
  rule still reads coherently there: a direct message is addressed by definition.
  *Rejected:* filtering bot messages out of ingestion — the model must keep seeing
  them (the deletion mirror and the group's visible history depend on it), they must
  merely trigger nothing.
- **An unsummoned bot message never carries the owing tail, 2026-08-30.** A turn
  also opens when a new message CARRIES an earlier unanswered debt
  (`answer_due` = own debt OR the owing tail, `kind.rs:342`), so without this rule
  a bot's plain message appended while the tail owes would open the forbidden turn
  with someone else's stale debt. Decided: for a bot sender's unsummoned message,
  `answer_due` is false outright — it neither takes debt nor carries the tail. The
  owed tail is NOT lost: it stays owed, and the next message that may carry it (any
  non-bot message, or a bot message with the mention) opens the turn with the debt
  intact. *Rejected:* letting the tail ride on bot messages — it is exactly "a bot
  triggers this bot" wearing another message's debt; *rejected:* answering the owed
  debt eagerly on a timer — a mechanism this unit has no order for.
- **Everything decided before the summons stays exactly as built, 2026-08-30.** The
  deletion mirror, command recognition, identity resolution and recording are
  untouched by construction: they run before or independently of the summons
  resolution. Stated so the reviewers hold the diff to it.

## Acceptance criteria

- **AC1 — the wire fact.** The `User` and `Joiner` decode structs carry `is_bot`
  (absent decodes false); `SenderIdentity` carries `bot`, filled at all three
  building sites from their own sender's fact (the joiner site from the joiner's
  flag); a bot sender's message delivers it true, a human's false (pins). The fact
  is stored nowhere: no schema change (the schema pins pass untouched).
- **AC2 — addressing for bots narrows to the mention.** Adapter pins beside the
  existing addressed cases: a bot's group message with the assistant's @mention is
  addressed; a bot's reply to the assistant is not; a bot's message speaking the
  wake name is not; the existing addressing pin set (`translate.rs:752-835`:
  direct, mention, command form, reply, wake name) passes with no behavioral
  change.
- **AC3 — no turn from an unmentioned bot.** In helpful mode, a bot sender's plain
  group message is recorded, takes no debt of its own, opens no turn, and is
  excluded from both budget counts; the same message carrying the @mention summons
  a turn (pins on both).
- **AC3b — the tail waits for a legitimate carrier.** With a conversation's tail
  owing (a summoned human message whose turn produced nothing), a bot's plain
  message opens no turn and the tail stays owed; the next human message opens the
  turn with the owed debt intact (pin constructing the whole sequence).
- **AC4 — the mirror is untouched.** The moderation bot's `/del` mirrors exactly as
  before (existing pins pass); a `/del` carrying the assistant's mention still
  mirrors and takes its command stamp (pin).
- **AC5 — nobody else moves behaviorally.** Mechanical fixture edits filling the
  new field are expected wherever a `SenderIdentity` is constructed; beyond them,
  the existing summons, addressing, budget and teaching pins pass with no
  behavioral change.
- **AC6 — the checks.** fmt, clippy with warnings denied, the full suite, the doc
  build, exit codes read bare.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-service`, branch `unit/service-quiet`,
  from `main` (`9ecd0b1`). The build's first step: `git rebase main`.
- Decision records number from the highest shipped at merge; expected records: the
  wire bot fact, the mention-only addressing for bots, the no-mode-summons rule.
- Deploy-relevant: the live assistant welcomed a joiner today; this unit rides the
  next deploy the operator approves.
