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
from both repositories.

## The finding that shapes the whole unit, stated at the top

The obvious reading of this feature is "carry location and venue and contact, in and out".
Three separate facts break that reading, and each one is checkable:

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

Nothing here is a shortfall to close later. Points 1 and 2 are decisions with their reasons
below; point 3 is an undocumented platform behaviour, named as undocumented and designed
around instead of assumed.

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
  *ephemeral_message_parameters*." The receiving side of this feature has not changed since
  7.3 (6 May 2024), which "Added support for live locations that can be edited indefinitely,
  allowing 0x7FFFFFFF to be used as *live_period*" and "Added the parameter *live_period* to
  the method *editMessageLiveLocation*".
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
- **No update is documented for a live position moving.** `Update.edited_message` is "New
  version of a message that is known to the bot and was edited. This update may at times be
  triggered by changes to message fields that are either unavailable or not actively used
  by your bot." Whether a member's movement produces one is stated nowhere in the
  documentation or the changelog. The default `allowed_updates` is "all update types except
  *chat_member*, *message_reaction*, and *message_reaction_count*", so an edit update would
  arrive if one is produced; this adapter asks for `edited_message` explicitly
  (`crates/adapters/telegram/src/client.rs:103`) and skips it.
- **`InputMediaLocation` and `InputMediaVenue` exist since 10.0 and are not a second way to
  send a message.** The `InputMedia` union is animation, audio, document, live photo, photo
  and video only; the location and venue forms belong to `InputPollMedia` ("the content of a
  poll description or a quiz explanation") and `InputPollOptionMedia`. The changelog lists
  them under the Polls heading beside `PollMedia`. So a poll option can carry a place, and
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
- **The edit skip is already a named case.** `translate` returns `Skip::EditedMessage` for
  any `edited_message` (`crates/adapters/telegram/src/translate.rs:123-125`), restating
  decision 0017. `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]`
  (`crates/adapters/telegram/src/client.rs:103`), and the doc there notes the selection is
  sent on every poll so an absent one cannot inherit an earlier setting.
- **The adapter decodes none of these objects.** `Incoming`
  (`crates/adapters/telegram/src/client.rs:124-144`) holds `message_id`, `date`, `chat`,
  `from`, `sender_chat`, `text`, `caption`, `reply_to_message`, `pinned_message` and nothing
  else; unknown fields are dropped by the decoder. That not-decoding is already used as a
  deliberate protection: the sender's display name "is not decoded at all, so a display name
  never enters the process as a typed value"
  (`crates/adapters/telegram/src/client.rs:212-215`). It is the exact technique this unit
  reuses for coordinates, addresses and phone numbers.
- **Addressing needs no change.** A message with no text cannot mention the assistant, so
  `addressed` falls out of the existing derivation
  (`crates/adapters/telegram/src/translate.rs:170-178`): true in a direct chat, true when it
  replies to the assistant, false otherwise. Nothing about a shared item is special here.
- **The message kind and its projection.** `ChatMessage` at `crates/core/src/kind.rs:376-431`,
  its stored field map at `:444-485`, the descriptor's column list at `:575-597`.
  `projected_text` (`crates/core/src/kind.rs:555-569`) returns `ERASED_MARKER` when `text` is
  `None`, otherwise composes `{speaker}: {text}` and wraps that in
  `projected_origin_mark(origin)` from the outside — so a per-message line belongs inside
  `text`, and an erased row never reaches the composition at all.
- **`owes_answer` reads `text.is_some()`**, not non-emptiness
  (`crates/core/src/kind.rs:495-498`, the shared predicate), so an empty text is a real
  recorded message that can still owe a turn when addressed, and an erased one cannot.
- **Erasure's three steps.** The module doc (`crates/core/src/erasure.rs:1-50`) nulls the
  personal columns of the principal's messages — text, origin, send time, reply target,
  speaker — then removes the principal's direct conversations whole, then concludes the
  identity rows. Structural columns are left alone by design; `schema.rs` says so of the
  protection stamp in as many words ("Both columns are structure, not personal data: erasure
  leaves them", `crates/core/src/schema.rs:154-158`).
- **Schema growth is append-only, with frozen vocabularies.** The shipped `CREATE TABLE` is
  frozen (decision 0026); every change since is an appended step quoting a vocabulary list
  frozen when the step shipped, never the live enum
  (`crates/core/src/schema.rs:113-123`), with `PROTECTION_STAMP_MIGRATION`
  (`crates/core/src/schema.rs:159-172`) as the shape to copy: `ALTER TABLE … ADD COLUMN …
  TEXT CHECK (… IN (…))`.
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
  disagreement with that shape.
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
- **Four published statements are in range.** `docs/privacy/records-of-processing.md:61`
  (category D1, message content), `:66` (D6, special categories, incidentally) and `:144`
  (the data-minimisation measure, "Text only, no media, no files, no voice, no stickers, no
  edits"); `docs/privacy/dpia.md:127-168` (3.2, categories of data, including the
  special-categories paragraph at `:158-168`); `docs/privacy/lia.md:204-245` (section 5,
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
  greps the core's sources for the whole words in `docs/platform-vocabulary.txt`, which lists
  platform and SDK names only. A neutral-sounding name for a platform concept passes it
  automatically, so the form vocabulary below has to earn its neutrality by argument.
- **Decision numbering is contended this week.** `docs/units/telegram/01-receiving-media.md`
  claims 0106 through 0119, and the highest recorded record is
  `docs/decisions/0105-the-fixed-line-is-the-acknowledgments-fallback.md`. Units 02 through
  07 take their decisions in the unnumbered dated form. This unit does the same and assigns
  numbers at merge (see the notes).

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

- **One neutral column, four frozen values, 2026-08-25.** An appended migration step adds a
  single nullable column `shared_form` to the message content table, in the shape of
  `PROTECTION_STAMP_MIGRATION` (`crates/core/src/schema.rs:159-172`), with its vocabulary
  frozen in the step per `crates/core/src/schema.rs:113-123`. The vocabulary is exactly:

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

  An erased row never reaches this function — `projected_text` returns `ERASED_MARKER` from
  its first line when `text` is `None` — so no shared line survives erasure and no extra
  branch is needed for it. **Nothing new reaches the model provider**, which is why the
  recipients section of the record of processing does not change. *Rejected:* projecting the
  venue title and address, which are the least sharp of the four and the most conversationally
  useful — one rule with no exceptions is what makes the member-facing statement short and
  true, and a place a member shares still says where that member is or intends to be.
  *Rejected:* projecting the live period, so the model knows for how long the sharing runs —
  it is a number about how long a person will broadcast their movements, no behaviour reads
  it, and the model does not need it to answer anything. *Rejected:* saying nothing to the
  model, leaving an empty turn — silence reads as "nothing was sent", which is false, and an
  addressed member would get an answer to a message the assistant appears not to have
  received. *Rejected:* enforcing this in the prompt while the payload still travels —
  decision 0070 already rejected enforcement by prompt alone, and the never-decode rule is
  what turns "do not analyse people" from advice into a property of the system.

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

- **The proximity alert is a named skip, not an anonymous one, 2026-08-25.** A new
  `Skip::ProximityAlert` is returned ahead of the sender check, with the reason line "a
  service note that two members came near each other". Today the message is dropped by
  whichever of the sender check or the text check fires first, which makes the outcome depend
  on a platform detail nobody has verified; an explicit skip makes it a decision that a later
  widening of the record rule cannot undo by accident. The fact itself is a distance in
  metres between two named members, measured by the platform, addressed to nobody and asked
  for by neither — the one member who set the alert asked their own device, not this
  assistant. *Rejected:* recording it as a group observation the way a pin is recorded — a
  pin is the group's published governance, and this is two people's bodies; *rejected:*
  relying on the existing text skip — it would leave the decision unwritten and the reason
  unsaid, and the log line would call a service note about two members "a message with
  neither text nor caption".

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

  *Rejected:* a send capability limited to administrators — decision 0070 puts the human
  decision point in the mechanism, and there is no administrator command surface here; an
  administrator who wants to post a place posts it themselves in one tap, so the capability
  would add a fabrication risk and no reach. *Rejected:* sending a place for community events,
  which is the one plausible caller — the event's organiser posts it, the assistant has no
  event calendar to read one from, and a wrong address for a real meetup sends people to a
  real wrong door. *Rejected:* implementing the methods now and leaving them uncalled — a
  capability with no caller is dead code that the next reader takes as permission, which unit
  02 states as its own rule (`docs/units/telegram/02-sending-media.md:271-273`).

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
  share a location. If asked, it says plainly that it is not shown that. And the existing
  "Do not analyse people" rule at `:40-47` is extended by one clause, because a position is
  precisely the material from which the inferences that rule already forbids get made: where
  someone is, where they have been and who they are near are not things the assistant infers,
  comments on or is drawn into. *Rejected:* leaving the bracket convention to be self-evident
  — the prompt teaches every other convention explicitly, and a model handed
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
  whose families include `InputMediaLocation` and `InputMediaVenue` — noted for unit 05 in
  the notes below, not built here.

## The unit's contract

A message that shares a position, a live position, a place or a contact card is recorded
like any other message, with an empty text and one column naming which of the four it was;
only a message that shares nothing this adapter carries and has no text is skipped, and the
proximity-alert service note is a named skip with its own reason. The payload — coordinates,
accuracy, heading, proximity radius, venue title, venue address, both venue-provider
identifier pairs, phone number, first and last name, account identifier and vCard — is never
decoded by the adapter, so it is never stored, never logged and never transmitted; the
platform's own message stays in the chat where the member posted it, which is where an
administrator reads it. A live position is recorded once as a fact and its movement is never
followed: the edit skip stays, by decision, whether or not the platform emits an update for
it. The model reads one bracketed line naming what was shared and stating that it was not
shown the contents, and the prompt teaches it not to guess where anyone is and extends the
existing no-analysing-people rule to a person's whereabouts. Nothing new reaches the model
provider, so the recipients of the processing record are unchanged. The assistant sends no
position, live position, place or contact card, and the two live-location editing methods
remain unreachable because the send path still discards the identifier they need. Erasure
gains no new pass because the unit stores nothing personal that the existing nulls do not
already reach, and the recorded form survives erasure as structure. The record of processing,
the impact assessment, the legitimate-interest assessment and the member-facing policy state
what is now collected and, more importantly, what is deliberately not. No new dependency, no
bytes move, and the assistant still assesses nothing about anyone and takes no action against
anyone (decision 0070 untouched).

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary check and the secret scan clean; **no new dependency in any manifest**,
  pinned by a test that reads both manifests and asserts the dependency name set is unchanged.
- **AC2** The four forms record. An update carrying `location` with no `live_period` records
  `position`; the same with `live_period` present records `live_position`; an update carrying
  `venue` records `place`; an update carrying `contact` records `person_card`. Each produces
  `IngestOutcome::Recorded` with an empty stored text. Pinned per form.
- **AC3** The alias ordering holds: an update with **both** `venue` and `location` set — the
  shape the documentation says the platform always sends — records `place`, never `position`.
  Pinned as its own case, with a comment naming the documented alias.
- **AC4** Nothing else records. A proximity-alert service message returns
  `Skip::ProximityAlert` with its own reason line, pinned; a message with neither text,
  caption nor any of the four shared objects still returns the textless skip; an
  `edited_message` carrying a `location` with `live_period` returns `Skip::EditedMessage`
  and reaches no ledger write, pinned explicitly because that is the movement case.
- **AC5** The payload cannot enter the process. A source scan in the shape of
  `crates/core/tests/vocabulary.rs` asserts that the strings `latitude`, `longitude`,
  `horizontal_accuracy`, `heading`, `proximity_alert_radius`, `phone_number`, `vcard`,
  `first_name`, `last_name`, `foursquare`, `google_place`, `title` and `address` appear
  nowhere in `crates/adapters/telegram/src/client.rs`, and that no field of `Incoming` or any
  type it holds decodes them. Separately, a full update carrying every one of those fields
  with recognisable sentinel values is fed through translation and ingestion, and the whole
  store file plus the captured tracing output are searched for each sentinel: none appears.
  This is the criterion the storage decision stands or falls on, so it is checked from both
  directions.
- **AC6** The stored row is the form and nothing more: after ingesting each of the four,
  `shared_form` holds the frozen value, the text column holds the empty string, and no other
  column differs from a plain text message's row. A migration test asserts the CHECK
  constraint refuses a fifth value.
- **AC7** The model reads one honest line, pinned against the exact strings in the projection
  decision: each of the four projects as `{origin mark} {speaker}: {line}` with no other
  content; an erased row projects only `ERASED_MARKER` and no shared line; and a scan of the
  four strings asserts none of them names a coordinate, an address or a number.
- **AC8** Addressing and answering behave as they do for any other message: a shared position
  in a group with no reply to the assistant is recorded and owes no turn; the same message
  sent as a reply to one of the assistant's own messages is recorded, owes a turn, and the
  assistant's answer is delivered — proving an empty text is a real message end to end.
- **AC9** Erasure needs no new pass and loses nothing: after `erase_principal` a shared-item
  message's text, origin, send time and speaker are NULL, `shared_form` still holds its
  value, the row projects the erased marker alone, and a second erasure reports completion.
  Pinned for a group message and for a direct conversation, whose rows are removed whole.
- **AC10** The sending methods are absent, pinned and not assumed: a source scan asserts
  that `sendLocation`, `editMessageLiveLocation`, `stopMessageLiveLocation`, `sendVenue` and
  `sendContact` appear in no source file of the adapter, and that the outbound item type
  gained no arm in this unit. The scripted server sees none of those calls across the whole
  adapter suite.
- **AC11** The prompt carries the passage and keeps what was there: `prompts/30-conduct.md`
  contains the shared-line teaching and the whereabouts clause, the existing "Do not analyse
  people" sentence is retained verbatim, and the privacy paragraph at `:35-38` is retained
  verbatim. Pinned by a test in the shape of the existing prompt-composition tests.
- **AC12** The published privacy documents match the running system in the same commit. The
  record of processing gains a category for the shared-item form, stating that the payload is
  not collected, and its data-minimisation measure names coordinates, addresses and contact
  details among what is not kept; the impact assessment gains a dated addendum under its
  "change to what is collected" trigger covering the four forms, the deliberate non-collection,
  the special-category-by-inference reasoning and the third-party data subject a contact card
  names; the legitimate-interest assessment's Article 9 section gains the paragraph
  distinguishing a position from Article 9 data and stating why the residual does not grow;
  and the member-facing policy says in plain words that sharing a location, a place or a
  contact records only that it happened. `docs/compliance/ai-act.md` is checked and stated
  unchanged, because nothing new reaches the model.
- **AC13** The record of the work exists: each decision above is written into
  `docs/decisions` with `Date: 2026-08-25` and a `## Rejected alternatives` section, pinned
  by a test in the shape of `crates/assistant/tests/docs.rs:379-399`, and every follow-up
  named below is appended to `docs/follow-ups.md` naming this unit.

## Notes for launch

- Branches from `main` into its own worktree; builds against the agent-ledger checkout as it
  stands. **This unit needs no framework change** and adds no dependency, no file handling and
  no wire method.
- Adapter sites: `crates/adapters/telegram/src/client.rs:124-144` — `Incoming` gains four
  presence-only fields, decoded as narrowly as possible so no payload becomes a typed value.
  The shape that gives this by construction is a marker type per object holding only what the
  form rule needs: an empty struct for `contact` and `venue`, and for `location` a struct with
  the single field `live_period: Option<i64>` used as a discriminator and never stored.
  `crates/adapters/telegram/src/translate.rs:53,165-167,481` — the new `ProximityAlert` skip
  variant and its reason line, the shared-item dispatch placed with `venue` ahead of
  `location`, and the empty-text record path. `crates/adapters/telegram/src/client.rs:103`
  and `crates/adapters/telegram/src/translate.rs:123-125` are read and left alone, which is a
  decision above and should be commented as one at the skip.
- Core sites: `crates/core/src/message.rs` — a `SharedForm` enum with the four values and an
  optional field on `InboundMessage` beside `command`; `crates/core/src/kind.rs:444-485` for
  the stored field, `:555-569` for the line composed inside the text, `:575-597` for the
  descriptor column, and `:376-431` for the parsed field; `crates/core/src/schema.rs` — one
  appended step in the shape of `:159-172` with the vocabulary frozen per `:113-123`.
  `crates/core/src/erasure.rs` is read and left alone, which is also a decision above.
- Prompt and documents: the passage in `prompts/30-conduct.md` beside `:35-38` and the clause
  at `:40-47`; the four privacy documents named in AC12.
- **Cross-unit ordering, stated because seven sibling specs were written the same week and
  this one is not free to edit them.**
  - `docs/units/telegram/01-receiving-media.md` renames `Skip::NoText` to
    `Skip::NothingToRecord`, widens its meaning, and makes an empty stored text a normal
    recorded message. **This unit needs the same seam.** If unit 01 merges first, this unit
    reuses the renamed skip and the empty-text path and adds the four shared forms to the
    catch-all's meaning; if this unit merges first, it makes the same change in the same
    shape under its own name and unit 01 widens it. Whichever merges second reads the merged
    text, never the drafted text. The two units add separate columns and separate appended
    migration steps, which do not conflict, but the second step must be appended after the
    first in the merged step list.
  - Unit 01 also rewrites `docs/privacy/records-of-processing.md:61,144`,
    `docs/privacy/dpia.md` 3.2 and `docs/privacy/bot-assistant-privacy-policy.md:20-22`; unit
    03 rewrites the same policy sentence again. This unit touches the neighbouring sentences
    in all four documents. Same rule: read the merged text.
  - `docs/units/telegram/05-polls.md:465-478` introduces the `Outbound` item enum and unit 06
    adopts it. This unit deliberately adds no arm and depends on neither; it names the enum
    only so the absence reads as a decision.
  - `docs/units/telegram/05-polls.md:628-638` scopes poll media out and lists its families as
    "the photo, video and sticker forms". The families also include `InputMediaLocation` and
    `InputMediaVenue`, in both `InputPollMedia` and `InputPollOptionMedia`. When poll media
    ships, the decision above — the assistant sends no position and no place — applies there
    too, and a poll option carrying a place would be the first sending path to reach these
    classes. Recorded here instead of edited into that spec.
- Named follow-ups, recorded and not built: a member who shares a position may be sharing
  someone else's home, and no mechanism here detects that (an administrator reads the message
  in the chat, which is the human decision point decision 0070 asks for, and nothing about
  this unit changes it); the assistant cannot answer "how far is that from me", and the
  honest reason is that it has no geographic lookup, which is a tool decision with its own
  data-protection question and not a location decision; the group's own attached place is not
  collected, which a later unit could revisit if a group ever asks; poll media will need the
  streaming answer this unit did not have to give; and whether the platform emits an edit
  update when a live position moves is undocumented — if a later unit ever needs to know, it
  is answered by observation against a real chat and written down, not assumed.
