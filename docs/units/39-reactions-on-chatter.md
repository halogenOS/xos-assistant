# Unit 39 — a reaction where a reply would add nothing

Date: 2026-08-30. The operator's queue entry, verbatim: "Add reactions to the queue to
reduce the amount of terminal responses to off topic", confirmed as: the assistant
reacts to a message with an emoji reaction instead of sending a short reply where a
reply adds nothing — mostly the off-topic chatter that today draws either a low-value
text answer or total silence.

## Governing design, and what this unit changes about it

The machinery is already designed: `docs/units/telegram/06-reactions.md` (revised
after two independent reviews, 2026-08-25, unbuilt) specifies the mark tool, the
outbound edge's second arm, the MessageMark block kind with its erasure and mirror
passes, the per-origin duplicate check, the zero rate-limit interaction (budgets
count opened debts, not acts), and the quarantine on unproven platform behaviors
(nothing merged may depend on direct-chat reactions or empty-array withdrawal).
WHERE BOTH SPEAK, T6'S WORDING GOVERNS THE MECHANISM — except the decisions this
unit explicitly supersedes below. T6 itself is not edited; its anchors (about 176 of
them) predate units 31/36/37/38 and the implementer greps rather than
trusts them (worked examples: `OutboundReply` now `message.rs:573-599`;
`Assistant::replies` now `assembly.rs:1367-1377`; `consume_replies` now
`driver.rs:906-944`; `deliverable_of` now `outbound.rs:543-567`;
`CONSUMED_UPDATE_TYPES` now `client.rs:174`).

## Decisions taken with this unit

- **The trigger is inverted from T6, on the operator's confirmed reading,
  2026-08-30.** T6 taught the mark for messages speaking TO the assistant and called
  it "noise on a message that was not speaking to the assistant" — the operator's
  confirmed intent is the opposite: the reaction's home is the unaddressed chatter,
  where the hardened silence teaching (unit b46d6ba) forbids words. T6's "where a
  mark fits" rule is superseded whole; the carve-out below carries its own
  conditions and no other fit rule survives anywhere. Honesty note carried into the
  spec: in helpful mode every message summons, so the co-summoner aiming check
  admits overheard messages and the TEACHING is the real control — stated, not
  claimed otherwise.
- **The silence sentence is amended, not excepted beside, and the exact copy is
  this unit's, 2026-08-30.** Decision 0148 rejected carving an exception into the
  hardened sentence for the announce, resolving that case by wording alone — it
  could, because the announce added no new act. This unit adds one, so the sentence
  itself changes, a deliberate supersession recorded in its decision record. The
  Helpful arm's sentence, today verbatim "they get nothing from you, not an answer,
  not an acknowledgment, not a comment", becomes:
  `they get nothing from you in words: not an answer, not an acknowledgment, not a comment.`
  And the carve-out joins it, verbatim:
  `The one exception is the emoji reaction: where you would otherwise end an empty turn but a message genuinely lands - a share, a milestone, a joke that landed - you may put one reaction on it instead. A reaction never rides with words on the same message, most messages deserve no reaction either, and silence stays the default.`
  Both strings are pinned byte for byte. The addressed arm stays unamended on
  purpose: in addressed mode unaddressed chatter opens no turn at all, so the
  CHATTER carve-out never meets that arm's sentence. The react teaching itself
  composes in both modes (registration is unconditional, T6's one-predicate
  rule), and in addressed mode its empty-turn clause legitimately covers an
  ADDRESSED message that lands with nothing to say — a share aimed at her may
  draw the reaction there too, which contradicts nothing the addressed arm
  states. *Rejected:* a carve-out sentence beside
  the untouched rule — the composed prompt would contradict itself on a literal
  read, the exact collision 0148 documents.
- **The conduct line that earns text replies moves to the mark, with the copy
  decided here, 2026-08-30.** The line lives in the embedder's conduct prompt
  (prompts/30-conduct.md:94, wrapping to :96). Today, verbatim: "Match your length to
  the message's weight: a casual share earns a short reaction, a real question
  earns a real answer, and restating someone's own words back at them adds
  nothing." It becomes, verbatim and pinned:
  `Match your response to the message's weight: a casual share earns an emoji reaction, not a written reply; a real question earns a real answer, and restating someone's own words back at them adds nothing.`
