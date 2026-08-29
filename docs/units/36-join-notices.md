# Unit 36 — a join is seen, and a spam-named join is reported on sight

Date: 2026-08-29. The operator showed a join notice reading like a porn-spam banner —
the account's display name IS the offense, before any message — and ruled: the
assistant sees joins as their own block type and acts under the no-spam rule, where
acting means filing the existing report ("banning outright means /report",
2026-08-29). The human side of the moderation pair does the ban; decision 0070's
human-decides rule stands untouched.

## Grounding

**The lattice already has the slot.** Platform facts reach the core through the
observation surface: the adapter translates pin service messages into
`ObservedFact::PinnedAnnouncement` and the channel title into `ObservedFact::Title`
(`core/src/message.rs:230-242`), and `note_of` (`core/src/note.rs:154`) decides what
becomes a context note. Joins are the same class of platform fact — a service
message, currently skipped at translation (decision 0017 records text as the unit of
recording; service messages fall outside it).

**Why a join cannot be a context note.** A join carries a PERSON — the joiner's
name as the platform shows it, plus their identity — and a context note's text is
outside every erasure pass, which walks the chat-message columns by person and
origin (`core/src/kind.rs`, the mirror pass). Personal data may not land where
erasure cannot reach it. The join therefore lands as its own consumer kind with its
own columns, erasure-aware from birth, the chat message's storage discipline at a
smaller size.

**The name is content here, and decision 0077 is not reopened.** 0077 refuses to
STORE display names as identity data beside every message — re-collecting personal
data to protect it. A join notice is different in kind: the shown name is the
event's content, exactly as message text is content, stored once on the event that
carried it, erased with the person. The identity columns stay what they are
everywhere else; nothing starts recording display names on messages.

**The report tool can already name it.** Telegram's join notice is a real message
with an id, so the join block stores that origin the way a chat message does; the
report tool files against origins in the turn's assessment set
(`core/src/tools/report.rs:643-667` declines what was not assessed), and Rose-side
moderation can delete or ban from the reported message. The join block must
therefore join the assessment set like an absorbed message does.

## Decisions taken with this unit

- **A join lands as its own erasure-aware kind, 2026-08-29.** New consumer kind
  `join_notice`: the joiner's shown name and handle as event content, the service
  message's origin, the platform timestamp, stored in its own table with the
  chat-message columns' erasure discipline — the person-keyed and origin-keyed
  passes null it like a message, and the projection of an erased join renders
  nothing. *Rejected:* a context note (personal data beyond erasure's reach);
  *rejected:* widening the chat-message kind (a join is not a message and every
  message invariant — debt, addressing, budgets — would need carve-outs).
- **Joins project as a stated platform fact, and never wake by themselves,
  2026-08-29.** The projection renders one line in the platform-fact voice ("…
  joined"), so the model reads the name exactly as members saw it. A join owes no
  answer and summons no turn on its own — the assistant is not a doorman greeting
  entrants — but a join absorbed into a turn joins the assessment set, so the model
  CAN report it the moment anything wakes her, and the teaching below makes the
  spam-form join exactly such a wake-worthy case through the existing report path.
  *Rejected:* every join summoning a turn (a busy group would spend the model on
  greetings; and a silent assessment turn per join is the moderation-bot shape
  decision 0070's context already rejected).
- **A spam-form join wakes the report path in helpful mode, 2026-08-29.** In the
  deployed helpful mode the assistant already assesses what it absorbs; the
  teaching gains the join rule: a join whose name is itself promotional bait — the
  no-spam rule's own definition, taught by example shape, never a hardcoded list —
  is reported on sight with the join's origin, exactly as a violating message is.
  The report is the whole action; no ban, no reply to the joiner, no new effect —
  decision 0070 untouched. *Rejected:* a lexical spam filter in code (the model
  assesses under the rules note, the mechanism stays dumb — the same split every
  other assessment uses).
- **The privacy record moves with the new stored fact, 2026-08-29.** A join
  notice stores a person's shown name and handle with an origin — a new row in the
  processing record's data inventory, the same lawful basis and retention as
  message content, reachable by the same erasure. The published policy's erasure
  wording is checked against it; decision 0077 gains a dated annotation stating
  the content-not-identity distinction this unit records. *Rejected:* shipping the
  kind without the record moving (the exact defect class unit 27 named).

## The unit's contract

A member joining a group the assistant serves lands as a `join_notice` block —
name, handle, origin, timestamp, in its own erasure-aware table — projecting as one
platform-fact line, waking nothing by itself, joining the turn's assessment set
like an absorbed message. The teaching makes a promotional-bait join name a
report-on-sight case through the existing report tool, and the report is the whole
effect. Erasure nulls a join notice by person and by origin exactly as it nulls a
message. Decisions 0017, 0070 and 0077 carry dated annotations, the processing
record gains the new row, and nothing else about answering, waking, or moderation
changes.

## Acceptance criteria

- **AC1** Workspace green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** A join lands end to end: a translated join service message becomes a
  `join_notice` block with name, handle, origin and timestamp, projecting as its
  platform-fact line — pinned through a real ingest against the scripted platform.
- **AC3** A join wakes nothing: a join arriving alone summons no turn and owes no
  answer, in both answering modes — pinned.
- **AC4** The report path reaches it: a turn that absorbed a join can file the
  existing report against the join's origin, and the filing passes the assessment
  set's gate — pinned end to end; a join outside the turn's assessment set still
  declines.
- **AC5** Erasure reaches it: the person-keyed pass and the origin-keyed pass each
  null the join's name and handle, and the erased join projects nothing — pinned
  by running the real passes.
- **AC6** The teaching carries the join rule in helpful mode and the composed
  prompt states report-as-the-whole-action — pinned in the composed prompt; no
  ban, kick, or new effect exists anywhere in the diff.
- **AC7** The records move: the processing record's new row, the three dated
  decision annotations, and the policy's erasure wording checked — pinned by the
  documentation suite per file.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-joins`, branch
  `unit/join-notices`). Sites: `adapters/telegram/src/translate.rs` (the join
  service message translated where pins already are — `new_chat_members` on the
  wire), `core/src/message.rs` (the neutral carrier), the new kind beside
  `core/src/kind.rs`'s chat message with its erasure hooks, `core/src/teaching.rs`
  (the join rule), the report tool's assessment-set source, the spine suite, and
  `docs/privacy/records-of-processing.md` plus the three decision annotations.
- The join's WAKE-NOTHING rule must fall out of the existing summons machinery (a
  join_notice simply never carries a summons), not out of a special case in the
  dispatch — the operator's snowflake rule binds.
