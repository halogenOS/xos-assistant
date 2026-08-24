# Telegram unit 03 — editing messages: the member's edit, and the assistant's own

Date: 2026-08-25, revised the same day after two independent reviews. Two different things
share one name, and this unit separates them. A MEMBER edits a message: the platform
delivers a new version of a message the assistant may already have recorded and may already
have answered, and the ledger cannot rewrite what it stored. The ASSISTANT edits its own
message: a capability the platform grants without the time limit everyone assumes, and which
this unit does not build, for reasons that have nothing to do with the platform. The first
half ships; the second half is measured against the tree and left to the unit that already
owns it.

The feature is buildable. The first draft was not: two of its rules were wrong in ways that
would have shipped defects, and both are corrected below rather than papered over.

- **An edit could re-record erased content.** Erasure nulls a row's text AND its origin
  (`kind.rs:688-705`), so nothing joins to an erased message any more. The first draft said
  an edit naming a message the store cannot find records "as an ordinary new statement". The
  platform fires an edit update for changes the bot never asked about — a link preview
  attaching, hours later — so that rule would have written a person's erased words back into
  the ledger with no human act anywhere in the path. The rule is inverted below: an edit
  naming a message the store cannot find records nothing.
- **A deletion command arriving as an edit would have stopped being silent.** The stamp's
  command marking is currently derived from whether the deletion mirror ACTED
  (`assembly.rs:692`). The first draft made the mirror ignore revisions and said nothing
  about the stamp, which would have turned an administrator's corrected `/del` into an
  ordinary summoned message under helpful answering — a debt, a budget slot and a model turn
  on a command aimed at another bot. Recognition and action are separated below.

Everything below was verified on 2026-08-25 against the live Bot API documentation and
against both repositories. Do not re-research either; verify against the tree and the build.

## Grounding

**The platform.** Bot API 10.3 (24 August 2026) is current, not 10.1 — the changelog lists
10.1 (11 June 2026), 10.2 (14 July 2026) and 10.3 (24 August 2026).

- `Update.edited_message` is documented as "New version of a message that is known to the
  bot and was edited. This update may at times be triggered by changes to message fields
  that are either unavailable or not actively used by your bot." Two facts sit in that one
  sentence: the bot never receives an edit of a message it never received, and an
  `edited_message` update is not proof that a person changed any text.
- `Message.edit_date` — "Optional. Date the message was last edited in Unix time" — is a
  field of the same `Message` object, carried under the same `message_id`. An edit is the
  same message in a new version, which is also why `editMessageText` and `deleteMessage`
  address a message by `message_id` alone.
- **`message_id` is unique per chat with one documented exception, and the exception must be
  quoted whole:** "Unique message identifier inside this chat; 0 for ephemeral messages. In
  specific instances (e.g., a message containing a video sent to a big chat), the server
  might automatically schedule a message instead of sending it immediately. In such cases,
  this field will be 0 and the relevant message will be unusable until it is actually sent."
  The adapter stores the id as the opaque origin without inspecting it
  (`translate.rs:192`), so a documented `0` makes distinct messages share one key. That
  exposure predates this unit — the origin, the reply reference and the deletion mirror all
  key on it already — but this unit adds two more readers of the same key and therefore
  names it. See the decision on the zero id below.
- **There is no 48-hour limit on a bot editing its own ordinary message.** `editMessageText`
  says: "Use this method to edit text, rich and game messages. On success, if the edited
  message is not an inline message, the edited Message is returned, otherwise True is
  returned. Note that business messages that were not sent by the bot and do not contain an
  inline keyboard can only be edited within 48 hours from the time they were sent." The
  48-hour sentence binds business messages the bot did not send. The premise that a bot may
  only edit its own message for 48 hours is wrong.
- `editMessageText` parameters: `business_connection_id`, `chat_id` and `message_id` (or
  `inline_message_id`), `text` — "New text of the message, 1-4096 characters after entity
  parsing; required if rich_message isn't specified" — `parse_mode`, `entities`,
  `link_preview_options`, `rich_message`, `reply_markup`.
- **The 48-hour limit is real for deletion, and the deletion permissions go further than a
  bot's own messages.** `deleteMessage`'s limitation list, in full: "A message can only be
  deleted if it was sent less than 48 hours ago"; "Service messages about a supergroup,
  channel, or forum topic creation can't be deleted"; "A dice message in a private chat can
  only be deleted if it was sent more than 24 hours ago"; "Bots can delete outgoing messages
  in private chats, groups, and supergroups"; "Bots can delete incoming messages in private
  chats"; "Bots granted can_post_messages permissions can delete outgoing messages in
  channels"; "If the bot is an administrator of a group, it can delete any message there";
  "If the bot has can_delete_messages administrator right in a supergroup or a channel, it
  can delete any message there"; "If the bot has can_manage_direct_messages administrator
  right in a channel, it can delete any message in the corresponding direct messages chat."
  `deleteMessages` takes "a JSON-serialized list of 1-100 identifiers" and refers back to
  those limitations. The last three bullets matter to this project specifically: helpful-mode
  deployments run the bot as a group administrator (`dpia.md:169-176`), so the platform
  hands this deployment the power to delete any member's message. Decision 0070 refuses it —
  see the decision on member messages below.
