# Unit 35 — updates arrive by webhook when the deployment has a public door

Date: 2026-08-29. The operator watched three messages sit undelivered for seven
minutes while the assistant's long poll showed nothing, then arrive in one batch —
undetectable from inside, unattributable afterwards. Their decision: Telegram's other
sanctioned delivery, webhooks, where Telegram pushes each update over HTTPS the moment
it exists. The deployment terminates TLS in front (the deploy repository's half); this
unit gives the ADAPTER a webhook intake beside its polling one.

## Grounding

**The adapter's shape today.** `driver::run` (`adapters/telegram/src/driver.rs:257`)
is one loop: `client.get_updates(offset)` (`:309`), then per update the
translate-authority-ingest step (`:358`), `Step::Acknowledged` advancing the offset
(`:321`), the offset persisted after each batch (`:328-334`). Delivery is
at-least-once: a failed ingest halts the batch, the offset stays, redelivery follows
— the module doc states it (`driver.rs:2-9`). The per-update step is already a
separable function; the loop is only its feeder.

**What Telegram's documentation binds** (read 2026-08-29, core.telegram.org): long
polling and webhooks are mutually exclusive — `getUpdates` answers 409 while a
webhook is set; `setWebhook` takes a `secret_token`, and every delivery then carries
it back in the `X-Telegram-Bot-Api-Secret-Token` header; a failed delivery is retried
"a reasonable amount of attempts"; undelivered updates live at most 24 hours;
webhook endpoints must be HTTPS on port 443/80/88/8443 — which the deployment's
reverse proxy owns, so the adapter's listener itself speaks plain HTTP on loopback.

**What already carries over.** Translate, authority resolution, ingest, the observe
path and the outbound consumer are delivery-agnostic; nothing in them reads the
offset. At-least-once redelivery after a failed ingest is the polling contract
already, so webhook retries land on semantics the core was built for.

## Decisions taken with this unit

- **One intake seam, two sources, chosen by configuration, 2026-08-29.** The
  per-update step becomes the shared path it already almost is; the poll loop and the
  webhook listener are two feeders of it. A webhook configuration present (public
  address for registration plus a loopback listen port) means webhook mode; absent
  means polling, exactly as today — one predicate, no third state. John keeps polling
  locally with no public endpoint. *Rejected:* webhook-only (kills the local
  deployment); *rejected:* both at once (Telegram forbids it).
- **The mode announces itself to Telegram at startup, 2026-08-29.** Webhook mode
  calls `setWebhook` with the configured public address and the secret token;
  polling mode checks for a registered webhook and deletes it if and only if one is
  set, because `getUpdates` answers 409 forever otherwise. Registration failure
  refuses the start loudly — a webhook deployment that silently cannot register
  would sit deaf, which is the outage this unit exists to end. *Rejected:* leaving
  mode transitions to the operator's hands.
- **The secret token is the adapter's own, generated and kept, 2026-08-29.** At
  first webhook start the adapter generates a random token, persists it beside its
  state file with owner-only permissions, and reuses it thereafter. Every delivery
  must carry it back in `X-Telegram-Bot-Api-Secret-Token`; a mismatch or absence is
  answered 403 with nothing read and nothing logged beyond a counter-grade line —
  the door discards strangers without describing itself. No human carries this
  secret; it never enters configuration, logs, or chat. *Rejected:* an operator
  secret (a credential no human needs to know is a credential no human should
  handle); *rejected:* skipping the token (anyone who finds the path could feed her
  updates).
- **Acknowledgement is the response code, 2026-08-29.** The listener answers 200 if
  and only if the update went through the shared step to `Acknowledged` — Telegram's
  retry then plays the role the offset file plays in polling, the same at-least-once
  contract. A failed ingest answers 500 and Telegram redelivers. The offset file is
  simply unused in webhook mode; it is not written, not read, and not migrated.
  *Rejected:* 200-then-process (a crash between the two loses the update with no
  redelivery — the exact loss the polling design refuses).
- **The listener is loopback-only plain HTTP, smallest possible, 2026-08-29.** It
  binds the configured loopback port, accepts exactly POST on one path, bounds the
  body it reads, and parses the update with the same types the poll path parses.
  TLS, hostname, and the public path are the reverse proxy's job (the deploy
  repository's half, not this unit's). The HTTP surface is built on what the
  dependency tree already carries — reqwest's own hyper stack — and only if that is
  genuinely unusable as a server does a minimal, web-checked dependency enter
  through the review the repository requires; a web framework is out of the
  question for one route. *Rejected:* binding beyond loopback; *rejected:* a
  framework for one POST handler.
- **The adapter still decides nothing, 2026-08-29.** The listener translates
  deliveries into the same neutral messages and outcomes; no behavior moves into
  the adapter, and the core learns nothing about delivery mechanics. The core
  contains no platform vocabulary, before and after.

## The unit's contract

With a webhook configuration, the adapter registers the public address with its own
secret token at startup, listens on the configured loopback port, and feeds each
authenticated delivery through the same translate-authority-ingest step as polling —
answering 200 exactly on acknowledgement so Telegram's retry preserves at-least-once
delivery; deliveries without the right token are discarded with 403. Without the
configuration, the adapter polls exactly as today, first deleting a registered
webhook if one exists. The secret is machine-generated, persisted owner-only, and
never appears in configuration, logs, or chat. No behavior enters the adapter, no
platform vocabulary enters the core, and the outbound path is untouched.

## Acceptance criteria

- **AC1** Workspace green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; any new dependency web-checked and
  recorded in `docs/dependency-review.md` before the manifest names it.
- **AC2** Mode by one predicate: with the webhook configuration the listener serves
  and no poll happens (pinned: no getUpdates request reaches the wire); without it
  the poll loop runs exactly as today (existing pins stay green untouched).
- **AC3** Startup announces the mode: webhook start registers address plus token
  (pinned against a scripted Bot API); registration failure refuses the start with a
  named error; polling start deletes a registered webhook if and only if one exists.
- **AC4** The door authenticates: a delivery with the right token ingests and
  answers 200; a wrong or missing token answers 403 with nothing ingested; a
  malformed body and an oversized body are each rejected without a panic — each
  pinned through a real local HTTP round trip.
- **AC5** Acknowledgement is honest: an update whose ingest fails answers 500, and
  the same update redelivered afterwards ingests once — pinned end to end.
- **AC6** The secret stays dark: generated with owner-only permissions, absent from
  rendered configuration, logs and error text — pinned by assertions this unit
  authors; the persisted file's mode is asserted.
- **AC7** The offset file is untouched in webhook mode — pinned: a webhook run
  neither reads nor writes it.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-webhook`, branch
  `unit/webhook-intake`). Sites: `adapters/telegram/src/driver.rs` (the shared step
  extracted as the seam, the poll feeder around it), a new listener module beside
  it, `client.rs` (setWebhook/deleteWebhook/getWebhookInfo calls), `state.rs`
  (secret persistence beside the offset), the adapter config plus
  `assistant/src/config.rs` wiring, and the adapter suite (which already runs a
  scripted Bot API server for the poll path — the listener pins ride the same
  pattern).
- The deploy repository's half (Caddy, Let's Encrypt, the subdomain, firewall
  ports) is deliberately outside this unit and waits on the operator's hostname.
- The quality bar from the operator, verbatim scope for the reviewers: "The code
  must always be better and cleaner afterwards than it was before. If you had to
  add a snowflake if somewhere, it's a smell." If the intake seam needs the poll
  loop rewritten to fit, restructure it — two feeders of one step, not a mode flag
  threaded through one loop.
