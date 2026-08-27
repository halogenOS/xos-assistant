# Telegram unit 08 — inline queries: the answer is no, and the refusal is made checkable

Date: 2026-08-25, revised the same day against two independent reviews. Every other unit in
this series adds a capability. This one examines a capability and declines it, then spends its
implementation making the decline a property of the code instead of an accident of it. Inline
mode lets a person type the assistant's username in the input field of **any** chat — a chat
the assistant was never added to, a private conversation between two strangers, a channel —
and receive results the person then sends under their own name. The Bot API side is small and
well documented. The problem is everything the query does not carry: an inline query names no
chat, so decision 0052's per-group admission has nothing to check; it names a person who may
never have written a word to the assistant, so authority, budgets and erasure have no
conversation to key on; and the published statements in the privacy documents describe a bot
that reads a community group, not a bot that reads what people type in chats it was never
invited to.

The unit ships five small things: the recorded decision, a pinned refusal in the poll, a
pinned scan proving the assistant never emits an inline affordance either, a pinned fail-safe
for the window where an inline or guest update could arrive anyway, and a startup notice so a
token whose inline mode was switched on elsewhere cannot stay switched on unnoticed. It also
writes down, precisely, what would have to be true before anyone reopens the question.

## What the reviews changed, stated plainly

The refusal itself is implementable exactly as first written. One acceptance criterion was
not, and two claims in the first draft's finding were wrong about this repository's own code.
Both are corrected below, and the corrections are named here because a reader who saw the
first draft should know which sentences moved.

- **The first draft's decode was impossible.** It asked for two `Option<bool>` fields with
  `#[serde(default)]` and then required that a `getMe` answer carrying a flag *as the wrong
  type* still decode. `#[serde(default)]` fires only on an absent field; a present field of the
  wrong type is a hard `serde` error, and `decode` reads the whole envelope through
  `response.json()` with no custom deserializer (`client.rs:563-578`). One wrong-typed flag
  would fail the entire `getMe` decode, and `fetch_identity` retries that failure on the poll
  backoff forever (`driver.rs:342-355`), so the adapter would never poll. The unit now
  specifies a named tolerant reader for these fields and says what an unreadable flag means.
- **The first draft misread two core mechanisms.** It said an unresolved standing would leave
  "every tool at the floor" — the core refuses the whole message instead
  (`assembly.rs:1154-1156`) — and it said both spend counts key on a conversation, when
  `opened_debts_by_principal` is explicitly global (`kind.rs:942-944`). Corrected in the
  finding. The correction makes the second point stronger and the third weaker, and both are
  stated at their real strength.

## The finding

**An inline query has no chat identifier at all, and every admission, protection and erasure
mechanism in this core keys on a conversation.** `InlineQuery` carries `id`, `from`, `query`,
`offset`, an Optional `chat_type` and an Optional `location` — and `chat_type` is a *kind*
("private", "group", "supergroup", "channel", or "sender"), never an identity. There is no
chat id, no chat title, nothing that could be turned into a `ChannelKey` (`message.rs:16-21`).

That single absence disables the project's whole safety structure at once:

1. **Admission cannot run.** `is_authorized` asks whether the operator admitted *this
   channel* (`authorization.rs:60-73`, "Absence is refusal"), and the withdraw directive it
   returns leaves *that group*. With no channel there is nothing to look up and nothing to
   leave — the fail-closed refusal that protects every other inbound path is unavailable by
   construction, not merely unimplemented.
2. **Authority cannot resolve, and the core's answer to that is refusal.** Standing is read
   from the chat's administrator list (decision 0015, `driver.rs:405-424`). No chat, no list,
   no standing — and an unresolved standing is never defaulted: `let Some(authority) =
   message.authority else { return Err(CoreError::AuthorityUnresolved) }`
   (`assembly.rs:1154-1156`), documented as "never recorded with a default"
   (`message.rs:180-185`). So an inline request would not run tools at a reduced authority; it
   would be refused as a transient error on every attempt, forever, because the condition never
   clears. A permanent transient failure is a worse shape than a missing feature.
3. **Half the spend protection cannot bind.** `opened_debts_by_principal` is global by design —
   "across every conversation — spend is global, so heavy direct-chat use and group use share
   one budget" (`kind.rs:942-944`) — so that half *would* still bind an inline user, once a
   principal row existed for them. `opened_debts_in_conversation` (`kind.rs:971-978`) counts
   "by any sender" within one conversation and has nothing to count for an unplaced request.
   The point is narrower than the first draft claimed and still holds: the per-conversation
   bound, which is what stops one surface being flooded, does not exist for inline.
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
at `core.telegram.org/bots/api-changelog` on 25 August 2026, and re-read the same day against
the raw page during this revision. The brief for this series named Bot API 10.1 (11 June 2026)
as current; the changelog's newest entry is **Bot API 10.3, dated 24 August 2026**, with 10.2
on 14 July 2026. Every sentence in quotation marks was read from those pages on that date.

