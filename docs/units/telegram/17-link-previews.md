# Telegram unit 17 — link previews: the assistant's messages carry none, and the option says so on every send

Date: 2026-08-27. A link preview is the card the platform draws under a message — a title, a
description, usually an image — built from a page the message names. The platform generates
it; the sender's only say in it is one optional object on the send call. This project sends no
such object today, so every message the assistant has ever posted took the platform's default,
which is a choice nobody in this project made.

**Read this before the rest: the property has a boundary, and the boundary is the platform's.**
Switching previews off is possible for exactly one kind of send — the plain text message the
assistant uses today. The platform's newest text sends, `sendRichMessage` and
`sendRichMessageDraft`, accept no preview option at all, and the documentation states nowhere
what a captioned media send draws. So this unit does not establish "the assistant never shows a
card". It establishes "every send this project makes goes through one function, and that
function switches previews off", and it makes any send that would leave that function fail a
test instead of reaching the group. Unit 02's captioned sends and any future rich message are
named below as fresh decisions, not as things this unit has already covered.

The default decides more here than it looks. Answers are required to come from tool lookups
(`docs/units/16-grounded-answer-discipline.md`), and those lookups put addresses in front of the
model: the commit lookup prints "Link: {link}" from the forge's `html_url`
(`commit.rs:147,161`), the release lookup does the same from the mirror (`release.rs:163,170`),
and the wiki lookup returns a whole wiki page's markdown to the model (`wiki.rs:208-220`), which
carries whatever addresses that page carries. The deterministic privacy answer prints the
configured policy address (`privacy.rs:210-215`). Most answers this assistant sends therefore
name an address, and under the default each one asks the platform to fetch that address and show
the group whatever came back.

Two properties of that card decide the unit. The address is chosen inside prose a member can
steer: a member writes a message, the model answers it, and whichever address ends up in the
answer decides what the group sees under the assistant's name. And nobody in this project can
see the result — the platform reports the options a message was sent with, never the card it
drew, and the send response is discarded in any case (`client.rs:569`). So the unit sends
`link_preview_options` with `is_disabled` set on every message, writes it in the one function
that builds a text message so no later send can omit it, and records what would have to change
for the reverse to be a decision instead of a parameter.

The inbound half is short and is also a refusal. The assistant reads nothing about a member's
preview, stores nothing about it, and changes nothing about it: the platform offers no way to
touch another person's preview short of deleting their message, which decision 0070 refuses.

Everything below was checked on 2026-08-27 against the live Bot API page and the changelog, and
against this tree at `bd70be2`. Do not re-research it; verify against the tree and the build.

## Grounding

### The platform

**Version.** The changelog's newest entry is **Bot API 10.3, 24 August 2026**; 10.2 is 14 July
2026, 10.1 is 11 June 2026, 10.0 is 8 May 2026. The brief for this unit named 10.1 as current,
which is two releases behind — the same correction units 01, 02, 03, 04, 07, 12, 14 and 18 each
recorded. All three of 10.1, 10.2 and 10.3 are rich-message releases; none of them touches
`LinkPreviewOptions`, whose five fields still read exactly as Bot API 7.0 left them.

**`LinkPreviewOptions`** — "Describes the options used for link preview generation." Five
fields, every one optional, quoted verbatim:

| Field | Type | Description, verbatim |
|---|---|---|
| `is_disabled` | Boolean | "*True*, if the link preview is disabled" |
| `url` | String | "URL to use for the link preview. If empty, then the first URL found in the message text will be used." |
| `prefer_small_media` | Boolean | "*True*, if the media in the link preview is supposed to be shrunk; ignored if the URL isn't explicitly specified or media size change isn't supported for the preview" |
| `prefer_large_media` | Boolean | "*True*, if the media in the link preview is supposed to be enlarged; ignored if the URL isn't explicitly specified or media size change isn't supported for the preview" |
| `show_above_text` | Boolean | "*True*, if the link preview must be shown above the message text; otherwise, the link preview will be shown below the message text" |

Three facts sit inside those descriptions and each one matters to a decision below. The
previewed address **need not appear in the message text at all**: `url` names any address, and
the scan of the text is only what happens when `url` is empty. The two size preferences do
nothing unless `url` is given, so a caller wanting a large card must also name the address
explicitly. And `show_above_text` puts the card above the words, which is the platform stating
that the card competes with the message for the reader's first glance.

**Where the object is accepted.** The key `link_preview_options` occurs six times on the whole
API page. Four are places a bot can set it:

- **`sendMessage`** — parameter, optional, "Link preview generation options for the message".
- **`editMessageText`** — parameter, optional, same description.
- **`editEphemeralMessageText`** — parameter, optional, same description.
- **`InputTextMessageContent`** — field, optional, "*Optional*. Link preview generation options
  for the message".

The other two are read-only facts carried inbound: **`Message.link_preview_options`**,
"*Optional*. Options used for link preview generation for the message, if it is a text message
and link preview options were changed", and **`ExternalReplyInfo.link_preview_options`**,
"*Optional*. Options used for link preview generation for the original message, if it is a text
message". Neither is a lever.

