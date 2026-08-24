# 0091 — The report names its target, validated against the assessment set

Date: 2026-08-24

## Context

The report capability was member-initiated: a member replied to an offending
message and asked, and the tool resolved the target from that stored reply,
taking no parameter. The autonomous-moderation unit turns the assessment
around — the assistant judges each group message against the pinned rules
itself — and under helpful answering "the message being assessed" is not
singular: absorption folds several messages into one turn, so a parameterless
target is undefined exactly where the capability lives.

## Decision

The tool takes ONE parameter: the offending message, named by its stored
origin — the platform message id. The projection shows that id to the model,
in brackets ahead of every user-voiced message that has one, so the model can
quote what it names; the id mark is prose like the speaker prefix, and a
typed forgery is bounded by the validation below. The resolution VALIDATES
the named origin against the current turn's co-summoner set, the same walk
the privacy tool resolves over: the named origin must belong to a message
the model is actually assessing this turn. An origin outside that set — an
old message, an arbitrary id, another channel's — is refused. Member-initiated
reporting is removed as redundant, with its reply-target resolution; the
co-summoner walk stays as the shared validator. The tool's description,
guards and result lines are reframed for assessment, each decline teaching
no-retry, the exact copy pinned as named constants: the not-assessed decline,
the self-report refusal (the named message resolving to the assistant's own
stored voice — reachable in principle now that the model names ids), the
unrecorded-target refusal (a row without a recorded principal names nobody
erasure can reach), the missing-target decline, and the group-only refusal.
A report and an answer stay independent: the filed block delivers whether
the turn's answer is spoken or abstained.

## Rejected alternatives

- **A parameterless "the assessed message".** Undefined under absorption —
  the probe's deepest finding. Several co-summoned messages leave no single
  referent.
- **An unvalidated origin parameter.** The model could aim a report at any
  message in the store, old or invented. The co-summoner bound is the whole
  anti-aiming mechanism.
- **Reporting every violating co-summoner automatically.** The model's
  judgment picks; one report per call keeps each filing a deliberate,
  reasoned act.
- **Keeping the member-initiated path beside the autonomous one.** Redundant:
  the assistant reads every message a member could ask about, and two target
  resolutions in one tool are two ways to disagree.
