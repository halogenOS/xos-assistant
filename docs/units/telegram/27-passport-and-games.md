# Telegram unit 27 — Passport and games: both refused, and one of them is not refused today

Date: 2026-08-27. Two of the platform's older surfaces, examined together because both end in
a no and because one of them is a hole in this repository right now. Telegram Passport lets a
person hand a bot their identity documents — passport, driving licence, identity card,
residential address, a selfie holding the document, a bank statement — encrypted to a key the
bot owns. Games let a bot send an HTML5 game into a chat, set scores for named people, and read
back a high-score table that names other people.

The two refusals are not equal in urgency. Games arrive nowhere near this assistant: a game
must be created with `/newgame` in BotFather and the terms accepted per game, and this token has
no game, so `sendGame` would fail even if a line of ours called it. Passport is different, and
it is the reason this document exists. **Passport data rides the `message` update type, which
this adapter subscribes to.** There is no subscription to withhold, no update type to leave out
of `allowed_updates`, and no capability flag on `getMe` to read. The only thing standing between
this process and a set of encrypted identity documents is that `Incoming` does not decode the
field and that a message with no text is skipped for an unrelated reason — a rule about text
that unit 01 is in the middle of rewriting.

So this unit does two different things. For games it records the decision and pins the outcome
by test, the shape unit 24 uses. For Passport it also adds a named refusal to the translation,
checked before every other condition, so that the refusal stops depending on a rule that was
never about documents.

## The findings, stated at the top

### 1. What happens today when a Passport message arrives, exactly

An unhandled update is a behaviour, not an absence. The behaviour today, traced through the
code:

1. The person's client sends the encrypted payload; the platform delivers it as a `message`
   update whose `Message` carries `passport_data` and no text. That it is a service message is
   visible on the protocol side: MTProto delivers it as `messageService` carrying
   `messageActionSecureValuesSentMe`.
2. `get_updates` reads the whole response body through `response.json()` (`client.rs:671-681`),
   so the ciphertext does pass through this process's memory as bytes of an HTTP body. That is
   true and is stated here, not glossed.
3. `Update` and `Incoming` decode a fixed, small set of fields and "Unknown fields are ignored
   by the decoder" (`client.rs:172-174`, `client.rs:192-211`). Neither struct names
   `passport_data`, so nothing of the payload survives into a value this code holds. Nothing is
   base64-decoded, nothing is written, and it could not be decrypted in any case: decryption
   needs an RSA private key this deployment has never generated.
4. `translate` reaches the text rule and returns `Translation::Skip(Skip::NoText)`
   (`translate.rs:167-168`), whose own doc comment says it means "A message with neither text
   nor caption (decision 0017)". If the platform were to omit `from` on the service message, it
   would be `Skip::NoSender` (`translate.rs:164-165`) instead. Either way it is skipped.
5. `process` writes one debug line carrying the update id and the skip reason and returns
   `Step::Acknowledged` (`driver.rs:368-370`); the offset advances past it (`driver.rs:321`).
   The reason string is "a message with neither text nor caption" (`translate.rs:502`), so no
   part of the payload reaches the log.

The outcome is right. The reason is wrong, and that is the finding. Nothing in the tree says
"this project does not accept identity documents"; what it says is "this project records text".
Unit 01 rewrites exactly that rule: `Skip::NoText` becomes `Skip::NothingToRecord` and a message
that carries no text is recorded anyway when it carries a file the adapter carries
(`docs/units/telegram/01-receiving-media.md:249`, `:602`). Unit 01's list of carried files is an
enumeration — photo, video, animation, document, audio, voice note, video note, live photo — and
Passport files sit inside `passport_data`, not on the message, so unit 01 as specified does not
reach them. The hazard is the next widening after that one, written by somebody who reads the
skip's name and concludes that anything with bytes in it is fair to fetch. A `PassportFile` has
a `file_id` and `getFile` downloads it like any other file; the difference is only that the
bytes are ciphertext and that they are somebody's passport.

### 2. Passport data has no lawful path through this system, and one reason is a statute

Four things, each sufficient on its own.

**The private key would have to exist.** Receiving Passport data at all requires generating an
RSA key pair and registering the public half with `/setpublickey` in BotFather, plus a privacy
policy link set with `/setprivacypolicy`. The private key would then have to live on the machine
that runs the assistant, next to the bot token, and its compromise would retroactively expose
every document ever sent. This project keeps one secret today and the operator contract already
says how. Adding a second secret whose loss is worse than the first, to serve no purpose, is not
a trade anybody has proposed.

**German law forbids passing on a copy of the identity card.** § 20(2) of the
Personalausweisgesetz (read at gesetze-im-internet.de on 2026-08-27) allows a copy only by the
holder or with the holder's consent, requires the copy to be "eindeutig und dauerhaft als Kopie
erkennbar", and then states: "Andere Personen als der Ausweisinhaber dürfen die Kopie nicht an
Dritte weitergeben." This assistant's whole answering path sends what it holds to a model
provider outside the European Union. A decrypted identity-card scan reaching that provider is
precisely the passing-on the section prohibits, and no configuration of this repository puts a
category of stored content outside the model's reach today.

**The published documents describe a different activity, and this one has no Article 9
condition.** The record of processing lists three purposes, all on Article 6(1)(f), and its data
categories reach special categories only "incidentally", with the reasoning that what a person
posts about themselves in a publicly readable group is covered by Article 9(2)(e)
(`docs/privacy/records-of-processing.md:66`). The legitimate-interests assessment rests on
"Messages people chose to post to an open community group ... not private correspondence, not
observed behavior, not data collected behind someone's back, and not data enriched from other
sources" (`docs/privacy/lia.md:117-119`). A document deliberately handed over in a private chat
is none of those, and Article 9(2)(e) cannot reach it, since the person published nothing. The
only condition that could fit is explicit consent under Article 9(2)(a), which the record says
in terms is not used anywhere in this activity. The impact assessment would move too: its
criterion 4 is currently met by a capability not in use (`docs/privacy/dpia.md:70-73`), and a
document intake would put it into use.

