# Unit 36 — a join is seen, and a spam-named join is reported on sight

Date: 2026-08-29. The operator showed a join notice reading like a porn-spam banner —
the account's display name IS the offense, before any message — and ruled: the
assistant sees joins as their own block type and acts under the no-spam rule, where
acting means filing the existing report ("banning outright means /report",
2026-08-29). The human side of the moderation pair does the ban; decision 0070's
human-decides rule stands untouched.

## Grounding

**The lattice already has the slot, and the slot carries the gates this unit needs.**
Platform facts reach the core through the observation surface: the adapter
translates pin service messages into `ObservedFact::PinnedAnnouncement` and the
title into `ObservedFact::Title` (`core/src/message.rs:230-242`), the observe path
(`assembly.rs:1014`) runs the admitted-channel authorization and withdraws for a
group the assistant does not serve, and `note_of` (`core/src/note.rs:154`) decides
what becomes a note. Joins are the same class of platform fact — a `new_chat_members`
service message, a LIST of joiners in one message, decoded nowhere in the tree today
(it dies at the no-text skip; decision 0017's recording unit is text).

**Why a join cannot be a context note.** A join carries a PERSON, and context-note
text is beyond every erasure pass — decision 0055 records that as an open fact. The
chat-message columns show the discipline to copy: the person-keyed pass
(`kind.rs:691-708`) nulls by principal, the origin-keyed pass (`kind.rs:746-787`)
nulls by origin within a conversation.

**The name is content here, and decision 0077 is not reopened.** 0077 refuses to
store display names as identity data beside every message. A join notice is
different in kind: the shown name is the event's content, exactly as message text is
content, stored once on the event that carried it, erased with the person. Nothing
starts recording display names on messages.

**The report tool's real gate, and who else drinks from its spring.** The filing
resolves targets through `resolve_reportable` (`report.rs:293-320`), fed by
`co_summoners` (`provenance.rs:112-153`), which reads ONLY chat messages bearing
taken debt — and co_summoners feeds FOUR questions with different needs: the report
resolver, `sole_principal` (`provenance.rs:171` — the search budget's key and the
rights commands' person), the answer threading (`outbound.rs:589`, filtering by the
literal-addressed flag), and the disclosure fold (`disclosure.rs:145`, deciding the
first-contact line from every principal in the set). A join pushed into that shared
spring would change disclosure and person-counting for turns it merely witnessed —
so the widening belongs to the REPORT'S question alone. Telegram's join notice is a
real message with an id, so the join carries an origin the report can name and the
human side can act on.

**Two more facts the first rounds surfaced.** A kind that merely never summons can
still BURY: an opaque tail stands the frontier down (`ratchet.rs:238`), and the
context note is agency-inert AND frontier-transparent for exactly that reason
(`note.rs:263-265`) — the join kind needs both properties. And the suppression
flag stops collection for an opted-out person (the processing record's D2 row;
decision 0074's stub discipline; the read-only lookup at `identity.rs:110` exists
so a flagged person's data is never refreshed) — the observe path has no
suppression machinery today, and this unit must not store a flagged person's join.

## Decisions taken with this unit

- **Joins ride the observation surface and are stored by it, 2026-08-29.** The
  adapter translates a `new_chat_members` service message into one new fact,
  `ObservedFact::MembersJoined` — the joiners as the platform shows them, the
  service message's origin, the platform timestamp — and the observe path, behind
  its EXISTING authorization gate, stores the join blocks: an unadmitted group
  withdraws and stores nothing, which is the privacy answer for free. `note_of`
  answers None for it; a join is a block, never a note. The assistant's OWN entry
  is excluded at translation — her membership is `ObservedFact::Added`'s territory
  already — and a join in anything but a group, and every other membership service
  shape (`left_chat_member`, kicked, chat-created), keeps today's named skip: they
  are not joins, and decision 0017 still governs them. *Rejected:* the ingest seam
  (its contract is member speech — sender, authority, addressed, text, debt — and
  a join is none of that); *rejected:* a new assembly entry beside observe (a
  second door for the thing the observation door exists for).
- **One block per joiner, one shared origin, 2026-08-29.** A service message
  naming several joiners lands one `join_notice` block per joiner — name and
  handle as event content, the joiner's principal resolved through the SAME
  identity path a sender's is (`identity.rs`; a joiner is a member), the shared
  origin, the timestamp — in its own table with the chat-message columns' erasure
  discipline. The person-keyed pass nulls one joiner's block, registered at the
  erasure composition site (`erasure.rs:141-148`) beside the message kind's; the
  origin-keyed pass rides the deletion mirror (`mirror.rs:53-64`,
  `erase_message_named`'s shape extended to the join table), because deleting the
  service message removes the event; the new table enters through an appended
  schema migration step, decision 0026's discipline, beside the report's
  (`schema.rs:298`). The joiner's rendered name is the platform's own
  composition — the first name, and the last name when one exists, space-joined —
  recorded here so the fixture and the projection agree. An erased join projects
  nothing. *Rejected:* one multi-person
  block (person-keyed erasure cannot null one person out of a shared row);
  *rejected:* a context note (0055); *rejected:* widening chat_message (every
  message invariant would need carve-outs).
- **Joins project as a stated platform fact, and never wake by themselves,
  2026-08-29.** The projection renders one line per joiner in the platform-fact
  voice, so the model reads the name exactly as members saw it. A join owes no
  answer and summons no turn — falling out of the existing summons machinery,
  because a join_notice simply never carries a summons, never as a special case in
  dispatch. A join is seen when a turn composes over a window containing it; the
  window and composition already read the ledger the join sits in, and the unit
  pins that a join inside the window reaches the request. *Rejected:* every join
  summoning a turn (the model spent on greetings, and the silent per-event
  assessment turn is the shape 0070's context already rejected).
- **The report's own source unions the windowed joins; co_summoners is untouched,
  2026-08-29** (the first round steered toward one shared spring; the second
  proved the shared spring poisons the other three drinkers — this is the
  corrected shape, and the two rounds together are why it is right). A new
  provenance question — the turn's assessed joins — reads the join notices the
  turn's window carried, and `resolve_reportable` resolves over co_summoners
  UNION that set; a join contributes its origin and the joiner's principal to the
  report's resolution only. `co_summoners` itself does not change, so
  `sole_principal`, the answer threading and the disclosure fold are unchanged BY
  CONSTRUCTION — their source is byte-identical — and AC4 pins exactly that.
  *Rejected:* widening co_summoners (a windowed joiner would count as a person
  toward disclosure's first-contact line and sole_principal's two-distinct
  decline); *rejected:* a second resolution path INSIDE the report tool (the
  provenance module owns the turn-membership questions; the report keeps asking,
  provenance keeps answering).
- **The join kind is agency-inert and frontier-transparent, 2026-08-29.** The
  context note's twin properties, for the context note's reason: a join appended
  over an unanswered message must bury nothing — the frontier reads through it
  and the addressed message behind it still gets its turn. Never a summons, never
  an anchor, never a debt. *Rejected:* the framework's opaque default (an
  unanswered question behind a join would silently die — the burial the ratchet
  exists to prevent).
- **A suppressed person's join stores nothing, 2026-08-29.** Before resolving or
  refreshing anything, the observe path consults the suppression flag through the
  read-only identity lookup; a flag-bearing joiner's notice is withdrawn whole —
  no block, no name, no principal refresh — because the processing record
  promises collection stops with the flag, and this unit does not bend a promise
  to gain a feature. The group still sees the platform's own join line; the
  assistant simply keeps no record. *Rejected:* storing with a suppressed name
  (a contradiction of the published record); *rejected:* a stub block (a record
  of the fact of joining is still a record about the person).
- **A spam-form join wakes the report path through teaching, not code,
  2026-08-29.** In helpful mode the teaching gains the join rule: a join whose
  name is itself promotional bait — the no-spam rule's own definition, taught by
  shape, never a hardcoded list — is reported on sight with the join's origin,
  exactly as a violating message is. The report is the whole action; no ban, no
  kick, no reply to the joiner, no new effect — decision 0070 untouched.
  *Rejected:* a lexical spam filter in code (the model assesses under the rules
  note; the mechanism stays dumb, the same split every assessment uses).
- **The privacy documents move wherever the name now travels, 2026-08-29.** A
  join notice stores a person's shown name and handle with an origin, and the
  projected line rides inside requests to the model route — stored AND
  transmitted. Every published sentence that becomes false moves with a dated
  note: the processing record's R1 claim that no display name exists to send,
  its minimisation row, the policy's "we do not store your display name"
  sentence (reworded to the truth: display names are not stored as identity
  data; a join event's shown name is stored once as erasable event content), the
  impact assessment's and the legitimate-interests assessment's identity claims,
  plus the new inventory row and the dated annotations on decisions 0017, 0070
  and 0077. *Rejected:* shipping the kind while any published sentence
  misstates it — the exact defect class this spec refuses elsewhere.
- **The teaching reaches existing conversations at the next deploy start,
  2026-08-29.** Prompts are recorded per conversation and the deploy's
  prompt-refresh mechanism forks stale conversations onto the current prompt at
  startup — existing behavior, nothing to build, stated so nobody reads AC6 as
  leaving old conversations untaught forever.

## The unit's contract

A member joining a group the assistant serves lands as one `join_notice` block per
joiner — name and handle as content, principal resolved like a sender's, the
service message's shared origin, in an erasure-aware table — stored through the
observation surface behind its existing authorization gate, projecting as one
platform-fact line per joiner, waking nothing by itself. A turn whose window
carries a join can report it through the existing report tool, whose assessment
set now openly includes windowed joins; the report is the whole effect. Erasure
nulls a join by person and by origin exactly as it nulls a message. The
assistant's own entry, non-group joins, and every other membership shape keep
their named skips. Decisions 0017, 0070 and 0077 carry dated annotations, the
processing record gains its row, and nothing else about answering, waking, or
moderation changes.

## Acceptance criteria

- **AC1** Workspace green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** A join lands end to end through the observation seam: a translated
  `new_chat_members` service message becomes one block per joiner with name,
  handle, principal, shared origin and timestamp, projecting its platform-fact
  lines — pinned against the scripted platform, including the several-joiners
  case; an unadmitted group stores nothing, pinned.
- **AC3** A join wakes nothing and buries nothing: arriving alone it summons no
  turn and owes no answer; appended AFTER an unanswered addressed message, that
  message still draws its turn (the frontier reads through the join) — both
  pinned in both answering modes; and the assistant's own entry produces no
  join_notice, pinned.
- **AC4** The report path reaches it and nobody else feels it: a turn whose
  window carried a join files the existing report against the join's origin and
  passes the gate; a join outside the window declines; and `co_summoners` is
  byte-identical (the diff shows no change to it), so the disclosure fold, the
  answer threading, `sole_principal`, the search budget's key and the rights
  commands are unchanged by construction — pinned by the diff plus one
  behavioral spot-pin: a turn with a windowed join still discloses and threads
  exactly as without it.
- **AC5** Erasure reaches it: the person-keyed pass nulls one joiner's block
  leaving a co-joiner's intact; the deletion mirror's origin-keyed pass nulls
  the whole event; an erased join projects nothing; and a SUPPRESSED person's
  join stores nothing at all — each pinned by running the real passes.
- **AC6** The teaching carries the join rule in helpful mode with
  report-as-the-whole-action — pinned in the composed prompt; no ban, kick, or
  new effect exists anywhere in the diff.
- **AC7** The records move, the full inventory: the processing record's new row
  AND its corrected R1/minimisation sentences, the policy's display-name and
  erasure wording, the impact assessment's and legitimate-interests assessment's
  identity claims, the three dated decision annotations — pinned by the
  documentation suite per file, and no published sentence claims display names
  are never stored or sent.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-joins`, branch
  `unit/join-notices`). Sites: `adapters/telegram/src/translate.rs` (the
  `new_chat_members` arm beside the pin's, with the own-entry and non-group
  skips), `core/src/message.rs` (the new fact), the observe path in
  `core/src/assembly.rs` (storage behind the existing gate), the new kind beside
  `core/src/kind.rs`'s chat message with its erasure hooks,
  `core/src/tools/provenance.rs` (the new assessed-joins question beside
  co_summoners), `core/src/mirror.rs` and `core/src/erasure.rs:141-148` (the
  origin and person passes), `core/src/schema.rs` (the appended migration
  step),
  `core/src/teaching.rs` (the join rule), the spine suite, and
  `docs/privacy/records-of-processing.md` plus the three decision annotations.
- The join's wake-nothing rule must fall out of the existing summons machinery,
  not out of a special case in the dispatch — the operator's snowflake rule
  binds.
