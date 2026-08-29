# Telegram unit 18 — message entities, formatted text, and the quoted fragment

Date: 2026-08-27. The platform delivers every message twice: once as plain text, and once as a
list of ranges saying which parts of that text are bold, hidden, monospaced, a link, or a
person. This project reads the first list and drops the second. In the other direction it
writes formatting by hand: the model's markdown becomes the platform's HTML in the adapter's
converter. Three questions were open. What does an incoming entity mean to the model? Does the
core see formatting at all, or only text? And should the assistant ever reply quoting a
fragment of the message it answers?

The answers, in order.

Formatting is presentation, so the core learns none of it and the ledger keeps recording the
platform's plain text. Of the twenty entity types, exactly one carries content that the
recorded text does not contain and no reader of that text can reconstruct: the address behind
words that do not show where they lead. That address is recorded and shown to the model,
because the assistant is asked to assess messages against a group's rules (unit 15) and a
masked address is the commonest shape of the abuse it is asked to notice — today it reads
"click here for the fix" and sees nothing a human would see. And the assistant never quotes a
fragment, because the platform requires the quote to be an exact substring of the original
*including its formatting entities*, this project stores no entities, and a quote that does not
match fails the whole send. The refusal is written as a property of the request the adapter
builds, not as a habit of the code.

Two changes go the other way. A fenced code block's language reaches the platform in the
platform's own form, which is the one thing an assistant for an Android distribution gains
something real from: build logs, shell lines and Nix snippets are most of what it prints. And
the converter's escape set grows the fourth character the platform reserves, closing a defect
this unit found in the existing anchor rendering.

Everything below was checked on 2026-08-27 against the live Bot API page, its changelog and its
formatting section, and against this tree at `7fb217d`. Do not re-research it; verify against
the tree and the build.

## Grounding

### The platform

**Version.** The changelog's newest entry is **Bot API 10.3, 24 August 2026**; 10.2 is 14 July
2026 and 10.1 is 11 June 2026. The brief for this unit named 10.1 as current, which is two
releases behind — the same correction units 01, 02, 03, 04, 07, 12, 14 and 17 each recorded.

**`MessageEntity`, the whole type vocabulary**, read off the live field list in documentation
order: `mention`, `hashtag`, `cashtag`, `bot_command`, `url`, `email`, `phone_number`, `bold`,
`italic`, `underline`, `strikethrough`, `spoiler`, `blockquote`, `expandable_blockquote`,
`code`, `pre`, `text_link`, `text_mention`, `custom_emoji`, `date_time`. Twenty types. The last
is new: **Bot API 9.5, 1 March 2026** "Added the `MessageEntity` type "date_time", allowing bots
to show a formatted date and time to the user", and **9.6, 3 April 2026** allowed `date_time`
entities in a checklist title, a checklist task's text, `TextQuote` and `ReplyParameters`'
quote. `blockquote` arrived in **7.0, 29 December 2023**.

**`MessageEntity`'s fields.** `type`, `offset`, `length`, plus `url` for `text_link` only,
`user` for `text_mention` only, `language` for `pre` only, `custom_emoji_id` for `custom_emoji`
only, and — the two the type gained with `date_time` — `unix_time`, "the Unix time associated
with the entity", and `date_time_format`, "the string that defines the formatting of the date
and time". `offset` is "Offset in UTF-16 code units to the start of the entity" and `length` is
"Length of the entity in UTF-16 code units". Both counts are UTF-16, in a JSON payload whose
strings this process holds as UTF-8.

**Date-time entities render on the reader's client, not in the text.** The format string "must
adhere to the following regular expression: `r|w?[dD]?[tT]?`", where `r` is a relative time,
`w` the weekday, `d`/`D` a short or long date and `t`/`T` a short or long time. "If the format
string is empty, the underlying text is displayed as-is; however, the user can still receive
the underlying date in their local format." So the text carries one rendering and the entity
carries the instant.

**Where entities ride on an incoming message.** `Message.entities`, "For text messages, special
entities … that appear in the text", and `Message.caption_entities`, "For messages with a
caption, special entities … that appear in the caption". Two lists indexing two different
strings. `Message.text` is "the actual UTF-8 text of the message".

