# Telegram unit 12 — forwarding and copying: marking words that came from elsewhere

Date: 2026-08-25. Two halves, and they end in opposite places.

The **receiving** half fixes a defect that is in the tree today. When a member forwards
somebody else's message into the group, the adapter records the forwarded text as that
member's own words, with nothing beside it to say otherwise (`translate.rs:179-193` stores
`text_of(message)` and never reads `forward_origin`). Everything downstream then treats those
words as the member's: the model reads them under the member's handle
(`kind.rs:555-570`), the moderation assessment of unit 15 judges them as the member's
statement, and the record of processing describes the store as holding "the text of a
message" without saying whose. This unit records the fact the platform already hands us — that
the words were first said somewhere else — and refuses, deliberately, to record who said
them.

The **sending** half ships nothing. `forwardMessage`, `forwardMessages`, `copyMessage` and
`copyMessages` are all reachable with the token the assistant already holds, and the unit
decides that none of them is called. Forwarding moves a person's words to a place they did
not put them; copying does the same and removes their name on the way. Neither has a purpose
here that the existing report path does not already serve, and building either would put a
new recipient into the record of processing and a new power over a person into a bystander
bot. The reasoning, with what was rejected, is below, and the seam that would accept a
human-instructed forward later is named so the refusal is a decision and not an omission.

## Grounding

Verified against the live Bot API documentation, page fetched 2026-08-25, and against the
tree at `1891fcd`. Every anchor below was opened and read.

### The platform

**Version.** The changelog's newest entry is **Bot API 10.3, 24 August 2026**. 10.2 is 14 July
2026 and 10.1 is 11 June 2026. The brief for this unit named 10.1 as current; it is two
releases behind. Nothing in 10.1, 10.2 or 10.3 changes forwarding, copying or `MessageOrigin`.

**`forwardMessage`** — "Use this method to forward messages of any kind. Service messages and
messages with protected content can't be forwarded. On success, the sent Message is returned."
Parameters: `chat_id` (required), `message_thread_id`, `direct_messages_topic_id`,
`from_chat_id` (required), `video_start_timestamp`, `disable_notification`, `protect_content`,
`message_effect_id`, `suggested_post_parameters`, `message_id` (required). **There is no
`reply_parameters` and no `caption`**: a forward cannot be threaded onto another message and
cannot be annotated. Forwarding something "as a reply, with a note" is two sends, not one.

**`forwardMessages`** — same prohibitions, plus: "If some of the specified messages can't be
found or forwarded, they are skipped. Album grouping is kept for forwarded messages. On
success, an Array of MessageId of the sent messages is returned." `message_ids` is "a
JSON-serialized list of **1-100** identifiers of messages in the chat `from_chat_id` to
forward. The identifiers must be specified in a **strictly increasing order**." The
documentation states that skipped messages are omitted from the result; it states no
correspondence between the returned array and the input ids, so **a caller of the batch form
cannot tell which input was dropped**, only how many. No `reply_parameters` here either.

**`copyMessage`** — "The method is analogous to the method forwardMessage, but the copied
message doesn't have a link to the original message. Returns the **MessageId** of the sent
message on success." It cannot copy "Service messages, paid media messages, giveaway messages,
giveaway winners messages, and invoice messages", and "A quiz poll can be copied only if the
value of the field `correct_option_ids` is known to the bot". It does take `caption`,
`parse_mode`, `caption_entities`, `show_caption_above_media`, `reply_parameters` and
`reply_markup`. Two consequences that matter: a copy **can** be threaded where a forward
cannot, and the return value is a bare `MessageId`, so the caller never receives the sent
message's own content back.

**`copyMessages`** — same prohibitions and the same 1-100 strictly-increasing `message_ids`
rule, returns an Array of MessageId, keeps album grouping, and offers `remove_caption` but no
caption replacement.

**`Message.forward_origin`** is typed `MessageOrigin`, "Information about the original message
for forwarded messages". `MessageOrigin` has exactly four variants:

