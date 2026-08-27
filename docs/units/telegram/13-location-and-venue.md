# Telegram 13 — a shared position, place or contact card

Date: 2026-08-25. A member can put four things into a chat that this assistant has never
seen: a point on the map, a live point that keeps moving, a named place with an address,
and another person's phone number. All four are skipped today, and all four are the
sharpest personal data the platform can hand us — a position is where a body physically
is, and a contact card describes somebody who never joined the conversation and cannot be
told anything. This unit records that such a message happened and records nothing else:
the coordinates, the address, the name and the phone number are read off the wire, not
decoded, not stored and not sent anywhere. It also answers the outbound half of the
feature, which is the loud part: the assistant does not send a position, a live position,
a place or a contact card, and two of those methods cannot be called from this tree at
all. The receipts below come from the live Bot API documentation read on 2026-08-25 and
from both repositories. Two unbriefed reviews were run against the first draft; what they
proved is folded in below, and what they got wrong is answered where it belongs rather
than dropped.

## The findings that shape the whole unit, stated at the top

The obvious reading of this feature is "carry location and venue and contact, in and out".
Four separate facts break that reading, and each one is checkable.

1. **The outbound half has no honest caller.** The assistant has no body, no map and no
   phone book. Every coordinate, address and phone number it could send would come from a
   language model's memory, which decision 0096's answering discipline already forbids for
   any substantive claim — and a fabricated coordinate does not read as a sentence a member
   can weigh, it renders as an authoritative pin on a map. The unit therefore ships no
   sending capability and records why, so that a later unit does not reopen it by accident.
2. **Two of the five methods are unreachable from this tree regardless.**
   `editMessageLiveLocation` and `stopMessageLiveLocation` both need the identifier of a
   message the bot itself sent, and `send_body` throws the platform's returned `Message`
   away (`crates/adapters/telegram/src/client.rs:455-460`, `let _sent: serde_json::Value`).
   Unit 05 found the same thing from the other side and stated it as "nothing in the tree
   can learn the identifier of a message the assistant sent"
   (`docs/units/telegram/05-polls.md:180-186`).
3. **A live position's movement is not documented as reaching a bot at all.** The
   documentation says how a live location is changed (`editMessageLiveLocation`) and what
   an edit update is (`Update.edited_message`), and it never says that a member's own
   movement produces one. This unit is written so the answer does not matter: the edit skip
   stays, by decision, and a live position is recorded once as a fact and never followed.
4. **The never-decode promise cannot be checked by scanning source text, and the first
   draft's criterion for it was unsatisfiable.** The draft asked that the strings `title`,
   `first_name` and `address` appear nowhere in the adapter's client module. Two of them are
   there and are wanted there: `ChatInfo.title` at
   `crates/adapters/telegram/src/client.rs:190` is the group title the channel enrichment
   collects as category D4 (`docs/privacy/records-of-processing.md:64`), and
   `BotIdentity.first_name` at `crates/adapters/telegram/src/client.rs:238` is the display
   name the embedder reads for the assistant's own default name. `address` occurs inside
   `addressing` at `:139` and `:195`. Worse, the scan shape the draft borrowed is vacuous
   for the compound names: `carries_word` in `crates/core/tests/vocabulary.rs:63-66` splits a
   line on every non-alphanumeric character, so no token can ever equal `first_name`,
   `phone_number`, `horizontal_accuracy`, `proximity_alert_radius`, `google_place` or
   `last_name`. The criterion was impossible on one word and empty on six. It is replaced
   below by a structural check that proves the same property exactly: the decoding types are
   sized so they cannot hold a payload, and a sentinel-bearing update is followed through the
   decoder, the store and the tracing output.

Nothing here is left open. Points 1 and 2 are decisions with their reasons below; point 3 is
an undocumented platform behaviour, named as undocumented and designed around instead of
assumed; point 4 is a defect in the first draft, corrected in the decisions and in AC5.

### The collision with unit 01, which a merge must resolve rather than paper over

`docs/units/telegram/01-receiving-media.md` and this unit disagree on a stated fact, and the
disagreement is not additive.

- Unit 01's decision 0106 (`docs/units/telegram/01-receiving-media.md:249-252`) names what
  still skips after that unit, in a closed list: "a captionless sticker, a forwarded story,
  **a contact, a location**, a poll, dice, a captionless paid-media post, and a
  `rich_message`". Its AC2 (`docs/units/telegram/01-receiving-media.md:601-606`) pins
  `Skip::NothingToRecord` for "a location".
- This unit records a location and a contact.

One tree cannot hold both. This unit takes the position that a shared position, place or
contact card is a recorded message, and the reconciliation is written as a decision below
with its exact edits, in both merge orders. It is stated here because the first draft's
cross-unit note covered only the skip's rename, which reads as compatible and is not.

Two further points where these specs disagree, this one being correct on the platform:

- Unit 01 states at `docs/units/telegram/01-receiving-media.md:252-254` that "a *captioned*
  message of any of those kinds already records today through `text_of`'s caption fallback".
  For a contact and for a location that cannot happen: `caption` is documented as "Caption
  for the animation, audio, document, paid media, photo, video or voice", and neither
  location nor contact appears in that list. The claim holds for the sticker and paid-media
  cases it was written for and not for the two this unit takes over. Recorded here, not
  edited into that spec.
- Unit 01's decision 0106 covers the file-bearing kinds. It does not reach the proximity
  alert, which carries no file, no text and no caption, and which this unit names as its own
  skip.

## Grounding

### The platform

- **The live documentation is Bot API 10.3, dated 24 August 2026** (top entry of
  `/bots/api-changelog`), not 10.1 as the brief assumed: 10.2 shipped 14 July 2026, 10.1 on
  11 June 2026, 10.0 on 8 May 2026, 9.6 on 3 April 2026.
- **All three sending methods changed twice in the last two months.** 10.2: "Added the
  parameters *receiver_user_id* and *callback_query_id* to the methods … *sendContact*,
  *sendLocation*, *sendVenue*." 10.3: "Added the class *EphemeralMessageParameters* and
  replaced the parameters *receiver_user_id* and *callback_query_id* in the methods …
  *sendContact*, *sendLocation* and *sendVenue* with the parameter
  *ephemeral_message_parameters*."