- **Two inbound update types exist and both are off by default.** The `Update` table lists
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
- **Four outbound doors lead back into inline mode, and none of them needs an inline update.**
  This is the half the first draft left unpinned, and it matters because unit 07 ships inline
  keyboards. `InlineKeyboardButton` carries three Optional fields, each of which puts the
  assistant's username into somebody's input field without a single inline update being
  subscribed: `switch_inline_query` — "If set, pressing the button will prompt the user to
  select one of their chats, open that chat and insert the bot's username and the specified
  inline query in the input field"; `switch_inline_query_current_chat` — "If set, pressing the
  button will insert the bot's username and the specified inline query in the current chat's
  input field. May be empty, in which case only the bot's username will be inserted.This
  offers a quick way for the user to open your bot in inline mode"; and
  `switch_inline_query_chosen_chat` — "If set, pressing the button will prompt the user to
  select one of their chats of the specified type, open that chat and insert the bot's
  username and the specified inline query in the input field." The fourth is
  `savePreparedInlineMessage`, which "Stores a message that can be sent by a user of a Mini
  App" and takes a `user_id` and an `InlineQueryResult`. A keyboard carrying any of the three
  fields, or a call to either method, would undo this unit's refusal without touching
  `allowed_updates` at all.
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
- **Guest mode is a second door into the same room, and unlike inline it is on by default.**
  Bot API 10.0 (8 May 2026) "Introduced support for guest mode, allowing bots to receive
  certain messages and issue replies within chats they are not a member of". The changelog
  lists what it added: `supports_guest_queries` to `User`, `guest_bot_caller_user` and
  `guest_bot_caller_chat` and `guest_query_id` to `Message`, `guest_message` to `Update`, and
  "the class SentGuestMessage and the method answerGuestQuery". `Update.guest_message` reads
  "New guest message. The bot can use the field Message.guest_query_id and the method
  answerGuestQuery to send a message in response", and it is **not** in the default exclusion
  set quoted above — so it sits inside exactly the transition window this unit's fail-safe
  exists for. `User.supports_guest_queries` is "True, if the bot supports guest queries from
  chats it is not a member of. Returned only in getMe."
- **No enable or disable command for guest mode is documented anywhere.** All three pages were
  read for one. The API reference names no method, the inline page covers only `/setinline`,
  `/setinlinegeo` and `/setinlinefeedback`, and the changelog entry describes only how guest
  mode behaves. This is why the startup notice's remedy text differs between the two flags:
  for inline there is a command to name, for guest mode there is not.
- **`Message.via_bot` exists and this adapter does not read it.** "Optional. Bot through which
  the message was sent." Its consequence for the assistant is examined in its own section
  below.

### Our tree, at `fec0ebd`

Every citation below was re-checked against `fec0ebd`, which is `main` today. The first draft
pinned `1891fcd`; two adapter commits have merged since (`9fb714d` markdown rendering,
`7cce113` conversation fork) and every line number still lands correctly.

- **The poll names its update types on every request, so the two inline types never arrive
  today.** `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]`
  (`client.rs:103`), passed as `allowed_updates` in `get_updates` (`client.rs:313-320`)
  because "an absent selection would inherit whatever an earlier setting left on the token"
  (`client.rs:99-102`). An adapter test already asserts the exact list on the wire:
  `the_poll_names_the_update_types_it_consumes`
  (`crates/adapters/telegram/tests/adapter/group_context.rs:22-35`).
- **An inline or guest update that arrived anyway is already skipped — by accident, not by
  design.** The decoded `Update` has three optional payload fields beside its id, and "Unknown
  fields are ignored by the decoder, so the model stays exactly as small as the translation
  needs" (`client.rs:105-121`). An update carrying only `inline_query` decodes with all three
  absent, falls through `translate` to `Translation::Skip(Skip::NonMessage)`
  (`translate.rs:119-128`), and `process` acknowledges it and advances the offset
  (`driver.rs:367-370`). Nothing is fetched, nothing is written, nothing is answered. This is
  the behaviour the unit wants; it is currently untested and undocumented, which is the whole
  reason to pin it.
- **The window where that matters is documented.** `getUpdates` warns: "Please note that this
  parameter doesn't affect updates created before the call to getUpdates, so unwanted updates
  may be received for a short period of time." So a token that had inline mode and a wider
  `allowed_updates` set earlier can deliver inline updates into this adapter's first polls, and
  a token that never touched inline can deliver a `guest_message` in the same window.
