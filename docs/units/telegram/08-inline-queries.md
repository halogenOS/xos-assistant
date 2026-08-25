# Telegram unit 08 — inline queries: the answer is no, and the refusal is made checkable

Date: 2026-08-25. Every other unit in this series adds a capability. This one examines a
capability and declines it, then spends its implementation making the decline a property of
the code instead of an accident of it. Inline mode lets a person type the assistant's
username in the input field of **any** chat — a chat the assistant was never added to, a
private conversation between two strangers, a channel — and receive results the person then
sends under their own name. The Bot API side is small and well documented. The problem is
everything the query does not carry: an inline query names no chat, so decision 0052's
per-group admission has nothing to check; it names a person who may never have written a
word to the assistant, so authority, budgets and erasure have no conversation to key on; and
the published statements in the privacy documents describe a bot that reads a community
group, not a bot that reads what people type in chats it was never invited to.

The unit therefore ships four small things: the recorded decision, a pinned refusal in the
poll, a pinned fail-safe for the one window where an inline update could arrive anyway, and
a startup notice so a token whose inline mode was switched on elsewhere cannot stay switched
on unnoticed. It also writes down, precisely, what would have to be true before anyone
reopens the question, so the next person to ask is not left re-deriving this from scratch.

## The finding

**An inline query has no chat identifier at all, and every admission, protection and erasure
mechanism in this core keys on a conversation.** `InlineQuery` carries `id`, `from`, `query`,
`offset`, an Optional `chat_type` and an Optional `location` — and `chat_type` is a *kind*
("private", "group", "supergroup", "channel", or "sender"), never an identity. There is no
chat id, no chat title, nothing that could be turned into a `ChannelKey`
(`message.rs:16-21`).

That single absence disables the project's whole safety structure at once:

1. **Admission cannot run.** `is_authorized` asks whether the operator admitted *this
   channel* (`authorization.rs:60-73`, "Absence is refusal"), and the withdraw directive it
   returns leaves *that group*. With no channel there is nothing to look up and nothing to
   leave — the fail-closed refusal that protects every other inbound path is unavailable by
   construction, not merely unimplemented.
2. **Authority cannot resolve.** Standing is read from the chat's administrator list
   (decision 0015, `driver.rs:405-424`). No chat, no list, no standing — every tool would run
   at the floor, which is the degraded behaviour unit 07 already identified as wrong and not
   merely limited.
3. **The budgets cannot bind.** Both counts key on a principal *and* a conversation
   (`kind.rs:950`, `kind.rs:978`). An inline surface is reachable by every Telegram account in
   existence, and the per-conversation half of the protection would have nothing to count.
4. **Erasure and the rights path have no reach.** Erasure removes a person's direct
   conversations and nulls their message columns (`erasure.rs:1-40`); the privacy commands are
   messages in a conversation (`privacy.rs:97-101`). A person known only from inline queries
   has no conversation, so they could neither be told what is stored about them nor reach the
   assistant to ask.
5. **The first-interaction disclosure cannot be discharged.** The AI Act record states the
   line is "resolved per person from the ledger's own memory" (`docs/compliance/ai-act.md` §3).
   An inline answer has no ledger memory of the person and no message of its own to carry the
   line.

None of that is fixed by adding an inline branch to the core. The neutral vocabulary for the
thing would be an **unplaced request** — a request from a person that belongs to no
conversation — and naming it is the easy half. The hard half is that admission, authority,
protection, disclosure and erasure would each need a second answer for the unplaced case,
which doubles the core's admission model to serve a surface nobody asked for. That is the
structural reason this unit says no; the privacy reasons below are independent and each
sufficient on their own.

## Grounding

### The platform, read 2026-08-25

Fetched from `core.telegram.org/bots/api`, `core.telegram.org/bots/inline` and the changelog
at `core.telegram.org/bots/api-changelog` on 25 August 2026. The brief for this series named
Bot API 10.1 (11 June 2026) as current; the changelog's newest entry is **Bot API 10.3, dated
24 August 2026**, with 10.2 on 14 July 2026. Every sentence in quotation marks was read from
those pages on that date.

