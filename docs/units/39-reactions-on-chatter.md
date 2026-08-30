# Unit 39 — a reaction where a reply would add nothing

Date: 2026-08-30. The operator's queue entry, verbatim: "Add reactions to the queue to
reduce the amount of terminal responses to off topic", confirmed as: the assistant
reacts to a message with an emoji reaction instead of sending a short reply where a
reply adds nothing — mostly the off-topic chatter that today draws either a low-value
text answer or total silence.

## Governing design, and what this unit changes about it

The machinery is already designed: `docs/units/telegram/06-reactions.md` (revised
after two independent reviews, 2026-08-25, unbuilt) specifies the mark tool, the
closed core mark enum with structurally no judging variant (decision 0070's
extension), the adapter-only glyph translation with byte-pinned escape sequences, the
outbound edge's second arm, the MessageMark block kind with its erasure and mirror
passes, the per-origin duplicate check, the zero rate-limit interaction (budgets
count opened debts, not acts), and the quarantine on unproven platform behaviors
(nothing merged may depend on direct-chat reactions or empty-array withdrawal).
WHERE BOTH SPEAK, T6'S WORDING GOVERNS THE MECHANISM — except the decisions this
unit explicitly supersedes below. T6 itself is not edited; its ~40 line anchors
predate units 31/36/37/38 and the implementer greps rather than trusts them (worked
examples: `OutboundReply` now `message.rs:573-599`; `Assistant::replies` now
`assembly.rs:1367-1372`; `consume_replies` now `driver.rs:906-944`; `deliverable_of`
now `outbound.rs:543-567`; `CONSUMED_UPDATE_TYPES` now `client.rs:174`).

## Decisions taken with this unit

- **The trigger is inverted from T6, on the operator's confirmed reading,
  2026-08-30.** T6 taught the mark for messages speaking TO the assistant and called
  it "noise on a message that was not speaking to the assistant" — the operator's
  confirmed intent is the opposite: the reaction's home is the unaddressed chatter,
  where the hardened silence teaching (unit b46d6ba) forbids words. T6's "where it
  fits" rule is superseded whole; the new teaching is this unit's load-bearing
  decision. Honesty note carried into the spec: in helpful mode every message
  summons, so the co-summoner aiming check admits overheard messages and the
  TEACHING is the real gate — stated, not claimed otherwise.
- **The silence teaching carves the reaction out deliberately and narrowly,
  2026-08-30.** The helpful-mode paragraph keeps its "no answer, no acknowledgment,
  no comment" rule for WORDS and gains the one exception: where the model would end
  an empty turn but the message genuinely lands (a share, a milestone, a joke that
  landed), it MAY place one mark instead — the mark replaces the empty-turn silence,
  never accompanies words on the same message, follows the it-must-fit bar, and
  silence remains the default when nothing fits. Decision 0098's silence-default
  intent is extended, not amended.
- **The conduct line that earns text replies moves to the mark, 2026-08-30.**
  `prompts/30-conduct.md` ("a casual share earns a short reaction") today teaches a
  short TEXT reply; after this unit the same sentence points at the mark tool, so
  the words that line used to spend become a reaction.
- **The mark vocabulary is the full palette, on the operator's answer, 2026-08-30.**
  The operator's answer, verbatim: "I didn't say only a positive set. Give her the
  full palette." This supersedes three of T6's decisions at once: the closed core
  mark enum, its structurally-no-judging-variant shape, and the adapter's byte-pinned
  glyph table. The mark tool takes the emoji itself as its one parameter; the core
  records it verbatim as the MessageMark block's content, exactly as it records
  answer text, and owns no emoji list — an emoji is content, not platform
  vocabulary. The adapter owns the platform clamp: the platform's allowed reaction
  set lives there as the one membership check, a mark outside it is dropped and
  logged without a send, and a chat-level refusal from the platform is the same
  accepted loss the edge already records for a death-window mark. Decision 0070
  stands untouched: a reaction is expression, not a moderation effect, and no human
  decision point moves. The teaching adds no vocabulary restriction — the persona's
  existing emoji rules (match the message, never a sign-off) are the taste line,
  shipping with the deploy per the persona decision below.
- **The deployment persona gains one sentence about reactions, as deployment work,
  2026-08-30.** The persona's emoji rules (match, never a sign-off, no repeats)
  extend naturally to reactions; the persona is the deployment's file, so that
  sentence ships with the next deploy, outside this repository's fence.
- **The edge's recorded losses carry over, restated, 2026-08-30.** A mark
  undelivered at process death is lost (the edge's cursor discipline), and under the
  per-origin existence check that message stays permanently unmarked — accepted for
  an act whose whole point is being cheap.

## Acceptance criteria

The sixteen criteria of T6 bind the mechanism, re-read against today's tree, MINUS
its trigger-teaching criterion and MINUS every criterion pinning the closed enum,
the no-judging-variant structure, or the adapter glyph table (all superseded by the
palette decision; AC-D replaces them), PLUS:

- **AC-A** The silence teaching's carve-out is verbatim-pinned beside the hardened
  paragraph, and the mark tool's own teaching states: chatter that lands may draw
  one mark instead of an empty turn; words and a mark never land on one message.
- **AC-B** The conduct line change is pinned: the casual-share sentence names the
  mark, and no teaching sentence still directs a short text reply at chatter.
- **AC-C** The decision records land numbered from the highest shipped (0151+),
  the trigger inversion and the palette supersession each recording T6's superseded
  rule with its date.
- **AC-D** The mark tool's one parameter is the emoji; the core stores it verbatim
  on the block (pin) and holds no emoji list (the vocabulary scan stays clean and
  no reaction set appears in the core); the adapter's membership check is pinned
  both ways: an allowed emoji is delivered as the reaction, one outside the set is
  dropped without a platform call and the drop is logged.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-reactions`, branch `unit/reactions`, from
  `main` (`e4222a7`). First step of the build: `git rebase main`.
- The build's implementer reads T6 in full alongside this doc; anchors are grepped,
  never trusted.
- The quality bar from the operator, verbatim scope for the reviewers: "The code
  must always be better and cleaner afterwards than it was before. If you had to
  add a snowflake if somewhere, it's a smell."