- **`getMe` runs at two independent sites, and only one of them retries.** The poll's
  `fetch_identity` retries on the poll backoff until it answers, because "translating blind
  would record wrong facts into a durable ledger" (`driver.rs:342-355`, `client.rs:304-307`).
  Separately, when no display name is configured, the embedder calls
  `TelegramAdapter::read_display_name` (`crates/adapters/telegram/src/lib.rs:155-163`) from
  `crates/assistant/src/main.rs:394-401`, on its own `BotClient`, with "One attempt, no retry",
  mapping any failure to `AdapterError::Identity` and refusing the start. Anything that can
  make a `getMe` answer fail to decode therefore has two consequences: an endless retry in one
  place and a refused start in the other.
- **The identity decode is less tolerant than its own comment claims.** `BotIdentity`
  (`client.rs:227-239`) has a required `id: i64` and two Optional strings, and the comment on
  each says a "malformed answer degrades ... instead of refusing to decode". That is true of an
  *absent* field and false of a present one of the wrong type: `serde` rejects
  `"username": 7` with an `invalid type` error, `decode` reads the envelope with a plain
  `response.json()` (`client.rs:563-578`), and the whole answer is lost. Nothing in the tree
  ever tested it. The first draft repeated the comment as fact and built an acceptance
  criterion on it; this revision fixes the comment, the decode and the criterion together.
- **`client.rs` has no test module.** `grep -rn "cfg(test)" crates/adapters/telegram/src/`
  returns exactly three hits: `driver.rs:763`, `formatting.rs:157`, `translate.rs:497`. No test
  anywhere decodes a `BotIdentity` from JSON; `translate.rs:503` constructs one by hand. The
  first draft's "beside the existing decode tests" named a place that does not exist.
- **The scripted server cannot script a `getMe` body.** Its `getMe` arm builds a hardcoded
  result object from an `(i64, String)` tuple
  (`crates/adapters/telegram/tests/adapter/server.rs:368-385`), the state field is
  `me: Mutex<Option<(i64, String)>>` (`server.rs:67-70`), and the only setter is
  `set_me(id, username)` (`server.rs:144-147`), called from two places
  (`server.rs:109`, `addressing.rs:38`). There is no way today to script a capability flag, an
  omitted flag or a malformed one, so this unit's fixture work is named as a site, not
  assumed.
- **The scripted server selects and acknowledges updates by `update_id`.** `get_updates`
  filters on `update["update_id"]` (`server.rs:504-525`), so an update without one is never
  returned. A scripted update in this unit therefore carries an `update_id` plus its single
  payload object, and nothing else.
- **Tests can read the store directly.** `fixture.store.run` takes a closure over the
  connection (`offset.rs:148-157` renames a table through it), and `await_conversations` /
  `await_chat_messages` (`support.rs:791`, `support.rs:820`) already read the ledger. An
  assertion that no row was created has an existing means.
- **A source scan is an established shape in this repository.** The platform-vocabulary test
  reads "every source file of this crate — the library, its manifest and these tests" and
  fails on a match, reporting `file:line` for each finding
  (`crates/core/tests/vocabulary.rs:1-9, 60-87`). This unit's outbound scan copies that shape
  into the adapter crate.
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
  putting text the person never typed nor read on the ledger as their own words. And direct
  chats may be switched off entirely (decision 0069), in which case the deep link leads to a
  chat that records nothing and answers nothing.
- **The published privacy statements describe a different bot.** The record of processing
  describes the activity as "A bot in the halogenOS community groups stores the groups'
  messages" (§2) and enumerates its data subjects as group members and "People who write to
  the assistant directly" (§4, S1–S2). The legitimate-interests assessment rests its balancing
  on exactly that: "Messages people chose to post to an open community group, in front of
  every other member. This is **not private correspondence**, not observed behavior, not data
  collected behind someone's back" (§4.1), and "A person joining a project's support group
  where an announced assistant answers questions expects to be read by that assistant. Nobody
  is surprised" (§4.2). The public notice members actually read says the same in shorter words:
  "We store the text of each message in a group the assistant belongs to"
  (`docs/privacy/bot-assistant-privacy-policy.md:20-21`) and "We take nothing about you from
  anywhere else" (`:50`). Text typed into the input field of a stranger's private chat is
  private correspondence by any reading, and the person typing it has had no notice from any
  pinned rules. The record's review triggers name this case directly: "a change to what is
  collected" and "a new path that sends message content off the machine" (§11).
