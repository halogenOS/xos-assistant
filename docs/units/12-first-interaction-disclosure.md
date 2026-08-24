# Unit 12 — the first answer discloses the machine

Date: 2026-08-23. Revision 1. The Act's transparency duty, verified against the
official texts: the system itself must inform each natural person, clearly, at
the latest at their first interaction — a profile field or a policy page is not
the system informing anyone (the guidelines rule that placement out by name).
The operator approved the shape: the assistant's first answer to each person
carries a one-line disclosure, which is also good hosting.

## Decisions taken with this unit

- **The disclosure rides the first answer to each person, from the ledger's own
  memory, 2026-08-23.** The fact "has this person ever been answered here" is
  ledger-derivable: at answer delivery, the outbound edge prepends the fixed
  disclosure line to the first answer whose summoning person has no earlier
  answered turn in the store — resolved through the same debt-origin machinery
  every per-person fact uses, with several co-summoners each checked and the
  line shown if ANY of them is new. Per person, not per conversation: the duty
  attaches to the natural person. A person returning after full deletion is a
  new person to the store and gets the line again — correct in both directions
  (the store genuinely does not know them; the duty resets with the erased
  memory). No table, no flag: the ledger IS the memory. Rejected: a
  disclosed-flag on the identity row (a second copy of a derivable fact, and
  erasure would have to decide what deleting it means); a per-conversation
  line (para 143 attaches the duty to the person); disclosure as its own
  message (two sends where one line does it, and a flood surface).
- **The line's copy, child-appropriate, 2026-08-23.** The fixed constant, exact
  copy: `I am Xenia, an AI assistant. My answers are written by a machine and
  can be wrong.` — followed by a blank line, then the answer. Plain words a
  young reader understands, per the guidelines' accessibility note. The
  deterministic replies (privacy commands, acknowledgments) carry no
  disclosure: they are fixed lines a human operator wrote, not model output,
  and burdening a rights reply with it would blur what it marks. (Refined
  2026-08-24, unit 20: the rules acknowledgment is now model output — a
  bounded one-shot completion with the fixed line as its fallback — and still
  carries no disclosure because it rides the observation return, never the
  answer edge, not because a human wrote it.) Rejected:
  legalese; a disclosure on deterministic lines (nothing machine-generated to
  mark).
- **The model answers the question honestly, 2026-08-23.** The prompt gains
  the teaching: asked whether it is an AI, a bot or a machine, the assistant
  says yes plainly and never claims to be human. Prompt-level is the right
  layer — the question arrives in free text and the answer is conversation.
- **The written artifacts, 2026-08-23.** A tracked compliance page
  (docs/compliance/ai-act.md): the provider-role conclusion with its grounds,
  the obligations map (what applies, what does not, article-cited), the
  Art 50(2) position — upstream text marking relied on per the permitted
  path, with the due-diligence check's result recorded, and the under-200-
  token practice bound noted — the detection route, the gap analysis in
  place of the Code of Practice, and the Art 4 literacy note (the operator
  built and operates the system; the documents in this repository are the
  ongoing literacy record). The DPIA's role paragraph corrects from deployer
  to provider with a dated note.

## The unit's contract

The disclosure constant with exact copy; the first-answer resolution at the
outbound edge (ledger-derived, person-keyed, co-summoner-inclusive); the
prompt teaching; the compliance page; the DPIA correction. No configuration,
no adapter change, no new table.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** The first answer to a new person carries the line then the answer;
  their second answer carries no line; a second new person in the same
  conversation gets the line on their own first answer; an absorbed
  co-summoner who is new gets it counted (the line shows); a person returning
  after deletion gets the line again — pinned block by block and over the
  wire.
- **AC3** Deterministic replies carry no disclosure — pinned.
- **AC4** The prompt teaching ships; the compliance page exists with the
  role conclusion, the obligations map, the marking position and the two
  notes; the DPIA correction is dated — pinned in the docs test.

## Settled with the operator, 2026-08-23

Two amendments from the operator's own design review, binding over the letter
above:

1. **The line's copy is the operator's, verbatim:** `Hi, I'm Xenia, the
   halogenOS Assistant Bot, an AI system, made to assist members of the
   community.` It replaces the earlier draft copy; the fallibility note lives
   in the bio and the policy, and the line's one job is the disclosure.
2. **The line is stored, not added at delivery:** it is prepended into the
   final answer block itself, so the ledger carries what the chat saw and the
   model sees in history that this person was already introduced. The
   guarantee stays mechanical — the prepend happens where the answer
   finalizes, never by model judgment; the natural-conversation side stays
   with the prompt's honest self-identification teaching. The operator's
   second variant (a system nudge making the model weave the intro) was
   REJECTED for the duty itself: a disclosure that depends on model obedience
   is advice, not a mechanism — the human-decides rule's own reasoning.
