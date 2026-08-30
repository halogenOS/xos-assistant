# 0160 — The command catalogue arrives minimally, with the session resets

Date: 2026-08-30, with unit 45.

## Context

Recognition of a command was hand-matched three times: the privacy family's five exact
spellings, the moderation bot's deletion token, and the list of commands a reply acts on.
The commands-menu design of 2026-08-25 was written to end exactly that, and it named the
shape: one enum in the core, one recognition over it, one reading of who is offered what.
Nothing of it had been built. The two session-reset commands would have added a fourth
hand-written list.

## Decision

The catalogue arrives here, minimally, adopted from that design: an enum of the commands
this assistant answers with its pinned order, the token each is invoked by, the reading of
who is offered which command in which kind of channel, and one recognition that folds ASCII
case. It carries the five privacy commands and the two session resets, and nothing else.

Three consequences follow immediately:

- The privacy family's matcher becomes a projection of the one recognition, so a spelling
  the family accepts and a spelling the catalogue accepts cannot drift apart. Every pin the
  family had stands, except the one asserting that its spellings are matched exactly: case
  folding is the catalogue's rule, so that pin becomes its opposite — a hand-typed mixed-case
  data right is recognized, and a longer word merely carrying the token is not.
- The command stamp widens from "the privacy family, or the mirror" to "a recognised
  command, or the mirror". A command takes no debt by its nature, whoever invoked it and
  wherever; the audience reading below decides only whether it is ANSWERED.
- The moderation bot's deletion token stays out of the catalogue and keeps its exact
  comparison where it is. It is not this assistant's command, and folding case on a token
  whose match ERASES a stored message would widen an irreversible act on an assumption
  about another program's parser that nobody has checked.

What a person reads in a published menu — the per-command summary, and the publication
itself — stays with the commands-menu unit. Its spec carries a dated note recording what
this unit built of its design.

## Rejected alternatives

- **A fourth hand-matched list for the two resets.** The recorded smell the catalogue design
  exists to end, and adding to it while the design sits unbuilt would make the eventual
  adoption strictly harder.
- **Waiting for another adopter to land the catalogue first.** No other unit is approved; the
  session resets are, and a catalogue whose first adopter never arrives is a design document
  rather than a decision.
- **Landing the full design here — summaries, the help answer, the platform menu.** That is a
  different unit's work with its own audience and its own copy, and pulling it in would put
  user-facing wording into a unit whose copy was settled without it.