**Where it is not accepted, and what that absence does and does not prove.** No captioned send
takes it: `sendPhoto`, `sendDocument`, `sendVideo` and the rest of unit 02's methods have no
preview parameter. That means the sender has no lever there. It does **not** mean no card is
drawn — the documentation states nowhere whether a caption naming an address produces a preview,
and absence of a parameter is not absence of a behaviour. This is named as an unknown, not
resolved, and it is why the contract below is worded as a property of the send path and not
of every pixel the group sees. `forwardMessage` and `copyMessage` do not take it either, and
unit 12 calls neither. `sendMessageDraft` — "Use this method to stream a partial message to a
user while the message is being generated. Note that the streamed draft is ephemeral and acts as
a temporary 30-second preview - once the output is finalized, you must call `sendMessage` with
the complete message to persist it in the user's chat." — takes `chat_id` ("Unique identifier for
the target **private chat**"), `message_thread_id`, `draft_id`, `text`, `parse_mode`, `entities`,
`can_stop` and `keep_on_stop`, and no preview options. Any secondary source claiming otherwise is
wrong; the page does not.

**The loud finding: the newest way to send text has no preview control at all.**
`sendRichMessage`, added with rich messages in Bot API 10.1 and extended in 10.2 and 10.3, takes
`business_connection_id`, `chat_id`, `message_thread_id`, `direct_messages_topic_id`,
`ephemeral_message_parameters`, `rich_message`, `disable_notification`, `protect_content`,
`allow_paid_broadcast` and the rest — and **no `link_preview_options`**. Its content object,
`InputRichMessage`, is documented as "Exactly one of the fields `html`, `markdown`, or `blocks`
must be used", so a rich message carries HTML or markdown that can name addresses exactly as a
text message can, with no parameter to switch a card off. `sendRichMessageDraft` has none
either. The asymmetry is real and documented: `editMessageText` and `editEphemeralMessageText`
each accept **both** `rich_message` and `link_preview_options`, while the two rich sends accept
neither preview field. The property this unit establishes is therefore defined only for the plain
text send, and no parameter can carry it into rich messages. That is why decision 5 below makes
rich messages a fresh decision instead of an implementation detail.

**The default, marked as an inference and not a quotation.** The page states nowhere in one
sentence what happens when the object is absent. That an absent object means a preview is
generated follows from the field being "*True*, if the link preview is disabled" and from the
parameter it replaced having existed to switch previews off. It is stated here as an inference
so nobody later cites it as documentation.

**History.** Bot API 7.0, 29 December 2023, under "Link Preview Customization": "Allowed to
explicitly specify the URL that will be used for link preview generation in outgoing text
messages."; "Allowed to position link previews above the message text."; "Allowed to choose
media size in link previews."; "Added the class *LinkPreviewOptions* and replaced the parameter
*disable_web_page_preview* with *link_preview_options* in the methods *sendMessage* and
*editMessageText*."; "Replaced the field *disable_web_page_preview* with *link_preview_options*
in the class *InputTextMessageContent*."; "Added the field *link_preview_options* to the class
*Message* with information about the link preview options used to send the message." The string
`disable_web_page_preview` occurs zero times on today's page, checked 2026-08-27: the old
parameter is gone, not deprecated, so a snippet copied from an older example would be accepted
and ignored.

**The group's own permission, and whose it is.** `ChatPermissions.can_add_web_page_previews` is
"*True*, if the user is allowed to add web page previews to their messages", and
`ChatMemberRestricted.can_add_web_page_previews` is the same sentence with "*Optional*." in
front. `setChatPermissions` and `restrictChatMember` both carry
`use_independent_chat_permissions`, whose description names `can_add_web_page_previews` among
the permissions that otherwise imply the send permissions. This is a restriction on people,
applied by administrators, and it is exactly the kind of effect decision 0070 keeps in
administrators' hands. The documentation says nothing about how it applies to a bot's own
sends, so nothing in this unit depends on it.

**What the platform never reports to a bot.** There is no field, method or update that says what
a preview actually rendered — no title, no description, no image, no fetched address.
`Message.link_preview_options` comes back only "if link preview options were changed", and what
it returns is the options, never the card. A bot cannot inspect, log or assess the content shown
under its own message.

**What the platform never lets a bot do.** No method changes the preview of a message the bot
did not send. `editMessageText` edits the bot's own messages and, on a business connection,
messages sent on the account's behalf; it edits nothing else. The single mechanism that removes
another person's preview is removing their message, and that path is decided elsewhere:
`deleteMessage` gives an administrator bot the power to delete any message in a group (quoted in
full at `03-editing-messages.md:67-80`), and decision 0070 refuses it.

**Four things the documentation does not say, named as unknowns so nobody builds on them.** It
does not say which party performs the fetch that produces a card, only that the platform
performs "link preview generation". It does not say whether an address that exists only inside a
`text_link` entity — the shape this adapter emits for every markdown link
(`formatting.rs:138-155`) — counts as "the first URL found in the message text", so under the
default it is not predictable from the documentation whether the assistant's own answers would
produce a card at all. It does not say whether a media caption produces a card. And it does not
say whether a rich message generates previews, which is the second half of why decision 5 stops
instead of assuming.

### Our tree, at `bd70be2`

Every anchor below was re-read at `bd70be2`. An earlier draft of this spec pinned them at
`7fb217d`; unit 26 merged in between and moved most of them, which is why the receipts and the
send-path description differ from that draft.

