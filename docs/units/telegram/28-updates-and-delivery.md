# Telegram unit 28 — the update transport: one delivery, once

Date: 2026-08-27. Every other unit in this series sits on the poll. Unit 06 cannot see a
reaction the poll never asked for, unit 07 cannot answer a callback that was never
subscribed to, and unit 03's edit arrives only because `edited_message` happens to be in a
three-element array in the wire client. This unit specifies the transport itself: what is
asked for, what one delivered update means, and what a restart does to the updates that were
in flight when the process died.

Three findings shape it, and all three are stated here because an implementer who does not
believe them will build the wrong thing.

**The platform never complains about an update type you did not ask for.** `allowed_updates`
is a closed list; omitting a type produces silence, not an error. The current list is
`["message", "edited_message", "my_chat_member"]` (`client.rs:103`), hand-written beside a
hand-written decode struct (`client.rs:108-121`). Nothing in the build makes the two agree,
and nothing tells anyone when the platform grows a type. Since that constant was written the
platform has added three new fields to `Update` — `guest_message` (Bot API 10.0),
`subscription` (10.2) and `stopped_message_generation` (10.3). None of them is wanted here.
That is the point: the correct set is a decision, and a decision has to be visible in one
place instead of implied by two lists that can drift apart.

**The current delivery contract accepts a duplicate, and the duplicate is not harmless.**
Decision 0014 states it plainly — a crash between the ingest and the offset write
redelivers, "and the duplicates are the accepted outcome" — and a test pins that outcome
today (`tests/adapter/offset.rs:51`). What the duplicate actually costs was never priced.
A redelivered addressed message takes a second answer debt against the sender's budget
(`assembly.rs:697`, counted by `COUNTED_DEBT_SQL` at `kind.rs:930`), appends a second block
with the same origin, and shows the model the same question twice under the same bracketed
identifier (`kind.rs:181`, `kind.rs:565`). The observation path already avoids all of this:
a context note appends only on a delta (`assembly.rs:991-993`) and an authorization is
`INSERT OR IGNORE` (`authorization.rs:45-48`). The message path is the one place the
project's own idempotency discipline was not applied.

**A webhook left on the token makes this deployment permanently deaf, and the log does not
say so.** The platform documents that `getUpdates` "will not work if an outgoing webhook is
set up". The wire shape of that failure is not documented, and today the client discards the
API's own description for any non-success status (`client.rs:566-570` returns
`ClientError::Status` before the envelope is read), so the poll loop logs "the API answered
status 409", backs off two seconds (`driver.rs:71`) and repeats that forever while the group
gets no answers.

## What the transport cannot promise

Written first, because the acceptance criteria below deliberately do not claim any of it.

1. **Nothing survives an outage longer than a day.** "Incoming updates are stored on the
   server until the bot receives them either way, but they will not be kept longer than 24
   hours." A process down for longer loses whatever fell out of the queue, permanently.
2. **The loss is not detectable from the wire.** Update identifiers are sequential, so a gap
   looks like loss — but a filtered subscription means the identifiers this deployment sees
   are not contiguous anyway, and the platform publishes no rule tying the sequence to the
   filtered set. Any code claiming "updates were lost here" would be stating something it
   cannot know.
3. **The identifier sequence can restart.** "If there are no new updates for at least a
   week, then identifier of the next update will be chosen randomly instead of
   sequentially." A stored offset ahead of a renumbered sequence would confirm the new
   updates away unseen, because "an update is considered confirmed as soon as getUpdates is
   called with an offset higher than its update_id". The repair is to remove the stored
   offset, which this unit makes safe instead of merely tolerable — see the decision that
   makes the offset file advisory.
4. **The platform offers at-least-once, never exactly-once.** An unconfirmed update is
   redelivered on every poll until an offset past it confirms it. Exactly-once is something
   this side has to build; it cannot be asked for.

## Grounding

### What the platform actually does

Fetched 2026-08-27 from `https://core.telegram.org/bots/api` and
`https://core.telegram.org/bots/api-changelog`, read from the page text and not from a
summary — the first summarising fetch of the same page dropped two sentences of the
`allowed_updates` description, including the one this unit turns on.

**The current version is Bot API 10.3, dated August 24, 2026.** The changelog's four newest
entries are 10.3 (August 24, 2026), 10.2 (July 14, 2026), 10.1 (June 11, 2026) and 10.0
(May 8, 2026). The brief for this series named 10.1 as current; it is two releases behind.
Nothing in 10.2 or 10.3 changes `getUpdates`, `setWebhook` or the offset semantics.

