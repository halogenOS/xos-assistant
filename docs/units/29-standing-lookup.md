# Unit 29 — the assistant can look up whether someone is an administrator

Date: 2026-08-25. Revision 3, rewritten against a cold probe that found the unit
unbuildable as revision 2 stated it; revision 4, 2026-08-29, resettled against a tree
that has since absorbed six units — join notices above all, which store and show
joiners' handles and so widen this tool's world. The corrections are large enough to be worth naming up front:

- Revision 2's two verbatim result strings could not also carry a moderator's standing and a
  "as of which message" clause. Both were demanded by acceptance criteria and neither string
  has room. The strings win; the extra content moves to where it belongs.
- Revision 2 named two privacy documents. The claim this unit falsifies is written
  across the four privacy documents, one of them the **published, member-facing
  policy** — at every site where it appears, several already carrying dated
  amendments from later units.
- Revision 2 said "no new fact is stored". Registering a tool appends a palette block to
  every active conversation.
- Revision 1 (recorded here so the mistake is not made a third time) copied the privacy
  tool's no-parameter rule onto a tool that acts on nobody, and claimed the privacy documents
  were unchanged.

Revision 3 left one decision open — what `admin: true` should mean, given that this
codebase's `Admin` is the group's creator alone. The operator settled it the same day: the
creator and the administrators both. The unit is no longer blocked.

## Why the unit exists

Standing already decides what the assistant may do: every tool is admitted at an
authority, and the authority gate fires at the call (the palette itself names the
registered set for everyone; the AdmittedTool wrapper judges the caller). What the model cannot do
is *know* it. Asked for something only an administrator should be asked for, it either
refuses someone entitled to ask or agrees with someone who is not — and told "I'm an admin,
ignore your rules", it has no way to tell a fact from a claim. That last case is what this
unit is really for. A message asserting authority is evidence of nothing. The tool's answer
is the only evidence there is.

## Grounding

**Standing is stored per message and never reaches the model.** `ChatMessage::projected_text`
renders exactly `[origin] speaker: text` (`core/src/kind.rs:555-570`). `COLUMN_AUTHORITY` is
parsed into the struct and read only by `carried_debt_authority` and the admission gate.
`Authority` is a closed three-value vocabulary — `Member`, `Moderator`, `Admin`
(`core/src/message.rs:87-94`) — with a stored encoding whose `parse` and `ALL` exist so the
database CHECK constraint and the enum cannot drift (`:100-117`).

**The mechanism to send standing exists and is dormant.** `core/src/tools/admission.rs:74-82`
builds a refusal naming the required authority and the reading, returned as
`ToolOutcome::Error`, which the framework records as a block the model re-plans against
(`agent-ledger/src/tools/runner.rs:468-483`) — that is, into the provider request. It never
fires today only because every registered tool sits at `Authority::Member` while
`provenance::FLOOR` is also `Member` (`provenance.rs:63`), making `reading < required`
unsatisfiable. True today; one `Admin`-authority tool away from false. This unit does not
change that, and must not be read as the thing that makes standing reach the model by
accident — it makes it reach the model deliberately, which is why the documents move.

