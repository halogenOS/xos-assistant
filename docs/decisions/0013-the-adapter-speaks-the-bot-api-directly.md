# 0013 — The adapter speaks the Bot API directly

Date: 2026-08-21

## Context

The first platform adapter needs a wire to its platform. The platform offers an HTTP Bot
API — long polling with `getUpdates`, sending with `sendMessage` — and the ecosystem
offers SDK crates around it.

## Decision

A thin client over an HTTP library: long polling in, plain sends out. The client is one
module that owns request building, the token and the JSON decoding into the adapter's
own minimal update model.

## Rejected alternatives

- **An SDK crate.** A large dependency tree to audit for what is, for this unit, two
  endpoints and a JSON model — and SDK update types must not cross into the core anyway,
  so the SDK would be wrapped as thoroughly as the raw API.
- **Webhooks.** They require a public HTTPS endpoint and certificate wiring — deployment
  surface this project does not have or need yet; long polling works from anywhere.