| Variant | `type` | What it carries besides `date` |
|---|---|---|
| `MessageOriginUser` | `"user"` | `sender_user: User` — the full user object of the original author |
| `MessageOriginHiddenUser` | `"hidden_user"` | `sender_user_name: String` — a name only; the author's forward privacy hides the account |
| `MessageOriginChat` | `"chat"` | `sender_chat: Chat`, optional `author_signature` |
| `MessageOriginChannel` | `"channel"` | `chat: Chat`, `message_id: Integer`, optional `author_signature` |

So the platform offers us, unasked, the account identifier and username of a person who is not
in this group and has never spoken to this assistant. That offer is the whole privacy question
of the receiving half.

**`Message.is_automatic_forward`** — "True, if the message is a channel post that was
automatically forwarded to the connected discussion group". Such a message also carries
`sender_chat`, documented as "a linked channel for messages automatically forwarded to the
channel's discussion group", and `sender_chat` is exactly what decision 0016 skips today
(`translate.rs:159-160`). Automatic forwards therefore never reach the core, before or after
this unit.

**Quoting.** `Message.quote` is a `TextQuote`: `text` (the quoted part), `entities`,
`position`, and `is_manual` — "True, if the quote was chosen manually by the message sender.
Otherwise, the quote was added automatically by the server." It is the quoted fragment of the
message being replied to, carried inside the replying message.

**`Message.external_reply`** is an `ExternalReplyInfo` for a reply "which may come from another
chat or forum topic". Its fields are `origin` (a `MessageOrigin`), `chat`, `message_id`,
`link_preview_options`, and one field per media kind. **It carries no text and no caption.** So
when a member replies to a message the bot cannot see — a post in the linked channel, or the
original of something forwarded in — the only text of that message the bot will ever receive is
whatever the member quoted. A reply and its quote are the same shape whether the replied-to
message is in this chat or not; only the presence of `reply_to_message` versus `external_reply`
differs.

**Protection.** `Message.has_protected_content` — "True, if the message can't be forwarded".
`Chat.has_protected_content` — "True, if messages from the chat can't be forwarded to other
chats". `forwardMessage`, `forwardMessages`, `copyMessage` and `copyMessages` each take a
`protect_content` parameter, "Protects the contents of the forwarded message from forwarding
and saving", and so does the `sendMessage` the assistant already uses. The documentation does
not state that the chat-level setting sets the per-message flag on every message, so nothing
here is derived from that.

**What the platform never tells a bot.** There is no update for "a member forwarded your
message", none for "a member forwarded somebody's message out of this group", and no method to
ask. The receiving side is the only side of forwarding the assistant can observe at all. This
is the same shape unit T4 recorded for deletions, and that unit already noted in passing that
`forwardMessage`/`copyMessage` are not usable as probes because both send a message to ask
their question (`04-deleting-messages.md:60-63`).

### Our tree

- **The forward fact is dropped at the wire.** `Incoming` (`client.rs:125-144`) decodes
  `message_id`, `date`, `chat`, `from`, `sender_chat`, `text`, `caption`, `reply_to_message`
  and `pinned_message`. There is no `forward_origin` field and no `quote` field; unknown keys
  are ignored by the decoder, so the fact arrives and is discarded silently.
- **A member's forward is recorded as an ordinary message today.** `translate` skips on
  `sender_chat` (`translate.rs:159-160`), then requires `from` (`:162`) and text or caption
  (`:165-167`, `text_of` at `:466-473`), and builds `Pending` with the forwarded text as
  `text` (`:179-193`). A member forwarding a channel post has `from` set to themselves and no
  `sender_chat`, so the record is made, attributed to them, unmarked.