- **One function builds every text send.** `send_body` (`client.rs:549-571`) is the only
  `sendMessage` call in the workspace. It writes `chat_id` and `text` (`:556`), adds
  `parse_mode: "HTML"` when the text is formatted (`:557-559`), adds `reply_parameters` with
  `allow_sending_without_reply` when a reply target is given (`:560-565`), calls `request`
  (`:566-568`) and discards the decoded result (`:569`). The transport posts the value as JSON
  (`post`, `:642` onward, `.json(body)`), so a nested object needs no extra serialization step.
- **One chunk can reach that function up to four times.** This is the fact the earlier draft got
  wrong, and it changes an acceptance criterion. `send_message` (`client.rs:440-460`) splits the
  text with `chunks_within_cap` (`:709-726`, cap `MESSAGE_UTF16_UNIT_LIMIT = 4096` at `:34`) and
  calls `send_chunk_threaded` per piece. `send_chunk_threaded` (`:483-501`, decision 0109) sends
  the chunk with its reply target and, if the platform refuses, sends the same text once more
  with no target. Each of those two attempts is a `send_chunk` (`:521-546`), which sends the HTML
  form and, when the refusal names the formatting (`rejects_formatting`, `:588-595`), re-sends
  the same text unformatted. Threaded-formatted, threaded-unformatted, plain-formatted,
  plain-unformatted: four possible bodies for one chunk, all through `send_body`. A per-call
  preview setting would have to be repeated in four places today, and the count grows with every
  unit that sends text.
- **Both outbound paths go through that client.** The reply consumer (`driver.rs:744`) and the
  returned-item send (`driver.rs:604`) both call `send_message`; nothing else sends text.
- **The client speaks seven methods and only one of them sends text.** `getMe` (`:372`),
  `getUpdates` (`:391`), `getChat` (`:404`), `leaveChat` (`:418`), `sendChatAction` (`:514`),
  `sendMessage` (`:567`) and `getChatAdministrators` (`:579`). No edit, forward, copy, delete or
  rich-message method is called anywhere in `crates/`, checked by a scan of the whole tree: the
  string `RichMessage` occurs zero times.
- **Every wire method name reaches the transport as a double-quoted literal, and every doc
  comment names it in backticks.** The three lines carrying `sendMessage` in the adapter's
  sources are `client.rs:520` and `:548`, both doc comments in backticks, and `client.rs:567`,
  the call, in double quotes. That difference is what makes the scan in criterion 5 decidable
  without a Rust parser, and it is why the criterion is written against the quoted literal.
- **Answers routinely put addresses in front of the model.** The commit lookup prints "Link:
  {link}" from the forge's `html_url` (`commit.rs:147,161`), the release lookup does the same
  from the mirror (`release.rs:163,170`), the wiki lookup returns the page's own markdown
  (`wiki.rs:208-220`) whose links are the page's, and the privacy answer prints the configured
  policy address (`privacy.rs:210-215`, pinned at `:352-357`). The wiki lookup prints no `Link:`
  line of its own — an earlier draft claimed it did and cited a test that pins default base URLs
  (`tools/mod.rs:168-177`); the claim is corrected here, not repeated.
- **A markdown link becomes an anchor, so the address leaves the visible text.** `link`
  (`formatting.rs:138-155`) renders `[label](target)` as `<a href="…">label</a>` for `http` and
  `https` only, everything else staying literal text. Under HTML parse mode the platform reads
  that anchor as a `text_link` entity, which is the same shape unit 18 handles from the other
  direction.
- **The outbound reply carries no rendering options and needs none.** `OutboundReply`
  (`message.rs:414-434`) is channel, text, kind and reply target, the last being
  `Option<ReplyThread>` since unit 26 (`message.rs:380-412`); `deliverable_of`
  (`outbound.rs:528-549`) builds it from a block. Nothing in the core describes how a platform
  draws a message, and this unit adds nothing to that struct.
- **The inbound decoder ignores the field already.** `Incoming` (`client.rs:191-192` and its
  fields) derives `Deserialize` without `deny_unknown_fields`, so a member message carrying
  `link_preview_options` decodes exactly as one without it and the fact is discarded at the wire.
- **The inbound fixture that can carry the field is the wire one, not the unit-test one.** The
  unit tests inside `translate.rs` build `Incoming` by struct literal (`:536`, `:572`, `:898`),
  and `Incoming` has no preview field, so a preview cannot be written into those fixtures at all
  — two such fixtures would be byte-identical values and any assertion between them vacuous. The
  adapter suite's translation tests build updates as JSON and push them through the scripted
  server (`tests/adapter/translation.rs:23-46`, `support.rs:751`, `:758`), which is the only
  place a wire key the decoder does not know can be expressed. Criterion 8 is written against
  that suite for this reason. An earlier draft named the `translate.rs` unit tests; that was
  wrong.
- **`InboundMessage` cannot be compared by equality.** It derives `Debug, Clone` only
  (`message.rs:170-171`). Any criterion phrased as "the two inbound messages are equal" would
  require adding `PartialEq` to a core type, which the contract forbids. Criterion 8 is therefore
  phrased against what the suite can already observe: the stored row and the recorded send.
- **A preview attaching itself is already handled inbound.** Unit 03 records that an
  `edited_message` update "may at times be triggered by changes to message fields that are
  either unavailable or not actively used by your bot" — "a link preview attaching itself, for
  one" — and answers it with the identical-text rule that records nothing
  (`03-editing-messages.md:257-268`). This unit adds nothing there and depends on it.
