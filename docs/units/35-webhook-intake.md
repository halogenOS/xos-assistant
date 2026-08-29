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

**What already carries over, and what does not.** Translate, authority resolution,
ingest, the observe path and the outbound consumer are delivery-agnostic; nothing in
them reads the offset (verified by the cold round). What does NOT carry over is
unbounded redelivery: polling's offset holds a failed update forever, while Telegram
retries a refused webhook delivery only so many times and drops undelivered updates
after 24 hours. Webhook mode therefore trades unbounded redelivery for bounded,
logged redelivery — an accepted trade, stated below, not a hidden one. The polling
loop also processes strictly serially through one mutable memories carrier
(`driver.rs:206-216`); whatever feeds the shared step must preserve that discipline
or restructure it deliberately.

## Decisions taken with this unit

- **One intake seam, two sources, chosen by configuration, 2026-08-29.** The
  per-update step becomes the shared path it already almost is; the poll loop and the
  webhook listener are two feeders of it. The configuration is a `[webhook]` section
  in the assistant's own configuration (`assistant/src/config.rs`), carried into the
  adapter's `Config` the way the endpoint override already travels: `public_url`,
  the full HTTPS address Telegram will call (refused at start unless it parses as an
  `https` URL), and `listen_port`, the loopback port the listener binds. No
  defaults, no partial state: both fields or neither — a section with one field
  refuses the start naming the other. Section present means webhook mode; absent
  means polling, exactly as today — one predicate, no third state. John keeps
  polling locally with no public endpoint. The deployed values are
  `https://xenia.halogenos.org/telegram/webhook` and port 8085, the contract the
  deploy repository's reverse proxy already records. *Rejected:* webhook-only
  (kills the local deployment); *rejected:* both at once (Telegram forbids it);
  *rejected:* adapter-file configuration apart from the assistant's (two files
  deciding one mode).
- **Deliveries are processed serially by one consumer; the listener only queues,
  2026-08-29.** One consumer task owns the shared per-update step and the mutable
  memories the poll loop owns today — the serial discipline is kept, not worked
  around. The listener authenticates, bounds and parses a delivery, then hands the
  update with a one-shot answer channel over a BOUNDED queue to the consumer and
  answers the HTTP request with the outcome the consumer reports, waiting at most
  twenty-five seconds — past that it answers 500 and moves on, so a wedged
  platform call inside the shared step cannot pin HTTP connections open until
  Telegram's delivery pool (forty connections by default) fills and the deployment
  goes deaf while looking busy. An expiry may answer 500 for an ingest that later
  completes; Telegram's redelivery then meets the acknowledged-id set and gets its
  200 — the contract holds. A lost outcome channel (a consumer that dropped the
  answer) reads as the same 500 for the same reason. A full queue answers 503 with
  nothing read into the pipeline — honest backpressure Telegram's retry absorbs.
  *Rejected:* concurrent handling sharing the memories (a restructure nothing asked
  for); *rejected:* an unbounded queue (memory as backpressure); *rejected:* an
  unbounded wait (the connection pool is a resource the spec refuses to leak).
- **Duplicates are answered from a bounded memory of acknowledged ids, 2026-08-29.**
  The consumer keeps a bounded in-memory set of recently acknowledged update ids —
  one thousand and twenty-four, evicting the OLDEST at the cap, because clearing
  whole would re-open the duplicate window for exactly the ids most likely to
  retry. A delivery whose id is in the set — a retry that raced its original, or
  one arriving after — answers 200 without re-ingesting; because processing is
  serial, a racing retry simply queues behind its original and meets the set. The
  set does not survive a restart, so a crash between ingest and answer can
  re-ingest one update — the exact at-least-once window polling's offset file
  already has, not a new one. Deliveries are processed in ARRIVAL order, not
  update-id order: a previously refused older update that redelivers after its
  successors ingested appends late, the way a late-delivered message reads to a
  human — accepted and stated; the ledger records arrival truth and nothing
  re-sorts. *Rejected:* a persisted dedup ledger (a second offset file by another
  name, for a window polling already accepts); *rejected:* holding successors back
  for a refused predecessor (polling's batch-halt has that shape, but a webhook
  cannot hold back what Telegram already delivered).
- **The bounded-loss trade is stated and logged, never silent, 2026-08-29.** Every
  refused delivery (500, 503) logs one structural warning naming the update id and
  the reason — never content — so an update Telegram eventually gives up on leaves
  its trail of refusals in the log. When the store is so broken that ingest fails
  past Telegram's patience and the 24-hour expiry, the update is lost; in that state
  polling would sit equally broken, holding updates it cannot ingest against the
  same 24-hour server-side expiry. Accepted with eyes open, because the alternative
  — acknowledging what was not ingested — loses updates silently on every crash.
