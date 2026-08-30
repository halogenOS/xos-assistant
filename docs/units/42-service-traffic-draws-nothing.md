# Unit 42 — service traffic draws nothing

Date: 2026-08-30. The operator's instruction, verbatim, after the assistant welcomed a
joiner ("New member joining, welcome!") in the live group: "The bot shouldnt react like
this. Not when rose announces and not on join messages. The joins are for moderation
only." And, on the moderation bot's announcements: "Btw rose's announcements get
deleted after a bit." So: the moderation bot's messages and the join notices draw no
response from the assistant — not a welcome, not a comment, not an answer — while the
moderation assessment of joins stays exactly as built.

## Grounding

**How the incident happened.** In helpful mode every message summons
(`resolved_summons`, `crates/core/src/assembly.rs:1586-1591`: summoned = addressed OR
Helpful). The moderation bot's captcha prompt to the joiner is an ordinary inbound
message from a bot member, so it summoned a turn; the join notice sat in context; the
model produced a welcome. The join rule that forbids exactly this ("you never ban,
kick, or reply to the joiner, and a join you do not report needs no comment") exists
but rides inside `MODERATION_TEACHING` (`crates/core/src/teaching.rs:74-96`), and a
taught sentence is probabilistic where a mechanism is not.

**What the core already knows.** The moderation bot is configuration: the assembly
holds `moderation_handle: Option<String>` (`assembly.rs:285,409-441`; prod sets the
handle). The inbound sender carries `username: Option<String>`
(`SenderIdentity`, `crates/core/src/message.rs:73-78`). The deletion mirror is decided
before and independently of summoning (`assembly.rs:788-801`): a `/del` from the
moderation bot takes `LimitedBy::Command` whether or not anything summons. A
non-summoned message in helpful mode is recorded as history and opens no debt — the
counted-debt machinery counts summoned, un-limited debts only
(`crates/core/src/kind.rs:1069-1081`).

**Join notices wake nothing by themselves** (unit 36's recorded design): they project
into context with a bracketed id for the report tool, and the report-on-sight teaching
composes only under `moderation_taught` (`teaching.rs:50,155-157`) — both of that
function's conditions held in the incident deployment, so the teaching alone is proven
insufficient for the no-welcome half.

**A teaching pin will invert.** `teaching.rs:531-532` pins that a prompt WITHOUT the
moderation teaching "teaches no join rule either". This unit moves the no-comment half
of the join rule out of that gating, so the pin's claim changes deliberately.

## Decisions taken with this unit

- **The configured moderation bot's messages never summon, structurally,
  2026-08-30.** When `moderation_handle` is configured and the inbound sender's
  username matches it (compared the way the adapter already compares the assistant's
  own handle, case-insensitively), the message is recorded as history exactly as
  today and summons nothing — in every answering mode, addressed or not. Nobody
  legitimate speaks to the assistant through the moderation bot's mouth, and a
  member-crafted mention smuggled into one of its service messages must not open a
  turn. The deletion mirror is untouched by construction: it is decided before the
  summons and keyed on the command, so `/del` bookkeeping continues exactly as
  built. With no `moderation_handle` configured nothing changes anywhere.
  *Rejected:* teaching alone — the violated sentence already existed; a mechanism
  cannot be talked out of. *Rejected:* excluding all bot senders — the wire does not
  carry a bot flag today, and the operator named the moderation bot; a general
  bot-sender rule is its own decision when a case for it exists.
- **The join no-comment rule is taught wherever joins are seen, not only where
  reporting is, 2026-08-30.** The join teaching splits along its two halves: the
  never-greet half (a join notice is a moderation fact; no welcome, no comment, no
  reply to the joiner) becomes its own composed section, present whenever join
  notices are projected; the report-the-bait half stays behind `moderation_taught`,
  because it is worthless without the report tool. The split is a move, not a
  rewrite: the surviving sentences keep their wording where it still fits, and the
  `teaching.rs:531-532` pin is rewritten to state the new composition truthfully
  (the ungated prompt now carries the no-comment half and still no report rule).
  *Rejected:* leaving the rule gated and adding a second copy outside — two places
  recording one decision.
- **The moderation bot's transience is taught in one sentence, 2026-08-30.** The
  moderation teaching gains: the moderation bot's own service messages are removed
  by it shortly after they appear — never build an answer on them, never refer
  members to them. One sentence, composed only where the moderation teaching already
  composes. *Rejected:* a mechanism that expires the stored copies — the ledger is
  append-only history and erasure law governs removals; staleness is a fact about
  the platform, taught, not a storage behavior.
- **The reactions unit inherits the exclusions, recorded here for its spec,
  2026-08-30.** When the mark tool ships (unit 39), a mark never targets a join
  notice or a moderation-bot message; unit 39's teaching carve-out names both
  exclusions. Nothing in this unit implements that; the sentence exists so the two
  specs cannot drift apart.

## Acceptance criteria

- **AC1** With a moderation handle configured, a helpful-mode message from that
  sender is recorded, summons no turn, opens no debt, and is excluded from both
  budget counts; an addressed message from that sender (the assistant's mention in
  its text) summons nothing either (pins on both).
- **AC2** A `/del` from the moderation bot still mirrors exactly as before (existing
  pins pass unchanged); a `/del` from the moderation bot with the assistant's
  mention in the same text still mirrors and still summons nothing (pin).
- **AC3** With no moderation handle configured, a bot-named sender's message summons
  under today's rules (pin), and the whole exclusion is absent.
- **AC4** The never-greet join section composes in a helpful-mode group prompt with
  NO moderation handle (pin), the report-the-bait section still composes only under
  `moderation_taught` (existing pins, re-anchored), and the rewritten
  no-join-rule pin states the new composition.
- **AC5** The transience sentence is pinned verbatim inside the moderation teaching.
- **AC6** The checks pass: fmt, clippy with warnings denied, the full suite, the doc
  build, exit codes read bare.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-service`, branch `unit/service-quiet`,
  from `main` (`9ecd0b1`). The build's first step: `git rebase main`.
- Decision records number from the highest shipped at merge; expected records: the
  structural exclusion (with the teaching-alone rejection), the join-teaching split,
  the transience sentence.
- This unit is deploy-relevant: the live assistant welcomed a joiner today, so it
  rides the next deploy the operator approves.