- **The projection puts that text under the member's handle.** `projected_text`
  (`kind.rs:555-570`) renders `[origin] speaker: text` for a user-voiced row with an origin and
  a storable speaker, and the erased row renders `ERASED_MARKER` (`kind.rs:166`) before any
  prefix is considered. `projected_origin_mark` (`kind.rs:181-184`) is the existing precedent
  for a fixed marker in the projected line, and it records the forgery residual that applies to
  any such marker: a member can type the same bytes into their own message.
- **The message row already carries facts of exactly this shape.** `COLUMN_SPEAKER`
  (`kind.rs:114`) is a nullable projected-prose column with a storable-shape bound,
  `storable_speaker` (`kind.rs:128-130`), refusing an empty handle, one containing `:` and one
  containing whitespace, precisely so the projected prefix cannot be blurred.
- **`stored_fields` is the single write encoding** (`kind.rs:444-485`), called once from
  ingestion (`assembly.rs:725-739`), with `LeafKind::parse` reading the same columns back
  (`kind.rs:599-639`) and `DESCRIPTORS` declaring them (`kind.rs:575-597`). It already takes six
  parameters, and the editing unit's `revises` adds a seventh.
- **Erasure has two content passes and both are one UPDATE.** `erase_principal_content`
  (`kind.rs:688-...`) nulls `text`, `origin`, `sent_at`, `reply_target` and `speaker` for every
  message of one principal; `erase_message_named` (`kind.rs:743-...`) nulls the same columns for
  one named message and is what the deletion mirror calls. Any new content column is one more
  name in each list.
- **Schema growth is an appended step with a frozen vocabulary.** `store_config`
  (`schema.rs:373-395`) lists fourteen steps in order; each new closed vocabulary is frozen as
  a constant at the moment its step ships (`schema.rs:120-144`), and a test pins the newest
  frozen list to its live enum (`schema.rs:398-436`) so growing the enum fails loudly.
- **Teaching composes from shared sections.** `composed_system_prompt` (`teaching.rs:69`)
  assembles the embedder's prompt, the identity section, the answering section for the mode,
  and — only when `moderation_taught` holds (`teaching.rs:33`) — the moderation teaching.
  `sourcing_rules` (`teaching.rs:143`) and `audience_rules` (`teaching.rs:170`) are the
  precedent for a rule shared by both modes.
- **The outbound edge sends one thing.** `BotClient` exposes `get_me`, `get_updates`,
  `get_chat`, `leave_chat`, `send_message`, `send_chat_action` and `chat_administrators`
  (`client.rs:304-470`), and `send_body` (`client.rs:439-461`) builds the only `sendMessage`
  call, with `reply_parameters` and `allow_sending_without_reply` where a target is given.
  There is no forwarding or copying call anywhere in the adapter today.
- **The report path already moves nothing.** A filed report is a fixed line delivered as a
  threaded reply in the same group (`outbound.rs:489-495`), and the record of processing states
  that the report event "carries the reported message's identifier — a message the
  administrators already see in their own group — and no data from the assistant's store"
  (`records-of-processing.md:86`). Nothing in the moderation path needs a message moved.
- **The published documents say things this unit must keep true.** The record of processing
  states "Anonymous administrator posts and automatic channel forwards are not stored at all"
  (`records-of-processing.md:75`) and the impact assessment says the same
  (`dpia.md:172-175`) — both correct, both about the automatic case. The
  legitimate-interest assessment drops the qualifier: "Anonymous administrator posts and
  channel forwards are skipped" (`lia.md:107`), which reads as a claim about every forward and
  is **already false** for a member's own forward. The impact assessment's mandatory review
  triggers include "A change to what is collected" and "any change to which identifiers travel
  with a request" (`dpia.md:563-566`). The member-facing notice is given "under Articles 13 and
  14 GDPR" (`bot-assistant-privacy-policy.md:3-4`) and already offers a route for content about
  a non-member: "We delete a stored message that concerns you on request, whether you wrote it
  or not, your own through the route below and somebody else's after a person reviews it"
  (`:27-29`). Document text is pinned by `crates/assistant/tests/docs.rs`.