Two points of precision, because overstating this is as bad as understating it. Identity
documents are not automatically Article 9 data: a name, a date of birth, a document number and
an address are ordinary personal data, held to a high standard by their sensitivity, not
by Article 9. What crosses into Article 9 is narrower and still present — `PersonalDetails`
carries `country_code`, "Citizenship (ISO 3166-1 alpha-2 country code)", which can reveal
ethnic origin, and the selfie is a facial image that becomes biometric data once processed for
unique identification. The honest statement is that the set is highly sensitive throughout and
special-category in parts, and that neither part has a condition here.

**`setPassportDataErrors` acts on a person.** It is the only Passport method in the Bot API, and
what it does is stated plainly: "The user will not be able to re-submit their Passport to you
until the errors are fixed". A bot deciding that somebody's document is unacceptable, and
blocking their next attempt, is an effect on a person with no human in the mechanism, which
decision 0070 settles against.

### 3. Games put people's names on a table this project cannot see and cannot erase

`getGameHighScores` returns an array of `GameHighScore`, each carrying `position`, a full `User`
and `score`. The method's own note says it returns "scores for the target user, plus two of
their closest neighbors on each side" and "the top three users if the user and their neighbors
are not among them". So one call returns identifying rows for up to eight people the assistant
never spoke to, fetched from the platform's servers. The public privacy notice tells members "We
take nothing about you from anywhere else"
(`docs/privacy/bot-assistant-privacy-policy.md:50`). That sentence stops being true on the day a
high-score fetch merges.

Worse, the table is not ours to delete. Scores live on the platform, per chat — `chat_instance`
is documented as "Useful for high scores in games" — and the API offers no method that removes a
score. `setGameScore` can lower one with `force`, whose stated purpose is "fixing mistakes or
banning cheaters", and `score` "must be non-negative", so the floor is a zero-score row that
still names the person. Erasure in this project nulls the personal columns of a person's
messages and removes their direct conversations (`erasure.rs:1-40`), and the notice promises
"ask, and it goes" with exactly one named exception
(`docs/privacy/bot-assistant-privacy-policy.md:108-118`). A game score would be the second
exception, in a place erasure cannot reach at all, created by us and keyed to a person by
numeric id.

Two smaller findings sit beside that one. First, a game message's content changes without us:
"A game message will also display high scores for the current chat", and `setGameScore` edits
the message to show the current scoreboard unless `disable_edit_message` is passed. So a block
this repository appended for its own outbound message would describe a message whose visible
content the platform rewrites afterwards, with no update telling us. The ledger is append-only
and a superseding fact would have to come from somewhere; there is nowhere for it to come from.
Unit 03 owns the edits this assistant performs, and this is not one of them. Second, a game is a
play surface, and the impact assessment already records that children are among the people in
these groups because the platform applies no age check (`docs/privacy/dpia.md:77-79`). A
scoreboard that names them is not the feature to add on that basis.

### 4. Neither refusal is visible from outside the source

`getMe` reports `supports_inline_queries`, `supports_guest_queries`, `can_connect_to_business`,
`has_main_web_app` and several more, and none of them is about games or Passport. So the startup
notice unit 08 specifies has nothing to read here, and unlike unit 08 there is no wire assertion
to make either, because neither surface has an update type of its own. What is left is a source
scan for the outbound methods, the operator contract for the two BotFather commands, and — for
Passport alone — a named refusal in the translation. Saying which check does not exist matters as
much as building the ones that do.

## Grounding

### The platform, read 2026-08-27

Fetched from `core.telegram.org/bots/api`, `core.telegram.org/passport`,
`core.telegram.org/api/passport`, `core.telegram.org/bots/games` and the changelog at
`core.telegram.org/bots/api-changelog` on 27 August 2026. Every sentence in quotation marks was
read from those pages on that date. The brief for this series named Bot API 10.1 (11 June 2026)
as current; the changelog's newest entry is **Bot API 10.3, dated 24 August 2026**, the same
correction units 08 and 24 recorded.

**Passport: how it arrives.**

- **There is no Passport update type.** `Update` is documented "At most one of the optional
  fields can be present in any given update" and its field list has no Passport entry. The
  payload rides `Message.passport_data`, described only as "Optional. Telegram Passport data",
  on the `message` type this adapter consumes (`client.rs:170`).
- **The manual states the delivery.** "When the user confirms your request by pressing the
  'Authorize' button, the Bot API sends an Update with the field passport_data to the bot that
  contains encrypted Telegram Passport data." The MTProto page describes the same event as an
  `updateNewMessage` carrying a `messageService` with `messageActionSecureValuesSentMe` — a
  service message, which is why it carries no text.
