# Telegram unit 15 — the commands menu: published from the list the core accepts

Date: 2026-08-25. The assistant already has commands. Five of them are the privacy family
(`privacy.rs:45-60`), the adapter reports which one a message invoked and the core matches the
report instead of the text (`translate.rs:293-307`, `message.rs:125-147`). None of them is
announced anywhere on the platform. A person who does not already know that `/privacydelete`
exists cannot find it: the platform's command list is empty because nothing has ever called
`setMyCommands`, and the published privacy policy is the only place the five names are written
down (`docs/privacy/bot-assistant-privacy-policy.md:145-147`).

This unit publishes the list, and answers the question that publishing a list forces: what is
the list, and who owns it. The answer is that there is exactly one list, it lives in the core
beside the matcher, and the platform's menu is a projection of it — so the menu cannot
advertise a command the core would ignore, and the core cannot accept a command nobody is told
about. The unit also fixes two things the work uncovered: a hand-typed `/Privacy` is not
recognised today, and the platform's own Start button already sends a command the core answers
by opening a model turn.

## Grounding

### What the platform actually does

Fetched from `https://core.telegram.org/bots/api` and `/bots/api-changelog` on 2026-08-25.
**The current version is Bot API 10.3, dated 24 August 2026** — one day before this spec, and
two releases past the 10.1 the unit brief names as current. Nothing in 10.2 or 10.3 changes
the four methods below; the one field they added to `BotCommand` is covered under its own
decision.

- **`setMyCommands`** takes `commands` (array of `BotCommand`, required, **at most 100**),
  `scope` (a `BotCommandScope` object, optional, defaults to `BotCommandScopeDefault`) and
  `language_code` (optional, two-letter ISO 639-1; empty means the list applies to every user
  in the scope for whose language no dedicated list exists). Returns `True`.
- **`BotCommand`** carries `command` — "Text of the command; **1-32 characters**. Can contain
  only **lowercase English letters, digits and underscores**" — and `description`, **1-256
  characters**. The leading `/` is not part of the field: the features page states separately
  that "commands must always start with the `/` symbol", which is the typed form, not the
  declared one.
- **`deleteMyCommands`** takes `scope` and `language_code` and nothing else. Its documented
  effect is exact and shapes this whole unit: "After deletion, **higher level commands will be
  shown** to affected users." Deleting a scope does not hide commands; it makes the next level
  up show instead.
- **`getMyCommands`** takes `scope` and `language_code` and returns an array of `BotCommand`.
  "If commands aren't set, an empty list is returned" — so from the outside, a scope set to an
  empty array and a scope never set are indistinguishable.
- **The seven scopes** are `default`, `all_private_chats`, `all_group_chats`,
  `all_chat_administrators`, `chat` (needs a `chat_id`), `chat_administrators` (needs a
  `chat_id`) and `chat_member` (needs a `chat_id` and a `user_id`).
- **Resolution is first-match, not union.** The documented algorithm is "the first list of
  commands which is set is returned". For a private chat the order is `chat` →
  `all_private_chats` → `default`, each preceded by its `language_code` variant. For a group
  the order is `chat_member` → `chat_administrators` → `chat` → `all_chat_administrators` →
  `all_group_chats` → `default`. An administrator therefore sees the administrator list
  *instead of* the member list, never both.
- **`setChatMenuButton`** takes `chat_id` (**Integer, optional, a private chat only**) and
  `menu_button` (optional, defaults to `MenuButtonDefault`). There is no group form: a group's
  menu button cannot be set. `MenuButton` is one of `MenuButtonCommands`, `MenuButtonWebApp`,
  `MenuButtonDefault`, and the documentation states that "if a menu button other than
  `MenuButtonDefault` is set for a private chat, then it is applied in the chat. Otherwise the
  default menu button is applied. By default, the menu button opens the list of bot commands."
- **A tap sends the bare command.** The features page: "Suggest a list of supported commands
  with descriptions when the user enters a `/` … Selecting a command from the list immediately
  sends it." No `@handle` suffix is added.
- **The platform disclaims the list.** Verbatim from the features page, Command Scopes: "Keep
  in mind that Bot API updates will not contain any information about the scope of a command
  sent by the user — in fact, **they may contain commands that don't exist at all in your
  bot**. Your backend should always verify that received commands are valid and that the user
  was authorized to use them regardless of scope."