- The platform's own answer to progressive output is `sendMessageDraft` (added in **Bot API
  9.3, 31 December 2025**: "Added the method sendMessageDraft, allowing partial messages to
  be streamed to a user while being generated") and `sendRichMessageDraft` (**10.1, 11 June
  2026**). Both take "Unique identifier for the target **private chat**" and both are "a
  temporary 30-second preview - once the output is finalized, you must call sendMessage with
  the complete message to persist it". In a group they do not exist.
- Rate limits, from the bot FAQ: "In a single chat, avoid sending more than one message per
  second"; "In a group, bots are not able to send more than 20 messages per minute".
- Privacy mode (bots/features): a bot in a group without administrator rights sees only
  commands aimed at it, general commands when it spoke last, and replies to its own
  messages. Combined with "known to the bot", this means a member cannot summon the
  assistant by editing an ordinary message to add a mention — that update never arrives.
  Helpful-mode deployments run with privacy mode off or the bot as an administrator
  (`docs/privacy/dpia.md:169-176`), and there every edit of every text message arrives.
- **Not documented anywhere, and therefore not a receipt:** the widely repeated claim that
  the API refuses an edit whose content is unchanged. It does not appear in `bots/api`,
  `bots/api-changelog` or `bots/faq` — the string "not modified" is absent from all three,
  checked on 2026-08-25. Whatever the API does in practice, no future unit may build on it
  from this document.

**Our tree.**

- The edit updates are already fetched and thrown away. `CONSUMED_UPDATE_TYPES` names
  `edited_message` on every poll (`client.rs:103`), `Update.edited_message` is already
  decoded into the same `Incoming` shape as a new message (`client.rs:118`), and translation
  discards it at `translate.rs:123-125` as `Skip::EditedMessage` (`translate.rs:44-45`),
  citing decision 0017.
- Decision 0017 deferred exactly this unit: "an edit kind — appending the revision as its
  own block — is a later unit's decision, taken when the acting policy exists to read it."
  The acting policy exists. Its rejected alternative was recording edits as fresh messages
  with no marking, so a marked revision does not reopen it.
- Everything the revision needs already exists on the inbound path: text and caption
  (`translate.rs:465-472`), addressing (`:171-178`), the invoked command (`:293-307`), the
  reply target (`:454-462`), the sender identity (`:186-189`). A revision differs from a
  fresh message in exactly one recorded fact.
- `text_of` returns `None` for an empty text and an empty caption alike
  (`translate.rs:465-472`), and translation then reports `Skip::NoText`
  (`translate.rs:166`). A member who deletes a caption produces precisely that shape.
- The message row's stored facts are `kind.rs:32-158`; the append's field map is
  `ChatMessage::stored_fields` (`kind.rs:444-485`) and its inverse is `parse`
  (`kind.rs:599-637`), both in one module so a column cannot split them. The descriptor's
  column list is `kind.rs:575-597`: role, text, principal id, authority, speaker, origin,
  send time, addressed, literal addressed, answer due, limited, debt authority, reply target,
  reply-to-assistant. **There is no column for the invoked command** — a command's only
  stored trace is the limited fact reading `command` (`kind.rs:185-205`).
- The projection is a per-block reading with no ledger access (`kind.rs:546-549`,
  `projected_text` at `kind.rs:555-569`): it cannot fold history, so no projection change can
  hide an earlier version. The id mark is `projected_origin_mark` — `[{origin}]`
  (`kind.rs:181-183`) — and its own documentation already names the forgery hazard a fixed
  prose mark carries, together with the bound that contains it (`kind.rs:168-182`).
- `projected_text` reads an absent text as erased and says why that reading is exact: "Only
  erasure produces the absent text — the adapter never records an empty message and the
  schema stores the column NOT NULL — so the marker speaks exactly for erased messages"
  (`kind.rs:539-542`).
- **Erasure nulls the origin along with the text**, five personal columns per row,
  author-keyed (`kind.rs:688-705`), plus the target-keyed reply pass (`kind.rs:799-823`). An
  erased row therefore matches no origin lookup at all — the property the mirror's own
  idempotence already relies on (`kind.rs:738-742`).
- The deletion mirror's named erasure matches by origin across the conversation and returns a
  row count (`kind.rs:743-784`); its `WHERE {origin} = ?1` matches every row carrying that
  origin. Its documentation calls the first pass "the ONE target row" (`kind.rs:719-722`),
  which stops being true once two rows share an origin.
- The deletion mirror's trigger is a pure predicate on the message (`mirror.rs:58-70`),
  called at `assembly.rs:687-691`. Its premise, stated in its own module doc (`mirror.rs:1-24`)
  and in decision 0082: both bots receive the same command independently, the moderation bot
  deletes and the assistant erases its copy.
- **The command stamp is derived from the mirror's action, not from the message.**
  `assembly.rs:692-699` reads `let limited = if family.is_some() || mirrored.is_some()`, and
  `/del` is not a privacy family member — the family is the five spellings at
  `privacy.rs:45-60`. Under helpful answering `resolved_summons` returns summoned for every
  group message (`assembly.rs:1246`), and `Stamp::compose` then takes a debt whenever a
  summoned message is unlimited (`kind.rs:328-352`).