- **`PassportData`** is `data` ("Array with information about documents and other Telegram
  Passport elements that was shared with the bot") and `credentials`, an `EncryptedCredentials`.
- **`EncryptedPassportElement.type`** is one of "personal_details", "passport",
  "driver_license", "identity_card", "internal_passport", "address", "utility_bill",
  "bank_statement", "rental_agreement", "passport_registration", "temporary_registration",
  "phone_number", "email". Beside `type` it carries `data` (base64, encrypted), `phone_number`
  ("User's verified phone number"), `email` ("User's verified email address"), `files`,
  `front_side`, `reverse_side`, `selfie` ("Encrypted file with the selfie of the user holding a
  document"), `translation` and `hash`. **`phone_number` and `email` are plaintext**, not
  encrypted, which the manual confirms with its `securePlainPhone` and `securePlainEmail`
  constructors.
- **What is inside the encrypted parts**, from the manual's own field tables: `PersonalDetails`
  is `first_name`, `last_name`, `middle_name`, `birth_date` ("Date of birth in DD.MM.YYYY
  format"), `gender` ("male or female"), `country_code` ("Citizenship (ISO 3166-1 alpha-2
  country code)"), `residence_country_code` and the three native-language name fields.
  `ResidentialAddress` is `street_line1`, `street_line2`, `city`, `state`, `country_code` and
  `post_code`. `IdDocumentData` is a document number and an optional expiry date.
- **`PassportFile`**: "Currently all Telegram Passport files are in JPEG format when decrypted
  and don't exceed 10MB", with `file_id`, `file_unique_id`, `file_size` and `file_date`. The
  manual adds that they are downloaded through `getFile`, whose own ceiling is "The maximum file
  size to download is 20 MB" — so every Passport file is fetchable by an ordinary bot call.
- **The bot must hold a private key, and the setup is entirely in BotFather.** "To request data
  from Telegram Passport users, your bot will need to generate a pair of encryption keys",
  `openssl genrsa 2048 > private.key`, "WARNING: Keep your private key SECRET!", and "Use the
  /setpublickey command with @BotFather to connect this public key with your bot." A privacy
  policy link is set with `/setprivacypolicy`: "Users will see this link when offered to
  authorize you to access their data."
- **Decryption is authenticated at the end, not as it goes.** For files: "Download the encrypted
  file using the getFile method", then "Use AES256-CBC with this file_key and file_iv to decrypt
  the content of the file. IMPORTANT: At this step, make sure that file_hash from the credentials
  is equal to SHA256( file_content )", and "The content of the file is padded with 32 to 255
  random padding bytes ... The first byte contains the length of the padding (including that
  byte)." The credentials themselves carry a nonce, with "IMPORTANT: Make sure that the nonce is
  the same as was passed in the request."
- **`setPassportDataErrors(user_id, errors)`** returns True. "Informs a user that some of the
  Telegram Passport elements they provided contains errors. **The user will not be able to
  re-submit their Passport to you until the errors are fixed** (the contents of the field for
  which you returned the error must change)." Nine error types exist, from
  `PassportElementErrorDataField` to `PassportElementErrorUnspecified`, each keyed by a base64
  hash of the thing being rejected.
- **Passport has not changed in eight years.** The changelog's last Passport entry is Bot API
  4.1, 27 August 2018, adding translations and three error types; Bot API 4.0 on 26 July 2018
  introduced it. Recorded as a fact about the platform's attention to the surface, not as a
  claim that it is withdrawn.

**Games: how they work.**

- **A game must be created in BotFather, per game, with terms accepted.** "Create games via
  @BotFather using the /newgame command. Please note that this kind of power requires
  responsibility: you will need to accept the terms for each game that your bots will be
  offering." The gaming platform page repeats it and adds that the bot supplies an HTML5 page:
  "You provide the correct URL for this particular user and the app automatically opens the game
  in the in-app browser."
- **`sendGame(business_connection_id, chat_id, message_thread_id, game_short_name,
  disable_notification, protect_content, allow_paid_broadcast, message_effect_id,
  reply_parameters, reply_markup)`** returns the sent `Message`. `chat_id` carries a platform
  constraint: "Games can't be sent to channel direct messages chats and channel chats."
  `reply_markup`: "If empty, one 'Play game_title' button will be shown. If not empty, the first
  button must launch the game."
- **`Game`** is `title`, `description`, `photo` (an array of `PhotoSize`), an optional `text`
  ("Brief description of the game or high scores included in the game message ... 0-4096
  characters"), `text_entities` and an optional `animation`. Every one of those sits inside the
  `Game` object; `Message.game` is the only field the message itself gains, and `Message.photo`
  stays absent. That is why unit 01's enumeration of carried files does not reach a game
  message.
- **`CallbackGame`** is "A placeholder, currently holds no information."
  `InlineKeyboardButton.callback_game` is "Description of the game that will be launched when
  the user presses the button. NOTE: This type of button must always be the first button in the
  first row." A press produces a `CallbackQuery` with `game_short_name` and no `data`, since
  "Exactly one of the fields data or game_short_name will be present". `answerCallbackQuery`'s
  `url` parameter is the game's door: "If you have created a Game and accepted the conditions
  via @BotFather, specify the URL that opens your game - note that this will only work if the
  query comes from a callback_game button."
- **`setGameScore(user_id, score, force, disable_edit_message, chat_id, message_id,
  inline_message_id)`** returns the `Message` or True. "Returns an error, if the new score is not
  greater than the user's current score in the chat and force is False." `score`: "New score,
  must be non-negative." `force`: "Pass True if the high score is allowed to decrease. This can
  be useful when fixing mistakes or banning cheaters." `disable_edit_message`: "Pass True if the
  game message should not be automatically edited to include the current scoreboard."
- **`getGameHighScores(user_id, chat_id, message_id, inline_message_id)`** returns "an Array of
  GameHighScore objects", each `position`, `user` (a full `User`) and `score`. "This method will
  currently return scores for the target user, plus two of their closest neighbors on each side.
  Will also return the top three users if the user and their neighbors are not among them.
  Please note that this behavior is subject to change."
- **Scores are stored by the platform and shown in the chat.** "A game message will also display
  high scores for the current chat", and the gaming platform page adds "When a new high score is
  set, a service message will be sent to the chat and the message with the current scoreboard
  will be updated." No method removes a score.
- **There is no Message field for that service message.** The whole `Message` object was read for
  one and it carries `game` and `passport_data` and nothing score-shaped. What such a message
  would look like on the wire is therefore not established; see the unproven inferences below.
- **`InlineQueryResultGame`** exists (`type`, `id`, `game_short_name`, `reply_markup`), which is a
  second door into games through inline mode. Unit 08 already refuses inline mode, so that door
  is closed twice over.
- **Administrators can only block games wholesale.** `ChatPermissions` and
  `ChatMemberRestricted` carry `can_send_other_messages`, "True, if the user is allowed to send
  animations, games, stickers and use inline bots". A group that wants no games loses stickers
  and inline bots with them, exactly as unit 08 recorded for inline.
- **Games are maintained but not developed.** The last game-specific changelog entry is Bot API
  2.3, December 2016, which added `force` and `disable_edit_message`; since then `sendGame` has
  only collected the parameters every send method collects — `message_thread_id`,
  `reply_parameters`, `business_connection_id`, `message_effect_id`, `allow_paid_broadcast`.

**Where the platform gives no protection.** Neither surface appears in `allowed_updates`: games
have no inbound update type at all beyond an ordinary `message` and a `callback_query` that can
only come from a button we sent, and Passport rides `message`. Neither appears on `getMe`. So
the platform offers no switch to leave off, which is the same situation unit 24 found for
Telegram Stars and the opposite of unit 08's inline mode.

### German and Union law, read 2026-08-27

- **§ 20(2) Personalausweisgesetz**, read at gesetze-im-internet.de: the identity card "darf nur
  vom Ausweisinhaber oder von anderen Personen mit Zustimmung des Ausweisinhabers ... abgelichtet
  werden", the copy must be "eindeutig und dauerhaft als Kopie erkennbar", and "Andere Personen
  als der Ausweisinhaber dürfen die Kopie nicht an Dritte weitergeben."
- **Article 9(1) GDPR** covers, among others, data revealing racial or ethnic origin and
  biometric data processed for the purpose of uniquely identifying a natural person. Article
  9(2)(e) — the condition this project relies on for incidental special-category content — reaches
  only data manifestly made public by the person concerned, which a document handed over
  privately is not. Article 9(2)(a), explicit consent, is the only condition that could fit, and
  the record of processing states that consent is not used in this activity.
- **Article 35(3)(b) GDPR** makes an impact assessment mandatory for processing of special
  categories on a large scale. The existing assessment does not cover a document intake and
  would need one before, not after.

### Our tree, at `bd70be2`

- **The poll consumes `message`, so nothing about the subscription protects us here.**
  `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]` (`client.rs:170`),
  named on every poll because "an absent selection inherits whatever an earlier setting left on
  the token" (`client.rs:166-169`). Passport data and game messages both arrive on the first of
  those three.
- **The decode is small and ignores what it does not name.** `Update` (`client.rs:172-188`) has
  the update id and three optional payloads; `Incoming` (`client.rs:192-211`) decodes
  `message_id`, `date`, `chat`, `from`, `sender_chat`, `text`, `caption`, `reply_to_message` and
  `pinned_message`. No `deny_unknown_fields` appears anywhere in the adapter, so an unknown field
  is dropped and does not fail the batch.
- **The skip that currently covers Passport is about text.** `Skip::NoText` is documented "A
  message with neither text nor caption (decision 0017)" (`translate.rs:54-55`), reached at
  `translate.rs:167-168` through `text_of` (`translate.rs:487-493`).
- **The skip log carries no payload.** `tracing::debug!(update_id = update.id, %reason, "update
  skipped")` (`driver.rs:369`), and the reason strings are fixed prose (`translate.rs:495-516`).
- **A recorded decision already lives as a named skip.** `Skip::OnBehalfOfChat` carries "(decision
  0016)" in its own doc comment (`translate.rs:51-53`) and is checked at `translate.rs:161-162`.
  So a refusal expressed as a named translation skip is an established shape in this adapter, not
  an invention of this unit.
- **Private chats are a configuration switch, not an absence.** `DirectChats::Off` refuses a
  direct-channel inbound before any write (`assembly.rs:1206-1208`, decision 0069), and the
  default is on. A Passport message would arrive in a private chat, so it is important that the
  adapter's refusal does not depend on that switch: it is skipped in translation, before the core
  is reached, under either setting.
- **Erasure has one answer per row and no reach outside this machine.** It nulls the personal
  columns of a person's messages and removes their direct conversations (`erasure.rs:1-40`).
  Nothing in it can reach a score held on the platform's servers.
- **The core has no vocabulary for either surface.** `InboundMessage` is a channel, a sender, an
  authority, an addressed flag, a reply target, a command, text, an origin and a timestamp
  (`message.rs:171-210`); `OutboundReply` is a channel, text, a kind and a reply target
  (`message.rs:414-434`). There is no identity claim, no document, no score and no leaderboard,
  and `docs/platform-vocabulary.txt` needs no new entry because the core learns nothing here.
- **Tools are admitted per conversation and checked at the call** (`tools/mod.rs:14-24`), and
  decision 0040 puts tool behaviour in the core. A score-setting tool or a document-checking tool
  would have to go there, which is where decision 0070 meets it.
- **Unit 01 rewrites the skip this refusal currently rests on**, renaming `Skip::NoText` to
  `Skip::NothingToRecord` and recording a captionless message that carries a carried file
  (`docs/units/telegram/01-receiving-media.md:249`, `:602`). Its carried set is an enumeration of
  message-level media fields, and neither `passport_data` nor `game` is in it.
- **Unit 07 decodes `callback_query` and reads `data`.** Its contract requires that "the token
  must resolve to an offer", and the API states that a game press carries `game_short_name`
  instead of `data`. This unit re-specifies none of that; it adds `callback_game` to the outbound
  scan so unit 07's keyboard cannot grow a game button.
- **Unit 08 specified the outbound source scan, and unit 24 extended it.** A test over the
  adapter crate's sources, failing with `file:line` on a forbidden literal, in the shape
  `crates/core/tests/vocabulary.rs:33-87` uses, with the literals in a committed list file so the
  scanning file does not contain them (`docs/units/telegram/08-inline-queries.md:679-685`,
  `docs/units/telegram/24-payments-stars-and-gifts.md:448-455`). Case-sensitive matching, with the
  lowercasing step of the vocabulary scan (`crates/core/tests/vocabulary.rs:23`, `:76`) dropped.
- **The adapter suite can script an update and read the store.** `push_update`
  (`tests/adapter/server.rs:154`) takes a whole JSON value and `get_updates` selects by
  `update["update_id"]` (`server.rs:539-553`), so a scripted update carries an `update_id` and one
  payload object. `await_conversations` and `await_chat_messages` (`tests/adapter/support.rs:820`,
  `:791`) read the ledger, and `fixture.store.run` reaches the connection directly
  (`tests/adapter/offset.rs:149-158`).
- **The adapter already owns a scanning test binary.** `tests/token_scan.rs` proves the token
  reaches no log line or error string, and documents why a capture subscriber needs its own
  target.
- **The suite never leaves the machine**: the scripted server binds loopback
  (`tests/adapter/server.rs:1-11`), so every criterion below is provable before merge with no
  live call.

## Decisions taken with this unit

- **Neither Passport nor games ships, 2026-08-27.** For Passport: no key pair is generated, no
  public key is registered, `passport_data` is never decoded, no Passport file is fetched and
  `setPassportDataErrors` is never called. For games: no game is created in BotFather, and
  `sendGame`, `setGameScore`, `getGameHighScores` and `callback_game` are never used. The reasons
  are the four findings above, and the shortest form of each is that Passport has no Article 9
  condition and a German statutory prohibition on passing the copy to a third party, while games
  create personal data on the platform's servers that this project's deletion promise cannot
  reach. *Rejected:* Passport as a joining check for the group, the shape somebody will propose
  first — the assistant would then decide who may participate on the strength of a document,
  which decision 0070 settles against, and the platform offers no way to make group membership
  depend on it in any case, so the check would be advice the assistant gives an administrator,
  for which a document is not needed. *Rejected:* Passport for age verification — same objection,
  plus it would put children's documents specifically into a store whose impact assessment
  records children as data subjects and offers them no protection matched to that. *Rejected:* a
  game as a community amusement — unit 14 already ships the platform's built-in throwaway
  animation, which names nobody and stores nothing, and it covers the actual want without a
  scoreboard. *Rejected:* leaving either question open as a follow-up — follow-ups record accepted
  shortfalls in shipped work; these are decisions, and an unrecorded decision is re-derived or
  quietly reversed by the next unit.
- **A Passport message is refused by name in the translation, and the check runs before every
  other condition, 2026-08-27.** `Incoming` gains one field, `passport_data:
  Option<serde::de::IgnoredAny>`, which detects presence and retains nothing — `IgnoredAny`
  discards the value as it parses, so the ciphertext is never materialised into any value this
  code holds, exactly as today, while the presence becomes visible. `translate` checks it
  immediately after the message is unwrapped, before the chat-kind match, before the pin branch,
  before the sender checks and before the text rule, and returns a new
  `Translation::Skip(Skip::PassportData)` whose doc comment names this unit's decision. The
  ordering is the point: every other condition in that function is one a sibling unit is
  rewriting or may rewrite, and a refusal that depends on any of them is a refusal that can be
  removed by somebody working on something else. *Rejected:* leaving it to `Skip::NoText`, which
  is what happens today — it is correct by coincidence, it is documented as being about text
  (`translate.rs:54-55`), and unit 01 renames and re-conditions it. *Rejected:* decoding the
  payload into named types so the skip could log what was declined — that reproduces the document
  metadata in this process's memory and invites a log line naming somebody's document types,
  which is a category of data this project has no basis to hold for one microsecond. *Rejected:*
  putting the check in the core instead, on the argument that a refusal is a decision — the core
  has no vocabulary for identity documents and would have to learn one in order to refuse one,
  which is the opposite of what the invariant asks; declining to translate a payload that has no
  neutral form is a translation fact, and decision 0016 already lives in this same enum as a
  named skip. *Rejected:* adding "passport" to `docs/platform-vocabulary.txt` so the core can
  never name it — that list is for platform and SDK names, the word is ordinary English, and
  bending a general mechanism around one concrete case is the smearing this project refactors
  away from.
- **A game message gets no named skip; its outcome is pinned by test instead, 2026-08-27.** The
  asymmetry with Passport is deliberate and has a reason. A game message in a group is a member
  using somebody else's game bot, and "nothing here to record" is a true and complete description
  of it — the title and description belong to a third party's product, not to the member, and
  decision 0017 already says the text column is what the person typed. A Passport payload is not
  a message with nothing in it; it is a category of data this repository must never hold, which
  is a different statement and deserves its own name. What games get is unit 24's shape: a test
  asserting the outcome — no block, no conversation, no principal row, no outbound request — and
  deliberately not naming the `Skip` variant, so unit 01's rewrite cannot invalidate it and
  cannot quietly change it either. *Rejected:* a `Skip::GameMessage` variant for symmetry — it
  would state a decision this unit does not need to make, since a game message is genuinely
  nothing to record, and an enum that grows a variant per platform content type is the bolted-on
  shape the project refactors away from. *Rejected:* recording the game's title as the message
  text so the model sees that something was played — it is not the member's words, and decision
  0017 is explicit that the text column is verbatim.
- **The outbound refusal joins the existing scan, and no inbound name goes in the list,
  2026-08-27.** The committed list that units 08 and 24 build gains `sendGame`, `setGameScore`,
  `getGameHighScores`, `callback_game`, `CallbackGame`, `InlineQueryResultGame`, `GameHighScore`,
  `game_short_name`, `setPassportDataErrors` and `PassportElementError`, each with a comment
  naming this unit. Matching stays case-sensitive substring matching, as unit 24 established.
  `passport_data` is deliberately **not** in the list: this unit's own diff puts that name into
  `client.rs` and into a test fixture, so a scan covering it would fail on the change that
  implements the refusal — the same reason unit 08 keeps `inline_query` out of its list. The
  Passport refusal is checked by the translation and its tests instead, which is the stronger
  check for an inbound field anyway. *Rejected:* a second scanner for this unit's literals — one
  mechanism, one list file, per unit 24's decision. *Rejected:* scanning for `Game` or `Passport`
  as bare words — both occur in ordinary prose and in the doc comments this unit writes, and a
  check that cries wolf is deleted by the first person it inconveniences.
- **A Passport message is answered with nothing, 2026-08-27.** No reply, no
  `setPassportDataErrors`, no acknowledgement of any kind; the update is acknowledged to the
  platform and the offset advances, as it does today. *Rejected:* telling the person their
  documents were not accepted — the reply would have to be a new deterministic core message in a
  private chat, which decision 0069 may have switched off entirely, and it would mean this
  assistant holding a conversation about somebody's documents in order to say it holds nothing.
  *Rejected:* calling `setPassportDataErrors` with an unspecified error to unblock the person's
  next attempt — it blocks re-submission until the errors are fixed, which is an effect on a
  person for the sake of tidiness, and it requires naming the elements that were sent.
- **The two methods that act on a person are refused as effects, not merely as unused features,
  2026-08-27.** `setPassportDataErrors` stops somebody re-submitting until they change their
  document, and `setGameScore` with `force` exists, in the platform's own words, for "banning
  cheaters". Decision 0070 settles that the assistant assesses and a human decides; unit 24
  extended that to value transfer. This unit records the same for these two: neither is
  registered as a tool, neither is called from any path, and neither ships without a mechanism in
  which a person approves the concrete action first. *Rejected:* treating them as harmless because
  no game and no Passport key exists — the absence of a game is a BotFather setting the operator
  can change in ten seconds, and a rule that holds only while a setting holds is not a rule.
- **No privacy or compliance document changes with this unit, and the reversal list is written
  down instead, 2026-08-27.** Nothing new is received, decoded, stored or sent anywhere; the one
  code change makes an existing refusal explicit. None of the record's review triggers
  (`docs/privacy/records-of-processing.md:181-190`) fires. *Rejected:* an amendment recording that
  the project considered accepting identity documents and did not — a record of processing
  describes processing, and non-events in it make the real entries harder to audit.
- **Nothing streams, and the reason a Passport intake would be hard to stream is recorded,
  2026-08-27.** This unit moves no bytes: it adds one presence-only field, one skip, four tests, a
  decision record and a contract section. Recorded because the streaming constraint binds every
  spec. If the question were ever reopened, the shape is known and is not a dodge: a Passport file
  is at most 10 MB, `getFile` allows 20 MB, and AES-256-CBC decrypts block by block, so the bytes
  can move chunk by chunk into a staged file exactly as unit 01 specifies — but the plaintext's
  first byte gives a 32-to-255-byte padding run that must be stripped from the head, and the
  authenticity check is SHA256 over the whole decrypted content compared against `file_hash`, so
  nothing may be trusted or promoted until the last byte has arrived. Unit 01's stage-then-rename
  already has that property; the difference is that the staged file would hold somebody's passport
  while it was being checked.
- **The operator contract states both refusals in the operator's own terms, 2026-08-27.** A new
  section says that `/newgame`, `/setpublickey` and `/setprivacypolicy` are never sent for this
  token, that no RSA private key for Passport exists on the machine that runs the assistant, and
  that `getMe` reports no flag for either surface, so the source and BotFather between them are
  the only places these refusals live. *Rejected:* leaving it undocumented because there is
  nothing for the operator to do — the one action that would break this unit's contract without
  touching a line of code is the operator's, so it belongs in the operator's document.

## What this unit examined and deliberately leaves alone

**Another bot's game played in an admitted group.** A member can send a game into the group
through a third-party bot's inline mode, and members can play it and set scores there. Nothing in
this repository is involved: the game message carries no text and is skipped, the scores are the
other bot's, and `Message.via_bot` remains undecoded, which unit 08 already examined and left to
whichever unit next opens the message decode. This unit changes nothing about that and names it so
the next reader finds it examined.

**The high-score service message.** The gaming platform page says "When a new high score is set, a
service message will be sent to the chat", and the `Message` object has no field for it. What such
an update looks like on the wire is therefore not established from the documentation, and no test
here can script it faithfully. Nothing merged depends on it: a message with no text and no field
this adapter decodes is skipped whatever its shape, which is the same outcome the game-message pin
asserts. Marked unproven in the shape unit 06's AC13 and unit 08's AC16 use.

**Whether Passport data can reach a bot that never registered a public key.** The manual requires
a `public_key` in the authorization request and a `/setpublickey` registration, which strongly
suggests the flow cannot complete against this token at all, and no page read on 2026-08-27 states
what happens when the two disagree or when none is registered. It is recorded as an inference and
nothing depends on it: the translation refusal fires on presence, whether the payload can arrive
today, arrives after a platform change, or never arrives.

**`chat_instance` on a callback query.** Documented as "Useful for high scores in games", it is
the one game-shaped field on an update type unit 07 subscribes to. Unit 07 does not decode it and
this unit does not ask it to; the field carries no personal data and no game exists to score.

## What would have to be true before either is reopened

Refusing without naming what could work is refusing without examining. Neither list is a deferred
decision; the answer is no today, for the reasons above.

**Passport.** Five things, in order:

1. **A purpose has to exist.** Nothing in a community assistant for an Android distribution needs
   an identity document. A reopening that cannot name what the document decides is a mechanism
   looking for a use.
2. **An Article 9 condition has to be claimed and be true**, which in practice means explicit
   consent under 9(2)(a) — freely given, specific, informed, withdrawable — collected before any
   document arrives, in a record that today states consent is used nowhere.
3. **The German prohibition has to be answered, not stepped around.** § 20(2) PAuswG forbids
   passing the copy to a third party. Any design must therefore keep document content out of the
   model path entirely, which means the core would gain its first stored category that must never
   be projected, decided in its own unit, not inherited from this one.
4. **A second secret has to be justified and protected.** The RSA private key, its storage, its
   rotation, and what happens to every past document if it leaks.
5. **Four documents change before the code merges.** The record of processing gains a purpose, a
   basis, a data category for identity documents and a retention entry; the impact assessment
   gains an addendum and, under Article 35(3)(b), a fresh assessment and not a paragraph; the
   legitimate-interests assessment must state that this processing does not run on legitimate
   interest at all; and the public notice, the one document a member reads, changes where it says
   "We take nothing about you from anywhere else". The public notice is named first because it is
   the promise made to people, not to an auditor.

**Games.** Three things:

1. **The erasure hole has to be closed or accepted in the open.** A score is personal data on the
   platform's servers, keyed to a person, with no delete method. Either the platform gains one, or
   the notice's deletion promise gains a second exception written in plain words before the
   feature merges.
2. **The high-score fetch has to be justified against the notice's own sentence.**
   `getGameHighScores` returns up to eight named strangers; "We take nothing about you from
   anywhere else" would have to change first.
3. **The message-drift problem needs an answer.** A game message's content is rewritten by the
   platform as scores change, and the ledger is append-only with nothing to append from. Either
   the outbound block records that it names a message whose content is not ours, or the ledger's
   copy silently diverges from the chat.

## The unit's contract

After this unit the repository's answer to "may the assistant receive identity documents or run
games" is a recorded no with its reasoning, and for Passport the no stops being a coincidence.
`Incoming` decodes `passport_data` as a presence-only field that retains nothing, and `translate`
refuses such a message by its own name, `Skip::PassportData`, checked immediately after the
message is unwrapped and therefore before the chat kind, the pin branch, the sender checks and the
text rule — so no sibling unit's rewrite of any of those conditions can remove the refusal, and no
part of the payload is decoded, stored, logged or sent anywhere. A game message continues to
record nothing, pinned by outcome and not by variant name so unit 01's rewrite of the
nothing-to-record condition cannot change it silently. On the outbound side the committed refusal
list that units 08 and 24 build gains this unit's literals, so no keyboard can grow a
`callback_game` button and no path can call `sendGame`, `setGameScore`, `getGameHighScores` or
`setPassportDataErrors` without failing a check that names the file and line. The core is
untouched: no new entry point, no new kind, no new table, no new tool, no vocabulary for
documents, scores or leaderboards, and `docs/platform-vocabulary.txt` is unchanged. Two documents
are added: a decision recording both refusals, and an operator contract section stating that
`/newgame`, `/setpublickey` and `/setprivacypolicy` are never sent for this token and that no
Passport private key exists on the machine. No privacy or compliance document changes, because
nothing new is received, stored or sent. Nothing streams, because nothing here carries a byte. No
new dependency, no new configuration entry, and no change to any behaviour a member can observe.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes — `AnsweringMode::Helpful` and
  `AnsweringMode::Addressed` (`assembly.rs:180-188`); clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan and the token scan clean; no new dependency and no new
  configuration entry; the diff touches no file under `crates/core/` and none under
  `docs/privacy/` or `docs/compliance/`.
- **AC2** The presence field retains nothing: `Incoming` (`client.rs:192-211`) gains
  `passport_data: Option<serde::de::IgnoredAny>` with a doc comment naming this unit's decision
  and stating that the type exists so the presence is visible while the value is discarded as it
  parses. Checked by reading the diff and by AC3.
- **AC3** The refusal is by name and runs first: a unit test in `translate.rs`'s existing test
  module (`translate.rs:519-531`) builds an `Incoming` in a private chat that carries a sender, a
  non-empty text, a caption **and** `passport_data: Some(IgnoredAny)`, and asserts
  `Translation::Skip(Skip::PassportData)`. Giving the fixture text is the point of the test: it
  proves the check does not depend on the text rule. A second case, the same message in a
  supergroup, asserts the same, so the refusal does not depend on the chat kind either.
- **AC4** End to end, a Passport message stores nothing and requests nothing: with the scripted
  server pushing an update carrying an `update_id` and a `message` object with a chat, a sender, a
  date, no text and a `passport_data` object holding a `data` array of two elements — one of type
  `personal_details` with a `data` string, one of type `passport` with a `front_side` and a
  `selfie` — and a `credentials` object with `data`, `hash` and `secret`, the adapter acknowledges
  it, the next poll's offset is past it, the store holds no new block, conversation, channel
  mapping or principal row, and the recorded requests for that batch contain no `getFile`, no
  `sendMessage`, no `setPassportDataErrors`, no `getChat` and no `getChatAdministrators`. Row
  counts are read through `fixture.store.run` before and after, in the shape
  `tests/adapter/offset.rs:149-158` uses.
- **AC5** The fixture is visibly synthetic and no captured log line carries any of it: every
  string in AC4's payload is an obvious placeholder that is not valid base64 of anything, and a
  capture assertion proves that no log line from that batch contains any of those strings. The
  emitted line is the existing skip line with the update id and the fixed reason. In the shape
  `tests/token_scan.rs` already uses for the token, in its own test target for the same reason.
- **AC6** A game message records nothing, pinned by outcome: a scripted `message` update whose
  message carries a chat, a sender, a date, no `text` and no `caption`, and a `game` object with a
  `title`, a `description`, a `photo` array and a `text`, is acknowledged; the store gains no
  block, no conversation, no channel mapping and no principal row; and no outbound request is
  recorded for that batch. The assertion names the outcome and not the `Skip` variant, and carries
  a comment saying why: unit 01 renames and re-conditions that variant.
- **AC7** The outbound refusal is checked: the committed list beside the adapter's refusal scan
  gains `sendGame`, `setGameScore`, `getGameHighScores`, `callback_game`, `CallbackGame`,
  `InlineQueryResultGame`, `GameHighScore`, `game_short_name`, `setPassportDataErrors` and
  `PassportElementError`, each under a comment naming this unit, and the scan fails with
  `file:line` on any occurrence. Matching is case-sensitive substring matching, per unit 24. If
  neither unit 08 nor unit 24 has merged, this unit creates the scanner and the list in the shape
  those units specify; if either has, this unit adds lines and creates no second scanner.
- **AC8** The list contains no inbound name: `passport_data` and `game` are absent from it,
  asserted by reading the list file, with a comment in the file stating that this unit's own
  sources and fixtures contain both and that the inbound refusal is checked by AC3 and AC4
  instead.
- **AC9** The negative check holds: the scan proves it can fail, by matching a string assembled at
  runtime that no source file contains verbatim. Carried over from the scan's own unit if it has
  merged; written here if this unit creates the scanner.
- **AC10** The decision is recorded: a file in `docs/decisions/` carries both refusals with their
  date and their rejected alternatives, including the statement that `setPassportDataErrors` and
  `setGameScore` are effects on a person and fall under decision 0070's rule. The number is taken
  at merge time, continuing the numbering after whatever is unclaimed then.
- **AC11** The operator is told: `docs/reference/group-operator-contract.md` gains a section
  stating that no game is created for this token, that `/newgame` is never sent, that
  `/setpublickey` and `/setprivacypolicy` are never sent and no Passport private key exists on the
  machine, that `getMe` reports no capability flag for either surface so nothing in the code can
  detect a change, and that a group wanting to stop games generally can only do so by removing
  `can_send_other_messages`, which also removes stickers and inline bots.
- **AC12** No privacy or compliance document is modified by this unit's diff, and the reopening
  checklists in this document name the record of processing, the impact assessment, the
  legitimate-interests assessment and the public privacy notice explicitly, with the clause each
  would have to change.
- **AC13** The unproven inferences are named here and nothing merged depends on them: that a
  Passport authorization cannot complete against a token with no registered public key, and what a
  game high-score service message looks like on the wire. Both are recorded above with the reason
  each is unprovable from the documentation read on 2026-08-27, and in both cases the refusal
  holds whichever way the fact falls.

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. The production diff is one field, one enum variant, one branch and one doc comment;
  the tests and the two documents are the rest.
- Adapter sites, in `crates/adapters/telegram/src/`:
  - `Incoming` (`client.rs:192-211`) gains the presence-only `passport_data` field per AC2.
  - `Skip` (`translate.rs:42-78`) gains `PassportData`, with a doc comment naming the decision, in
    the shape `OnBehalfOfChat` (`translate.rs:51-53`) already uses for decision 0016.
  - `Display for Skip` (`translate.rs:495-516`) gains its reason string. Word it as a refusal —
    the assistant does not accept identity documents — not as a description of the payload, since
    the string reaches the log.
  - `translate` (`translate.rs:121-129`) gains the check immediately after `let Some(message) =
    &update.message`, before the chat-kind match. Placement is the substance of the decision, so
    the code carries a short comment saying why it is first.
- Adapter test sites, under `crates/adapters/telegram/tests/`:
  - AC3 in `translate.rs`'s own test module, beside the existing skip tests.
  - AC4 and AC6 in `adapter/offset.rs`, which already exercises acknowledgement and offset through
    the scripted server and already reads the store directly. Each scripted update carries an
    `update_id` plus one payload object, since `get_updates` selects by that field
    (`adapter/server.rs:539-553`).
  - AC5 in its own test target, for the reason `tests/token_scan.rs` documents.
  - AC7, AC8 and AC9 in the refusal scan target and its committed list file. Check first which of
    units 08 and 24 has merged and extend it instead of duplicating it.
- Documentation sites: one decision file, number taken at merge; a section in
  `docs/reference/group-operator-contract.md` per AC11. No entry in `docs/follow-ups.md` — these
  are decisions, not accepted shortfalls.
- Sibling collisions: none. This unit adds nothing to `CONSUMED_UPDATE_TYPES` and asserts nothing
  about it, because neither surface has an update type of its own — the collision units 05, 07, 08
  and 09 documented between themselves does not reach here. The one real interaction is with unit
  01, in both directions: unit 01 renames the variant AC6 deliberately does not name, and unit 01's
  implementer should read AC4 before widening what a file-bearing message records.
- One thing to watch after merge: if the operator ever creates a game or registers a public key in
  BotFather for an unrelated reason, nothing in this repository can see it. There is no capability
  flag for either, which is why the refusal is written into the operator contract as well as into
  the code.
