# Telegram unit 15 — the commands menu: published from the list the core accepts

**Note, 2026-08-30: part of this design is built.** Unit 45 (the session-reset commands)
adopted the catalogue's core half and shipped it, because it would otherwise have written a
fourth hand-matched command list. What exists now: `crates/core/src/commands.rs` with
`enum Command`, its pinned `ALL` order, `invocation()`, `offered(ChannelKind, Authority)`
and `recognized()` folding ASCII case; the seven variants are the five privacy commands
plus `/wipe` and `/compact`. `privacy::family_command` is a projection of `recognized()`;
the command stamp has widened to any recognised command; `offered()` already decides the
answer as well as the audience; `/del` stayed out of the catalogue as decided below. The
decisions above are recorded as 0160 and 0161.

What is left for THIS unit, unchanged: `summary()` and all the copy, `/help` and `/start`
with their answers and their bounds, `offered_commands` on the assembly, the `LineWindow`
re-keying, the whole platform publication with its scopes, retries and menu button, the
drift check, and the documentation edits. Adding `Start` and `Help` to the enum will change
`ALL`'s order to the one decided below; the pin over it is the deliberate act that records
the change. This unit's own decision numbers are assigned when it merges, from the next
free number then.

Date: 2026-08-25, revised the same day against two independent reviews. The assistant already
has commands. Five of them are the privacy family (`privacy.rs:45-60`), the adapter reports
which one a message invoked and the core matches the report instead of the text
(`translate.rs:293-307`, `message.rs:125-147`). None of them is announced anywhere on the
platform. A person who does not already know that `/privacydelete` exists cannot find it: the
platform's command list is empty because nothing has ever called `setMyCommands`.

This unit publishes the list, and answers the question that publishing a list forces: what is
the list, and who owns it. The answer is that there is exactly one list, it lives in the core
beside the matcher, and the platform's menu is a projection of it. The unit also fixes three
things the work uncovered: a hand-typed `/Privacy` is not recognised today, the platform's own
Start button already sends a command the core answers by opening a model turn, and the
operator contract states privacy mode's effect incorrectly.

## What the menu cannot promise

The first draft of this unit claimed that "a menu entry that the core does not act on cannot
exist" and that "the core cannot accept a command nobody is told about". Both halves are
false, and the reason is structural rather than fixable. It is stated here, at the top, because
an implementer who believes the strong version will write the wrong tests.

**The published scopes are token-global; admission and suppression are per channel and per
person.** `setMyCommands` addresses classes of chat (`all_group_chats`, `all_private_chats`),
never "the groups this deployment serves". Three cases follow, and none of them can be closed
inside this unit:

1. **A group the operator never admitted.** `admit_channel` returns `IngestOutcome::Withdraw`
   (`assembly.rs:1201-1204`, decision 0052's fail-closed shape) and the driver leaves
   (`driver.rs:445`). Until the leave completes — and a failed leave is "logged and left to the
   authorization check's self-healing" (`driver.rs:610-614`) — every member of that group sees
   the full group menu for a bot that answers none of it.
2. **An opted-out person.** Their `/help` and `/start` are dropped at ingestion
   (`assembly.rs:1149-1153`), and this unit deliberately does not widen the exemption. Their
   menu still lists both, because the platform has no per-person list we can address without
   knowing every person in advance.
3. **The process is down.** The menu is durable platform state; the assistant is not.

**And the core acts on one command that is never published.** `/del` is the moderation bot's
token, matched at `mirror.rs:34` and `mirror.rs:60-72`, and this unit decides below that it
never enters the catalogue. The core therefore does act on a command nobody is told about — on
purpose.

What actually holds, and what the acceptance criteria check, is narrower and true: **no menu
entry names a command the binary has no implementation for, and no command the binary
implements for this assistant is missing from the menu of a channel class where it is
offered.** Whether a particular person in a particular channel receives an answer is decided
afterwards by admission, suppression and the direct-chat switch. The residuals above are
accepted, named in the operator contract, and not papered over.

## Grounding

### What the platform actually does

Fetched from `https://core.telegram.org/bots/api` and `/bots/features` on 2026-08-25, and
verified against the raw pages rather than a summary — a summarising fetch of the same pages
returned a wrong description for `is_ephemeral`, which is why every quotation below is taken
from the page text itself.

**The current version is Bot API 10.3, dated 24 August 2026** — one day before this spec, and
two releases past the 10.1 the unit brief names as current. Nothing in 10.2 or 10.3 changes
the four command methods; the one field 10.2 added to `BotCommand` is covered under its own
decision.

- **`setMyCommands`** takes `commands` (array of `BotCommand`, required — "At most 100
  commands can be specified"), `scope` (a `BotCommandScope` object, optional, defaulting to
  `BotCommandScopeDefault`) and `language_code` (optional, two-letter ISO 639-1; empty means
  the list applies to every user in the scope for whose language no dedicated list exists).
  Returns `True`.
- **`BotCommand`** carries `command` — "Text of the command; 1-32 characters. Can contain only
  lowercase English letters, digits and underscores" — and `description`, "Description of the
  command; 1-256 characters". The leading `/` is not part of the field: the features page
  states separately that "Commands must always start with the `/` symbol and contain up to 32
  characters", which describes the typed form, not the declared one.
- **`deleteMyCommands`** takes `scope` and `language_code` and nothing else. Its documented
  effect is exact and shapes this whole unit: "After deletion, higher level commands will be
  shown to affected users." Deleting a scope does not hide commands; it makes the next level up
  show instead.
- **`getMyCommands`** takes `scope` and `language_code` and returns an array of `BotCommand`.
  "If commands aren't set, an empty list is returned" — so from the outside, a scope set to an
  empty array and a scope never set are indistinguishable.
- **The seven scopes** are `default`, `all_private_chats`, `all_group_chats`,
  `all_chat_administrators`, `chat` (needs a `chat_id`), `chat_administrators` (needs a
  `chat_id`) and `chat_member` (needs a `chat_id` and a `user_id`).
- **Resolution is first-match, not union.** "The following algorithm is used to determine the
  list of commands for a particular user viewing the bot menu. The first list of commands which
  is set is returned." For a private chat: `chat` → `all_private_chats` → `default`. For a
  group: `chat_member` → `chat_administrators` (administrators only) → `chat` →
  `all_chat_administrators` (administrators only) → `all_group_chats` → `default`. Each level
  is preceded by its `language_code` variant. An administrator sees the administrator list
  *instead of* the member list, never both.
- **`setChatMenuButton`** "change[s] the bot's menu button in a private chat, or the default
  menu button". `chat_id` is "Integer, Optional — Unique identifier for the target private
  chat. If not specified, the bot's default menu button will be changed." There is no group
  form. `MenuButton` is one of `MenuButtonCommands`, `MenuButtonWebApp`, `MenuButtonDefault`,
  and "By default, the menu button opens the list of bot commands."
- **A tap sends the command, and the page does not say in what form.** The features page says
  only "Selecting a command from the list immediately sends it", and, of a highlighted command
  in a message, "that command is immediately sent again". Nothing on either page states whether
  a client appends `@handle` in a group. There is contrary evidence that clients sometimes do:
  the deep-linking section shows a group `startgroup` link producing "`/start@your_bot
  spaceship`". Both forms are handled identically here — `invoked_command` strips exactly our
  own handle (`translate.rs:293-307`) — so nothing in this unit rests on the answer. The one
  place it matters is privacy mode, and the sentence there is written to survive either form.
- **The platform disclaims the list.** Verbatim, Command Scopes: "Keep in mind that Bot API
  updates will not contain any information about the scope of a command sent by the user – in
  fact, they may contain commands that don't exist at all in your bot. Your backend should
  always verify that received commands are valid and that the user was authorized to use them
  regardless of scope."
- **Three commands are asked of every bot.** Global Commands lists `/start` ("begins the
  interaction with the user, like sending an introductory message"), `/help` ("returns a help
  message") and `/settings` ("if applicable"). "Users will see a Start button the first time
  they open a chat with your bot" — that button is the client's, and pressing it sends `/start`
  whether or not the bot ever declared it.
- **Privacy mode decides whether a bare command arrives in a group at all.** With privacy mode
  on, a bot receives, verbatim: "Commands explicitly meant for them (e.g., `/command@this_bot`).
  General commands (e.g. `/start`) if the bot was the last bot to send a message to the group.
  Inline messages sent via the bot. Replies to any messages implicitly or explicitly meant for
  this bot." A bare command from a person who is not replying to us, when we were not the last
  bot to speak, does not arrive. This deployment runs with privacy mode off by contract
  (`docs/reference/group-operator-contract.md:8-18`), so the menu works — but the menu is only
  as true as that contract, and that contract's current wording is wrong (see below).
- **`is_ephemeral`** was added to `BotCommand` in Bot API 10.2 (14 July 2026): "Optional. True,
  if the command sends an ephemeral message, which can be seen only by the sender of the
  message and the bot". The changelog states the effect on other bots: an ephemeral command "is
  received by the target bot but remains invisible to all members of the chat, including both
  users and other bots". Answering one requires the ephemeral send path —
  `ephemeral_message_parameters` on `sendMessage` (10.3, replacing 10.2's `receiver_user_id`)
  or `reply_parameters.ephemeral_message_id` — "within 15 seconds of the incoming eligible
  action", unless the bot is a chat administrator, which this one deliberately is not.

### What already exists in this tree

- **The invoked-command report.** `InvokedCommand` is the core's own type (`message.rs:125-147`);
  the adapter reports the leading token with its own handle suffix removed and reports nothing
  for a foreign handle (`translate.rs:285-307`, wired at `driver.rs:432`). The handle is
  compared case-insensitively; **the token is not folded** (`translate.rs:304-306`), so
  `/Privacy` reaches the core as `InvokedCommand("/Privacy")`. **Only the first token is read**
  (`translate.rs:293-300`), so anything after a space is not part of the report.
- **The matcher.** `privacy::family_command` matches the report against five exact spellings and
  returns a typed `PrivacyCommand` (`privacy.rs:95-104`). The five constants are
  `privacy.rs:45-60`.
- **The repository writes its fixed copy in the unit spec.** `privacy.rs:110` heads the fixed
  lines with "The fixed lines, exact copy per the unit spec (2026-08-23)", and `message.rs:247-250`
  states the rule: the core supplies the exact wording, because wording is behaviour. This unit
  therefore carries its copy in full, below.
- **The deterministic answer path.** A recognised command answers through
  `DeliveryItem::CommandAnswer` on the `IngestOutcome` (`message.rs:251-268`), which the driver
  sends with no reply target (`driver.rs:438-443`, `driver.rs:603-607`). No model turn is
  involved.
- **The command stamp.** A message invoking the privacy family, or triggering the deletion
  mirror, takes `LimitedBy::Command`: no debt, no answer-window count, no unlatch
  (`assembly.rs:694-700`).
- **Two different bounds already exist.** The notice takes a channel-keyed `LineWindow` over
  `ACKNOWLEDGMENT_WINDOW` plus a budget consultation (`assembly.rs:711-715`,
  `assembly.rs:1296-1302`, `window.rs:35-41`, `window.rs:63-91`); the four rights commands take a
  principal-keyed `ReplyWindow` and no budget consultation (`window.rs:102-176`). Both use
  `tokio::time::Instant` (`window.rs:33`), so a paused-clock pin works.
- **The suppression exemption is the privacy family's alone** (decision 0072,
  `assembly.rs:1149-1153`): an opted-out person's `/unblockprivacy` must work, so the family
  skips the drop. Every other message from them is dropped at ingestion.
- **The direct-chat switch refuses before the family is ever consulted.** With `DirectChats::Off`
  a direct message is disregarded at `assembly.rs:1206-1208` — including `/privacy`. The default
  is `On` (`assembly.rs:199-205`).
- **A direct message is always addressed** (`translate.rs:171-176`) and an addressed message
  summons a turn (`assembly.rs:1244-1249`). So today, in a served direct chat, pressing the
  platform's Start button opens a model turn whose entire user message is `/start`. In a group,
  a bare tapped command is unaddressed and summons nothing, while `/start@handle` mentions the
  bot and does summon a turn.
- **The mirror's token is not ours, and it compares exactly.** `/del` is the moderation bot's
  command; the assistant matches it only to erase its own stored copy of the named message,
  silently (`mirror.rs:34`, `mirror.rs:60-72`). Its floor is `Authority::Moderator`
  (`mirror.rs:37-41`), because decision 0015 resolves the group's owner to `Admin` and its
  administrators to `Moderator` (`authority.rs:62-63`), and the moderation bot obeys that whole
  set.
- **`Authority` has three variants** and `ChannelKind` two (`message.rs:87-123`,
  `message.rs:29-45`).
- **The disclosure value is already resolved on the assembly** (`assembly.rs:291`,
  `assembly.rs:416`) and already knows how to prefix a text: `Disclosure::disclosed`
  (`disclosure.rs:94-96`). Its module states the fold this unit relies on: an answer whose
  provenance cannot be read "is introduced as if everyone were new, because a repeated line is
  harmless and a skipped first one is the violation" (`disclosure.rs:39-42`). The introduction
  receipt is read from stored *answer blocks* (`disclosure.rs:206-233`), so a deterministic
  command answer never records an introduction.
- **The wire client posts `application/json`** (`client.rs:537-539`) and already sends nested
  objects in the body — `reply_parameters` at `client.rs:451-454` — so a scope object needs no
  string-encoding special case, even though the parameter tables describe "JSON-serialized"
  values for form encoding.
- **The client's `request` handles 429 per call** with a bounded wait ceiling
  (`client.rs:505-527`, `client.rs:45`), and the driver carries an injectable sleep
  (`driver.rs:259`, `driver.rs:302`) that the suite can pause.
- **The adapter suite runs against a loopback fake** that records every request before
  answering, spawns a task per connection so one slow method cannot block another
  (`server.rs:123-129`), and already answers `{"ok":true,"result":true}` to any method it has no
  script for (`server.rs:450`). The three new methods therefore need no new happy-path handler —
  only new failure and hang scripts.

### Three findings that shape the unit

**1. A scope cannot be emptied, only replaced or dropped one level.** There is no "show nothing
here". `deleteMyCommands` explicitly falls through to the level above, and an empty array is
indistinguishable from unset in `getMyCommands`' answer, so nothing in the documentation says an
empty array stops the fall-through. The only way to make a chat class show nothing is to leave
every level of its chain unset — and of the private chain's three levels we can address two:
`chat` is keyed by a chat id and is not published by this unit at all. A list left at the `chat`
level by a previous deployment or another holder of the token would shadow ours and cannot be
detected without enumerating chats. That is a design constraint, not an implementation detail:
it decides which list the `default` scope has to carry, and it caps what the unit may promise.

**2. `is_ephemeral` can silently falsify a published privacy statement — and not through our own
commands.** The published policy states that when an administrator removes a message with the
moderation bot's reply command, that message is removed from our store as well
(`docs/privacy/bot-assistant-privacy-policy.md:117`). That promise rests on the assistant seeing
the administrator's bare `/del` (`mirror.rs:60-72`). If the moderation bot ever declares `/del`
ephemeral, the platform makes that message invisible to other bots, the mirror stops firing, no
error is raised anywhere, and the published sentence becomes false. Nothing in this unit can
prevent it; the unit records it, and the operator contract gains the one sentence that lets an
operator recognise the symptom.

**3. The operator contract states privacy mode's effect incorrectly today.**
`docs/reference/group-operator-contract.md:11-13` says that with privacy mode on the assistant
"receives only messages that mention it, reply to it, or are commands". The platform's own list
is narrower: a bare command arrives only when it is addressed as `/command@this_bot` or when the
assistant was the last bot to speak. The current wording would let an operator believe the
commands work with privacy mode left on. This unit is the one that publishes a menu of commands,
so it is the one that has to correct the sentence.

## The copy

Every user-facing string this unit ships is written here, in full. The `summary()` values are
what a person reads in the `/` list of every group the assistant serves, so they are short,
plain, and written in the person's own terms rather than the system's.

| Command | `summary()` |
| --- | --- |
| `/start` | `What I am and what I can do` |
| `/help` | `The commands I answer here` |
| `/privacy` | `Where to read the privacy policy` |
| `/privacyout` | `Stop collecting and answering my messages` |
| `/privacydelete` | `Delete my stored data, after I confirm` |
| `/confirmdelete` | `Confirm the deletion I just asked for` |
| `/unblockprivacy` | `Start collecting my messages again` |

`/help`'s answer is the lead line, then one line per offered command, in `Command::ALL` order,
each written as the invocation, a space-surrounded en dash and the summary:

```
Commands I answer here:
/help — The commands I answer here
/privacy — Where to read the privacy policy
...
```

The lead line, as a constant:

> `Commands I answer here:`

`/start`'s answer is the resolved disclosure line, a blank line, then this fixed text — which
names no product, because the assistant's name and community are configuration
(`assembly.rs:285-291`):

> `Ask me a question and I answer from what the community has written down. When I cannot find
> an answer, I say so instead of inventing one.`
>
> `Send /help for the commands I answer, or /privacy to read how your data is handled.`

The second paragraph names `/help` and `/privacy` literally rather than deriving them, because
they are prose, not a list; the drift check below covers exactly this risk.

## Decisions taken with this unit

- **One catalogue in the core, and the matcher reads it, 2026-08-25.** A new
  `crates/core/src/commands.rs` holds `enum Command` with `ALL`, and three total matches on it:
  `invocation()` (the token, leading `/` included, as the report carries it), `summary()` (the
  line above) and `offered(ChannelKind, Authority) -> bool` (whether this class of person is
  offered it in this class of channel). `recognized(Option<&InvokedCommand>) -> Option<Command>`
  is the one recognition function, and `privacy::family_command` becomes a projection of it,
  keeping `PrivacyCommand` and its opposite-rules split exactly as they are. Adding a command
  means adding a variant; the compiler then demands its spelling, its description and its
  audience. *Rejected:* a list in the adapter, which is the drift this unit exists to remove and
  would put wording — which is behaviour — in a crate that holds none. *Rejected:* a list in the
  deployment configuration, which would let an operator advertise a command the binary does not
  implement.
- **`Command::ALL` order is the published order and the `/help` order, 2026-08-25.** The order
  is `Start`, `Help`, `Privacy`, `PrivacyOut`, `PrivacyDelete`, `ConfirmDelete`, `PrivacyIn`:
  what the assistant is, what it can do, where the policy is, then the rights in the order a
  person exercises them. It is pinned by a test so a reordering is a deliberate act.
  *Rejected:* alphabetical, which would separate `/privacydelete` from `/confirmdelete` and put
  the confirm before the ask.
- **The audience is a channel kind and an authority floor, not a scope, 2026-08-25.** The core
  gains no scope vocabulary. `Assistant::offered_commands(ChannelKind, Authority) ->
  Vec<OfferedCommand>` answers "what is offered to a person of at least this standing in this
  class of channel", using vocabulary the core already has (`message.rs:29-45`,
  `message.rs:87-123`). The authority is a floor, not a point: the caller passes the *lowest*
  standing in the audience it is asking about, and a core test asserts the answer is monotone —
  a higher standing is offered a superset. *Rejected:* a `CommandScope` enum in the core
  mirroring the platform's seven scopes — platform shapes in the core, six of which have no
  neutral meaning, and the invariant would be broken for nothing. *Rejected:* an exact-authority
  parameter, which has no honest answer for a scope covering two standings (below).
- **The adapter asks for the group administrator list at the `Moderator` floor, 2026-08-25.**
  The platform offers one administrator scope for a group, `all_chat_administrators`, while
  decision 0015 splits the platform's administrator set into two core standings — the group's
  creator resolves to `Admin`, its administrators to `Moderator` (`authority.rs:62-63`). The
  adapter therefore asks `offered_commands(Group, Moderator)`, the lower edge of that set, and
  the same edge `mirror::ADMINISTRATOR_FLOOR` already names (`mirror.rs:37-41`). This is a
  translation, which is the adapter's job, and it is the honest direction: an administrator's
  menu never lists something a moderator cannot use. The residual, stated because there is no
  way to remove it: a command whose floor is `Admin` alone would be published to nobody's menu
  and would need its own decision. No such command exists today; the monotonicity test and this
  paragraph are what an implementer meets when the first one appears. *Rejected:* asking at
  `Admin` and publishing that list to the whole administrator set, which would advertise to
  moderators a command they are refused.
- **`offered()` decides the answer, not only the menu, 2026-08-25.** The delivery match
  (`assembly.rs:760-768`) consults `offered_commands` for the message's channel kind and the
  resolved authority, and answers only a recognised command that is offered there. Recognition
  stays global: a recognised command always takes the command stamp, so it never opens a turn
  and never takes debt, whether or not it is answered. This is what the platform itself
  instructs — "always verify that received commands are valid and that the user was authorized
  to use them regardless of scope" — and it makes `offered()` one fact with two readers instead
  of a decorative annotation on the menu. *Rejected:* keying the answer on recognition alone and
  treating the floor as documentation, which is how a floor silently stops meaning anything.
- **`/start` is offered in direct channels only; everything else in both, 2026-08-25.** The
  Start button exists only in a private chat, `/start` is the platform's private-chat onboarding
  command, and every other bot in a group has one too. Offering it in a group would put a
  duplicate introduction in the menu of a chat where it answers nothing useful. Because
  `offered()` now decides the answer, `/start@handle` typed in a group is recognised, takes the
  command stamp, opens no turn and answers nothing — which is a strict improvement on today,
  where it summons a model turn on the text `/start`. The private list is therefore exactly the
  group list plus `/start`. *Rejected:* offering `/start` everywhere, which would make the group
  menu's first entry a command nobody in a group needs. *Rejected:* not recognising `/start` in
  a group at all, which would leave `/start@handle` summoning a model turn.
- **All four chat-class scopes are owned and republished at every startup, 2026-08-25.**
  `default`, `all_private_chats`, `all_group_chats` and `all_chat_administrators` are set every
  time the process starts, unconditionally, with no `language_code`. Because resolution stops at
  the first list that is set, owning every level of both chains **that a token-global call can
  address** is the most that can be done to put a person on our list. It is not a guarantee:
  three of the group chain's six levels and one of the private chain's three are chat- or
  user-keyed and stay exactly as exposed as before this unit. *Rejected:* publishing only
  `default` — two of the five group levels above it are ours to own, and leaving them unset
  would let a list set by another holder of the same token shadow ours silently. *Rejected:*
  publishing once and remembering — the platform is the authority on what is currently
  published, a second process with the same token can overwrite it, and five calls at startup
  are cheaper than a memory that can be wrong.
- **The chat-keyed scopes are not published, 2026-08-25.** A `chat`-scope list, keyed by the
  groups the core already knows from its authorization rows, is the only mechanism inside the
  platform's fence that could align the menu with admission — and it is refused. It needs a set
  at admission and a delete at withdrawal, so the publish stops being a startup projection and
  becomes durable per-chat state with its own lifetime; it grows one call per group; it needs a
  new core query whose only purpose is a platform call; and it does not even close the case it
  is for, because the menu is visible from the moment the assistant is added, before any
  admission decision has run. The residual is the one named at the top of this spec, and the
  mitigation is the one that already exists: the assistant leaves an unadmitted group.
  *Rejected:* per-chat publishing, for the reasons above. *Rejected:* leaving the residual
  unmentioned, which is what the first draft did.
- **An empty list is cleared with `deleteMyCommands`, never set as an empty array, 2026-08-25.**
  The documented way to unset a scope is the delete method; whether an empty array counts as
  "set" for the first-match algorithm is not documented anywhere, and no shipped behaviour will
  rest on an inference. *Rejected:* `setMyCommands` with `commands: []`.
- **The `default` scope carries the private list, 2026-08-25.** `default` is the last level of
  the private chain and both group levels above it are owned, so making it the private list is
  what keeps both chains honest. *Rejected:* letting `default` carry the group list — a private
  chat under `DirectChats::Off` would then be offered group commands that answer nothing, which
  is the precise falsehood this unit exists to prevent.
- **The publish order is narrower first, `default` last, then the button, 2026-08-25.**
  `all_group_chats`, `all_chat_administrators`, `all_private_chats`, `default`,
  `setChatMenuButton`. `default` goes last because it is the level both chains fall back to: the
  narrower levels are correct before the fallback changes underneath them. Two partial-failure
  residuals follow, and both are computed rather than left implicit. *Set side:* if the group
  publish fails while `default` succeeded, a group shows the private list; the private list is
  the group list plus `/start`, so every command shown still works and `/start` answers nothing
  there. *Delete side, under `DirectChats::Off`:* if `all_private_chats` is cleared and
  `default`'s delete fails, a private chat falls back to whatever `default` still holds — a list
  from an earlier `DirectChats::On` run, or one set through the platform's bot management
  interface. That is the dangerous one, because every message in that chat is disregarded at
  `assembly.rs:1206-1208`. It is why the publish retries, below, and why the operator contract
  names the symptom. *Rejected:* publishing `default` first, which trades the same window for a
  stale `all_private_chats` instead and fixes nothing.
- **A failed publish is retried a bounded number of times, then given up until the next start,
  2026-08-25.** The whole sequence is attempted up to three times; each attempt repeats only the
  calls that failed, with a delay between attempts taken from the driver's injectable sleep
  (`driver.rs:259`) so the suite can pause it. Three attempts cover the realistic failure — a
  transport blip while the network settles at boot — and turn "wrong until the next restart" on
  an always-on deployment into a short window. After the third, the failure is logged and the
  publish stops. *Rejected:* no retry at all, which the delete-side residual above makes
  unacceptable. *Rejected:* a periodic background republish, which is a second lifetime to
  reason about, would fight nothing useful (nothing else changes the list) and would keep a
  timer alive for a cosmetic projection.
- **`getMyCommands` is not called, 2026-08-25.** It answers per scope and per language, and
  neither the language-keyed lists nor the chat-keyed lists that would actually shadow ours can
  be enumerated, so a read-back can never prove what a given person sees. An unconditional
  publish costs the same calls and needs no comparison. *Rejected:* publish-only-if-changed,
  which doubles the call count to prove something weaker than it appears to prove. *Rejected:*
  a read-back to detect a shadowing `chat`-scope list, which cannot be done without a chat id
  the publish does not have.
- **The default menu button is set once to `MenuButtonCommands`, 2026-08-25.** The platform's own
  default already opens the command list, but the button is per-token durable state: a
  `MenuButtonWebApp` set once through the platform's bot management interface would keep hiding
  the list, and this assistant has no Web App. One `setChatMenuButton` with no `chat_id` at
  startup makes the button the code's decision instead of a leftover. It is set under
  `DirectChats::Off` too, where it opens an empty list: the switch is temporary deployment state,
  the button is not, and an empty list in a chat where nothing is answered is exactly true.
  *Rejected:* not calling it — a leftover button hides the whole menu and nothing in the
  repository would explain why. *Rejected:* per-chat buttons — `chat_id` accepts a private chat
  only, so there is nothing per-group to set, and a per-person call would buy nothing over the
  default.
- **`/help` and `/start` join the core as fixed-answer commands, 2026-08-25.** The platform asks
  every bot to support both, the Start button exists in every private chat whether we declare the
  command or not, and today pressing it opens a model turn against the text `/start`
  (`translate.rs:171-176`, `assembly.rs:1244-1249`). `/help`'s answer is composed from the same
  catalogue that feeds the menu — the offered commands for that channel kind and that authority,
  each with its `summary()` — so the help text and the menu can never disagree. *Rejected:*
  leaving both unhandled, which keeps a model turn on the platform's own onboarding tap and
  leaves the menu's `/help` entry either absent or false. *Rejected:* a hand-written help text, a
  second copy of the list. *Rejected:* `/settings`, the third command the platform names: this
  assistant has no per-person settings, and an entry that answers "there is nothing to set" is
  worse than no entry.
- **`/start`'s answer opens with the resolved disclosure line; `/help`'s does not, 2026-08-25.**
  `/start` is by definition a person's first interaction, and a deterministic command answer is
  not an answer block, so the ledger-based introduction (unit 12) never covers it. The line is
  already resolved on the assembly and already has a prefixing method (`disclosure.rs:94-96`);
  because the introduction receipt is read from stored answer blocks (`disclosure.rs:206-233`),
  prefixing here records nothing, and the person's first model answer still carries the line —
  which the disclosure module already names as the harmless direction (`disclosure.rs:39-42`).
  `/help` is left plain because it is a list of commands, not an introduction, and repeating the
  line on every tap in a busy group is noise. This means a person whose very first contact is
  `/help` reads a spoken answer with no disclosure line — a shape that already exists, since
  `/privacy` has answered without the line since 2026-08-23 (`assembly.rs:1296-1302`). The AI Act
  record's discharge is the answer block's per-person resolution, so this changes no published
  claim; the record gains a dated sentence saying so explicitly, and the general question of the
  deterministic answers goes to follow-ups. *Rejected:* recording the `/start` answer as an
  answer block so the introduction receipt sees it — that would put a machine-written block into
  the model's own history for a message the model never took a turn on. *Rejected:* prefixing
  every deterministic answer, which would change `/privacy`'s pinned behaviour inside a unit that
  is not about it.
- **The suppression exemption does not widen, 2026-08-25.** Only the privacy family stays exempt
  (decision 0072). An opted-out person's `/help` or `/start` is dropped at ingestion like every
  other message from them: they asked not to be processed, and a help text is not a right that
  has to reach them. Their menu still lists both, which is the second residual named at the top.
  *Rejected:* exempting every recognised command, which would quietly turn the opt-out into
  "except when you type a slash".
- **The command stamp widens to any recognised command, 2026-08-25.** `assembly.rs:694-700`
  becomes "a recognised command, or the mirror" instead of "the privacy family, or the mirror",
  so `/help` and `/start` take no debt, no answer-window count and no unlatch — including when
  the command is recognised but not offered there. This is the widening the existing shape
  already implies; no new branch appears.
- **`/help` and `/start` are bounded like the notice, 2026-08-25**: a budget consultation plus a
  channel-keyed window, not the rights commands' per-person window. They are notices, not
  rights: nobody's data protection depends on them arriving, and they are lines anyone in the
  chat can trigger, which is the flood shape the notice window was built for (`window.rs:35-41`).
  To give each fixed line its own window without a field per command, `LineWindow` keys on the
  conversation **and the command** instead of the conversation alone. *Rejected:* a second and
  third `LineWindow` field on the assembly — a field per command is the bolted-on shape the
  repository's standards refuse. *Rejected:* one shared window across all three — a person who
  taps Start and then sends `/help` would meet silence on the first question they ever ask.
- **Recognition folds ASCII case on the token, 2026-08-25.** The platform allows only lowercase
  letters, digits and underscores in a declared command, so no two declared commands can collide
  under folding. Today `/Privacy` typed by hand is unrecognised and falls through to an ordinary
  message, which means a person exercising a data right with an autocapitalising keyboard gets
  nothing. The fold lives in `commands::recognized`, in the core, because which spellings the
  core accepts is the core's decision. *Rejected:* folding the token in `translate.rs`'s report
  — the report is meant to carry what the platform delivered, minus the handle, and rewriting it
  would hide the typed form from every later reader.
- **The deletion mirror keeps its exact comparison, 2026-08-25.** `mirror::mirrored_target`
  compares `/del` byte for byte (`mirror.rs:60-72`) and this unit does not fold it, so after this
  unit `/Privacy` works and `/Del` still mirrors nothing. The asymmetry is deliberate and it runs
  the safe way. Folding ours widens what we answer, and a wrong answer is a message; folding
  theirs widens what we *erase*, on an assumption about another bot's parser we have never
  checked, and a wrong erasure is irreversible. Missing a mirror leaves a stored copy of a
  message the moderators removed, which is recoverable; erasing on a mistaken match is not.
  *Rejected:* folding the mirror's comparison for symmetry. The follow-up records what would
  resolve it: an observation of the moderation bot actually accepting `/Del`.
- **An argument after the command is ignored by recognition and recorded verbatim, 2026-08-25.**
  The adapter reports the first token only (`translate.rs:293-300`), so `/start airplane`,
  `/start@handle spaceship` — the platform's own deep-link forms — `/help foo` and
  `/privacydelete now` all recognise as their bare command. The payload stays in the message text
  exactly as sent, recorded like any other text, and nothing reads it. *Rejected:* parsing the
  deep-link payload — no feature consumes it, and giving meaning to a link-supplied string would
  let whoever wrote the link choose input the assistant acts on. *Rejected:* refusing a command
  that carries an argument, which would break the platform's standard `/start` deep link and the
  Manage Bot flow.
- **An unrecognised command stays an ordinary message, 2026-08-25.** Nothing answers "I do not
  know that command". Privacy mode is off by contract, so every other bot's commands arrive here
  too — the deletion mirror depends on exactly that — and a bot that answers unknown commands
  would interrupt every other bot in the group. The platform states outright that updates may
  carry commands that do not exist in the bot. *Rejected:* a fixed unknown-command line.
  *Rejected:* suppressing unrecognised commands from the model's context, which would blind the
  model to half of what the group is doing.
- **Nothing about the publish is written to the ledger, 2026-08-25.** The published list is a
  projection of code, re-derived at every startup. *Rejected:* a note block naming what was
  published — the ledger cannot supersede a fact it cannot observe, and a second process with the
  same token would make the block quietly false.
- **No command is declared ephemeral, 2026-08-25.** `is_ephemeral` is genuinely attractive for
  `/privacydelete`, whose typed form is currently visible to a whole public group. It cannot be
  taken here: answering an ephemeral command needs the ephemeral send path, which the adapter
  does not have (`client.rs:439-461` sends one plain `sendMessage`), within 15 seconds, and the
  administrator exemption is unavailable because the operator contract keeps the assistant a
  non-administrator. Declaring the flag without the send path would answer an invisible question
  with a message the whole group sees — the opposite of the intent. It goes to
  `docs/follow-ups.md` with its preconditions. *Rejected:* declaring the flag now and sending the
  answer as an ordinary message.
- **`/del` is never published and never enters the catalogue, 2026-08-25.** It is the moderation
  bot's command; the assistant only keeps its own books when an administrator uses it. Listing it
  would advertise the assistant as the actor that deletes messages, which contradicts decision
  0070 — the assistant assesses, a human decides. It stays matched where it is today, in
  `mirror.rs`, outside `commands::recognized`. This is the deliberate half of "the core acts on a
  command nobody is told about".
- **The drift check covers the documents that state our command surface to a person,
  2026-08-25.** `docs/privacy/bot-assistant-privacy-policy.md`, `prompts/30-conduct.md` and
  `README.md` each write our command names by hand, and the prompt's copy is pinned verbatim by
  `crates/assistant/tests/docs.rs:367` — it is what the model reads when it tells someone which
  commands exist, so it is where drift bites hardest. A docs test scans those three files and
  asserts every command token it finds exists in the catalogue. The token rule is stated rather
  than assumed: a `/` at the start of a line, after whitespace, or after a backtick, followed by
  one to thirty-two characters from lowercase letters, digits and underscores, and not
  immediately followed by `@`. The `@` exclusion is what lets `/report@<handle>` (`README.md:140`)
  and `/del@…` stay out — a command written with a handle is addressed to a named bot and is not
  ours. Without the delimiter rule the scan would trip on
  `https://www.lda.bayern.de` (`bot-assistant-privacy-policy.md:172`). Verified against the tree:
  the rule yields exactly `/privacy`, `/privacyout`, `/privacydelete`, `/confirmdelete` and
  `/unblockprivacy` across the three files and nothing else. *Rejected:* covering the privacy
  policy alone, which is what the first draft proposed and which leaves the prompt — the copy
  that actually reaches a person through the model — unchecked. *Rejected:* covering
  `docs/reference/group-operator-contract.md`, which documents the moderation bot's bare `/del`
  on purpose and would need an exception the check cannot judge. *Rejected:* covering
  `docs/decisions/`, because a decision record states what was decided on its date and a later
  rename does not make it false.

## The unit's contract

There is one list of commands, `Command::ALL` in the core, and three things read it: the matcher
that recognises an invocation, the `/help` answer, and the platform menu. At startup the adapter
asks the core for the commands offered to a member in a direct channel, to a member in a group
and to a moderator in a group, translates each answer into `BotCommand` objects with the leading
marker stripped, and publishes them to `all_group_chats`, `all_chat_administrators`,
`all_private_chats` and `default` in that order; a scope whose list is empty is cleared with
`deleteMyCommands` instead, and the default menu button is set to the commands button last. A
failed call is retried within the same start, up to three attempts, then logged and abandoned
until the next start. The publish runs beside the poll as a further arm of the driver's
`select!`, so no wait inside it can precede the first poll, and it never refuses the start. No
menu entry names a command the binary has no implementation for, and no command implemented for
this assistant is missing from the menu of a channel class where it is offered; whether a
particular person in a particular channel is answered is decided afterwards by admission,
suppression and the direct-chat switch, which are per channel and per person and cannot be
expressed in a token-global list — the three residuals that follow are named in the operator
contract, not hidden. `offered()` decides the answer as well as the menu, so a command recognised
where it is not offered takes the command stamp and answers nothing: `/start@handle` in a group
no longer opens a model turn. A command the core does not recognise — another bot's, or one the
platform delivered that never existed — stays an ordinary message, recorded verbatim and answered
by nothing deterministic; so does any argument after the command token. `/help` and `/start`
answer from the catalogue with no model turn, no debt and no unlatch, bounded per channel per
command per window and consulted against the budgets; `/start`'s answer opens with the disclosure
line and `/help`'s does not. Recognition folds case, so a hand-typed `/Privacy` works; the
deletion mirror's own comparison stays exact, because widening what is erased on an unchecked
assumption is the one direction that cannot be undone. The suppression exemption, the rights
commands' per-person bound, the deletion mirror and the direct-chat switch all behave exactly as
before. Nothing streams, because nothing here carries bytes: the whole publish is five small JSON
bodies with no file, no upload and no download anywhere in the path. Nothing is written to the
ledger by the publish, and no personal data reaches a new recipient, a new store or a new
category — `setMyCommands`, `deleteMyCommands` and the default-button call carry no chat id, no
user id and no message text, and their only inputs are the catalogue and the direct-chat switch
— so `docs/privacy/records-of-processing.md`, `docs/privacy/dpia.md`, `docs/privacy/lia.md` and
the published policy are all unchanged by this unit; publishing the rights commands makes them
easier to find, which supports the policy's existing statement rather than altering it. No new
dependency.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  secret scan clean; no new dependency. The platform-vocabulary scan is made evidence for this
  unit instead of silent about it: `docs/platform-vocabulary.txt` gains `setmycommands`,
  `deletemycommands`, `setchatmenubutton`, `menubutton`, `botcommandscope` and `menu`, and the
  scan (`crates/core/tests/vocabulary.rs`) passes with them. The word "scope" cannot join the
  list — the core already uses it neutrally at `identity.rs:275` and `acknowledgment.rs:121` —
  so its absence from the core is a review matter, stated here rather than claimed as checked.
- **AC2** The catalogue's own invariants, in a core test: for every `Command::ALL` variant,
  `recognized(Some(&InvokedCommand::new(variant.invocation())))` returns that same variant, and
  so does the same call on the uppercased spelling; every `invocation()` is a leading `/`
  followed by one or more characters from lowercase ASCII letters, digits and underscores; every
  `summary()` is non-empty; `Command::ALL`'s order is pinned literally; and `offered` is monotone
  in authority — for each channel kind, the set offered at `Moderator` contains the set offered
  at `Member`, and the set at `Admin` contains the set at `Moderator`. A second test asserts
  `privacy::family_command` agrees with the catalogue on all five privacy tokens and returns
  `None` for `/help`, `/start` and `/del`. (The platform's length bounds are checked in the
  adapter, AC13, because they are the platform's numbers.)
- **AC3** The startup publish is pinned against the loopback fake: under the default
  configuration exactly four `setMyCommands` requests are recorded, in the order
  `all_group_chats`, `all_chat_administrators`, `all_private_chats`, `default`, each carrying its
  scope object with the documented `type` string; every `command` field is the invocation with
  the leading `/` removed; every `description` is the variant's `summary()`; the entries appear
  in `Command::ALL` order; no request carries a `language_code`; no list exceeds 100 entries. The
  group list and the administrator list are equal, the `default` list equals the
  `all_private_chats` list, and the private list is the group list plus exactly `start`.
- **AC4** With `DirectChats::Off`, `all_private_chats` and `default` are each cleared with
  `deleteMyCommands` and neither is set, while `all_group_chats` and `all_chat_administrators`
  are still set — pinned on the recorded requests, including that the four calls keep their
  order.
- **AC5** Exactly one `setChatMenuButton` is recorded, carrying `menu_button.type = "commands"`
  and no `chat_id`, and it is the last of the five calls. It is recorded under `DirectChats::Off`
  too.
- **AC6** A publish failure is contained and retried: with the fake scripted to refuse one
  command method, that call is re-attempted and the whole sequence is attempted at most three
  times, pinned under paused time on the recorded requests; with every command method refusing,
  the adapter still starts, still polls, still ingests and still answers, and gives up after the
  third attempt. The publish never delays the first poll, which is structural rather than
  incidental — it is a further arm of the driver's `select!`, so no wait inside it precedes the
  poll loop — and is pinned by driving an update through to its answer while a command method is
  scripted to never respond. The contract also requires one warning per abandoned call; that
  half is deliberately **not** pinned, because observing a `tracing` event from the adapter
  suite would need `tracing-subscriber` as a new dev-dependency (it is a dependency of
  `crates/assistant` alone) or a hand-written subscriber, and neither is justified for a log
  line.
- **AC7** `/help` answers from the catalogue: the answer is the lead line followed by one line
  per command offered in that channel for that sender's authority, in `Command::ALL` order, each
  as its invocation and its `summary()`, and names none that is not offered there — so a group's
  `/help` omits `/start` and a direct chat's includes it. It answers to a bare, unaddressed
  `/help` in a group, which is the form a menu tap sends and which `translate.rs:171-176` leaves
  unaddressed. It answers at most once per channel per command per `ACKNOWLEDGMENT_WINDOW`, is
  silent when a budget refuses the sender, takes no model turn and carries `LimitedBy::Command`
  on its stored message — all pinned.
- **AC8** `/start` in a direct chat answers with the resolved disclosure line, a blank line and
  the fixed text, and its stored message carries `LimitedBy::Command`. That it opens no model
  turn is pinned positively: the scripted provider records no completion request for the
  message, and no ledger answer block is written for it.
- **AC9** `/Privacy` and `/PRIVACYDELETE` are recognised and answer exactly as their lowercase
  forms do, while the stored message text stays verbatim in every case — pinned in the core and
  through the adapter. `/Del` is pinned as still mirroring nothing.
- **AC10** `offered()` decides the answer: `/start` sent in a group — bare and as `/start@handle`
  — is recognised, carries `LimitedBy::Command`, draws no deterministic answer, and opens no
  model turn even in the addressed form, pinned by the absence of a completion request.
- **AC11** Unchanged behaviour, pinned as regressions: an opted-out person's `/privacy` and
  `/unblockprivacy` still answer, their `/help` and `/start` are disregarded with nothing
  written; the four rights commands keep their per-person window and their budget exemption; an
  administrator's `/del` reply still mirrors; `/start airplane` and `/privacydelete now`
  recognise as their bare commands and their message text is stored with the argument intact; a
  `/foo` nobody declared is recorded as an ordinary message and draws no deterministic answer.
- **AC12** `LineWindow` keyed by conversation and command: `/privacy`, `/help` and `/start` each
  keep an independent window in the same channel, and each is silent for the rest of its own
  window — pinned under paused time, with the notice's existing behaviour unchanged.
- **AC13** The platform's own bounds are checked where the platform is spoken: an adapter test
  asserts that every published `command` field is 1 to 32 characters of lowercase ASCII letters,
  digits and underscores, and every `description` is 1 to 256 characters, counted as Unicode
  scalar values rather than bytes — the copy above is ASCII, and the rule is stated so a later
  edit with an en dash cannot land on a different reading of the same number.
- **AC14** Documentation ships with the code:
  - `docs/reference/group-operator-contract.md` corrects its privacy-mode paragraph (`:11-13`)
    to the platform's actual list, and gains a short section stating that the assistant
    publishes its own command list at every start (so an edit made through the platform's bot
    management interface is overwritten on the next restart), that the menu is visible in a
    group before the assistant has decided whether it serves that group, that an opted-out
    person still sees the menu, that a list left at the platform's chat-keyed level can shadow
    ours undetectably, and that a moderation bot declaring `/del` ephemeral would silently stop
    the deletion mirror.
  - `docs/compliance/ai-act.md` §3 gains one dated sentence: the Article 50(1) discharge is the
    answer block's per-person introduction, so the deterministic command answers are outside it;
    `/start`'s line is a courtesy addition that records no introduction, and `/help`, like
    `/privacy` before it, carries none.
  - `docs/follow-ups.md` gains the ephemeral entry with its preconditions, the deletion mirror's
    unfolded comparison with what would resolve it, and the open question of whether the
    deterministic command answers should carry the disclosure line.
  - A docs test implements the drift check exactly as the decision above states it, over
    `docs/privacy/bot-assistant-privacy-policy.md`, `prompts/30-conduct.md` and `README.md`.
  - No file under `docs/privacy/` changes at all.
  - `docs/decisions/` gains one record per decision above, each with its date and its rejected
    alternatives, written from the next free number at merge time. The sibling specs in this
    folder claim 0106 onward (`01-receiving-media.md:236`, `02-sending-media.md:643`,
    `03-editing-messages.md:611`) and 0105 is taken, so the numbers are assigned when this unit
    merges, not now.

## Notes for launch

- Branches from `main` into its own worktree; no dependency on any other open unit. The live
  worktrees at the time of writing are `unit/sees-images`, `unit/threaded-replies` and
  `unit/web-search`, and none of them touches this path.
- **New:** `crates/core/src/commands.rs` (the catalogue, `recognized`, `OfferedCommand`, the
  fixed copy), plus a `pub mod commands;` and the re-exports at `lib.rs:51-94`.
- **Core edits:** `privacy.rs:95-104` (`family_command` becomes a projection of `recognized`);
  `assembly.rs:694-700` (the stamp's command condition widens to any recognised command),
  `:711-715` (the budget consultation widens to the three fixed-notice commands), `:760-768` (the
  delivery match gains the `/help` and `/start` arms and consults `offered_commands`),
  `:1296-1302` (`notice_answer` generalises to a fixed-answer method keyed by command);
  `window.rs:63-91` (the `LineWindow` key becomes conversation plus command); `assembly.rs` gains
  `offered_commands` beside the existing public entry points, reading `direct_chats`
  (`assembly.rs:311`) so an unserved channel kind offers nothing; `disclosure.rs:94-96` is reused
  unchanged for `/start`. The delivery match and the publish call the same `offered_commands`, so
  there is one place the direct-chat switch is applied.
- **Adapter edits:** a new publish routine in `driver.rs`, run as a fourth arm of the
  `tokio::select!` at `driver.rs:283-287` that publishes, retries and then parks on
  `std::future::pending()` — the same "lives as long as the run future" shape the other three
  arms have, and no detached task; `client.rs` gains `set_my_commands`, `delete_my_commands` and
  `set_chat_menu_button`, each under the existing `request` contract with the send ceiling
  (`client.rs:45`, `client.rs:505-527`), since a bounded wait there parks only the publish. The
  administrator ask uses `Authority::Moderator` and carries the reason in a comment beside it.
- **Test edits:** the loopback fake already records every request before answering and already
  returns `{"ok":true,"result":true}` for a method it has no script for (`server.rs:355-368`,
  `server.rs:450`), so the happy path needs no new handler. It gains two scripts: one that
  refuses a named command method, and one that never answers a named method — the second is a new
  kind for this suite, and it works because the fake spawns a task per connection
  (`server.rs:123-129`), so a hung method does not block `getUpdates`. A new
  `crates/adapters/telegram/tests/adapter/commands.rs` module is registered in `main.rs`.
- The publish routine's whole job is one core call per audience and a translation. If a reviewer
  finds a decision being made inside it — which command goes where, what a description says,
  whether a person may use one — the list moved into the wrong crate.
- `docs/units/telegram/07-buttons-and-callbacks.md` says at `:757` that it will write decisions
  "continuing from 0105", which is now taken. That is that unit's spec to correct, not this
  one's; it is noted here so whoever merges either unit sees the collision.