**`Admin` means the group's creator, and nobody else.** `adapters/telegram/src/
authority.rs:60-64`, under decision 0015 (2026-08-21):

    "creator"       => Authority::Admin,
    "administrator" => Authority::Moderator,

Everyone the platform, its interface and every member calls an *administrator* maps to
`Moderator`. This is the finding that reshapes the unit, and the first decision below is its answer.

**Handles are stored in TWO places now; the "handle we were shown" bound is prose.**
`COLUMN_SPEAKER` holds "the sender's public username as the platform delivered it at
receipt", bounded by `storable_speaker` (`core/src/kind.rs`); and since unit 36
(2026-08-29) every JOINER's handle is stored under the same bound in the join table
(`core/src/join.rs`) and projected AT-sign-prefixed into the model's own context — so
the model can see a handle that never spoke. Display names are not stored as identity
data (decision 0077, since annotated: a join event's shown name is erasable event
content). The bound revision 2 told the implementer to reuse is **prompt prose with no
code behind it** (`prompts/30-conduct.md`: "never guess a handle you were not shown");
the nearest code analogue, `resolve_reportable`, now resolves a message's origin or a
join event's — a different key. This unit therefore builds the matcher; it does not
reuse one.

**Erasure keeps standing and drops the handle.** `erasure.rs` via `kind.rs:688-705` nulls
`text`, `origin`, `sent_at`, `reply_target` and `speaker`, and leaves `authority` and
`principal_id` standing. Which key the tool matches on therefore decides an erasure outcome,
and the spec must say which.

**The palette reconciles itself.** `reconcile_palette` (`assembly.rs`, first
activity per process) compares the newest
stored palette against the registered set on first activity per process and appends a fresh
block on difference (decided 2026-08-23), so conversations predating this tool gain it. That
append is also a new stored fact, which revision 2 denied.

**Where non-lookup tools register, and where the teaching goes.**
`core/src/tools/lookup.rs` carries the bounded-GET contract; `mod.rs` is the whole
tools tree. The peers split three ways: report and rights register in
`admit_assembled_tools` taking the erasure fence; web search registers there without
one; the runtime-facts tool registers later, in `Assistant::start`, unconditionally.
This tool reads person data and registers in `admit_assembled_tools` WITH the fence,
exactly the rights precedent. The teaching goes in `prompts/30-conduct.md` beside the privacy tool's
(`:49-58`), because a conduct rule belongs in the conduct prose — NOT because
`teaching.rs` is conditional-only, which stopped being true when unit 32 put the
unconditional identity-routing paragraph there; the site is chosen on fit, stated so
the reason survives verification.

**Fixed results are how this codebase answers a call that cannot proceed — and the
no-retry line belongs to the PERMANENT ones only.** The shared `NO_RETRY` close
(`admission.rs`) ends every refusal whose fact will not change; the TRANSIENT class
deliberately carries none — `admission.rs` documents "the fact may not hold beyond
this failure" and the report tool's transient is PINNED to not contain it. Revision 2
demanded byte-exactness of two strings and left a third unwritten; revision 4
wrongly told every refusal to close with no-retry, which the transient convention
forbids.

## Decisions taken with this unit

- **`admin: true` means what the group's own member list means — the creator and the
  administrators both, settled by the operator 2026-08-25.** This codebase's `Admin` is the
  creator alone and `Moderator` is everyone the platform labels an administrator, so a tool
  answering only for `Admin` would tell a real administrator they were not one, and pin that
  false statement into a test. It would also dissolve the unit's purpose by contradicting an
  honest claimant. The result string is read by a model that knows nothing of this codebase's
  enum; it should mean what a member sees. *Rejected:* the creator alone, which would have
  needed different wording throughout because "administrator" would be false for the people
  who are ones.
- **The mapping from the three-value vocabulary to the two answers is written down once,
  2026-08-25.** `Admin` and `Moderator` answer true, `Member` answers false. It lives in one
  named place so a reader is never left inferring it, and so a second place cannot decide it
  differently — which is how a privilege check becomes a privilege escalation. *Rejected:* a
  third result string naming the standing found (revision 2's AC3) — the operator specified
  two strings and pinned them byte-exact; a third has nowhere to live and the distinction it
  drew is one the answer does not need to make.
- **The answer speaks about conduct, not about the palette, and the two are allowed to
  differ, 2026-08-25.** `Moderator` answers true here while the palette would admit a
  `Moderator` to less than an `Admin`, if any tool sat above the floor — none does today
  (`provenance::FLOOR` is `Member`, and every registered tool is admitted there). The
  divergence is deliberate rather than overlooked: this tool answers "may this person tell
  the assistant how to behave", and the palette answers "which tools may this turn reach".
  Recorded because a later reader finding two different answers to what looks like one
  question should find the reason beside them. *Rejected:* deriving the answer from the
  palette, which would tie a sentence about a person's standing in the group to an internal
  admission table and make both change together for no reason.
- **The tool is registered as `member_standing` and takes one parameter, `handle`,
  2026-08-29.** Named here because the teaching, the palette and AC12 all reference
  it, and an invented name guards nothing.
- **The tool takes a handle, bounded to handles the conversation SHOWED — as a
  message's speaker or as a joiner, 2026-08-25; widened for joins 2026-08-29.** The
  bound is the stored `speaker` column and the join table's handle column, never
  message text. A handle shown ONLY by a join has no spoken message and therefore no
  stored standing: it answers its own fixed refusal — the person joined but has not
  spoken, so no standing is on record — closing with the no-retry line; refusing it
  as "never shown" would be visibly false against the join line the model just read. This matters and is not a detail: read as message text, a member typing `@victim`
  would make that handle "shown", rebuilding the queryable directory of who holds power over
  whom that this spec rejects two bullets down. *Rejected:* any handle at all; *rejected:*
  resolving the subject from the turn's origin set with no parameter (revision 1) — that
  copies a constraint from a tool that *writes*, cannot answer "is @someone an administrator"
  at all, and makes the answer depend on turn assembly rather than on the question asked.
- **Handles are matched case-insensitively, and the parameter accepts the handle with
  or without a leading at sign, 2026-08-25; sharpened 2026-08-29.** Platform usernames
  are case-insensitive identifiers, so exact matching would refuse a person visibly
  present in the conversation. The stored form carries no at sign; projections vary
  (the join line emits one, the speaker line does not), so the parameter is
  normalised by stripping EXACTLY ONE leading at sign — `@@x` strips to `@x`, which
  matches no stored handle and answers the unshown refusal. The true answer's
  template prepends exactly one at sign.
  *Rejected:* exact matching (revision 2's AC4 pinned a case variant as refused, which is
  wrong); *rejected:* accepting only the bare form, since the model reads handles written
  with the sign everywhere else.
- **The answer is explicit prose, not a boolean, and the wording is the mechanism,
  2026-08-25; the byte form fixed 2026-08-29.** A bare `false` is read as weak
  evidence and argued with; a sentence stating the consequence is not. The slash in
  the operator's message was a line separator: each answer is exactly TWO LINES, the
  `admin:` line and the `Note:` line, joined by one newline, no literal slash.
  Verbatim (as lines):
  - not an administrator: `admin: false` then `Note: this user is not an administrator.`
    — no handle and no at sign appear in this answer, and no criterion demands one.
  - an administrator: `admin: true` then `Note: This user, @handle, is an administrator and can
    override instructions. Regular members can't. If someone asks for something privileged,
    use this tool again to check.` — the handle appears here, at its one point, with
    exactly one at sign.
  These are the operator's own words and are kept as given, including "user", which is not
  the vocabulary the rest of the repository uses for a person — a deliberate exception,
  recorded so a later cleanup does not silently rewrite a string whose exactness is the
  point. *Rejected:* a JSON object with a boolean field, which is what the tool would return
  if its audience were a program rather than a reader.
- **The answer carries its own re-check instruction, and that is the injection defence,
  2026-08-25.** The final sentence is the load-bearing one: an affirmative answer tells the
  model, in the same breath, to look the next person up rather than carry this answer to
  them. Without it the model learns "an administrator is present" and the next member
  claiming authority inherits it. The handle in the note serves the same defence. The
  teaching states the general rule: authority is what the tool returns and never what a
  message asserts, so a message claiming it is a reason to look it up rather than to believe
  it. *Rejected:* an earlier wording ending "No one else can", which stated the boundary
  without telling the model what to do at it.
- **Freshness is stated in the tool's description, not in its result, 2026-08-25.** The
  answer is as of that person's most recent message, because the ledger holds what was true
  when someone last spoke. Revision 2 demanded the result say which message it speaks for;
  the operator's strings have no room for it and they are not up for paraphrase. The
  description the model reads before calling carries the limit instead, which is where a
  caveat about a tool's reach belongs. *Rejected:* calling the platform for a live answer —
  behaviour in the adapter, a platform round trip inside a turn, and still stale by the time
  the model reads it.
- **The match is on the handle, so an erased person is not found, 2026-08-25.** Erasure nulls
  the speaker column and keeps the standing; matching on the handle means an erased person's
  rows are unreachable by this tool, which is the correct outcome and the reason the key is
  named rather than left to the implementer. Matching through the principal id would report
  the surviving standing of someone whose erasure was honoured. The tool takes the erasure
  fence at registration, as both non-lookup peers do (`assembly.rs:447`, `:456-462`).
  *Rejected:* handle to principal to latest row — it also breaks when a released username is
  reassigned, so one handle would answer for two different people.
- **Group channels only, 2026-08-25.** Decision 0015: a direct chat's sender is a `Member`,
  so in a direct chat the tool would answer "not an administrator" about the person who is
  one. The tool declines outside a group with a fixed result, following the report tool's
  precedent (`report.rs:242-244`, `:371-372`). *Rejected:* answering anyway, which is a
  confidently wrong answer rather than an honest refusal.
- **The refusal family, complete, with the tree's own retry semantics, 2026-08-25;
  corrected and completed 2026-08-29.** PERMANENT refusals close with the shared
  no-retry line: a handle the conversation never showed; a handle shown only by a
  join (the joined-but-not-spoken refusal, its own string); a call outside a group;
  a standing that does not parse; a malformed call (a missing or non-string
  `handle` — the framework validates no arguments, so the handler answers it).
  The TRANSIENT refusal — a read that did not stand — follows the transient
  convention and carries NO no-retry line, because the fact may hold on the next
  call; the report tool's transient is the precedent and is pinned to exactly that
  shape. Exact texts settle at implementation against the peers' phrasing and are
  pinned. *Rejected:* leaving them to the implementer (revision 2's failure);
  *rejected:* a no-retry line on the transient (contradicts the documented,
  pinned convention).
- **Member authority, because the question is not privileged, 2026-08-25.** The answer is
  visible in the group's own member list. *Rejected:* admitting it at `Admin`, which would answer only for
  people who already know the answer, and which would also wake the dormant refusal path
  described in the grounding.
- **What an override reaches is the conduct, never the mechanism, 2026-08-25.** An
  administrator can tell the assistant how to conduct itself and cannot make a tool do
  something the tool does not do. Decision 0070's human decision point, the privacy tool's
  subject resolution, the admission rule and the erasure fence are mechanism and are reachable
  by instruction from nobody. The teaching says so, so the model neither refuses an
  administrator out of caution nor believes an instruction can unlock a guard. *Rejected:*
  leaving it unsaid — the guards hold either way, but a model that believes an instruction
  *could* work will keep trying and will say so to the member.
- **The privacy documents move with this unit — the four files, at every site the
  claim appears, 2026-08-25; re-anchored 2026-08-29.** Standing is stored today and
  never leaves the machine; this tool sends it to the model provider, a new category
  of personal data reaching a processor. The claim it falsifies appears across the
  four privacy documents, several sites already carrying dated amendments from units
  27 and 36, so the implementer anchors on the SENTENCES, not on line numbers: the
  record of processing's R1 "what it receives" row and its minimisation row; the
  impact assessment's identity claims (the "no other attribute of a person is
  attached to a request" family, amended once already) and its risk register; the
  legitimate-interests assessment's "exactly one identifier" claim and its
  re-weighing obligation — a standing procedural clause unit 27 discharged twice
  (its dated "Re-weighed" notes are the precedent shape this unit follows; unit 36
  amended the condition without a discharge); and **the published policy's** list of
  what each request carries, already reopened once by a dated addition. Each edit carries a dated amendment note, and
  the docs suite pins each per file. *Rejected:* shipping and amending after — the
  spec named this defect class itself and revision 2 then walked into it.

## The unit's contract

The model can call one tool, at member authority, in a group channel, naming a handle that
appears as the speaker of some message in the conversation, matched case-insensitively and
accepted with or without a leading at sign. It receives one of two fixed-wording answers
stating whether that person held administrator standing when they last spoke, naming the
handle, and — where the answer is yes — telling the model to look the next person up rather
than carry this answer to them. A handle the conversation never showed, a call outside a
group, and a read that does not stand each return their own fixed refusal closing with a
no-retry line, and assert nothing about anybody. The mapping from the stored three-value
vocabulary to the two answers is written in one place. No platform call is made and no
adapter gains behaviour. No new table or column is added; registering the tool does append
one palette block per active conversation, which is the existing reconciliation doing its
job. The tool takes the erasure fence, and an erased person is not found. The teaching states
that authority is what the tool returns and never what a message claims, that an override
reaches conduct and never a mechanism, and what to do when a lookup is refused. The four
privacy documents, the published policy among them, carry standing as data reaching
the model provider at every site the claim appears, each with a dated amendment note,
before this ships. A handle shown only by a join answers the joined-but-not-spoken
refusal.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The two answers are byte-exact, with the handle substituted at its one point and
  exactly one at sign in the output — pinned character for character, since the wording is
  the mechanism and a paraphrase is a defect. Pinned for a handle supplied bare and for the
  same handle supplied with an at sign, proving one output, not two.
- **AC3** The vocabulary maps completely: each of the three stored values produces its
  specified answer — pinned per value, so the mapping cannot be read off one example.
  `Moderator` answers true, and that case carries the pin a careless implementation fails.
- **AC4** The handle bound holds and does not over-refuse: a handle the conversation
  never showed is refused; a handle appearing only inside another member's message
  TEXT is refused, since that is the directory this unit rejects; a handle differing
  from a shown one only in case IS answered, not refused; a handle shown ONLY by a
  join answers the joined-but-not-spoken refusal, never the never-shown one and never
  a guessed standing — pinned per case.
- **AC5** The answer is as of the last message: a person whose stored standing differs
  between two of their messages is reported at the later one — pinned. The limit is stated in
  the tool's description, checked there rather than in the result.
- **AC6** No adapter behaviour and no platform call: the diff adds nothing to
  `crates/adapters/`, and the tool's answer is computed from stored facts alone — checked as
  a property of the change, since the core holds no client and a test that unplugs one would
  pass whether the tool were right or wrong.
- **AC7** The tool is reachable by an ordinary member in a group and declines outside one —
  pinned through the palette at member standing, not by calling the handler directly, since
  what is checked is the admission.
- **AC8** The refusal family is complete and carries the tree's retry semantics:
  the five PERMANENT refusals (unshown handle, joined-but-not-spoken, non-group
  channel, unparseable standing, malformed call) each a fixed string closing with
  the no-retry line; the TRANSIENT failed read a fixed string WITHOUT one — pinned
  per case, including pinning the transient's absence of the line, the report
  precedent's own pin shape. None asserts any standing.
- **AC9** An erased person is not found: a person whose messages were erased, whose stored
  standing survives the erasure by design, is answered with the unshown-handle refusal —
  pinned, because the erasure keeping standing while dropping the handle is exactly the trap
  a principal-keyed implementation falls into.
- **AC10** The teaching addition changes no mechanism outcome: the admission refusal, the
  privacy tool's subject resolution and the human decision point behave identically with the
  teaching present — checked against the existing pins (`admission.rs:280-320`, the report
  and privacy-rights spine tests) rather than by adding tests that vary an input no mechanism
  reads.
- **AC11** The documents move, all four files at every claim site: the record of
  processing, the impact assessment, the legitimate-interests assessment (its
  re-weigh discharged with a dated note, the units-27/36 precedent shape) and **the
  published privacy policy** each carry standing as data reaching the model provider,
  each with a dated amendment note — checked per file, and pinned by the
  documentation suite. A green AC while the published policy's request-contents list
  no longer holds is the defect this criterion exists to prevent.
- **AC12** The tool's registered name (`member_standing`) and its model-facing
  description are pinned, the description carrying both the freshness limit and the
  group-only bound — since the description is the surface the model actually chooses
  from, and no other criterion covers it.
- **AC13** The unit's decisions land as numbered records in `docs/decisions`
  (continuing from the highest shipped number) and the documentation suite pins
  them — the unit-27 precedent, whose absence from revision 4's site list was
  itself the omission this spec mocks revision 2 for.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-standing`, branch
  `unit/standing-lookup`, rebased 2026-08-29 onto a main carrying units 27-36; a
  stale pre-freeze partial build sits archived in the worktree's stash and is NOT to
  be resumed — build fresh). Sites: a new tool module beside
  `core/src/tools/rights.rs`; registration in `admit_assembled_tools`
  (`assembly.rs:426-453`; report and rights take the erasure fence, this tool takes
  it too) and **not** in `core/src/tools/mod.rs`, whose contract is bounded HTTP
  lookups; the match over the speaker column AND the join table's handle column; the
  teaching in **`prompts/30-conduct.md`** beside the privacy tool's; the four
  privacy documents at their claim sites; and `docs/decisions` (the unit's numbered
  records, from the highest shipped number on).
- Read `core/src/tools/rights.rs` end to end, module documentation included — for its
  fixed-result form, its no-retry phrasing and its member authority, NOT for its no-parameter
  rule, whose reason is that it writes and this one does not.
- `report.rs` is the second precedent worth reading whole: its group-only decline, its
  transient error, and `resolve_reportable`'s shape for validating a caller-supplied
  identifier against the conversation.
- The unit is ready to build. The decision revision 3 left open was settled on 2026-08-25
  and is recorded as a decision above, not as a question.
