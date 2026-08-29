# 0114 — An unconfigured search does not exist

Date: 2026-08-27, with unit 27; the key's shape decided 2026-08-25.

## Context

The search needs a vendor key. A deployment without one must not carry a tool that
fails on use, and must not carry a prompt that teaches a tool the palette does not
hold — the report tool's registration already settled that shape, gating admission
and teaching on ONE predicate so the two can never disagree.

An earlier revision of this unit decided the same case twice, in opposite ways: it
said the tool is absent without a key, and also specified a structured refusal for a
missing key at call time.

## Decision

The configured key is the search's whole predicate. With a key, the assembly admits
the tool and the composed prompt teaches it. Without one, the tool is not admitted,
the palette every conversation records names no search, the delta mechanism removes
it from conversations that had it, and no sentence of the search teaching is
composed. There is no call path on which an unconfigured search can answer, so the
missing-key refusal is deleted rather than kept as a second answer to a settled
question. Nothing fails at startup either way.

The key is a secret reference — an environment variable name or a file path — and
never a value in the configuration file, the mirror token's exact shape. It is not
logged, not rendered, and the type that carries it writes its own `Debug` that
redacts it, because the assembly's own configuration carrier derives one.

## Rejected alternatives

- **Admitting the tool and failing on use.** A palette entry the deployment cannot
  serve, a teaching that instructs the model to call it, and one wasted round trip
  per attempt.
- **Keeping the structured missing-key refusal beside the absence rule.** Two
  answers to one question; the second is unreachable, which is worse than wrong.
- **The key as a configuration value.** Secrets are referenced, never written, in
  every other place in this repository.