- **Bytes.** Nothing in this unit moves a byte. Forwarding and copying are server-side content
  moves keyed by identifier, and the unit calls neither; the relay fact is one short string and
  one flag on a row that is already being written. A forwarded photo is fetched by the media
  unit's streaming path exactly like any other photo, unchanged by this unit, and its sniff
  window stays the only buffer held. The streaming constraint is satisfied because no stream
  exists here.

## Decisions taken with this unit

- **The core's word is `relayed`, not "forwarded", 2026-08-25.** `InboundMessage` gains
  `relay: Option<RelayOrigin>` — the fact that the message repeats words first said elsewhere.
  "Forward" is the platform's own verb for its own mechanism, and the second platform on the
  roadmap has no equivalent primitive at all; a core field called `forwarded` would be reasoning
  about Telegram's model even though the vocabulary check (`docs/platform-vocabulary.txt`) would
  not catch the word. `relay` states the neutral fact any platform can report: these words were
  brought here, they were not written here. *Rejected:* `forwarded` (the platform's verb, and it
  invites the next adapter to answer "what is our forward?" instead of "did these words come
  from elsewhere?"); `quoted` (a quote is a different mechanism, present on this platform under
  its own field, and this unit deliberately does not record it — see below).

- **Two facts are recorded, and the original author is not one of them, 2026-08-25.**
  `RelayOrigin` is `Person` or `Publication { handle: Option<String> }`, stored as
  `relay_origin` (TEXT, CHECK in the frozen list `'person','publication'`) and `relay_handle`
  (TEXT, nullable). Nothing else. No account identifier, no username, no display name, no author
  signature, for any of the four origin shapes the platform reports. The reason is that the
  original author is not in this group, has had no notice, has no way to reach the assistant,
  and — with no identifier stored — has no rows for erasure to find; storing their name would
  create a category of person whose rights this system cannot serve, in exchange for prose the
  model does not need. The forwarded text itself continues to be stored exactly as it is stored
  today, as the message the member sent. *Rejected:* recording `sender_user.id` or the
  username from `MessageOriginUser` (it creates that unreachable category, it is the one field
  Article 14's notice duty would attach to hardest, and decision 0077 already removed a stored
  name that nothing consumed); recording `sender_user_name` from the hidden-user shape (the
  author set forward privacy precisely to prevent that link, and honouring the platform's flag
  by branching on it is weaker than never storing a name at all); recording `author_signature`
  (a signature is a person's chosen name in another room).

- **The known author and the hidden author are one case, 2026-08-25.** `MessageOriginUser` and
  `MessageOriginHiddenUser` both translate to `RelayOrigin::Person`. The distinction exists on
  the platform to say whether a name may be shown; once no name is stored either way, keeping
  the split would import the platform's privacy flag into our store for no reader. The
  consequence is a structural one and is the point: there is no `if hidden` branch anywhere in
  this unit, so the concealed case cannot be handled wrongly by a later change. *Rejected:* a
  four-value vocabulary mirroring `MessageOrigin` (the platform's shape in the core's table,
  three of whose four values would render identically).

- **A publication's public handle is stored; a private place's name is not, 2026-08-25.** The
  adapter fills `handle` from the origin chat's public handle, in the form the platform's own
  readers use, and from nothing else — never from a title. A chat with a public handle is
  addressable by anyone; its name in our store says nothing about the member who relayed the
  message. A private group's title would: it would tell us which private rooms a member sits in.
  The core's rule is neutral and the adapter's job is pure translation of "which field is the
  public handle" — an adapter that reports a title where a handle was asked for is a broken
  adapter, not a policy difference. A publication with no public handle stores `person`-like
  emptiness: kind `publication`, handle NULL. *Rejected:* storing the origin chat's title (it
  is a fact about the member's other memberships, obtained without their doing anything);
  storing the origin's `message_id` from `MessageOriginChannel` (a pointer into a room the
  assistant has no business following, and nothing reads it).

