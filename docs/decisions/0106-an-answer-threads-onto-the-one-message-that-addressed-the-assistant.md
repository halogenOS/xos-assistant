# 0106 — An answer threads onto the one message that addressed the assistant

Date: 2026-08-24. The answer-threading decision decisions 0018 and 0059 left open.

## Context

An answer arrives in a group as a loose message with nothing tying it to the
question. Decision 0018 refused to thread it because the only target available then
would have been a guess, and decision 0059 built the plumbing — an optional reply
target on the outbound reply, translated by the adapter — while keeping answers
plain until this decision.

Two facts about the ledger settle what the target can be. The first: a turn does not
record who asked. Every block a turn writes carries the id of the summoning frontier,
which is whatever line the dispatch woke on — a warm-up line, an unrelated message
after a dead batch, a bystander re-engaging. Threading onto it quote-replies the
wrong member routinely. The second: what a turn does record is which messages it
absorbed, and each message stores whether the person literally addressed the
assistant, beside the summons the answering mode folds into.

## Decision

An answer is delivered as a reply to the one message the turn absorbed whose stored
literal-addressed fact is true, and only when exactly one is. The turn's absorbed
messages are read through the same walk the tool admission and the report's aiming
already use, over the ledger the delivery edge has loaded. None means nobody
addressed the assistant; several mean the turn answered a crowd, and naming one tells
the others they were ignored. Both send the answer plainly. So does an addressed
message whose origin an erasure nulled, and any message recorded before origins were
stored. In no case is an answer withheld or delayed by the absence of a target.

The lookup lives in the delivery loop and not in the block reading beside it: that
reading is pure over one block and holds no ledger, while the message an answer
answers is a fact about the turn around it. The core names the target as the
platform-neutral origin already stored on the message; the adapter translates it, as
it does for the report's delivery.

A direct chat is unscoped by this rule and threads under it like any other channel,
because a one-to-one conversation is addressed by definition. Nothing new is rendered
to the model, no new stored fact, no new configuration.

## Rejected alternatives

- **Threading onto the dispatch anchor.** The first revision of this unit did, and a
  probe over real turns showed the anchor is the frontier: three members talking and
  one turn answering one of them anchors on the oldest warm-up line, and a dead batch
  followed by an unrelated message anchors on that unrelated speaker. Decision 0018's
  objection is not spent — it is inverted.
- **Threading onto the newest addressed message when several addressed.** The same
  guess decision 0018 refused, wearing a different hat.
- **Reading the stored summons instead of the literal fact.** Helpful answering
  summons the assistant for every message, so the summons is true for people who
  never addressed it, and the rule would quote-reply bystanders in exactly the
  channels helpful answering exists for.
- **Rendering the reply link into what the model reads.** Dropped whole: a deletion
  request keeps its own reply reference by decision 0085, so an erased message's
  origin would print on the next line, the erasure residual would be exported to the
  vendor on every request, and with ids visible a model naming the reply target files
  a report against an innocent message — proven against real runs, not theorised.