- Ingestion's write path, in order: the erasure fence read hold, the channel admission, the
  sender resolution, the conversation mapping, the stamp lock, the suppression re-read (which
  returns `Disregarded` and exempts the command family), the palette reconciliation (which
  APPENDS a delta block on the conversation's first activity per process,
  `assembly.rs:668-672`), the deletion mirror, the tail read, the stamp, the append —
  `assembly.rs:623-781`. `IngestOutcome::Disregarded` documents itself as "Nothing touched
  the ledger or the identity tables" (`message.rs:299-309`), the adapter acknowledges it and
  advances the offset (`driver.rs:451-457`), and its meaning was widened once before.
- A store failure inside ingestion propagates as `CoreError::Store` and the driver's batch
  discipline retries a transient refusal (`driver.rs:459-470`). Fail-closed on a store read is
  the standing pattern (decisions 0041, 0052).
- The report tool resolves a named id against the turn's assessment set by
  `find(|message| message.origin.as_deref() == Some(origin))` — first match in turn order —
  and refuses a second report of the same origin (`tools/report.rs:293-320`, decision 0092).
  The only facts it reads from the resolved row are the role and the principal id.
- The outbound side cannot address its own messages. `send_body` discards the returned
  `Message` (`client.rs:459`), one answer becomes several platform messages
  (`client.rs:377-391`, decision 0019), and `OutboundReply` carries no identity of its own
  (`message.rs:372-390`). `ReplyTarget::AssistantMessage` exists precisely because the
  assistant's messages have no stored origin (`message.rs:164-166`).
- **Telegram unit 04, dated the same day in this directory, already specifies the delivery
  record and the assistant's own deletions**: a `Delivered` block holding every chunk's
  origin in send order, the adapter issuing `deleteMessage` and `deleteMessages`, and
  `ReplyTarget::AssistantMessage` gaining an optional origin. This unit does not touch that
  file and does not restate its design; it only stops claiming the ground unit 04 stands on.
- Decision 0079's precedent: the disclosure line is written into the stored answer block
  BEFORE the send (`disclosure.rs:109-131`, `store_line` at `:235-246`), so "the returned
  text is the stored text either way; delivery and ledger cannot disagree."
- The framework's fork primitive is a conversation-level operation: `Continuation::Edit` cuts
  the group off and inserts the replacement into a NEW conversation
  (`agent-ledger/crates/agent-ledger/src/store/conversations.rs:80-101`, `:410-428`). Our
  channel mapping is one conversation per channel with `conversation_id` UNIQUE
  (`schema.rs:94-105`).
- Decision 0030 forbids a protection mechanism dropping a message from the ledger, and
  decision 0003 records that message history is kept with no retention timer. Together they
  fix what may and may not bound the volume this unit adds — see the decision on the
  identical-text drop.
- The published privacy documents state the opposite of this feature today:
  `bot-assistant-privacy-policy.md:22` ("We do not store the media itself, edits, or …"),
  `records-of-processing.md:61` and `:144`, `dpia.md:130` and `:336`, `lia.md:107`. The DPIA
  lists "A change to what is collected: media, edits, reactions, membership events" as a
  mandatory review trigger (`dpia.md:566`), and `crates/assistant/tests/docs.rs` pins the
  documents as committed. The operator reference states the mirror's bounds as a complete
  list — "only deletions issued as a reply `/del` reach it, and only the bare token"
  (`docs/reference/group-operator-contract.md:150-158`) — and that section is pinned by
  `crates/assistant/tests/docs.rs:749-757`.

**One assumption, stated because three conclusions rest on it.** On this platform only a
message's own author can edit it, so the revision's sender is the original's sender. The Bot
API does not state this anywhere; it holds here because the two shapes that would break it
never reach the core — a channel forward and an anonymous administrator both arrive with
`sender_chat` set and are skipped at `translate.rs:158-160` under decision 0016. The
conclusions that depend on it are the report's principal being the same for both versions,
the author-keyed erasure pass reaching a revision reference without a second target-keyed
pass, and an edited privacy command being that person's own request about their own data. A
platform where a third party may edit another person's message breaks all three and needs
its own decision.

## Decisions taken with this unit

- **A member's edit is recorded as a new message that names the message it revises,
  2026-08-25.** The deferral in decision 0017 falls due. The adapter stops skipping the edit
  update and translates it exactly like a fresh message, with one extra fact: the origin of
  the message being revised. The core appends an ordinary message block carrying that fact in
  a new nullable column. Nothing is rewritten, the earlier version keeps its row and its
  place, and the ledger reads as what it is: a person said one thing and then said it
  differently. *Rejected:* keeping the skip (the assistant answers a question its author has
  already withdrawn, and the stored record of what someone said is knowingly stale, which the
  accuracy principle in Article 5(1)(d) argues against); updating the stored row in place (the
  append-only rule, and the earlier version was already read, already answered and possibly
  already reported); the framework's `Continuation::Edit` fork (it creates a second
  conversation, and the channel mapping holds one conversation per channel with a UNIQUE
  constraint — the fork exists for a composer re-running one user turn, not for a room where
  twenty people are talking).
