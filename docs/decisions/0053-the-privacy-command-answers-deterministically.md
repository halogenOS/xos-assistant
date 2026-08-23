# 0053 — The privacy command answers deterministically

Date: 2026-08-23

## Context

The developer terms demand a privacy policy that is easy for users to
reach; the command is this project's chosen surface for it. Recorded
precisely: the platform mandates the policy, not the command, and the
platform-side policy field named in the operator reference is deployment
wiring.

## Decision

A chat message whose first token is exactly `/privacy` — or `/privacy@`
plus the assistant's own handle, case-insensitive on the handle, which the
adapter normalizes away as translation; a foreign-handle suffix is NOT
normalized and NOT answered, that command was aimed at someone else — is
recorded on the ledger like any message, stamped as taking no debt through
the stamp's existing limited classification extended with a command kind:
no turn, no answer-window count, no unlatch, and a pending debt on the tail
propagates past it exactly as past any non-owing message. The returned
value carries the fixed answer: `Privacy policy: ` plus the configured
address, or `The privacy policy is not published yet.` when none is
configured. The command answers whether or not the message was addressed —
invoking a command is addressing by form; the stored addressed column keeps
the adapter's resolution untouched. When the channel's answer window is
exhausted, the command is recorded and answered with silence, the same
discipline the protection unit set for notices — a deterministic reply is
not a protection bypass.

The widening of the limited vocabulary reaches deployed stores through an
appended migration step that recreates the content table under the full
constraint — a column CHECK cannot be altered in place — copying every row
unchanged; the shipped protection step keeps quoting its own frozen
two-kind list, so an already-applied step's SQL never changes.

## Rejected alternatives

- **Routing the command through the model.** A legal pointer must be exact
  and free.
- **Counting command replies against the answer window.** The stored
  counting shape is the owing shape — counting without owing would need a
  bolted-on stamp branch, and the window's job is bounding model cost.
- **A separate cooldown mechanism.** The exhausted-window silence rule
  already bounds the reply rate.

Refined 2026-08-23, at the unit's close, on refuted evidence. The premise
that the exhausted-window silence bounds the reply rate was wrong: the
command stamp keeps the command out of both budget counts, so a quiet
channel never exhausts and every repeat was answered. The deterministic
answer now shares the acknowledgment-window mechanism — at most one per
channel per window, recorded silence within it, the budget check still
consulted first; the window is spent only after the successful append, so a
transient failure never eats the grant. The "separate cooldown" rejection
stands: this is the same mechanism, shared, though each line keeps its own
per-channel bookkeeping — a rules acknowledgment does not spend the privacy
answer's window. Under the same close: the ledger records the typed text
verbatim — the adapter reports the invoked command as a typed translation
beside the addressed flag, and the core matches the report, never the text;
a foreign-handle suffix reports no command. And every appended migration
step now quotes frozen vocabulary lists, so the next enum variant cannot
diverge fresh stores from upgraded ones — coincidence pins fail loudly when
an enum grows, naming the widening step the divergence needs.
