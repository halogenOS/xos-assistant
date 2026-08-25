# Telegram unit 14 — stickers and dice rolls arrive as facts; the assistant sends neither

Date: 2026-08-25. A member who answers a question with a thumbs-up sticker has answered it.
Today the assistant does not see that at all: a sticker message carries no text and no
caption, so translation returns `Skip::NoText` (`crates/adapters/telegram/src/translate.rs:166`)
and the message never reaches the core. The same is true of a dice roll. In a conversation
the model reads, both come out as holes — two people talking with a silent gap between them,
and the model attributing the gap to nobody.

This unit closes the receiving half and refuses the sending half, on purpose.

**Receiving.** A sticker and a dice roll become recorded messages whose text is empty and
whose meaning sits in five new columns on the message row. The model reads one honest line
for each. No bytes are fetched: a sticker's drawing is never downloaded, because the model is
shown no picture anyway (unit 01's decision 0111) and a group repeats the same twelve stickers
all week.

**Sending.** The assistant sends no sticker, no dice and no custom emoji. Each refusal has its
own reason and each is structural, not taught — there is no tool, no outbound item and no
configuration key for any of them.

Three platform facts shape the whole design and none of them is obvious:

1. **A sticker's emoji belongs to the drawing, not to the sender.** It is assigned when the
   sticker is added to a set (`InputSticker.emoji_list`, `setStickerEmojiList`), so a member
   who sends a sticker is not choosing that emoji at all. The projection has to say this or
   the model will read a picture-label as a word somebody typed.
2. **A sticker message cannot carry a caption.** `Message.caption` is documented for
   "the animation, audio, document, paid media, photo, video or voice" — sticker is absent
   from that list. A sticker and its explanation are always two messages.
3. **A dice roll's outcome is decided by the platform and arrives inside the update.** The
   `Dice` object carries `value` on receipt, and `sendDice` picks the number itself. A bot
   that talks about that number is either reporting a fact it did not produce, or spoiling an
   animation the members are still watching.

Everything below is checked against the live Bot API page and changelog (Bot API 10.3,
24 August 2026, fetched 2026-08-25) and against this tree. Every claim carries its source.

## Grounding

### What the platform actually does

- **The live documentation is Bot API 10.3, dated 24 August 2026** — the top entry of
  `/bots/api-changelog`. 10.2 shipped 14 July 2026, 10.1 on 11 June 2026, 10.0 on 8 May 2026.
  The brief's assumption of 10.1 is two releases behind; nothing in 10.1 through 10.3 changes
  `Sticker`, `Dice`, `sendSticker` or `sendDice` beyond the send parameters listed below.
- **`Message.sticker`**, verbatim: "*Optional*. Message is a sticker, information about the
  sticker". **`Message.dice`**, verbatim: "*Optional*. Message is a dice with random value".
- **`Message.caption`**, verbatim: "*Optional*. Caption for the animation, audio, document,
  paid media, photo, video or voice". Neither a sticker nor a dice appears in that list, so a
  sticker message and a dice message never carry a caption. This is the fact that makes
  `text_of` (`translate.rs:466-472`) return `None` for both today.
- **The `Sticker` object**, verbatim field list: `file_id` ("Identifier for this file, which
  can be used to download or reuse the file"), `file_unique_id` ("Unique identifier for this
  file, which is supposed to be the same over time and for different bots. Can't be used to
  download or reuse the file"), `type` ("Type of the sticker, currently one of "regular",
  "mask", "custom_emoji""), `width`, `height`, `is_animated`, `is_video`, optional
  `thumbnail` (PhotoSize), optional **`emoji`** ("Emoji associated with the sticker"),
  optional **`set_name`** ("Name of the sticker set to which the sticker belongs"), optional
  `premium_animation` (File), optional `mask_position`, optional `custom_emoji_id`, optional
  `needs_repainting`, optional `file_size`. Both fields this unit stores are **optional**: a
  sticker with no emoji and no set name is a documented shape.
- **The emoji is a property of the sticker, set by whoever made the set.** `InputSticker`
  carries `emoji_list`, "List of 1-20 emoji associated with the sticker", supplied when the
  sticker is added to a set; `setStickerEmojiList` is "Use this method to change the list of
  emoji assigned to a regular or custom emoji sticker. The sticker must belong to a sticker
  set created by the bot." `sendSticker`'s own `emoji` parameter is "Emoji associated with the
  sticker; **only for just uploaded stickers**". Nothing in the API lets a sender pick an
  emoji for a sticker they are sending from an existing set. So `Sticker.emoji` is the set
  author's label on a drawing, one step removed from the person who sent it.
- **The `Dice` object**, verbatim: "This object represents an animated emoji that displays a
  random value." Two fields, **both required**: `emoji` ("Emoji on which the dice throw
  animation is based") and `value` ("Value of the dice, 1-6 for "🎲", "🎯" and "🎳" base
  emoji, 1-5 for "🏀" and "⚽" base emoji, 1-64 for "🎰" base emoji").
- **The six animations and their exact codepoints**, extracted from the page's image alt text
  because the emoji are rendered as images: 🎲 U+1F3B2, 🎯 U+1F3AF, 🎳 U+1F3B3 (each 1-6);
  🏀 U+1F3C0, ⚽ U+26BD (each 1-5); 🎰 U+1F3B0 (1-64). **The same variation-selector hazard
  unit 06 recorded applies here**: ⚽ is given as U+26BD alone, with no U+FE0F, and a copy
  from a chat client or an editor will silently add one. Any table keyed on these bytes must
  be written as escape sequences and must tolerate a trailing U+FE0F on the wire.
- **What a value means visually is documented for none of them.** The page states the ranges
  and nothing else: no mapping from 6 to "bullseye" for the darts, and no reading at all of
  the slot machine's 1-64, which encodes three reels. A design that interprets the number
  would be inventing the interpretation.
- **`sendDice`**, verbatim: "Use this method to send an animated emoji that will display a
  random value. On success, the sent Message is returned." Its `emoji` parameter: "Emoji on
  which the dice throw animation is based. Currently, must be one of "🎲", "🎯", "🏀", "⚽",
  "🎳", or "🎰". Dice can have values 1-6 for "🎲", "🎯" and "🎳", values 1-5 for "🏀" and
  "⚽", and values 1-64 for "🎰". Defaults to "🎲"." There is no parameter for the outcome and
  no method that sets one. The caller learns the value only from the returned `Message`.
- **A dice message cannot be deleted for the first 24 hours in a one-to-one chat.**
  `deleteMessage`, verbatim among its limitations: "A message can only be deleted if it was
  sent less than 48 hours ago." … "**A dice message in a private chat can only be deleted if
  it was sent more than 24 hours ago.**" The two rules together leave a one-day window in
  which a dice the assistant sent in a direct chat can be removed by nobody, and a
  twenty-four-hour window in which it cannot be removed at all. This is the inverse of every
  other message's rule and it is the strongest single argument in this unit.
- **`sendSticker`**, verbatim: "Use this method to send static .WEBP, animated .TGS, or video
  .WEBM stickers. On success, the sent Message is returned." Its `sticker` parameter: "Sticker
  to send. Pass a file_id as String to send a file that exists on the Telegram servers
  (recommended), pass an HTTP URL as a String for Telegram to get a .WEBP sticker from the
  Internet, or upload a new .WEBP, .TGS, or .WEBM sticker using multipart/form-data. …
  Video and animated stickers can't be sent via an HTTP URL." Full parameter list at 10.3:
  `business_connection_id`, `chat_id`, `message_thread_id`, `direct_messages_topic_id`,
  `ephemeral_message_parameters`, `sticker`, `emoji`, `disable_notification`,
  `protect_content`, `allow_paid_broadcast`, `message_effect_id`,
  `suggested_post_parameters`, `reply_parameters`, `reply_markup`.
- **A `file_id` is neither portable nor stable.** Under "Sending files": "file_id is unique
  for each individual bot and can't be transferred from one bot to another" and "a file can
  have different valid file_ids even for the same bot". So a sticker identifier written into
  a configuration file works for exactly one deployment and can stop working without notice.
- **`getFile`**, verbatim: "Use this method to get basic information about a file and prepare
  it for downloading. For the moment, bots can download files of up to 20MB in size. … It is
  guaranteed that the link will be valid for at least 1 hour." A sticker is small enough that
  the cap never binds; the cost of fetching one is the call and the disk, not the size.
- **Custom emoji, receiving.** `MessageEntity.type` includes ""custom_emoji" (for inline
  custom emoji stickers)", and `MessageEntity.custom_emoji_id` is "*Optional*. For
  "custom_emoji" only, unique identifier of the custom emoji. Use getCustomEmojiStickers to
  get full information about the sticker." `getCustomEmojiStickers` takes "A JSON-serialized
  list of custom emoji identifiers. **At most 200 custom emoji identifiers can be specified**"
  and returns an array of `Sticker`.
- **The platform defines the fallback for a custom emoji itself, and defines it as an
  ordinary emoji.** In both the MarkdownV2 and the HTML formatting sections, verbatim: "A
  valid emoji must be provided as an alternative value for the custom emoji. **The emoji will
  be shown instead of the custom emoji in places where a custom emoji cannot be displayed**
  (e.g., system notifications) or if the message is forwarded by a non-premium user. It is
  recommended to use the emoji from the emoji field of the custom emoji sticker." The rich
  message type added in 10.1 repeats the same shape: `RichTextCustomEmoji` carries
  `custom_emoji_id` and `alternative_text`, "Alternative emoji for the custom emoji".
- **Custom emoji, sending — this bot cannot rely on it.** Stated twice, once per formatting
  mode, verbatim: "**Custom emoji entities can only be used by bots that purchased additional
  usernames on Fragment or in the messages directly sent by the bot to private, group and
  supergroup chats if the owner of the bot has a Telegram Premium subscription.**" One
  documented exception exists and it is group-scoped:
  `ChatFullInfo.custom_emoji_sticker_set_name`, "*Optional*. For supergroups, the name of the
  group's custom emoji sticker set. **Custom emoji from this set can be used by all users and
  bots in the group.**" So the capability depends on the operator's purchases or on one
  group's own pack, and it is unavailable by default.
- **Changelog dates for the relevant history**: `sendDice` and `Message.dice` arrived in Bot
  API 4.7, 30 March 2020, with values 1 to 6; the darts animation and `Dice.emoji` in 4.8,
  24 April 2020; basketball in 4.9, 4 November 2020; football and the slot machine in 5.0;
  bowling in 5.1. Custom emoji entities became expressible through HTML and MarkdownV2 in Bot
  API 6.7, 21 April 2023. Nothing since changes either object's shape.
- **Privacy mode decides whether any of this arrives.** A bot in a group with privacy mode on
  receives only commands aimed at it, replies to it, and service messages. The operator's
  obligation to switch it off is already recorded at
  `docs/reference/group-operator-contract.md:8-17`, and nothing in this unit works without it.

### What the neighbouring platform calls these things

- The Matrix client-server specification (fetched 2026-08-25) carries a **"Sticker Messages"**
  module of its own, so "sticker" is a concept two platforms share and not one platform's
  word. It defines **no dice roll and no roll outcome at all**. That asymmetry decides the
  naming below: the core may learn "sticker", and must not learn "dice".

### What already exists in this tree

- **The skip that hides both.** `translate.rs:164-167` — `let Some(text) = text_of(message)
  else { return Translation::Skip(Skip::NoText) };`. `text_of` (`translate.rs:466-472`) reads
  `message.text` then `message.caption` and filters out the empty string. Since a sticker and
  a dice carry neither, both messages die here. `Skip::NoText`'s own display string
  (`translate.rs:481`) is "a message with neither text nor caption".
- **The adapter decodes only what translation reads.** `Incoming` (`client.rs:125-144`)
  carries `message_id`, `date`, `chat`, `from`, `sender_chat`, `text`, `caption`,
  `reply_to_message`, `pinned_message` and nothing else; the struct's own comment says "the
  model stays exactly as small as the translation needs". There is **no `entities` field**, so
  no custom emoji identifier enters this process today, by construction and not by accident.
  `User` (`client.rs:217-222`) shows the same discipline for the display name: a field that is
  not decoded cannot be used by mistake.
- **`Pending` is the adapter's own carrier** (`translate.rs:79-114`) and `InboundMessage`
  (`crates/core/src/message.rs:171-210`) is the core's. `InboundMessage.text` is documented at
  `message.rs:200-202` as "What was said, verbatim: the ledger records what the person typed,
  never a rewritten form". Anything invented in that column would be indistinguishable from
  something a member typed, on every later read.
- **The message row already grows by appended `ALTER TABLE` steps, CHECK included.**
  `LITERAL_ADDRESSED_MIGRATION` (`crates/core/src/schema.rs:361-367`) is
  `ALTER TABLE {CHAT_MESSAGE_TABLE} ADD COLUMN {COLUMN_LITERAL_ADDRESSED} INTEGER CHECK
  ({COLUMN_LITERAL_ADDRESSED} IN (0, 1));` — an added column carrying its own CHECK, with no
  table recreation. `SPEAKER_MIGRATION` (`:318-323`) and `REPLY_TARGET_MIGRATION` (`:279`) are
  plain added columns. Only a step that **widens an existing** CHECK recreates the table
  (`COMMAND_STAMP_MIGRATION`, `:229-277`). Verified independently against SQLite 3.51.2: an
  added column with a value CHECK is accepted, enforces the constraint on later inserts, and
  admits NULL on every existing row. The bundled engine here is libsqlite3-sys 0.38.2 under
  rusqlite 0.40 (`Cargo.lock`), and the implementer confirms the same behaviour there.
- **Frozen vocabularies are the rule for stored enums.** `schema.rs:113-123` states it:
  every appended step quotes a list frozen when the step shipped, never a live enum, and the
  tests at `schema.rs:397-436` pin each newest frozen list to its enum so growing the enum
  fails loudly. `quoted_list` is at `schema.rs:35`. `store_config()` (`schema.rs:373-395`)
  lists the steps in order and a new one is appended last.
- **The projection composes from the stored columns.** `ChatMessage::projected_text`
  (`crates/core/src/kind.rs:555-569`) returns `ERASED_MARKER` when `text` is `None`, otherwise
  prefixes the speaker for a user-voiced row and wraps the whole line in
  `projected_origin_mark` (`kind.rs:174-182`, `format!("[{origin}]")`). Any content line this
  unit adds goes **inside** the text, exactly as unit 01's file line does, because the origin
  mark is applied from outside.
- **Erasure nulls five columns, in two places, and they must stay in step.**
  `erase_principal_content` (`kind.rs:688-705`) and `erase_message_named` (`kind.rs:743-784`)
  both set `text`, `origin`, `sent_at`, `reply_target` and `speaker` to NULL;
  `MirrorNulls`'s comment (`kind.rs:710-717`) calls them "five personal columns". A new
  content column that is not added to both statements is personal data erasure does not reach.
- **A message's summons is stamped at the write, and helpful answering summons everything.**
  `Assistant::resolved_summons` (`crates/core/src/assembly.rs:1244-1246`) stores
  `summoned: message.addressed || self.answering == AnsweringMode::Helpful`. So in the
  shipping mode every recorded message owes the model a turn, and a sticker will too.
- **A turn that says nothing is refunded.** `COUNTED_DEBT_SQL` (`kind.rs:928-941`) excludes a
  debt whose anchored assistant block trims to the empty string — unit 22's re-keying. A
  sticker the model ignores therefore costs the sender no budget.
- **The HTML renderer already makes a custom emoji tag unsendable.** `formatting.rs:35-46`
  escapes `&`, `<` and `>` before any tag is inserted, and the module's own comment
  (`formatting.rs:26-29`) states the property: "It escapes first and inserts second." A model
  that writes `<tg-emoji emoji-id="…">👍</tg-emoji>` in its prose gets that text delivered
  literally, escaped. No change is needed to keep custom emoji out of the assistant's
  messages; the mechanism that keeps them out already exists and is tested.
- **The outbound edge is untouched by this unit.** `Assistant::replies`
  (`assembly.rs:1048-1051`) yields `OutboundReply` and `consume_replies`
  (`crates/adapters/telegram/src/driver.rs:730-761`) sends each one. Three sibling specs
  (`05-polls.md:248-257`, `06-reactions.md:438-445`, `07-buttons-and-callbacks.md:528`) all
  propose changing that element type. This unit sends nothing new, so it adds no arm and joins
  no part of that contention.
- **The platform-vocabulary check scans alphanumeric words.**
  `crates/core/tests/vocabulary.rs:63-67` matches whole runs of alphanumeric characters
  against `docs/platform-vocabulary.txt`, whose seven entries are platform and SDK names.
  `sticker`, `pack`, `roll` and `outcome` are on nobody's list and none of them is a platform
  or SDK name.
- **Unit 06 proposes a scan that a careless test fixture here would break.**
  `06-reactions.md:320-337` adds a test asserting that every non-ASCII character in
  `crates/core/src` is on a short allowlist. The core's tests live **inside** `src` —
  `crates/core/src/kind.rs` and `crates/core/src/message.rs` each hold an inline
  `#[cfg(test)] mod tests`. So a projection test written with a pasted 🎲 would fail that
  scan. Every fixture in this unit is written as an escape sequence, which is ASCII source.
- **Four published statements name stickers and would become false.**
  `docs/privacy/records-of-processing.md:61` ("No media, no files, no voice, no stickers"),
  the same document's data-minimisation line at `:144` ("Text only, no media, no files, no
  voice, no stickers, no edits"), `docs/privacy/dpia.md:128-130` (the same sentence), and
  `docs/privacy/dpia.md:566`, which lists "A change to what is collected: media, edits,
  reactions, membership events" as a trigger for retaking the assessment.
  `docs/privacy/lia.md:106` says "Media, files and voice are not stored" and stays true —
  this unit stores no media — but its necessity paragraph is written around text only and
  needs one sentence.
- **Sibling ordering.** `01-receiving-media.md` decision 0106 changes `Skip::NoText` to
  `Skip::NothingToRecord`, permits an empty text column, and names "a captionless sticker" and
  "dice" among what still skips, with the reason recorded at `01-receiving-media.md:260-263`:
  "a sticker is a fixed drawing from a set with its own emoji, closer to punctuation than to a
  file, and whether the emoji stands in for it is a separate decision". This unit is that
  decision.

### One correction to a sibling spec, recorded and not edited there

`01-receiving-media.md:602-605` says a **captioned** sticker "still records its caption". The
platform makes that case unreachable: `Message.caption` is documented only for the animation,
audio, document, paid media, photo, video and voice, so a sticker message never carries one.
Nothing in that unit depends on the claim — its skip list already names the captionless
sticker — but an acceptance criterion written against a captioned sticker cannot be satisfied
by any real update, and a test asserting it would be pinning a fixture the platform never
produces. Whoever implements unit 01 should drop the sticker from that half of the criterion.
The same applies to a "captioned" dice. This spec does not edit that file.

## Decisions taken with this unit

- **A sticker and a dice roll become recorded messages; a game, a story and a contact stay
  skips, 2026-08-25.** Translation stops discarding them: a message carrying `sticker` or
  `dice` is recorded with an empty text column and its facts in the columns below. The reason
  is what the skip costs: an answer given by sticker is an answer, and a transcript with the
  answer missing invites the model to attribute the following message to the wrong person, or
  to answer a question somebody already settled. `Message.game`, `Message.story`,
  `Message.contact`, `Message.location`, `Message.venue` and `Message.poll` remain skips —
  a poll is unit 05's, and the rest carry no meaning this unit knows how to state honestly.
  *Rejected:* continuing to skip both. It is the status quo and its cost is a silent hole in
  the record of a group conversation, which is the one thing the ledger exists to hold.
  *Rejected:* recording stickers and continuing to skip dice, on the argument that a roll is
  noise. A roll is frequently the whole of somebody's turn in a chat, and a hole is a hole.
  *Rejected:* recording them as observations instead of messages (`ObservedFact`,
  `message.rs:213-243`). An observation is a fact about a channel; these are things a person
  said, with an author, an authority and a place in the conversation.

- **The bytes are not fetched, and this spec says exactly what fetching them would take,
  2026-08-25.** No `getFile`, no download, no attachment record, nothing on disk. Three
  reasons, in order of weight. The model is shown no picture in any case — unit 01's decision
  0111 refuses `ContentPart::Image` because the variant takes a `Vec<u8>` and would put a
  whole file in memory for every replay of the conversation — so fetched sticker bytes would
  be written and never read. A group reuses the same handful of stickers all day, so a
  per-message fetch pays for the same drawing dozens of times. And a fetch is a `getFile` plus
  a download inside the strictly sequential update batch (`driver.rs:319-320`), which is the
  cost unit 01 had to build a whole batch budget to contain.
  *Rejected:* fetching the sticker's `thumbnail` instead of the sticker. It is smaller and
  still unread, so it is the same waste in a smaller size.
  *Rejected:* fetching once per `file_unique_id` and reusing it. That is the right shape the
  day the drawing is actually looked at, and it is written down here as the seam: unit 01's
  sink is reused verbatim, keyed on `file_unique_id` (documented as "the same over time and
  for different bots"), and it needs its own privacy amendment because a picture reaching the
  model provider is a new category of data reaching a processor. Building it now is a store
  with no reader.

- **The core learns "a sticker" and "a chance roll", never "dice", 2026-08-25.** The neutral
  vocabulary is a closed enum in the core:

  ```
  WordlessContent {
      Sticker { label: Option<String>, pack: Option<String> },
      ChanceRoll { label: String, outcome: i64, outcome_max: Option<i64> },
  }
  ```

  "Sticker" is shared vocabulary — the Matrix client-server specification carries a Sticker
  Messages module of its own — so it names a thing two platforms already have. "Dice" is not:
  Matrix defines no roll, the word names one platform's feature, and a `dice_value` column
  would be that platform's vocabulary wearing a column name. What the core stores instead is
  what the thing actually is anywhere: **a roll whose outcome the platform chose**, with the
  range it chose from. An adapter for a platform with no such feature simply never produces
  the variant.
  *Rejected:* `dice_emoji` and `dice_value` columns. Shorter, and it writes one platform's
  feature name into the core's schema, which the project's first invariant forbids.
  *Rejected:* a closed enum of the six animations (`Die`, `Dart`, `Basketball`, `Football`,
  `Bowling`, `SlotMachine`). It is Telegram's list under English names, it goes stale the day
  a seventh animation ships, and the core would have to grow a variant for a decoration it
  never reads.

- **The label is opaque data the core stores and never interprets, 2026-08-25.** Both variants
  carry a `label` that is the platform's own short token — the sticker's associated emoji, the
  roll's animation emoji. The core stores it, sanitises it and prints it, exactly as it treats
  `origin` (`message.rs:203-205`, "The platform's own id for the message, opaque"). It never
  parses it, never branches on it and never contains one in its source. That keeps unit 06's
  no-glyph property true for this unit without any special handling: every emoji in the core
  is a value read out of a row, and every test fixture is written as `"\u{1F3B2}"`.
  *Rejected:* mapping the emoji to an English word in the core ("a thumbs-up sticker"). It
  needs the emoji table in the core, it is wrong for every sticker whose label is not a
  gesture, and it is the machine deciding what a member meant.
  *Rejected:* dropping the label and projecting only "[sent a sticker]". It throws away the
  single most informative fact the platform gives, and leaves the model unable to tell a
  thumbs-up from a shrug.

- **The projection states whose label it is, in fixed wording, 2026-08-25.** One function
  composes the line from the stored columns and places it inside the text, ahead of any
  caption, because `projected_text` wraps the whole speaker line in the origin mark from
  outside (`kind.rs:555-569`). A sticker never has a caption, so in practice the line is the
  whole message. The wording, given in full because "the exact projected string" is a
  criterion:

  | stored | projected line |
  | --- | --- |
  | sticker, label and pack | `[sent a sticker the set labels 👍, from the pack "halogenos"]` |
  | sticker, label only | `[sent a sticker the set labels 👍]` |
  | sticker, pack only | `[sent a sticker from the pack "halogenos"]` |
  | sticker, neither | `[sent a sticker]` |
  | roll, with a range | `[sent a 🎲 roll; the platform chose 4 of 1-6]` |
  | roll, no range | `[sent a 🎲 roll; the platform chose 4]` |

  "the set labels" is the whole point of the line: the emoji was assigned when the sticker was
  added to its set, so it describes the drawing and not the sender's intent. "the platform
  chose" names who produced the number. Both the label and the pack name are sanitised the way
  unit 01 sanitises a file name — control characters stripped, clipped to 255 characters —
  because the pack name is authored by whoever made the pack, which is anybody.
  *Rejected:* writing the emoji into the message text column so the model reads "👍" as the
  message. The text column is what the person typed, verbatim (`message.rs:200-202`), and unit
  01's decision 0106 already refused an invented caption for exactly this reason: an invented
  value is indistinguishable from a real one for the rest of the row's life.
  *Rejected:* "sent a 👍 sticker", which reads as if the member chose that emoji.
  *Rejected:* omitting the pack name. It is the one cheap fact that tells the model whether a
  sticker came from the project's own pack or from a stranger's, and it is not personal data
  (a pack's short name is public and shared by everyone who uses it).

- **The roll's outcome is projected with its range and never interpreted, 2026-08-25.** The
  model reads the number the platform chose and the range it came from, and nothing else. The
  documentation defines the ranges and never defines what a value looks like on screen — there
  is no published reading of 6 as a bullseye and none at all of the slot machine's 1 to 64,
  which encodes three reels. So the honest line prints the platform's number and stops.
  *Rejected:* hiding the value from the model and projecting `[sent a roll]`. The outcome is
  the entire content of that message; hiding it leaves the model unable to follow a
  conversation in which somebody reacts to it, and stores a fact nothing reads.
  *Rejected:* translating the value ("a bullseye", "three sevens"). Every such reading would
  be invented, and an invented reading printed as fact is the defect the sourcing discipline
  (decision 0096) exists to prevent.
  *Rejected:* withholding the value for a few seconds so the animation finishes first. The
  outbound edge delivers on `StreamDone`, `StreamError` or a lag and holds no timers
  (`crates/core/src/outbound.rs:186-262`); a delay mechanism would be built for one cosmetic
  case, and the model's own turn already takes longer than the animation.

- **The range comes from the adapter, because the range is platform knowledge, 2026-08-25.**
  The adapter holds the six-entry table from the animation emoji to its documented maximum
  (1F3B2, 1F3AF, 1F3B3 → 6; 1F3C0, 26BD → 5; 1F3B0 → 64), written as escape sequences, with
  a trailing U+FE0F stripped before the lookup because the documentation gives ⚽ as U+26BD
  alone while a client may send the variation-selector form. An emoji the table does not know
  — a seventh animation Telegram adds later — yields no range, and the projection drops that
  clause. This is translation of a documented platform fact, the same shape as
  `authority.rs`'s status mapping, and it decides nothing.
  *Rejected:* the table in the core. It is six of one platform's emoji and their limits, which
  is platform vocabulary by any reading.
  *Rejected:* refusing to record a roll whose emoji is unknown. A new animation would then
  silently reopen the hole this unit closes.

- **The assistant sends no sticker, 2026-08-25.** No tool, no outbound item, no configuration
  key, no `sendSticker` method on the client. Four reasons. The wordless acknowledgement
  already exists and is cheaper: unit 06 ships a 👀 reaction that costs no message and does
  not push the conversation down the screen, and a sticker is strictly worse at the same job.
  A sticker is a fixed drawing with a fixed tone, and the assistant's voice is plain answers
  from lookups with silence as the default (decisions 0096, 0098) — a stock cartoon is the
  chatter that default exists to prevent. Sending a member's own sticker back at them reads as
  mimicry or mockery, which is a public judgement of a person with no human decision point in
  it, and decision 0070 does not allow the assistant that. And there is no dependable source
  for the identifier: a `file_id` "is unique for each individual bot" and "a file can have
  different valid file_ids even for the same bot", so a configured one works for one
  deployment and can stop working with no warning and no error the group would understand.
  *Rejected:* a small operator-configured set of sticker identifiers. It is the unstable
  identifier above, plus a new configuration surface, for a capability nobody asked for.
  *Rejected:* the assistant creating its own sticker set through `createNewStickerSet`. That
  method requires a `user_id` to own the set and an upload, so the assistant would be making
  and owning artwork — a product decision far outside a support assistant, and one that adds a
  publishing capability to a bot whose whole design is that it answers questions.
  *Rejected:* teaching the model not to send stickers while leaving a tool reachable. Unit 06
  already recorded why this is not enough: a structural absence cannot be prompted around, a
  taught rule can.

- **The assistant sends no dice, and could not take one back if it did, 2026-08-25.** No
  `roll` tool and no `sendDice`. The outcome belongs to the platform: `sendDice` has no
  parameter for it and the caller learns the number only from the returned message, so every
  sentence the assistant could write about it is either a report of something it did not
  produce or a claim it did. An assistant whose one discipline is that substantive claims come
  from lookups (decision 0096) has no business producing a number from nowhere. Two mechanical
  facts finish the argument. The value arrives to the bot in the same update that starts the
  animation on everyone's screen, so any immediate comment is a spoiler — that clients animate
  for a few seconds is an inference about client behaviour, not a documented fact, and it is
  named as an inference; the value's presence in the update is documented and is enough on its
  own. And `deleteMessage` states that "a dice message in a private chat can only be deleted
  if it was sent more than 24 hours ago", against a general rule that nothing older than 48
  hours can be deleted at all — so a dice the assistant sent by mistake in a direct chat is
  irretrievable for a day, and unit 04's deletion capability does not reach it.
  *Rejected:* a `roll` tool with teaching that says the assistant does not control the result.
  The teaching would be true and the group would still read "the bot rolled a 1 for me", and
  the first member who asks for a re-roll turns the assistant into a slot machine with a
  disclaimer.
  *Rejected:* sending the roll and commenting on it afterwards. Two-stage delivery with a
  delay does not exist on the outbound edge and would be built for this alone.
  *Rejected:* answering "roll a dice" with a refusal message. The model already ends a turn
  with no text when it has nothing to do (unit 22); the teaching below tells it plainly that
  it cannot roll, so an asked member gets a plain sentence, not machinery.

- **Custom emoji reach the model as the fallback the platform itself defines, and no entity is
  decoded, 2026-08-25.** `Incoming` gains no `entities` field, so no `custom_emoji_id` enters
  this process. The justification is the platform's own: the alternative emoji "will be shown
  instead of the custom emoji in places where a custom emoji cannot be displayed", and a text
  model reading a transcript is exactly such a place. The message text already contains that
  emoji at the entity's offset, so the meaning is already in the column the ledger stores.
  *Rejected:* decoding entities and resolving them through `getCustomEmojiStickers`. It buys a
  network call per message (capped at 200 identifiers) to replace an emoji the platform
  already chose as the substitute, and it stores a new identifier that would need its own row
  in the record of processing and its own reach for erasure.
  *Rejected:* storing `custom_emoji_id` now for later use. A stored identifier nothing reads
  is a personal-data column with no purpose, which is the opposite of data minimisation.

- **The assistant sends no custom emoji, and the mechanism that prevents it already exists,
  2026-08-25.** The platform's own rule, stated twice: "Custom emoji entities can only be used
  by bots that purchased additional usernames on Fragment or in the messages directly sent by
  the bot to private, group and supergroup chats if the owner of the bot has a Telegram
  Premium subscription." So the capability is not the assistant's to have; it depends on what
  the operator bought. The one exception, a supergroup's own
  `custom_emoji_sticker_set_name` pack, is usable "by all users and bots in the group" and is
  a per-group fact the assistant would have to read and track. Nothing is built for either.
  The important part is that nothing needs to be: `formatting.rs:35-46` escapes `<`, `>` and
  `&` before inserting any tag, so a model writing a `tg-emoji` tag gets it delivered as
  literal text. This unit adds an assertion of that property so a later change to the renderer
  cannot quietly enable an entity type nobody decided on.
  *Rejected:* a configuration switch for operators with Premium or a Fragment username. It is
  a decorative capability with an operator-specific precondition, and the failure mode when
  the precondition lapses is a refused send — a lost answer, per the renderer module's own
  reasoning at `formatting.rs:8-13`.

- **Five nullable columns on the message row, added by one appended step, 2026-08-25.**
  `wordless_form`, `wordless_label`, `sticker_pack`, `roll_outcome`, `roll_outcome_max`, added
  to `block_chat_message` by an `ALTER TABLE … ADD COLUMN` step per column, appended last in
  `store_config()` (`schema.rs:373-395`) — the `LITERAL_ADDRESSED_MIGRATION` pattern
  (`schema.rs:361-367`), not a table recreation, because no existing CHECK is being widened.
  `wordless_form` carries its own frozen CHECK over exactly `'sticker'` and `'chance_roll'`,
  through `quoted_list` (`schema.rs:35`) against a frozen list, with its pin in the schema
  tests (`schema.rs:397-436`) so growing the enum fails loudly. `wordless_form` is the
  discriminator: `Sticker.emoji` and `Sticker.set_name` are both optional, so a sticker can
  store NULL in every other column and would otherwise be indistinguishable from a plain
  message. `roll_outcome` and `roll_outcome_max` are integers; the roll's outcome is required
  by the platform and required by the core's enum, and the column is nullable only because an
  appended column must be.
  *Rejected:* reusing unit 01's `attachment_form` vocabulary with a seventh value. A sticker
  whose bytes are deliberately not fetched does not fit that unit's `attachment_withheld`
  vocabulary — none of `too_large`, `fetch_failed`, `not_attempted`, `no_room` or
  `not_configured` is true of it — and a dice roll is not a file at all. Stretching a file's
  columns over a thing that is not a file is the bolted-on conditional the engineering
  standard says to refactor away from. The two column families are independent and the
  projection composes both lines if a message ever carries both, so nothing here depends on
  the platform keeping them mutually exclusive.
  *Rejected:* a side table keyed on the block. Unit 01's decision 0109 already recorded why:
  the block loader loads a kind's own content row and nothing else, so a side table is
  invisible to `Projection` and the line could not be composed at all.
  *Rejected:* a separate block kind ahead of the message. It doubles the block count, leaves
  the message block holding an empty text, and duplicates the origin, authority and addressing
  facts to stay useful.
  *Rejected:* one `wordless_detail` text column holding an encoded outcome and range. Encoding
  two integers into a string the core then parses is a private format inside a column, and the
  core would be parsing what it is supposed to store.

- **Erasure and the deletion mirror null the five columns with the text, 2026-08-25.** Both
  `erase_principal_content` (`kind.rs:688-705`) and `erase_message_named` (`kind.rs:743-784`)
  add the five columns to their `SET` lists, and `MirrorNulls`'s comment stops saying "five
  personal columns" and says ten. The projection would already withhold the line after erasure
  — `projected_text` returns `ERASED_MARKER` from its first line when `text` is `None` — but
  withheld is not erased: what a member sent is content they sent, and decision 0003's rule is
  that erasure nulls the columns. A sticker's label plus its pack name is a small, real
  statement of what a person said.
  *Rejected:* leaving the columns and relying on the projection's marker. It keeps a person's
  content in the database after they asked for it to go, which is the exact gap the erasure
  concept exists to close.
  *Rejected:* a pass of its own for the wordless columns. Two statements that must always run
  together are one statement; a separate pass is one more place to forget.

- **A sticker and a roll take a turn like any other message, and nothing special-cases them,
  2026-08-25.** The summons stamp is unchanged (`assembly.rs:1244-1246`): in helpful answering
  every recorded message summons the model, so a sticker does too, and in addressed answering
  a sticker sent as a reply to one of the assistant's messages is addressed, which is exactly
  right — that is somebody answering it. The budget is unchanged and needs no special case:
  a turn that produces no text is already refunded by `COUNTED_DEBT_SQL` (`kind.rs:928-941`),
  so a sticker the model has nothing to say about costs the sender nothing.
  *Rejected:* recording a sticker without owing a turn. It is a content-keyed condition in the
  core's owed-turn logic, and it takes the silence judgement back from the model that unit 22
  gave it.
  *Rejected:* a budget of its own for wordless messages. It counts one layer down something
  already counted.

- **The model is taught what the two lines mean, in the operator's prompt file, 2026-08-25.**
  The prompt file that unit 01's decision 0118 extends with the file-line passage gains a
  short second passage: a bracketed sticker line means a picture arrived that the assistant
  cannot see, and the emoji in it was assigned to the drawing by whoever made the sticker set,
  so it is a hint at a gesture and never a sentence the member typed; a sticker is usually a
  short acknowledgement and usually warrants no reply at all; a bracketed roll line reports a
  number the platform generated, the assistant did not cause it and does not comment on
  somebody's roll; and the assistant cannot send a sticker, cannot roll anything, and says so
  plainly if asked. Without this a model handed
  `[sent a sticker the set labels 👍]` as an addressed member's whole turn will answer as
  though the member wrote a paragraph.
  *Rejected:* composing it in `crates/core/src/teaching.rs`. That module holds what depends on
  configuration — the resolved name, the answering mode, the moderation handle
  (`teaching.rs:1-13`) — and this passage depends on nothing. Its neighbour, the file-line
  passage, is in the prompt file for the same reason.
  *Rejected:* relying on the bracket convention to explain itself. Unit 01 already refused
  that: a convention only the code knows is a convention the model breaks under pressure.

- **Nothing here moves bytes, and the spec says so instead of inventing a stream,
  2026-08-25.** No file is downloaded, uploaded, staged or held: the whole unit moves two
  short strings and two integers per message, all of them already inside the update the poll
  loop decoded. The standing streaming constraint has no surface to bind to. It is named here
  so a reviewer does not have to wonder whether it was overlooked, and the one place a stream
  would appear later — fetching a sticker's drawing — is specified above as a reuse of unit
  01's sink, not as a new mechanism.

## The unit's contract

A message carrying a sticker or a dice roll is recorded like any other message, with an empty
text column and its meaning in five new columns; only a message carrying neither text, caption
nor one of those two is skipped, and the games, stories, contacts and locations that still
skip are named skips. The adapter decodes exactly four platform fields — the sticker's emoji
and set name, the roll's emoji and value — supplies the roll's documented range from a
six-entry table keyed on escape sequences, and decides nothing else; it decodes no message
entities, so no custom emoji identifier enters the process. The core's vocabulary is a closed
`WordlessContent` enum of a sticker and a chance roll, holds the platform's emoji only as
opaque stored data with no glyph in its source, and never learns the word "dice". The model
reads one line per message: for a sticker, that a picture arrived and what its set labels it,
stated as the set's label and not as the sender's word; for a roll, the number the platform
chose and the range it came from, with no interpretation. No bytes are fetched, nothing is
written to disk, and nothing new travels to the model provider beyond those short strings
inside the conversation text that already travels. The assistant sends no sticker, no dice and
no custom emoji, and each refusal is structural: there is no tool, no outbound item, no client
method and no configuration key for any of them, and the existing HTML escaping already makes
a custom emoji entity unsendable. Erasure and the deletion mirror null the five columns
together with the text. A sticker owes the model a turn like any message and an empty turn is
refunded as it already is. The record of processing, the impact assessment, the
legitimate-interest assessment and the member-facing policy stop saying the assistant collects
no stickers. No new dependency, and the assistant assesses nothing about a sticker and takes
no action against anyone — decision 0070 untouched.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary check and the secret scan clean; **no new dependency in any manifest**,
  pinned by the existing manifest assertion or an equivalent one.
- **AC2** A sticker message is recorded: an update whose `message` carries `sticker` with an
  emoji and a set name, and no text, produces `IngestOutcome::Recorded` with an empty text
  column, `wordless_form = 'sticker'`, the emoji in `wordless_label` and the set name in
  `sticker_pack`. A sticker with neither optional field records with both columns NULL and
  `wordless_form` still `'sticker'` — pinned, because that row is otherwise indistinguishable
  from a plain message and the discriminator is the only thing that saves it.
- **AC3** A dice message is recorded: an update carrying `dice` with `emoji` U+1F3B2 and
  `value` 4 records `wordless_form = 'chance_roll'`, `wordless_label` the emoji,
  `roll_outcome` 4 and `roll_outcome_max` 6. All six documented animations are pinned to their
  documented maxima in one table-driven test — U+1F3B2, U+1F3AF, U+1F3B3 to 6; U+1F3C0, U+26BD
  to 5; U+1F3B0 to 64 — every emoji written as an escape sequence and never as a pasted glyph.
  A seventh, unknown emoji records the roll with `roll_outcome_max` NULL and is not skipped.
- **AC4** The variation-selector form is tolerated: a `dice.emoji` of `"\u{26BD}\u{FE0F}"`
  resolves to the same maximum as `"\u{26BD}"`, pinned. The assertion states the reason — the
  documentation gives the football as U+26BD alone and a client may send either form.
- **AC5** The projected lines are exactly the six strings in the decision table above, pinned
  character for character, with each emoji written in the test as an escape sequence so unit
  06's non-ASCII scan over `crates/core/src` stays green (the core's tests live inside `src`).
  The sticker line reads "the set labels", never "sent a 👍 sticker", and the roll line reads
  "the platform chose".
- **AC6** The label and the pack name are sanitised before projection: a pack name containing
  a newline, a NUL and 400 characters projects with the control characters removed and the
  text clipped to 255, pinned against the same rule unit 01 applies to a file name. A pack
  name containing `[` or `]` does not disturb the surrounding origin mark's brackets in any
  way a test can distinguish from ordinary text — asserted, and the assertion's comment states
  that the mark is prose, exactly as `kind.rs:174-182` already records for a forged id.
- **AC7** Nothing is fetched and nothing is written: ingesting a sticker message against the
  loopback server records **no** `getFile` request and creates no file anywhere under the
  media directory, whether or not `[media]` is configured — pinned against the recording
  server, not against log volume.
- **AC8** No entity is decoded: `Incoming` has no `entities` field, and an update whose
  `message` carries a `text` plus an `entities` array with a `custom_emoji` entry records the
  text verbatim and stores no identifier anywhere. Pinned by a decode test asserting the
  recorded row's columns and by a grep of the diff for `custom_emoji_id` returning nothing.
- **AC9** The assistant cannot send any of the three, asserted structurally instead of by the
  absence of a test: the diff introduces no `sendSticker`, `sendDice`, `createNewStickerSet`
  or `getCustomEmojiStickers` call — pinned by a test over the adapter's source for those
  method-name strings, the same shape as the `CONSUMED_UPDATE_TYPES` assertion unit 06 asks
  for, with its comment naming the reasons (the identifier is unstable, the outcome is the
  platform's, the entity needs a purchase). And the renderer property is pinned: a model
  answer containing `<tg-emoji emoji-id="5368324170671202286">X</tg-emoji>` is delivered with
  every angle bracket escaped, asserted against `formatting::to_html` output.
- **AC10** Erasure reaches the five columns from both directions: after `erase_principal` the
  person's sticker and roll rows have NULL in all five, and after an administrator's deletion
  command naming a sticker message that row has NULL in all five with the count reflected in
  `MirrorNulls`; both are idempotent on a second run, and the block header rows are untouched
  — pinned beside the existing erasure and mirror tests.
- **AC11** The migration is appended and the vocabulary is frozen: a store opened on a
  database created before this unit gains the five columns with NULL on every existing row and
  reads back identically to a freshly created store; the `wordless_form` CHECK refuses a value
  outside the frozen pair; and a schema test pins the frozen list to the live enum so growing
  it fails with the message telling the author to append a widening step.
- **AC12** A sticker still takes a turn and an empty turn is still refunded: an addressed
  sticker in a group opens a turn, a model turn that produces no text delivers nothing to the
  chat, and that debt is not counted against the sender's budget — pinned through the existing
  budget test's shape, so the claim rests on `COUNTED_DEBT_SQL` behaviour and not on a fixture.
- **AC13** The model is taught: the prompt file carries the sticker and roll passage, and a
  prompt-composition test asserts the composed prompt contains it. The passage states that the
  emoji belongs to the set, that a sticker usually warrants no reply, that the assistant did
  not cause a roll and does not comment on one, and that it can send neither.
- **AC14** The published documents are true again in the same commit, each site named:
  - `docs/privacy/records-of-processing.md:61` (D1) loses "no stickers" and gains the sticker
    label, the pack name and the roll outcome as recorded message content, with the note that
    the drawing itself is not stored;
  - the same document's data-minimisation line at `:144` loses "no stickers" on the same terms;
  - a new row in section 5, at the next free D-number at merge, describing the wordless
    content record — the form, the platform's label, the pack name and the roll outcome — and
    stating that it travels to the processor inside the conversation text like any message
    content, with **no new recipient**;
  - a new row in section 8's erasure table stating that both the person's own erasure and the
    deletion mirror empty those columns;
  - `docs/privacy/dpia.md:128-130` loses the same clause, and an addendum under the review
    trigger at `:566` ("a change to what is collected") records that stickers and rolls are now
    collected as labels and numbers, that no image or file is stored, that no new recipient
    exists, and that the assistant still sends none of them;
  - `docs/privacy/lia.md:106` keeps its true sentence about media and gains one sentence in
    the necessity paragraph naming the label and the outcome as the content this unit needs;
  - `docs/privacy/bot-assistant-privacy-policy.md:20-24` tells members in plain words that a
    sticker or a dice roll they send is kept as its label and its number, never as a picture.
- **AC15** A decision file per the repository's convention records the three decisions with
  standing beyond this unit: the core's neutral wordless vocabulary with the platform label as
  opaque data; the assistant sends no sticker and no dice, structurally; and custom emoji are
  read as the platform's own fallback and never sent. Numbers are assigned at merge —
  0106 through 0119 are claimed by `01-receiving-media.md` and further numbers by the other
  sibling specs, so the implementer takes the next free ones and says so in the commit.

## Notes for launch

- **Order against unit 01.** This unit rides unit 01's decision 0106: an empty text column and
  a renamed `Skip::NothingToRecord`. If unit 01 has merged, this unit adds two more cases to
  that skip's escape and nothing else changes. If it has not, this unit makes the same change
  itself, for its own two cases, under the same reasoning — the skip becomes "neither text,
  caption, sticker nor roll" and `Skip::NoText`'s display string is reworded. Either way the
  two units must not both introduce the empty-text allowance; whichever merges second adapts.
- **Sites, exactly:**
  - Adapter, `crates/adapters/telegram/src/client.rs` — `Incoming` (`:125-144`) gains
    `sticker: Option<StickerContent>` and `dice: Option<DiceContent>`; two new decode structs
    beside it carrying only `emoji` and `set_name`, and `emoji` and `value`. No other sticker
    field is decoded, for the reason `User` (`:217-222`) gives about the display name. No
    change to `CONSUMED_UPDATE_TYPES` (`:103`) — both arrive on the `message` update.
  - Adapter, `crates/adapters/telegram/src/translate.rs` — `Pending` (`:79-114`) gains
    `wordless: Option<WordlessContent>`; the skip at `:164-167` admits a message that carries
    one; the six-entry range table and the U+FE0F strip live here as private helpers; the
    `Skip` display string at `:481` is reworded.
  - Adapter, `crates/adapters/telegram/src/driver.rs` — the `Pending` to `InboundMessage`
    hand-off passes the new field through; no other change.
  - Core, `crates/core/src/message.rs` — the `WordlessContent` enum with its two variants,
    re-exported through `crates/core/src/lib.rs`; `InboundMessage` (`:171-210`) gains
    `wordless: Option<WordlessContent>` beside `text`.
  - Core, `crates/core/src/kind.rs` — `ChatMessage` gains the five fields, the five
    `Column::new` entries in the descriptor, the `stored_fields` writes and the `parse`
    reads; `projected_text` (`:555-569`) composes the wordless line ahead of the text through
    one new private function; `erase_principal_content` (`:688-705`) and
    `erase_message_named` (`:743-784`) add the five columns to their `SET` lists and
    `MirrorNulls`'s comment (`:710-717`) updates its count.
  - Core, `crates/core/src/schema.rs` — five column constants; one `WORDLESS_MIGRATION`
    appended last in `store_config()` (`:373-395`), shaped on `LITERAL_ADDRESSED_MIGRATION`
    (`:361-367`); the frozen form list beside the other frozen lists (`:113-123`) and its pin
    in the tests (`:397-436`).
  - Prompt: the operator's prompt file, beside unit 01's file-line passage.
  - Docs: the four privacy documents named in AC14 and the decision files of AC15.
- **What is deliberately absent, so nobody re-derives it.** There is no `set_message_sticker`,
  no roll tool, no `entities` decoding, no sticker download, no attachment record, no new
  outbound arm and no new configuration key. Each absence has a decision above with its
  reasons; a later unit that wants one of them starts by disagreeing with that decision in
  writing.
- **The one inference this unit records as an inference.** That a dice animation runs for a
  few seconds on members' clients, so a bot commenting immediately would spoil it, is a claim
  about client behaviour that the API documentation never makes. Nothing built here depends on
  it: the assistant sends no dice for the documented reasons, and the projection reports a
  member's roll without narrating it because narration would be invention, not because of the
  timing.
- **The seam for looking at a sticker, if it is ever wanted.** Fetch once per
  `file_unique_id` through unit 01's sink, store the bytes as an ordinary attachment, and take
  the impact assessment again under the trigger at `dpia.md:566` before any image reaches the
  model provider. That is a unit of its own and it starts with a product decision this spec
  does not make.