- **Unit 18 records a different fact, and the two must not be merged.** Unit 18 decides that the
  address behind a `text_link` is recorded and shown to the model, because the plain text cannot
  re-derive it (`18-entities-and-quotes.md:14-17`). That is the address a member's words point
  at. This unit's inbound decision is about `link_preview_options`, which is a display choice
  about the member's own message, and it stays undecoded.
- **The scripted server records whole bodies, and each test owns its own server.** The server
  stores every request as method plus decoded body (`tests/adapter/server.rs:31-34`, recorded in
  `dispatch` at `:410-419`) and hands them back in arrival order (`recorded`, `:269-278`;
  `await_recorded`, `:282-296`). Every test calls `BotApiServer::start()` for itself, so there is
  no suite-wide request log: a check phrased as "every send the suite ever made" is not reachable
  by reading one log. It is reachable by putting the check inside the accessor every test reads
  sends through, which is what criterion 2 specifies. An earlier draft phrased it as a
  suite-wide assertion called from one end-to-end test; that could not have worked, and
  `tests/adapter/end_to_end.rs` holds one test (`:17`) covering plain answer sends, not the five
  cases the draft listed.
- **There are 53 places that read sends, and one arm that answers them.** `"sendMessage"` occurs
  53 times under `crates/adapters/telegram/tests/`, all but one of them a
  `recorded("sendMessage")` or `await_recorded("sendMessage", n)` call; the exception is the
  dispatch arm at `server.rs:449`. Routing all 53 through one accessor is a mechanical change and
  is what makes criterion 2 hold by construction instead of by enumeration.
- **A second scripted server exists, in another crate.** `crates/assistant/tests/process/`
  answers `"sendMessage"` from its own stub (`support.rs:234`) and records sends that
  `process/main.rs` reads. It is a separate test crate and cannot share the adapter suite's
  fixtures. Decision 3 below states why the pin is not duplicated there.
- **The scripted server can refuse a threaded send, but not a send by description alone.**
  `refuse_threaded_sends` (`tests/adapter/server.rs:237-244`) answers a status and description
  for every send carrying `reply_parameters` and serves a plain send normally
  (`send_answer`, `:383-408`). `SendScript` (`:42-50`) has `RateLimited`, `Failing` and
  `Delivered`, and `Failing` answers a fixed description that `rejects_formatting` does not
  match. So no test exercises the unformatted retry today, and criterion 4 needs one new variant.
  An earlier review suggested `refuse_threaded_sends` makes that variant redundant: it does not.
  Scripting a parse-shaped description there fires the threaded refusal and the formatting
  refusal at once, and the sequence that results proves neither recovery cleanly.
- **The chunking test is the place the per-chunk assertion belongs.** `sending.rs:173-201` sends
  an over-cap reply and asserts per recorded send already. The threaded-refusal recovery has its
  own test at `:263`.
- **The precedents for a scan test, and their limits.** The core's vocabulary check is a
  whole-word scan of the core's sources (`crates/core/tests/vocabulary.rs:69-85`) whose matcher,
  `carries_word` (`:64-67`), splits a line on every non-alphanumeric character. That matcher can
  never match `link_preview_options`, because the underscores split it into three words — so the
  vocabulary mechanism is not reusable here and a substring scan is needed instead.
  `crates/adapters/telegram/tests/token_scan.rs:18-27` is the precedent for a scan test that owns
  its own binary inside the adapter's suite and pulls in the shared fixtures. Unit 11's criterion
  2 (`11-pinning.md:392-397`) is the precedent for a scan proving a method is never called;
  unit 11 is unimplemented, so it is a precedent in shape only and its scan has a hole this unit
  closes (see decision 5).
- **The published privacy statements.** The record of processing lists the chat platform as an
  independent controller whose own handling of the same messages is "unchanged by the assistant"
  (`docs/privacy/records-of-processing.md:85`), and the impact assessment's mandatory review
  triggers include "Any new path that sends message content off the machine"
  (`docs/privacy/dpia.md:564`). No document under `docs/privacy/` mentions previews at all.
- **Decision numbering.** `docs/decisions` ends at `0109` today — `0106` to `0109` are unit 26's
  records, committed in `8b44b41` — so this unit's numbers start at `0110` and are fixed when it
  merges. An earlier draft said the directory ends at `0105` and described `0106` onward as
  numbers sibling specs claim; both statements are now false.

### Bytes, and the ledger

Nothing here streams and nothing here is stored, so the standing streaming constraint is met by
there being no stream: no file, no upload, no buffer. The whole change is one small object
inside a JSON body that is already built and already posted in one piece. The ledger gains no
column and no row shape — the assistant's answer text is what it always was, and the platform's
rendering of that text was never recorded. If a later decision reverses this one, it reverses
forward: a new dated decision superseding this one, with messages already sent keeping whatever
the platform drew at the time, because no record of that drawing exists to rewrite.

## Decisions taken with this unit