**Two transports, mutually exclusive, one day of retention.** "There are two mutually
exclusive ways of receiving updates for your bot - the getUpdates method on one hand and
webhooks on the other. Incoming updates are stored on the server until the bot receives them
either way, but they will not be kept longer than 24 hours."

**`getUpdates` parameters, verbatim.**

- `offset`: "Identifier of the first update to be returned. Must be greater by one than the
  highest among the identifiers of previously received updates. By default, updates starting
  with the earliest unconfirmed update are returned. An update is considered confirmed as
  soon as getUpdates is called with an offset higher than its update_id. The negative offset
  can be specified to retrieve updates starting from -offset update from the end of the
  updates queue. All previous updates will be forgotten."
- `limit`: "Limits the number of updates to be retrieved. Values between 1-100 are accepted.
  Defaults to 100."
- `timeout`: "Timeout in seconds for long polling. Defaults to 0, i.e. usual short polling.
  Should be positive, short polling should be used for testing purposes only."
- `allowed_updates`: "A JSON-serialized list of the update types you want your bot to
  receive. For example, specify ["message", "edited_channel_post", "callback_query"] to only
  receive updates of these types. See Update for a complete list of available update types.
  Specify an empty list to receive all update types except chat_member, message_reaction,
  and message_reaction_count (default). If not specified, the previous setting will be used.
  Please note that this parameter doesn't affect updates created before the call to
  getUpdates, so unwanted updates may be received for a short period of time."
- Notes: "1. This method will not work if an outgoing webhook is set up. 2. In order to
  avoid getting duplicate updates, recalculate offset after each server response."

**`Update`.** "This object represents an incoming update. At most one of the optional fields
can be present in any given update." `update_id`: "The update's unique identifier. Update
identifiers start from a certain positive number and increase sequentially. This identifier
becomes especially handy if you're using webhooks, since it allows you to ignore repeated
updates or to restore the correct update sequence, should they get out of order. If there
are no new updates for at least a week, then identifier of the next update will be chosen
randomly instead of sequentially."

The full field list as of 10.3, in page order: `message`, `edited_message`, `channel_post`,
`edited_channel_post`, `business_connection`, `business_message`, `edited_business_message`,
`deleted_business_messages`, `guest_message`, `message_reaction`, `message_reaction_count`,
`inline_query`, `chosen_inline_result`, `callback_query`, `shipping_query`,
`pre_checkout_query`, `purchased_paid_media`, `poll`, `poll_answer`, `my_chat_member`,
`chat_member`, `chat_join_request`, `chat_boost`, `removed_chat_boost`, `managed_bot`,
`subscription`, `stopped_message_generation`. Twenty-seven types; this deployment asks for
three.

`my_chat_member`: "The bot's chat member status was updated in a chat. For private chats,
this update is received only when the bot is blocked or unblocked by the user." No
administrator requirement and no explicit-subscription requirement — it is in the default
set and is subscribed to explicitly here anyway.