- **The precedent for declining half a platform feature is in this series already.** Unit 06
  ships the placing half of reactions and refuses the receiving half, on the reasoning that
  subscribing to updates that cannot arrive would add "two decode paths and two translation
  branches that never execute — dead code with a privacy notice attached to it", and pins the
  refusal as its AC12 with the reason written into the assertion. Its AC13 is the shape this
  unit borrows for an inference it cannot prove before merge.
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
  05, 07 and 09 each assert something about the same constant, and an exact-list assertion in
  this unit would collide with two of them. *Rejected:* an exact-list assertion matching the
  existing wire test (`group_context.rs:22-35`) — it would fail the moment either sibling
  merges, and this unit's claim is about absence, not about the list's contents.
- **The refusal is checked on the outbound side too, by a source scan, 2026-08-25.** Not
  subscribing to inline updates does not stop the assistant putting its own username into
  somebody's input field: `switch_inline_query`, `switch_inline_query_current_chat` and
  `switch_inline_query_chosen_chat` do exactly that from an ordinary inline keyboard, which is
  what unit 07 is adding, and `savePreparedInlineMessage` and `answerInlineQuery` are outbound
  methods needing no update at all. A test scans the adapter crate's source for those literal
  names, in the shape `crates/core/tests/vocabulary.rs:60-87` already uses, and fails with
  `file:line` on any hit. Without it this unit's contract would be true of the inbound half
  only, and a future keyboard field could reverse the decision without failing a single check.
  *Rejected:* relying on the `allowed_updates` assertion alone — it proves nothing about a
  button. *Rejected:* forbidding the inbound names `inline_query` and `chosen_inline_result`
  in the same scan — the wire assertion and the constant's own comment must name them, so a
  scan covering them would either fail on this unit's own diff or need exceptions that blunt
  it. The two halves are checked by the two mechanisms that fit them.
- **The arriving-anyway case is pinned as a skip for all three update types, not given a decode
  path, 2026-08-25.** The documented `allowed_updates` transition window means an inline update
  can reach the poll on a token that had inline mode on before, and `guest_message` is in the
  platform's default set, so it can reach the poll on any token in the same window. Today each
  of the three decodes with every known field absent and is skipped anonymously as
  `Skip::NonMessage` — correct behaviour that nothing proves. This unit pins all three: a
  scripted update carrying an `update_id` and one such payload object is acknowledged, advances
  the offset, and causes no request of any kind. Pinning `guest_message` here is not deciding
  what guest mode should do; it is the same fail-safe for the same window, and leaving it out
  would have made the unit's own transition-window reasoning asymmetric. *Rejected:* adding
  `inline_query` and friends to the decoded `Update` with named `Skip` reasons, the way
  `edited_message` earns one (`client.rs:117-118`) — that field exists because edits arrive
  constantly on a subscribed type, whereas these would decode a query text the assistant has
  decided not to receive, on a path that runs only in a misconfiguration. Unit 06's reasoning
  against dead paths applies unchanged. *Rejected:* pinning only the two inline types — the
  guest hazard is the larger one, since it lets a bot post *into* a chat it never joined,
  whereas inline only answers a typist.
- **The capability flags decode through a named tolerant reader, and an unreadable flag reads
  as set, 2026-08-25.** This replaces the first draft's `Option<bool>` with `#[serde(default)]`,
  which could not do what was asked of it. The two new fields are read by one small function,
  `deserialize_with` plus `default`, decoding through an owned `serde_json::Value`: absent or
  `null` gives `None`, a JSON boolean gives `Some(b)`, and any other JSON value gives
  `Some(true)`. The direction is deliberate. The notice is advisory and stops nothing, so a
  false alarm costs one log line the operator can check in BotFather in ten seconds, while a
  missed alarm leaves a token answering strangers with nobody told. *Rejected:* the plain
  `Option<bool>` — a `getMe` answering `"supports_inline_queries": "true"` would fail the whole
  envelope decode, hanging `fetch_identity` on an endless retry and refusing the embedder's
  start outright at the second call site. *Rejected:* mapping an unreadable value to `None` —
  it decodes safely but silences the one signal the field exists to raise, which is the wrong
  direction for a check about a setting nobody can see from inside this repository.
- **The two existing Optional identity fields gain the same tolerance, so their own comment
  becomes true, 2026-08-25.** `username` and `first_name` each promise that "a malformed answer
  degrades ... instead of refusing to decode" (`client.rs:233-239`) and neither delivers it for
  a wrong-typed value. This unit cites that comment as its licence to add a lenient field, so
  making the citation true belongs here, not nowhere: a second small reader gives
  `Some(s)` for a JSON string and `None` for anything else, which is exactly the degradation the
  comment describes — mention-blindness for `username`, a refused start for `first_name` at the
  embedder's own check. The diff stays inside `client.rs` and its new test module. *Rejected:*
  leaving them and noting the defect for someone else — the unit is editing that struct anyway,
  the fix is two attributes, and the hazard is an adapter that never polls. *Rejected:* making
  `id` tolerant as well — a `getMe` answer with no usable id is unusable, and refusing it is
  correct; the retry then means the poll waits for a working answer instead of translating
  against a wrong identity.
