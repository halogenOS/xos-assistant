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
unit explicitly supersedes below. T6 itself is not edited; its anchors (roughly
ninety of them) predate units 31/36/37/38 and the implementer greps rather than
trusts them (worked examples: `OutboundReply` now `message.rs:573-599`;
`Assistant::replies` now `assembly.rs:1367-1372`; `consume_replies` now
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
  Both strings are pinned byte for byte. *Rejected:* a carve-out sentence beside
  the untouched rule — the composed prompt would contradict itself on a literal
  read, the exact collision 0148 documents.
- **The conduct line that earns text replies moves to the mark, 2026-08-30.**
  `prompts/30-conduct.md:94` ("a casual share earns a short reaction") today
  teaches a short TEXT reply; after this unit the same sentence points at the
  reaction tool, so the words that line used to spend become a reaction.
- **The mark vocabulary is the full palette, on the operator's answer, 2026-08-30.**
  The operator's answer, verbatim: "I didn't say only a positive set. Give her the
  full palette." This supersedes three of T6's decisions at once: the closed core
  mark enum, its structurally-no-judging-variant shape, and the adapter's
  byte-pinned glyph table. The mark tool takes the emoji itself as its one
  parameter; the core records it verbatim as the MessageMark block's content,
  exactly as it records answer text, and owns no emoji list — an emoji is content,
  not platform vocabulary. Decision 0070 stands untouched: a reaction is
  expression, not a moderation effect, and no human decision point moves. The
  in-repo teaching adds no vocabulary restriction; the persona's emoji rules are
  the taste line (see the launch notes for how it ships without a gap).
- **The core bounds the mark content and accepts the silent drop, 2026-08-30.** The
  tool refuses an empty emoji argument and one longer than 32 bytes (every entry of
  the platform's reaction list fits, joined sequences included), with a teaching
  error naming the bound; within the bound the string is stored verbatim. A mark
  the platform cannot carry is dropped by the adapter with a log line and the model
  is never told — the tool has already returned, and an act whose whole point is
  being cheap earns no delivery report. Stated as the accepted consequence.
  *Rejected:* echoing delivery back into the conversation — a second write for a
  cheap act, and T6's edge design has no return path from the send.
- **The adapter's membership rule: one canonical list, selector-blind matching, the
  platform's bytes on the wire, 2026-08-30.** The adapter pins the platform's
  documented reaction list (seventy-three entries) as escape sequences — never
  glyph literals, T6's byte-hazard rule kept — each entry in the byte form the
  platform documents. A model-chosen emoji matches an entry when the two are equal
  after removing every variation selector (U+FE0F) from both sides; on a match the
  adapter sends the LIST's bytes, never the model's, so both heart forms map to the
  one wire form the platform expects. No match: the mark is dropped and logged
  (the accepted silent drop above). Custom-emoji reactions are structurally out:
  the adapter never sends a custom-emoji parameter. A chat that restricts its
  available reactions refuses at the send; that refusal is the same accepted loss
  the edge already records for a death-window mark. *Rejected:* exact byte
  equality against a pasted list — a copy-paste silently gains or loses U+FE0F and
  legal reactions would drop invisibly; *rejected:* sending the model's bytes — the
  platform's accepted form is documented per entry and the list is the one place
  that records it.
- **The privacy documents move with the palette, and their sentences are decided
  here, 2026-08-30.** T6's AC14 pinned document content the palette falsifies (a
  named single mark, a no-negative-marks DPIA rationale); this unit replaces it as
  AC-E. The records of processing gain row D11 (main's section 5 ends at D10, the
  join-notice names): the assistant's chosen reaction emoji, stored with the
  marked message's id and time — model-authored content, referencing no member
  data beyond the already-recorded message. The privacy policy's plain-language
  list gains:
  `The assistant may put an emoji reaction on a message; the emoji it chose is stored with that message's record.`
  The impact assessment's reactions passage replaces the no-negative-mark
  rationale with:
  `A reaction is expression, not enforcement: it changes nobody's standing, rights or access, and every moderation effect keeps its human decision point. The palette includes negative emojis; choosing one is a conduct matter governed by the deployed persona, with no data-protection effect beyond the stored choice itself.`
  The DPIA's review trigger names reactions, so its review note is dated with this
  unit. Exact placement follows each document's own structure; the sentences above
  are the copy, pinned where each document's tests already pin its claims.
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
  platform will not decorate (operator-observed); no structural refusal is built
  for it — the carve-out already aims marks at messages that land, a join line is
  not one, and if the model ever aims at a join origin anyway the platform's
  refusal at the send is the same accepted, logged loss the edge records for every
  undeliverable mark. *Rejected:* a tool-side refusal for join targets — a special
  case for a call the platform already answers, on a behavior this repo records as
  operator-observed rather than proven.

## Acceptance criteria

The sixteen criteria of T6 bind the mechanism, re-read against today's tree, with
these exceptions: every criterion or criterion-part pinning the closed enum, the
no-judging-variant structure, the glyph table, or the single named mark's wire form
is superseded (AC-D and AC-E replace them; AC4's block-append half survives and is
re-pinned under AC-D); T6's AC14 is replaced whole by AC-E. T6's trigger rule lives
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
- **AC-D** The mark tool's one parameter is the emoji; the core refuses empty and
  over-32-byte arguments with the taught error (pins) and stores an accepted one
  verbatim on the block (pin); the core holds no emoji list — pinned by a committed
  check that greps `crates/core/src` for emoji-range escape sequences as well as
  the surviving non-ASCII scan, so an escape-written list cannot hide; the
  adapter's membership rule is pinned three ways: both heart byte forms map to the
  one wire form, an out-of-list emoji is dropped without a platform call and
  logged, and the sent bytes are the list's, not the model's.
- **AC-E** The three privacy-document changes land with the sentences decided
  above: the D11 row, the plain-language line, and the impact-assessment passage,
  each pinned the way its document's existing claims are pinned, and the DPIA
  review note is dated.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-reactions`, branch `unit/reactions`,
  from `main` (`e4222a7`). First step of the build: `git rebase main`.
- The build's implementer reads T6 in full alongside this doc; anchors are grepped,
  never trusted.
- Every decision is settled; the build launches after one more unbriefed round
  clears this revision.
- The persona sentence is committed to the deployment repository before the deploy
  that ships this unit, so the palette and its taste line go live together; the
  push itself waits on the operator's deploy approval as always.
- The quality bar from the operator, verbatim scope for the reviewers: "The code
  must always be better and cleaner afterwards than it was before. If you had to
  add a snowflake if somewhere, it's a smell."
