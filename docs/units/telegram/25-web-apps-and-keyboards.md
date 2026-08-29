# Telegram unit 25 — reply keyboards and web apps: refused on five counts, and the refusal is checkable

Date: 2026-08-27. Unit 07 specifies the half of the button surface this project wants: an inline
keyboard under an answer, a press recorded as its own act, an offer bound to one person. This
unit covers the other half — the keyboard that replaces a member's typing area, the buttons that
ask a member for a phone number or a position, the buttons that ask a member to point at another
person or to hand over a chat, and everything Mini App: `WebAppInfo`, `web_app_data`,
`answerWebAppQuery`, a menu button that launches an app, and the signed `initData` such an app
receives.

The answer to all of it is no. That is not scope trimmed to save effort. Five separate facts,
each provable from the platform documentation or from this repository, each sufficient on its
own, say that this surface either cannot work here or must not. The unit's job is to make the no
a property of the source — an assertion that fails when somebody reverses it — instead of an
absence that survives only while nobody writes the field.

What ships is small: seventeen names added to the substring refusal scan a sibling unit already
specifies, two more capability flags on the startup notice a sibling unit already builds, four
pinned fail-safe skips for the service messages this surface produces, one containment assertion
on the poll's update selection, one decision record, and one section of the operator contract.
The document is the deliverable that matters.

## The five findings, stated at the top

**1. Every button this code could set is private-chat only, and this deployment serves no
private chats — but a Mini App still reaches a group by a route no code here controls.** The Bot
API repeats the sentence once per field: `request_users`, `request_chat`, `request_managed_bot`,
`request_contact`, `request_location`, `request_poll` and `web_app` on `KeyboardButton` each
carry "Available in private chats only", and the inline `web_app` field says "Available only in
private chats between a user and the bot". `setChatMenuButton` changes "the bot's menu button in
a private chat, or the default menu button". The published privacy notice states this
deployment's position — "We do not serve direct chats: a direct message is rejected and not
stored" (`docs/privacy/bot-assistant-privacy-policy.md:23-24`) — and the core enforces it at
`crates/core/src/assembly.rs:1206-1208`, where a direct channel under `DirectChats::Off` returns
`IngestOutcome::Disregarded` before any write. So every affordance on this surface that the Bot
API can set is offered only where this assistant answers nothing. The half-truth to avoid is the
comfortable one: Direct Link Mini Apps "can be launched from a direct link in any chat" and are
"aware of the current chat context" through `chat_type` and `chat_instance`, so a Mini App is not
confined to private chats. It is confined to routes configured outside the Bot API, in the
platform's bot management interface, which is why finding 3 and the startup notice below exist
at all.

**2. A reply-keyboard press arrives as an ordinary message, with nothing on it saying so.** The
platform describes the plain button: "If none of the fields other than text,
icon_custom_emoji_id, and style are used, it will be sent as a message when the button is
pressed." The `Message` object has exactly one keyboard-shaped field, `reply_markup`, typed
`InlineKeyboardMarkup` — there is no `via_keyboard`, no button reference, no flag. Decision 0017
records what a person typed, verbatim, and the adapter's `text_of`
(`crates/adapters/telegram/src/translate.rs:466-473`) reads `text` then `caption` and knows
nothing else. So the assistant's own label, chosen by a model, would land on an append-only
ledger as the member's own words, and no later reader could separate the two. Unit 07 examined
that exact shape and refused it: "*Rejected:* synthesising a message ('I want to use it on my
device') — putting words in a member's mouth, and it would make the ledger's record of what was
said untrue" (`docs/units/telegram/07-buttons-and-callbacks.md:357-359`). A reply keyboard is
that refused design with the platform performing the synthesis.

**3. A Mini App is a website, and this deployment serves no HTTP.** `WebAppInfo` has one field:
"An HTTPS URL of a Web App to be opened with additional data as specified in Initializing Web
Apps." Nothing in the workspace binds a listening socket. `TcpListener` appears in five files,
every one of them test support — `crates/adapters/telegram/tests/adapter/server.rs:19`,
`crates/adapters/telegram/tests/adapter/tools.rs:15`,
`crates/core/tests/spine/chat_completions.rs:19`, `crates/core/tests/spine/lookup_wire.rs:11`,
`crates/assistant/tests/process/support.rs:17` — and in no `src` tree. The process is one
outbound long poll (`crates/adapters/telegram/src/client.rs:305-322`) and nothing else. An app
needs a public origin, a certificate, static assets, a second process, and the bot token
available as an HMAC key wherever `initData` is checked — a token that today lives only inside
the adapter's client module under the stated rule that "no log line and no error string ever
carries the token" (`crates/adapters/telegram/src/client.rs:1-9`).

**4. `answerWebAppQuery` sends a message on behalf of the member, and takes no `chat_id`.**
Verbatim: "Use this method to set the result of an interaction with a Web App and send a
corresponding message on behalf of the user to the chat from which the query originated." Its
two parameters are `web_app_query_id` and `result`; the bot does not choose where the message
appears. The inline `web_app` field and `MenuButtonWebApp` both repeat the same sentence: "The
Web App will be able to send an arbitrary message on behalf of the user using the method
answerWebAppQuery." This is finding 2 one layer further out — not the assistant's words recorded
as a member's, but the assistant's words posted in a chat under a member's name, read by
everyone else as theirs.

**5. Two of these buttons hand over power on a single tap.** `KeyboardButtonRequestChat`'s own
preamble: "The bot will be granted requested rights in the chat if appropriate", with
`bot_administrator_rights` naming which. One press by anybody who can see the button puts this
assistant into a chat of the presser's choosing, with the administrator rights the button asked
for — a route around decision 0052, which makes the configured operator's invitation the only
admission. `request_managed_bot` "will ask the user to create and share a bot that will be
managed by the current bot", the created bot arrives as `ManagedBotCreated`, and its own field
documentation states "The bot's token can be fetched using the method getManagedBotToken". A
second bot's credential, obtainable by this process, in a project whose token discipline is one
module and one secret reference.

None of the five is repaired by writing more code. Findings 2, 4 and 5 are about attribution and
authority, which this project has already decided; finding 1 is a platform constraint the
deployment cannot reach around and a launch route no code here controls; finding 3 is a second
system that does not exist and is not this unit's to invent.

## Grounding

### The platform, read 2026-08-27

Fetched from `core.telegram.org/bots/api`, `core.telegram.org/bots/webapps` and the changelog at
`core.telegram.org/bots/api-changelog` on 27 August 2026, from the raw pages. The brief for this
series named Bot API 10.1 (11 June 2026) as current; the changelog's newest entry is **Bot API
10.3, dated 24 August 2026**, with 10.2 on 14 July 2026. Every sentence in quotation marks below
was read on that date.