- **A token with either capability set is reported at startup, loudly, and the process still
  runs, 2026-08-25.** A pure function over the decoded identity produces the notice text, so it
  is pinned without capturing logs, and `poll_loop` writes it at error level once after
  `fetch_identity` returns and before the first `get_updates`. For inline the text names the
  documented remedy, `/setinline` in BotFather. For guest mode it cannot: the platform
  documents no command that turns guest mode on or off, so the text says that plainly and
  directs the operator to the token's BotFather settings instead of inventing a command name.
  This is not behaviour in the adapter: it decides nothing, changes no outcome for any message,
  and addresses whoever runs the process, never a member — the rule that keeps user-facing text
  in the core is about text addressed to people, which is why `driver.rs` already carries
  operator-facing strings such as "the poll failed; backing off" (`driver.rs:312`). *Rejected:*
  refusing to start — that is an adapter deciding to take the community's assistant offline over
  a setting that, with the update types unsubscribed and the outbound scan clean, leaks nothing,
  and an outage costs the community more than a broken affordance does. *Rejected:* no check at
  all — the switch lives in BotFather, outside every file this repository controls, so without
  this line nobody would ever learn it had been flipped. *Rejected:* re-checking periodically —
  one read per process start matches the lifetime of every other identity fact the poll holds.
