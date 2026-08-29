# The assistant assesses, a human decides

Date: 2026-08-23. The operator's standing rule, binding every unit from here on.

The assistant may form and voice moderation assessments — that a message looks
like spam, that a report seems warranted — but the final decision over any
moderation EFFECT belongs to a human administrator, structurally, not as a
convention. Every path that could touch a person's standing must carry the
human decision point in its mechanism:

- The report relay files to the group's moderation bot, and the group's human
  administrators judge every report. The assistant cannot escalate past them.
- The deletion mirror acts only on a command a human administrator issued.
- Any future administrative tool — a warn, a ban, a mute — ships only behind a
  mechanism where a human approves the concrete action before it takes effect
  (the moderation bot's review queue is the known shape), never on the model's
  own output alone.
- Autonomous detection, if it ever ships, may only ever produce an assessment
  a human sees — never an effect.

A unit that cannot satisfy this structurally does not ship its capability.
The impact assessment's review trigger for standing-touching capabilities
binds alongside; the two fire together.

## Amended 2026-08-29 — the join notice changes nothing here

Unit 36 lets the assistant see joins and report one whose shown name is
itself promotional bait. The report is the WHOLE effect, exactly as for a
violating message: no ban, no kick, no reply to the joiner, no new
capability anywhere. The group's human administrators still decide, and a
suppressed person's join is simply not recorded — the assessment surface
widens, the effect surface does not move.

## Rejected

- **Enforcement by prompt alone.** The prompt already teaches restraint, but a
  prompt is advice to a model, not a bound on the system. The invariant lives
  in the mechanisms.
- **Bot-executed actions with post-hoc review.** An effect that lands before a
  human sees it is a decision the machine took. Review must precede effect.