- **The neutral word is `revises`, and it names the ORIGINAL message, 2026-08-25.**
  `InboundMessage` gains `revises: Option<String>` — the opaque origin of the message this
  one supersedes — beside the existing `origin`, which stays this version's own identifier.
  On this platform the two are equal, because an edit is the same message under the same
  `message_id`. The value is defined as the origin of the message as first known, not of the
  version immediately superseded: a third edit of the same message names the same identifier
  as the first, so every version of one message shares one key, and a single match reaches
  them all. Where a future platform delivers an edit as its own event pointing only at the
  preceding version, that platform's adapter has a fact this one does not, and its own unit
  decides how to report it; nothing in the core changes. The core never learns which platform
  it is talking to. *Rejected:* a boolean "this is an edit" (it cannot say WHAT was edited, so
  the second platform needs a second field and the core grows a platform branch); deriving the
  relation in the core from two rows sharing an origin (true here, false elsewhere, and the
  core would be reasoning about a platform's id scheme); letting `revises` name the
  immediately-preceding version (erasure and the report would each need a recursive walk up
  the chain, and one erased link in the middle — whose origin is nulled — orphans everything
  behind it).
- **The send time of a revision is the edit time, 2026-08-25.** The adapter translates
  `edit_date` into the neutral timestamp and falls back to `date` when the platform sends
  none. `edit_date` is read only on the edited-message branch: the update TYPE decides that a
  message is a revision, never the presence of a field on the shared inbound shape, so an
  ordinary message carrying an edit date is still an ordinary message. Whichever timestamp is
  chosen passes the existing representable-range guard, and a value outside it is the same
  named skip it is today (`translate.rs:168`). The block header keeps the store's own receipt
  time, so the ledger still holds both times it always held. *Rejected:* keeping the original
  send time (the row would claim a version existed hours before it did); deciding "this is a
  revision" from a non-null edit date (a forwarded message can carry one, and the fact would
  then be inferred rather than reported).
- **A revision whose text equals the newest recorded version records nothing, 2026-08-25.**
  Under the stamp lock, ingestion asks the store for the newest recorded version of the named
  message in that conversation and, when the incoming text is identical, answers `Disregarded`
  and touches nothing. The platform's own warning is the reason: an `edited_message` update
  "may at times be triggered by changes to message fields that are either unavailable or not
  actively used by your bot" — a link preview attaching itself, for one — and under helpful
  answering every group message is summoned, so each such update would otherwise open a debt
  and run a model turn on text nobody changed. It also makes a redelivered update after a
  halted batch idempotent, where a redelivered new message still duplicates. **This is not a
  protection mechanism and decision 0030 is not touched:** what is dropped is a redelivery of
  content the ledger already holds, byte for byte, under that same message — no statement a
  person made goes unrecorded. A genuinely different edit always records, however many times
  the person makes one. *Rejected:* comparing in the adapter (an adapter decides nothing, and
  it holds no store); a new outcome variant for "nothing changed" (every adapter would have to
  match a case it must treat exactly like `Disregarded`); recording every update faithfully (a
  duplicate of the same sentence in the model's context, and a turn for every link preview);
  bounding the volume by a budget instead (a budget refuses an ANSWER, never a row — decision
  0030 — so it cannot bound rows at all).
- **The comparison is one store read, defined exactly, 2026-08-25.** A new read on the
  message kind — the only place the column names live — returns the text of the newest
  recorded version of a named message in one conversation: `WHERE (origin = ?1 OR revises =
  ?1)` within the conversation junction, ordered by block id descending, one row. The block id
  is the ledger's own append order, so "newest" means the last version recorded and never a
  clock a platform supplied. It is one indexed statement, not a conversation load: the
  identical-text check runs on every edit update, and reading the whole ledger per edit would
  be a different cost class. Matching origin OR revises is what makes the read correct on a
  platform where a revision carries its own distinct origin; on this one the two coincide. It
  is a store READ, so it fails closed: a `StoreError` propagates as `CoreError::Store`,
  ingestion refuses, and the driver retries the batch — the same choice decisions 0041 and
  0052 record for every other admission read. Recording anyway on a failed read would write a
  duplicate row and, in helpful mode, spend a model turn on it. *Rejected:* reading the whole
  conversation and scanning it in memory (a full ledger load per link preview); ordering by
  the stored send time (a platform-supplied clock, and two edits within one second are then
  unordered); matching on origin alone (correct here, a silent no-op on the second platform
  this design exists for).
- **A revision naming a message the store does not hold records nothing, 2026-08-25.** When
  the read above finds no recorded version at all, the update is disregarded. This is the
  erasure guard, and it is the reason the rule is inverted from the obvious one. Erasure nulls
  a row's origin together with its text, so an erased message matches nothing — and the
  platform sends edit updates the person never asked for. Recording the revision as a fresh
  statement would therefore write a person's erased words, and their erased identifier, back
  into the ledger with no human act anywhere in the path, and in helpful mode would run a
  model turn on them. Between the two costs the erasure promise wins: what is given up is the
  case where an edit adds text to a message the store never held — a caption typed onto a
  photo that arrived without one. That message is not in the ledger, no one has read it, and
  nothing about the group's memory silently changes; an erased message resurrecting itself is
  a defect against a published promise. The exception is the privacy command family, exempt
  here exactly as it is exempt from the suppression re-read one line above: a rights command
  is answered whatever the store holds, so an edit invoking one records and is answered, and
  its row carries its `revises` fact even though nothing joins to it. *Rejected:* recording it
  as an ordinary new statement (the erasure defect above); a stored marker distinguishing
  "erased" from "never held" (erasure exists to leave nothing that points at a person's
  removed message, and a per-message tombstone is that pointer under another name);
  distinguishing a person-generated edit from a platform-generated one by the edit date (the
  platform documents no such guarantee, and building on it would be building on folklore).
- **A revision is stamped like any other message, 2026-08-25.** The same summons resolution,
  the same budgets, the same debt propagation, the same absorption rule for a message arriving
  mid-turn (decision 0010). A person who edits a question into shape gets an answer; a person
  who edits five times spends five of their own budget slots, which is what the budgets are
  for. A revision arriving while its own original is being answered is absorbed exactly like
  any mid-turn message, so the answer in flight may answer the pre-edit wording and the next
  turn sees both versions — the framework's scheduling law, unchanged here. In a direct
  channel every message is addressed by the channel's nature (`translate.rs:172`), so every
  revision there summons, exactly as every message there does. *Rejected:* a revision that
  never summons (a member fixing a typo in their question would never be answered, and under
  privacy mode the edit route is the only one that stayed open for a message the bot already
  knew); a separate edit budget (a second counter measuring the same thing).
- **The revision projects under the revised message's id, marked, and the earlier version
  keeps its line, 2026-08-25.** A revision projects as `[origin-of-the-revised-message]
  speaker: (edited) text`, the marker a fixed constant beside the erased marker. The bracketed
  id stays exactly the shape the report teaching names and the report tool validates, and the
  fixed word sits at the head of the text where it reads as what the room sees. The earlier
  version keeps projecting its own words: the projection reads one block with no ledger access
  (`kind.rs:546-549`), so hiding it is not something a per-block reading can do, and rewriting
  it is not something an append-only ledger does. The marker is prose and a member can type
  it, exactly as a member can type a bracketed id (`kind.rs:168-182`); the bound is that
  nothing mechanical reads it — no tool, no stamp and no erasure pass consults the marker, so
  a forgery can mislead the model's reading and reach nothing else, and the report tool's
  co-summoner validation still bounds where any forgery can aim. *Rejected:* folding the
  marker into the bracket (`[123 edited]`), which corrupts the one token the model is taught
  to name a message by; deriving the marker from the stored fact at read time in a way any
  mechanism could act on (it would make forged prose actionable, which the id mark is careful
  not to be); suppressing the superseded version through a write-time stamp on the older row
  (a mutation of a fact already read); waiting for the framework's superseding-block
  compaction, which `docs/follow-ups.md:19-23` records as unbuilt.
- **An edit that leaves no text records nothing, and the words already recorded stay,
  2026-08-25.** A member who deletes a message's caption produces an update with neither text
  nor caption, which translation already reports as the textless skip
  (`translate.rs:166,465-472`). It stays a skip. The alternative would be a recorded message
  with empty text, and the whole erased-marker reading rests on absent text meaning erasure
  and on nothing else ever producing it (`kind.rs:539-542`); a message row that projects as
  neither words nor the erasure marker has no honest reading. Recording a fixed retraction
  sentence instead would be the machine putting words in a person's mouth, and a member could
  type the same sentence. The consequence is stated rather than engineered away: the earlier
  wording stays in the ledger, and the route to removing it is the one the product already
  offers — the person's own deletion command, or an administrator's reply deletion. It goes
  into the impact assessment's addendum as a residual. *Rejected:* a recorded empty message (it
  breaks the erasure marker's exactness); a fixed retraction text (invented words attributed to
  a person, and forgeable); treating a text-emptying edit as an erasure of the original (the
  assistant would erase a person's recorded words on an act that is not a deletion request,
  and nothing distinguishes a caption deleted by mistake from one deleted on purpose).