- **Guest mode is read and designed nowhere here, 2026-08-25.** `supports_guest_queries` names
  the same hazard through a different mechanism, replies inside chats the assistant is not a
  member of, and reading it is the same one line in the same decode. Reading it is in scope, and
  so is the fail-safe skip, because both are about updates the assistant refuses. Deciding what
  guest mode *should* do is not, because a guest message, unlike an inline query, does carry a
  chat (`Message.guest_query_id` says the message "belongs to the chat where the guest bot was
  summoned") and therefore deserves its own examination instead of an answer inherited from
  this one. *Rejected:* reading only the inline flag, which would let the neighbouring hole open
  in silence. *Rejected:* specifying guest mode here to finish the subject — it is a different
  mechanism with a different admission story, and folding it in would hide a real decision
  inside a unit about something else.
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
  inline update, answers none, emits no inline affordance and stores nothing new, so no new
  category of data, no new recipient and no new storage exists; none of the record's review
  triggers fires. Writing an amendment saying "we considered a thing and did not do it" would
  put a non-event into a register of processing activities. What the unit does instead is the
  section below, which names every document and every clause a reversal would have to change
  first. *Rejected:* a privacy-document note for completeness — a record of processing describes
  processing, and padding it with refusals makes the real entries harder to audit.
- **Nothing streams, and nothing here ever will, 2026-08-25.** This unit moves no bytes: it
  adds two booleans to an existing decode, two small deserializers, one log line and three
  tests. Recorded because the constraint binds every spec. If the question is ever reopened,
  note that inline results carry URLs and not uploads — "All URLs passed in inline query results
  will be available to end users and therefore must be assumed to be public" — so a future
  inline answer would reference public files by address and would still upload nothing.

## What this unit examined and deliberately leaves alone

**`Message.via_bot` is unread, and that is recorded, not fixed here.** A review raised
it, correctly on the facts: `Incoming` (`client.rs:123-144`) decodes no `via_bot`, so when a
member of an admitted group uses somebody else's inline bot, the resulting message reaches the
ledger as ordinary text from that member under decision 0017. The review's further claim — that
this is the same defect the unit names against `/start <parameter>` — does not hold, and the
distinction is the reason nothing changes here. A message sent through an inline bot is chosen
by the person, appears in the group under their own name, and is read by every other member as
their message; attributing it to them is right. A deep link's `/start <parameter>` is inserted
by a button, never typed and usually never read by the person, and attributing it to them is
wrong. The two look alike and are not.

What remains true is that `via_bot` carries a fact the ledger does not record, and whether the
assistant should record it is a question about message provenance, not about inline mode. It
belongs to whichever unit next opens the message decode, and it is named here so the next
person finds it already examined instead of re-deriving it. This unit changes no translation.

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
3. **Four documents would have to change before the code merges**, not after. The record of
   processing gains a category of data subject for people who are in no admitted group, a data
   category for the query text, and a statement that the record of what was published cannot be
   complete because `ChosenInlineResult` names no chat. The impact assessment gains an addendum
   for a surface reachable by the whole platform. The legitimate-interests assessment's §4.1 and
   §4.2 need rewriting, because both currently rest on messages posted openly in a community
   group. And the public notice, the one document a member actually reads, is falsified in two
   places: "We store the text of each message in a group the assistant belongs to"
   (`docs/privacy/bot-assistant-privacy-policy.md:20-21`) would no longer be the whole truth,
   and "We take nothing about you from anywhere else" (`:50`) would be false outright. The
   public notice is named first in any reopening, because it is the promise made to people,
   not to an auditor.
4. **The disclosure duty would need a discharge that works without a ledger memory of the
   person** (`docs/compliance/ai-act.md` §3), or a demonstration that an article result linking
   to a wiki page is not the assistant "interacting" with anyone in Article 50's sense — an
   argument that must be made, not assumed.
5. **The protection story would need a second half.** The global per-principal count would bind
   an inline user once a principal row existed, but the per-conversation count
   (`kind.rs:971-978`) cannot bind an unplaced request at all, so an inline surface would need
   its own bound, decided and not inherited.

Nothing above is a decision deferred into legitimacy: the decision is no, today, on the
reasoning in the previous section. This list exists so that a future yes has to pay the price
in the open.

## The unit's contract

After this unit the repository's answer to "can a person use the assistant from a chat it was
never admitted to" is a recorded no with its reasoning, and the no is checkable on both sides
instead of assumed. Inbound: the poll's `allowed_updates` is asserted to contain neither inline
update type, with the reason written into the assertion, and an update delivered inside the
platform's documented transition window — `inline_query`, `chosen_inline_result` or
`guest_message` — is proven to be acknowledged and skipped without a single outbound request, a
stored row or a decode of the person's query text. Outbound: a source scan over the adapter
crate proves the assistant emits no `switch_inline_query`, no
`switch_inline_query_current_chat`, no `switch_inline_query_chosen_chat`, no
`answerInlineQuery` and no `savePreparedInlineMessage`, so a later keyboard cannot reverse the
decision quietly. The adapter's `getMe` decode gains one tolerantly-read Optional boolean per
capability flag, `supports_inline_queries` and `supports_guest_queries`, read through a named
deserializer that treats an unreadable value as set; the same tolerance is extended to the two
Optional strings already there, so a wrong-typed `getMe` answer can no longer hang the poll's
retry or refuse the embedder's start; and a pure reading over the decoded identity produces an
error-level line at poll start when either flag is set, naming the finding and, for inline, the
BotFather command that reverses it, while the process continues polling exactly as before. The
core is untouched: no new entry point, no new vocabulary, no new kind, no new table, and
`docs/platform-vocabulary.txt` is unchanged because nothing in the core learned a new platform
word. No privacy or compliance document changes, because nothing new is received, stored or
sent anywhere. No new dependency, no new configuration entry, no change to any behaviour a
member can observe.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes — `AnsweringMode::Helpful` and
  `AnsweringMode::Addressed` (`assembly.rs:180-188`); clippy, fmt and doc under denied
  warnings; the platform-vocabulary scan and the secret scan clean; no new dependency and no
  new configuration entry; the diff touches no file under `crates/core/`.
- **AC2** The poll subscribes to neither inline update type: an adapter test asserts that the
  `allowed_updates` array sent on the wire contains neither `"inline_query"` nor
  `"chosen_inline_result"`, with the assertion's message naming the decision — written as a
  containment check so it survives whatever the sibling units add to the list, and placed
  beside the existing wire assertion (`group_context.rs:22-35`) without editing it.
- **AC3** The assistant emits no inline affordance: a test scans every source file of the
  adapter crate, in the shape `crates/core/tests/vocabulary.rs:60-87` uses, and fails with
  `file:line` on any occurrence of the literals `switch_inline_query`,
  `switch_inline_query_current_chat`, `switch_inline_query_chosen_chat`, `answerInlineQuery`,
  `answerGuestQuery`, `savePreparedInlineMessage` or `InlineQueryResultsButton`. The scan
  carries a comment naming this unit's decision and stating why the inbound names are absent
  from its list. A negative check accompanies it: the scan proves it can fail, by matching a
  string built at runtime that no source file contains verbatim.
- **AC4** An inline query delivered anyway is acknowledged and ignored: with the scripted
  server pushing an update carrying an `update_id` and an `inline_query` object — an id, a
  sender and a query text — and nothing else, the adapter acknowledges it, the next poll's
  offset is past it, and the recorded requests for that batch contain no `sendMessage`, no
  `getChatAdministrators` and no `getChat`. Pinned.
- **AC5** The same holds for a chosen-result update: an update carrying an `update_id` and a
  `chosen_inline_result` object is acknowledged with no request of any kind — pinned, so both
  inline update types are covered instead of one standing in for the other.
- **AC6** The same holds for a guest message: an update carrying an `update_id` and a
  `guest_message` object, whose inner message names a chat, a sender and a text, is
  acknowledged with no request of any kind — pinned, because `guest_message` is in the
  platform's default `allowed_updates` set and therefore reaches the same transition window.
- **AC7** Nothing about those three updates reaches the ledger or the identity tables: after
  AC4, AC5 and AC6 the store holds no new block, no new conversation, no new channel mapping
  and no new principal row for the sender named in them. Asserted explicitly, by counting rows
  through `fixture.store.run` in the shape `offset.rs:148-157` uses, before and after.
- **AC8** The capability reading is a pure function over a decoded `BotIdentity`, returning
  `Option<String>`, pinned for the four outcomes that matter and for the equivalence that makes
  the rest of the domain redundant: neither flag set gives `None`; inline alone gives a text
  containing the substrings `supports_inline_queries` and `/setinline`; guest alone gives a
  text containing `supports_guest_queries` and not containing `/setinline`; both give a text
  containing both field names. The test also pins that `None` and `Some(false)` produce the
  same result for each flag, so the nine decoded states reduce to the four asserted here. No
  returned text contains the token, in keeping with the client module's no-token-in-any-string
  rule (`client.rs:1-9`).
- **AC9** The two capability flags decode tolerantly, pinned in a new `#[cfg(test)] mod tests`
  in `client.rs` (the crate has none today; `BotIdentity` is `pub(crate)`, so the test lives
  in-crate): a `getMe` result omitting both flags, one carrying `null`, one carrying `false`,
  one carrying `true` and one carrying a string, a number and an object all decode into a
  `BotIdentity` without an error. The wrong-typed cases read as `Some(true)`, asserted, and the
  assertion's message states why an unreadable flag counts as set.
- **AC10** The identity decode no longer refuses an answer over a wrong-typed optional string:
  a `getMe` result whose `username` is a number and whose `first_name` is an object decodes,
  with both fields `None`, and the doc comments on those fields are true of that case. A
  wrong-typed `id`, by contrast, is still an error — pinned, so the deliberate asymmetry is
  visible.
- **AC11** A token with a flag set does not stop the adapter: with the scripted `getMe`
  answering `supports_inline_queries: true`, the adapter completes its startup, polls, ingests
  a group message and answers it exactly as it does with the flag absent — pinned, because the
  decision that a misconfiguration is reported and not enforced is the one a future reader is
  most likely to reverse by accident.
- **AC12** The scripted server can script a whole `getMe` result: a new
  `set_me_result(&self, result: serde_json::Value)` stores the object the `getMe` arm answers
  with, and the existing `set_me(id, username)` (`server.rs:144-147`) becomes a thin wrapper
  over it that builds today's body, so its two call sites (`server.rs:109`,
  `addressing.rs:38`) are unchanged. Without this AC11 cannot be written.