- **`ReplyKeyboardMarkup`, complete.** `keyboard` ("Array of button rows, each represented by an
  Array of KeyboardButton objects") and six Optional fields: `is_persistent` ("Requests clients
  to always show the keyboard when the regular keyboard is hidden. Defaults to False"),
  `resize_keyboard`, `one_time_keyboard` ("Requests clients to hide the keyboard as soon as it's
  been used. The keyboard will still be available"), `input_field_placeholder` ("1-64
  characters"), `selective`, and — new in 10.3 — `force_reply` ("Pass True if the reply interface
  must be shown to the user, as if they had manually selected the bot's message and tapped
  'Reply'"). The changelog entry reads "Added the field _force_reply_ to the classes
  InlineKeyboardMarkup and ReplyKeyboardMarkup." The object's preamble: "Not supported in channels
  and for messages sent on behalf of a business account." **Groups are supported**, which is the
  case that matters here. No cap on rows or buttons is documented.
- **`selective` targets a rule, not a person.** Verbatim: "Use this parameter if you want to show
  the keyboard to specific users only. Targets: 1) users that are @mentioned in the text of the
  Message object; 2) if the bot's message is a reply to a message in the same chat and forum
  topic, sender of the original message." The illustrative example states the default plainly:
  "Other users in the group don't see the keyboard."