- **The model is taught which version is meant, 2026-08-25.** A shared rules section in
  `teaching.rs`, composed into both answering modes beside the sourcing and audience rules,
  states: a message may appear again marked as edited under the same id; the edited version is
  what the person now means, so answer that one; when the earlier wording was already answered
  and the edit does not change what was asked, end the turn with no text. The last clause
  needs no new mechanism — unit 22 already delivers an empty turn as nothing. *Rejected:* a
  mechanical suppression of a second answer to the same id (the machine deciding what a person
  meant by their own edit, and the assistant then falling silent on a genuinely rewritten
  question).
- **Recognising a deletion command and acting on it become two separate readings,
  2026-08-25.** Today one predicate answers both "does this message name the moderation bot's
  deletion command" and "does the mirror act", and the stamp reads the second to decide the
  first (`assembly.rs:692`). They are split: the recognition keeps the existing body — the
  deletion token as the reported command, a group channel, a reply naming a stored message, a
  sender at or above the administrator floor — and the mirror's action is that recognition
  gated on the message revising nothing. The stamp reads recognition, so a deletion command
  arriving as an edit is marked as a command, takes no debt, spends no budget slot and stays
  silent, which is what an administrator addressing the other bot should get either way. This
  is the structural half of the mirror decision below and it exists because the first draft
  changed the action and silently changed the stamp with it. *Rejected:* leaving the two
  joined and accepting that an edited `/del` becomes an ordinary summoned message (a model
  turn on a command aimed at another bot, in a group where the assistant is meant to be
  invisible for these); adding the deletion token to the privacy command family to recover the
  stamp (the family is a rights mechanism with its own answer windows and its own suppression
  exemption, and a moderation command is neither).