- **AC13** The constant's comment carries the reason in place: `CONSUMED_UPDATE_TYPES`
  (`client.rs:99-103`) is unchanged as a value, and its doc comment gains a sentence naming
  `inline_query` and `chosen_inline_result` as deliberately absent with the decision's number,
  so the next person adding an update type reads it there. Checked by reading the diff.
- **AC14** The decision is recorded and the operator is told: a file in `docs/decisions/`
  records this unit's refusal with its date and its rejected alternatives, and
  `docs/reference/group-operator-contract.md` gains a section stating that the assistant is not
  an inline bot, that inline mode must stay off in BotFather, and that a group wishing to stop
  inline bots generally can only do so for all of them at once through the members'
  `can_send_other_messages` right. Both files are named in the diff; no repository index is
  claimed, because none exists.
- **AC15** No file under `docs/privacy/` or `docs/compliance/` is modified by this unit's diff,
  and the reversal list in this document names the record of processing, the impact assessment,
  the legitimate-interests assessment, the public privacy notice and the AI Act record
  explicitly — so a future unit that reopens the question cannot claim the documents were never
  mentioned.
- **AC16** The one unproven platform inference is marked as unproven here and nothing merged
  depends on it, in the shape unit 06's AC13 uses. The inference: that `getMe` omits
  `supports_inline_queries`, or answers it false, on a token where `/setinline` was never sent.
  The API says only "True, if the bot supports inline queries", never what a non-inline bot
  returns, and the adapter suite has no live-endpoint path (`server.rs:1-11`), so the tests can
  only prove the reading agrees with the scripted answer. Nothing depends on the inference: a
  token that never reports the flag simply produces no line, which is the same outcome as
  inline mode being off. The named post-merge check is one `getMe` against the real token, read
  once by whoever deploys, with the result written into the operator contract.

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. The diff is small on purpose; the document is the deliverable that matters most.
- Adapter sites, all in `crates/adapters/telegram/src/`:
  - `BotIdentity` (`client.rs:227-239`) gains `supports_inline_queries: Option<bool>` and
    `supports_guest_queries: Option<bool>`, each with `#[serde(default, deserialize_with =
    "...")]` naming the new capability reader; `username` and `first_name` take the new string
    reader with the same pair of attributes, and their doc comments are corrected to say what
    the code now does. `id` stays required.
  - Two small free functions beside the struct: one decoding an owned `serde_json::Value` into
    `Option<bool>` — absent or null to `None`, a boolean through, anything else to `Some(true)`
    — and one into `Option<String>` — a string through, anything else to `None`. Each carries a
    doc comment stating the direction and why.
  - A pure `fn capability_notice(me: &BotIdentity) -> Option<String>` beside them, returning the
    operator text. It names `/setinline` for the inline flag and, for the guest flag, states
    that the platform documents no command to reverse it and points at BotFather's settings for
    the token.
  - `poll_loop` (`driver.rs:298-308`) writes it at error level once, immediately after
    `fetch_identity` returns and before the first `get_updates`.
  - `CONSUMED_UPDATE_TYPES` (`client.rs:99-103`): comment only, per AC13.