- **Two commands are asked of every bot.** Global Commands: `/start` "begins the interaction
  with the user", `/help` "returns a help message, like a short text about what your bot can
  do and a list of commands". "Users will see a Start button the first time they open a chat
  with your bot" — that button is the client's, and pressing it sends `/start` whether or not
  the bot ever declared it.
- **Privacy mode decides whether a bare command arrives in a group at all.** With privacy mode
  on, a bot receives only commands aimed at it explicitly (`/command@this_bot`), general
  commands if it was the last bot to send a message, replies to its own messages, and inline
  messages. A menu tap sends the bare form, so under privacy mode a tapped command usually
  never reaches the bot. This deployment runs with privacy mode off by contract
  (`docs/reference/group-operator-contract.md:8-18`), so the menu works — but the menu is only
  as true as that contract.
- **`is_ephemeral`** was added to `BotCommand` in Bot API 10.2 (14 July 2026). A command
  declared ephemeral "is received by the target bot but remains invisible to all members of
  the chat, including both users and other bots". Answering one requires the ephemeral send
  path — `ephemeral_message_parameters` on `sendMessage` (10.3, replacing 10.2's
  `receiver_user_id`) or `reply_parameters.ephemeral_message_id` — within **15 seconds** of the
  triggering action, unless the bot is a chat administrator, which this one deliberately is
  not.

### What already exists in this tree

- **The invoked-command report.** `InvokedCommand` is the core's own type; the adapter reports
  the leading token with its own handle suffix removed and reports nothing for a foreign handle
  (`translate.rs:285-307`, wired at `driver.rs:432`). The handle is compared case-insensitively;
  **the token is not folded** (`translate.rs:304-306`), so `/Privacy` reaches the core as
  `InvokedCommand("/Privacy")`.
- **The matcher.** `privacy::family_command` matches the report against five exact spellings
  and returns a typed `PrivacyCommand` (`privacy.rs:95-104`). The five constants are
  `privacy.rs:45-60`.
- **The deterministic answer path.** A recognised command answers through
  `DeliveryItem::CommandAnswer` on the `IngestOutcome` (`message.rs:251-268`), which the driver
  sends with no reply target (`driver.rs:438-443`, `driver.rs:603-607`). No model turn is
  involved.
- **The command stamp.** A message invoking the privacy family, or triggering the deletion
  mirror, takes `LimitedBy::Command`: no debt, no answer-window count, no unlatch
  (`assembly.rs:694-700`).
- **Two different bounds already exist.** The notice takes a channel-keyed `LineWindow` over
  `ACKNOWLEDGMENT_WINDOW` plus a budget consultation (`assembly.rs:711-715`,
  `assembly.rs:1296-1302`, `window.rs:41,63-91`); the four rights commands take a
  principal-keyed `ReplyWindow` and no budget consultation (`window.rs:102-176`).
- **The suppression exemption is the privacy family's alone** (decision 0072,
  `assembly.rs:1149-1153`): an opted-out person's `/unblockprivacy` must work, so the family
  skips the drop. Every other message from them is dropped at ingestion.
- **The direct-chat switch refuses before the family is ever consulted.** `admit_channel` runs
  at `assembly.rs:629`, `resolve_writing_sender` at `:633`; with `DirectChats::Off` a direct
  message is disregarded at `assembly.rs:1206-1208` — including `/privacy`.
- **A direct message is always addressed** (`translate.rs:172`) and an addressed message
  summons a turn (`assembly.rs:1244-1249`). So today, in a served direct chat, pressing the
  platform's Start button opens a model turn whose entire user message is `/start`.
- **The mirror's token is not ours.** `/del` is the moderation bot's command; the assistant
  matches it only to erase its own stored copy of the named message, silently
  (`mirror.rs:1-35`, `mirror.rs:60-72`).
- **The disclosure value is already resolved on the assembly** (`assembly.rs:291,416`) and
  already knows how to prefix a text: `Disclosure::disclosed` (`disclosure.rs:93-97`). Its
  module states the fold this unit relies on: "a repeated line is harmless and a skipped first
  one is the violation" (`disclosure.rs:41-43`).
- **The wire client sends nested objects in its JSON body already** — `reply_parameters` at
  `client.rs:451-454` — so a scope object needs no string-encoding special case.
- **The adapter suite runs against a loopback fake** that answers per method and records every
  request (`crates/adapters/telegram/tests/adapter/server.rs:369-431`), which is where the
  publish is pinned.

### Two findings that shape the unit