- **Two update types exist and both are off by default.** The `Update` table lists
  `inline_query` — "New incoming inline query" — and `chosen_inline_result` — "The result of
  an inline query that was chosen by a user and sent to their chat partner. Please see our
  documentation on the feedback collecting for details on how to enable these updates for your
  bot." Neither carries an administrator condition, unlike `message_reaction` and
  `chat_member`, and `getUpdates` confirms both are in the default set: "Specify an empty list
  to receive all update types except `chat_member`, `message_reaction`, and
  `message_reaction_count` (default)."
- **No API method turns inline mode on.** The inline section states: "To enable this option,
  send the `/setinline` command to @BotFather and provide the placeholder text that the user
  will see in the input field after typing your bot's name." The switch lives with whoever
  holds the token in BotFather, not in any code this repository can write. Whether it is on is
  readable, though: `User.supports_inline_queries` — "True, if the bot supports inline
  queries. Returned only in getMe."
- **The reach is the whole platform, per token.** The inline documentation states that people
  "can request content from your bot in **any** of their chats, groups, or channels without
  sending any messages at all". There is no per-chat, per-group or per-person scoping of any
  kind: inline mode is one switch on the token, and it is on everywhere or nowhere.
- **`InlineQuery` names no chat.** Its fields are `id` ("Unique identifier for this query"),
  `from` ("Sender", a full `User` with id, first and last name, username, language code and
  premium flag), `query` ("Text of the query (up to 256 characters)"), `offset` ("Offset of
  the results to be returned, can be controlled by the bot"), and two Optional fields:
  `chat_type` — "Type of the chat from which the inline query was sent. Can be either
  'sender' for a private chat with the inline query sender, 'private', 'group', 'supergroup',
  or 'channel'. The chat type should be always known for requests sent from official clients
  and most third-party clients, unless the request was sent from a secret chat" — and
  `location` ("Sender location, only for bots that request user location", enabled separately
  with `/setinlinegeo`).
- **`answerInlineQuery(inline_query_id, results, cache_time, is_personal, next_offset,
  button)`** returns True. "No more than **50** results per query are allowed." `cache_time`
  is "The maximum amount of time in seconds that the result of the inline query may be cached
  on the server. **Defaults to 300**." `next_offset`: "Offset length can't exceed **64
  bytes**." A result's own `id` is "Unique identifier for this result, **1-64 Bytes**". There
  are **20** result types; the only one relevant to a text assistant is
  `InlineQueryResultArticle` carrying `InputTextMessageContent` ("Text of the message to be
  sent, 1-4096 characters"). The API adds: "Note: All URLs passed in inline query results will
  be available to end users and therefore must be assumed to be public."
- **The default cache serves one person's answer to another person.** `is_personal`: "Pass
  True if results may be cached on the server side only for the user that sent the query. **By
  default, results may be returned to any user who sends the same query.**" So without
  `is_personal`, an answer computed for one person is redistributed by the platform to anyone
  typing the same words, for `cache_time` seconds, with no further call to the bot. The inline
  documentation states the consequence for feedback plainly: "you may receive more results
  than actual requests due to caching (see the `cache_time` parameter in
  `answerInlineQuery`)."
- **The record of what the assistant published cannot be complete.** `chosen_inline_result`
  requires "/setinlinefeedback", and the same page recommends sampling for popular bots:
  "we recommend adjusting the probability setting to receive 1/10, 1/100 or 1/1000 of the
  results." `ChosenInlineResult` carries `result_id`, `from`, Optional `location`, Optional
  `inline_message_id` ("Available only if there is an inline keyboard attached to the
  message") and `query` — and **no chat of any kind**. So even with feedback on, the bot
  learns that somebody sent one of its answers somewhere, never where. The API repeats the
  precondition: "Note: It is necessary to enable inline feedback via @BotFather in order to
  receive these objects in updates."
- **The switch-to-private-chat affordance is a deep link, nothing more.**
  `InlineQueryResultsButton` — "You must use exactly one of the optional fields" — carries
  `text`, an Optional `web_app`, and an Optional `start_parameter`: "Deep-linking parameter
  for the /start message sent to the bot when a user presses the button. 1-64 characters, only
  A-Z, a-z, 0-9, _ and - are allowed." The documented example is an OAuth setup flow. Pressing
  it opens a private chat with the bot and sends `/start <parameter>` as an ordinary message.
- **The documentation states no deadline for answering an inline query** and no error text for
  answering late. The whole method section was read for such a sentence and there is none —
  recorded as undocumented, not as a number. What is certain from the mechanism is that a
  client re-queries as the person types, so an answer that arrives seconds later answers a
  query the person has already replaced.
- **Administrators can only block inline bots wholesale.** `ChatPermissions` and
  `ChatMemberRestricted` carry one relevant right: `can_send_other_messages`, "True, if the
  user is allowed to send animations, games, stickers and use inline bots." A group that wants
  to stop this assistant appearing inline must stop stickers, games and every other inline bot
  for its members at the same time. There is no per-bot control.
- **The adjacent mechanism, named so it is not confused with this one.** Bot API 10.0 (8 May
  2026) "Introduced support for guest mode, allowing bots to receive certain messages and
  issue replies within chats they are not a member of", adding `Update.guest_message`,
  `Message.guest_query_id`, the method `answerGuestQuery`, and `User.supports_guest_queries`
  ("True, if the bot supports guest queries from chats it is not a member of. Returned only in
  getMe"). It is a different door into the same room. This unit reads its capability flag and
  designs nothing for it; see the decisions.

### Our tree, at `1891fcd`

- **The poll names its update types on every request, so the two inline types never arrive
  today.** `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]`
  (`client.rs:103`), passed as `allowed_updates` in `get_updates` (`client.rs:313-320`)
  because "an absent selection would inherit whatever an earlier setting left on the token"
  (`client.rs:99-102`). An adapter test already asserts the exact list on the wire:
  `the_poll_names_the_update_types_it_consumes`
  (`crates/adapters/telegram/tests/adapter/group_context.rs:22-35`).
- **An inline update that arrived anyway is already skipped — by accident, not by design.**
  The decoded `Update` has three optional fields and "Unknown fields are ignored by the
  decoder, so the model stays exactly as small as the translation needs"
  (`client.rs:105-121`). An update carrying only `inline_query` decodes with all three absent,
  falls through `translate` to `Translation::Skip(Skip::NonMessage)` (`translate.rs:119-128`),
  and `process` acknowledges it and advances the offset (`driver.rs:367-371`). Nothing is
  fetched, nothing is written, nothing is answered. This is the behaviour the unit wants; it
  is currently untested and undocumented, which is the whole reason to pin it.
- **The window where that matters is documented.** `getUpdates` warns: "Please note that this
  parameter doesn't affect updates created before the call to getUpdates, so unwanted updates
  may be received for a short period of time." So a token that had inline mode and a wider
  `allowed_updates` set earlier can deliver inline updates into this adapter's first polls.
- **`getMe` already runs once at poll start**, retried on the poll backoff until it answers,
  because "translating blind would record wrong facts into a durable ledger"
  (`driver.rs:342-355`, `client.rs:304-307`). It decodes into `BotIdentity`, three fields, each
  Optional-tolerant so "a malformed answer degrades ... instead of refusing to decode"
  (`client.rs:227-239`). Adding one Optional boolean to that decode costs nothing and needs no
  new call.
- **Every core entry point requires a channel.** `Assistant::ingest` takes an `InboundMessage`
  (`assembly.rs:623`), whose first field is `channel: ChannelKey` (`message.rs:171-177`), and
  the mapping module is "the one place a channel key is stored" (`mapping.rs:1-8`), resolving
  key to conversation on the way in. There is no entry point that accepts a request without
  one, and `ObserveOutcome`/`IngestOutcome` (`message.rs:271-310`) have no variant for a
  request with nowhere to belong.
- **Tools are admitted per conversation and checked at the call.** "Required authority is
  enforced at the call, per decision 0043: every wrapped execute reads the turn's provenance
  through the call block's dispatch anchor ... and declines when it falls below the tool's
  required authority" (`tools/mod.rs:19-24`), and "A conversation without a palette admits
  nothing" (`tools/mod.rs:16-17`). So even the most harmless conceivable inline answer — a
  link to a public wiki page — has no path to the wiki lookup that does not first invent a
  second, conversation-free calling path beside the existing one.
- **The model is not a fast path, by this repository's own numbers.** The shortest, most
  bounded model call in the tree is the rules acknowledgment, and it budgets
  `GENERATION_TIMEOUT: Duration = Duration::from_secs(10)` (`acknowledgment.rs:40`). The
  ordinary answering path is a streamed turn that may call tools, and decision 0103 stops and
  resumes it per tool call. Nothing here can answer a keystroke.
- **There is no start command.** The core's whole command vocabulary is the five privacy
  commands (`privacy.rs:97-101`). A `/start <parameter>` from a deep link is not an invocation
  the core recognises; it would be recorded verbatim as what the person said (decision 0017),
  putting a machine-generated token on the ledger as a person's words. And direct chats may be
  switched off entirely (decision 0069), in which case the deep link leads to a chat that
  records nothing and answers nothing.
- **The published privacy statements describe a different bot.** The record of processing
  describes the activity as "A bot in the halogenOS community groups stores the groups'
  messages" (§2) and enumerates its data subjects as group members and "People who write to
  the assistant directly" (§4, S1–S2). The legitimate-interests assessment rests its balancing
  on exactly that: "Messages people chose to post to an open community group, in front of
  every other member. This is **not private correspondence**, not observed behavior, not data
  collected behind someone's back" (§4.1), and "A person joining a project's support group
  where an announced assistant answers questions expects to be read by that assistant. Nobody
  is surprised" (§4.2). Text typed into the input field of a stranger's private chat is
  private correspondence by any reading, and the person typing it has had no notice from any
  pinned rules. The record's review triggers name this case directly: "a change to what is
  collected" and "a new path that sends message content off the machine" (§11).
- **The precedent for declining half a platform feature is in this series already.** Unit 06
  ships the placing half of reactions and refuses the receiving half, on the reasoning that
  subscribing to updates that cannot arrive would add "two decode paths and two translation
  branches that never execute — dead code with a privacy notice attached to it", and pins the
  refusal as its AC12 with the reason written into the assertion. This unit follows that shape
  exactly.
- **The adapter suite has no live-endpoint path**: the scripted server binds loopback and
  "Nothing here leaves the machine" (`crates/adapters/telegram/tests/adapter/server.rs:1-11`),
  so every criterion below is provable in the suite as it stands, with no live call before
  merge.

## Decisions taken with this unit

- **Inline mode stays off; the assistant answers only where it was admitted, 2026-08-25.**
  Three independent reasons, each sufficient. First, admission: decision 0052 makes the
  operator's invitation a durable fact and refuses every group without one, and an inline
  query carries no chat to check — so the one surface with no admission would be the one
  reachable by everybody. Second, effectiveness of withdrawal: an inline result is sent by the
  *person*, so a group that removed the assistant could still have its content posted there by
  any member, and the only platform control is `can_send_other_messages`, which blocks every
  inline bot and stickers besides. Third, the balancing: the legitimate-interests assessment
  rests on messages posted openly in a community group and says in terms that this is "not
  private correspondence"; inline queries are typed in private chats, so shipping this would
  make a published assessment untrue on the day it merged, which is a defect and not a
  follow-up. *Rejected:* enabling it and answering only queries from people the assistant
  already knows — the query names no chat, and recognising a person would mean creating or
  looking up an identity row for every stranger who types the bot's name anywhere, which is
  precisely the collection unit 07 refuses for a stranger's button press. *Rejected:*
  enabling it and answering nothing — the placeholder text appears in every chat on the
  platform, advertising a capability the assistant declines to perform, and the queries reach
  the platform's servers either way. *Rejected:* holding the whole question open as a
  follow-up — follow-ups record accepted shortfalls in shipped work; this is a decision, and
  leaving it unrecorded means the next unit re-derives it or, worse, ships it.
- **The two inline update types are never subscribed, and the refusal is asserted on the wire,
  2026-08-25.** `CONSUMED_UPDATE_TYPES` gains nothing; a new assertion states that the list
  sent as `allowed_updates` contains neither `"inline_query"` nor `"chosen_inline_result"`,
  with the reason in the assertion's own message so a later change has to read it before
  removing it. The assertion is written as a containment check, not an equality check: units
  05, 06 and 07 are each adding or considering entries, and an exact-list assertion in this
  unit would collide with all of them. *Rejected:* an exact-list assertion matching the
  existing wire test (`group_context.rs:22-35`) — it would fail the moment any sibling unit
  merges, and this unit's claim is about absence, not about the list's contents.
- **The arriving-anyway case is pinned as a skip, not given a decode path, 2026-08-25.** The
  documented `allowed_updates` transition window means an inline update can reach the poll on a
  token that had inline mode on before. Today that update decodes with every known field
  absent and is skipped anonymously as `Skip::NonMessage` — correct behaviour that nothing
  proves. This unit pins it: a scripted update carrying only an `inline_query` object is
  acknowledged, advances the offset, and causes no request of any kind. *Rejected:* adding an
  `inline_query` field to the decoded `Update` and a named `Skip::InlineQuery` reason, the way
  `edited_message` earns a named skip (`client.rs:117-118`) — that field exists because edits
  arrive constantly on a subscribed type, whereas this one would decode a query text the
  assistant has decided not to receive, on a path that runs only in a misconfiguration. Unit
  06's reasoning against dead paths applies unchanged. *Rejected:* leaving it untested — the
  fail-safe currently depends on the decoder ignoring unknown fields, which is a property of
  the deserializer's configuration, not a stated intention.
- **A token with inline mode on is reported at startup, loudly, and the process still runs,
  2026-08-25.** `getMe` already runs once before the first poll and its answer decodes into
  `BotIdentity`; this unit adds one Optional boolean, `supports_inline_queries`, decoded
  leniently like every other field there, and one error-level log line at poll start naming
  the finding and the BotFather remedy. The reading is a pure function over the decoded
  identity so it can be pinned without capturing logs. This is not behaviour in the adapter: it
  decides nothing, changes no outcome for any message, and speaks to whoever runs the process,
  never to a member — the wording rule that keeps user-facing text in the core is about text
  addressed to people. *Rejected:* refusing to start — that is an adapter deciding to take the
  community's assistant offline over a setting that, with the update types unsubscribed, leaks
  nothing, and an outage costs the community more than a broken affordance does. *Rejected:* no check at all — the switch
  lives in BotFather, outside every file this repository controls, so without this line nobody
  would ever learn it had been flipped. *Rejected:* re-checking periodically — one read at
  startup matches the lifetime of every other identity fact the poll holds, and the manager
  restarts sessions anyway.
- **The same startup reading covers guest mode, and guest mode is designed nowhere here,
  2026-08-25.** `supports_guest_queries` names the same hazard through a different mechanism —
  replies inside chats the assistant is not a member of — and reading it is the same one line
  in the same decode. Reading it is in scope; deciding what guest mode should do is not, because
  a guest message, unlike an inline query, does carry a chat and therefore deserves its own
  examination instead of an answer inherited from this one. The startup line names whichever
  flags are set. *Rejected:* reading only the inline flag, which would let the neighbouring hole
  open in silence. *Rejected:* specifying guest mode here to "finish the subject" — it is a
  different mechanism with a different admission story, and folding it in would hide a real
  decision inside a unit about something else.
- **The switch-to-private-chat affordance is refused with the rest, and its landing place is
  named as missing, 2026-08-25.** Even if inline mode were on, `InlineQueryResultsButton`'s
  deep link has nowhere to land: the core has no start command (`privacy.rs:97-101`), so
  `/start <parameter>` would be stored verbatim as a person's own words (decision 0017), and
  with direct chats switched off (decision 0069) the tap opens a chat that records nothing and
  says nothing. *Rejected:* adding a start command with this unit so the affordance would work
  — that is a direct-chat onboarding feature wearing an inline-mode costume, and it would put a
  first-contact surface in front of strangers without the disclosure work unit 12 governs.
- **Nothing about a person is received or recorded, so no privacy document changes with this
  unit — and the reversal list is written down instead, 2026-08-25.** The assistant receives no
  inline update, answers none, and stores nothing new, so no new category of data, no new
  recipient and no new storage exists; none of the record's review triggers fires. Writing an
  amendment saying "we considered a thing and did not do it" would put a non-event into a
  register of processing activities. What the unit does instead is the section below, which
  names every document and every clause a reversal would have to change first. *Rejected:* a
  privacy-document note for completeness — a record of processing describes processing, and
  padding it with refusals makes the real entries harder to audit.
- **Nothing streams, and nothing here ever will, 2026-08-25.** This unit moves no bytes: it
  adds one boolean to an existing decode and one log line. Recorded because the constraint binds
  every spec. If the question is ever reopened, note that inline results carry URLs and not
  uploads — "All URLs passed in inline query results will be available to end users and
  therefore must be assumed to be public" — so a future inline answer would reference public
  files by address and would still upload nothing.

## What would have to be true before this is reopened

Refusing without naming what could work is refusing without examining. The one shape that
could conceivably be defensible is narrow: **article results linking to public project wiki
pages**, filtered by title from the page list unit 17 enumerates, with no model in the path, no
ledger write, no personal data recorded, `is_personal` left false and a long `cache_time`,
since the answer is the same for everyone. Its content would carry nothing about any person.
Even that shape is blocked today, and the checklist is the useful part:

1. **A conversation-free calling path for a lookup would have to exist and be justified on its
   own.** Tools are admitted per conversation and checked at the call against the turn's
   provenance (`tools/mod.rs:16-24`). A second calling path invented beside it, for one caller,
   is the bolted-on shape this project refactors away from — so it would need to be a seam the
   core wants for other reasons, not a special case for this.
2. **The reach problem would have to be answered, not waved past.** Inline mode is one switch
   on the token with no per-chat scope; enabling it for the community group means enabling it
   for everyone on the platform. Any reopening has to state plainly that this is accepted.
3. **The privacy documents would have to change before the code merges**, not after: the record
   of processing gains a category of data subject for people who are in no admitted group, a
   data category for the query text, and a statement that the record of what was published
   cannot be complete because `ChosenInlineResult` names no chat; the impact assessment gains an
   addendum for a surface reachable by the whole platform; and the legitimate-interests
   assessment's §4.1 and §4.2 need rewriting, because both currently rest on messages posted
   openly in a community group.
4. **The disclosure duty would need a discharge that works without a ledger memory of the
   person** (`docs/compliance/ai-act.md` §3), or a demonstration that an article result linking
   to a wiki page is not the assistant "interacting" with anyone in Article 50's sense — an
   argument that must be made, not assumed.
5. **The protection story would need a second half.** The per-conversation budget cannot bind
   an unplaced request, so an inline surface would need its own bound, decided and not
   inherited.

Nothing above is a decision deferred into legitimacy: the decision is no, today, on the
reasoning in the previous section. This list exists so that a future yes has to pay the price
in the open.

## The unit's contract

After this unit the repository's answer to "can a person use the assistant from a chat it was
never admitted to" is a recorded no with its reasoning, and the no is checkable instead of
assumed: the poll's `allowed_updates` is asserted to contain neither inline update type, with
the reason written into the assertion; an inline update delivered inside the platform's
documented transition window is proven to be acknowledged and skipped without a single
outbound request, a stored row or a decode of the person's query text; the adapter's existing
`getMe` read gains one leniently-decoded Optional boolean per capability flag —
`supports_inline_queries` and `supports_guest_queries` — and a pure reading over them produces
an error-level line at poll start when either is set, naming the finding and the remedy,
while the process continues polling exactly as before. The core is untouched: no new entry
point, no new vocabulary, no new kind, no new table, and the platform-vocabulary file is
unchanged because nothing in the core learned a new platform word. No privacy or compliance
document changes, because nothing new is received, stored or sent anywhere. No new dependency,
no new configuration entry, no change to any behaviour a member can observe.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary scan and the secret scan clean; no new dependency and no new
  configuration entry; the core crate's diff is empty.
- **AC2** The poll subscribes to neither inline update type: an adapter test asserts that the
  `allowed_updates` array sent on the wire contains neither `"inline_query"` nor
  `"chosen_inline_result"`, with the assertion's message naming the decision — written as a
  containment check so it survives whatever the sibling units add to the list, and placed
  beside the existing wire assertion (`group_context.rs:22-35`) without editing it.
- **AC3** An inline query delivered anyway is acknowledged and ignored: with the scripted
  server pushing an update whose only payload is an `inline_query` object carrying an id, a
  sender and a query text, the adapter acknowledges it — the next poll's offset is past it —
  and the recorded requests for that batch contain no `answerInlineQuery`, no `sendMessage`,
  no `getChatAdministrators` and no `getChat`. Pinned.
- **AC4** The same holds for a chosen-result update: an update whose only payload is a
  `chosen_inline_result` object is acknowledged with no request of any kind — pinned, so both
  update types are covered instead of one standing in for the other.
- **AC5** Nothing about such an update reaches the ledger or the identity tables: after AC3 and
  AC4 the store holds no new block, no new conversation, no new channel mapping and no new
  principal row for the querying sender — asserted explicitly, in the shape unit 07 uses for a
  stranger's press.
- **AC6** The capability reading is a pure function over the decoded identity, pinned for all
  four combinations: neither flag set yields no line, inline alone names inline, guest alone
  names guest, both name both. The text names the finding and the BotFather remedy and contains
  no token, in keeping with the client module's no-token-in-any-string rule
  (`client.rs:1-9`).
- **AC7** A token with the flag set does not stop the adapter: with the scripted `getMe`
  answering `supports_inline_queries: true`, the adapter completes its startup, polls, ingests
  a group message and answers it exactly as it does with the flag absent — pinned, because the
  decision that a misconfiguration is reported and not enforced is the one a future reader is
  most likely to reverse by accident.
- **AC8** The identity decode stays tolerant: a `getMe` answer omitting both flags, and one
  carrying them as an unexpected type, both decode without refusing the answer — pinned beside
  the existing lenient-decode tests, since the whole poll depends on that call succeeding.
- **AC9** The decision record exists and is reachable: a decision file records this unit's
  refusal with its date and its rejected alternatives, and the operator contract gains the
  sentence naming inline mode as off. Checked against this document's launch notes, not against
  a reader's judgement.
- **AC10** No privacy or compliance document is modified by this unit's diff, and the reversal
  list in this document names the record of processing, the impact assessment, the
  legitimate-interests assessment and the AI Act record explicitly — so a future unit that
  reopens the question cannot claim the documents were never mentioned.

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. The diff is small on purpose; the document is the deliverable that matters most.
- Adapter sites: `BotIdentity` (`client.rs:227-239`) gains
  `supports_inline_queries: Option<bool>` and `supports_guest_queries: Option<bool>`, both
  `#[serde(default)]`, documented as the platform's own getMe-only fields; a small pure reading
  beside it turns an identity into the optional notice text; `poll_loop`
  (`driver.rs:298-308`) logs it once after `fetch_identity` returns, before the first
  `get_updates`. `CONSUMED_UPDATE_TYPES` (`client.rs:103`) is not touched — its comment gains
  one sentence naming the two inline types as deliberately absent, so the next person to add an
  update type reads the reason in place.
- Adapter test sites: the wire assertion beside `the_poll_names_the_update_types_it_consumes`
  (`group_context.rs:22-35`); the two scripted-update pins and their store assertions in the
  adapter suite's existing offset and end-to-end files, which already exercise the acknowledgement
  contract through the scripted server (`tests/adapter/server.rs:1-11`); the identity pins
  beside the existing decode tests.
- Documentation sites: one decision file continuing the numbering after whatever is unclaimed
  when this merges — unit 07 reserves a long series from 0105 onward, so the number is taken at
  merge time and not fixed here; a short section in the group operator contract stating that
  the assistant is not an inline bot, that inline mode must stay off in BotFather, and that a
  group wishing to stop inline bots generally can only do so for all of them at once through the
  members' `can_send_other_messages` right; no change to `docs/follow-ups.md`, per the decision.
- Sibling collisions, stated and not acted on: unit 05's second criterion asserts the consumed
  update types are *exactly* a four-element list and unit 07 adds a fifth, so those two collide
  with each other after both merge. This unit's criterion is a containment check and collides
  with neither, and unit 05's wording should be relaxed in unit 05, not here.
- One thing to watch after merge: if the operator ever enables inline mode for an unrelated
  reason — a placeholder for a different product, an experiment — the startup line will appear
  and stay until the switch is reversed. That is the intended outcome, and the line's wording
  should say what to do instead of merely reporting the state.
