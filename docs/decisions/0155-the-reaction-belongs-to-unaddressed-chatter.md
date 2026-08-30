# 0155 — The reaction's home is unaddressed chatter, not the messages that speak to the assistant

Date: 2026-08-30, with unit 39.

## Context

The reaction machinery was designed before it was built. The design of 2026-08-25 taught the
mark for messages speaking TO the assistant where no answer was owed — a thank-you, a "that
worked", a correction it accepts — and it named the opposite case noise, in its own words
"noise on a message that was not speaking to the assistant".

That is the reverse of what the capability is for. The messages that speak to the assistant
already draw words when words are owed, and the hardened silence teaching forbids words on
everything else. So the case with nothing at all to spend on it is the unaddressed chatter,
and that is the case a reaction fits: the assistant read it, something landed, and there is
nothing to say.

## Decision

The reaction's home is the chatter that lands where the silence rule ends the turn empty.
The design's "where a mark fits" rule is superseded WHOLE — its fit clause, its noise clause
and its per-message bounds alike. What replaces it is the carve-out on the silence sentence
(decision 0156) and the tool's own teaching, which together carry every condition that
survives: chatter that lands may draw one reaction instead of an empty turn, words and a
reaction never land on one message, one message takes at most one reaction ever, and most
messages deserve none. No other fit rule exists anywhere, in the prompt or in a doc comment.

One honesty note rides with this, because a reader will otherwise assume the mechanism
narrows what the teaching says. It does not. In helpful answering every unlimited message is
a co-summoner, so the tool's aiming check admits a message the assistant merely overheard —
which is exactly the case this decision aims at, and also means the aiming check is not what
keeps a reaction off the wrong message. The TEACHING is the real control, and it is stated
as such rather than dressed up as a mechanism.

The other half of the same honesty, stated because the composite is easy to miss: the
window the model READS is wider than the set it may aim at, and two shapes it meets on
ordinary turns sit in that gap. A join notice is projected in the system voice with its own
bracketed id, and an unsummoned bot's message is projected as an ordinary user line —
decision 0153 leaves it recorded and projected while it summons nothing. Neither is a
co-summoner, so both take the anti-aiming decline, with no clause of their own: the aiming
check IS the refusal, exactly as it is for the assistant's own words. This is also why a
bot's message may draw a mark "like anyone's" and still, in helpful answering, be markable
only where it carried the mention — the mention is what makes it a summoner at all, and the
reaction inherits that from 0153 rather than adding a rule of its own. The decline's wording
follows from the same fact: it states what the model may REACT TO, never what it is reading,
because a decline claiming the model is not reading a line the projection just showed it
would be false in both of those shapes.

## Rejected alternatives

- **Keeping the original trigger and adding chatter beside it.** Two homes for one act, with
  the noise clause of the first contradicting the second on a literal read. The teaching is
  the whole control here, and a control that contradicts itself controls nothing.
- **Narrowing the aiming check to unaddressed messages, so the mechanism enforces the
  trigger.** The check reads the turn's co-summoners, which is what keeps a reaction aimed
  at a message the model is actually reading; rewriting it into a trigger filter would put a
  taste judgment into the anti-aiming bound and leave the bound weaker for it.
- **Leaving the reaction to addressed messages only.** That is the case where the assistant
  has words available and usually owes some, so it is the case where a reaction adds least.