**1. A scope cannot be emptied, only replaced or dropped one level.** There is no "show
nothing here". `deleteMyCommands` explicitly falls through to the level above, and an empty
array is indistinguishable from unset in `getMyCommands`' answer, so nothing in the
documentation says an empty array stops the fall-through. The only way to make a chat class
show nothing is to leave **every level of its chain** unset. That is a design constraint, not
an implementation detail: it decides which list the `default` scope has to carry.

**2. `is_ephemeral` can silently falsify a published privacy statement — and not through our
own commands.** The published policy states that when an administrator removes a message with
the moderation bot's reply command, that message is removed from our store as well
(`docs/privacy/bot-assistant-privacy-policy.md:117`). That promise rests on the assistant
seeing the administrator's bare `/del` (`mirror.rs:60-72`). If the moderation bot ever declares
`/del` ephemeral, the platform makes that message invisible to other bots, the mirror stops
firing, no error is raised anywhere, and the published sentence becomes false. Nothing in this
unit can prevent it; the unit records it, and the operator contract gains the one sentence that
lets an operator recognise the symptom.

## Decisions taken with this unit

- **One catalogue in the core, and the matcher reads it, 2026-08-25.** A new
  `crates/core/src/commands.rs` holds `enum Command` with `ALL`, and three total matches on it:
  `invocation()` (the token, leading `/` included, as the report carries it), `summary()` (the
  one-line description) and `offered()` (the channel kinds it is offered in, with its authority
  floor). `recognized(Option<&InvokedCommand>) -> Option<Command>` is the one recognition
  function, and `privacy::family_command` becomes a projection of it, keeping `PrivacyCommand`
  and its opposite-rules split exactly as they are. Adding a command means adding a variant;
  the compiler then demands its spelling, its description and its audience. *Rejected:* a list
  in the adapter, which is the drift this unit exists to remove and would put wording — which
  is behaviour — in a crate that holds none. *Rejected:* a list in the deployment
  configuration, which would let an operator advertise a command the binary does not implement.
- **The audience is a channel kind and an authority, not a scope, 2026-08-25.** The core gains
  no scope vocabulary. `Assistant::offered_commands(ChannelKind, Authority) -> Vec<OfferedCommand>`
  answers "what may this class of person invoke in this class of channel", using vocabulary the
  core already has (`message.rs:95-123`). The adapter asks three times and maps the answers onto
  the platform's scopes. *Rejected:* a `CommandScope` enum in the core mirroring the platform's
  seven scopes — platform shapes in the core, six of which have no neutral meaning, and the
  invariant would be broken for nothing.
- **All four chat-class scopes are owned and republished at every startup, 2026-08-25.**
  `default`, `all_private_chats`, `all_group_chats` and `all_chat_administrators` are set every
  time the process starts, unconditionally, with no `language_code`. Because resolution stops
  at the first list that is set, owning every addressable level of both chains is the only way
  to be sure a person lands on our list. *Rejected:* publishing only `default` — the group chain
  has five levels above it, and any one of them set by another holder of the same token would
  shadow ours silently. *Rejected:* publishing once and remembering — the platform is the
  authority on what is currently published, a second process with the same token can overwrite
  it, and three calls at startup are cheaper than a memory that can be wrong.
- **An empty list is cleared with `deleteMyCommands`, never set as an empty array, 2026-08-25.**
  The documented way to unset a scope is the delete method; whether an empty array counts as
  "set" for the first-match algorithm is not documented anywhere, and no shipped behaviour will
  rest on an inference. *Rejected:* `setMyCommands` with `commands: []`.
- **The `default` scope carries the PRIVATE list, 2026-08-25.** `default` is the last level of
  the private chain and every level of the group chain above it is owned, so making it the
  private list is what keeps both chains honest. Under `DirectChats::Off` the private list is
  empty, so `all_private_chats` and `default` are both deleted, the private chain ends unset,
  and a private chat shows no commands at all — which is exactly true, since a direct message
  is disregarded before any command is looked at (`assembly.rs:1206-1208`). *Rejected:* letting
  `default` carry the group list — a private chat under `DirectChats::Off` would then be
  offered group commands that answer nothing, which is the precise lie this unit exists to
  prevent. The residual, accepted: if the group publish fails while the default publish
  succeeded, a group shows the private list for that process's lifetime; the private list is a
  superset of the group list, so every command shown still works.
- **`getMyCommands` is not called, 2026-08-25.** It answers per scope and per language, and the
  language-keyed lists that would actually shadow ours cannot be enumerated, so a read-back can
  never prove what a given person sees. An unconditional publish costs the same calls and needs
  no comparison. *Rejected:* publish-only-if-changed, which doubles the call count to prove
  something weaker than it appears to prove.