- **The deletion mirror ignores revisions, 2026-08-25.** A message that revises another
  mirrors nothing. The mirror's whole premise is that the moderation bot receives the same
  command and deletes the message in the chat (decision 0082); nothing establishes that the
  moderation bot acts on edited commands, and an assistant that erased its stored copy of a
  message still visible to everyone would produce precisely the divergence the mirror exists to
  prevent. The privacy self-service commands are the opposite case and stay reachable through
  an edit: only the author can edit their own message, so an edited `/forget` is that person's
  own ask about their own data — with the honest limit that under platform privacy mode a
  command first appearing in an edit may never arrive at all. *Rejected:* mirroring anyway (an
  invisible one-sided deletion); refusing every command that arrives through an edit (it would
  silently swallow a person's own rights request).
- **Erasure and the report reach every version, 2026-08-25.** The revision reference is
  personal data of its author, so the author-keyed pass nulls it with the other five columns.
  The mirror's named erasure matches `origin = ?1 OR revises = ?1` and nulls the revision
  reference too, so "delete this message" reaches every recorded version of it, on this
  platform and on one where the revision carries its own distinct origin. Because every
  version names the original, one match reaches a chain of any length. The report tool's
  resolution matches the named id against `origin` or `revises`. Which version it resolves is
  defined rather than left open: the first match in turn order, the earliest version present
  in the turn's assessment set. Nothing depends on the choice — the only facts the tool reads
  from that row are the role and the principal id, and both are identical across versions of
  one message (the author assumption above) — and the report block carries the id, not the
  text (`records-of-processing.md` R5). Decision 0092 stands unchanged: one report per
  message, not per version — a report already filed is not re-filed because the text moved, and
  administrators read the message as it now stands. *Rejected:* one report per version (an
  assessment per keystroke, and an edit-spammer could manufacture a report flood aimed at their
  own message); resolving deliberately to the newest version (an extra ordering read that buys
  nothing, because no fact the tool reads differs between the rows); storing the assessed text
  in the report block so the evidence is fixed (it would send message content to a recipient
  that receives none today — `records-of-processing.md` R5 — which is a change of what a
  recipient receives, not a bug fix).
- **The zero message id is named, not defended against, 2026-08-25.** The platform documents
  `message_id` as `0` for an ephemeral message and for a message the server scheduled instead
  of sending. The adapter stores the id opaquely, and the core treats every origin as an opaque
  key, so a `0` would let two distinct messages share one key — for the revision reference
  exactly as for the reply reference and the deletion mirror that already key on it. This unit
  does not add a validity check for one platform's sentinel value in the core, because a check
  on the SHAPE of an id is the platform vocabulary the core must not carry, and the adapter
  decides nothing. It is recorded here as the known edge, with its receipt, so the unit that
  handles ephemeral messages inherits a stated fact rather than rediscovering it. *Rejected:*
  refusing an origin of `"0"` in the core (a platform's sentinel spelled into platform-neutral
  code); silently letting the exception go unrecorded because it predates this unit (the two
  new readers this unit adds make it worse, and an unwritten edge is the defect this document
  exists to prevent).
- **The assistant does not edit or delete its own delivered messages in this unit, and does
  not delete anyone else's ever, 2026-08-25.** Two separate statements, for two separate
  reasons.
  The self-edit half is a scope statement, not a capability judgement: the platform allows it
  without a time limit, and our tree cannot address its own messages today — the client
  discards the `Message` every send returns (`client.rs:459`), one answer reaches the chat as
  several messages under the chunking rule (`client.rs:377-391`), and the core keeps no record
  of any platform id it ever spoke under (`message.rs:164-166,372-390`). Closing that needs a
  delivery record binding an answer block to the ordered platform ids it was delivered as, and
  Telegram unit 04 specifies exactly that, in this directory and on this date. This unit adds
  no such call and takes no position on unit 04's design; the two units touch different code
  and whichever lands second inherits the other's vocabulary. What this unit does refuse is
  re-editing a delivered answer when its question is edited: the answer would silently change
  under readers who already read it, and the stored answer block would stop being what the
  channel saw, which is the equality decision 0079 exists to keep. Any future capability
  appends its superseding record before the edit call, on that same rule.
  The deletion half is a settled refusal. The platform grants an administrator bot the power to
  delete any message in the group, and helpful-mode deployments run the bot as an administrator
  (`dpia.md:169-176`), so the capability is live and unused on purpose. Decision 0070 places a
  human at every moderation effect: the assistant assesses and administrators act. A message
  the assistant removed from the chat on its own reading would be a moderation effect with no
  human in it, and no unit may add one. The deletion mirror is not a counter-example — there
  the administrator's own command is the human decision, and the assistant only erases its
  stored copy.
  *Rejected:* building the delivery record here (unit 04 owns it, and two specifications of one
  block is how they diverge); editing without recording (the ledger would carry an answer nobody
  can read in the chat any more); streaming the answer into a message edited as it grows (the
  group rate limits, the moving chunk boundaries, and an answer that reads as finished several
  times before it is — and the platform's own streaming methods are private-chat-only and 30
  seconds long).
- **Five documents change with the code, 2026-08-25.** Shipping this while the published texts
  say edits are not collected would make a published statement false, which is a defect. The
  policy's message paragraph (`bot-assistant-privacy-policy.md:22`) says plainly that an edited
  message is stored as a further version beside the first. The record of processing changes D1
  and its minimisation row (`records-of-processing.md:61`, `:144`), adds the revision reference
  to the erasure table in section 8, and states that the edit time is recorded as circumstance.
  The impact assessment changes 3.2 and the necessity paragraph (`dpia.md:130`, `:336`) and
  gains a dated addendum, because its own section 10 makes "a change to what is collected: …
  edits" a mandatory review trigger (`dpia.md:566`). The legitimate-interest assessment moves
  "edits are not collected" out of its not-necessary-therefore-not-done list (`lia.md:107`) and
  states the necessity that replaces it — storing only the first version means holding a record
  of what a person said that the person has already corrected — together with the honest bound:
  every distinct version is kept, decision 0003 sets no retention timer, and what the drops
  above remove is only the platform's own repeated deliveries of unchanged text, never a
  version a person wrote. The fifth document is the operator reference, whose mirror section
  states its bounds as a complete list (`group-operator-contract.md:150-158`): a third bound
  joins them, that a deletion command arriving as an edit mirrors nothing and stays silent. No
  new recipient and no new category of recipient: the revised text reaches the same processor
  as every other message, under the same terms. The AI Act record needs no change — nothing
  here touches a person's standing in the group. *Rejected:* shipping the code first and
  correcting the documents afterwards (between the two, the published statements are wrong);
  claiming in the assessment that the added volume is bounded (it is not, and a document that
  overstates a safeguard is worse than one that states the residual).
- **The impact assessment's addendum states four residuals, 2026-08-25.** Not one. A member
  can edit a message after it was reported, so what administrators then see is not what the
  assistant assessed. Every version a person writes is retained, and the product offers no
  per-message correction — only the person's own deletion command, which removes everything,
  or an administrator's reply deletion, which now reaches every version of the one message. An
  edit that empties a message's text records nothing, so the earlier wording stays. And an edit
  naming a message the store does not hold is dropped, which is the completeness price paid for
  the erasure guard. *Rejected:* naming only the report residual (the assessment exists to name
  what the design accepts, and three of these are accepted by decisions taken above).

## The unit's contract

A member's edit is recorded, never lost and never rewritten, and never resurrects what
erasure removed: the adapter translates the edit update like any message and reports the
revised message's origin and the edit time, and the core appends an ordinary message block
carrying a `revises` reference — unless the text equals the newest recorded version, or the
store holds no version of that message at all, in both of which cases nothing is recorded and
the update is acknowledged, the privacy command family excepted as it is everywhere else. An
edit that leaves no text stays the textless skip it is today. The revision is stamped,
budgeted, absorbed and answered exactly like a fresh message; it projects under the revised
message's bracketed id with a fixed edited marker that no mechanism reads, the earlier version
keeps its own line, and the teaching tells the model that the later version is what the person
means and that an unchanged ask needs no second answer. Recognising the moderation bot's
deletion command and acting on it become two readings, so a deletion command arriving as an
edit is still marked a command — silent, no debt, no budget slot — while the mirror leaves the
store alone, because the moderation bot's matching act is not established for edits. Erasure
reaches the revision reference from both ends, the mirror's named erasure reaches every version
of one message, and the report tool resolves a named id through either column and still files
at most one report per message. The assistant edits none of its own delivered messages in this
unit and deletes no member's message ever. Five published documents change in the same commit.
No new dependency, no new platform vocabulary in the core.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan and the secret scan clean; no new dependency.
- **AC2** An edit of a recorded message appends a second message block in the same
  conversation carrying the revised message's origin in the new column, this version's own
  origin, the sender, the speaker, the reply target, the edit time as the send time and the
  stamp the message earns — pinned block by block against the descriptor's actual column list,
  and pinned over the wire from an edit update. The earlier row is untouched in every column.
  `Skip::EditedMessage` no longer exists.
- **AC3** An edit whose text equals the newest recorded version of that message records
  nothing and delivers nothing, and the update is acknowledged. A different edit records
  normally, and an edit that returns to a previous wording after a different one records
  normally too — the comparison is against the newest version, not against the history.
  Redelivery of the same update with no other version recorded in between records once —
  pinned.
- **AC4** An edit naming a message the store holds no recorded version of — never held, or
  emptied by erasure — records nothing and delivers nothing; the erased row stays erased and
  its text does not reappear in the ledger or in any projection. An edit invoking a privacy
  self-service command in the same situation records and is answered, its row carrying the
  revision reference — pinned, including the full sequence: record, erase person-wide, deliver
  an edit of the erased message, assert no row carries the erased text.
- **AC5** A failed store read in the identical-text path refuses the ingestion as a store
  error and records nothing; the adapter's batch discipline treats it as it treats any other
  transient refusal — pinned with a failing store.
- **AC6** The revision summons and answers under both answering modes exactly as a fresh
  message does, including the budget count and the debt propagation, and a revision arriving
  mid-turn is absorbed under decision 0010. In a direct channel a revision summons like every
  message there — pinned, the direct case beside the existing direct-chat spine cases.
- **AC7** The projection of a revision is the revised message's bracketed id, the speaker
  prefix, then the fixed edited marker and the text; the superseded version projects unchanged;
  an erased revision projects the erasure marker with no id and no marker. A member's own
  message whose text begins with the marker's literal characters records and projects
  unchanged, and no stamp, tool or erasure pass reads the marker — pinned in the projection
  tests.
- **AC8** An edit that leaves a message with neither text nor caption records nothing and is
  acknowledged, and the earlier version's row is untouched — pinned over the wire.
- **AC9** The teaching carries the revision rules in both modes and no platform vocabulary,
  with the whole sourcing paragraph and the end-with-no-text rule retained verbatim — pinned in
  the prompt-composition tests by comparing the composed sourcing paragraph against
  `sourcing_rules()` itself, not against a copied string.
- **AC10** An administrator's deletion command arriving as an edit erases nothing and is
  recorded with the command stamp: no debt taken, no budget slot spent, no delivery. The same
  command sent as a new message mirrors as before, and a non-administrator's deletion command
  is unchanged in both forms — pinned in helpful mode, where an unstamped message would
  otherwise summon.
- **AC11** A person's erasure nulls the revision reference on every row they wrote, and the
  deletion mirror's named erasure empties every recorded version of the named message —
  including a version stored under a distinct origin, and a chain of three versions — plus the
  reply references naming it, and its row count reports the versions it emptied — pinned.
- **AC12** The report tool resolves an id the projection showed whether it matches the origin
  or the revision reference, reports the same principal for either version, and refuses a
  second report of the same message after an edit — pinned.
- **AC13** No call to `editMessageText`, `deleteMessage`, `deleteMessages` or either draft
  method is introduced by this unit, and the outbound consumer still discards the sent message
  — checked as the absence of those method names in the adapter's request builders at the
  commit this unit lands, stated so that a later unit adding one does so deliberately.
- **AC14** The five documents ship changed as the decisions describe — pinned in the docs
  test with exact substrings: the policy stating that an edited message is stored as a further
  version beside the first, the record of processing naming the revision reference in its
  erasure table, the impact assessment's dated addendum carrying all four residuals, the
  legitimate-interest assessment's replacement necessity with its unbounded-volume statement,
  and the operator reference's third mirror bound. The docs test's existing pins on the mirror
  section and the piggyback bounds still pass.

## Notes for launch

- Sites, adapter: `translate.rs:26-76` (drop `Skip::EditedMessage`), `:79-114` (`Pending`
  gains `revises`), `:119-195` (translate handles the edit branch and carries the revised
  origin and the edit time; the ordinary branch ignores the edit date entirely), `:474-495`
  (the Display arm goes), `client.rs:124-144` (`Incoming` gains `edit_date`),
  `driver.rs:425-436` (the neutral message gains the field). `CONSUMED_UPDATE_TYPES` at
  `client.rs:103` already subscribes; nothing there changes.
- Sites, core: `message.rs:169-210` (the `revises` field and its doc), `kind.rs:151-158`
  (the new column constant beside the reply-target pair), `kind.rs` (the revised marker
  constant beside `ERASED_MARKER` at `:159-166`), `:444-485` and `:599-637` (the field map and
  its inverse — carry the two identifiers as one small value, the way `RecordedSender` and
  `Stamp` already travel), `:575-597` (the descriptor column), `:555-569` (the projection),
  `:688-705` (a sixth null), `:743-784` (match origin or revises, null revises, and correct the
  "ONE target row" sentence at `:719-722` — the doc contract decision 0085 records is amended,
  not the decision's substance), a new read in the same module beside the erasure reads
  (`:688-823`) returning the newest recorded version's text, `schema.rs:361-392` (one appended
  migration step after the literal-addressed step,
  added to the list in order), `assembly.rs:661-666` (both drops immediately after the
  suppression re-read and BEFORE `reconcile_palette` at `:667-672`, so `Disregarded` keeps its
  documented meaning — a drop placed after the palette reconciliation would have appended a
  delta block, and the conversation id it needs is already resolved above the stamp lock),
  `assembly.rs:687-699` (recognition read once for the stamp, the mirror's action gated on it),
  `mirror.rs:43-70` (the split predicate, both halves in this module),
  `teaching.rs:98-135` (the shared revision rules, composed into both modes),
  `tools/report.rs:293-320` (resolve through either column).
- The privacy family exemption for the two drops reads exactly like the one already at
  `assembly.rs:661-666`; keep it one condition, not two, so the exemption cannot drift apart.
- Tests: `crates/adapters/telegram/tests/adapter/translation.rs:207-230` is rewritten from a
  skip pin to a recording pin; the spine gains revision cases beside `storage.rs`,
  `projection.rs`, `mirror.rs`, `report.rs`, `erasure.rs`, `helpful.rs`, `protection.rs` and
  `direct_chats.rs`; `crates/assistant/tests/docs.rs` covers the document changes.
- Records: amend decision 0017 with the deferral falling due, note the mirror's narrowing and
  the recognition/action split on decision 0082, note the amended row-count wording on decision
  0085, and write the new records from the next free number (0106 onward). Decisions 0030,
  0070, 0079 and 0092 are unchanged and are cited, not amended.
- Telegram unit 04 in this directory specifies the delivery record and the assistant's own
  deletions. This unit deliberately does not restate that design and does not edit that file;
  the only overlap is `ReplyTarget::AssistantMessage`, which unit 04 changes and this unit only
  cites.
- The platform facts above were read from the live documentation on 2026-08-25 and are the
  receipts for this unit; the implementer verifies against the tree and the build, not against
  the API again. The one claim deliberately NOT recorded as a fact is the folklore that the API
  refuses an unchanged edit: it is undocumented, and nothing may be built on it from here.
