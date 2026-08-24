# 0093 — The moderation teaching composes only where it can act

Date: 2026-08-24

## Context

The assessment is the model's: the prompt teaches it to judge each message
against the pinned rules, think first, and report a clear violation —
borderline, rule-absent and no-rules cases stay unreported. A keyword engine
cannot do this: rules are natural language and judgment is contextual — a
member quoting a banned phrase to ask about it is not a violation. But a
prompt that teaches a capability the deployment does not have instructs the
model into dead calls.

## Decision

The moderation teaching is composed into the system prompt exactly when a
moderation handle is configured AND the answering mode is helpful — the two
conditions autonomous assessment needs: the report line goes nowhere without
a handle, and only helpful answering shows the model every message it would
judge. The report tool's registration takes the same predicate, stated once
in the teaching module, so the prompt never instructs a tool the palette
does not carry and a registered tool is never left untaught. A deployment
without a handle, or in addressed mode, teaches no moderation and registers
no report tool; the palette-delta mechanism removes the tool from
conversations that had it. The judgment is reasoned — the reasoning-effort
key sizes the thinking — and bounded by decision 0070: the assistant
ASSESSES, the administrators DECIDE; it never bans, mutes or removes.

## Rejected alternatives

- **A keyword engine in the core.** Rules are prose; judgment is contextual.
  Deterministic matching would report the member who quotes a rule to ask
  about it.
- **Teaching moderation unconditionally.** An instruction to use a tool that
  is not registered, drawing declined calls in every deployment the
  conditions exclude.
- **Registering the tool on the handle alone.** In addressed mode the model
  never sees the messages it would judge; a registered, untaught tool is
  surface without capability.