- **Every message the assistant sends carries `link_preview_options` with `is_disabled` true,
  2026-08-27.** Three reasons stack and each alone would decide it. A card shows content this
  project did not write, under the assistant's name, in a group that reads the assistant as a
  project voice. The address that decides the card sits in prose the model wrote in answer to a
  member, so a member who can steer an answer into naming an address can put an arbitrary image
  and title under the assistant's message; with previews off the same input produces at most a
  line of text. And naming an address is what asks the platform to fetch it, so with previews
  off the assistant never causes a request to a third-party host on the strength of
  member-supplied prose. On top of those, nobody in the path can see the result: the platform
  reports the options and never the card, so an answer with a preview carries content that
  cannot be inspected, logged or accounted for afterwards.
  *Rejected:* taking the platform's default, which is what ships today — it is a choice nobody
  made, and the documentation does not even state it in a sentence, so the group's experience
  would rest on an undocumented behaviour. *Rejected:* previews on for the project's own hosts
  and off elsewhere — deciding per message means reading the answer's text and choosing, which
  is behaviour, and the adapter is where the choice would have to sit; a host list also answers
  the wrong question, since even a wiki card shows words the model did not write and the
  assistant still cannot see them. *Rejected:* previews on with `url` pinned to a fixed project
  address — that shows the same card under every answer, which is advertising, not information.
  *Rejected:* leaving it to the group's `can_add_web_page_previews` permission — the
  documentation states no effect on a bot's own sends, and it would make the assistant's output
  depend on a setting an administrator can change with nobody noticing.

- **Off is a fixed constant of the adapter's wire layer, not a field on the outbound reply,
  2026-08-27.** The invariant says an adapter contains no behaviour, and this decision sits close
  enough to that line to be argued explicitly. A value that never varies decides nothing per
  message: it is how this platform is spoken to, exactly as `parse_mode: "HTML"`
  (`client.rs:557-559`) and the 4096-unit chunk cap (`:34`) are. Behaviour would be choosing
  between values, and the moment a value has to be chosen it stops being translation and has to
  move to the core. Putting the choice in the core now would put a concept there that only this
  platform has: on Matrix, the next adapter on the roadmap, previews are generated by the
  receiving client and the sender has no say at all, so a neutral-looking `preview: bool` would
  translate to nothing there — the platform's model wearing neutral words, which the vocabulary
  check cannot catch because "preview" is ordinary English.
  If the decision ever does have to vary per message, the neutral form is named here so nobody
  invents a worse one under time pressure: the core would state a property of the **content**,
  not of the rendering — that a stretch of the answer repeats an address the assistant did not
  choose — and each adapter would render that property with whatever it has. The core says what
  is true about the text; the platform decides what to draw.
  *Rejected:* `OutboundReply` gaining a preview field whose only value is "off" — one decision
  written in two places, and the second will eventually disagree with the first. *Rejected:* an
  operator configuration key — it changes what a group sees under every answer, is decided once
  at deployment and is invisible afterwards, and it would make the pinned test conditional on
  configuration.

- **The option is written where the body is built, and the check that nothing sends text around
  it is two separate mechanisms, 2026-08-27.** The option goes into `send_body`
  (`client.rs:549-571`), the only function in the workspace that constructs a text message, so
  the formatted send, the unformatted retry, the plain retry of a refused thread and every chunk
  carry it without any caller knowing it exists. Proving that takes two mechanisms and not one,
  because neither alone is sound:
  a **runtime** mechanism, which asserts the key on every send the adapter's tests ever observe;
  and a **source** mechanism, which asserts there is exactly one wire call and exactly one place
  the key is written. The runtime one proves the key reaches the wire on every path a test
  exercises; the source one proves no untested path exists. Together they say what "inside
  `send_body`" was meant to say, and unlike a scan for a function's extent they need no parser
  and no rule about which side of a `fn` line a doc comment falls on.
  The runtime mechanism is placed in the accessor and not in each test: `BotApiServer` gains
  `sends()` and `await_sends(count)`, which return the recorded `sendMessage` requests and assert
  the key on each before returning, and every existing reader is moved onto them. Every test that
  looks at a send therefore inherits the check, and a test written next year inherits it too.
  *Rejected:* setting the option at each call site (four bodies today, more later, and the one
  that forgets is the one that ships a card). *Rejected:* a typed wrapper around the request body
  (a whole type to carry one value that has exactly one place to live). *Rejected:* a helper each
  test calls (opt-in, so the test that forgets is the test that proves nothing). *Rejected:*
  putting the assertion inside the server's request handler — a panic there happens on the
  server's own task, where it hangs or is swallowed instead of failing the test. *Rejected:*
  putting the assertion inside `recorded` and `await_recorded` themselves, keyed on the method
  name — a general accessor would then know about one concrete method, which is the smearing the
  engineering standards forbid; a named accessor for sends does not. *Rejected:* duplicating the
  runtime check in `crates/assistant/tests/process/` — it is a separate test crate with its own
  stub, the send path it exercises is the same adapter code the adapter suite covers, and the
  same decision written in two crates is the second one waiting to disagree.

- **Only `is_disabled` is ever sent, 2026-08-27.** The emitted object is `{"is_disabled": true}`
  and nothing else: no `url`, no `prefer_small_media`, no `prefer_large_media`, no
  `show_above_text`. A size preference or a position beside a disabled preview states two
  intentions in one object and the documentation defines no behaviour for the combination.
  Keeping the object minimal also keeps the assertion exact, so a half-configured preview cannot
  arrive as an extra key nobody asserted on. *Rejected:* naming `url` as an empty string — an
  empty `url` is documented as "then the first URL found in the message text will be used",
  which is the opposite of the intent.