- Adapter test sites:
  - A new `#[cfg(test)] mod tests` at the end of `client.rs` for AC8, AC9 and AC10. The crate
    has no test module in that file today; `driver.rs:763`, `formatting.rs:157` and
    `translate.rs:497` are the three that exist, and none of them is the right home for a decode
    pin of a `client.rs` type.
  - The wire assertion for AC2 beside `the_poll_names_the_update_types_it_consumes`
    (`group_context.rs:22-35`), as a new test in the same file, without editing the existing one.
  - The three scripted-update pins (AC4–AC6) and their store assertions (AC7) in
    `tests/adapter/offset.rs`, which already exercises the acknowledgement and offset contract
    through the scripted server and already reads the store directly (`offset.rs:140-157`).
  - AC11 in `tests/adapter/end_to_end.rs`, using the new `set_me_result`.
  - AC12 in `tests/adapter/server.rs`: the state field `me` (`server.rs:67-70`) becomes an
    `Option<serde_json::Value>` holding the result object, the `getMe` arm (`server.rs:368-385`)
    answers it directly, `set_me_result` is added and `set_me` is rewritten as a wrapper.
  - AC3 as a new integration test file in `crates/adapters/telegram/tests/`, scanning the crate's
    own `src/` and `tests/` trees. Note when writing it that the scan's own list of forbidden
    literals is in a source file the scan reads, so the list is built from parts, exactly as
    `docs/platform-vocabulary.txt` keeps the core's list out of the scanning file
    (`crates/core/tests/vocabulary.rs:1-9`). Simplest fitting shape: keep the literals in a
    committed list file beside the test and read it, matching the existing precedent instead of
    inventing a second mechanism.
- Documentation sites: one decision file continuing the numbering after whatever is unclaimed
  when this merges. The highest number on `main` today is
  `0105-the-fixed-line-is-the-acknowledgments-fallback.md`, and unit 07's spec reserves a series
  above it, so the number is taken at merge time and not fixed here. A short section in
  `docs/reference/group-operator-contract.md` per AC14. No change to `docs/follow-ups.md`, per
  the decision.
- Sibling collisions, stated and not acted on. Three sibling units assert something about
  `CONSUMED_UPDATE_TYPES` and two of them will collide with each other once both merge: unit
  05's AC2 (`05-polls.md:734-736`) asserts the list is *exactly* four elements ending in
  `"poll"`, unit 07 adds a fifth (`07-buttons-and-callbacks.md:740`), and unit 09's AC2
  (`09-chat-member-events.md:390-395`) asserts it is *exactly* the three-element list it is
  today. Any two of those three merging together breaks one of them. This unit's assertion is a
  containment check and collides with none, and the exact-list wordings should be relaxed in
  their own units, not here. Unit 06's AC12 (`06-reactions.md:751-754`) is not a collider: it
  asserts absence by containment and adds no entry, which is the shape this unit copies.
- One thing to watch after merge: if the operator ever enables inline mode for an unrelated
  reason — a placeholder for a different product, an experiment — the startup line will appear
  and stay until the switch is reversed. That is the intended outcome, and the line's wording
  says what to do, not merely what the state is.
