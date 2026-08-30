# 0157 — The emoji is content the core records, and the platform's list is the adapter's

Date: 2026-08-30, with unit 39.

## Context

The reaction design of 2026-08-25 gave the core a closed vocabulary: an enum with one
variant, structurally incapable of a negative or judging mark, its glyph held in a
one-line table in the adapter and frozen in the stored column's CHECK. Three of its
decisions rested on that shape — the closed enum, the no-judging-variant structure, and
the byte-pinned glyph table.

The operator was asked whether the reactions should be a curated positive set and ruled
the opposite: the full palette, nothing pre-filtered by sentiment. With no curation there
is no vocabulary left to close, and the three decisions above lose the thing they were
about.

## Decision

An emoji is CONTENT, not vocabulary, and the core records it the way it records answer
text.

- **The tool takes the emoji as a parameter** beside the message id, and the core stores
  the string verbatim on the block. It owns no emoji list of any kind. The tool's
  model-facing name moves with the vocabulary — `mark_seen` no longer says what the tool
  does — so it is named `react`.
- **What the core owns is the BOUND.** A non-empty argument of at most thirty-two bytes,
  taught in the refusal, with the stored column's CHECK as its schema twin. The frozen
  vocabulary CHECK of the earlier design dies with the enum; a length bound replaces it,
  and a widened bound is an appended migration exactly as a widened vocabulary was.
- **The platform's own reaction list lives in the ADAPTER**, as escape sequences and
  never as pasted glyphs, with selector-blind matching and the list's own bytes on the
  wire. Which tokens a platform can place is a platform fact.
- **"The core carries no emoji" gains two checks that can fail for it.** A non-ASCII
  character scan over production core source against an enumerated allowlist, and an
  escape scan over the emoji codepoint ranges — the second because the byte-hazard rule
  requires an emoji to be written as an escape, which the first cannot see. Each carries a
  deliberately-failing fixture through its own predicate, so a green run proves the scan
  bites.
- **The target validation survives with one collapse.** A call naming no id, an unknown
  origin, a message with no recorded principal, an already-marked origin and an
  already-reported one are all refused exactly as designed. The DISTINCT own-message
  decline is not built: the assistant's voice writes no chat rows, so her message ids are
  never among the turn's co-summoners, and the anti-aiming decline answers the attempt
  before anything else is read. A second refusal for a case the first already covers would
  record one decision twice.

Decision 0070 stands untouched. A reaction is expression, not a moderation effect: it
changes nobody's standing, rights or access, and every moderation effect keeps its human
decision point. The palette includes unkind emojis; choosing one is a conduct matter
governed by the deployed persona, and the in-repo teaching adds no vocabulary restriction.
That is a taste line, and taste is the deployment's.

Two consequences are accepted rather than engineered away, and both are stated where a
reader will meet them. A mark the platform cannot carry is dropped by the adapter with a
log line and the model is never told — the tool has already returned, and a cheap act
earns no delivery report. And because the per-origin check then refuses every later
attempt on that message, one bad pick permanently unmarks it: the same accepted permanence
as a mark lost when the process dies mid-flight.

The per-origin check reads the STORED origin, so it binds for exactly as long as that
origin is stored, and that boundary is deliberate. An erasure or the deletion mirror
empties the reference, the check then matches nothing, and a later turn may react to that
message again. This is not a hole in the bound: erasure must leave no shadow saying
something was here, and a later mark is a fresh act rather than the old one returning. A
bound that survived the erasure would be that shadow, in the one table erasure had just
emptied.

## Rejected alternatives

- **Keeping the closed enum and curating the set.** The operator ruled the full palette.
  Curation is exactly what the enum was for.
- **A closed enum widened to the platform's seventy-three.** That imports the platform's
  vocabulary into the core wholesale under a different spelling, which is the thing the
  no-platform-vocabulary invariant exists to stop.
- **Validating the emoji against a list in the core, so a mis-pick is refused at the
  tool.** The list is a platform fact, and a core that held one would decide for every
  adapter what its platform can place. The drop is where a platform fact belongs.
- **Echoing the placement outcome back to the model, or recording it on the block.** A
  return path exists since unit 38 — the reply arm carries a delivery handle the adapter
  hands back — and the mark arm omits it deliberately. A cheap act earns no bookkeeping
  row. Stated so nobody completes the symmetry.
- **Keeping the structural no-judging guarantee by refusing a short list of unkind
  emojis.** A negative-emoji blocklist inside the core is a curated set with extra steps,
  against the operator's own ruling, and it would put a taste judgment in the one place
  taste must not live.