- **Webhook startup is identity, then bind, then register — refusal anywhere,
  2026-08-29.** The bot identity (`getMe`) is fetched first, because the shared
  step cannot translate without it (`driver.rs:305`'s comment stands: translating
  blind records wrong facts into a durable ledger); in webhook mode a failed
  identity fetch refuses the start instead of poll mode's endless retry, the same
  loud-refusal rule as everything else in this list. The listener binds second, and
  only then does `setWebhook` register the address — so Telegram is never pointed
  at a port nothing serves; a registration left by a previous run is simply
  overwritten. Registration failure refuses the start loudly — a webhook deployment
  that silently cannot register would sit deaf, which is the outage this unit
  exists to end. Registration pins `allowed_updates` to the same
  `CONSUMED_UPDATE_TYPES` list the poll request already pins — one list, reused —
  and sets `drop_pending_updates` false: updates queued through an outage flood in
  at start, which is the at-least-once promise working, never a discard.
  *Rejected:* register-then-bind (a crash between the two leaves Telegram
  delivering into a dead port until the supervisor cycles — the outage class this
  unit ends). Polling mode checks `getWebhookInfo` and deletes the webhook if and only if
  one is registered; if that check itself fails, polling starts anyway and the poll
  loop's existing error reporting carries any 409 — a local deployment must not be
  refused its start by a transient check, and a genuinely registered webhook
  surfaces in the loop's own errors within seconds. Switching modes accepts what
  the polling crash window always accepted: redelivered duplicates, and the
  24-hour server-side expiry for a box that slept across the switch. *Rejected:*
  leaving mode transitions to the operator's hands; *rejected:* refusing a polling
  start on a failed check (brittle exactly where the local deployment lives).
- **The secret token is the adapter's own, generated and kept, 2026-08-29.** At
  first webhook start the adapter generates the token — 64 characters from
  Telegram's own permitted alphabet (letters, digits, underscore, hyphen), read
  from the operating system's randomness with no new dependency — persists it
  beside its state file with owner-only permissions, and reuses it thereafter. Every delivery
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
  binds the configured loopback port — a failed bind refuses the start naming the
  port — accepts exactly POST on one path, bounds the body it reads at one
  megabyte (an update is kilobytes; oversized answers 413), answers 400 to a body
  that does not parse, 404 to any other path and 405 to any other method (both
  without reading a body — the door describes nothing), and parses the update with
  the same types the poll path parses. The assistant's configuration refuses port
  zero (a deployment's port is a contract with its reverse proxy); the adapter's
  own `Config` accepts any port and exposes the bound address, which is how the
  suite drives an ephemeral listener. The listener future joins the run entry's existing select; a listener
  that dies mid-run ends the run the way a dropped outbound edge does, and the
  service supervisor restarts the process — a deaf webhook deployment must never
  keep running quietly.
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
- **AC2** Mode by one predicate: with the `[webhook]` section the listener serves
  and no poll happens (pinned: no getUpdates request reaches the wire); without it
  the poll loop runs exactly as today (existing pins stay green untouched); a
  half-filled section refuses the start naming the missing field — pinned.
- **AC3** Startup order and announcement: webhook start fetches identity, binds,
  then registers address plus token with `allowed_updates` pinned to the shared
  list and `drop_pending_updates` false — the order pinned against a scripted Bot
  API (a registration observed before the listener serves is a failure);
  identity-fetch, bind and registration failures each refuse the start with a
  named error; polling start deletes a registered webhook if and only if one
  exists, and starts anyway when the check itself fails.
- **AC4** The door authenticates and bounds: right token ingests and answers 200;
  wrong or missing token answers 403 with nothing ingested; a malformed body
  answers 400, an oversized one 413, a full queue 503 — each with nothing
  ingested, each pinned through a real local HTTP round trip, none panicking.
- **AC5** Acknowledgement is honest and duplicates are met: an update whose ingest
  fails answers 500 with a structural warning naming the update id, and the same
  update redelivered afterwards ingests once; a duplicate of an ACKNOWLEDGED update
  answers 200 without a second ingest; a consumer that never answers has the
  listener answering 500 at its deadline instead of holding the connection — each
  pinned end to end through the queue and the consumer, not around them.
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