- **The original send time is not recorded, 2026-08-25.** Every `MessageOrigin` variant carries
  `date`. It is not stored. For the `person` case it is a fact about a third party's message;
  for the `publication` case the argument for it — "the model should know a claim is two years
  old" — is real but thin, since a forwarded post usually carries its own dating and the
  platform shows the original date in the chat to every human reader. One less column, one less
  sentence in the record of processing. *Rejected:* storing it for publications only (a column
  whose meaning depends on a sibling column's value, for a reader nobody has asked for);
  storing it for both (a third party's timestamp).

- **Quoted fragments are still not recorded, 2026-08-25.** `Message.quote` and
  `Message.external_reply` stay undecoded. A quote is a verbatim fragment of one person's
  message carried inside a different person's message, and our erasure is keyed by author: a
  fragment stored on the quoting member's row would be unreachable when the quoted person is
  erased, because the row belongs to somebody else. That is the same residual decision 0063
  already records for reply targets, whose stated follow-up is "a reach key resolved when the
  reply is recorded" (`kind.rs:130-151`); adding a second, wider instance of a known defect to
  ship a nicety is not a trade this unit takes. The cost is stated plainly: when a member
  replies to a message from outside this group, the assistant sees `external_reply` with no
  text, so it will never know what was replied to. *Rejected:* storing the quote text on the
  replying member's row (unreachable by erasure, and it duplicates another person's words into
  a row keyed to the wrong author); storing it only when the quoted message is one the store
  already holds (the useful case is exactly the other one, and the same erasure key problem
  returns the moment the quoted row is erased first).

- **The model is told, in the projected line, 2026-08-25.** `projected_text` gains a relay mark
  between the speaker prefix and the text, so a relayed message reads
  `[4711] alice: (relayed from @halogenos_news) the update bricks devices`, and a person-origin
  message reads `(relayed from a person elsewhere)`, and a handleless publication reads
  `(relayed from a publication)`. The mark rides only a row that still has text: the erased
  branch returns before it, exactly as it does for the speaker and the origin mark. The handle
  is projected as the adapter reported it, with no re-formatting in the core. The handle is
  admitted by the same shape bound the speaker uses: `storable_speaker`'s three checks are
  re-exposed as `storable_handle` and called from both places, so the rule that keeps projected
  prose parseable exists once. The forgery residual `projected_origin_mark` records applies
  unchanged and is not re-litigated here. *Rejected:* a separate context note or a system-prompt
  line listing relayed messages (the fact belongs to the message, and the projection is where a
  per-message fact already lives); putting the mark before the speaker (it would read as the
  speaker being relayed, not the words); a machine-readable tag such as `relay=publication`
  (the projection is prose the model reads, `kind.rs:555-570`, and every other fact on the line
  is prose).

- **Erasure nulls both relay columns with the text, 2026-08-25.** `relay_origin` and
  `relay_handle` join the column lists of `erase_principal_content` and `erase_message_named`.
  They describe what the message was, not how the machine handled it, and "this erased person
  once relayed something from @somewhere" is a residue about the erased person. Both passes stay
  one UPDATE and stay idempotent, since nulling a null column is a no-op. *Rejected:* treating
  the relay fact as structure and leaving it (the precedent for "structure survives" is the
  addressing and budget stamps, which are the machine's own decisions, not the message's
  content).

- **Nothing else about the message changes, 2026-08-25.** A relayed message is addressed or not
  by the same rules, opens the same debt, spends the same budgets, threads the same way, and is
  assessed by the same moderation path. The relay fact is context for the reader, not an input
  to any decision the machinery takes. *Rejected:* suppressing summons for relayed messages, or
  exempting them from budgets (either one is the machine deciding that some members' messages
  matter less, from a fact the member did not choose to disclose).

- **The teaching says what the mark means, and what it does not license, 2026-08-25.** A shared
  `relay_rules()` section composes into both answering modes, beside `sourcing_rules` and
  `audience_rules`. Its content, verbatim: a message may carry words first written somewhere
  else; the mark after the handle says so; the member sent it and the words are not theirs; the
  claims in it are claims from outside this group and the lookup discipline applies before any
  of them is repeated as true; where the mark names a place, that is the public name of where
  the words were published; where it does not, who wrote them is unknown and must not be
  invented. One further sentence composes only inside the moderation teaching, which already
  runs conditionally (`teaching.rs:33`): an assessment of a relayed message concerns the act of
  bringing it into this group, and the person who wrote it is not in this group and is not the
  assistant's to judge. *Rejected:* leaving the mark unexplained (an unexplained parenthetical
  in front of the text is exactly the kind of string a model narrates back at people).