- **The default menu button is set once to `MenuButtonCommands`, 2026-08-25.** The platform's
  own default already opens the command list, but the button is per-token state: a
  `MenuButtonWebApp` set once through the platform's bot management interface would keep hiding
  the list, and this
  assistant has no Web App. One `setChatMenuButton` with no `chat_id` at startup makes the
  button the code's decision instead of a leftover. *Rejected:* not calling it — a leftover
  button hides the whole menu and nothing in the repository would explain why. *Rejected:*
  per-chat buttons — `chat_id` accepts a private chat only, so there is nothing per-group to
  set, and a per-person call would buy nothing over the default.
- **`/help` and `/start` join the core as fixed-answer commands, 2026-08-25.** The platform asks
  every bot to support both, the Start button exists in every private chat whether we declare
  the command or not, and today pressing it opens a model turn against the text `/start`
  (`translate.rs:172`, `assembly.rs:1244-1249`). `/help`'s answer is composed from the same
  catalogue that feeds the menu — the offered commands for that channel kind and authority,
  each with its `summary()` — so the help text and the menu can never disagree. *Rejected:*
  leaving both unhandled, which keeps a model turn on the platform's own onboarding tap and
  leaves the menu's `/help` entry either absent or lying. *Rejected:* a hand-written help text,
  a second copy of the list.
- **`/start`'s answer opens with the resolved disclosure line, 2026-08-25.** It is by definition
  a person's first interaction, and a deterministic command answer is not an answer block, so
  the ledger-based introduction (unit 12) never covers it. The line is already resolved on the
  assembly and already has a prefixing method (`disclosure.rs:93-97`); the person's first model
  answer may repeat it, which the disclosure module already names as the harmless direction
  (`disclosure.rs:41-43`). *Rejected:* recording the `/start` answer as an answer block so the
  introduction receipt sees it — that would put a machine-written block into the model's own
  history for a message the model never took a turn on.
- **The suppression exemption does not widen, 2026-08-25.** Only the privacy family stays exempt
  (decision 0072). An opted-out person's `/help` or `/start` is dropped at ingestion like every
  other message from them: they asked not to be processed, and a help text is not a right that
  has to reach them. *Rejected:* exempting every recognised command, which would quietly turn
  the opt-out into "except when you type a slash".
- **The command stamp widens to any recognised command, 2026-08-25.** `assembly.rs:694-700`
  becomes "a recognised command, or the mirror" instead of "the privacy family, or the mirror",
  so `/help` and `/start` take no debt, no answer-window count and no unlatch. This is the
  widening the existing shape already implies; no new branch appears.
- **`/help` and `/start` are bounded like the notice, 2026-08-25**: a budget consultation plus a
  channel-keyed window, not the rights commands' per-person window. They are notices, not
  rights: nobody's data protection depends on them arriving, and they are lines anyone in the
  chat can trigger, which is the flood shape the notice window was built for (`window.rs:35-41`).
  To give each fixed line its own window without a new field per command, `LineWindow` keys on
  the conversation **and the command** instead of the conversation alone. *Rejected:* a second
  and third `LineWindow` field on the assembly — a field per command is the bolted-on shape the
  repository's standards refuse. *Rejected:* one shared window across all three — a person who
  taps Start and then types `/help` would meet silence on the first question they ever ask.
- **Recognition folds ASCII case on the token, 2026-08-25.** The platform allows only lowercase
  letters, digits and underscores in a declared command, so no two declared commands can collide
  under folding. Today `/Privacy` typed by hand is unrecognised and falls through to an ordinary
  message, which means a person exercising a data right with an autocapitalising keyboard gets
  nothing. The fold lives in `commands::recognized`, in the core, because which spellings the
  core accepts is the core's decision. *Rejected:* folding the token in `translate.rs`'s report
  — the report is meant to carry what the platform delivered, minus the handle, and rewriting it
  would hide the typed form from every later reader.
- **An unrecognised command stays an ordinary message, 2026-08-25.** Nothing answers "I do not
  know that command". Privacy mode is off by contract, so every other bot's commands arrive here
  too — the deletion mirror depends on exactly that — and a bot that answers unknown commands
  would interrupt every other bot in the group. The platform states outright that updates may
  carry commands that do not exist in the bot. *Rejected:* a fixed unknown-command line.
  *Rejected:* suppressing unrecognised commands from the model's context, which would blind the
  model to half of what the group is doing.