- **The mark vocabulary is the full palette, on the operator's answer, 2026-08-30.**
  The operator's answer, verbatim: "I didn't say only a positive set. Give her the
  full palette." This supersedes three of T6's decisions at once: the closed core
  mark enum, its structurally-no-judging-variant shape, and the adapter's
  byte-pinned glyph table. The mark tool takes the emoji as its
  vocabulary parameter beside the message id; the core records it verbatim as the
  MessageMark block's content, exactly as it records answer text, and owns no
  emoji list — an emoji is content, not platform vocabulary. T6's frozen-
  vocabulary CHECK on the mark column dies with the enum; in its place the mark
  table's CHECK bounds length alone (non-empty, at most 32 bytes), the schema
  twin of the tool bound below. T6's target validation survives with one collapse: a call
  naming no id or an unknown origin is refused exactly as T6 pins it, and the
  assistant's own message is caught by the ANTI-AIMING decline before any other
  check — her voice writes no chat rows, so her message ids are never in the
  co-summoner set — so T6's DISTINCT own-message decline text is unreachable,
  is not built, and its pin dies with it — "bot messages are reactable"
  means OTHER bots' messages, which carry principals like any member's. The tool's model-facing name moves with the vocabulary: `mark_seen` no
  longer says what the tool does, so the tool is named `react`, its parameters
  the message id and the emoji — a model-facing string decided here, T6's
  naming superseded with the enum. Decision 0070 stands untouched: a reaction is
  expression, not a moderation effect, and no human decision point moves. The
  in-repo teaching adds no vocabulary restriction; the persona's emoji rules are
  the taste line (see the launch notes for how it ships without a gap).
- **The core bounds the mark content and accepts the silent drop, 2026-08-30.** The
  tool refuses an empty emoji argument and one longer than 32 bytes (every entry of
  the platform's reaction list fits, joined sequences included), with a teaching
  error naming the bound; within the bound the string is stored verbatim. A mark
  the platform cannot carry is dropped by the adapter with a log line and the model
  is never told — the tool has already returned, and an act whose whole point is
  being cheap earns no delivery report. The composite is stated plainly: a
  mis-picked emoji files the mark, the drop loses it, and the per-origin
  existence check then refuses every later attempt on that message — one bad
  pick permanently unmarks it, the same accepted permanence as the death-window
  loss. Stated as the accepted consequence.
  *Rejected:* echoing delivery back into the conversation, or recording the
  mark's delivery at all — a return path EXISTS since unit 38 (the Reply arm
  carries a delivery handle the adapter hands back), and the Mark arm omits it
  deliberately: a cheap act earns no bookkeeping row. Stated so nobody completes
  the symmetry.
