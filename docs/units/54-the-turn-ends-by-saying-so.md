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
registered in the assembly's unconditional-tools home — where the react, report and standing
tools already join, never the lookups set, whose three-name assertion stands:

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

**The teaching becomes one rule with three arms, in precedence order.** When a turn has
nothing to say: react if one reaction honestly answers chatter; otherwise call
`work_is_done` if the turn took actions and those actions are the whole answer — a mark
placed, a report filed, a lookup run for the assistant's own orientation and found to need
no telling; otherwise call `no_reply_needed` if the turn took no action and nothing was
asked of the assistant. The qualifier is part of the rule everywhere it is written: a search
or lookup run TO ANSWER someone still ends in the written answer, so the search and sourcing
teaching sentences stand unchanged and collide with neither arm. The closing call is the
turn's only output — prose written before a tool call is posted to the group as its own
message, by the mechanism the heads-up line rides, so a narrated close would repeat the
production failure with extra steps; the teaching says to call the tool bare, and the
mechanism is stated here because no machinery can forbid it. Never post a message whose only
content is that the assistant is not participating. The empty turn stays valid and stays
taught as the fallback.

**Both answering modes teach both tools.** A registered tool is never left untaught, and the
recorded tool choice names the pair in every conversation. The helpful-mode teaching carries
the full three-arm rule; the addressed-mode silence sentence extends to name the pair for
its own two shapes — an addressed message that leaves nothing useful to say ends through
`no_reply_needed`, an addressed request completed by actions ends through `work_is_done`.
Each mode's wording stays its own; neither imports the other's triggers.

**What the tools do not do.** They take no parameters, read nothing, write nothing beyond
their resolution, and their `admit` answers the same authority hook every tool answers.
They are not moderation and never suppress someone else's delivery; they end only the
assistant's own turn.

## Acceptance criteria

1. Both tools exist, declare `ends_turn`, take no parameters, and answer the admission hook
   at member authority through the same macro every tool module carries — the admission scan
   test is what holds that, since a member-level answer is behaviorally identical to the
   default. A test reads the definitions and the `ends_turn` flag off the registry.
2. Calling either tool bare — no prose ahead of the call — ends the turn: the resolution row
   carries the framework's ends-turn stamp, no continuation is dispatched, and nothing is
   delivered to the channel. A spine test runs a narration-free scripted turn through each
   tool and asserts all three. Prose streamed ahead of the call delivers as its own message
   by the standing mechanism; that shape is taught against, not forbidden, and criterion 5's
   teaching assertions carry the sentence.
3. Each tool's stored result text is its module's byte-fixed sentence. A test asserts both
   sentences whole.
4. Both tools ride the registered set: a created conversation's recorded tool choice names
   them, and the compaction fork's empty choice keeps them out of a summary turn. Tests
   read both ledgers.
5. The helpful-mode teaching presents the three-arm rule in its precedence order with the
   orientation qualifier and the call-it-bare sentence; the addressed-mode silence sentence
   names the pair for its two shapes; the never-announce-silence sentence is present; and
   under the qualified wording no remaining teaching sentence tells the model to write when
   an arm applies — the search and sourcing sentences stand unchanged and are asserted
   unchanged. The teaching tests cover each mode's new wording.
6. The two tools register in the assembly's unconditional-tools home, beside the react and
   report tools, never in the lookups set. The test fixtures see them wherever they see the
   unconditional tools, and existing tests that count or enumerate registered tools pass
   updated, the three-lookup assertion untouched.
7. A turn that calls `no_reply_needed` while a sibling tool call of the same round is still
   unresolved follows the framework's rule: the stamped outcome does not hold the turn, the
   sibling's resolution still lands, and the sibling's continuation round is still summoned.
   A test covers the pair in one round and asserts all three.
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