- **Rich messages stay out of this project, and adopting them needs a new decision, 2026-08-27.**
  `sendRichMessage` and `sendRichMessageDraft` accept no preview options at all, while their
  content is HTML or markdown that can name any address. Adopting them would silently end the
  property this unit establishes, and no parameter could restore it: the only remedy the platform
  offers would be sending a plain text message instead. So the refusal is made checkable — no
  rich-message method name may appear in any production source file in the workspace — and any
  future unit that wants rich formatting has to reopen this decision instead of inheriting it.
  The scan walks the `src` tree of every crate and not the `tests` trees, which is what keeps it
  from matching the needles in its own source; unit 11's scan is written over all of `crates/`
  and would match itself, and this unit does not edit that spec but does not copy the hole
  either. What the narrower walk gives up is a rich-message call inside a test, which sends
  nothing to a group and would be answered by a scripted server. *Rejected:* adopting rich
  messages for better-looking answers and accepting whatever previews appear (it hands the
  member-steerable card back, by a different door). *Rejected:* saying nothing and letting a
  later implementer discover the asymmetry (the scan exists precisely so the discovery happens at
  the test, not in the group). *Rejected:* excluding the scan's own file by path — it works, but
  it names a file inside the check and the next test file added beside it is unprotected, where
  the `src`-only walk states a rule instead of an exception.

- **`docs/platform-vocabulary.txt` is not touched, 2026-08-27.** Two sibling specs grow that file
  for their own words — unit 11 adds three pin method names, unit 16 adds `typing` and
  `sendchataction` — so the choice not to is stated instead of assumed. Three reasons. The
  file's own check scans `crates/core` only, while the refusal this unit needs to make checkable
  is about the whole workspace, so the file would carry a word whose real check lives elsewhere.
  The matcher cannot express the key in any case: `carries_word` splits on every non-alphanumeric
  character, so `link_preview_options` is three words to it and never matches as one. And the
  word that would actually have to go on the list is "preview", which is ordinary English a core
  comment may legitimately use. *Rejected:* adding `sendrichmessage` and `sendrichmessagedraft`
  to the list so the vocabulary check carries part of the refusal — it would be one decision
  recorded in two places with two different reaches, and the narrower reach would be the one a
  reader trusts.

- **Nothing about a member's preview is read, stored, or acted on, 2026-08-27.** The inbound
  decoder keeps ignoring `link_preview_options` on `Message` and on `ExternalReplyInfo`. A
  member's preview options are a display choice about their own message; recording them would add
  a content column to the message row, a name to both erasure passes and a sentence to the record
  of processing, in exchange for a fact that changes no answer the assistant can give.
  *Rejected:* decoding `url` so that the assessment of `docs/units/15-autonomous-moderation.md`
  can see the address behind a card — new personal data collected for a mechanism that only ever
  files an assessment for a person to read, and the assessment is about the words, not the
  rendering. *Rejected:* suppressing or replacing a member's preview — the platform offers no
  method that touches another person's message except deletion, so this would be the assistant
  acting on a member's message with no human decision in the path, which decision 0070 forbids.
  The consequence is stated plainly instead of left implied: a member's message can show the
  group a card whose address the assistant never receives, so what the assistant records is the
  words and not everything the group saw. That is a limit of what the platform reports to a bot;
  it is named here so the moderation unit's reader knows it, and this unit does not edit that
  unit's spec.

- **No teaching rule about links is added to the model's prompt, 2026-08-27.** The tempting
  sentence is one telling the model not to repeat an address a member posted. It is refused:
  with previews off, a repeated address is a line of text, the mechanism holds without the
  model's cooperation, and a rule the model may or may not follow, sitting beside a mechanism
  that always holds, teaches a later reader that the mechanism is optional. *Rejected:* the
  sentence, for that reason.

- **No privacy document changes, and the reason is recorded instead of assumed, 2026-08-27.**
  No new category of data is collected, nothing new is stored, nothing new reaches the model
  provider, and no new recipient appears — this unit removes an outbound effect instead of adding
  one. The record of processing's sentence that the platform's handling of the same messages is
  "unchanged by the assistant" (`docs/privacy/records-of-processing.md:85`) becomes true in one
  more respect: the assistant never asks the platform to fetch an address on its behalf. The
  impact assessment's review trigger for "Any new path that sends message content off the
  machine" (`docs/privacy/dpia.md:564`) is not met, because no path is added.
  *Rejected:* adding a line to the record of processing anyway, so a reader can see the question
  was considered — a record of processing that lists what a change did not do stops being a
  record of processing and becomes a changelog, and the next reader has to work out which lines
  describe reality. The reasoning belongs in this decision record, which is where it is.
  *Rejected:* treating this as needing no decision record at all — the question is asked of every
  unit and answering it in a commit message puts the answer where nobody looks.

- **What would have to be true to reopen the first decision, 2026-08-27.** A preview would need
  all four of: a message whose whole text is written by a person in this project and not by the
  model, so the address is not member-steerable; the address named explicitly in `url` instead of
  left to the text scan, so what renders is what was decided; a purpose that plain text cannot
  serve, stated in the reopening decision; and the owner's approval, because what a group sees
  under the assistant's name is the owner's call and not an implementation detail. Absent all
  four, the answer stays no.
  *Rejected:* leaving the reopening conditions unwritten, so a future decision starts fresh — the
  reasons above would then be re-derived from memory by whoever wants the feature, which is the
  worst possible reader to re-derive them.

## The unit's contract