- **The adapter's membership rule: one canonical list, selector-blind matching,
  the list's bytes on the wire, and the wire acceptance recorded as unproven,
  2026-08-30.** The adapter pins the platform's reaction list (seventy-three
  entries) as escape sequences — never glyph literals, T6's byte-hazard rule
  kept. The derivation rule for each entry's bytes, since the platform states no
  wire form in prose (T6's own grounding): the form the platform's page carries
  as the entry's image alt text, taken as-is — bare U+2764 for the heart, the
  FE0F-less and ZWJ forms where that is what the page carries. That those bytes
  are accepted at the wire is the unit's THIRD recorded unproven platform
  inference, joining T6's two (its ledger extends): the accepted drop and the
  platform's send refusal contain the failure, the drop log line is the
  observation channel, and a logged refusal pattern corrects an entry's bytes as
  a follow-up. Matching is selector-blind: a model-chosen emoji matches an entry
  when the two are equal after removing every variation selector (U+FE0F) from
  both sides; on a match the adapter sends the LIST's bytes, never the model's.
  No match: the mark is dropped and logged (the accepted silent drop above).
  Custom-emoji reactions are structurally out: the adapter never sends a
  custom-emoji parameter, and the request-shape pin under AC-D is what exercises
  that claim. *Rejected:* exact byte equality against a pasted list — a
  copy-paste silently gains or loses U+FE0F and legal reactions would drop
  invisibly; *rejected:* trimming the byte-ambiguous entries — the operator
  ordered the full palette; *rejected:* probing the live platform to prove the
  bytes first — the drop log observes the same fact without depending on an
  unproven probe path.
- **The privacy documents move with the palette, and their content is decided
  here, 2026-08-30.** T6's AC14 pinned document content the palette falsifies (a
  named single mark, a no-negative-marks DPIA rationale); this unit replaces its
  PALETTE-FALSIFIED content and keeps its surviving obligations — the erasure
  row, the retention clause, and the recipients statement T6's un-superseded
  erasure decision mandates. Four sites. The records of processing gain
  collection row D11 (main's section 5 ends at D10), carrying these facts in the
  table's own voice: the emoji the assistant chose, the marked message's
  reference, the marked member's internal identifier (the same datum D7 names
  for reports — the mark table's principal column survives from T6), and the
  time. The same document's erasure section gains the D11 row with these facts,
  precise to the mechanism (T6's AC10 nulls the message reference alone): the
  marked person's erasure empties the mark's stored message reference — the row
  survives with the erasure-keyed principal identifier and the emoji, and an
  emptied mark states nothing to the model (the D10 row's honesty pattern); an
  administrator's deletion of the marked message empties the reference through
  the mirror pass; the visible reaction on the platform is not withdrawn — the
  stated residual. The retention fact rides where the document states retention:
  the mark record lives exactly as long as the message record beside it. The
  recipients statement: the chosen emoji travels to the platform with the send
  and to nobody new. Table rows are pinned on their facts the way the document's
  tests pin its claims; the two free-standing sentences below are pinned byte
  for byte. The privacy policy's plain-language list gains:
  `The assistant may put an emoji reaction on a message; the emoji it chose is stored with that message's record.`
  The impact assessment's reactions passage replaces the no-negative-mark
  rationale with:
  `A reaction is expression, not enforcement: it changes nobody's standing, rights or access, and every moderation effect keeps its human decision point. The palette includes negative emojis; choosing one is a conduct matter governed by the deployed persona, with no data-protection effect beyond the stored choice itself.`
  The DPIA's review trigger names reactions, so its review note is dated with
  this unit.
- **The two core-cleanliness checks are defined here, buildable against today's
  tree, 2026-08-30.** T6 AC2's non-ASCII scan does not exist yet and, as T6 wrote
  it, would fail on main (the search guard legitimately holds confusable
  character literals, and fixtures hold Cyrillic and umlaut text). Decided: the
  scan this unit builds reads PRODUCTION core source with test modules excluded;
  its allowlist is enumerated in the check itself against the tree at merge,
  each entry with the reason it belongs (the punctuation actually present, plus
  the search guard's padding-separator literals — the guard's Cyrillic sits in
  its test module, which the scan already excludes); and a deliberately-failing fixture proves the scan
  bites. The second check greps production core source for emoji escape
  sequences in the ranges U+1F000-U+1FAFF and U+2600-U+27BF plus U+FE0F and
  U+200D — the joiner is the one palette codepoint outside the ranges, and the
  guard's format-control escapes still lie outside the whole set — and it too
  carries a failing fixture. *Rejected:* T6's allowlist verbatim — it fails on the tree it merges
  into; *rejected:* scanning fixtures — test text legitimately speaks other
  scripts.
- **The deployment persona gains one sentence about reactions, as deployment work,
  2026-08-30.** The persona's emoji rules (match, never a sign-off, no repeats)
  extend naturally to reactions; the persona is the deployment's file, so that
  sentence ships with the deploy that ships this unit — prepared in the deployment
  repository beforehand so no live window carries the palette without its taste
  line (launch notes).
- **The edge's recorded losses carry over, restated, 2026-08-30.** A mark
  undelivered at process death is lost (the edge's cursor discipline), and under
  the per-origin existence check that message stays permanently unmarked —
  accepted for an act whose whole point is being cheap.
- **Bot messages are reactable; join notices are not reactable on the platform,
  on the operator's answer, 2026-08-30.** Verbatim: "She can react to bot messages
  too. Join messages aren't reactable at least i cant put a reaction on tg's system
  join message." So no exclusion clause exists anywhere: a bot's message may draw a
  mark like anyone's. A join notice's origin is a platform service message the
  platform will not decorate (operator-observed); no NEW refusal is built for it —
  the mark tool's surviving origin validation already declines a join origin,
  because the aiming check reads chat messages and a join notice is its own
  block kind, never among them. The platform fact is recorded as the operator's
  observation; nothing depends on it. *Rejected:* a dedicated join-target
  clause — the existing validation IS the refusal, and a second one would record
  the same decision twice.

## Acceptance criteria

The sixteen criteria of T6 bind the mechanism, re-read against today's tree, with
these exceptions: every criterion or criterion-part pinning the closed enum, the
no-judging-variant structure, the glyph table, or the single named mark's wire form
is superseded (AC-D and AC-E replace them; AC4's block-append half survives and is
re-pinned under AC-D, its request-shape pin re-created among AC-D's membership
pins); T6's AC2 allowlist is superseded by the cleanliness decision's enumerated
scan; T6 AC5's own-message decline pin dies with the collapse the palette
decision records; T6's AC14 is replaced whole by AC-E; and T6's AC16 — the decision-file
recording of the closed enum and the no-negative-mark shape as standing decisions
— is replaced whole by AC-C, whose records supersede exactly those statements. T6's trigger rule lives
in its decisions, not its criteria, and is superseded by the trigger decision above.

- **AC-A** The amended silence sentence and the carve-out are pinned byte for byte
  as written in the decision, and the mark tool's own teaching states: chatter that
  lands may draw one reaction instead of an empty turn; words and a reaction never
  land on one message.
- **AC-B** The conduct line change is pinned: the casual-share sentence names the
  reaction, and no teaching sentence still directs a short text reply at chatter.
- **AC-C** The decision records land numbered from the highest shipped at merge,
  the trigger inversion, the sentence amendment (distinguishing 0148), and the
  palette supersession each recording the superseded rule with its date.
- **AC-D** The mark tool takes the emoji as its vocabulary parameter beside the
  message id (T6 AC5's target validation passing unchanged); the core refuses
  empty and over-32-byte emoji arguments with the taught error (pins), stores an
  accepted one verbatim on the block (pin), and the mark table's CHECK bounds
  length (schema pin); the core holds no emoji list — enforced by the two
  checks the cleanliness decision defines, each proven by its failing fixture;
  the adapter's membership rule is pinned five ways: the model's bare heart and
  its selector-carrying form both map to the list's bare-heart entry (the
  heart-on-fire entry stays distinct under its joiner), the WHOLE list is asserted — all
  seventy-three entries' bytes and the count itself, the byte-hazard rule read
  against a palette where every entry is sendable (a dropped, transposed or
  selector-polluted entry must fail a test, not surface as invisible drops) — the request
  shape is pinned (one send, a one-element emoji-typed reaction array — no
  custom-emoji parameter can be built), an out-of-list emoji is dropped without
  a platform call and logged, and the sent bytes are the list's, not the
  model's.
- **AC-E** The privacy-document changes land as the privacy decision states:
  the D11 collection row, the D11 erasure row, the retention fact and the
  recipients statement pinned on their facts; the plain-language line and the
  impact-assessment passage pinned byte for byte, the passage keeping the two
  surviving facts the palette never falsified (the assistant collects nobody
  else's reactions, on the administrator-requirement grounds T6 recorded, and
  the stored mark record travels to no processor beyond the platform send
  itself); the DPIA review note dated.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-reactions`, branch `unit/reactions`,
  from `main` (`e4222a7`). First step of the build: `git rebase main`.
- The build's implementer reads T6 in full alongside this doc; anchors are grepped,
  never trusted.
- Every decision is settled; the build launches after one more unbriefed round
  clears this revision.
- The persona sentence is committed to the deployment repository before the deploy
  that ships this unit, so the palette and its taste line go live together; the
  push itself waits on the operator's deploy approval as always. The sentence
  must also steer the choice toward the platform's REACTION set: the persona's
  chat-emoji vocabulary lies mostly outside the 73 reactions, and a non-list
  choice is silently dropped — the taste line naming the reaction set is what
  keeps her picks placeable.
- The quality bar from the operator, verbatim scope for the reviewers: "The code
  must always be better and cleaner afterwards than it was before. If you had to
  add a snowflake if somewhere, it's a smell."