- **The assistant never copies a person's words, 2026-08-25.** `copyMessage` and `copyMessages`
  are not called, and no code path exists that could. A copy arrives with no link to the
  original: in the destination it is the assistant's own message, so a member's words would be
  read as the assistant's, and the person's words would appear somewhere with their name
  removed. Both directions are misattribution, and the second is the thing a person is entitled
  to object to most. The mechanism reinforces it: `copyMessage` returns only a `MessageId`, so
  the assistant could not even record what it had published. *Rejected:* copying with an
  attribution line the assistant writes itself (the assistant then becomes the publisher of
  someone's words under a name it typed, and a reader cannot check the attribution the way the
  platform's own forward chrome can be checked); copying the assistant's OWN messages to
  republish them (nothing needs it — the assistant holds its own text and the existing send path
  formats and chunks it, `client.rs:371-461` — and a copy would return no text to record).

- **The assistant forwards nothing, and no forwarding tool is registered, 2026-08-25.**
  `forwardMessage` and `forwardMessages` are not called either. Forwarding a member's message
  out of the group it was said in is a new recipient in the record of processing, an outbound
  copy the assistant cannot withdraw once the destination holds it, and — with no human naming
  the concrete message and the concrete destination — the assistant moving a person's words on
  its own assessment, which decision 0070 refuses structurally, not as advice. The existing
  moderation path needs none of it: the report is a fixed line, threaded onto the message, in
  the group the administrators already read (`outbound.rs:489-495`,
  `records-of-processing.md:86`). *Rejected:* forwarding a reported message to a moderation
  chat (it is the strongest candidate and it still loses today: the destination must be a second
  admitted group under decision 0052, it adds recipient R6 with its own assessment, and the
  administrators can already see the message in their own group, so the flow spends a new data
  transfer to save a tap); forwarding on an administrator's explicit command (this one satisfies
  0070's human decision point and is the shape a later unit should build if a real need appears
  — the seam is `mirror::mirrored_target`'s command reading, `mirror.rs:58-70`, which already
  turns an administrator's reply into a named target; it is not built now because no one has
  asked for it and an unused standing capability over members' words is the exact posture unit
  T4 refused for deletion); a model-callable "share this" tool (the model deciding to move a
  person's words is the prohibition itself).

- **The assistant does not protect its own messages from being forwarded, 2026-08-25.**
  `protect_content` stays unset on `sendMessage`. The assistant's answers are community help,
  and a member who was helped should be able to carry the answer into another room or save it;
  protecting it would also stop the person who asked from keeping it. The flag protects nobody's
  personal data here, because the assistant's own prose is what would be protected. *Rejected:*
  setting it on answers (it withholds usefulness from members to prevent nothing).

- **A group that turns on content protection is the operator's call, and the reference says so,
  2026-08-25.** `Chat.has_protected_content` means "messages from the chat can't be forwarded to
  other chats". Sending message text to the model processor is not a forward, and the assistant
  will not silently start behaving differently when the flag appears; the group operator
  reference gains one sentence stating plainly that content protection does not stop the
  assistant from reading messages and sending their text to the processor, so a group turning it
  on decides with that in view. No code reads either protection flag. *Rejected:* refusing to
  answer in a protected chat (the assistant would go silent from a setting that means something
  else, and no published statement promised that behaviour); recording the per-message flag (a
  column nothing reads, on a fact the documentation does not tie to the chat setting).

- **The write encoding takes one more parameter now, and the grouping happens on the second
  merge, 2026-08-25.** `stored_fields` (`kind.rs:444-485`) gains `relay: Option<&RelayOrigin>`.
  It is already at six parameters and the editing unit adds `revises` to the same signature, so
  the honest note is this: whichever of the two units merges second performs the grouping,
  moving `origin`, `reply_target`, `revises` and `relay` into one `RecordedReferences` value
  beside the existing `RecordedSender` and `Stamp` groupings, and that refactor is part of that
  unit's work, not a follow-up nobody owns. If this unit merges second, it does the
  grouping. *Rejected:* doing the grouping unconditionally now (it would collide with the
  editing unit's change to the same function and make one of the two merges a manual
  reconciliation); leaving eight positional parameters standing (a call site where two adjacent
  `Option<&str>` arguments can be swapped without a type error).

## The unit's contract

A message a member forwarded into the group is recorded as that member's message, with two new
facts beside it: that the words were first said elsewhere, and — when the platform publishes a
public handle for the place they were published — that handle. No identifier, username, display
name, signature or timestamp of the original author is decoded, stored or transmitted, for any
of the four origin shapes the platform reports, and the concealed-author case needs no branch
because the known-author case stores nothing either. The model reads the fact as a short mark in
the projected line, between the speaker's handle and the text, and the teaching tells it that
the words are not the member's, that claims inside them are claims from outside the group, and
that the person who wrote them is not the assistant's to judge. Addressing, debts, budgets,
threading and the moderation assessment behave exactly as before; the mark makes an existing
assessment honest and grants no new power. Erasure nulls both new columns with the text, on the
author-keyed pass and on the deletion mirror's message-keyed pass alike. Automatic channel
forwards stay skipped, quoted fragments and external replies stay undecoded, and the assistant
calls no forwarding or copying method at all: it moves no person's words to a place they did not
put them, and it never publishes anyone's words with the attribution removed. Nothing streams,
because nothing here moves bytes. The record of processing, the impact assessment, the
legitimate-interest assessment, the member-facing notice and the group operator reference all
change in the same commit, and the legitimate-interest assessment's sentence about forwards
stops being false.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary check and the secret scan clean; no new dependency in any manifest, pinned
  by the manifest-reading test the media unit introduced.
- **AC2** A member's forward records the relay fact, one pinned adapter case per platform origin
  shape: `MessageOriginUser` and `MessageOriginHiddenUser` both produce `RelayOrigin::Person`;
  `MessageOriginChannel` and `MessageOriginChat` produce `RelayOrigin::Publication`, carrying the
  origin chat's public handle when the payload has one and `None` when it does not. A message
  with no `forward_origin` produces `None` and a row whose two relay columns are NULL.
- **AC3** No third-party identity is stored or transmitted. A test decodes an update whose
  `forward_origin` carries `sender_user.id`, `sender_user.username`, `sender_user_name`,
  `author_signature`, `date` and the origin chat's `title`, and asserts that none of those byte
  sequences appears in the recorded row, in the projected text, or in the request the provider
  receives.
- **AC4** The projection reads `[origin] speaker: (relayed …) text` with the three documented
  wordings, and an erased relayed row projects the erased marker alone, with no relay mark —
  both pinned.
- **AC5** A handle whose shape would blur the projected line is refused by the shared
  `storable_handle` bound and stores NULL, and the speaker column's behaviour under the same
  predicate is unchanged — pinned, including that the predicate exists once.
- **AC6** Erasure nulls `relay_origin` and `relay_handle`: pinned for the author-keyed pass over
  a principal's relayed messages, for the deletion mirror's single named message, and for a
  second run of each proving idempotence.
- **AC7** Both answering modes' composed prompt carries the relay rules verbatim, and the
  moderation sentence appears exactly when `moderation_taught` holds and never otherwise —
  pinned against the composition, as the existing prompt pins are.
- **AC8** No forwarding or copying call exists. A source-scanning test over the adapter crate —
  the platform-vocabulary check is the precedent — asserts that none of `forwardMessage`,
  `forwardMessages`, `copyMessage` or `copyMessages` appears as a request method name, and the
  adapter's fake-server suite asserts that a full ingestion-and-answer run issues no request
  outside the known method set.
- **AC9** Behaviour around a relayed message is unchanged: a relayed message that is also a
  reply keeps its reply target; a relayed message that addresses the assistant opens the same
  debt and spends the same budgets as an identical non-relayed one; an automatic channel forward
  is still skipped under decision 0016 — all pinned.
- **AC10** The appended migration step is the last entry in `store_config`'s list, its frozen
  vocabulary constant is pinned to the live `RelayOrigin` enum by a test in the existing style,
  a store created before the step reads NULL in both columns and projects bare, and a fresh
  store and an upgraded store end at one schema.
- **AC11** The documents change in the same commit and are pinned in `crates/assistant/tests/docs.rs`:
  the record of processing gains the relay category and names it in what the processor receives,
  and states that no data-subject entry is created for the original author because no identifier
  for them is stored; the impact assessment records that the "change to what is collected" and
  "which identifiers travel with a request" review triggers fired, and carries the residual that
  content by a person outside the group enters the store and the processor with no notice
  reachable to them, together with its three mitigations; the legitimate-interest assessment's
  sentence about forwards gains the word "automatic" and states that a member's own forward is
  stored and marked; the member-facing notice gains one plain sentence saying what is stored and
  that who first wrote it is not; the group operator reference gains the content-protection
  sentence.

## Notes for launch

- Branches from `main`; the worktree merges back and is deleted, as every unit does. The
  decisions above take their numbers from `docs/decisions` at merge, in the order the merges
  happen.
- Adapter sites: `client.rs:125-144` gains one optional `forward_origin` field decoded as a
  tagged shape on `type`, with unknown types decoding to nothing instead of refusing the batch,
  in the same lenient style as `PinnedContent` (`client.rs:146-157`); `translate.rs:179-193`
  fills the new `Pending` field; `translate.rs`'s test fixtures at `:523`, `:562` and `:885`
  gain the field.
- Core sites: `message.rs` gains `RelayOrigin` and the `InboundMessage` field beside
  `reply_target` (`message.rs:196`); `kind.rs` gains the two column constants beside
  `COLUMN_SPEAKER` (`:114`), the `storable_handle` rename at `:128-130`, the fields in
  `stored_fields` (`:444-485`), `parse` (`:599-639`) and `DESCRIPTORS` (`:575-597`), the mark in
  `projected_text` (`:555-570`), and the two column names in both erasure passes (`:688`,
  `:743`); `schema.rs` gains the appended step and its frozen list (`:120-144`, `:389-411`);
  `assembly.rs:725-739` passes the field through; `teaching.rs` gains `relay_rules()` beside
  `sourcing_rules` (`:143`) and the one moderation sentence in the conditional teaching (`:37`).
- Two coordination points with units being written in parallel, neither of which is edited here.
  The editing unit adds `revises` to `stored_fields` and adds its own appended migration step;
  the second of the two merges appends its step after the first and performs the parameter
  grouping named in the decision above. The media unit records a file's facts on the same row,
  and a relayed file message simply carries both marks; that unit's residual about third-party
  content now includes files, which its own document changes should state when both have merged.
- The unit's most valuable finding for whoever reads it next: the platform will hand over the
  identity of a person who never spoke to this assistant, and the only reason that is not a
  problem here is that the code never asks for it. If a later change starts decoding
  `sender_user`, every consequence in the decisions above returns at once — the unreachable
  data-subject category, the Article 14 notice with no addressee, and an erasure key that cannot
  be built.