Every message the assistant sends to this platform — an answer, an answer's later chunks, the
unformatted retry of a refused chunk, the plain retry of a refused thread, a failure notice, a
report line, a returned item — carries `link_preview_options` set to exactly
`{"is_disabled": true}`, written once in `send_body`, the single function that builds a text
message, so no caller decides it and no new call site can omit it without a test failing; no
preview field other than `is_disabled` is ever sent, and no rich-message method name appears in
any production source in the workspace, so no send path exists that the option cannot reach. The
property is a property of that send path and of nothing else: a media caption's rendering is not
documented and is not covered here, and unit 02 is named as the decision that has to cover it.
The core is untouched: it gains no field, no vocabulary and no knowledge that a platform draws
cards, and the platform-vocabulary check stays clean and unchanged. Inbound, a member's preview
options stay undecoded and unstored, and the assistant neither changes nor assesses another
person's preview. No published document of this project becomes false, no new dependency is
added, no schema changes, and no bytes move.

## Acceptance criteria

1. Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the secret
   scan and the platform-vocabulary scan clean; no new dependency and no schema migration.
   `docs/platform-vocabulary.txt` is unchanged, for the three reasons decision 6 records. That
   this differs from unit 11's and unit 16's criterion 1 is deliberate and argued there, not an
   oversight.
2. Every recorded `sendMessage` body that any adapter test observes carries `link_preview_options`
   equal to `{"is_disabled": true}` — asserted by equality on that value, so an extra preview key
   fails. The check lives in two new accessors on the scripted server, `sends()` and
   `await_sends(count)`, which return the recorded `sendMessage` requests and assert on each
   before returning; all 53 existing `recorded("sendMessage")` and `await_recorded("sendMessage",
   n)` call sites under `crates/adapters/telegram/tests/` are moved onto them, and after the move
   the literal `"sendMessage"` appears in that tree only in `server.rs`. A reviewer checks the
   criterion by that count, not by enumerating which scenarios are covered.
3. A chunked answer carries the option on **every** chunk and not only the first: the chunking
   test (`tests/adapter/sending.rs:173-201`) asserts it per recorded send through the new
   accessor, beside its existing text assertions.
4. Both recoveries carry it. A new `SendScript` variant answering a refusal with a caller-chosen
   description makes `send_chunk` re-send unformatted; a test scripts it and asserts that **every**
   recorded body of that send carries the option, and that the body without `parse_mode` carries
   it too — proving the option belongs to the body and not to the formatted case. The existing
   threaded-refusal test (`tests/adapter/sending.rs:263`) gains the same assertion for the plain
   retry through `send_chunk_threaded`. The criterion is written over every recorded body rather
   than over an ordinal, because with both recoveries live one chunk can produce up to four
   bodies and "the second one" names nothing definite.
5. A source scan over `crates/adapters/telegram/src` finds: exactly one line carrying the
   double-quoted literal `"sendMessage"`; no line carrying `"editMessageText"` or
   `"editEphemeralMessageText"` as a double-quoted literal; and exactly one line carrying
   `"link_preview_options"` as a double-quoted literal. The matcher is a plain substring test on
   each line, and the quoted form is what makes it decidable: every wire method name reaches the
   transport in double quotes and every doc comment names it in backticks, so this criterion
   places no constraint on prose and `send_body`'s doc comment may name the key freely. The
   criterion does not assert which function the lines fall in — with one wire call and one key
   written, and with criterion 2 showing the key on every observed send, there is nowhere else
   for either to be, and asserting a function's extent would need a parser the suite does not
   have. The test states in its own documentation that it is the check behind the decision, so
   deleting it deletes a stated guarantee, not a redundant assertion.
6. A source scan over the `src` tree of every crate in the workspace finds no occurrence of
   `sendRichMessage` or `sendRichMessageDraft`. The walk covers `src` and not `tests`, so the
   scan's own source — which must carry both needles — is outside it and no self-exclusion is
   needed. Its documentation names the reason: those methods accept no preview options, so a send
   through them would end the property criterion 2 pins, and it names the difference from unit
   11's scan, which walks all of `crates/` and would match itself.
7. The core is untouched by this unit: it adds no field to `OutboundReply`, no derive to
   `InboundMessage`, and no file under `crates/core/src`. The criterion is phrased as an
   invariant and not as a field list pinned to a commit, because unit 26 changed
   `OutboundReply.reply_target` from `Option<String>` to `Option<ReplyThread>` while this spec
   was being written and a pinned list would have failed for another unit's reasons. A reviewer
   checks it as an empty diff under `crates/core/`.
8. A member message carrying `link_preview_options` translates the same as one without it. The
   fixture is a wire update built as JSON and pushed through the scripted server in
   `tests/adapter/translation.rs` — the only place a key the decoder does not know can be
   expressed, since `Incoming` has no such field and a struct-literal fixture cannot carry one.
   Three variants go through: `is_disabled`, a `url`, and `show_above_text`. Each stores a
   message row whose text and fields equal those of the same message sent without the field,
   apart from the platform message identifier, and the conversation's block count after the four
   messages is what four messages produce. Nothing new is decoded and nothing new is stored; no
   production code changes inbound.
9. No other observable change: the existing addressing, composing, deletion, report, group
   context, tools and end-to-end tests pass with their request bodies unchanged apart from the
   new key. Because every one of those tests reads a named key off the body
   (`addressing.rs:41`-style) instead of comparing a whole body, the new key breaks none of
   them — and the same style is why criterion 2 puts the positive assertion in the accessor,
   since a key-scoped read can never notice a key that is missing.