- **The receiving objects have not changed in the searched range.** Every changelog entry
  from 7.3 (6 May 2024) to 10.3 was read for a bullet adding, changing or removing a field of
  the classes `Location`, `Venue`, `Contact` or `ProximityAlertTriggered`; there is none. The
  7.3 entry is the boundary of that search and not evidence about the receiving side — both
  of its live-location bullets are about sending and editing ("Added support for live
  locations that can be edited indefinitely, allowing 0x7FFFFFFF to be used as *live_period*"
  is about the argument, and "Added the parameter *live_period* to the method
  *editMessageLiveLocation*" is a method parameter). The receiving classes are older than the
  searched range; what matters for this unit is that nothing in two years has widened them,
  which is what was checked.
- **`Location`, verbatim field list**: `latitude` ("Latitude as defined by the sender"),
  `longitude` ("Longitude as defined by the sender"), `horizontal_accuracy` ("Optional. The
  radius of uncertainty for the location, measured in meters; 0-1500"), `live_period`
  ("Optional. Time relative to the message sending date, during which the location can be
  updated; in seconds. For active live locations only."), `heading` ("Optional. The
  direction in which user is moving, in degrees; 1-360. For active live locations only."),
  `proximity_alert_radius` ("Optional. The maximum distance for proximity alerts about
  approaching another chat member, in meters. For sent live locations only."). The three
  optional fields marked "for active live locations only" are the discriminator: **a live
  position is distinguishable from a plain one on the first message, by the presence of
  `live_period`.**
- **`Venue`, verbatim**: `location` ("Venue location. Can't be a live location."), `title`
  ("Name of the venue"), `address` ("Address of the venue"), `foursquare_id`,
  `foursquare_type`, `google_place_id`, `google_place_type`. Both venue-provider identifier
  pairs are third-party catalogue references.
- **`Contact`, verbatim**: `phone_number` ("Contact's phone number"), `first_name`,
  `last_name` (optional), `user_id` ("Optional. Contact's user identifier in Telegram" —
  the same 52-significant-bit note the other account identifiers carry), `vcard`
  ("Optional. Additional data about the contact in the form of a vCard"). On `sendContact`
  the same field is bounded: "Additional data about the contact in the form of a vCard,
  0-2048 bytes". A vCard is an open-ended record: it can carry postal addresses, several
  numbers, an employer, a birthday and a photograph.
- **`Message.venue` aliases `Message.location`, verbatim**: "Optional. Message is a venue,
  information about the venue. **For backward compatibility, when this field is set, the
  location field will also be set.**" This is the same alias shape unit 01 found on
  `animation`/`document` and `live_photo`/`photo`, and it decides dispatch order: a place
  must be tested before a position, or every place records as a position.
- **`Message.contact`**: "Optional. Message is a shared contact, information about the
  contact." **`Message.location`**: "Optional. Message is a shared location, information
  about the location."
- **These messages never carry text.** `caption` is documented as "Caption for the
  animation, audio, document, paid media, photo, video or voice" — location, venue and
  contact are not in that list, and `Message.text` is a text message's own field. So a
  shared position, place or contact card arrives with no words on it at all, ever.
- **`Message.proximity_alert_triggered`**: "Optional. Service message: a user in the chat
  triggered another user's proximity alert while sharing Live Location." The
  `ProximityAlertTriggered` object is `traveler` (User, "User that triggered the alert"),
  `watcher` (User, "User that set the alert") and `distance` (Integer, "The distance between
  the users"). That is a measured physical distance between two named members, computed by
  the platform, addressed to nobody.
- **A bot receives every service message whatever its privacy setting.** The features page,
  verbatim: "All bots will also receive, regardless of privacy mode: All service messages.
  All messages from private chats. All messages from channels where they are a member." So
  the proximity alert reaches a bot that is deliberately kept blind to ordinary chat, and
  refusing it has to be a decision, not a side effect of a setting.
- **Privacy mode decides whether the other four arrive at all.** With it on a bot sees only
  commands aimed at it, general commands when it spoke last, inline messages via the bot,
  and replies to it. This deployment already requires it off
  (`docs/reference/group-operator-contract.md:9-18`), and nothing in this unit works
  without that.
- **`sendLocation`** — required `chat_id`, `latitude`, `longitude`. Optional, with the
  limits verbatim: `horizontal_accuracy` ("0-1500"), `live_period` ("Period in seconds
  during which the location will be updated (see Live Locations), must be between 60 and
  86400, or 0x7FFFFFFF for live locations that can be edited indefinitely. Must be 0 for
  ephemeral messages."), `heading` ("Must be between 1 and 360 if specified."),
  `proximity_alert_radius` ("Must be between 1 and 100000 if specified."), plus the common
  sending parameters.
- **`sendVenue`** — required `chat_id`, `latitude`, `longitude`, `title` ("Name of the
  venue"), `address` ("Address of the venue"); optional the two provider identifier pairs.
  There is no live form: `Venue.location` "Can't be a live location."
- **`sendContact`** — required `chat_id`, `phone_number`, `first_name`; optional
  `last_name` and `vcard` (0-2048 bytes). Nothing in the method asks whether the person
  named consented to being sent.
- **`editMessageLiveLocation`**, verbatim: "Use this method to edit live location messages.
  A location can be edited until its live_period expires or editing is explicitly disabled
  by a call to stopMessageLiveLocation." `message_id` is "Required if inline_message_id is
  not specified. Identifier of the message to edit." Its `live_period` carries a second
  bound: "the new value must not exceed the current live_period by more than a day, and the
  live location expiration date must remain within the next 90 days".
- **`stopMessageLiveLocation`**, verbatim: "Use this method to stop updating a live location
  message before live_period expires." Same identifier requirement: "Identifier of the
  message with live location to stop."
- **The five named sending methods are not the only way to produce one of these messages.**
  Six further classes exist and were checked by name in the live documentation:
  `InlineQueryResultLocation`, `InlineQueryResultVenue`, `InlineQueryResultContact`,
  `InputLocationMessageContent`, `InputVenueMessageContent` and `InputContactMessageContent`.
  All six are reachable only through inline mode, which `docs/units/telegram/08-inline-queries.md`
  refuses outright and pins as refused. That refusal is what makes the five-method fence
  complete today, and it is named as a dependency in the notes rather than assumed.
- **Inline mode is also a fifth inbound route to a member's coordinates.** `InlineQuery`
  carries `location`, documented as "Sender location, only for bots that request user
  location" and enabled separately (`docs/units/telegram/08-inline-queries.md:98`). This unit
  does not have to handle it because unit 08 refuses inline queries; it is recorded so the
  count of routes is honest.
- **A poll can carry a place, on the receiving side as well as the sending side.**
  `PollMedia`'s field list is `animation`, `audio`, `document`, `link`, `live_photo`,
  `location`, `photo`, `sticker`, `venue`, `video` — so an inbound poll is a sixth route to a
  position or a place. This adapter records no poll today, and unit 05 owns polls; the fact is
  written down for that unit in the notes.
- **`InputMediaLocation` and `InputMediaVenue` exist since 10.0 and are not a second way to
  send an ordinary message.** The 10.0 entry reads "Added the classes *InputMediaSticker*,
  *InputMediaLocation*, and *InputMediaVenue*", and the same version "Added the class
  *PollMedia*, representing a media in a poll". The `InputMedia` union is animation, audio,
  document, live photo, photo and video only; the location and venue forms belong to
  `InputPollMedia` and `InputPollOptionMedia`. So a poll option can carry a place, and
  ordinary message sending cannot reach these classes at all.
- **A group can itself be attached to a place.** `ChatLocation` is "Represents a location to
  which a chat is connected": a `Location` that "Can't be a live location", plus an
  `address` of "1-64 characters, as defined by the chat owner". That is the group's own
  property, not a member's position, and this unit does not collect it (a decision below).

### Our tree

- **The skip that blocks the whole feature.** `text_of` reads text then caption, filtered
  non-empty (`crates/adapters/telegram/src/translate.rs:467-472`), and translation returns
  `Translation::Skip(Skip::NoText)` when it yields nothing
  (`crates/adapters/telegram/src/translate.rs:165-167`); the variant is at `:53` and its
  reason line at `:481` reads "a message with neither text nor caption". Since a shared
  position, place or contact card never carries text or a caption, **all four are invisible
  to the assistant today**, and so is the proximity alert.
- **The order of the checks in `translate`, exactly.** The membership branch, then the edit
  skip (`crates/adapters/telegram/src/translate.rs:123-125`), then the missing-message skip,
  then the chat-kind match (`:129-134`), then the whole pin block (`:138-158`), then the
  on-behalf-of-chat check `message.sender_chat.is_some()` at `:159`, then
  `let Some(from) = &message.from` at `:162`, then `text_of` at `:165`. There are two sender
  checks, not one; a criterion that says "ahead of the sender check" names nothing, so the
  proximity-alert decision below names line 159 exactly.
- **The edit skip is already a named case.** `CONSUMED_UPDATE_TYPES` is
  `["message", "edited_message", "my_chat_member"]`
  (`crates/adapters/telegram/src/client.rs:103`), and the doc there notes the selection is
  sent on every poll so an absent one cannot inherit an earlier setting.
- **The pin path has three ways to yield nothing already.** `PinnedContent` decodes only
  `text` and `caption` (`crates/adapters/telegram/src/translate.rs:145-152`), so a pinned
  shared position falls to `Skip::TextlessPin` (`:151`). That is the behaviour this unit
  wants, and it means the pin path gains a fourth way to be textless without gaining a
  branch.
- **The adapter decodes none of these objects.** `Incoming`
  (`crates/adapters/telegram/src/client.rs:124-144`) holds `message_id`, `date`, `chat`,
  `from`, `sender_chat`, `text`, `caption`, `reply_to_message`, `pinned_message` and nothing
  else; unknown fields are dropped by the decoder. That not-decoding is already used as a
  deliberate protection: the sender's display name "is not decoded at all, so a display name
  never enters the process as a typed value"
  (`crates/adapters/telegram/src/client.rs:212-215`). It is the exact technique this unit
  reuses for coordinates, addresses and phone numbers.
- **Two of the payload words are already in that file and are wanted there.**
  `crates/adapters/telegram/src/client.rs:190` is `pub title: Option<String>` on `ChatInfo`,
  the group title the channel enrichment reads; `crates/adapters/telegram/src/client.rs:238`
  is `pub first_name: Option<String>` on `BotIdentity`, the display name the embedder reads
  for the assistant's own default name. `addressing` at `:139` and `:195` contains `address`
  as a substring. Any whole-file scan for the payload words fails on live features.
- **The word scan the draft borrowed cannot match a compound name.**
  `crates/core/tests/vocabulary.rs:63-66` splits each line on every non-alphanumeric
  character and compares whole tokens, so `first_name` and its five siblings can never match.
  The file's own doc says the same thing from the other side: "a word is one run of letters
  and digits". A payload scan in that shape would pass whatever the file contained.
- **A decoding failure refuses the whole update batch.** `ClientError::Decode { detail }` is
  produced when the envelope fails to parse (`crates/adapters/telegram/src/client.rs:570-573`),
  and its detail carries the redacted serde message only — no payload path into tracing exists
  through it. It also means the shape of the new decoding types matters: a type that refuses
  a well-formed update wedges the poll.
- **Addressing needs no change.** A message with no text cannot mention the assistant, so
  `addressed` falls out of the existing derivation
  (`crates/adapters/telegram/src/translate.rs:170-178`): true in a direct chat, true when it
  replies to the assistant, false otherwise. Nothing about a shared item is special here.
- **The adapter's own carrier and the seam into the core.** `Pending`
  (`crates/adapters/telegram/src/translate.rs:79-114`) is what `translate` returns and what
  `driver.rs:425-436` reads to build `InboundMessage`. A new neutral fact reaches the core
  through those two sites and no other; neither was named in the first draft.
- **The message kind and its projection.** `ChatMessage` at `crates/core/src/kind.rs:376-431`,
  `stored_fields` at `:444-485`, `projected_text` at `:555-569`, the descriptor's column list
  at `:575-597`, and `parse` at `:598-637` — the read-back half, without which a new column is
  written and never seen. `projected_text` returns `ERASED_MARKER` when `text` is `None`,
  otherwise composes `{speaker}: {text}` **only when the speaker is `Some` and the role is
  User**, and wraps the result in `projected_origin_mark(origin)` from the outside. A
  handleless sender stores NULL and projects bare (decision 0056,
  `crates/core/src/assembly.rs:718-724`), so there are two projected shapes, not one.
  `projected_origin_mark` renders `id-9` as `[id-9]` (`crates/core/src/kind.rs:181`, pinned at
  `:1318`).
- **`stored_fields` is positional, and its one production caller is the assembly.**
  `crates/core/src/assembly.rs:725` is the only call outside test code; the calls at
  `crates/core/src/outbound.rs:638` and `crates/core/src/tools/provenance.rs:392,424` sit
  inside `#[cfg(test)]` modules (`outbound.rs:501`, `provenance.rs:290`), as do the ones in
  `kind.rs` (`:1147`). The suite's call sites are real work all the same:
  `crates/core/tests/spine/group_context.rs:665`, `crates/core/tests/spine/protection.rs:162,802`
  and `crates/core/tests/spine/speaker.rs:129,503`. A sixth positional parameter touches every
  one of them, which is why the shape is decided below instead of left to the implementer.
- **`owes_answer` reads `text.is_some()`**, not non-emptiness
  (`crates/core/src/kind.rs:495-498`), so an empty text is a real recorded message that can
  still owe a turn when addressed, and an erased one cannot.
- **One doc comment states a premise this unit falsifies.** `crates/core/src/kind.rs:498-503`
  explains `erased()` as "the text is the one column whose absence only erasure produces —
  **the adapter never records an empty message** — so its null speaks for the whole row." The
  function is unaffected, because `Some("")` is not `None`; the sentence in the middle stops
  being true the moment either this unit or unit 01 merges, and it is named as a site.
- **Erasure's three steps.** The module doc (`crates/core/src/erasure.rs:1-50`) nulls the
  personal columns of the principal's messages — text, origin, send time, reply target,
  speaker — then removes the principal's direct conversations whole, then concludes the
  identity rows. Structural columns are left alone by design; `schema.rs` says so of the
  protection stamp in as many words ("Both columns are structure, not personal data: erasure
  leaves them", `crates/core/src/schema.rs:154-158`).
- **Direct chats are a configuration switch, defaulting on.** `DirectChats`
  (`crates/core/src/assembly.rs:198-206`) is `On` by default — "Direct channels are refused
  fail-closed before any write" is the `Off` arm — and decision 0069 records the switch. The
  shipped policy states "We do not serve direct chats"
  (`docs/privacy/bot-assistant-privacy-policy.md:23`) for the running deployment. The erasure
  criterion below therefore names the switch it depends on.
- **Schema growth is append-only, with frozen vocabularies.** The shipped `CREATE TABLE` is
  frozen (decision 0026); every change since is an appended step quoting a vocabulary list
  frozen when the step shipped, never the live enum
  (`crates/core/src/schema.rs:113-123`), with `PROTECTION_STAMP_MIGRATION`
  (`crates/core/src/schema.rs:159-172`) as the shape to copy: `ALTER TABLE … ADD COLUMN …
  TEXT CHECK (… IN (…))`. The same doc names the discipline's own check: "The tests at the
  bottom pin each newest frozen list to its enum, so growing an enum fails loudly right here."
  Those tests are at `crates/core/src/schema.rs:398-432`, one per vocabulary; a new frozen
  list without its pin leaves the next widening silent.
- **Outbound is one-way, text-only, and forgets what it sent.** `OutboundReply` carries
  channel, text, kind and an optional reply target
  (`crates/core/src/message.rs:373-390`); `deliverable_of` maps a finished assistant text
  block or a report block to one (`crates/core/src/outbound.rs:478-499`); `consume_replies`
  sends and discards (`crates/adapters/telegram/src/driver.rs:730-760`); and `send_body`
  discards the platform's returned `Message`
  (`crates/adapters/telegram/src/client.rs:455-460`). The identifier the two live-location
  editing methods require does not exist anywhere in this process.
- **Sibling specs are converging on one outbound item enum.** Unit 05 changes the outbound
  channel's element type to an `Outbound` enum, `Reply(OutboundReply)` unchanged plus its own
  arms (`docs/units/telegram/05-polls.md:465-478`); unit 06 adopts that name
  (`docs/units/telegram/06-reactions.md:438-441`) and unit 02 builds on it. **This unit adds
  no arm to it**, which is the point of the decision below, and it must not be read as
  disagreement with that shape. The enum does not exist yet, so the absence is a fact about
  the diff and not a property a test can assert.
- **The adapter's tests are source files of the adapter crate.** `crates/adapters/telegram/tests/adapter/`
  holds fourteen modules including `server.rs`, `sending.rs` and `end_to_end.rs`, and there are
  inline `#[cfg(test)]` modules in the sources. A scan for method names across "the adapter"
  would fail on the very tests that assert those methods are never called, which is why AC10
  below fixes its scope to `crates/adapters/telegram/src`.
- **The model sees every group message in helpful mode, with a reportable identifier on it.**
  Decision 0093 composes the moderation teaching exactly when a moderation handle is
  configured and answering is helpful, because "only helpful answering shows the model every
  message it would judge"; decision 0091 puts the stored origin in brackets ahead of every
  user-voiced message so the model can name a report target, validated against the turn's
  co-summoner set. A message this unit records therefore enters the assessment surface. What
  the report carries is fixed: `report_line` is `REPORT_LINE_LEAD` plus the moderation handle
  (`crates/core/src/tools/report.rs:266-268`), and the stored report is the target origin, the
  reported principal and that fixed line (`:98-111`) — no model prose about the message
  reaches anyone.
- **The prompt already carries the rule this unit depends on.** `prompts/30-conduct.md:35-38`
  — "Respect other people's privacy. Do not tag or mention someone on another member's
  behalf … Do not repeat or pass on what one member has said about another". And
  `prompts/30-conduct.md:40-47` — "Do not analyse people. You do not describe anyone's
  personality, character or behaviour, you do not draw conclusions about who someone is from
  what they write, and you do not infer or comment on anyone's health, beliefs, politics,
  religion, ethnicity, sex life or sexual orientation — not when asked, not in passing, not
  as a joke." The teaching states the same discipline for audience reading: read from the
  message and the conversation, "never from a profile of the person"
  (`crates/core/src/teaching.rs:158-164`).
- **Decision 0070 rejects enforcement by prompt alone**, in its own words: "a prompt is
  advice to a model, not a bound on the system. The invariant lives in the mechanisms"
  (`docs/decisions/0070-the-assistant-assesses-a-human-decides.md`). That reasoning transfers
  directly: a rule against inferring somebody's religion is advice while the model holds the
  coordinates of the building they are standing in.
- **Six published statements are in range.** `docs/privacy/records-of-processing.md:61`
  (category D1, message content), `:66` (D6, special categories, incidentally), `:106-112`
  (section 8, erasure concept and time limits, which carries one row per category), `:144`
  (the data-minimisation measure, "Text only, no media, no files, no voice, no stickers, no
  edits") and `:145` (minimisation at the boundary, "no other attribute of a person is
  attached to a request"); `docs/privacy/dpia.md:127-168` (3.2, categories of data, including
  the special-categories paragraph at `:158-168`); `docs/privacy/lia.md:204-245` (section 5,
  the Article 9 condition and the residual it does not reach); and
  `docs/privacy/bot-assistant-privacy-policy.md:20-23` ("We store the text of each message …
  We do not store the media itself, edits, or posts made anonymously").
- **The impact assessment's review trigger fires by name.** "A change to what is collected:
  media, edits, reactions, membership events" (`docs/privacy/dpia.md:566`). Adding a fifth
  collected thing is exactly that trigger, whatever its size.
- **The residual this unit has to reason about already exists in writing.**
  `docs/privacy/lia.md:228-233`: member A posts something sensitive about member B, "B
  published nothing, so Article 9(2)(e) cannot reach it, and no other condition in
  Article 9(2) fits either", and the exposure is "incidental, unsought and undetectable …
  introduced by a third party's act". A contact card is that same shape made deliberate and
  structured, which is why it gets its own paragraph instead of being folded into the
  existing residual.
- **The platform-vocabulary check scans words, not concepts.** `crates/core/tests/vocabulary.rs`
  greps the core's sources for the whole words in `docs/platform-vocabulary.txt`, which holds
  seven platform and SDK names only (`telegram`, `teloxide`, `frankenstein`, `tgbot`,
  `grammers`, `matrix`, `ruma`). All four proposed column values pass it, and a neutral-sounding
  name for a platform concept passes it automatically, so the form vocabulary below has to earn
  its neutrality by argument.
- **There are four manifests, three of which carry dependencies.** `Cargo.toml` at the root is
  a workspace manifest with `members`, `workspace.package` and the two lint tables, and it has
  **no `[workspace.dependencies]` table**; dependencies live in `crates/core/Cargo.toml`,
  `crates/adapters/telegram/Cargo.toml` and `crates/assistant/Cargo.toml`. A criterion that
  says "both manifests" names an inventory that does not exist.
- **The decision-record test reads a fixed list of filenames.**
  `crates/assistant/tests/docs.rs:379-399` iterates six named paths and asserts each contains
  its date line and a `## Rejected alternatives` heading. A criterion in that shape has to name
  its files, which means the numbers must exist before the test does.
- **Decision numbering is contended this week.** `docs/units/telegram/01-receiving-media.md`
  claims 0106 through 0119, and the highest recorded record is
  `docs/decisions/0105-the-fixed-line-is-the-acknowledgments-fallback.md`. Units 02 through
  07 take their decisions in the unnumbered dated form. This unit does the same and assigns
  numbers at merge, by the rule stated in AC13.

## Decisions taken with this unit

- **A shared position, place or contact card becomes a recorded message; its payload does
  not, 2026-08-25.** The message is recorded with an empty text and one new column naming
  what was shared. The coordinates, the accuracy, the heading, the proximity radius, the
  venue title, the venue address, both provider identifier pairs, the contact's phone
  number, first name, last name, account identifier and vCard are **not decoded by the
  adapter at all** — the same technique the display name already uses
  (`crates/adapters/telegram/src/client.rs:212-215`) — so they cannot be stored, logged or
  transmitted by accident. The reason is minimisation with a purpose test behind it: nothing
  in this system reads a coordinate. No tool takes one, no lookup consumes one, the model is
  not shown one, and the report path names its target by the message identifier, so an
  administrator who needs to see a shared position opens the message in the chat where the
  member posted it. A column no code reads is a category the record of processing would have
  to list with a purpose the controller cannot state. *Rejected:* storing the coordinates
  for administrators — the group's own chat is the archive of the group's own messages, the
  assistant is not a second copy of it, and a precise-position table accumulating for every
  member is the single most attractive thing on this machine to anyone who breaks into it.
  *Rejected:* skipping the message entirely and leaving today's behaviour — the message
  happened, it is often the answer to the question above it, and a hole in the conversation
  makes the model reply into a gap it cannot see. *Rejected:* storing a coarsened position
  (a city, a grid square) — a coarsened position is still a position, deriving one needs a
  geographic dataset this machine does not have and would not be allowed to fetch per
  message, and it would put the assistant in the business of computing where people are.

- **The never-decode promise is checked structurally, not by scanning source text,
  2026-08-25.** The first draft asked for a whole-file word scan; that criterion was
  impossible against two live fields and vacuous for six of its thirteen words, as the top of
  this document sets out with receipts. The property is instead proved three ways, each of
  which is exact:
  - **By size.** The marker type for `contact` and the one for `venue` are zero-sized, and the
    one for `location` is exactly the size of `Option<i64>`. A zero-sized type cannot hold a
    phone number, a title or an address; there is no reading of the code under which it does.
  - **By decoded value.** A full update carrying every payload field with recognisable
    sentinel values is decoded, and the `Debug` rendering of the decoded update contains no
    sentinel. `Debug` renders every field a type holds, so its silence is the direct statement
    that no payload became a typed value.
  - **By outcome.** The same update runs through translation and ingestion, and the whole store
    file plus the captured tracing output are searched for each sentinel: none appears.

  *Rejected:* the whole-file word scan — it fails on `ChatInfo.title`
  (`crates/adapters/telegram/src/client.rs:190`) and `BotIdentity.first_name` (`:238`), which
  two shipped features need, and it matches nothing at all for the compound field names given
  `carries_word`'s tokenizer (`crates/core/tests/vocabulary.rs:63-66`). *Rejected:* keeping the
  scan and weakening it to the compound names only — that is the vacuous half; it would assert
  nothing while reading as though it asserted the most important property in the unit.
  *Rejected:* scoping a text scan to the new marker types' source range — the scan would then
  depend on a source-text range that any refactor moves, and the size check states the same
  thing without parsing anything.

- **One neutral column, four frozen values, 2026-08-25.** An appended migration step adds a
  single nullable column `shared_form` to the message content table, in the shape of
  `PROTECTION_STAMP_MIGRATION` (`crates/core/src/schema.rs:159-172`), with its vocabulary
  frozen in the step per `crates/core/src/schema.rs:113-123` **and pinned to the live enum by
  a test beside the three that already exist** (`crates/core/src/schema.rs:398-432`), so a
  fifth value cannot ship without its own appended widening step. The vocabulary is exactly:

  - **`position`** — a point on a map a member shared.
  - **`live_position`** — a point the member's device keeps updating.
  - **`place`** — a named place with an address.
  - **`person_card`** — details of a person, shared as a card.

  Four values, no more. The names are a neutral taxonomy of what a person shares, not a copy
  of the platform's field names: the first three describe a point, a moving point and a named
  place, which every chat platform that carries locations at all distinguishes, and the fourth
  describes a person's details shared as a card, which is a vCard on any platform. An adapter
  for a platform with no live sharing simply never emits `live_position`. *Rejected:* reusing
  unit 01's `attachment_form` column — an attachment is a file with bytes, a media type and a
  size, and a shared position has none of the three; overloading the column would force every
  reader of it to ask which kind of thing it is looking at. *Rejected:* a side table — the
  block loader loads a kind's own content row and nothing else, so a side table would be
  invisible to the projection, exactly as unit 01 found. *Rejected:* one boolean "something
  was shared" — the four cases carry genuinely different sensitivity and the projected line
  differs, so a reader that cannot tell them apart is a reader that has to guess.
  *Rejected:* omitting the frozen-list pin because the vocabulary is closed by decision — the
  pin costs four lines and the discipline's own doc says the failure is the reminder; a
  vocabulary that is closed today and pinned nowhere is a vocabulary the next unit widens in
  the enum alone.

- **The new fact travels as an optional neutral field, added to `stored_fields` as its last
  parameter, 2026-08-25.** `SharedForm` is a core enum with the four values; `InboundMessage`
  gains `shared_form: Option<SharedForm>` beside `command`; `Pending` gains the same field;
  `ChatMessage::stored_fields` gains a sixth positional parameter, `Option<SharedForm>`, in
  last position, and `ChatMessage::parse` reads the column back. Every other call site passes
  `None`. The reach is named because it is wide: one production call
  (`crates/core/src/assembly.rs:725`), four in-module test calls
  (`crates/core/src/kind.rs:1147+`, `crates/core/src/outbound.rs:638`,
  `crates/core/src/tools/provenance.rs:392,424`) and five suite call sites
  (`crates/core/tests/spine/group_context.rs:665`,
  `crates/core/tests/spine/protection.rs:162,802`,
  `crates/core/tests/spine/speaker.rs:129,503`). *Rejected:* folding the form into
  `RecordedSender` or into `Stamp`, the two existing grouping parameters — the form is a
  property of the message, not of the sender and not of the answering decision, and putting it
  in either would make both mean two things. *Rejected:* a builder or an options struct to
  avoid touching the call sites — the signature is deliberately positional so a column rename
  cannot split the encode and decode halves (`crates/core/src/kind.rs:436-443`), and swapping
  it for a builder is a change to a shared shape that this unit has no reason to make.

- **A place is tested before a position, and the alias is the reason, 2026-08-25.** The
  documented alias — "when this field is set, the location field will also be set" — means
  the dispatch order is a correctness property, not a style choice, and it is pinned by its
  own criterion. This is one rule, applied here to one pair, and it is the same rule unit 01
  applies to two other pairs. *Rejected:* an order that happens to work because of the field
  order in a struct — the next person to reorder the struct silently records every place as a
  position, and no test would have said so.

- **The model is told what was shared and never what it was, 2026-08-25.** The projection
  composes one line from the stored form and places it **inside** the text, ahead of any
  words, because `projected_text` wraps the whole speaker line in the origin mark from the
  outside (`crates/core/src/kind.rs:555-569`). The full wording, given here because the exact
  projected string is a criterion:

  | form | projected line |
  | --- | --- |
  | `position` | `[shared a position, not shown to the assistant]` |
  | `live_position` | `[shared a live position, not shown to the assistant]` |
  | `place` | `[shared a place, not shown to the assistant]` |
  | `person_card` | `[shared someone's contact card, not shown to the assistant]` |

  The line is the text; the composition around it is the existing one, unchanged, which means
  there are two projected shapes and not one. A sender with a stored handle projects
  `[id-9] alice: [shared a position, not shown to the assistant]`; a handleless sender stores
  NULL by decision 0056 and projects `[id-9] [shared a position, not shown to the assistant]`.
  An erased row never reaches this function — `projected_text` returns `ERASED_MARKER` from
  its first line when `text` is `None` — so no shared line survives erasure and no extra
  branch is needed for it. *Rejected:* projecting the venue title and address, which are the
  least sharp of the four and the most conversationally useful — one rule with no exceptions
  is what makes the member-facing statement short and true, and a place a member shares still
  says where that member is or intends to be. *Rejected:* projecting the live period, so the
  model knows for how long the sharing runs — it is a number about how long a person will
  broadcast their movements, no behaviour reads it, and the model does not need it to answer
  anything. *Rejected:* saying nothing to the model, leaving an empty turn — silence reads as
  "nothing was sent", which is false, and an addressed member would get an answer to a message
  the assistant appears not to have received. *Rejected:* enforcing this in the prompt while
  the payload still travels — decision 0070 already rejected enforcement by prompt alone, and
  the never-decode rule is what turns "do not analyse people" from advice into a property of
  the system.

- **The recipients of the record of processing do not change, and the reason is stated
  precisely, 2026-08-25.** The first draft claimed "nothing new reaches the model provider".
  That is false as written: the bracketed line is new content, composed by this unit, and it
  travels to the provider with the rest of the conversation. The true claim is narrower and is
  the one the documents will carry: **the set of recipients is unchanged, because the line is
  conversation text and the provider already receives the conversation's text**
  (`docs/privacy/records-of-processing.md:81`). Two published sentences are checked against it
  rather than assumed: the data-minimisation measure at `:144` gains coordinates, addresses and
  contact details among what is not kept, and "Minimisation at the boundary" at `:145` — "no
  other attribute of a person is attached to a request" — is re-read and, if the shared line
  reads as an attribute of a person, amended in the same commit to say what does travel. This
  unit's position is that the line is a fact about a message, the same kind of fact as its
  speaker prefix, and the amendment is a clarification rather than a new category; the
  criterion requires the sentence to be checked either way rather than left to stand on that
  reading alone. *Rejected:* leaving `:145` untouched on the argument that it is about
  identifiers — the sentence says "no other attribute of a person", not "no other identifier",
  and a published sentence that has to be read charitably to stay true is a defect.

- **A shared item is assessable like any other message, and the honest limits are written
  down, 2026-08-25.** After this unit the four forms enter the surface decision 0093 describes:
  in helpful mode with a moderation handle configured, the model judges every group message
  against the pinned rules, and decision 0091 puts a reportable identifier in front of each
  one. Three consequences, all decided rather than discovered:
  - **A shared-item message can be named in a report.** It is not excluded from the
    co-summoner set. What the report contains is the target origin, the reported principal and
    a fixed line (`crates/core/src/tools/report.rs:98-111,266-268`) — no model prose about the
    message, and therefore no claim about a payload the model was not shown. The administrator
    opens the message in the chat and sees the card or the pin for themselves. That is exactly
    the human decision point decision 0070 requires, and it is why the visibility is safe.
  - **The assistant cannot assess the payload, and says so.** A contact card is precisely what
    a no-doxxing rule targets, and the assistant is permanently unable to tell whether a
    particular card carries a stranger's number. This is a consequence of the storage decision
    above, not a shortfall to close: the alternative is holding the phone number.
  - **The prompt is taught the difference.** The passage below tells the model that a bracketed
    shared line is a fact about a message and not its contents, so a judgement about the
    contents is one it cannot make; it may name such a message for a human to look at when the
    surrounding conversation gives a reason, and never on the form label alone.

  *Rejected:* excluding shared-item messages from the assessment set so they can never be
  reported — a contact card posted to dox somebody would then be the one message in the group
  no administrator ever hears about, and the mechanism would be a silence dressed as a
  protection. *Rejected:* letting the report carry a sentence about what was shared — the
  report line is fixed by decision, and a model-composed sentence about a payload it was not
  shown is exactly the ungrounded claim decision 0096 forbids. *Rejected:* leaving this
  unstated because "the mechanism does not change" — the mechanism does not change and the
  surface does; an unwritten widening of what the assistant judges is the kind of thing
  decision 0070 exists to make explicit.

- **The special-category reasoning, taken deliberately and not left to the incidental
  paragraph, 2026-08-25.** A position is not itself Article 9 data. It is, reliably, the
  route to it: a position is a building, and a building is a hospital, an addiction clinic,
  a mosque, a synagogue, a party office, a union hall or a gay bar. The inference needs no
  cleverness and no dataset — it needs a model with world knowledge and a coordinate, and it
  runs in one sentence. The existing assessment carries incidental special-category exposure
  as content a member wrote about themselves in free text
  (`docs/privacy/records-of-processing.md:66`, `docs/privacy/dpia.md:158-168`), with
  Article 9(2)(e) claimed for the self-posted part on the fact that the groups are publicly
  readable (`docs/privacy/lia.md:204-227`). Carrying coordinates would change the character
  of that exposure in two ways the existing analysis does not cover: it would be structured,
  not incidental, and it would be **detectable** — the assessment's own defence is
  that the assistant "does not seek such data and cannot detect it", and a coordinate handed
  to a general-purpose model is data it can detect. Not decoding the payload keeps that
  defence true instead of amending it into something weaker. *Rejected:* carrying the payload
  and amending the assessment to describe the new risk honestly — an accurate description of
  an avoidable risk is still an avoidable risk, the mitigation here costs one struct field
  that is never written, and the review trigger at `docs/privacy/dpia.md:566` would fire for
  a capability nothing in the product asked for.

- **A contact card is another person's data and is treated as such, 2026-08-25.** The person
  named on a shared card never joined the group, was never given the Article 13 notice
  (`docs/privacy/bot-assistant-privacy-policy.md`), has no message of their own in the ledger,
  and — decisively — **has no erasure route**: erasure keys on a principal in the identity
  tables (`crates/core/src/erasure.rs:1-50`), and a phone number stored on a message row
  belongs to somebody who is not a principal and never becomes one. Storing it would create a
  data subject the controller can neither inform under Article 14 nor serve under Article 17
  by mechanism. The card's payload is therefore never decoded, and the ledger records only
  that a card was shared. *Rejected:* storing the payload and serving such a person by hand
  within the month, as the policy already promises for a message about somebody else — that
  promise works because a message can be found by its author and its text; a phone number
  belonging to a stranger cannot be found by anyone who does not already know it, so the
  promise would be unkeepable in the one case it was written for. *Rejected:* storing the
  card's platform account identifier only, on the argument that it is opaque — it is that
  person's account, opaque to us and not to the platform, and it is the join key that turns a
  set of cards into a social graph.

- **A live position is a fact recorded once, and the movement is never followed,
  2026-08-25.** `Skip::EditedMessage` stays exactly as it is
  (`crates/adapters/telegram/src/translate.rs:123-125`), and `edited_message` stays in
  `CONSUMED_UPDATE_TYPES` (`crates/adapters/telegram/src/client.rs:103`). Whether the
  platform emits an edit update per movement is undocumented, and this decision makes the
  answer irrelevant: either it does and we skip it by a named rule, or it does not and there
  was never anything to skip. What a following mechanism would produce is the thing this
  whole unit exists to prevent — a stored track of where a person went, updated for up to a
  day, or forever under `0x7FFFFFFF`. *Rejected:* consuming edit updates for live positions
  only — it is a track whatever the trigger is called, and it would arrive with no member
  ever having addressed it to the assistant. *Rejected:* dropping `edited_message` from the
  consumed update types to make sure nothing arrives — the skip is what names the case in a
  log line, decision 0017 is what it restates, and a narrower subscription would silently
  change the edit behaviour of every later unit that wants edits for another reason.

- **The proximity alert is a named skip, placed at line 159, 2026-08-25.** A new
  `Skip::ProximityAlert` is returned immediately **before** the on-behalf-of-chat check at
  `crates/adapters/telegram/src/translate.rs:159`, which puts it after the chat-kind match and
  after the pin block and ahead of both sender checks. The placement is stated as a line and
  not as "ahead of the sender check", because there are two sender checks and the choice
  between them decides the outcome. The consequences of that exact placement, stated: a
  proximity alert in a broadcast channel still returns `Skip::ChannelBroadcast`, which is
  right because no channel is served at all; a proximity alert in a group returns
  `Skip::ProximityAlert` whether or not the platform sets `from` on it, which is the point —
  today's outcome depends on a platform detail nobody has verified. Its reason line is "a
  service note that two members came near each other". The fact itself is a distance in metres
  between two named members, measured by the platform, addressed to nobody and asked for by
  neither — the one member who set the alert asked their own device, not this assistant.
  *Rejected:* recording it as a group observation the way a pin is recorded — a pin is the
  group's published governance, and this is two people's bodies. *Rejected:* relying on the
  existing text skip — it would leave the decision unwritten and the reason unsaid, and the
  log line would call a service note about two members "a message with neither text nor
  caption".

- **The textless skip widens, and unit 01's list loses two of its named cases, 2026-08-25.**
  Whichever of this unit and unit 01 merges second, the merged tree holds one skip meaning "a
  message with neither text, caption, a file this adapter carries, nor a shared form this
  adapter records". Unit 01 names that skip `Skip::NothingToRecord`, and this unit adopts the
  name in either order. The part that is not additive is written out here so a merge cannot
  miss it: **decision 0106's skip list loses `a contact` and `a location`, and unit 01's AC2
  loses its `a location` pin.** The remaining cases in that list — a captionless sticker, a
  forwarded story, a poll, dice, a captionless paid-media post and a `rich_message` — are
  unaffected by this unit. If this unit merges first, unit 01 reads the merged text and drops
  those two from its own decision and criterion before it lands; if unit 01 merges first, this
  unit's implementer edits decision 0106's record and unit 01's criterion in the same commit
  that records a location, and says so in the commit body. *Rejected:* leaving the two specs
  to be read as compatible because the rename is compatible — they are not compatible, and a
  merge that only reconciles the rename ships a decision record that describes behaviour the
  tree does not have. *Rejected:* keeping a separate skip variant for shared items so neither
  list has to change — two variants meaning "nothing to record" is the bolted-on conditional
  the engineering standard tells us to refactor away, and a reader of the log could not say
  why one message got one name and another got the other.

- **The assistant sends no position, live position, place or contact card, 2026-08-25.** No
  arm is added to the outbound item enum unit 05 introduces, no tool is registered, and
  `sendLocation`, `editMessageLiveLocation`, `stopMessageLiveLocation`, `sendVenue` and
  `sendContact` are not implemented in the client. Four reasons, one per method family, so
  none of them has to carry the others:
  - *A position from this assistant would be a fabrication.* The process has no position and
    no map. Any coordinate would come from the model's memory, which decision 0096's rule —
    a substantive claim must be one a lookup can back — already forbids, and a wrong
    coordinate does not read as a sentence a member can weigh: it renders as a pin.
  - *A live position would be a standing false claim of physical presence*, refreshed for up
    to a day or, under `0x7FFFFFFF`, indefinitely.
  - *The two editing methods cannot be called from this tree*, because the identifier they
    require is discarded at `crates/adapters/telegram/src/client.rs:455-460`. Shipping them
    would mean widening the send path to retain message identifiers for a capability the
    first two reasons already refuse.
  - *A contact card would hand out a third party's phone number* on the say-so of a language
    model with no phone book, to a public group, irreversibly. It is the clearest possible
    case of the prompt's own rule at `prompts/30-conduct.md:35-38` against passing on what
    concerns another member, and a mechanism is the only place that rule can actually hold.

  The five method names are not the whole surface. Six inline classes produce the same three
  message kinds without naming any of them — `InlineQueryResultLocation`,
  `InlineQueryResultVenue`, `InlineQueryResultContact`, `InputLocationMessageContent`,
  `InputVenueMessageContent` and `InputContactMessageContent` — and they are out of reach only
  because `docs/units/telegram/08-inline-queries.md` refuses inline mode outright. **This
  decision therefore depends on unit 08's refusal**, and a later unit that reverses it must
  extend AC10's list to the six classes in the same change. Stated here so the dependency is a
  written one rather than an accident of scope.

  *Rejected:* a send capability limited to administrators — decision 0070 puts the human
  decision point in the mechanism, and there is no administrator command surface here; an
  administrator who wants to post a place posts it themselves in one tap, so the capability
  would add a fabrication risk and no reach. *Rejected:* sending a place for community events,
  which is the one plausible caller — the event's organiser posts it, the assistant has no
  event calendar to read one from, and a wrong address for a real meetup sends people to a
  real wrong door. *Rejected:* implementing the methods now and leaving them uncalled — a
  capability with no caller is dead code that the next reader takes as permission, which unit
  02 states as its own rule (`docs/units/telegram/02-sending-media.md:271-273`).
  *Rejected:* asserting "no arm was added to the outbound item enum" as a test — the enum does
  not exist in `main` yet, and "gained no arm in this unit" is a property of a diff, not of a
  tree; no assertion can express it, and any pinned arm list breaks the moment unit 05 or 06
  merges. It moves to the review of the change, named in the notes.

- **No new personal column means no new erasure work, and that is the test, 2026-08-25.**
  `shared_form` is structure in the same sense the protection stamp is structure
  (`crates/core/src/schema.rs:155-158`): erasure leaves it, the text, origin, send time,
  reply target and speaker nulls already reach everything personal on the row, and the
  projection shows only `ERASED_MARKER` afterwards. The residue an erased row leaves is
  stated plainly and not glossed: somebody, no longer identifiable through this store,
  shared a position at a position in this conversation, and nothing about who or where. This
  is the same residue unit 01 accepts for an erased media message. The absence of an erasure
  pass is how the storage decision above is checked — a feature that needed one would be a
  feature that stored personal data after saying it did not. *Rejected:* nulling
  `shared_form` on erasure for symmetry — it would remove the one fact that keeps the
  conversation's shape honest and would suggest the column had held something personal.

- **The prompt is taught what a shared line means, and the no-analysing rule reaches where a
  person is, 2026-08-25.** `prompts/30-conduct.md` gains a short passage in the privacy
  paragraph at `:35-38`: a bracketed shared line is the assistant's own note that a member
  shared a position, a place or somebody's contact card; the assistant is not shown the
  coordinates, the address or the details and cannot work them out; it must not describe,
  name, guess at or reason about where anyone is or was, and it does not ask a member to
  share a location. If asked, it says plainly that it is not shown that. The passage also
  carries the assessment clause from the decision above: the line says a message happened, not
  what it contained, so the assistant does not judge the contents, and it names such a message
  for a human only when the conversation around it gives a reason. And the existing "Do not
  analyse people" rule at `:40-47` is extended by one clause, because a position is precisely
  the material from which the inferences that rule already forbids get made: where someone is,
  where they have been and who they are near are not things the assistant infers, comments on
  or is drawn into. *Rejected:* leaving the bracket convention to be self-evident — the prompt
  teaches every other convention explicitly, and a model handed
  `[shared a position, not shown to the assistant]` as an addressed member's whole turn will
  otherwise answer as though it saw a map. *Rejected:* a separate prompt file for it — this
  is one clause of the conduct the file already carries, and splitting it would leave two
  places to keep in agreement.

- **The group's own attached place is not collected, 2026-08-25.** `ChatLocation` is the
  group's property, not a member's position, and the channel lookup already reads only the
  title and the pinned announcement. A group's address would be a new stored category with no
  reader and would sit beside the pinned rules as if it were governance. *Rejected:* storing
  it as a context note for the model, which is where a group fact would naturally go — no
  answer the assistant gives depends on it, and the operator can put an address in the pinned
  rules if the group wants one.

- **Nothing streams, because nothing is fetched, 2026-08-25.** This feature moves no bytes at
  any point: the four objects arrive inside the update body the poll already reads, there is
  no `getFile`, no download, no upload and no multipart body, and the payload fields are
  dropped by the decoder before any of them reaches a variable. The standing streaming rule
  is satisfied by there being no stream. The one place it will apply nearby is poll media,
  which carries `location` and `venue` on the receiving side as well as the sending side —
  noted for unit 05 in the notes below, not built here.

## The unit's contract

A message that shares a position, a live position, a place or a contact card is recorded
like any other message, with an empty text and one column naming which of the four it was;
only a message that shares nothing this adapter carries and has no text is skipped, and the
proximity-alert service note is a named skip with its own reason, returned at a named line.
The payload — coordinates, accuracy, heading, proximity radius, venue title, venue address,
both venue-provider identifier pairs, phone number, first and last name, account identifier
and vCard — is never decoded by the adapter, proved by the decoding types' sizes and by a
sentinel that appears in no decoded value, no stored row and no log line; the platform's own
message stays in the chat where the member posted it, which is where an administrator reads
it. A live position is recorded once as a fact and its movement is never followed: the edit
skip stays, by decision, whether or not the platform emits an update for it. The model reads
one bracketed line naming what was shared and stating that it was not shown the contents, in
whichever of the two existing projected shapes the sender's stored handle produces, and the
prompt teaches it not to guess where anyone is, extends the existing no-analysing-people rule
to a person's whereabouts, and tells it that a shared line is a fact about a message rather
than its contents. Such a message enters the assessment surface like any other group message
and can be named in a report, whose content is the fixed line and the target identifier, so an
administrator reads the original and decides. The recipients of the processing record are
unchanged, because the bracketed line is conversation text and the provider already receives
the conversation's text. The assistant sends no position, live position, place or contact
card; the two live-location editing methods remain unreachable because the send path still
discards the identifier they need; and the six inline classes that produce the same messages
stay out of reach only for as long as unit 08's refusal of inline mode stands. Erasure gains
no new pass because the unit stores nothing personal that the existing nulls do not already
reach, and the recorded form survives erasure as structure. The record of processing, the
impact assessment, the legitimate-interest assessment and the member-facing policy state what
is now collected and, more importantly, what is deliberately not. No new dependency, no bytes
move, and the assistant still takes no action against anyone: it assesses and a human decides
(decision 0070 held to, with its surface widened deliberately and in writing).

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary check and the secret scan clean; **no new dependency**, pinned by a test
  that reads the three dependency-bearing manifests — `crates/core/Cargo.toml`,
  `crates/adapters/telegram/Cargo.toml` and `crates/assistant/Cargo.toml` — and asserts each
  one's dependency name set equals a list committed with the test. The root `Cargo.toml` is a
  workspace manifest with no `[workspace.dependencies]` table and is asserted to have none, so
  a dependency cannot appear there instead.
- **AC2** The four forms record. An update carrying `location` with no `live_period` records
  `position`; the same with `live_period` present records `live_position`; an update carrying
  `venue` records `place`; an update carrying `contact` records `person_card`. Each produces
  `IngestOutcome::Recorded` with an empty stored text. Pinned per form. This criterion is the
  one that collides with unit 01: the merged tree cannot also pin `Skip::NothingToRecord` for
  a location, and the reconciliation decided above — decision 0106's list loses `a contact`
  and `a location`, and unit 01's AC2 loses its `a location` pin — is performed in whichever
  commit merges second and named in its body.
- **AC3** The alias ordering holds: an update with **both** `venue` and `location` set — the
  shape the documentation says the platform always sends — records `place`, never `position`.
  Pinned as its own case, with a comment naming the documented alias.
- **AC4** Nothing else records. A proximity-alert service message in a group returns
  `Skip::ProximityAlert` with its own reason line, pinned, and the same message in a broadcast
  channel still returns `Skip::ChannelBroadcast`, pinned, because the placement at
  `crates/adapters/telegram/src/translate.rs:159` decides that. A message with neither text,
  caption nor any of the four shared objects still returns the textless skip. A pinned shared
  position still returns `Skip::TextlessPin`, pinned, because the pin path gains a fourth way
  to be textless and gains no branch. An `edited_message` carrying a `location` with
  `live_period` returns `Skip::EditedMessage` and reaches no ledger write, pinned explicitly
  because that is the movement case.
- **AC5** The payload cannot enter the process, checked three ways and by no source-text scan.
  First, `size_of` of the venue marker type and of the contact marker type is 0, and
  `size_of` of the location marker type equals `size_of::<Option<i64>>()` — pinned, so no
  refactor can widen them without failing here. Second, a full update carrying every payload
  field named in the storage decision with recognisable sentinel values decodes successfully,
  and the `Debug` rendering of the decoded update contains none of the sentinels. Third, that
  same update runs through translation and ingestion, and the whole store file plus the
  captured tracing output are searched for each sentinel: none appears. The whole-file word
  scan the first draft asked for is deliberately not implemented, for the reasons in the
  decision above; a comment at the test says so, naming `client.rs:190` and `client.rs:238`,
  so nobody re-adds it.
- **AC6** The stored row is the form and nothing more. Two messages are ingested from the same
  sender into the same conversation at the same recorded send time, one a plain text message
  and one a shared position; their stored field maps are compared and differ in exactly three
  entries — `shared_form`, `text` and `origin`, the platform message identifier, which differs
  between any two messages by construction. Repeated per form. A migration test asserts the
  CHECK constraint refuses a fifth value, and a test beside those at
  `crates/core/src/schema.rs:398-432` pins the step's frozen four-value list to the live
  `SharedForm` enum.
- **AC7** The model reads one honest line, pinned by byte equality against the four strings in
  the projection decision. Both projected shapes are pinned: a sender with a stored handle
  projects `{origin mark} {speaker}: {line}` and a handleless sender projects
  `{origin mark} {line}`, per decision 0056 and `crates/core/src/kind.rs:559-563`. An erased
  row projects only `ERASED_MARKER` and no shared line. Each of the four strings is asserted to
  contain no digit, which is the mechanical form of "it names no coordinate and no number";
  the byte-equality pins carry the rest.
- **AC8** Addressing and answering behave as they do for any other message: a shared position
  in a group with no reply to the assistant is recorded and owes no turn; the same message
  sent as a reply to one of the assistant's own messages is recorded, owes a turn, and the
  assistant's answer is delivered — proving an empty text is a real message end to end. A
  recorded shared-item row returns `false` from `erased()` and `true` from `owes_answer()`
  when addressed, pinned, because an empty text is now a shape the tree produces and
  `crates/core/src/kind.rs:498-503` used to say it never was.
- **AC9** Erasure needs no new pass and loses nothing: after `erase_principal` a shared-item
  message's text, origin, send time and speaker are NULL, `shared_form` still holds its
  value, the row projects the erased marker alone, and a second erasure reports completion.
  Pinned for a group message, and pinned for a direct conversation with `DirectChats::On`
  configured — the default per decision 0069 — whose rows are removed whole.
- **AC10** The sending methods are absent, pinned and not assumed: a source scan over
  `crates/adapters/telegram/src` **only**, with the method names held in a committed data file
  the test itself does not spell — the technique `crates/core/tests/vocabulary.rs:9` uses and
  states — asserts that `sendLocation`, `editMessageLiveLocation`, `stopMessageLiveLocation`,
  `sendVenue` and `sendContact` appear in no source file there. The adapter's own test tree is
  out of scope by construction, because the scripted-server assertion has to spell the same
  names. Separately, the scripted server sees none of those calls across the whole adapter
  suite. Whether the change added an arm to the outbound item enum is a question for the review
  of the diff and not a criterion, for the reason recorded in the sending decision.
- **AC11** The prompt carries the passage and keeps what was there: `prompts/30-conduct.md`
  contains the shared-line teaching, the assessment clause and the whereabouts clause; the
  existing "Do not analyse people" sentence is retained verbatim, and the privacy paragraph at
  `:35-38` is retained verbatim. Pinned by a test in the shape of the existing
  prompt-composition tests.
- **AC12** The published privacy documents match the running system in the same commit.
  - The record of processing gains a category for the shared-item form in section 5, stating
    that the payload is not collected, **and a row for that category in section 8, "Erasure
    concept and time limits"** (`docs/privacy/records-of-processing.md:106-112`), which carries
    one row per category and is where a reader looks for the fact that the form survives
    erasure as structure.
  - Its data-minimisation measure at `:144` names coordinates, addresses and contact details
    among what is not kept, and "Minimisation at the boundary" at `:145` is re-read against the
    bracketed line and amended if it does not stay true as written.
  - The recipients section is asserted unchanged, and the assertion carries the reason from the
    decision above — the line is conversation text, and `:81` already lists the conversation's
    text as what the processor receives.
  - The impact assessment gains a dated addendum under its "change to what is collected"
    trigger (`docs/privacy/dpia.md:566`) covering the four forms, the deliberate
    non-collection, the special-category-by-inference reasoning, the third-party data subject a
    contact card names, and the widened assessment surface.
  - The legitimate-interest assessment's Article 9 section gains the paragraph distinguishing a
    position from Article 9 data and stating why the residual does not grow.
  - The member-facing policy says in plain words that sharing a location, a place or a contact
    records only that it happened.
  - `docs/compliance/ai-act.md` is checked and stated unchanged, because the recipients and the
    model's role are unchanged.
- **AC13** The record of the work exists. Each decision above is written into `docs/decisions`
  with `Date: 2026-08-25` and a `## Rejected alternatives` section, pinned by a test in the
  shape of `crates/assistant/tests/docs.rs:379-399`, which names its files explicitly. The
  sixteen decisions to be recorded are, in the order they appear above: the payload is not
  recorded; the never-decode promise is checked structurally; one neutral column with four
  frozen values; the neutral fact travels as an optional field; a place is tested before a
  position; the model is told what was shared and never what it was; the recipients do not
  change and the reason is precise; a shared item is assessable and the limits are written
  down; the special-category reasoning; a contact card is another person's data; a live
  position is recorded once; the proximity alert is a named skip at line 159; the textless skip
  widens and unit 01's list loses two cases; the assistant sends none of the four; no new
  personal column means no new erasure work; the prompt teaching. Numbers are assigned at merge
  from the first number free after every sibling already merged — unit 01 claims 0106 through
  0119 and the highest recorded record is 0105 — and the filenames the docs test names are
  written once the numbers are fixed, in the same commit. Every follow-up named below is
  appended to `docs/follow-ups.md` naming this unit.

## Notes for launch

- Branches from `main` into its own worktree; builds against the agent-ledger checkout as it
  stands. **This unit needs no framework change** and adds no dependency, no file handling and
  no wire method.
- Adapter sites:
  - `crates/adapters/telegram/src/client.rs:124-144` — `Incoming` gains four presence-only
    fields, decoded as narrowly as possible so no payload becomes a typed value. One marker
    type per object, holding only what the form rule needs: for `contact` and `venue` an empty
    struct, **written `struct VenueMarker {}` with braces, not `struct VenueMarker;`** — a unit
    struct deserializes from `null` only and would refuse the platform's object, and a decode
    refusal rejects the whole update batch (`crates/adapters/telegram/src/client.rs:570-573`);
    for `location` a struct with the single field `live_period: Option<i64>`, read as a
    discriminator and never stored.
  - `crates/adapters/telegram/src/translate.rs:53` and `:481` — the `ProximityAlert` variant and
    its reason line. `:159` — the skip's placement, immediately before the on-behalf-of-chat
    check, commented with the reason it is not further down. `:165-167` — the textless skip
    becomes the catch-all after the shared-item dispatch, with `venue` tested ahead of
    `location`. `:79-114` — `Pending` gains the neutral form field.
  - `crates/adapters/telegram/src/driver.rs:425-436` — the `Pending` to `InboundMessage`
    construction, where the field crosses into the core. Not named in the first draft and
    without it the column is never written.
  - `crates/adapters/telegram/src/client.rs:103` and
    `crates/adapters/telegram/src/translate.rs:123-125` are read and left alone, which is a
    decision above and should be commented as one at the skip.
- Core sites:
  - `crates/core/src/message.rs` — the `SharedForm` enum with the four values, and
    `shared_form: Option<SharedForm>` on `InboundMessage` beside `command`.
  - `crates/core/src/kind.rs:444-485` — `stored_fields` gains its sixth positional parameter;
    `:555-569` — the line composed inside the text; `:575-597` — the descriptor column;
    `:376-431` — the struct's own field; **`:598-637` — `parse`, the read-back half, without
    which the column is written and never seen**; `:498-503` — the `erased()` doc comment whose
    middle clause ("the adapter never records an empty message") stops being true and is
    rewritten to say that an empty text is a recorded shared item while a null text is erasure.
  - `crates/core/src/assembly.rs:725` — the one production call of `stored_fields`, which passes
    the message's form through.
  - Test call sites that the signature change reaches, all passing `None`:
    `crates/core/src/kind.rs:1162,1418,1479`, `crates/core/src/outbound.rs:638`,
    `crates/core/src/tools/provenance.rs:392,424`,
    `crates/core/tests/spine/group_context.rs:665`,
    `crates/core/tests/spine/protection.rs:162,802`,
    `crates/core/tests/spine/speaker.rs:129,503`.
  - `crates/core/src/schema.rs` — one appended step in the shape of `:159-172` with the
    vocabulary frozen per `:113-123`, plus its pin test beside those at `:398-432`.
  - `crates/core/src/erasure.rs` is read and left alone, which is also a decision above.
- Prompt and documents: the passage in `prompts/30-conduct.md` beside `:35-38` and the clause
  at `:40-47`; the four privacy documents named in AC12, including the erasure table at
  `docs/privacy/records-of-processing.md:106-112`.
- **For the review of the diff, not for a test:** that no arm was added to the outbound item
  enum, and that no marker type grew a field. Both are diff properties; AC5's size pins cover
  the second from the other side.
- **Cross-unit ordering, stated because seven sibling specs were written the same week and
  this one is not free to edit them.**
  - `docs/units/telegram/01-receiving-media.md` renames `Skip::NoText` to
    `Skip::NothingToRecord`, widens its meaning, and makes an empty stored text a normal
    recorded message. **This unit needs the same seam, and the two specs also contradict each
    other** — see the collision section at the top and the decision that resolves it. If unit 01
    merges first, this unit reuses the renamed skip and the empty-text path, adds the four
    shared forms to the catch-all's meaning, and edits decision 0106's skip list and unit 01's
    AC2 to drop `a contact` and `a location`; if this unit merges first, it makes the same seam
    under its own name and unit 01 widens it and drops those two cases before landing. Whichever
    merges second reads the merged text, never the drafted text. The two units add separate
    columns and separate appended migration steps, which do not conflict, but the second step
    must be appended after the first in the merged step list.
  - Unit 01 also rewrites `docs/privacy/records-of-processing.md:61,144`,
    `docs/privacy/dpia.md` 3.2 and `docs/privacy/bot-assistant-privacy-policy.md:20-22`; unit
    03 rewrites the same policy sentence again. This unit touches the neighbouring sentences
    in all four documents and adds a row to the erasure table at `:106-112`. Same rule: read
    the merged text.
  - `docs/units/telegram/08-inline-queries.md` refuses inline mode, and that refusal is what
    keeps the six inline classes out of reach. A unit that ever reverses it must extend AC10 to
    `InlineQueryResultLocation`, `InlineQueryResultVenue`, `InlineQueryResultContact`,
    `InputLocationMessageContent`, `InputVenueMessageContent` and
    `InputContactMessageContent`, and must decide what to do with `InlineQuery.location`
    (`docs/units/telegram/08-inline-queries.md:98`), which is a fifth inbound route to a
    member's coordinates.
  - `docs/units/telegram/05-polls.md:465-478` introduces the `Outbound` item enum and unit 06
    adopts it. This unit deliberately adds no arm and depends on neither; it names the enum
    only so the absence reads as a decision.
  - `docs/units/telegram/05-polls.md:628-638` scopes poll media out and lists its families as
    "the photo, video and sticker forms". `PollMedia`'s own field list includes `location` and
    `venue`, and so do `InputPollMedia` and `InputPollOptionMedia`. So poll media is a route to
    a position or a place **in both directions**: when it ships, the sending decision above
    applies to a poll option carrying a place, and the receiving side needs the same
    never-decode treatment this unit gives an ordinary message. Recorded here instead of edited
    into that spec.
- **Review findings answered and not adopted.** Each was checked against the tree before being
  set aside, and the receipt is given so the next reader does not re-open it blind.
  - *That `stored_fields` has three other production callers.* It does not:
    `crates/core/src/outbound.rs:638` and `crates/core/src/tools/provenance.rs:392,424` sit
    inside `#[cfg(test)]` modules (`outbound.rs:501`, `provenance.rs:290`). The reach is real
    and is listed above; it is a test-surface reach with one production call.
  - *That a shared-item message lets the model make an ungrounded claim about a member in a
    report.* The report carries no model prose: it is a target origin, a principal identifier
    and a fixed line (`crates/core/src/tools/report.rs:98-111,266-268`). The visibility is
    accepted deliberately in its own decision above, with the limitation written down; the
    ungrounded-claim reading does not survive the report's actual shape.
  - *That `ChatLocation`'s collection, the group's own attached place, should be revisited
    here.* It is a decision above with its rejected alternative and a named follow-up; nothing
    in the reviews changed the reasoning.
- Named follow-ups, recorded and not built: a member who shares a position may be sharing
  someone else's home, and no mechanism here detects that (an administrator reads the message
  in the chat, which is the human decision point decision 0070 asks for, and nothing about
  this unit changes it); the assistant cannot answer "how far is that from me", and the
  honest reason is that it has no geographic lookup, which is a tool decision with its own
  data-protection question and not a location decision; the group's own attached place is not
  collected, which a later unit could revisit if a group ever asks; poll media will need the
  streaming answer this unit did not have to give, on the receiving side as well as the
  sending side; and whether the platform emits an edit update when a live position moves is
  undocumented — if a later unit ever needs to know, it is answered by observation against a
  real chat and written down, not assumed.