**`setWebhook`.** "Whenever there is an update for the bot, we will send an HTTPS POST
request to the specified URL, containing a JSON-serialized Update. In case of an
unsuccessful request (a request with response HTTP status code different from 2XY), we will
repeat the request and give up after a reasonable amount of attempts." Parameters: `url`
(required, HTTPS, empty string removes the integration), `certificate` ("Upload your public
key certificate so that the root certificate in use can be checked"), `ip_address` ("The
fixed IP address which will be used to send webhook requests instead of the IP address
resolved through DNS"), `max_connections` ("The maximum allowed number of simultaneous HTTPS
connections to the webhook for update delivery, 1-100. Defaults to 40"), `allowed_updates`
(the same description as above, with "before the call to the setWebhook"),
`drop_pending_updates` ("Pass True to drop all pending updates"), and `secret_token` ("A
secret token to be sent in a header "X-Telegram-Bot-Api-Secret-Token" in every webhook
request, 1-256 characters. Only characters A-Z, a-z, 0-9, _ and - are allowed"). Notes: "1.
You will not be able to receive updates using getUpdates for as long as an outgoing webhook
is set up. 2. To use a self-signed certificate, you need to upload your public key
certificate using certificate parameter. Please upload as InputFile, sending a String will
not work. 3. Ports currently supported for webhooks: 443, 80, 88, 8443."

**`deleteWebhook`** takes `drop_pending_updates` and returns True. **`getWebhookInfo`**
"Requires no parameters. On success, returns a WebhookInfo object. If the bot is using
getUpdates, will return an object with the url field empty." `WebhookInfo` carries `url`
("may be empty if webhook is not set up"), `has_custom_certificate`,
`pending_update_count` ("Number of updates awaiting delivery"), `ip_address`,
`last_error_date`, `last_error_message`, `last_synchronization_error_date`,
`max_connections` and `allowed_updates` ("A list of update types the bot is subscribed to").

**Failure answers carry their reason.** "In case of an unsuccessful request, 'ok' equals
False and the error is explained in the 'description'. An Integer 'error_code' field is also
returned, but its contents are subject to change in the future. Some errors may also have an
optional field 'parameters' of the type ResponseParameters" — whose `retry_after` is "In
case of exceeding flood control, the number of seconds left to wait before the request can
be repeated". The description text for a webhook conflict is not documented anywhere on the
page, and neither is its status code.

**The local Bot API server.** Running `telegram-bot-api` locally would let the bot "Download
files without a size limit", "Upload files up to 2000 MB", "Upload files using their local
path and the file URI scheme", "Use an HTTP URL for the webhook", "Use any local IP address
for the webhook", "Use any port for the webhook", "Set max_webhook_connections up to 100000"
and "Receive the absolute local path as a value of the file_path field without the need to
download the file after a getFile request". Switching is not free: `logOut` says "You must
log out the bot before running it locally, otherwise there is no guarantee that the bot will
receive updates. After a successful call, you can immediately log in on a local server, but
will not be able to log in back to the cloud Bot API server for 10 minutes", and `close`
says "You need to delete the webhook before calling this method to ensure that the bot isn't
launched again after server restart."

### What this deployment does today

- **The poll.** `BotClient::get_updates` (`client.rs:313-326`) sends `timeout` = 25
  (`client.rs:19`), `allowed_updates` = `CONSUMED_UPDATE_TYPES` (`client.rs:103`,
  `client.rs:319`) and `offset` only when one is known (`client.rs:321-323`). `limit` is
  never sent, so the platform's default of 100 applies unstated. The whole request times out
  at 35 seconds (`client.rs:22`).
- **The subscription is stated on every poll, deliberately.** The constant's own comment
  says an absent selection "would inherit whatever an earlier setting left on the token"
  (`client.rs:99-103`) — the platform's "If not specified, the previous setting will be
  used" already accounted for. That part is right and stays.
- **The decode is minimal and lenient.** `Update` carries `update_id`, `message`,
  `edited_message` and `my_chat_member` (`client.rs:108-121`); unknown fields are ignored by
  serde, so an unrequested type decodes into an update with every payload absent.
- **The loop.** `poll_loop` (`driver.rs:298-340`) reads the persisted offset once
  (`driver.rs:306`), polls, and advances `next_offset` to `update.id + 1` per update that
  reached a terminal step (`driver.rs:319-327`), stopping the batch on the first transient
  failure. It persists after the batch when the offset moved (`driver.rs:328-335`) and
  treats a failed write as survivable, because "the acknowledged updates redeliver after a
  restart as accepted duplicates". A failed poll logs "the poll failed; backing off" and
  sleeps `POLL_BACKOFF`, two seconds (`driver.rs:71`, `driver.rs:311-315`).
- **Terminal refusals are acknowledged past** so one bad update cannot wedge the chat behind
  it (`driver.rs:461-467`), and a translation skip is acknowledged too (`driver.rs:367-371`).
  An update with no known payload becomes `Skip::NonMessage` (`translate.rs:128-130`).
- **The offset file.** `state::read`/`state::write` (`state.rs:12-46`): one integer, written
  to a sidecar and renamed, no fsync, with absent, empty and malformed all reading as absent.
  The path comes from the configuration key `telegram_state_path` (`config.rs:27-28`, sample
  value `telegram.offset` at `config.rs:672`), passed at `main.rs:387-390` and documented on
  `Config::state_file` (`lib.rs:62-65`).
- **The identity read runs before the first poll** and retries on the poll backoff
  (`driver.rs:305`, `driver.rs:345-355`), because translating without the bot's own identity
  would record wrong facts. That is the precedent for a second startup call.
- **The rate-limit contract** is one place (`client.rs:505-527`): up to three attempts
  honouring `retry_after`, with an optional per-caller ceiling. The poll passes no ceiling on
  purpose — "the identity fetch and the poll, which run ahead of any batch with nothing
  queued behind them" (`client.rs:496-504`).
- **Non-success statuses lose their description.** `decode` returns `ClientError::Status`
  from the status alone (`client.rs:566-570`) and only reads `description` for a 200 answer
  carrying `ok: false` (`client.rs:574-580`).
- **The token never reaches a log or an error string** (`client.rs:1-9`, `client.rs:589-592`),
  pinned by its own test binary (`tests/token_scan.rs:1-16`).

### What the core does with a delivered message

- `ingest` (`assembly.rs:623-780`) admits the channel, resolves the sender, resolves the
  conversation, then takes the stamp lock (`assembly.rs:660`) and, under it: re-reads
  suppression (`:664`), reconciles the tool palette (`:671`), applies the deletion mirror
  (`:687-690`), reads the owing tail (`:692`), consults the budgets (`:693-698`), composes
  the stamp (`:703`), appends the block (`:725-754`), decides the deterministic delivery
  (`:760-768`) and emits the unlatch intent when the stamp took its own debt (`:769-773`).
- **The append is unconditional.** `append_consumer_block` has no uniqueness contract, and
  neither the origin column nor any other column is unique (`schema.rs:54-66`,
  `schema.rs:246-269`). `COLUMN_ORIGIN` is nullable because erasure nulls it (`kind.rs:746-786`).
- **A restart runs nothing by itself.** The framework's conversation actor is boot-latched:
  "nothing drives a conversation until an explicit intent (an append, a promotion, an
  unlatch) releases it, so a process restart cannot fire turns out of a ledger nobody asked
  to resume" (agent-ledger `actor.rs:1466-1469`). A turn owed when the process died is not
  resumed by starting the process; it waits for the next unlatch.
- **The stamp is readable back off the row.** `ChatMessage::own_debt_taken`
  (`kind.rs:538-540`) is the row-side spelling of the predicate `Stamp::own_debt_taken`
  (`kind.rs:354-356`) applies at the write, and the doc comment already names the three
  spellings that must agree.
- **The rights replies run after the append** (`assembly.rs:760-768`, `:1315`), with the
  state change applied "exactly when its reply is granted" and the comment at
  `assembly.rs:751-759` already reasoning about "the redelivered command".
- **An origin-keyed lookup inside one conversation already exists** as the mirror's erasure
  query (`kind.rs:746-786`): `WHERE origin = ?1 AND EXISTS (SELECT 1 FROM conversation_blocks
  ...)`. There is no index behind it; the only index on the message table is
  `(principal_id, addressed)` (`schema.rs:110`, `schema.rs:264-268`).

### The suite

`tests/adapter/offset.rs` owns the transport pins: the restart that re-ingests nothing
(`:18`), the crash-window duplicate (`:51`), the midway failure (`:90`), the transient
ingest failure (`:136`) and the malformed state file (`:201`). The loopback fake models the
poll faithfully — it confirms destructively at request arrival and answers what remains
(`tests/adapter/server.rs:530-575`) — scripts rate-limited polls (`:257`) and records every
request for assertion (`:269-283`).

### What adjacent specs already decided, and this one does not revisit

- **Telegram unit 09** decided that `chat_member` stays unsubscribed, because the platform
  requires the bot to be an administrator and the operator contract requires it not to be.
  Telegram unit 06 decided the same for `message_reaction` and `message_reaction_count`.
  This unit keeps both refusals and adds the mechanism that makes such a decision visible.
- **Telegram unit 03** owns what an edit means; `edited_message` stays subscribed and stays
  skipped at `translate.rs:125-127` until that unit changes it.
- **Telegram units 01 and 02** own media, and with it the case for a local Bot API server.
  This unit records what switching would cost but decides nothing about it.
- **Telegram unit 15** established the precedent for a startup platform call living as a
  fourth arm of the run future's `tokio::select!` (`driver.rs:283-287`).
- **Decision 0013** rejected webhooks for the first adapter unit; **decision 0014** put the
  offset in a file beside the process and accepted the duplicate. This unit refines the
  first and supersedes the second half of the second.

## Decisions taken with this unit

- **Long polling stays, and the refusal of webhooks becomes a property of the code,
  2026-08-27.** Decision 0013 rejected webhooks on deployment grounds; two stronger reasons
  have since appeared. First, ordering: the platform's own `update_id` documentation offers
  the identifier so a webhook consumer can "restore the correct update sequence, should they
  get out of order", and `max_connections` defaults to 40 simultaneous deliveries — a ledger
  whose conversation state is derived from block order would have to re-sequence what the
  poll delivers in order for free. Second, exposure: a webhook is an inbound public endpoint
  receiving members' message content, which changes the collection and security sections of
  the impact assessment, not only the deployment's wiring. The refusal becomes checkable: the
  wire client gains no `setWebhook` and no `deleteWebhook`, and a source scan in the adapter's
  test target asserts that no file in the crate names either method.
  *Rejected:* leaving the refusal as prose. Decision 0013 is prose, and prose did not stop
  this spec's author from having to check.
  *Rejected:* supporting both transports behind a configuration key. Two transports are two
  delivery contracts, one of which would never run in production and would rot untested.
- **The adapter refuses to start while a webhook is set on the token, and never removes it,
  2026-08-27.** A `getWebhookInfo` probe runs beside the identity read, before the first poll
  (`driver.rs:305`). A non-empty `url` refuses the start with a named error saying a webhook
  is set and that `deleteWebhook` clears it. The URL itself is never logged or put in an
  error: webhook URLs commonly carry a secret path segment, and this crate's rule is that no
  credential reaches a log line (`client.rs:1-9`). `pending_update_count` is logged at
  startup on the healthy path too, as the one cheap answer to "how far behind is it".
  *Rejected:* calling `deleteWebhook` automatically. It would take a token another process
  may be serving, and two processes each repairing the other's setting would flap; whether
  the pending queue is dropped along with it is an operator's decision, not a startup's.
  *Rejected:* leaving the failure to the poll. Today that is silent deafness with a two-second
  retry, which is the exact failure this unit was asked to find.
- **A poll failure that repeats re-probes the webhook and says which cause it is,
  2026-08-27.** After three consecutive failed polls the loop calls `getWebhookInfo` once and
  logs the diagnosis: a webhook is set, or the token is empty of one and the failure is
  something else — most likely a second poller on the same token, which produces the same
  symptom. The repeat-failure backoff widens from two seconds to thirty for as long as the
  failures continue, because neither cause clears within two seconds and re-asking at that
  rate makes a rate limit or a two-poller conflict worse. A successful poll resets both.
  *Rejected:* classifying the failure by matching its description text. The platform
  documents neither the status nor the wording for a webhook conflict. The formatting
  fallback at `client.rs:478-485` does match description text, and it may: a misclassified
  formatting refusal costs one plain re-send. A misclassified transport failure costs the
  whole channel.
  *Rejected:* refusing to run on a repeated failure. A network outage is a repeated failure
  too, and the poll's job is to survive one.
- **`decode` reports the API's own description for a failed status, 2026-08-27.** For a
  non-success status the client reads the envelope and, when it carries one, returns
  `ClientError::Refused` with the description and the status; only a body that does not
  decode falls back to today's status-only error. The description is documented ("the error
  is explained in the 'description'"), it is what makes an operator's log line actionable,
  and it is the platform's own text, so it carries no credential of ours.
- **One declaration decides the subscription, the decode and the translation, 2026-08-27.**
  The three update types become a single declarative list in the wire client naming, for each
  type, its wire name and its payload type. From that list a macro generates the `Update`
  struct's optional fields, the `CONSUMED_UPDATE_TYPES` array sent as `allowed_updates`, and
  a borrowed `Payload` enum with one variant per declared type plus an accessor returning the
  one present payload. `translate` matches that enum exhaustively, so adding a type to the
  list fails the build until the translation handles it, and removing one from the
  subscription is impossible while its decode exists. An update carrying none of the declared
  payloads — the "unwanted updates may be received for a short period of time" the platform
  warns about after a subscription change — yields `None` and the existing `Skip::NonMessage`.
  *Rejected:* keeping two hand-written lists with a comment asking for care. That is the
  present state, and it is the reason a future unit can add a decode field, forget the
  subscription, and ship a feature that never fires while every test passes.
  *Rejected:* sending an empty `allowed_updates` to receive "all update types except
  chat_member, message_reaction, and message_reaction_count". It enrolls the deployment in
  every type the platform ever adds, all of them silently discarded by the decoder, and it
  makes the set of things this assistant sees depend on the platform's release notes instead
  of on a decision.
  *Rejected:* asserting our list against `WebhookInfo.allowed_updates` at startup. That field
  is documented as "a list of update types the bot is subscribed to", but no documented
  relationship ties it to the list a `getUpdates`-only bot passes per call, and building a
  startup check on an unstated relationship invents a fact.
- **`limit` is stated at 100 and `timeout` stays at 25, 2026-08-27.** The poll sends
  `limit: 100` explicitly, for the same reason `allowed_updates` is sent explicitly: a
  default that is not written down is a decision nobody made. One hundred is the platform's
  maximum and the current effective value, so the wire behaviour does not change.
  *Rejected:* a small limit to shorten the replay window. The replay window stops being a
  correctness concern once the append is idempotent, and a small limit buys nothing but round
  trips on a busy group.
- **The offset file becomes advisory; correctness moves to the ledger, 2026-08-27. This
  supersedes the second half of decision 0014.** The file keeps its job — after a restart,
  do not re-read up to a day of already-handled updates — and loses its claim to
  correctness. Absent, empty, malformed, stale or written by a crash halfway through a batch:
  every one of those costs a replay and nothing else, because the append below absorbs the
  replay. No fsync is added, and the reason is now positive instead of resigned: the file is
  a hint, and a lost hint costs a bounded replay.
  *Rejected:* moving the resume point into the store. It adds core surface for a fact the
  core has no use for, and it would still not be atomic with the append — the framework store
  is one writer serving independent closures (`store/mod.rs:178`, `store/mod.rs:704-731`), so
  two calls are two writes whatever table they touch. Two records that can disagree is what
  this decision removes, not what it should add.
  *Rejected:* keeping "the duplicate is the accepted outcome". Priced above: a second answer
  debt against a member's budget, a second block, the same question twice in the projection.
- **The append is idempotent on the recorded origin within its conversation, 2026-08-27.**
  Under the existing stamp lock, after the suppression re-read (`assembly.rs:664`) and before
  the palette reconciliation and the deletion mirror, `ingest` looks for a message block in
  this conversation whose stored origin equals the incoming one. When one exists the block is
  not appended a second time; everything else about the call proceeds from what that row
  records (next two decisions). The lock is what makes the check sound — it is already held
  from the tail read through the append precisely so no concurrent ingestion can slide a
  block in between (`assembly.rs:653-659`) — and the ordering is sound because the mirror
  runs before the append, so an existing row proves the mirror already ran. A message the
  adapter delivers without an origin is appended as today; the platform always carries one
  for a message, and the core does not assume it.
  *Rejected:* a table of consumed update identifiers. It is a second durable record of
  something the ledger already holds, it grows one row per member message forever, and it
  would need its own erasure story for identifiers that describe a person's messages.
  *Rejected:* remembering identifiers in the adapter's memory. Redelivery happens exactly
  when the process died, which is exactly when that memory is gone.
  *Rejected:* a UNIQUE index on the origin column. Erasure nulls origins, so uniqueness
  would quietly stop applying to erased rows; origins are unique per channel only, so the
  constraint would have to be composite with the conversation, which the content table does
  not carry; and a constraint refuses a write where this unit wants a no-op.
- **A redelivered message re-emits the answer intent from the recorded stamp, never a
  recomputed one, 2026-08-27.** The duplicate path reads `own_debt_taken` off the stored row
  (`kind.rs:538-540`) and emits `CoreEvent::UnlatchRequested` when it is true. This is not
  bookkeeping: the conversation actor is boot-latched, so a turn owed when the process died
  stays owed until an unlatch arrives (`actor.rs:1466-1469`). Without this emission the
  contract would be "loses nothing" only for messages whose turn had already finished, and a
  member whose question crashed the process would never be answered.
  *Rejected:* recomputing the stamp on the duplicate path. The budget count now includes this
  very message's stored debt, so the recomputation can conclude that the budget refuses it,
  drop the unlatch, and leave the crashed turn owed forever. The ledger's record of what was
  decided is the answer; deciding again is how the two disagree.
  *Rejected:* emitting the unlatch on every duplicate. An unsummoned message never unlatches
  by design (`assembly.rs:606-613`), and a rule that holds on the first delivery must hold on
  the second.
- **The deterministic answer on the duplicate path is decided from the recorded row too,
  2026-08-27.** The command family is derived from the message as it always is, and the
  budget half that `notice_admitted` needs (`assembly.rs:711-716`) is read from the stored
  row's recorded limitation instead of a fresh consultation. The delivery match at
  `assembly.rs:760-768` is extracted into one method that both paths call, so there is one
  place where a command's deterministic answer is decided. The privacy self-service replies
  therefore still run on a redelivered command, and that is the point: the state change and
  the reply happen after the append (`assembly.rs:755-768`), so a crash between the two would
  otherwise lose an erasure request permanently. Re-running them is safe — nulling
  already-null columns is a no-op (`kind.rs:740-744`), the suppression flag is idempotent,
  and the per-person reply window bounds the answer.
  *Rejected:* returning no delivery for a duplicate. It makes a member's rights request
  depend on where the process happened to die.
  *Rejected:* a new `IngestOutcome` variant for the duplicate. `Recorded` is a statement
  about the ledger's state, which is true either way; a second variant would make every
  caller re-decide something the core already decided.
- **An erased person's redelivered message is disregarded by the check that already exists,
  2026-08-27, and the residual is named.** Because origin is nulled by erasure
  (`kind.rs:751-758`), the idempotent append cannot recognise a redelivered message whose row
  was erased. It does not need to in the normal case: erasure leaves a suppression stub while
  the flag stands (decision 0074), and the under-lock suppression re-read disregards the
  message before any append (`assembly.rs:664-667`). This unit pins that as a property
  instead of leaving it as a coincidence. The residual, stated and not hidden: a person
  who erased their data and then opted back in could have a message from before the erasure
  re-recorded, if the platform still holds that update within its 24-hour window and the
  offset file was lost in the same interval. Narrow, dependent on three conditions at once,
  and not closable without storing an identifier of the erased message — which is the thing
  erasure removed.
- **Nothing drops pending updates automatically, 2026-08-27.** No startup uses a negative
  offset, and no code path calls `deleteWebhook` with `drop_pending_updates`. A backlog the
  assistant cannot process is a member's messages, and discarding them is an operator's
  decision made once, by hand, with the platform's own documented mechanism.
  *Rejected:* starting from `-1` to "catch up quickly" after a long outage. It forgets every
  earlier update, which is a data loss chosen by a program to save itself some work.
- **The 24-hour bound is written into the operator reference, not defended in code,
  2026-08-27.** The group operator contract gains one paragraph: updates not collected within
  a day are gone, so a process kept down longer than that loses messages, and nothing in the
  ledger will show a hole.
  *Rejected:* appending a "messages may have been lost" note to the ledger after a long
  restart. It would put a claim on a permanent record that no observation can support, since
  identifier gaps are normal under a filtered subscription.
- **`chat_member`, the reaction updates and the remaining twenty-one types stay
  unsubscribed, 2026-08-27.** Named here only so the closed set is visible in one document.
  The reasons belong to Telegram units 09 and 06 and are not re-argued; the new declaration
  makes any future change to the set a single visible edit.

## The unit's contract

The poll asks for exactly the update types the adapter can translate, from one declaration
that also generates the decode and forces an exhaustive translation, with `limit`, `timeout`
and `allowed_updates` all stated on every call and never inherited from the token's previous
setting. The adapter refuses to start while a webhook is set on the token, never sets or
removes one, logs how many updates were waiting, and — when polls keep failing — says which
of the two indistinguishable causes it is instead of repeating an unnamed status forever. A
delivered update is recorded at most once: the persisted offset is an advisory hint whose
loss costs a replay, and the ledger decides, from the origin already stored on the message
block, whether this message is a first delivery or a redelivery. A redelivery appends
nothing, re-emits the answer intent when the recorded stamp says a debt was taken, and
re-runs the deterministic reply the recorded row says was owed, so a crash between the append
and the answer costs neither the record nor the reply. Within the platform's 24-hour
retention, a restart loses nothing and replays nothing twice. Beyond it, the loss is the
platform's, is documented in the operator reference, and is not papered over by a claim the
code cannot support. No new data leaves the machine, no new category is stored, no core
vocabulary is added, and no file or byte stream is introduced by this unit.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan and the token scan clean; no new dependency.
- **AC2** The wire poll is fully stated: a recorded `getUpdates` body carries
  `allowed_updates` equal to the declared type names, `limit` 100 and `timeout` 25, and
  carries no `offset` on the first poll of a run with no state file — pinned off the fake's
  request recorder (`tests/adapter/server.rs:269-283`).
- **AC3** The subscription cannot drift from the decode: the declared list is the only place
  the three type names appear in the crate, the generated array is what the poll sends, and
  the translation matches the generated payload enum exhaustively — pinned by a test
  asserting array and enum agree, and by the build failing if a declared type has no
  translation arm.
- **AC4** An update carrying an unrequested or unknown payload is skipped, does not stop the
  batch, and advances the offset past itself — pinned with an update the fake serves whose
  only field is an unknown one.
- **AC5** Exactly-once on redelivery: the crash-window scenario of
  `tests/adapter/offset.rs:51` is rewritten to assert that the redelivered update leaves one
  message block, not two, and that the sender's counted answer debt is one — the current pin
  of the accepted duplicate is replaced, and the replacement names the superseding decision.
- **AC6** No answer is lost to the crash window: an addressed message whose block was
  appended before the process died is answered after the restart, because the redelivery
  re-emits the answer intent from the recorded stamp — pinned end to end against the
  boot-latched conversation, with the answer delivered exactly once.
- **AC7** An unsummoned redelivered message emits no answer intent, and a redelivered
  privacy self-service command is answered again within its own per-person window — both
  pinned.
- **AC8** The state file is advisory: absent, empty, malformed and stale-behind-by-a-batch
  all produce a replay with no duplicate block and no second debt — the malformed-file pin
  at `tests/adapter/offset.rs:201` extended to assert the ledger, not just the restart.
- **AC9** The startup probe: a fake `getWebhookInfo` answering a non-empty `url` refuses the
  start with an error naming the condition and the remedy, and neither the error nor any log
  line contains the URL; an empty `url` starts normally and logs the pending count — pinned,
  with the URL string scanned for in the captured log output the way the token scan does it.
- **AC10** A repeatedly failing poll re-probes once and names the cause, and the backoff
  widens to the repeat interval and resets on the first successful poll — pinned on the
  injectable sleep's recorded waits, with no busy spin.
- **AC11** A non-success status carrying an API description surfaces the description in the
  error, and a body that does not decode still yields the status-only error — pinned.
- **AC12** An erased person's redelivered message records nothing while their suppression
  stub stands — pinned through the existing erasure fixtures.
- **AC13** The idempotency lookup is indexed: the appended migration step creates the origin
  index, the schema pin reads it back by name, and the step is appended after
  `LITERAL_ADDRESSED_MIGRATION` with every earlier step's generated SQL byte-identical
  (decision 0026's discipline).
- **AC14** No privacy document required a change, and the unit says why in one paragraph: no
  new category is stored, no new recipient receives anything, nothing new is sent to the
  model, and the advisory file still holds one integer. The group operator contract gains the
  24-hour paragraph.

## Notes for launch

- **Wire client** (`crates/adapters/telegram/src/client.rs`): the declarative list and its
  macro replace `CONSUMED_UPDATE_TYPES` (`:103`) and the `Update` struct (`:108-121`), and
  produce the payload enum the translation matches; `get_updates` (`:313-326`) gains
  `limit`; `decode` (`:561-584`) reads the envelope for a failed status; a new
  `get_webhook_info` follows the shape of `get_me` (`:304-307`) and passes no wait ceiling,
  like the other startup call. No `set_webhook` and no `delete_webhook` are added, and the
  source scan that asserts their absence belongs beside `tests/token_scan.rs`.
- **Loop** (`crates/adapters/telegram/src/driver.rs`): the probe runs beside
  `fetch_identity` (`:305`) before `poll_loop`'s first iteration; the consecutive-failure
  counter, the widened backoff and the one re-probe live in the failure arm at `:311-315`;
  the per-update advance and the post-batch persist (`:318-335`) are unchanged in shape, and
  their module doc (`:6-12`) is rewritten to state the new contract instead of the accepted
  duplicate.
- **State** (`crates/adapters/telegram/src/state.rs`): behaviour unchanged, module doc
  rewritten — the file is a hint, the ledger is the record. The same correction belongs on
  `Config::state_file` (`lib.rs:62-65`) and on the configuration key's doc
  (`crates/assistant/src/config.rs:27-28`).
- **Core** (`crates/core/src/assembly.rs`): the existence check and the append branch sit
  between `:664` and `:671`; the delivery match at `:760-768` is extracted into one method
  both paths call; the unlatch at `:769-773` reads the stored row's predicate on the
  duplicate path. Nothing above the stamp lock moves.
- **Kind and schema** (`crates/core/src/kind.rs`, `schema.rs`): the by-origin loader goes
  beside `erase_message_named` (`kind.rs:746-786`) and reuses its `conversation_blocks` join;
  the index step is appended to the migration list at `schema.rs:373-397`, named the way
  `PRINCIPAL_ADDRESSED_INDEX` is named (`schema.rs:110`).
- **Suite**: `tests/adapter/offset.rs` is the unit's home — rewrite `:51`, extend `:201`, add
  the redelivered-answer and unsubscribed-type pins. `tests/adapter/server.rs` gains a
  `getWebhookInfo` handler with a scripted URL, a scripted poll failure that is not a rate
  limit, and keeps its existing recorder for AC2.
- **Decision records**: one superseding decision for 0014's duplicate half, one refining
  0013 with the ordering and exposure arguments, and one for the single declaration. Numbers
  continue from the highest free at merge time; Telegram unit 07 has already reserved a range
  it may not still be entitled to, so check before assigning.
- The whole unit moves JSON envelopes measured in kilobytes and no file bytes at all, so the
  streaming constraint has nothing to bind here. If the local Bot API server is ever adopted
  for the media units, `logOut` must run first, the file path semantics change, and that is
  their decision to record, not this one's.
