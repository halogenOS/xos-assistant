# Unit 21 — the assistant reads its audience and asks before it assumes

Date: 2026-08-24. Revision 1. A community question can come from an END USER who
wants to USE the ROM, or from a BUILDER/DEVELOPER who wants to IMPLEMENT or
integrate something — and those are different questions with different answers.
Shown "How do I use sandboxed play in xos", the assistant found the right wiki
page but answered it as if the asker were building a ROM ("If you're looking to
integrate Sandboxed Google Play into your build… use the corresponding branch…
merge or cherry-pick the commits…"), locking into the developer reading of an
end-user question. This unit teaches the assistant to notice the ambiguity and
ask ONE short clarifying question instead of committing to an assumption.

## Decisions taken with this unit

- **The assistant distinguishes using from building, and asks when the question
  does not say which, 2026-08-24.** The answering teaching gains the distinction:
  many questions read one way to an end user (how do I use this on my phone) and
  another to a developer (how do I integrate this into a build), and the right
  answer differs sharply. When a question is genuinely ambiguous about which, the
  assistant asks ONE brief clarifying question — "are you asking how to use it on
  your device, or how to build it into a ROM?" — and stops there, rather than
  assuming a reading and delivering a locked-in answer down the wrong track. When
  the intent IS clear from the question or the context, it answers directly: the
  clarifying question is for real ambiguity, not a reflex on every message.
  Rejected: always answering the most technical reading (the live failure — it
  buries an end user in build instructions); always asking a clarifying question
  (annoying, and most questions are not ambiguous — the assistant should read the
  room and only ask when it genuinely cannot tell).
- **A clarifying question is a full, grounded-discipline-exempt answer,
  2026-08-24.** Unit 16 makes an ungrounded or no-help turn abstain or miss. A
  clarifying question makes no substantive claim about the project — it asks the
  member which question they are asking — so it needs no lookup, does not abstain,
  and does not emit the miss sentinel: it is a genuine, warranted reply. The
  teaching states this so the grounded-answer discipline does not swallow a
  clarifying question as "no grounded answer available". The member's reply then
  arrives as the next message and the assistant answers the now-disambiguated
  question (with the lookup discipline applying to that real answer). Rejected:
  treating a clarifying question as an ungrounded answer (unit 16 would swallow
  it — the exact wrong outcome).
- **The teaching is audience-aware without inventing facts about the member,
  2026-08-24.** The assistant reads the audience from what the message and the
  conversation actually show — the words used, the level of the question, prior
  turns — not from guesses about who the person is. It does not assert the member
  is a developer or a user; it recognizes when the question itself is ambiguous
  between the two and asks. This keeps the grounding discipline intact (no
  invented facts) while fixing the assume-and-lock behavior. Rejected: profiling
  the member (guessing a person's expertise is the same ungrounded-assertion
  failure the project avoids elsewhere).

## The unit's contract

The composed answering teaching (`teaching.rs`, and the base prose where the
answering register lives) gains: the use-versus-build distinction; the rule to
ask ONE brief clarifying question on genuine ambiguity rather than assume; the
rule to answer directly when intent is clear; and the statement that a clarifying
question is a warranted reply that does not abstain or miss and needs no lookup.
No mechanism change — this is a teaching/prompt change; the abstention sentinel,
the miss routing, the lookup discipline and every other mechanism are unchanged.
No configuration, no new dependency.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency.
- **AC2** The teaching text carries, verbatim-pinned: the use-versus-build
  distinction, the ask-one-clarifying-question-on-ambiguity rule, the
  answer-directly-when-clear rule, and the clarifying-question-is-warranted (no
  abstain/miss, no lookup) rule. Present in the composed helpful teaching (and the
  addressed teaching where it applies).
- **AC3** An ambiguous question draws a clarifying question, not a locked-in
  answer: a scripted turn on a genuinely ambiguous use/build question delivers the
  model's clarifying question (a real reply, not swallowed by abstention/miss) —
  pinned; and the follow-up disambiguating reply is absorbed and answered on the
  next turn (the existing absorption path, unchanged).
- **AC4** A clear question is answered directly, not interrogated: a scripted turn
  on an unambiguous question delivers the answer with no clarifying question —
  pinned (the clarifying behavior does not fire on every message).
- **AC5** No mechanism regressed: the grounded-answer/abstention/miss discipline
  (unit 16), the disclosure and the report machinery behave as their units pinned
  them — the relevant prior pins pass unchanged; the clarifying question is not
  routed through the miss/abstain path.

## Notes for launch

- Branches from main AFTER U-ACK merges (both touch `teaching.rs` / the prompt).
- Primarily a teaching change; a light cold probe is worthwhile to confirm the
  clarifying-question path does not collide with unit 16's abstention/miss
  discipline (a clarifying question must not read as "no grounded answer").