- **Nothing about the publish is written to the ledger, 2026-08-25.** The published list is a
  projection of code, re-derived at every startup. *Rejected:* a note block naming what was
  published — the ledger cannot supersede a fact it cannot observe, and a second process with
  the same token would make the block quietly false.
- **No command is declared ephemeral, 2026-08-25.** `is_ephemeral` is genuinely attractive for
  `/privacydelete`, whose typed form is currently visible to a whole public group. It cannot be
  taken here: answering an ephemeral command needs the ephemeral send path, which the adapter
  does not have (`client.rs:439-461` sends one plain `sendMessage`), within 15 seconds, and the
  administrator exemption is unavailable because the operator contract keeps the assistant a
  non-administrator. Declaring the flag without the send path would answer an invisible question
  with a message the whole group sees — the opposite of the intent. It goes to
  `docs/follow-ups.md` with its preconditions. *Rejected:* declaring the flag now and sending
  the answer as an ordinary message.
- **`/del` is never published and never enters the catalogue, 2026-08-25.** It is the moderation
  bot's command; the assistant only keeps its own books when an administrator uses it. Listing it
  would advertise the assistant as the actor that deletes messages, which contradicts decision
  0070 — the assistant assesses, a human decides. It stays matched where it is today, in
  `mirror.rs`, outside `commands::recognized`.

## The unit's contract

There is one list of commands, `Command::ALL` in the core, and three things read it: the
matcher that recognises an invocation, the `/help` answer, and the platform menu. At startup the
adapter asks the core for the commands offered to a member in a direct channel, to a member in a
group and to an administrator in a group, translates each answer into `BotCommand` objects with
the leading marker stripped, and publishes them to `default`, `all_private_chats`,
`all_group_chats` and `all_chat_administrators`; a scope whose list is empty is cleared with
`deleteMyCommands` instead, and the default menu button is set to the commands button once. The
publish runs beside the poll, never delays it and never refuses the start; a failure is logged
and the next start republishes. A menu entry that the core does not act on cannot exist, because
the menu is derived from the recognition; a command the core does not recognise — another bot's,
or one the platform delivered that never existed — stays an ordinary message, recorded verbatim
and answered by nothing deterministic. `/help` and `/start` answer from the catalogue with no
model turn, no debt and no unlatch, bounded per channel per window and consulted against the
budgets; `/start`'s answer opens with the disclosure line. Recognition folds case, so a
hand-typed `/Privacy` works. The suppression exemption, the rights commands' per-person bound,
the deletion mirror and the direct-chat switch all behave exactly as before. Nothing streams,
because nothing here carries bytes: the whole publish is five small JSON bodies with no file,
no upload and no download anywhere in the path. Nothing is written to the ledger by the publish,
and no personal data reaches a new recipient, a new store or a new category — `setMyCommands`,
`deleteMyCommands` and the default-button call carry no chat id, no user id and no message text,
so `docs/privacy/records-of-processing.md`, `docs/privacy/dpia.md`, `docs/privacy/lia.md` and the
published policy are all unchanged by this unit. No new dependency.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary scan and the secret scan clean; no new dependency. `crates/core` gains no
  platform word — no scope names, no method names, no "menu".
- **AC2** One list, mechanically: a core test asserts that for every `Command::ALL` variant,
  `recognized(&InvokedCommand::new(variant.invocation()))` returns that same variant, that every
  `summary()` is between 1 and 256 characters, and that every `invocation()` is a leading `/`
  followed by 1 to 32 characters drawn only from lowercase ASCII letters, digits and underscores
  — the platform's own bound, checked where the list is written. A second test asserts
  `privacy::family_command` agrees with the catalogue on all five privacy tokens and returns
  `None` for `/help`, `/start` and `/del`.
- **AC3** The startup publish is pinned against the loopback fake: exactly four `setMyCommands`
  requests are recorded under the default configuration, one per chat-class scope, each carrying
  its scope object with the documented `type` string; every `command` field is the invocation
  with the leading `/` removed; no request carries a `language_code`; no list exceeds 100
  entries. The group list and the administrator list are equal (no command is administrator-only
  today) and the private list is a superset of the group list.
- **AC4** With `DirectChats::Off`, `all_private_chats` and `default` are each cleared with
  `deleteMyCommands` and neither is set, while `all_group_chats` and `all_chat_administrators`
  are still set — pinned on the recorded requests.
- **AC5** Exactly one `setChatMenuButton` is recorded, carrying `menu_button.type = "commands"`
  and no `chat_id`.
