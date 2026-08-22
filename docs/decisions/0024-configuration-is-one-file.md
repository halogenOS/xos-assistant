# 0024 — Configuration is one file the process reads

Date: 2026-08-22

## Context

The first runnable binary needs its wiring named somewhere: paths, the model, the
endpoints, and a way to reach two secrets without ever holding them.

## Decision

TOML, located by the binary's single command-line argument. It names: the store
path, the Telegram state-file path, the prompt directory, the log destination, the
model id, and the endpoint overrides (the Telegram API root and the OpenRouter base
URL — both defaulting to the real hosts; tests point them at loopback servers).

Secrets — the bot token and the OpenRouter key — are named indirectly: an environment
variable name or a file path per secret, exactly one of the two. Secrets never
appear in the configuration file, the store, or any tracked file.

## Rejected alternatives

- **Flags.** A growing surface wrapped in a script anyway.
- **Environment-only configuration.** Paths and model choice deserve a reviewable
  file.