- **`KeyboardButton`, complete.** "At most one of the fields other than text,
  icon_custom_emoji_id, and style must be used to specify the type of the button. For simple text
  buttons, String can be used instead of this object to specify the button text." Fields: `text`
  (quoted in finding 2), `icon_custom_emoji_id`, `style` ("danger" / "success" / "primary"),
  `request_users`, `request_chat`, `request_managed_bot`, `request_contact` ("If True, the user's
  phone number will be sent as a contact when the button is pressed. Available in private chats
  only."), `request_location` ("If True, the user's current location will be sent when the button
  is pressed. Available in private chats only."), `request_poll`, and `web_app` ("If specified,
  the described Web App will be launched when the button is pressed. The Web App will be able to
  send a 'web_app_data' service message. Available in private chats only.").
- **`KeyboardButtonRequestUsers`.** `request_id` ("Signed 32-bit identifier of the request that
  will be received back in the UsersShared object. Must be unique within the message."),
  `user_is_bot`, `user_is_premium`, `max_quantity` ("The maximum number of users to be selected;
  **1-10**. Defaults to 1"), `request_name` ("Pass True to request the users' first and last
  names"), `request_username`, `request_photo`.
- **`KeyboardButtonRequestChat` grants rights.** Beside `request_id`, `chat_is_channel`,
  `chat_is_forum`, `chat_has_username`, `chat_is_created`, `bot_is_member`, `request_title`,
  `request_username` and `request_photo`, it carries `user_administrator_rights` and
  `bot_administrator_rights`, and the preamble states the consequence quoted in finding 5.
- **`KeyboardButtonRequestManagedBot`.** `request_id`, `suggested_name`, `suggested_username`.
  Its preamble: "Information about the created bot will be shared with the bot using the update
  managed_bot and a Message with the field managed_bot_created." So this button produces both a
  fourth service message and a fifth update type.
- **`KeyboardButtonPollType`** has one Optional field, `type`: "If quiz is passed, the user will
  be allowed to create only polls in the quiz mode. If regular is passed, only regular polls will
  be allowed."
- **What comes back.** `UsersShared` carries `request_id` and `users`, an "Array of SharedUser".
  `SharedUser` carries `user_id` plus Optional `first_name`, `last_name`, `username` and `photo`
  ("Array of PhotoSize"), each present only if requested. `ChatShared` carries `request_id`,
  `chat_id`, and Optional `title`, `username` and `photo`. `ManagedBotCreated` carries `bot`, a
  full `User`. Both shared objects warn that the identifier may be unusable: "The bot may not have
  access to the user and could be unable to use this identifier, unless the user is already known
  to the bot by some other means."
- **`ReplyKeyboardRemove`** carries `remove_keyboard` (type `True`) and `selective`. Its preamble
  states the durability that makes it a question at all: "By default, custom keyboards are
  displayed until a new keyboard is sent by a bot. An exception is made for one-time keyboards
  that are hidden immediately after the user presses a button."
- **`ForceReply`** carries `force_reply` (type `True`), `input_field_placeholder` (1-64
  characters) and `selective`, with the same two targeting rules quoted above, and the preamble
  "Not supported in channels and for messages sent on behalf of a user account." Its stated
  purpose is one this deployment does not have: "This can be extremely useful if you want to
  create user-friendly step-by-step interfaces without having to sacrifice privacy mode", and the
  worked example opens "A poll bot for groups runs in privacy mode (only receives commands,
  replies to its messages and mentions)."
- **All four markup objects travel in one field.** `sendMessage`'s `reply_markup` is typed
  "InlineKeyboardMarkup or ReplyKeyboardMarkup or ReplyKeyboardRemove or ForceReply". A message
  carries exactly one of them, so an unconditional keyboard removal on every send would make an
  inline keyboard impossible on the same message.
- **A reply keyboard would also make a message uneditable.** The updating-messages section:
  "Please note, that it is currently only possible to edit messages without reply_markup or with
  inline keyboards." Unit 03 owns editing; this is stated so that a future reversal cannot claim
  the interaction was unexamined.
- **A pressed reply-keyboard button leaves no trace on the incoming message.** The `Message` field
  table has one keyboard field: `reply_markup`, typed `InlineKeyboardMarkup`, "Optional. Inline
  keyboard attached to the message. login_url buttons are represented as ordinary url buttons."
  The whole table was read for a second one.
- **The four service messages this surface produces.** `Message.users_shared` ("Service message:
  users were shared with the bot"), `Message.chat_shared` ("Service message: a chat was shared
  with the bot"), `Message.web_app_data` ("Service message: data sent by a Web App") and
  `Message.managed_bot_created` ("Service message: user created a bot that will be managed by the
  current bot"). `WebAppData` carries `data` and `button_text`, and the documentation attaches the
  same warning to each: "Be aware that a bad client can send arbitrary data in this field."
- **The fifth update type.** `Update.managed_bot`, typed `ManagedBotUpdated`: "A new bot was
  created to be managed by the bot, or token or owner of a managed bot was changed."
  `ManagedBotUpdated` carries `user` and `bot`, with "Token of the bot can be fetched using the
  method getManagedBotToken."
- **`answerWebAppQuery`** takes `web_app_query_id` and a `result` of type `InlineQueryResult`, and
  returns `SentWebAppMessage`, whose one Optional field is `inline_message_id`. Its description is
  quoted in finding 4.
- **Two neighbouring methods complete the family.** `savePreparedInlineMessage` "Stores a message
  that can be sent by a user of a Mini App", taking `user_id`, an `InlineQueryResult` and four
  `allow_*_chats` switches; `savePreparedKeyboardButton` "Stores a keyboard button that can be
  used by a user within a Mini App", taking `user_id` and a `button`, and stating "The button must
  be of the type request_users, request_chat, or request_managed_bot" — the three that hand back
  other people's identities or a chat.
- **The menu button.** `setChatMenuButton` "change[s] the bot's menu button in a private chat, or
  the default menu button"; `chat_id` is "Unique identifier for the target private chat. If not
  specified, the bot's default menu button will be changed." `MenuButton` is one of
  `MenuButtonCommands`, `MenuButtonWebApp` or `MenuButtonDefault`. `MenuButtonWebApp` carries
  `type` (value "web_app"), `text` and `web_app`. `getChatMenuButton` reads one back, per chat or
  default.
- **Seven launch routes, and only three are settable from the Bot API.** The Mini Apps page:
  "Telegram currently supports seven different ways of launching Mini Apps: the main Mini App from
  a profile button, from a keyboard button, from an inline button, from the bot menu button, via
  inline mode, from a direct link – and even from the attachment menu." The keyboard button, the
  inline button and the menu button are Bot API objects; the main Mini App is set in the bot
  management interface ("go to @BotFather and set up your bot's Main Mini App"), the attachment
  menu by "the /setattach command" and only in the test environment for most bots ("Attachment
  menu integration is currently only available for major advertisers on the Telegram Ad
  Platform"), the direct link follows from an app existing, and the inline-mode route is the
  `button` parameter of `answerInlineQuery`, which unit 08 refuses.
- **An eighth route exists and belongs to unit 09.** `sendChatJoinRequestWebApp` shows a Mini App
  to a person who asked to join a chat, before the outcome is decided, and hands the app "basic
  user information (ID, name, username, language_code, photo), the chat info (ID, type, title,
  username, photo)" and a `chat_join_request_query_id`; `answerChatJoinRequestQuery` then takes a
  `result` that "Must be either 'approve' … 'decline' … or 'queue' to leave the decision to other
  administrators." Unit 09's AC2 already forbids both method names by scan. Named here only so the
  Mini App surface is enumerated whole.
- **`getMe` reports two settings on this surface, and they are the only observable ones.**
  `User.has_main_web_app`: "True, if the bot has a main Web App. Returned only in getMe."
  `User.can_manage_bots`: "True, if other bots can be created to be controlled by the bot.
  Returned only in getMe." The second is the precondition `request_managed_bot` names: "Available
  for bots that enabled management of other bots in the @BotFather Mini App."
- **What a launch actually hands the origin, per route.** `WebAppInitData` "is empty if the Mini
  App was launched from a keyboard button or from inline mode." An inline-button launch receives
  "basic user information (ID, name, username, language_code)" and a `query_id`; an
  attachment-menu launch adds a photo and the chat partner; a direct-link launch adds `chat_type`
  and `chat_instance` and has "no access to the chat". `WebAppUser`'s `photo_url` is "Only
  returned for Mini Apps launched from the attachment menu." So the identity handed over varies by
  route, and the constant across all of them is the HTTPS request itself: an origin learns the
  member's network address and client on every launch, whatever `initData` carries.
- **`initData` verification, both paths, verbatim.** The page warns on the field: "WARNING:
  Validate data from this field before using it on the bot's server", and on the parsed form:
  "Data from this field should not be trusted." The bot-side check: "comparing the received hash
  parameter with the hexadecimal representation of the HMAC-SHA-256 signature of the
  data-check-string with the secret key, which is the HMAC-SHA-256 signature of the bot's token
  with the constant string WebAppData used as a key." The data-check-string is "a chain of all
  received fields, sorted alphabetically, in the format key=<value> with a line feed character
  ('\n', 0x0A) used as separator", and the sketch is `secret_key = HMAC_SHA256(<bot_token>,
  "WebAppData")` compared against `hash`. The third-party path validates a base64url Ed25519
  `signature` over a data-check-string that prepends `<bot_id>:WebAppData` and excludes both
  `hash` and `signature`, against a published production key. Both paths add: "To prevent the use
  of outdated data, you can additionally check the auth_date field."
- **The keyboard-button app returns a string and needs no server for the reply.** "Mini Apps
  launched from a web_app type keyboard button can send data back to the bot in a service message
  using Telegram.WebApp.sendData. This makes it possible for the bot to produce a response without
  communicating with any external servers." The reply needs no server; the app itself still needs
  an origin, so this does not escape finding 3.
- **Mini App origins were tightened in 10.2.** "Hardened the security of Mini Apps by disallowing
  the usage of Mini App methods from origins different from the original Mini App domain. The
  protection will be automatically enabled for all Mini Apps on July 20, 2026. You can opt-out
  from the protection through the @BotFather Mini App. If you do so, you acknowledge that it is
  the responsibility of the bot to ensure that the Mini App has no links to untrusted sites." A
  hosting decision now carries an opt-out somebody could take by hand, outside this repository.

### Our tree, at `7fb217d`

- **The adapter's incoming model has nine fields and none is on this surface.** `Incoming`
  (`crates/adapters/telegram/src/client.rs:125-144`) decodes `message_id`, `date`, `chat`, `from`,
  `sender_chat`, `text`, `caption`, `reply_to_message` and `pinned_message`. No `users_shared`, no
  `chat_shared`, no `web_app_data`, no `managed_bot_created`, no `contact`, no `location`. `Update`
  is as narrow (`client.rs:109-121`): `update_id`, `message`, `edited_message`, `my_chat_member`,
  with the stated reason that "Unknown fields are ignored by the decoder, so the model stays
  exactly as small as the translation needs". Everything on this surface is discarded at the decode
  boundary, before any process memory holds it as a typed value. That is the property this unit
  pins.
- **A service message on this surface already reaches a named skip.** `translate`
  (`translate.rs:119-190`) checks membership, the edit, the non-message, the chat kind, the pin
  branch, then `sender_chat` (`:159-161`), then a missing sender (`:162-164`, `Skip::NoSender`),
  then `text_of` (`:165-167`, `Skip::NoText`). A `web_app_data`, `users_shared`, `chat_shared` or
  `managed_bot_created` message has neither `text` nor `caption`, so it exits at `Skip::NoText`
  when the platform names a sender and at `Skip::NoSender` when it does not. The documentation
  does not state which of the two shapes it sends for these four, so this unit's pins cover both
  instead of asserting one.
- **The poll names three update types; a fourth is reachable and a fifth is not.**
  `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]` (`client.rs:103`),
  sent as `allowed_updates` on every poll (`client.rs:319`), with the comment that "an absent
  selection inherits whatever an earlier setting left on the token, so the selection is stated
  instead of assumed". All four service messages above arrive inside `message`, which is already
  subscribed — so unlike unit 08's inline updates, they can arrive today, in a group, if anything
  ever set a keyboard on this token. `managed_bot` is a top-level update type and is not
  subscribed.
- **The send path builds one JSON body and sets no markup.** `send_body` (`client.rs:439-461`)
  writes `chat_id`, `text`, an Optional `parse_mode` and Optional `reply_parameters`, and throws
  the platform's answer away (`let _sent: serde_json::Value`, `client.rs:459`). There is no
  `reply_markup` key anywhere in the crate: a substring search of `crates/` for `reply_markup`,
  `keyboard` and `web_app` returns no hit in any `.rs` file. Unit 07 adds the first one.
- **The outbound model carries no markup either.** `OutboundReply`
  (`crates/core/src/message.rs:373-390`) is `channel`, `text`, `kind`, `reply_target`; `ReplyKind`
  (`message.rs:331-343`) is `Answer | Notice | Report`. Nothing in the core's neutral vocabulary
  can express "attach a keyboard", and this unit adds nothing to it.
- **Direct chats are refused before any write.** `DirectChats` (`assembly.rs:198-206`) defaults to
  `On`; the admission returns `IngestOutcome::Disregarded` for a direct channel when the switch is
  `Off` (`assembly.rs:1206-1208`). The published position is quoted in finding 1
  (`docs/privacy/bot-assistant-privacy-policy.md:23-24`).
- **The record of processing has no category for anything this surface would deliver.** D1 through
  D9 (`docs/privacy/records-of-processing.md:61-69`) are message content, identity, circumstance,
  group facts, derived state, incidental special categories, the report record, the reply
  reference and the suppression flag. D1 states "No media, no files, no voice, no stickers." There
  is no phone number, no coordinate, no photo, no third-party identity, no second bot's
  credential. The public notice states the collection boundary in one sentence — "We take nothing
  about you from anywhere else" (`docs/privacy/bot-assistant-privacy-policy.md:50`) — and the
  legitimate-interests assessment rests its balancing on messages posted openly in a group: "This
  is not private correspondence, not observed behavior, not data collected behind someone's back"
  (`docs/privacy/lia.md:115-117`).
- **Decision 0077 removed precisely what an inline-button launch hands back.** The display name "is
  no longer collected or stored — its column was dropped with its values, and the adapter stops
  decoding it" (`docs/privacy/records-of-processing.md:62`), and the decoded `User`
  (`client.rs:217-220`) is two fields, `id` and `username`, with the comment that "The platform's
  name fields are not decoded at all, so a display name never enters the process as a typed
  value."
- **Privacy mode is off by contract, which removes `ForceReply`'s stated purpose.** The operator
  contract's first section requires it: "Before the assistant can see the group's messages, its
  platform privacy mode must be disabled"
  (`docs/reference/group-operator-contract.md:8-18`). The assistant already receives every group
  message and needs no mechanism to make a reply reach it.
- **Addressing already covers the reply case without seizing anybody's input field.** A group
  message is addressed when it mentions the bot, replies to one of the bot's messages, or names
  the validated wake trigger (`translate.rs:171-178`). `ForceReply` would only pre-set the client
  state for a reply the member can make freely.
- **The token has exactly one home.** `bot_token` is a `SecretRef` resolved at start
  (`crates/assistant/src/config.rs:539`) and handed to `BotClient`, whose module documentation
  states the no-token-in-any-string rule (`client.rs:1-9`). An `initData` check needs that same
  value as an HMAC key in whatever process serves the app, and `getManagedBotToken` would put a
  second such secret into this one.
- **No crate in the workspace can do the arithmetic.** The adapter's dependencies are
  `assistant-core`, `chrono`, `reqwest`, `serde`, `serde_json`, `thiserror`, `tokio` and `tracing`
  (`crates/adapters/telegram/Cargo.toml`). There is no HMAC, no SHA-2, no Ed25519 and no
  constant-time comparison crate, and `docs/dependency-review.md` records every crate before a
  manifest names it.
- **The repository has the right shape for a refusal scan, and two units describe it
  differently.** Unit 09's AC2 specifies "a substring scan over the `src` directories of both
  crates" for six names, living in a `tests/` directory "so it never reads itself", and states why
  it does not reuse the core's word scan: "the repository's own scanner matches whole words only
  (`vocabulary.rs:62-67`), which a needle containing an underscore can never satisfy"
  (`docs/units/telegram/09-chat-member-events.md:632-639`). Unit 08's AC3 asks for a scan "in the
  shape `crates/core/tests/vocabulary.rs:60-87` uses" over literals that mostly contain
  underscores (`docs/units/telegram/08-inline-queries.md:564-571`). Unit 09 is the one that is
  right: `carries_word` splits each line on every non-alphanumeric character and compares whole
  tokens (`crates/core/tests/vocabulary.rs:62-67`), so no needle containing `_` can ever match.
  Unit 13 reached the same conclusion independently (`docs/units/telegram/13-location-and-venue.md:449`).
  This unit joins unit 09's mechanism and reports unit 08's wording in the notes instead of editing
  another unit's document.
- **Unit 08 already builds the startup capability notice this unit extends.** Its design adds
  tolerantly-decoded Optional booleans to `BotIdentity` (`client.rs:227-239`) through a named
  deserializer that reads an unreadable value as set, and a pure
  `fn capability_notice(me: &BotIdentity) -> Option<String>` written at error level once by
  `poll_loop` (`driver.rs:298-308`) after `fetch_identity` (`driver.rs:345`) returns
  (`docs/units/telegram/08-inline-queries.md:645-663`). Two more flags fit that function with no
  new mechanism.
- **The scripted server has no way to answer capability flags yet.** `set_me(id, username)`
  (`tests/adapter/server.rs:144-147`) scripts a fixed `getMe` result built from two values
  (`server.rs:369-381`). Unit 08 introduces a raw-result setter; if this unit merges first, it
  introduces the same one.
- **Unit 15 already owns the menu button.** It sets it once to `MenuButtonCommands` with no
  `chat_id`, on the reasoning that "a `MenuButtonWebApp` set once through the platform's bot
  management interface would keep hiding the list, and this assistant has no Web App"
  (`docs/units/telegram/15-commands-menu.md:396-401`), and its AC5 asserts exactly one
  `setChatMenuButton` carrying `menu_button.type = "commands"` (`15-commands-menu.md:598`). This
  unit re-specifies none of it.
- **Unit 13 already refused the outbound half of position and contact.** It ships "no sending
  capability" on the reasoning that "The assistant has no body, no map and no phone book"
  (`docs/units/telegram/13-location-and-venue.md:23-28`). A `request_location` or
  `request_contact` button is the same refusal from the other side: instead of the assistant
  sending a coordinate, the assistant asks a member to send one.
- **The framework's byte-carrying machinery is untouched.** The attachments store with sparse byte
  ranges (`store/attachments.rs`) and the conversation fork (`store/conversations.rs`) are both
  irrelevant here: this unit moves no bytes and splits no conversation. They are named because a
  Mini App would be the first thing in this project to need the first of them.

### The ledger, personal data and bytes

Nothing on this surface is written, so the append-only rule is not exercised: no block kind, no
column, no supersession. The pins below prove the absence instead of describing it.

No document under `docs/privacy/` or `docs/compliance/` changes, because no new category is
received, no new recipient is reached and nothing new is transmitted to the model provider. The
reopening list states which documents a reversal would falsify, in the same sentence as the price.

Nothing streams because nothing moves. The constraint is recorded anyway, because the reopening
case is the interesting one: a Mini App would be the first thing here to serve bytes outward, and
`SharedUser.photo` with `WebAppUser.photo_url` would be the first inbound byte-carrying
references. Neither may ever be read whole into memory; the framework's attachments store with
its sparse byte ranges is where such bytes would belong, written chunk by chunk as they arrive and
read chunk by chunk as they leave.

## What this unit depends on and does not re-specify

- **Unit 07** owns every inline-keyboard and callback question: the offer tool, the payload, the
  addressee rule, the press refusal for a non-addressee, the `choice_selection` kind. This unit's
  only statement about it is that its inline keyboard is the answer to what a reply keyboard would
  have been for, plus one refusal it inherits — the 10.3 `force_reply` field on
  `InlineKeyboardMarkup`.
- **Unit 08** owns the `getMe` capability decode, the tolerant readers, `capability_notice`, the
  scripted raw-result setter, and the refusal scan's first list of names. This unit extends the
  notice with two flags and adds names to the scan. If unit 08 has not merged when this one
  starts, this unit builds the reader, the notice and the setter exactly as unit 08 specifies, and
  unit 08 adds its flags to the finished function. Either order leaves one reader, one notice and
  one list.
- **Unit 09** owns the refusal scan's mechanism — substring, over both `src` trees, from a
  `tests/` directory — and the whole `chat_join_request` family including
  `sendChatJoinRequestWebApp` and `supports_join_request_queries`. This unit adds names to that
  list and creates no second scan.
- **Unit 13** owns position, venue and contact as inbound facts. This unit refuses the buttons
  that would ask for them and restates none of unit 13's decoding rules.
- **Unit 15** owns `setChatMenuButton` and the commands menu. This unit adds one negative check
  beside it and changes no call.
- **Unit 05** owns polls. This unit refuses `request_poll` and says nothing about how a poll is
  sent or read.
- **Unit 03** owns editing. The platform's rule that only a message without `reply_markup` or with
  an inline keyboard can be edited is recorded here as a consequence of the reply-keyboard refusal,
  not as an editing decision.

## Decisions taken with this unit

- **The reply keyboard is refused; unit 07's inline keyboard is the only button surface the
  assistant offers, 2026-08-27.** Three independent reasons. First, attribution: a press arrives as
  an ordinary message with no marker (finding 2), so the assistant's own wording would be stored
  verbatim as the member's under decision 0017, and nothing on the ledger could separate the two —
  unit 07 solved the same product problem with a kind that "never speaks in the member's voice"
  and projects "as an act, not a quote", and a second mechanism for the same purpose with worse
  properties is the duplication this project's structure rules forbid. Second, targeting: unit 07
  binds an offer to one principal when it is appended, and `selective` has no equivalent — it
  targets whoever is @mentioned in the message text or the sender of the replied-to message, so
  the core would have to encode who may act as a property of its own prose. Third, durability: a
  keyboard "displayed until a new keyboard is sent by a bot" is state living in every member's
  client, which this process cannot read back and does not survive a restart knowing about.
  *Rejected:* a reply keyboard for the clarifying question of unit 21, the one place options are
  wanted — unit 07 already answers it, and this would be a second answer to one question.
  *Rejected:* a reply keyboard with `selective` plus a mention of the addressee — the targeting
  would then depend on the assistant writing a handle into its own answer, which makes prose the
  mechanism for who may act, while decision 0067 leaves the model free to choose how it addresses
  people. *Rejected:* recording a pressed label as a selection by matching the incoming text
  against a stored offer — two people can type the same words, a member can type a label by hand,
  and the match would attribute an act on evidence the platform explicitly does not provide.
- **`ReplyKeyboardRemove` is not sent either, and the leftover case becomes an operator-contract
  line, 2026-08-27.** A removal clears a keyboard this bot sent, and this bot sends none, so there
  is normally nothing to remove. The exception is a token that served another bot first: its
  keyboards still stand in the chats they were sent to, and no method reads them back. Unit 15 met
  the same class of problem with the menu button and solved it by setting the value once at
  startup, but that remedy does not transfer here. `reply_markup` holds exactly one object, so an
  unconditional removal on every send would make unit 07's inline keyboard impossible on the same
  message; a removal on only the first message per channel would be per-channel state in the
  adapter deciding what a message carries, which is behaviour, and behaviour does not live in an
  adapter. The honest remedy is the contract: a token that has served another bot is not reused,
  and if one is, the leftover keyboard is named as the symptom. *Rejected:* the unconditional
  removal, for the collision above. *Rejected:* the one-shot removal per channel, for the
  behaviour-in-the-adapter objection. *Rejected:* saying nothing, which leaves a real if narrow
  case with no written home.
- **`ForceReply` is refused, and so is the 10.3 `force_reply` field wherever it appears,
  2026-08-27.** Its documented purpose is receiving replies while privacy mode is on; the operator
  contract requires privacy mode off (`docs/reference/group-operator-contract.md:8-18`), so the
  assistant already receives every message in the group and the mechanism buys nothing. What it
  would still do is seize the input field of every member of the group, or — with `selective` — of
  whoever the assistant's own text happens to mention: a prompt nobody asked for, in the one part
  of the client that belongs to the member, to obtain a reply the addressing rules already
  recognise when it comes freely. The 10.3 field is the same imposition attached to unit 07's own
  inline keyboard, so it is refused by name here, before unit 07's implementation can acquire it
  quietly. *Rejected:* `ForceReply` with `selective` for a clarifying question — unit 07's inline
  keyboard answers that, and a forced input field is a heavier imposition than a button somebody
  may ignore. *Rejected:* refusing the standalone object but leaving the new field to unit 07's
  judgement, which would split one decision across two documents.
- **No button asks a member for personal data, and the refusal does not rest on the chat type,
  2026-08-27.** `request_contact`, `request_location`, `request_users`, `request_chat`,
  `request_poll` and `request_managed_bot` are all refused. The platform already makes them
  unreachable here — private chats only, and this deployment serves none — but a refusal resting
  on a configuration switch stops holding the day the switch flips, so each is refused on its own
  merits. *A phone number* is a category the record of processing does not have
  (`docs/privacy/records-of-processing.md:61-69`) and the public notice excludes (`:50`); a tap on
  a button the assistant put there is not consent under Article 4(11), which asks for a freely
  given, specific, informed and unambiguous indication, and a model-authored label in a chat
  window is none of the four. *A position* is what unit 13 refused from the other direction, and
  its reasoning holds unchanged. *Other people's identities* are the sharpest: `request_users`
  returns up to ten `SharedUser` records with first and last names, usernames and photo sizes,
  about people who never wrote to the assistant, cannot be told what happened, and have no
  conversation through which to exercise a right. The photo alone would be the first media
  reference this project ever received, against D1's "No media, no files, no voice, no stickers".
  A button that asks one member to point at another is also a way for the assistant to receive an
  accusation with no human decision point in the mechanism, which is what decision 0070 exists to
  prevent. *A chat* is finding 5: one press grants rights and admits the assistant somewhere the
  configured operator never invited it, around decision 0052. Admission would still refuse the
  chat and unit 09's membership path would leave it, so the outcome is safe; the objection is that
  offering the button invites a person to do something the admission model then has to undo. *A
  managed bot* is a second credential this process could fetch, in a project whose token
  discipline is one module and one secret reference. *Rejected:* offering `request_contact` behind
  a consent line in the assistant's own text — the consent record would be a message on a ledger
  with no withdrawal mechanism and no lawful-basis assessment of its own, and building a consent
  apparatus to acquire data nothing here uses is work in the wrong direction. *Rejected:* keeping
  `request_poll` alone on the reasoning that a poll is not personal data — the poll a member would
  be asked to author is addressed to the bot, in a private chat this deployment does not answer,
  and unit 05 has no caller for it.
- **No Mini App, and no `initData` verification is written here, 2026-08-27.** Hosting is the first
  reason: a Mini App is an HTTPS origin with assets, a certificate and a lifetime of its own, and
  this workspace binds no listening socket outside its test support. The second is what the check
  would have to be trusted with. Verifying `initData` means rebuilding a data-check-string from
  every field except `hash`, sorted alphabetically, joined by line feeds, running HMAC-SHA-256
  twice with the bot token as the inner key, comparing in constant time, and bounding `auth_date`
  — five places to be subtly wrong, in a check whose failure mode is the silent acceptance of a
  forged identity, with the bot token as the key in a process the no-token-in-any-string rule was
  not written for (`client.rs:1-9`). The third is what a launch delivers even when it is honest:
  an origin learns every visitor's network address and client, and on the inline-button route also
  the identity fields decision 0077 deliberately stopped decoding. *Rejected:* a keyboard-button
  app using `Telegram.WebApp.sendData`, on the reasoning that it "makes it possible for the bot to
  produce a response without communicating with any external servers" — the reply needs no server,
  the app itself still does, and the returned string is documented as attacker-controlled ("a bad
  client can send arbitrary data in this field"). *Rejected:* writing the verification now against
  a future app — an unused cryptographic check is an unused cryptographic check, and adding an
  HMAC crate through `docs/dependency-review.md` for code nothing calls is cost with no product.
  *Rejected:* hosting the app from the assistant process by adding an HTTP listener — a second
  network surface on the machine that holds the ledger, serving the public, is a decision about
  how the project is operated and belongs to whoever operates it, not to a unit about buttons.
- **`answerWebAppQuery`, `savePreparedInlineMessage` and `savePreparedKeyboardButton` are refused
  as one family: the assistant never sends anything under a member's name, 2026-08-27.** All three
  produce content the platform attributes to a person, not to the bot, and the first takes
  no `chat_id`, so the bot does not even choose where it appears. This is the strongest form of
  the attribution objection behind finding 2 and of decision 0070's design: an assistant that can
  post under a member's name affects that member's standing in the group with no human deciding
  anything. `savePreparedKeyboardButton` joins them because the only buttons it may store are the
  three that hand back other people's identities or a chat. *Rejected:* refusing
  `answerWebAppQuery` and leaving the two `save*` methods unmentioned — they need no update
  subscription and no keyboard, so an unmentioned method is a method the scan does not cover.
- **The refusal is checked by adding names to the substring scan unit 09 specifies, not by a
  second scan, 2026-08-27.** One decision recorded once: three units in this series now refuse an
  outbound platform affordance, and three scans reading three lists would drift apart. The names
  this unit adds are outbound-only — a program writes them when building a request, never when
  decoding one — which is why the scan can read both `src` trees with no exceptions:
  `ReplyKeyboardMarkup`, `ReplyKeyboardRemove`, `remove_keyboard`, `ForceReply`, `force_reply`,
  `request_contact`, `request_location`, `request_users`, `request_chat`, `request_poll`,
  `request_managed_bot`, `WebAppInfo`, `"web_app"`, `MenuButtonWebApp`, `answerWebAppQuery`,
  `savePreparedKeyboardButton` and `getManagedBotToken`. The `"web_app"` entry carries its JSON
  quotation marks on purpose: a bare `web_app` is a substring of the inbound `web_app_data`, which
  this unit's own fixtures must write, while the quoted form appears only in an object the adapter
  sends. *Rejected:* a whole-word scan in the shape of `crates/core/tests/vocabulary.rs` —
  `carries_word` splits on every non-alphanumeric character (`vocabulary.rs:62-67`), so a needle
  containing an underscore can never match and eight of the seventeen entries would be vacuous.
  *Rejected:* forbidding the four inbound names `users_shared`, `chat_shared`, `web_app_data` and
  `managed_bot_created` — the fail-safe pins must write them, and a scan that excludes its own
  test tree to accommodate them is a weaker scan for no gain. *Rejected:* forbidding
  `setChatMenuButton`, which unit 15 calls legitimately; `MenuButtonWebApp` and `"web_app"` cover
  the case that matters, which is a menu button built to launch an app.
- **Two more `getMe` flags join unit 08's startup notice, and the notice stays advisory,
  2026-08-27.** `has_main_web_app` and `can_manage_bots` are the only settings on this surface any
  code here can observe: the main Mini App, the attachment menu and an app-launching menu button
  are all configured in the platform's bot management interface, outside every file in this
  repository. A token whose main Mini App was switched on elsewhere carries a Launch button on the
  assistant's public profile and answers a direct link for anybody, in any chat, and nothing in
  this process would otherwise know. The two flags read through unit 08's tolerant reader, an
  unreadable value counts as set for the same reason it does there, and the text names the
  platform's bot management interface as the remedy, since no API method reverses either. The
  process keeps running: an adapter that refused to start over a profile button would take the
  community's assistant offline for a setting that, with nothing on this surface sent or decoded,
  leaks nothing through this code. *Rejected:* refusing to start, per unit 08's own reasoning
  about outages. *Rejected:* calling `getChatMenuButton` at startup to read the default menu
  button back — unit 15 sets that value unconditionally on every start, so a read-back would
  report a state this process is about to overwrite, and it cannot see the per-chat buttons that
  would shadow it anyway. *Rejected:* decoding `can_connect_to_business`, `has_topics_enabled` and
  `allows_users_to_create_topics` while the decode is open — they belong to features other units
  own, and folding them in here would hide other people's decisions inside this one.
- **Four service-message shapes are pinned as skips, and the pin proves the payload never enters
  the process, 2026-08-27.** Unlike unit 08's inline updates, these can arrive today: they come
  inside `message`, which the poll already subscribes to (`client.rs:103`). A `web_app_data`
  message reaches the assistant if this token ever had a Mini App and a member relaunches an old
  keyboard; `users_shared`, `chat_shared` and `managed_bot_created` reach it the same way. Today
  all four exit at `Skip::NoText` or `Skip::NoSender` and nothing is written — correct behaviour
  that nothing proves. The pin is structural, in the shape unit 13 adopted after its own draft's
  scan proved unsatisfiable: a scripted update carrying a sentinel inside the service payload is
  acknowledged, advances the offset, causes no outbound request, adds no row to any table, and the
  sentinel appears nowhere in the process's tracing output — because `Incoming`
  (`client.rs:125-144`) has no field that could hold it. *Rejected:* adding the four fields to
  `Incoming` with named `Skip` reasons, the way `edited_message` earns one — that would decode a
  member's phone number, another person's name or an app's arbitrary string into process memory in
  order to discard it, which is the opposite of what the pin exists to show. *Rejected:* pinning
  only `web_app_data` and letting one shape stand for four — they arrive by different routes and
  carry different data, and a pin that covers one is evidence about one.
- **The `managed_bot` update type is refused by containment, not by a new decode, 2026-08-27.** It
  is the only top-level update this surface produces, and it is not in `CONSUMED_UPDATE_TYPES`
  today. The assertion is a containment check on the `allowed_updates` array as sent on the wire,
  written so it survives whatever sibling units add to the list, in the shape unit 08 uses for its
  own two update types. *Rejected:* an equality assertion on the constant, which unit 09 already
  owns and which would collide with every unit that extends the list. *Rejected:* saying nothing,
  since an unsubscribed update type is a setting on the token that any earlier configuration could
  have changed.
- **The menu button stays unit 15's, and this unit adds only the negative check, 2026-08-27.**
  Unit 15 sets `MenuButtonCommands` once at startup and asserts the type on the wire. A second
  assertion about the same call would be a second place deciding one thing. What this unit adds is
  the `MenuButtonWebApp` and `"web_app"` entries in the refusal scan, which cover the case unit
  15's assertion cannot: source that builds an app-launching button somewhere else. *Rejected:*
  widening unit 15's AC5 from this unit, which would mean editing another unit's document.
- **No privacy or compliance document changes, and the reversal list is written down instead,
  2026-08-27.** Nothing new is received, stored, transmitted to the model provider or reached to:
  the assistant emits none of these affordances, decodes none of these payloads and gains no data
  category. None of the record's review triggers fires. An amendment saying that a thing was
  considered and not done would put a non-event into a register of processing activities. The
  reopening section replaces it, naming every clause a reversal would falsify. *Rejected:* a note
  in the record for completeness, per unit 08's reasoning that padding a register with refusals
  makes the real entries harder to audit.

## What would have to be true before a Mini App is reopened

Refusing without naming what could work is refusing without examining. The one shape that could
conceivably be defensible is narrow: a read-only page listing the project's wiki topics, opened
from a menu button, sending nothing back — no `sendData`, no `answerWebAppQuery`, no ledger write,
identical content for everybody. Even that is blocked today, and the checklist is the useful part.

1. **Somebody has to own an origin.** An HTTPS host, a certificate with a renewal, static assets
   and a deployment lifetime, all outside this repository and outside the assistant process. That
   is a decision about how the project is operated, and it comes before any code here changes. The
   10.2 origin protection and its opt-out are part of that decision, not a detail under it.
2. **The launch hands the origin something even when the page ignores it.** Every launch is an
   HTTPS request, so the origin learns the member's network address and client. On the
   inline-button route it also receives the identity fields decision 0077 stopped decoding, and on
   the attachment-menu route a profile photo URL. The origin becomes a recipient of personal data
   whether the page reads the payload or not, and the record of processing gains one.
3. **`initData` verification would have to be reviewed as cryptography, not as parsing.** Sorted
   fields, `hash` excluded, line-feed separators, the double HMAC with the token as inner key, a
   constant-time comparison, and an `auth_date` bound with a stated number. The Ed25519 path exists
   for parties who should not hold the token and is the better shape if the origin is ever a
   separate process — it needs the `<bot_id>:WebAppData` prefix, both `hash` and `signature`
   excluded from the string, and a check against the published production key. Whichever path is
   chosen, the dependency passes `docs/dependency-review.md` first.
4. **Four documents change before the code merges, not after.** The record of processing gains a
   recipient and, if any launch datum is read, a category. The impact assessment gains an addendum
   for a surface reachable outside the group. The legitimate-interests assessment's balancing rests
   on messages posted openly in a community group and would need rewriting for a web surface. And
   the public notice, the document a member actually reads, is falsified in one line: "We take
   nothing about you from anywhere else"
   (`docs/privacy/bot-assistant-privacy-policy.md:50`) stops being true the moment a launch hands
   an origin this project runs anything about the person launching it.
5. **The private-chat problem stays for every route this code could set.** A keyboard-button app,
   an inline-button app and a menu-button app are all private-chat or default, so reaching them
   means reopening decision 0069 and shipping the direct-chat feature set that switch waits on. The
   direct-link route escapes that, and escapes this repository too: it is configured in the
   platform's bot management interface, which is why the startup notice reports it instead of
   trying to prevent it.

Nothing above defers the decision into legitimacy. The decision is no, today, on the reasoning in
the previous section. This list exists so that a future yes has to pay the price in the open.

## The unit's contract

After this unit the repository's answer to "can the assistant replace a member's keyboard, ask a
member for a phone number or a position, ask a member to point at another person, take a chat with
rights from a tap, obtain a second bot's token, launch a web app, or post under a member's name" is
a recorded no with its reasoning, and the no is a checkable property of the source instead of an
absence nobody maintains. One substring scan over both crates' `src` trees, the one unit 09
specifies, fails with `file:line` on any of `ReplyKeyboardMarkup`, `ReplyKeyboardRemove`,
`remove_keyboard`, `ForceReply`, `force_reply`, `request_contact`, `request_location`,
`request_users`, `request_chat`, `request_poll`, `request_managed_bot`, `WebAppInfo`,
`"web_app"`, `MenuButtonWebApp`, `answerWebAppQuery`, `savePreparedKeyboardButton` or
`getManagedBotToken`. The four service messages this surface produces — `users_shared`,
`chat_shared`, `web_app_data` and `managed_bot_created` — are proven to be acknowledged and
skipped whether or not the platform names a sender, with no outbound request, no stored row and no
appearance of their payload in the process's tracing output, because the adapter's incoming model
has no field that could hold one. The poll's `allowed_updates` array is proven not to contain
`managed_bot`. The `getMe` capability notice gains `has_main_web_app` and `can_manage_bots`, read
through the same tolerant reader and reported by the same pure function unit 08 introduces, naming
the platform's bot management interface as the remedy since no API method reverses either; a token
with either flag set still starts, polls and answers exactly as before. No outbound `sendMessage`
carries a `reply_markup` that is anything but an inline keyboard. The core is untouched: no new
entry point, no new vocabulary, no new kind, no new table, and `docs/platform-vocabulary.txt` is
unchanged, because nothing in the core learned a platform word. Unit 15's `setChatMenuButton` call
is unchanged. No privacy or compliance document changes, because nothing new is received, stored or
reached. No new dependency, no new configuration entry, and no change to anything a member can
observe.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes — `AnsweringMode::Helpful` and
  `AnsweringMode::Addressed`; clippy, fmt and doc under denied warnings; the platform-vocabulary
  scan and the secret scan clean; no new dependency and no new configuration entry; the diff
  touches no file under `crates/core/src/`.
- **AC2** The refusal scan carries this unit's seventeen names and matches by case-sensitive
  substring over the `src` directories of both crates, from a file in a `tests/` directory so it
  never reads itself. A negative check accompanies it, proving the scan can fail: a needle
  assembled at run time from parts, matching a string that does exist in a scanned file, produces a
  finding with the right `file:line`. If unit 09 has merged, its list file gains the entries and no
  second scan is created; if it has not, this unit creates the scan in unit 09's specified shape
  and unit 09's six names join it later.
- **AC3** The `"web_app"` entry is proven to discriminate: a test asserts directly on the matcher
  that the needle does not match the literal `"web_app_data"`, so the inbound name stays writable
  in fixtures while the outbound key stays forbidden. Asserted on the matcher, not inferred from
  the scan passing.
- **AC4** A `web_app_data` message delivered anyway is acknowledged and ignored. With the scripted
  server pushing an update carrying an `update_id` and a `message` whose `chat`, `date` and
  `message_id` are well formed and whose only payload is a `web_app_data` object holding a distinct
  sentinel in `data` and in `button_text`, the adapter acknowledges it, the next poll's offset is
  past it, and the recorded requests for that batch contain no `sendMessage`, no `getChat` and no
  `getChatAdministrators`. Scripted twice, once with `from` present and once absent, because the
  documentation does not state which shape the platform sends.
- **AC5** The same holds for `users_shared`: an update whose message's only payload is a
  `users_shared` object with a `request_id` and a `users` array carrying a `user_id`, a
  `first_name`, a `last_name`, a `username` and a `photo` array, each a distinct sentinel, is
  acknowledged with no request of any kind, in both sender shapes.
- **AC6** The same holds for `chat_shared`: an update whose message's only payload is a
  `chat_shared` object with a `request_id`, a `chat_id`, a `title`, a `username` and a `photo`
  array of sentinels is acknowledged with no request of any kind, in both sender shapes.
- **AC7** The same holds for `managed_bot_created`: an update whose message's only payload is a
  `managed_bot_created` object carrying a `bot` with an `id`, a `first_name` and a `username` of
  sentinels is acknowledged with no request of any kind, in both sender shapes.
- **AC8** Nothing from AC4 through AC7 reaches storage: the store holds no new block, no new
  conversation, no new channel mapping and no new principal row after all four, asserted by
  counting rows through the fixture's store before and after.
- **AC9** No sentinel from AC4 through AC7 appears in the process's tracing output for those
  batches. Captured through the suite's existing tracing capture where one exists, and otherwise
  asserted structurally: a test builds each service payload as JSON, decodes it into `Incoming`,
  and asserts every decoded field equals exactly what the well-formed envelope carried — proving
  the payload has no landing place in the type instead of proving a log line's absence. The
  criterion states which of the two forms was used.
- **AC10** The `allowed_updates` array sent on the wire contains no `"managed_bot"` element,
  asserted as a containment check beside the existing wire assertion so it survives whatever
  sibling units add to the list, with the assertion's message naming this unit's decision.
- **AC11** The capability reading covers the two new flags: the pure notice function returns `None`
  when neither is set; a text containing `has_main_web_app` when only that is set; a text
  containing `can_manage_bots` and not `has_main_web_app` when only that is set; and a text naming
  both when both are. `None` and `Some(false)` are pinned as producing the same result for each
  flag. No returned text contains the token, per the client module's no-token-in-any-string rule
  (`client.rs:1-9`). Neither text names an API method as the remedy, because none exists; both name
  the platform's bot management interface.
- **AC12** The two new flags decode tolerantly, in the same test module and through the same reader
  unit 08 specifies: a `getMe` result omitting both, one carrying `null`, one carrying `false`, one
  carrying `true`, and ones carrying a string, a number and an object all decode into a
  `BotIdentity` without an error, and the wrong-typed cases read as `Some(true)`, with the
  assertion's message stating why an unreadable flag counts as set.
- **AC13** A token with `has_main_web_app` set does not stop the adapter: with the scripted `getMe`
  answering it true, the adapter completes startup, polls, ingests a group message and answers it
  exactly as with the flag absent. Pinned, because the decision that a misconfiguration is reported
  and not enforced is the one a future reader is likeliest to reverse by accident.
- **AC14** No outbound request built anywhere in the adapter carries a `reply_markup` of any type
  other than an inline keyboard: a test drives one ordinary answer, one failure notice and one
  report through the scripted server and asserts that every recorded `sendMessage` body either has
  no `reply_markup` key or has one whose only key is `inline_keyboard`. This is the behavioural
  half of AC2, and it keeps holding after unit 07 merges its own keyboard.
- **AC15** The decision is recorded and the operator is told: a file in `docs/decisions/` records
  this unit's refusals with their date and rejected alternatives, and
  `docs/reference/group-operator-contract.md` gains a section stating that the assistant offers no
  reply keyboard and no web app, that the token must have no main Mini App and no attachment-menu
  app configured in the platform's bot management interface, that a token which previously served
  another bot is not reused because a reply keyboard it sent may still stand in chats this process
  cannot read, and what the startup notice means when it appears.
- **AC16** No file under `docs/privacy/` or `docs/compliance/` is modified by this unit's diff, and
  the reopening section names the record of processing, the impact assessment, the
  legitimate-interests assessment and the public privacy notice explicitly, so a future unit cannot
  claim the documents were never mentioned.
- **AC17** The one unproven platform inference is marked as unproven here and nothing merged
  depends on it, in the shape unit 06's AC13 uses. The inference: that `getMe` omits
  `has_main_web_app` and `can_manage_bots`, or answers them false, on a token where no main Mini
  App and no bot management were ever configured. The API states only "True, if the bot has a main
  Web App", never what a bot without one returns, and the adapter suite has no live-endpoint path.
  Nothing depends on it: a token that never reports a flag produces no line, the same outcome as
  the setting being off. The named post-merge check is one `getMe` against the real token, read
  once by whoever deploys, with the result written into the operator contract.

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. The diff is small on purpose.
- Adapter sites, all under `crates/adapters/telegram/`:
  - `BotIdentity` (`src/client.rs:227-239`) gains `has_main_web_app: Option<bool>` and
    `can_manage_bots: Option<bool>`, each with the same `#[serde(default, deserialize_with =
    "...")]` pair unit 08's two flags use.
  - `capability_notice` gains the two flags' text. If unit 08 has not merged, this unit writes the
    tolerant reader, the function and `poll_loop`'s single error-level call
    (`src/driver.rs:298-308`, after `fetch_identity` at `:345`) exactly as unit 08 specifies, so
    that unit adds its flags to a finished function.
  - The scripted server gains a raw-result setter for `getMe` beside `set_me`
    (`tests/adapter/server.rs:144-147`), unless unit 08 has already added one, in which case this
    unit uses it.
  - The four scripted-update pins (AC4–AC7) and their store assertions (AC8) go in
    `tests/adapter/offset.rs`, which already exercises acknowledgement and the offset contract and
    already reads the store directly. AC9's structural form, if used, goes in the new `client.rs`
    test module beside the decode pins.
  - AC10 goes beside the existing `allowed_updates` wire assertion in
    `tests/adapter/group_context.rs`, without editing it.
  - AC12 in the `#[cfg(test)] mod tests` at the end of `src/client.rs` that unit 08 creates.
  - AC13 and AC14 in `tests/adapter/end_to_end.rs`, beside the existing send assertions.
  - AC2 and AC3 in whichever `tests/` file holds the refusal scan when this unit merges.
- **A hazard the implementer will hit.** The scan reads both `src` trees, so no source comment may
  quote a forbidden literal: a comment in `client.rs` explaining why no keyboard is built must
  reference this unit's decision number and describe the refused fields in prose, never write
  `ReplyKeyboardMarkup` or `request_contact` verbatim. The literals live only in the test list, and
  the reasoning lives only in the decision file.
- Documentation sites: one decision file continuing the numbering after whatever is unclaimed at
  merge time — the highest number on `main` today is
  `0105-the-fixed-line-is-the-acknowledgments-fallback.md`, and units 07 and 08 both reserve
  numbers above it, so this unit's number is taken at merge and not fixed here. One new section in
  `docs/reference/group-operator-contract.md` per AC15. No change to `docs/follow-ups.md`, because
  this is decided, not deferred.
- **A defect in a sibling spec, reported and not acted on.** Unit 08's AC3
  (`docs/units/telegram/08-inline-queries.md:564-571`) asks for a scan "in the shape
  `crates/core/tests/vocabulary.rs:60-87` uses" over literals including `switch_inline_query`,
  `switch_inline_query_current_chat`, `answerInlineQuery` and `savePreparedInlineMessage`. That
  shape cannot match the underscored ones: `carries_word` (`crates/core/tests/vocabulary.rs:62-67`)
  splits each line on every non-alphanumeric character and compares whole tokens, so a needle
  containing an underscore matches nothing, and a camel-cased needle matches only where it stands
  alone between punctuation. Unit 09's AC2 reached the correct conclusion and specifies a substring
  scan; unit 13 corrected the same mistake in its own draft. Whoever implements unit 08 should
  write a substring scan. Nothing here depends on the correction, because this unit joins unit 09's
  mechanism either way. Not edited, because it is another unit's document.
- **Sibling collisions, stated and not acted on.** This unit adds nothing to
  `CONSUMED_UPDATE_TYPES` and asserts only non-containment of one name, so it collides with none of
  the units that extend the list. It adds no `setChatMenuButton` call, so it does not collide with
  unit 15's AC5. It adds two fields to `BotIdentity` and two branches to `capability_notice`, both
  of which unit 08 also touches: whichever merges second adds its fields to the other's finished
  code, and the tolerant reader is written once.
- One thing to watch after merge: if the operator ever configures a main Mini App for an unrelated
  reason, the startup line appears and stays until the setting is reversed in the platform's bot
  management interface. That is the intended outcome, and the line says what to do, not
  merely what the state is.
