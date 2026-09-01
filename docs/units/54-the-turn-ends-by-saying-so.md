# Unit 54 — the turn ends by saying so

Date: 2026-09-02. The assistant gains the two turn-ending tools the framework's park
capability was built for: one for a turn that was asked nothing, one for a turn whose
actions are complete. Calling either ends the turn with no message posted, and the teaching
stops the assistant announcing its own silence.

The repository: this one, at `9ff182e`. The framework at `b7c0c45` already carries the whole
mechanism (its park slice, 2026-08-30): `ToolHandler::ends_turn`, the resolution-row stamp,
the release rule, the refusal-run interaction. This unit registers consumers of it and
changes no framework code.

## What is true today

1. **The capability sits unused.** No handler in this repository overrides `ends_turn`; a
   grep returns nothing. The framework's park slice names the intended consumers: "a
   do-nothing, a no-reply-needed — its choice".
2. **The model's only silence is the empty turn.** The teaching says to end the turn empty
   when nothing is called for, and an empty answer is stored and delivered as nothing. A
   model bred to act keeps acting anyway: in production it answers a question aimed at
   someone else with an announcement that it is staying out — the exact behaviour the park
   capability exists to absorb.
3. **A reaction is the taught alternative to the empty turn** for chatter that merely lands.
   The teaching currently frames the choice as reaction-or-empty; the tools become a third
   arm and the teaching must stay one coherent rule.
4. **Registration is direct.** Tools implement the framework trait, register through the
   tool set, answer the admission hook at their required authority, and ride every
   conversation's recorded tool choice automatically.
5. **Threading is derived, never chosen.** An answer threads onto the one absorbed message
   that literally addressed the assistant; a turn ended by a park tool posts nothing, so
   threading never runs.

## The design

**Two tools, one capability.** Both declare `ends_turn`, both at member authority, both
registered unconditionally beside the production set:

- `no_reply_needed` — the turn was asked nothing. The absorbed messages are someone else's
  conversation, a question aimed at a named person who is not the assistant, or chatter that
  needs no acknowledgment beyond what a reaction already gave. Calling it says: nobody here
  is waiting on me.
- `work_is_done` — the turn acted and the actions are the whole answer. A report was filed,
  a reaction was placed, a lookup ran for the assistant's own orientation; a closing message
  would add words to a finished thing. Calling it says: what I did is complete, and prose
  would only narrate it.

**The stored close.** Each resolution stores a one-line result text, byte-fixed in the tool's
module and asserted by test, so the ledger reads why the turn ended:
`no_reply_needed` stores `Turn ended: no reply was needed.` and `work_is_done` stores
`Turn ended: the actions taken are the whole answer.` The model reads that row on the next
turn's replay, so the sentence addresses the model.

**The teaching becomes one rule with three arms.** When a turn has nothing to say: react if
one reaction honestly answers chatter, call `no_reply_needed` if nothing was asked, call
`work_is_done` if actions completed the turn — and never post a message whose only content
is that the assistant is not participating. The empty turn stays valid and stays taught as
the fallback; the tools are the explicit form of the same decision, and a model that would
rather act than write now has an action to take. No teaching sentence contradicts another;
the reaction arm keeps its current wording.

**What the tools do not do.** They take no parameters, read nothing, write nothing beyond
their resolution, and their `admit` answers the same authority hook every tool answers.
They are not moderation and never suppress someone else's delivery; they end only the
assistant's own turn.

## Acceptance criteria

1. Both tools exist, declare `ends_turn`, take no parameters, require member authority, and
   answer the admission hook the way every registered tool does. A test reads the
   definitions and the flags off the registry.
2. Calling either tool ends the turn: the resolution row carries the framework's ends-turn
   stamp, no continuation is dispatched, and nothing is delivered to the channel. A spine
   test runs a scripted turn through each tool and asserts all three.
3. Each tool's stored result text is its module's byte-fixed sentence. A test asserts both
   sentences whole.
4. Both tools ride the registered set: a created conversation's recorded tool choice names
   them, and the compaction fork's empty choice keeps them out of a summary turn. Tests
   read both ledgers.
5. The teaching presents the three-arm rule and the never-announce-silence sentence, and no
   remaining teaching sentence tells the model to write when one of the arms applies. The
   teaching tests cover the new wording.
6. The production set registers both unconditionally, and the test fixtures see them
   wherever they see the production tools. Existing tests that count or enumerate
   registered tools pass updated.
7. A turn that calls `no_reply_needed` while a sibling tool call of the same round is still
   unresolved follows the framework's rule (the stamped outcome does not hold the turn, the
   sibling's resolution still lands). A test covers the pair in one round.
8. Every check runs clean: `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and `cargo doc
   --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

## Rejected alternatives

- **One tool instead of two.** Rejected: the two name different facts — nothing was asked
  versus the asking is answered by deeds — and the ledger keeping which one the model meant
  is the point of recording decisions. The framework slice named the pair.
- **Retiring the empty turn now that tools exist.** Rejected: the empty turn is the
  framework's honest record of a model that ended with nothing, tool or no tool, and
  deleting a working mechanism to force another is churn. The teaching orders them; the
  machinery keeps both.
- **A parameter carrying the reason.** Rejected: the tool's identity is the reason, and a
  free-text parameter invites the model to write the very prose the tool exists to replace.

## Decisions on record

**2026-09-02, the order (verbatim):** "Before the idea please implement these two tools."

**2026-08-30, the framework slice:** "The consumer registers concrete tools on the
capability (a do-nothing, a no-reply-needed — its choice); the framework knows only the
property."

**Production evidence, 2026-09-01:** asked a question directed at another member by name,
the deployed assistant posted "A question directed at @xdevs23 (not me), so staying out of
it." — an announcement of non-participation, which this unit makes an action instead.
