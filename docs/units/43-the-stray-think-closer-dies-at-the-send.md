# Unit 43 — the stray think-closer dies at the send

Date: 2026-08-30. The live assistant once leaked a full reasoning trace: an
answer that read `...don't over-engage.</think>Haha, I'm an AI system...` went
to the chat whole, reasoning first. Two placements were weighed and the
sending layer won: the framework passes text through untouched, because a
model may emit the tag deliberately and the ledger keeps the model's own
words; the strip belongs in the app, at the edge where an answer becomes
wire text.

The cut rule was decided 2026-08-30, replacing this spec's earlier
last-stray-closer rule: a text containing exactly one closing think-tag
loses the tag and everything before it, and every other shape passes
untouched. The count replaces interpretation — no opener is read, nothing
guesses which closer ended a trace. Its accepted cost, stated plainly: an
answer legitimately mentioning the tag exactly once is indistinguishable
from the leak by count and is cut the same way; a shape that surfaces in
practice gets its own decision then instead of a guess now.

## What this unit builds

A strip at the APP's sending layer — the outbound edge in the core, where an
answer's text becomes the wire text — that removes a leaked reasoning prefix:
a model-authored ANSWER whose text contains exactly one closing think-tag
loses the tag and everything before it. The framework passes the
text through untouched (a model may output the tag on purpose, and the ledger
keeps the model's own words); the strip runs at the one edge that hands
the text to the platform, and it is idempotent there — at-least-once
delivery may run it again over the same block and the wire text is the
same.

## The rule, exactly

- The strip applies to `ReplyKind::Answer` texts ONLY — the model's own
  prose. The deterministic replies (the failure notice, the report line, the
  rules acknowledgment, the privacy answer) are fixed texts a person wrote
  and are never touched.
- A text containing EXACTLY ONE `</think>` loses that tag and everything
  before it; what follows the tag is the answer. The rule has no opener
  condition — one closer is the shape the live leak had, and the cut is
  unconditional.
- Any other count — zero closers, or two and more — leaves the text
  byte-identical. The rule covers exactly the observed leak shape; a shape
  not yet seen is a new decision, not a guess.
- The tag match is exact bytes `</think>` — no case folding, no attribute
  forms; the leak shape is the literal token the model emits. No opening
  tag is consulted anywhere.

## The wiring

The seam is the outbound edge's answer arm (`deliver_stored_items` in
`crates/core/src/outbound.rs`): the strip runs on the stored answer text
BEFORE the empty-answer judgment and before the disclosure resolution, so:

- an answer that is ALL reasoning — the strip leaves nothing, or only
  whitespace — takes the existing unit-22 silence path: accounted delivered,
  nothing sent, no disclosure resolved;
- the first-interaction disclosure line is prepended to the STRIPPED text,
  never buried behind a cut;
- the threading target and the moderation-command reading run on the text
  that goes out, exactly as today.

The stored block keeps the model's full text: the ledger and the model's
history carry what the model wrote, and only the wire text is stripped. This
deliberately narrows the one-text claim in the outbound module doc (and the
disclosure comment's "the ledger carries what the channel saw"): those
sentences are swept to say the channel sees the answer with any leaked
reasoning prefix removed. The strip itself is a pure function beside the edge
with its own unit tests; the edge calls it in one place.

## Acceptance criteria

- AC1: the exact leak shape — prose ending in `</think>` followed by the real
  answer — goes out as the real answer alone; pinned at the pure function AND
  through the edge (the spine's outbound suite shape).
- AC2: a clean answer (no closer) and an answer with two or more closers are
  byte-identical through the edge. Pinned.
- AC3: an all-reasoning answer yields silence: nothing sent, the answer
  accounted delivered — the existing empty-answer path, now reading the
  stripped text. Pinned.
- AC4: exactly one closer strips even when an opener precedes it — the count
  rule has no opener condition. Pinned at the function.
- AC5: the deterministic reply kinds are untouched by construction — the
  strip is called on the answer arm only; pinned by a test sending a fixed
  reply whose text contains the tag bytes and arrives whole.
- AC6: the checks pass: fmt, clippy with warnings denied, the full suite, the
  doc build, exit codes read bare.

## Bounds

- No framework change, no schema change, no new dependency.
- A decision record documents the send-layer decision with the rejected
  alternative (a framework strip — rejected because a model may emit the tag
  deliberately and the ledger must keep the model's words):
  `docs/decisions/0168-the-stray-think-closer-dies-at-the-send.md`.
- The module-doc sweep touches only the sentences the stored-vs-wire
  divergence falsifies.