10. The decision records ship with the code and are pinned by `crates/assistant/tests/docs.rs` in
    its existing shape (`:379`-style): one record per decision above, each carrying its date and
    its rejected alternatives — including the vocabulary-file decision, the rich-message refusal,
    the reopening conditions and the privacy decision. All ten decisions above carry rejected
    alternatives; an earlier draft had two that did not and the criterion could not have passed
    against it.

## Notes for launch

Exact sites, all anchors verified at `bd70be2`.

- **The option and its home** (`crates/adapters/telegram/src/client.rs:549-571`): one line in
  `send_body` writing `serde_json::json!({ "is_disabled": true })` into the body under
  `link_preview_options`, unconditionally, above the `parse_mode` branch so it reads as a
  property of every text send instead of part of the formatted case. It is written inline and not
  as a module constant: `serde_json::Value` is not const-constructible and `json!` is not a const
  macro, so a `const` here would have to be a string that criterion 5's scan could not see as the
  key. The doc comment on `send_body` states the reason in one sentence and cites the decision
  number; it may name the key in backticks without affecting criterion 5, which matches only the
  double-quoted form. The module documentation needs nothing, since this is neither a token nor a
  transport concern.
- **The send accessors** (`crates/adapters/telegram/tests/adapter/server.rs`, beside `recorded`
  at `:269-278` and `await_recorded` at `:282-296`): `sends()` and `await_sends(count)`, each
  delegating to the existing accessor for `"sendMessage"` and asserting the preview value on
  every returned body before handing it back, with a doc comment naming the decision. Then the
  mechanical move: 53 call sites under `crates/adapters/telegram/tests/`, after which the literal
  `"sendMessage"` survives only in the dispatch arm at `:449`.
- **The scripted refusal** (`crates/adapters/telegram/tests/adapter/server.rs:42-50` and
  `send_answer` at `:383-408`): one new `SendScript` variant carrying a description, answered as
  a refusal, plus the pusher beside the existing ones. Do not reuse `refuse_threaded_sends`
  (`:237-244`) for this: it fires only on sends carrying `reply_parameters`, so a parse-shaped
  description scripted there triggers the formatting retry and the threading retry in the same
  sequence and pins neither.
- **The assertions**: extend `tests/adapter/sending.rs` for the chunked case, the unformatted
  retry and the threaded-refusal retry; put the two source scans in their own test file under
  `crates/adapters/telegram/tests/`, with `token_scan.rs:18-27` as the precedent for a scan
  target that pulls in the shared fixtures. Do not copy `crates/core/tests/vocabulary.rs`'s
  matcher: `carries_word` (`:64-67`) splits on non-alphanumerics and can never match a
  snake_case key. State in the test's own documentation how criterion 6 differs from unit 11's
  scan — unit 11 proves a method is never called and walks all of `crates/`; this one walks `src`
  trees only, so it does not match itself.
- **The translation fixtures** (`crates/adapters/telegram/tests/adapter/translation.rs`, with the
  update builders in `tests/adapter/support.rs:751,758`): one update carrying
  `link_preview_options` in each of the three shapes, asserted to store the same row as the same
  message without it. Not `translate.rs`'s own unit tests: they build `Incoming` by struct
  literal (`:536`, `:572`, `:898`) and `Incoming` has no such field, so both fixtures would be
  the same value. The criterion pins the decoder's indifference, which is a property of
  `Incoming`'s `Deserialize` derive; a future `deny_unknown_fields` would break it, and this test
  is what stops that being silent.
- **Decision records** (`docs/decisions/`): one per decision above, numbered from `0110` — the
  directory ends at `0109`, unit 26's last record.
- **Follow-ups** (`docs/follow-ups.md`): record the platform limit named in the member-preview
  decision — a member's message can carry a preview whose address the assistant never receives,
  so the words the assistant records are not everything the group saw. It is recorded as a known
  limit of what the platform reports to a bot, with no work attached, because the only code that
  could act on it would store new member data for a mechanism that files assessments for people
  to read.
- **Contentions with sibling specs, named and not edited.** Unit 11's criterion 1 adds its three
  method names to `docs/platform-vocabulary.txt`; this unit's criterion 1 leaves the file alone,
  and decision 6 gives the reasons. Unit 16 adds two words to the same file, likewise. Unit 11's
  absence scan walks all of `crates/` and would match its own source; criterion 6 walks `src`
  trees instead, and this unit does not edit unit 11 to match. Unit 03 leaves the assistant's own
  edit path unbuilt; if it is ever built, `editMessageText` accepts `link_preview_options`
  (`03-editing-messages.md:66`) and must go through the same builder — criterion 5's zero-count
  for that literal is what will tell whoever builds it. Unit 02's captioned sends take no preview
  parameter and their rendering is undocumented, so unit 02 owns that question and this unit does
  not claim to have answered it; criterion 6 does not cover `sendPhoto`. Unit 08 refuses inline
  mode, which is what keeps `InputTextMessageContent`'s preview field out of this project
  entirely. Unit 12 calls neither `forwardMessage` nor `copyMessage`, and neither accepts preview
  options in any case. Unit 18 records the address behind a `text_link` entity, which is a
  different field from the one this unit refuses to decode; a reader who merges the two will
  wrongly conclude that preview options are stored.