**`Message.quote` is a `TextQuote`**: `text`, `entities` ("Currently, only bold, italic,
underline, strikethrough, spoiler, custom_emoji, and date_time entities are kept in quotes"),
`position` ("Approximate quote position in the original message in UTF-16 code units as
specified by the sender") and `is_manual` ("True, if the quote was chosen manually by the
message sender. Otherwise, the quote was added automatically by the server"). Unit 12 recorded
the same object independently (`12-forwarding-and-copying.md:88-90`).

**`ReplyParameters` and the sentence that decides the third question.** The object carries
`message_id`, `chat_id`, `ephemeral_message_id` (10.2), `allow_sending_without_reply`, `quote`,
`quote_parse_mode`, `quote_entities`, `quote_position`, `checklist_task_id` and
`poll_option_id`. `quote` is documented verbatim as: "Quoted part of the message to be replied
to; 0-1024 characters after entities parsing. **The quote must be an exact substring of the
message to be replied to, including bold, italic, underline, strikethrough, spoiler,
custom_emoji, and date_time entities. The message will fail to send if the quote isn't found in
the original message.** Ignored for ephemeral messages." `allow_sending_without_reply` is
documented against a different failure — "the specified message to be replied to is not
found" — so it does not cover a quote that does not match.

**The parse modes.** `parse_mode` takes `MarkdownV2`, `HTML` or the legacy `Markdown`.
`sendMessage`'s `entities` is "A JSON-serialized list of special entities that appear in message
text, which can be specified instead of `parse_mode`" — the two are alternatives, not partners.
`sendMessage`'s `text` is "1-4096 characters after entities parsing", so markup does not count
towards the cap.

**HTML mode's tags**, as the section lists them: `<b>`/`<strong>`, `<i>`/`<em>`, `<u>`/`<ins>`,
`<s>`/`<strike>`/`<del>`, `<span class="tg-spoiler">` and `<tg-spoiler>`, `<a href="…">`,
`<tg-emoji emoji-id="…">`, `<tg-time unix="…" format="…">`, `<code>`, `<pre>`, `<blockquote>`
and `<blockquote expandable>`. Four notes bind this unit: "Only the tags mentioned above are
currently supported"; "All `<`, `>` and `&` symbols that are not a part of a tag or an HTML
entity must be replaced with the corresponding HTML entities"; "**The API currently supports
only the following named HTML entities: `&lt;`, `&gt;`, `&amp;` and `&quot;`**"; and "Use nested
pre and code tags, to define programming language for pre entity … Programming language can't
be specified for standalone code tags", the documented form being
`<pre><code class="language-python">…</code></pre>`. The list of highlightable languages is
"libprisma#supported-languages"; the page states no refusal for a language outside it.

**MarkdownV2's escaping**, for the comparison this unit makes: "In all other places characters
`_`, `*`, `[`, `]`, `(`, `)`, `~`, `` ` ``, `>`, `#`, `+`, `-`, `=`, `|`, `{`, `}`, `.`, `!`
must be escaped with the preceding character `\`", with separate rules inside `pre` and `code`
entities, separate rules inside a link's parentheses, and a documented ambiguity between italic
and underline that needs an empty bold entity as a separator.

**Nesting**, verbatim: "If two entities have common characters, then one of them is fully
contained inside another"; "bold, italic, underline, strikethrough, and spoiler entities can
contain and can be part of any other entities, except pre and code"; "blockquote and
expandable_blockquote entities can't be nested"; "All other entities can't contain each other."

**A `tg://user` address only works inside a link.** Verbatim: "Links `tg://user?id=<user_id>`
can be used to mention a user by their identifier without using a username. Please note: These
links will work only if they are used inside an inline link or in an inline keyboard button.
For example, they will not work, when used in a message text." So a masked link is exactly how
one member names another who has no username, and `tg://emoji?id=` and `tg://time?unix=` are
the same shape for a custom emoji and a date.

**Every reader is shown the address already.** Verbatim, in the same section: "Note that
Telegram clients will display an alert to the user before opening an inline link ('Open this
link?' together with the full URL)." The address behind masked words is hidden from a reader of
plain text, not from a member of the group.

**Custom emoji cannot be sent by this bot.** Stated once per formatting mode: "Custom emoji
entities can only be used by bots that purchased additional usernames on Fragment or in the
messages directly sent by the bot to private, group and supergroup chats if the owner of the
bot has a Telegram Premium subscription." Unit 14 records the same sentence and its one
group-scoped exception (`14-stickers-and-dice.md:130-138`); this unit does not re-decide it.

**A second formatting surface exists, and it accepts markdown directly.** Bot API 10.1 added
rich messages: `sendRichMessage`, `InputRichMessage` ("Exactly one of the fields `html`,
`markdown`, or `blocks` must be used"), and a `markdown` field whose section states "Rich
Markdown is compatible with GitHub Flavored Markdown where possible". It carries headings,
lists, tables, footnotes and formulas, with limits of "32768 UTF-8 characters", 500 blocks, 16
nesting levels and 20 table columns. `sendRichMessageDraft` streams a partial message as a
30-second preview that must then be persisted by a full `sendRichMessage`. The page documents
no fallback for a client that cannot render one. Units 01 and 03 already record the surface as
skipped on their sides (`01-receiving-media.md:48-52`, `03-editing-messages.md:85`).

### Our tree, at `7fb217d`

- **No entity is decoded anywhere.** `Incoming` (`client.rs:123-144`) decodes `message_id`,
  `date`, `chat`, `from`, `sender_chat`, `text`, `caption`, `reply_to_message` and
  `pinned_message`. There is no `entities`, no `caption_entities`, no `quote`. Unknown keys are
  ignored by the decoder, so every entity list arrives on the wire and is dropped in silence.
- **A malformed member refuses the whole batch.** `get_updates` (`client.rs:313-320`) decodes
  into `Vec<Update>` in one step, which is why `PinnedContent.date` carries `#[serde(default)]`
  and `MemberState` decodes leniently (`client.rs:146-160`, `:162-180`): a payload the model
  does not expect degrades to a skip instead of stopping the poll.
- **The recorded text is the platform's plain text.** `text_of` (`translate.rs:466-472`) takes
  `message.text`, or `message.caption` when the message is media with a caption (decision 0017),
  and `translate` stores it verbatim (`translate.rs:165-167`, `:179-194`). Decision 0017 and
  `InboundMessage.text`'s own documentation (`message.rs:200-202`) bind the record to what the
  person typed, never a rewritten form.
- **Addressing is resolved from the text, not from entities.** `mentions_bot`
  (`translate.rs:389-406`) scans the text for `@` followed by the assistant's handle, with
  `buried_in_word` (`:419-431`) excluding an address like `a@b.example` and admitting the
  platform's own `/help@handle` form. `invoked_command` reads the leading token the same way.
  The core's own documentation puts the decision where it is: what "addressed" means on a
  platform is knowledge the adapter resolves (`message.rs:187-191`).
- **The outbound path already counts in UTF-16.** `chunks_within_cap` (`client.rs:599-616`)
  splits on character boundaries and accumulates `character.len_utf16()` against
  `MESSAGE_UTF16_UNIT_LIMIT = 4096` (`client.rs:34`). The platform's counting unit is already
  the one this adapter uses, and a surrogate pair is one Rust `char`, so no chunk boundary can
  fall inside one.
- **The converter is a character scanner that only emits balanced tags.**
  `formatting.rs:1-32` states the two properties it rests on: an opener is written only once its
  closer is found, and text is escaped before any tag is inserted. `to_html`
  (`formatting.rs:54-133`) handles fenced code, inline code, links, bold and italic, and drops
  everything else to escaped text. A fence's language token is read and thrown away
  (`formatting.rs:64-72`).
- **The escape set is three characters, and one attribute is already emitted.** `escape`
  (`formatting.rs:35-46`) replaces `&`, `<` and `>`, and nothing else. `link`
  (`formatting.rs:137-155`) admits only `http://` and `https://` targets and writes
  `<a href="{escape(target)}">`. A target containing a quotation mark — which passes the scheme
  check, `https://a" onmouseover="b` — therefore reaches the platform as an anchor with a second
  attribute the platform does not support. This is a defect in the tree today, bounded only by
  the refusal fallback below; the unit that adds a second attribute is the unit that fixes it.
- **A refused formatting send retries unformatted.** `send_chunk` (`client.rs:410-436`) falls
  back to a plain send when the refusal names the formatting, matched by `rejects_formatting`
  (`client.rs:478-485`) against five description fragments. Every other refusal ends the reply
  there.
- **The reply parameters carry two keys.** `send_body` (`client.rs:439-461`) builds
  `{"message_id": …, "allow_sending_without_reply": true}` and nothing else. Only the first
  chunk threads (`client.rs:377-382`), and only the report's delivery sets a target today
  (decision 0059, `message.rs:381-389`).
- **The message row is a flat set of columns on one content table.** `ChatMessage`'s descriptor
  lists fourteen (`kind.rs:575-595`); `stored_fields` (`kind.rs:444-485`) builds the field map at
  the write and takes six arguments, three of them grouped (`RecordedSender`, `ReplyTarget`,
  `Stamp`); `parse` (`kind.rs:599-637`) reads them back; `assembly.rs:725-740` is the one caller.
- **Two statements null a personal column, not one.** `erase_principal_content`
  (`kind.rs:688-705`) nulls `text`, `origin`, `sent_at`, `reply_target` and `speaker` for every
  row a principal wrote. `erase_message_named` (`kind.rs:743-784`) nulls the same five on the one
  row a deletion mirror names (`kind.rs:750-762`). A third statement,
  `erase_reply_targets_naming` (`kind.rs:799-822`), reaches only the reply reference. Unit 14
  states the rule plainly: a content column not added to both of the first two is personal data
  erasure does not reach.
- **The projection renders one row with no ledger access.** `projected_text`
  (`kind.rs:555-569`) returns the erased marker for a nulled row, then composes
  `[origin] speaker: text` for a user-voiced row. `projected_origin_mark`
  (`kind.rs:168-183`) records that such a mark is prose a member can forge, and that what bounds
  a forgery is the report tool's validation, not the mark.
- **A core-owned admission bound already has a shape.** `storable_speaker` (`kind.rs:116-130`)
  refuses an empty handle, one carrying the projection's separator and one carrying whitespace,
  and its comment states why the bound lives in the core: "The current platform's username
  alphabet can produce none of these; a second platform's could, and the core owns this bound
  instead of trusting every adapter."
- **Schema changes are appended steps.** `store_config` (`schema.rs:373-396`) lists three
  creating statements and eleven appended migrations in order; `SPEAKER_MIGRATION`
  (`schema.rs:318-323`) is the precedent for one nullable TEXT column carrying personal data.
- **The teaching is composed in the core.** `MODERATION_TEACHING` (`teaching.rs:48-61`) is the
  precedent: one constant, taught exactly when the mechanism behind it is registered
  (`teaching.rs:33-35`), stating that a report is an assessment and the administrators decide.
- **The vocabulary check is a word list.** `docs/platform-vocabulary.txt` holds seven platform
  and SDK names and `crates/core/tests/vocabulary.rs` greps the core against them. Nothing in
  this unit's core vocabulary — a link, a target, an address — appears on it.

### The published documents, and the bytes

- `records-of-processing.md:61` gives D1 as "The text of a message, including the caption of a
  media message"; `dpia.md:129` repeats it; `bot-assistant-privacy-policy.md:20-24` says it to
  members. R1 (`records-of-processing.md:82`) describes what reaches the processor as "the
  conversation's text and the public username of each speaker". The erasure sentence at
  `records-of-processing.md:117` and the policy's `:122` enumerate what an erasure empties: "the
  person's message text, send time and reply reference".
- Nothing in this unit moves a file, a byte range or an upload. The one new value on the wire is
  a short string decoded from an update the adapter already receives whole, and the outbound
  path keeps sending chunk by chunk exactly as it does today, so the streaming constraint has
  nothing here to bind.
- Nothing rewrites history. An address is written once at the insert with the row it belongs to,
  and erasure nulls its column exactly as it nulls the text.

## Decisions taken with this unit

- **Formatting is presentation, and the core learns none of it, 2026-08-27.** No core type gains
  a span, a range, a style or a markup string, and the ledger keeps storing the platform's plain
  text. A style says how a client draws words that are recorded in full; the assistant acts on
  the words. The invariant is what makes this cheap to hold: a neutral span model in the core
  would have to be rich enough for the next platform's markup too, and every adapter would then
  convert its markup into ours and back, two lossy conversions where today there are none.
  *Rejected:* an entity list stored beside the text for a later unit to use — a personal-data
  column nothing reads, which unit 14 already refused for `custom_emoji_id` on the same
  reasoning. *Rejected:* storing the message's markdown or HTML form instead of its plain text —
  it contradicts decision 0017's verbatim record and hands the model markup to misread as
  instruction. *Rejected:* passing the platform's `parse_mode` through the core as an option on
  the outbound reply — platform vocabulary in the core, and unit 17 refused the same shape for
  link previews on 2026-08-27.

- **One test decides which entity is carried, and exactly one type passes it, 2026-08-27.** The
  test: does the entity carry content the sender supplied that the recorded text does not
  contain and no reader of that text can reconstruct? Applied to all twenty types. `mention`,
  `hashtag`, `cashtag`, `bot_command`, `url`, `email` and `phone_number` mark characters that are
  in the text already. `bold`, `italic`, `underline`, `strikethrough`, `spoiler`, `blockquote`,
  `expandable_blockquote` and `code` are presentation of text recorded in full. `pre` adds
  `language`, a label over code that is itself present, which nothing branches on and which the
  model reads off the code. `custom_emoji` is answered by unit 14 with the platform's own
  reasoning: the alternative emoji is in the text and the platform defines it as the substitute
  wherever a custom emoji cannot be shown. `text_mention` carries somebody else's account
  identity, refused below on its own reasoning. `text_link` carries a `url` that appears nowhere
  in the text and that no reader of the text can reconstruct: it is carried.
  *Rejected:* carrying nothing and leaving the adapter as it is. The honest cost is that the
  assistant reads "click here for the fix" as the whole message while every member can see where
  it points, and a masked address is exactly the shape of the abuse unit 15's assessment exists
  to describe. *Rejected:* carrying everything and deciding later — thirteen new personal facts
  to erase, to document and to justify, for one reader.

- **`date_time`'s instant is not carried, and the imprecision is stated, 2026-08-27.** The
  entity's `unix_time` is content the recorded text does not literally hold: the text carries one
  rendering, the client re-renders it per `date_time_format` in the reader's own locale, and a
  relative format shows a different string every hour. So this type comes closest to passing the
  test above and still does not: the assistant reads a conversation, not a calendar, nothing in
  it computes on an instant a member wrote, and the column would exist for no reader. The
  residual is recorded plainly: where a member sends a date-time entity, the assistant reads the
  sender's rendering of it, which may differ from what a given reader saw.
  *Rejected:* storing `unix_time` beside the text (a second time column on a row that already
  carries two, feeding nothing). *Rejected:* rewriting the text to spell the instant out (an
  adapter deciding what a message says, against decision 0017 and against the invariant).

- **A spoiler is presentation and stays presentation, 2026-08-27.** The hidden words are in
  `Message.text` in full; the entity says only that a client hides them until a reader asks. The
  assistant therefore reads them as ordinary text, which is what it does today. Unit 01 left the
  media form of this question open — "a spoiler is a member's explicit request not to have
  something shown, which deserves its own decision" (`01-receiving-media.md:769-771`) — and this
  unit answers the text half only, touching neither `has_media_spoiler` nor unit 01's field. The
  residual is stated: an assistant that repeated a member's hidden words into the group would
  reveal them, and nothing in this unit prevents that. What prevents it in practice is decision
  0096 — an answer's substance comes from tool lookups, not from repeating a member's message —
  and unit 12's precedent that the assistant republishes nobody's words.
  *Rejected:* carrying a "part of this was hidden" flag (the model cannot tell which part from a
  flag, so it either ignores it or treats the whole message as untouchable). *Rejected:* carrying
  the hidden ranges so the model can tell (offsets, which the decision below refuses, and a
  second copy of the member's own words). *Rejected:* removing the hidden words before recording
  (an adapter deciding what a message says).

- **A third party's identity is not carried, in either of its two spellings, 2026-08-27.**
  `MessageEntity.user` names a person who is not the sender, who has done nothing to reach this
  assistant, and whose account identifier would sit on somebody else's row where the author-keyed
  erasure cannot reach it — the residual unit 12 refused to widen for quoted fragments
  (`12-forwarding-and-copying.md:247-260`). The display name of such a person is in the text
  already, and decision 0077 removed stored display names. The same reasoning closes the second
  door: the platform documents `tg://user?id=` as workable only inside a link, so a `text_link`
  aimed at one is the same identity in another spelling. The admission bound below therefore
  takes `http://` and `https://` targets only, which also excludes `tg://emoji?id=` and
  `tg://time?unix=`.
  *Rejected:* storing the mentioned person's account identifier (an unreachable identifier for a
  person with no rights path here). *Rejected:* storing the `tg://` address without reading it
  (the same identifier, spelled differently, passing unnoticed).

- **No entity offset is read, converted or stored anywhere, 2026-08-27.** The one carried value
  sits in a field of its own, so the design needs no offset arithmetic at all: the adapter reads
  `url` and passes the addresses in the order the entity list gives them. This is where the
  UTF-16 trap would otherwise appear. An entity's `offset` counts UTF-16 code units while a Rust
  `String` is indexed in UTF-8 bytes, so `&text[entity.offset..]` is wrong for every message
  carrying an emoji before the entity and lands inside a character often enough to panic instead
  of merely mislead. The design removes the arithmetic instead of getting it right, and an
  acceptance criterion pins that no offset is read. The one place UTF-16 genuinely binds this
  project stays where it is and is already correct: the outbound chunker's `len_utf16`
  accumulation (`client.rs:599-616`).
  *Rejected:* storing the visible words beside the address so the model can pair them (it needs
  exactly the slicing described, for a pairing that matters only in a message carrying several
  masked links — with one link the pairing is trivial, and the visible words are in the text
  either way). *Rejected:* storing the offsets for a later unit (an encoding this project would
  have to keep correct forever, in a column nothing reads). *Rejected:* adding a crate to convert
  them (a dependency for arithmetic this design does not perform).

- **The addresses are one nullable column on the message row, filtered and bounded by the core,
  2026-08-27.** `InboundMessage` gains `link_targets: Vec<String>`, the adapter fills it from the
  entity list, and `ChatMessage` gains `COLUMN_LINK_TARGETS` (`link_targets`, TEXT, nullable)
  through an appended migration step, written by `stored_fields` and read back by `parse`,
  newline-separated. The core owns the whole rule as one function, `recorded_link_targets(text,
  targets)`, in the shape `storable_speaker` has: an address is kept when it begins with
  `http://` or `https://`, carries no whitespace and no control character, is at most 4096
  characters, is not already contained in the recorded text, and is not a repeat of one already
  kept; at most eight are kept. The text check is the same test the decisions above apply, spelled
  as code: an address the text already shows is content the model already has. The caps exist
  because this column feeds the model's context and the core does not inherit a length bound from
  whatever a platform hands it. A message with nothing admitted stores NULL and reads exactly as
  it does today.
  *Rejected:* letting the adapter apply the rule (every adapter would restate it, and the
  vocabulary check cannot see a rule written in prose). *Rejected:* a side table keyed by block id
  (a second erasure surface, a second write path and a join, for a short list nobody queries by
  value). *Rejected:* a JSON array in the column (a second encoding for the projection to parse,
  where a malformed value breaks the reading of a whole message instead of losing one address —
  and the bound above admits no value containing the separator). *Rejected:* storing only each
  address's host (it needs a URL parser this workspace does not carry, a wrongly parsed host is
  worse than none, and the path is what distinguishes a wiki page from a payload). *Rejected:* no
  caps at all (one message could write an unbounded column and an unbounded projection).
  The list travels to the write inside a grouped argument: `stored_fields`'s first parameter
  becomes `RecordedContent { text, link_targets }`, following the module's own idiom where the
  sender's facts travel as `RecordedSender` and the decided facts as `Stamp`.
  *Rejected:* a seventh positional parameter (unit 12 adds an eighth to the same call, and a call
  of eight positional arguments is where two of them get swapped).

- **Erasure reaches the column from both nulling statements, 2026-08-27.**
  `COLUMN_LINK_TARGETS` joins the five columns nulled by `erase_principal_content`
  (`kind.rs:695-698`) **and** the five nulled by `erase_message_named` (`kind.rs:752-754`). One
  statement is not enough, and reading only the author-keyed one is the mistake this decision
  exists to prevent: without the second, an administrator's deletion of a message through the
  moderation bot would empty the words and leave the address standing on the same row (decisions
  0082 to 0085). No third reach is needed: unlike a reply target, whose value is another person's
  message identifier, an address is the row author's own content, so the target-keyed pass has
  nothing to do here.
  *Rejected:* treating the column as structure and leaving it standing (it is content a person
  sent; an erasure that empties the words and keeps the address is a broken promise).

- **The model is told, as a mark after the text, 2026-08-27.** `projected_text`
  (`kind.rs:555-569`) appends `(link targets: …)` to a user-voiced row that has stored addresses,
  joined with `, `, after the text and on the same line. The erased branch returns before it,
  exactly as it does for the speaker and the origin mark. The label is fixed and plural in every
  case, including one address: branching on the count is a second decision to keep consistent for
  no reader's benefit. A member can type the same parenthesis into their own text, which is the
  forgery surface the origin mark and the speaker prefix already carry (`kind.rs:176-182`) and is
  bounded the same way — nothing acts on the mark automatically.
  *Rejected:* a projected line per address (it multiplies one message into several, and the
  projection composes one line per block by design). *Rejected:* the mark ahead of the text (the
  text is what the person said; the machine's observation follows it).

- **The teaching says what the mark means, and that nobody is acted on because of it,
  2026-08-27.** One passage in `teaching.rs`, in the shape `MODERATION_TEACHING` has: the mark is
  the assistant's own observation that the message carries a link whose visible words do not show
  where it leads; the addresses are the member's and are never repeated into an answer as a
  recommendation; a masked address is a fact the model may describe when it assesses a message,
  and the assessment goes to the administrators through the existing report tool, where a human
  decides (decision 0070). It composes exactly when the moderation teaching does — the same
  predicate, `moderation_taught` (`teaching.rs:33-35`) — so a deployment that cannot report is not
  taught to reason about abuse it can do nothing about.
  *Rejected:* shipping the mark untaught (new syntax in the projection that the model reads as
  part of the member's words is worse than no mark). *Rejected:* a rule in the code that reports a
  message because it carries a masked address (a machine acting against a person, which decision
  0070 refuses — the model assesses, an administrator acts). *Rejected:* composing the passage
  always (in an addressed-only deployment with no moderation handle, it teaches a judgment with
  nowhere to go).

- **The assistant never quotes a fragment, 2026-08-27.** No `quote`, `quote_parse_mode`,
  `quote_entities` or `quote_position` is ever sent, and `OutboundReply` gains no field for one.
  The platform's own sentence decides it twice over. "The quote must be an exact substring of the
  message to be replied to, **including** bold, italic, underline, strikethrough, spoiler,
  custom_emoji and date_time entities", and this project stores no entities at all by the first
  decision above — so it cannot construct a quote it knows will match a formatted message, and
  the first member to bold a word inside the sentence the assistant quotes gets no answer. Even
  for unformatted text the fragment could only be chosen by the model, models paraphrase,
  `allow_sending_without_reply` covers a different failure, and a quote that does not match
  produces a refusal whose description matches none of `rejects_formatting`'s fragments
  (`client.rs:478-485`), so the reply ends there with the answer lost. Threading already shows
  which message an answer belongs to (decision 0059).
  *Rejected:* checking the fragment against the stored text of the target before sending (the core
  would hand the adapter a fragment, the adapter would need the original's text to check it, a
  check that passes against our stored copy can still fail against the platform's — an edited
  message is the plain case, and unit 03 records that edits are not followed — and none of it
  reaches the formatting half of the platform's rule). *Rejected:* quoting the first N characters
  of the target mechanically (a machine choosing which part of a person's words to hold up in
  front of a group, with no human deciding).

- **The reply parameters object keeps exactly the two keys it has, and a test says so,
  2026-08-27.** `send_body` (`client.rs:439-461`) is the one function that builds a text message,
  and the assertion is written against the object it constructs: `reply_parameters` has the key
  set `{message_id, allow_sending_without_reply}` and no other. That makes the refusal above a
  checkable property of the request instead of an absence nobody notices, in the shape unit 17
  uses for the link-preview option on the same function.
  *Rejected:* asserting that the string "quote" does not appear in the adapter (it would pass while
  the object grew any other key, and it would fail on an unrelated comment).

- **The incoming quote stays undecoded, upholding unit 12, 2026-08-27.** `Message.quote` and
  `Message.external_reply` are still not decoded, for the reason unit 12 recorded on 2026-08-25
  and this unit does not reopen: a quote is a verbatim fragment of one person's message carried
  inside a different person's message, and an author-keyed erasure cannot reach it
  (`12-forwarding-and-copying.md:247-260`). `is_manual` therefore never enters this process, and
  neither does `TextQuote.position`, the second UTF-16 offset the platform offers. The cost is
  repeated here because this is the unit somebody will search: when a member replies quoting a
  message the assistant never recorded, the assistant never learns what was replied to. The
  prerequisite for changing that is decision 0063's reach key, not a new column.
  *Rejected:* decoding the quote only when the store already holds the quoted message (unit 12
  refused it with the reason still standing — the useful case is the other one).

- **The decode is a filter, and unit 14's conclusion is kept as a property, 2026-08-27.** Unit 14
  decided that "`Incoming` gains no `entities` field, so no `custom_emoji_id` enters this process"
  (`14-stickers-and-dice.md:437-448`), and pinned it as AC8. This unit adds the field, so the part
  of that decision that matters is restated as a property instead of an absence: the decoded
  entity shape has two members, `type` and `url`, both optional with `#[serde(default)]` so a
  malformed entity degrades to one that contributes nothing — the leniency `PinnedContent` and
  `MemberState` already have, for the reason `get_updates` gives. No `custom_emoji_id`, no `user`,
  no `language`, no `unix_time`, no offset. Unit 14's AC8 is superseded in its structural half and
  upheld in its substance; its test asserts the narrower property.
  *Rejected:* decoding the whole `MessageEntity` shape and filtering later (the unread fields
  would sit in the process, and the next change to touch them would face no decision).

- **The entity list is chosen by the same rule that chose the text, 2026-08-27.**
  `link_targets_of` reads `entities` when `text_of` returned `message.text` and `caption_entities`
  when it returned `message.caption` — one function deciding both, so the pairing cannot drift.
  The two lists describe two different strings, and using `entities` for a caption would describe
  a string that is not the one recorded. The mistake is invisible in this design, since no offset
  is read, but the pairing is what makes an address belong to the text it was sent with.
  *Rejected:* concatenating both lists (a message carries one or the other, and a concatenation
  hides which).

- **A pinned announcement gains no addresses, 2026-08-27.** The rules statement is read through
  `PinnedContent` and stored as a context note (D4), a different kind with a different erasure
  position, and it is written by administrators, not by the members the assessment is about.
  Adding a second carrier of addresses there doubles the surface for no reader.
  *Rejected:* extending the note (a second storage site and a second document change, to show the
  model addresses inside a rules text it is told to treat as the group's rules).

- **The converter gains a fence's language, and its escape set gains the fourth reserved
  character, 2026-08-27.** `to_html` (`formatting.rs:64-72`) reads a fence's language token and
  drops it; it becomes `<pre><code class="language-…">…</code></pre>`, the platform's documented
  form, when the token is non-empty and made only of ASCII letters, digits, `+`, `-`, `.`, `_` and
  `#`; anything else keeps today's bare `<pre>`. And `escape` (`formatting.rs:35-46`) gains
  `"` → `&quot;`, one of the four named entities the platform accepts. The two go together: the
  alphabet keeps a model-written token from carrying anything strange into an attribute, and the
  escape fixes the anchor defect the tree already has, where a link target containing a quotation
  mark writes a second attribute into `<a href="…">`. One escape function keeps one decision in
  one place; text nodes escaping a quotation mark is harmless, since the platform parses the
  entity back and counts the message "after entities parsing". The balanced-tag property is
  unchanged: the opener is still written only once the closing fence is found. The platform
  documents no refusal for a language outside its highlighting list, and if one comes,
  `send_chunk`'s fallback delivers the answer unformatted (`client.rs:410-436`).
  *Rejected:* rendering blockquotes for the model's `>` lines (a blockquote is line-structured,
  and the converter is a character scanner whose safety property is per-character degradation;
  threading already shows what an answer replies to). *Rejected:* spoilers and custom emoji in the
  assistant's own messages (it has no reason to hide its own words, and the platform's rule makes
  custom emoji unavailable to this bot — unit 14 records both). *Rejected:* switching to
  `MarkdownV2` (eighteen characters to escape in ordinary prose, with different rules inside code
  spans and link parentheses and a documented italic-underline ambiguity, over prose written by a
  model, against a design that escapes first and inserts second and is tested against unpaired
  markers). *Rejected:* sending an `entities` array instead of a parse mode (the platform's own
  alternative, and it needs exactly the UTF-16 offset arithmetic this unit removed, recomputed for
  every chunk after the chunker splits the text).

- **Rich messages are not adopted here, and the question is named for its own unit, 2026-08-27.**
  `sendRichMessage` takes a `markdown` field compatible with GitHub Flavored Markdown, which is
  close to what the model already writes and would replace this converter with a platform call.
  It is not this unit's change: it is a different send method returning a different message
  shape, so the chunker, the refusal fallback, the unformatted retry and unit 17's preview option
  all sit on a path it does not use, and the page documents no fallback for a client that cannot
  render one. A unit that adopts it must answer those four, not this one.
  *Rejected:* adopting it inside this unit (a rewrite of the outbound path smuggled into a unit
  about entities). *Rejected:* leaving it unmentioned (the next person to read this converter will
  ask, and the answer should be recorded once).

## The unit's contract

The core holds no notion of formatting: it stores the platform's plain text and no style, span
or markup, and no adapter hands it any. Of the twenty entity types exactly one contributes to
the record — the address behind a `text_link` — carried as an ordered list on the inbound
message, filtered by one core-owned function that admits only `http`/`https` addresses the
recorded text does not already contain, capped in count and length, stored in one nullable
column that both of the store's content-nulling statements empty, projected to the model as a
fixed mark after the message, and explained in one teaching passage that composes only where a
report can be filed. Nothing acts on it automatically; a human decides, as decision 0070
requires. No entity offset is read, converted or stored anywhere in the workspace, and the only
UTF-16 counting in the project stays the outbound chunker's. A `text_mention`'s person, a
`tg://` target, a `custom_emoji_id`, a `pre` language, a `date_time` instant, an incoming quote
and its `is_manual` are all recorded as refused, each with its reason. On the way out the
model's markdown is still converted to the platform's HTML by the same scanner, now carrying a
fenced block's language in the platform's own form and escaping all four characters the platform
reserves, and the reply parameters carry exactly the two keys they carried before: the assistant
sends no quote, and a test proves the object cannot grow one unnoticed. The record of
processing, the impact assessment and the member-facing policy state that a stored message
includes the address behind a link whose words hide it. No new dependency, no new recipient, no
new update type consumed.

## Acceptance criteria

1. Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
   platform-vocabulary scan and the secret scan clean; no new dependency. The one schema change
   is an appended migration step, and a fresh store and an upgraded store end at the same schema.
2. A message carrying one entity of every type other than `text_link` — `bold`, `italic`,
   `underline`, `strikethrough`, `spoiler`, `blockquote`, `expandable_blockquote`, `code`, `pre`
   with a `language`, `custom_emoji` with a `custom_emoji_id`, `date_time` with a `unix_time` and
   a `date_time_format`, `mention`, `hashtag`, `cashtag`, `bot_command`, `url`, `email`,
   `phone_number`, `text_mention` with a `user` — records exactly the row it records today, with
   `link_targets` NULL, and projects exactly the line it projects today. Written as one test over
   the full type list, so a twenty-first platform type fails loudly instead of being carried by
   accident.
3. A `text_link` entity's `url` is recorded: a message carrying two of them stores both in the
   order the platform gave them and projects
   `[id] speaker: text (link targets: https://a.example, https://b.example)`.
4. The entity list follows the text: a media message with a caption reads `caption_entities` and
   a text message reads `entities`, pinned by a test whose two lists differ, so a swap fails.
5. The core's filter holds, each case asserted: an empty target, a `tg://user?id=…` target, a
   `tg://emoji?id=…` target, a `mailto:` target, a target carrying a space, a newline or a
   control character, and a target longer than 4096 characters each store nothing; an address the
   recorded text already contains stores nothing; a repeated address stores once; a ninth address
   is dropped; a mix of admitted and refused stores only the admitted, in order; a message whose
   every target is refused stores NULL and projects bare.
6. No offset arithmetic exists: `MessageEntity`'s `offset`, `length`, `position` and `unix_time`
   are read by no code in the workspace, and no function slices a message's text by an
   entity-derived index. Pinned by a source scan over the adapter and the core, in the shape of
   the existing token scan.
7. A malformed entity does not stop the poll: an update whose entity list carries an object with
   no `type`, an object with an unknown `type`, and an entity list that is not a list at all,
   each still records the message with the addresses the well-formed entities carried.
8. Erasure empties the addresses from both directions: after `erase_principal_content` a row
   carrying addresses reads NULL in `link_targets` and projects the erased marker alone, and
   after the deletion mirror's `erase_message_named` the named row does too. Pinned in the same
   tests that pin the text and the reply target.
9. Nothing acts on a masked address: a message carrying one produces the same stamp, the same
   answer-due, the same budget outcome and the same delivery as the identical message without
   one. The projection is the only new reader of the column, and the teaching passage composes
   exactly when the moderation teaching does.
10. The reply parameters object carries the key set `{message_id, allow_sending_without_reply}`
    exactly, asserted against the body `send_body` constructs; no core type carries a quote
    fragment; no `quote`, `quote_parse_mode`, `quote_entities` or `quote_position` key is built
    anywhere; `Incoming` decodes no `quote` and no `external_reply`.
11. A fenced block's language reaches the platform in its own form: ` ```sh ` renders as
    `<pre><code class="language-sh">…</code></pre>`; a fence with no language and one whose token
    carries a character outside the admitted alphabet both render as today's bare `<pre>`, with
    the token absent from the output either way.
12. The escape set covers four characters: a link target containing a quotation mark renders with
    `&quot;` inside the `href` value and no second attribute, and a member's quotation mark inside
    a code block round-trips through the platform's named entity. The converter's existing
    properties still hold with an attribute present: the truncated-chunk balance test passes over
    a string containing a language-tagged fence, and a member's HTML in a code block is still
    delivered escaped.
13. The published documents say what is stored, each edit dated 2026-08-27: D1
    (`records-of-processing.md:61`) and the impact assessment's matching entry (`dpia.md:129`)
    state that message content includes the address behind a link whose visible words do not show
    it; the erasure descriptions (`records-of-processing.md:117`,
    `bot-assistant-privacy-policy.md:122`) name it among what an erasure empties; R1
    (`records-of-processing.md:82`) states that such an address reaches the processor inside the
    conversation's text; and the policy's own words to members (`:20-24`) say the same. The
    legitimate-interests assessment gains one sentence and no new balance: the platform shows
    every reader the full address before opening the link, so what is collected is what the group
    already sees, for the assessment purpose already assessed. The documents test pins one
    sentence of each so a later edit cannot quietly drop it.
14. The decision records ship with the code in `docs/decisions`, one per decision above, each
    dated, each carrying its rejected alternatives, numbered from the next free number at merge
    time.

## Notes for launch

- Sites, adapter. `client.rs:123-144` gains `entities` and `caption_entities`, each decoded into
  a two-member entity shape whose fields are both optional. `translate.rs` gains
  `link_targets_of` beside `text_of` (`:466-472`), sharing the text-or-caption choice, and
  `translate.rs:179-194` fills the new field. `formatting.rs:35-46` gains the fourth escape and
  `:64-72` takes the language token. `client.rs:439-461` is asserted against, not changed.
- Sites, core. `message.rs:169-210` gains `link_targets`. `kind.rs` gains `COLUMN_LINK_TARGETS`,
  `recorded_link_targets`, `RecordedContent`, the descriptor column at `:575-595`, the write at
  `:444-485`, the read-back at `:599-637`, the projection at `:555-569`, and the column in both
  nulling statements at `:695-698` and `:752-754`. `schema.rs` appends the migration step and
  adds it to the list at `:373-396`. `assembly.rs:725-740` passes the new field. `teaching.rs`
  gains the passage beside `MODERATION_TEACHING` and composes it under `moderation_taught`.
- Merge interactions, each on a file another spec in this folder also touches. Unit 17 adds
  `link_preview_options` to `send_body` and pins the body's shape; this unit asserts the
  `reply_parameters` object's key set only, so the two assertions compose and whichever merges
  second reads the other's pin instead of rewriting it. Unit 12 adds a relay mark to
  `projected_text` between the speaker and the text and an argument to `stored_fields`; this
  unit's mark goes after the text, and the grouped `RecordedContent` is what keeps that call
  readable once both have merged. Unit 14's AC8 asserts that `Incoming` has no `entities` field
  and must be rewritten to its substance — that no `custom_emoji_id` is read — which the decision
  above records; unit 14's separate assertion that the converter escapes a model-written
  `tg-emoji` tag still holds, and the fourth escape character strengthens it.
- Dependencies named, not re-specified: decision 0017 (the text is what is recorded), decision
  0059 (the outbound reply's target and the adapter's threading), decision 0063 (the reply
  target's erasure residual and its reach key, the prerequisite for ever decoding a quote),
  decision 0070 (the assistant assesses, a human decides), decision 0077 (display names are not
  stored), decision 0096 (substantive answers come from lookups), decisions 0082 to 0085 (the
  deletion mirror, whose nulling statement this unit extends), `01-receiving-media.md:769-771`
  (the media spoiler, still open and untouched here), `12-forwarding-and-copying.md:247-260`
  (quoted fragments stay undecoded), `14-stickers-and-dice.md:130-138,437-448` (custom emoji,
  sending and receiving), unit 15 (the assessment this unit's mark feeds), unit 17 (the preview
  option on the same send function).
- Two follow-ups, recorded and not built. The addressing check reads the text
  (`translate.rs:389-406`), so a member pasting a log that contains the assistant's handle inside
  a code block summons it; the entity list would say the handle sits inside `code`, but which
  mentions count as addressing is a product decision, and the fact that the adapter resolves
  addressing does not make it the adapter's decision to change. And rich messages are the open
  question this unit named and did not answer: a platform surface that accepts markdown directly,
  against a converter this project maintains by hand, decided by the four questions the decision
  above lists.