- **AC6** A publish failure is contained: with the fake scripted to refuse every command method,
  the adapter still starts, still polls, still ingests and still answers, and one warning per
  failed call is logged — pinned end to end, so a cosmetic list can never keep the assistant off
  the air. The publish also never delays the first poll: pinned by driving an update through
  while the command methods hang.
- **AC7** `/help` answers from the catalogue: the answer names every command offered in that
  channel for that sender's authority, each with its `summary()`, and names none that is not
  offered there. It answers at most once per channel per `ACKNOWLEDGMENT_WINDOW`, is silent when
  a budget refuses the sender, takes no model turn and carries `LimitedBy::Command` on its stored
  message — all pinned.
- **AC8** `/start` in a direct chat answers with the resolved disclosure line, a blank line and
  the fixed text, opens no model turn, and its stored message carries `LimitedBy::Command` —
  pinned, including that the same tap opened a model turn before this unit.
- **AC9** `/Privacy` and `/PRIVACYDELETE` are recognised and answer exactly as their lowercase
  forms do, while the stored message text stays verbatim in every case — pinned in the core and
  through the adapter.
- **AC10** Unchanged behaviour, pinned as regressions: an opted-out person's `/privacy` and
  `/unblockprivacy` still answer, their `/help` and `/start` are disregarded with nothing
  written; the four rights commands keep their per-person window and their budget exemption; an
  administrator's `/del` reply still mirrors; a `/foo` nobody declared is recorded as an ordinary
  message and draws no deterministic answer.
- **AC11** `LineWindow` keyed by conversation and command: `/privacy`, `/help` and `/start` each
  keep an independent window in the same channel, and each is silent for the rest of its own
  window — pinned under paused time, with the notice's existing behaviour unchanged.
- **AC12** Documentation ships with the code: `docs/reference/group-operator-contract.md` gains a
  short section stating that the assistant publishes its own command list at every start (so an
  edit made through the platform's bot management interface is overwritten on the next restart),
  that the menu depends on privacy mode staying off, and that a moderation bot declaring `/del`
  ephemeral would silently stop the deletion mirror. `docs/follow-ups.md` gains the ephemeral
  entry with its preconditions. A docs test asserts that every command name written in
  `docs/privacy/bot-assistant-privacy-policy.md` exists in the catalogue, so the policy and the
  code cannot drift apart. No file under `docs/privacy/` changes otherwise, and a test asserts
  the publish path touches no store and no personal field.

## Notes for launch

- Branches from `main` into its own worktree; no dependency on any other open Telegram unit.
- **New:** `crates/core/src/commands.rs` (the catalogue, `recognized`, `OfferedCommand`), plus a
  `pub mod commands;` and the re-exports at `lib.rs:51-94`.
- **Core edits:** `privacy.rs:95-104` (`family_command` becomes a projection of `recognized`);
  `assembly.rs:694-700` (the stamp's command condition widens), `:711-715` (the budget
  consultation widens to the three fixed-notice commands), `:760-768` (the delivery match gains
  the `/help` and `/start` arms), `:1296-1302` (`notice_answer` generalises to a fixed-answer
  method keyed by command); `window.rs:63-91` (the `LineWindow` key becomes conversation plus
  command); `assembly.rs` gains `offered_commands` beside the existing public entry points,
  reading `direct_chats` (`:311`) so an unserved channel kind offers nothing; `disclosure.rs:93-97`
  is reused unchanged for `/start`.
- **Adapter edits:** a new publish routine in `driver.rs`, run as a fourth arm of the
  `tokio::select!` at `driver.rs:283-287` that publishes and then parks on
  `std::future::pending()` — the same "lives as long as the run future" shape the other three
  arms have, and no detached task; `client.rs` gains `set_my_commands`, `delete_my_commands` and
  `set_chat_menu_button`, each under the existing `request` contract with the send ceiling
  (`client.rs:45`, `:505-527`), since a bounded wait there parks only the publish.
- **Test edits:** `crates/adapters/telegram/tests/adapter/server.rs:369-431` gains handlers for
  the three new methods answering `true` and recording the body; a new
  `crates/adapters/telegram/tests/adapter/commands.rs` module registered in `main.rs`.
- The publish routine's whole job is one core call and a translation. If a reviewer finds a
  decision being made inside it — which command goes where, what a description says, whether a
  person may use one — the list moved into the wrong crate.
- Unit 07 (offered choices) is in flight and adds a second way a turn can be summoned. It does
  not touch this path: a pressed option is not an invoked command, and nothing here reads the
  frontier of a turn.
