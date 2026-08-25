# Telegram unit 10 — forum topics: one chat, many conversations

Date: 2026-08-25. A supergroup with topics enabled is one chat id carrying many separate
conversations. The assistant's channel-to-conversation mapping is keyed on the chat alone
(`mapping.rs:52-56`), so today every topic of a forum shares one ledger conversation, one
rules note, one channel budget and one privacy-notice window — and every answer the model
writes is delivered into the "General" topic, no matter which topic asked, because the
adapter never sends `message_thread_id` (`client.rs:439-460`). This unit makes a topic its
own conversation, splits the address the core routes on from the channel it admits, and
re-keys the two counters whose published wording says "per chat" so the split does not
quietly make four shipped documents false.

Two defects that exist in the tree right now are fixed on the way, both consequences of the
same blindness: the pseudo-reply every in-topic message carries is recorded as if the member
had replied to something (`translate.rs:454-462`), and a deterministic answer — the privacy
command, the rules acknowledgment — is sent to the chat without its topic
(`driver.rs:440,582,603-606`).

## The finding that reshapes this unit

**The naive fix makes the assistant leave the group.** The obvious change is to let the
adapter mint a per-topic channel string, `"<chat>:<topic>"`, and touch no core code at all —
the core treats `ChannelKey.channel` as opaque (`message.rs:12-21`). It cannot work. Group
admission is a persisted table keyed on exactly that string (`schema.rs:220-224`,
`authorization.rs:60-71`), and admission is fail-closed: a group with no row is refused with
the withdraw directive (`assembly.rs:1201-1205`), which the adapter performs as `leaveChat`
(`driver.rs:613-624`). The first message in any newly created topic would arrive under a
channel string nobody admitted, and the assistant would walk out of the whole supergroup.

So the split has to happen one level below the channel: the chat stays the unit of
admission, withdrawal, authority, budget and per-chat windows; the topic is a new part of
the address the mapping and the outbound edge use. That is the shape of every decision
below.

## Grounding

### The platform, read off the live documentation

Current version is **Bot API 10.3, 24 August 2026** (changelog heading). The prompt for this
unit named 10.1 as current; 10.2 (14 July 2026) and 10.3 have shipped since. Nothing in
either touches forum topics.

- **The fields on an incoming message.** `Message.message_thread_id` — "Unique identifier of
  a message thread or forum topic to which the message belongs; for supergroups and private
  chats only". `Message.is_topic_message` — "*True*, if the message is sent to a topic in a
  forum supergroup or a private chat with the bot". `Chat.is_forum` — "*True*, if the
  supergroup chat is a forum (has topics enabled)". The first field is **not** specific to
  forums: a non-forum supergroup linked to a channel carries `message_thread_id` for ordinary
  comment threads, and only `is_topic_message` distinguishes a forum topic from one. Keying
  on the presence of `message_thread_id` alone would fragment a discussion group into one
  conversation per comment thread.
- **The sending parameter.** `sendMessage` and every other sending method take
  `message_thread_id`: "Unique identifier for the target message thread (topic) of a forum;
  for forum supergroups and private chats of bots with forum topic mode enabled only".
  `sendChatAction` takes it too, added in Bot API 6.4: "Unique identifier for the target
  message thread or topic of a forum; for supergroups and private chats". Omitting it in a
  forum delivers to General.
- **The General topic is not a thread.** From Telegram's own forum documentation
  (`core.telegram.org/api/forum`): "Every forum has a non-deletable 'General' topic, with
  `id=1`; other topics will have other IDs, equal to the messageActionTopicCreate service
  message that created the topic." The Bot API maintainer, on
  `tdlib/telegram-bot-api` issue 356: "All messages not belonging to a topic are shown by
  apps in the 'General' topic", and "The on-site documentation is correct. It explains that
  'General' topic isn't a message thread and must not be used as such." So a General message
  arrives with no `message_thread_id` and no `is_topic_message`, and an answer to it is sent
  with no `message_thread_id`. The value 1 is never sent.
- **A bot cannot look a topic up.** The whole forum surface is thirteen methods —
  `getForumTopicIconStickers`, `createForumTopic`, `editForumTopic`, `closeForumTopic`,
  `reopenForumTopic`, `deleteForumTopic`, `unpinAllForumTopicMessages`,
  `editGeneralForumTopic`, `closeGeneralForumTopic`, `reopenGeneralForumTopic`,
  `hideGeneralForumTopic`, `unhideGeneralForumTopic`, `unpinAllGeneralForumTopicMessages` —
  and **not one of them reads**. There is no `getForumTopic`, no list method, and `getChat`
  returns no topic data. The maintainer, same issue: "Bots can't get topic name through Bot
  API. Currently, they can only save topic name from 'forum_topic_created' and
  'forum_topic_edited' messages as the closest approximation." **A topic created before the
  assistant joined, or while it was down past the update retention, has a name the assistant
  can never learn.** This is the single constraint that most shapes the feature, and it is
  not fixable on our side.
- **The service messages.** `Message.forum_topic_created` (`ForumTopicCreated`: `name`,
  `icon_color`, `icon_custom_emoji_id`, `is_name_implicit`), `forum_topic_edited`
  (`ForumTopicEdited`: optional `name`, optional `icon_custom_emoji_id`),
  `forum_topic_closed`, `forum_topic_reopened` (both "currently hold no information"),
  `general_forum_topic_hidden`, `general_forum_topic_unhidden`. They arrive as ordinary
  `message` updates, so the poll's `allowed_updates` selection
  (`client.rs:103`, `["message", "edited_message", "my_chat_member"]`) needs no change. Bot
  API 6.3 also states: "service messages about forum topic creation can't be deleted with
  the deleteMessage method".
- **Every in-topic message carries a pseudo-reply.** A message in a topic that replies to
  nothing arrives with `reply_to_message` set to the topic's own `forum_topic_created`
  service message — the community answer on issue 356, consistent with the platform's own
  rule that a topic's id equals the id of the service message that created it. So
  `reply_to_message.message_id == message_thread_id` identifies the pseudo-reply exactly.
- **Managing topics needs a permission the assistant will not ask for.** `createForumTopic`,
  `editForumTopic`, `closeForumTopic`, `reopenForumTopic` and the General variants all
  require the bot to be an administrator "with the `can_manage_topics` administrator right";
  `deleteForumTopic` requires `can_delete_messages` and "delete[s] a forum topic along with
  all its messages".
- **Ordinary members can create topics.** `ChatPermissions.can_manage_topics`: "*True*, if
  the user is allowed to create forum topics. If omitted, defaults to the value of
  `can_pin_messages`", and `ChatMemberRestricted` carries the same field. A forum may
  therefore let every member open topics — which is why the budget question below is a real
  abuse question and not a theoretical one.
- **A closed topic refuses messages.** `core.telegram.org/api/forum`: "Topics can be
  temporarily `closed`, preventing further messages from being sent to the topic", and
  `hideGeneralForumTopic` "will be automatically closed if it was open". A topic closed while
  a turn is composing refuses the answer's send.
- **Topics in private chats exist since Bot API 9.3 (31 December 2025)** and are off unless
  the bot is configured for them: `User.has_topics_enabled`, "*True*, if the bot has forum
  topic mode enabled in private chats. Returned only in `getMe`", beside
  `allows_users_to_create_topics`. Direct chats with topics are out of scope here, and the
  design costs nothing extra if they are ever switched on.

### Our tree

- **The mapping is keyed on the chat and nothing else.** `channels (adapter, channel, kind,
  conversation_id)`, `PRIMARY KEY (adapter, channel)`, `conversation_id` unique
  (`schema.rs:94-106`). Read one way on ingestion (`mapping.rs:45-64`, `assembly.rs:1192`),
  the other way on the outbound edge (`mapping.rs:136-152`, `outbound.rs:316,402`) and on the
  composing edge (`composing.rs:230`).
- **The outbound reply is already addressed by the key.** `OutboundReply.channel`
  (`message.rs:375`) is resolved from the mapping, and the adapter turns it back into a chat
  id through the one naming rule and its inverse (`translate.rs:312-325`). Widen what the key
  carries and the answer routes itself: no new plumbing between the core and the send.
- **The reply target and the chunking already exist.** `OutboundReply.reply_target`
  (`message.rs:389`) becomes `reply_parameters` with `allow_sending_without_reply`
  (`client.rs:455-460`); text past the cap goes out as consecutive chunks with only the first
  threaded (`client.rs:371-390`); markdown becomes HTML per chunk with a plain-text retry
  (`client.rs:439-452`, `formatting.rs`).
- **Group admission and withdrawal are per chat.** `group_authorizations (adapter, channel)`
  (`schema.rs:218-228`), checked at `assembly.rs:1201`, performed at `driver.rs:613-624`,
  rested per chat id (`driver.rs:130-144`).
- **The first-contact lookup and the administrator cache are per chat.**
  `driver.rs:532-566` (`getChat` once per chat, rested on failure) and
  `authority.rs:18-30` (a one-minute administrator list per chat id). Both stay per chat:
  admin standing and the group's own facts are chat-wide.
- **The channel budget is keyed on the conversation.** `opened_debts_in_conversation`
  (`kind.rs:978-998`, `WHERE cb.conversation_id = ?1`), called at `assembly.rs:1594-1600`.
  The per-person budget is already global across conversations (`kind.rs:942-969`).
- **The privacy notice's window is keyed on the conversation.** `LineWindow`
  (`window.rs:59-75`), constructed at `assembly.rs:497`, spent at `assembly.rs:1296-1302`.
- **The disclosure does not need a fix.** The first-interaction line is read per person
  *across* conversations — "a person introduced on one channel is not introduced again in
  another" — through `kind::conversations_of_principal` (`disclosure.rs:170-190`). Splitting a
  forum into many conversations does not re-introduce anybody.
- **Erasure is keyed on the principal, not the conversation.** The personal columns are
  nulled "in every conversation" (`erasure.rs:6-21`, `:141-148`); only *direct*
  conversations are deleted whole, found by walking the mapping rows of kind `Direct`
  (`erasure.rs:166-184`). A group split into sections changes nothing here.
- **Notes are blocks in one conversation.** A note appends only when the observed text
  differs from the newest stored note of the same note topic (`note.rs:1-15`), the vocabulary
  is closed by a CHECK constraint over a frozen list (`schema.rs:198-210`), and decision 0094
  guarantees the rules note sits in the model's context. A section conversation with no rules
  note would break that guarantee.
- **The framework can fork and detach, but not attach.** `fork_conversation` inherits the
  source's junction rows up to a block (`agent-ledger conversations.rs:317-336`) and
  `detach_block` removes one membership (`:358-371`). There is no operation that adds an
  existing block to another conversation, so notes cannot be shared between section
  conversations by junction.
- **Typing is keyed on the chat id.** `TypingRefreshers.running: HashMap<i64, AbortHandle>`
  (`driver.rs:634-680`), begun and stopped from the composing edge (`driver.rs:705-721`) and
  stopped again before each send (`driver.rs:730-745`).
- **Deterministic items are sent to the bare chat.** `send_item(client, pending.chat_id, …)`
  at `driver.rs:440` and `:582`, defined at `driver.rs:603-606`.
- **Widening a closed vocabulary has a worked precedent.** `COMMAND_STAMP_MIGRATION`
  recreates a content table under a frozen widened list, copies every row, drops, renames and
  rebuilds the index (`schema.rs:239-271`); every appended step quotes a frozen vocabulary,
  never a live enum (`schema.rs:115-124`), and a test pins the newest frozen list to the enum
  (`schema.rs:399-410`).
- **What the published documents say.** "counters bound how much it answers per person and
  per chat" (`docs/privacy/records-of-processing.md:40`, and again at `:149`),
  `docs/privacy/dpia.md:125,284,476`, `docs/privacy/lia.md:47`,
  `docs/privacy/bot-assistant-privacy-policy.md:34`; "the privacy command answers … at most
  once per chat per window" (`records-of-processing.md:153`). Group facts are recorded as
  "Channel title, pinned rules text, stored as context notes" (`records-of-processing.md:64`,
  row D4).

## Decisions taken with this unit

- **A topic is its own conversation, 2026-08-25.** Each forum topic maps to one ledger
  conversation of its own; the General topic keeps the chat's existing conversation. The
  reason is not tidiness, it is correctness: the framework decides an owed turn, a frontier
  and an absorbed span per conversation, so one conversation for a whole forum makes the
  provenance walk fold people who spoke in an unrelated topic into the summoners of an answer
  (`tools/provenance.rs`, the absorbed span of user messages since the last answer), serialises
  every topic behind whichever turn is running, and hands the model a context in which twelve
  subjects interleave. *Rejected:* one conversation per chat with a per-message topic label,
  stored on the message row and read at projection — cheaper to build, but it leaves the
  provenance fold, the serialisation and the mixed context exactly as they are, and it puts a
  platform-shaped field on a block that has no use for it. *Rejected:* a conversation only for
  topics the assistant has been named in — the mapping would then depend on the model's
  behaviour, and a member's second question in the same topic could land in a different
  conversation than the first.
- **The address splits from the channel, 2026-08-25.** `ChannelKey` keeps meaning exactly
  what it means today — one chat on one adapter — and a new `ChannelAddress { channel:
  ChannelKey, section: Option<String> }` names one conversation surface inside it. The
  mapping, `InboundMessage`, `Observation`, `OutboundReply` and `ComposingUpdate` carry the
  address; admission, authorization, withdrawal, the administrator cache, the first-contact
  lookup, the channel budget and the per-chat window keep the key. The type says which is
  which, so no call site can admit a section by accident. *Rejected:* a third field on
  `ChannelKey` itself — every existing site would then have to remember to ignore it, and the
  admission check that forgets is the one that makes the assistant leave the group.
  *Rejected:* the adapter minting `"<chat>:<topic>"` into the existing string — the same
  failure, with the core unable to tell that two conversations belong to one group, which
  also makes the budget and the note fan-out below impossible.
- **The neutral word is "section", 2026-08-25.** The core says a channel may be divided into
  sections; the adapter translates a forum topic to a section and back. *Rejected:* "topic" —
  the core already uses that word for a note's subject (`note.rs:68-80`, `NoteTopic`), and two
  meanings of one word in one crate is how a reader is misled. *Rejected:* "thread" — it is
  the platform's own word on two of the platforms we plan for, and Rust's word for something
  else entirely. *Rejected:* "compartment" — accurate but heavier, and "a section of a
  channel" reads plainly.
- **No section means the channel itself, 2026-08-25.** A `None` section is the address of a
  chat with no topics and of a forum's General topic alike, and the stored form of `None` is
  the empty string. Two consequences follow for free: an existing mapping row migrates to
  section `''` and keeps its conversation, so a group that becomes a forum later finds its
  whole history in the General conversation; and the adapter never sends `message_thread_id`
  for a General answer, so the value 1 — which the platform says must not be used as a thread
  — is never on the wire. *Rejected:* minting an explicit section "1" for General to make the
  vocabulary uniform, which contradicts the platform's own statement and would migrate every
  existing row into a section it never had. The empty string, not SQL NULL, is
  deliberate: SQLite permits NULL in a non-integer PRIMARY KEY column, so a NULL section would
  silently defeat the uniqueness the mapping depends on. The column is `NOT NULL DEFAULT ''`.
- **Only `is_topic_message` produces a section, 2026-08-25.** The adapter reads a section from
  `message_thread_id` exactly when `is_topic_message` is true; otherwise the address has no
  section. *Rejected:* keying on `message_thread_id` being present, which would split a
  discussion group's ordinary comment threads into a conversation each and scatter one group's
  context across hundreds of conversations that no lookup can ever name.
- **The channel budget and the per-chat window re-key from the conversation to the channel,
  2026-08-25.** `opened_debts_in_conversation` becomes `opened_debts_in_channel`: the same
  counting predicate with `cb.conversation_id = ?1` replaced by `cb.conversation_id IN (SELECT
  conversation_id FROM channels WHERE adapter = ?1 AND channel = ?2)`; `LineWindow` keys on
  `ChannelKey` instead of a conversation id. Two reasons, both hard. First, four published
  documents state that the counter is per chat and that the privacy answer comes at most once
  per chat per window (`records-of-processing.md:40,149,153`, `dpia.md:125,284,476`,
  `lia.md:47`, `bot-assistant-privacy-policy.md:34`); leaving the keys on the conversation
  would silently turn all of them into false statements, which is a defect on the day it
  ships, not a follow-up. Second, a forum may permit every member to create topics
  (`ChatPermissions.can_manage_topics`), so a per-conversation channel budget would let anyone
  multiply the group's whole answering allowance by opening topics. *Rejected:* keeping the
  per-conversation key and editing the documents to say "per topic" — it makes the abuse
  amplifier the official position. *Rejected:* a separate per-channel counter table — the
  ledger already holds every debt, and a second record of the same fact is a second thing to
  keep true.
- **The group's notes reach every section, 2026-08-25.** A title or rules note is a fact about
  the chat, so the observation surface appends it to every conversation the channel maps, and a
  section conversation created later is seeded at creation with the newest note of each note
  topic held by its siblings. Decision 0094 promises the rules note is in the model's context;
  a section conversation without one would break that promise silently — the model would
  answer in a topic under no rules at all. *Rejected:* sharing the note blocks by junction, so
  one block sits in every conversation — the framework has `detach_block` but no attach
  (`conversations.rs:358-371`), so this cannot be built without changing the framework.
  *Rejected:* forking each section conversation from the channel's General conversation and
  detaching everything that is not a note — it inherits and then unpicks the entire history,
  one detach per block. *Rejected:* keeping notes only in the General conversation — every
  other topic then runs without the group's rules. Appending the same text again in another
  conversation is a new fact about that conversation, which is what an append-only ledger
  expresses; nothing is rewritten.
- **A section's name is a note of its own, 2026-08-25.** `NoteTopic` grows a third variant for
  the section's name, written from `forum_topic_created` and superseded by `forum_topic_edited`
  through the note module's existing delta comparison. *Rejected:* recording the name as the
  existing title note — the channel's title fans out into the same conversation, and the two
  facts would supersede each other in turn, appending a new note on every observation forever.
  *Rejected:* not recording the name at all — the model would answer in a topic called "Bug
  reports" without knowing it, and the platform offers no second chance to learn the name.
  The widening is an appended migration that recreates the note table under a frozen
  three-value list, following `COMMAND_STAMP_MIGRATION` (`schema.rs:239-271`).
- **A nameless section is normal, 2026-08-25.** Because no method reads a topic, a section
  whose creation the assistant did not witness has no name and never will. Such a section works
  in every other respect; the model simply is not told what the topic is called. *Rejected:*
  deriving a name from the topic's first message, which would put a member's words into the
  system voice as if they were the group's own fact. *Rejected:* refusing to answer in an
  unnamed section, which would make the assistant useless in every group it joins after the
  topics were made.
- **A section-name observation creates the section's conversation, 2026-08-25.** The name
  arrives once, at creation, and there is no way to fetch it later, so the observation maps
  the section instead of dropping the fact. The cost is one conversation with its prompt and
  palette blocks per created topic, in a group the operator already admitted, where creating a
  topic needs a right the group itself grants; the expensive resource, model answering, stays
  bounded by the per-chat budget above. *Rejected:* waiting for a member's first message in
  the section, which loses the name permanently. *Rejected:* parking unmatched names in a side
  table until a first message arrives — new durable state whose only purpose is to postpone a
  conversation row.
- **The topic-root pseudo-reply is not a reply, 2026-08-25.** When a message's
  `reply_to_message` is the topic's own creation service message — recognised by
  `reply_to_message.message_id == message_thread_id`, which the platform's id rule makes exact
  — the adapter reports no reply target. This is a correction, not a new feature: today every
  first-level message in a forum topic is recorded as a reply to a message its author never
  replied to, in a stored personal-data field (D8 of the record of processing), which is an
  accuracy problem under Article 5(1)(d) before it is a threading problem. *Rejected:*
  decoding `forum_topic_created` on the replied-to message instead — it is a second field to
  decode for a fact the two ids already state, and it would miss nothing the id comparison
  misses.
- **The assistant manages no topics, 2026-08-25.** None of `createForumTopic`,
  `editForumTopic`, `closeForumTopic`, `reopenForumTopic`, `deleteForumTopic`, the General
  variants or the two unpin-all methods is called anywhere in this unit, and the client grows
  no method for them. Closing a topic silences people; deleting one destroys every message in
  it. Decision 0070 settles that the assistant assesses and a human decides, so a mechanism
  that hands it a moderation effect is out of the question — it is named here so that a later
  reader does not mistake the omission for an oversight. *Rejected:* opening a topic for a long
  answer, and closing a topic the assistant judges abusive; both are effects on people without
  a human decision point, and the second is exactly the power 0070 refuses.
- **Nothing tracks whether a topic is open, 2026-08-25.** A topic closed or hidden while a turn
  composes refuses the answer's send, and that refusal takes the existing path: logged, the
  reply dropped or cut short after the delivered chunks (`driver.rs:735-744`). *Rejected:*
  recording closure from `forum_topic_closed` and skipping turns for closed sections — the
  assistant may have missed the event entirely (it learns nothing about the time it was down),
  so the record would be confidently wrong, and the send's own refusal is the honest signal.
- **Topic mode in private chats stays off, 2026-08-25.** The bot is not configured for
  `has_topics_enabled`, so a direct chat has exactly one section, the empty one, and erasure's
  direct-conversation walk behaves as it does today. *Rejected:* enabling it in this unit —
  erasure finds a person's direct conversations by the messages they carry
  (`erasure.rs:166-184`), and a direct section the assistant itself opened could hold no
  message of theirs while its mapping row still names their chat. That is a hole to close
  before topic mode is switched on, not while.

## The unit's contract

After this unit, one forum supergroup is many conversations: each topic maps to its own ledger
conversation through an address that carries the chat and the section, the General topic keeps
the chat's existing conversation, and a non-forum chat is unaffected because its address has no
section. The chat remains the unit of everything that is about the group — admission,
withdrawal, the administrator list, the first-contact lookup, the answering budget and the
one-answer-per-window privacy notice — so the published "per chat" statements stay true and no
member can multiply the group's allowance by opening topics. Answers, reports, the rules
acknowledgment, the privacy answer and the typing indicator are delivered into the section they
belong to, with `message_thread_id` set for a topic and omitted for General. The group's title
and rules notes reach every section, and a section created later is seeded with them; a
section's own name is recorded when the assistant witnesses its creation or renaming, and its
absence is the normal case, not an error. A member's first-level message in a topic is no longer
recorded as a reply to the topic's creation service message. Erasure is unchanged: personal
columns are nulled by principal across every conversation of every section, and direct chats
still hold one section each. The assistant calls no topic management method and gains no power
over anybody. No bytes move differently: this unit adds no file, media or upload path, and the
reply path keeps sending chunk by chunk exactly as before. One new dependency: none.

## Acceptance criteria

1. **The suite is green in both modes**, with clippy, fmt and doc under denied warnings, the
   platform-vocabulary scan clean (`docs/platform-vocabulary.txt`), the secret scan clean, and
   no new dependency. The core names no forum, no topic in the platform sense and no
   `message_thread_id`.
2. **A forum becomes many conversations.** Two messages in two different topics of one chat
   record into two conversations; a third in General records into the chat's unsectioned
   conversation; all three channel rows share one `(adapter, channel)` and differ in `section`
   — pinned.
3. **The migration keeps every existing mapping.** A store written before this unit upgrades
   with each row's conversation intact under section `''`, and a fresh store ends at a
   byte-identical schema; the schema pin test covers both, and the note-topic vocabulary pin
   points at the new frozen three-value list.
4. **A discussion group is not fragmented.** A message carrying `message_thread_id` without
   `is_topic_message` records into the chat's unsectioned conversation — pinned against a
   decoded update, not a hand-built struct.
5. **Answers are delivered into the section that asked.** An answer to a question asked in a
   topic goes out with `message_thread_id` equal to that topic; an answer in General goes out
   with no `message_thread_id`; the report line and the deterministic delivery items (privacy
   answer, rules acknowledgment) use the same address as the message that caused them —
   pinned on the request bodies.
6. **Admission still follows the chat.** A first message in a newly created topic of an
   admitted group is recorded and does not withdraw; a message from an unadmitted group still
   withdraws once, with the withdrawal rest keyed on the chat — pinned, this being the failure
   the naive design produces.
7. **One chat, one budget.** Answers spread across several topics of one chat count against the
   single channel budget; crossing it silences the next addressed message in any topic of that
   chat; the per-person budget is unchanged — pinned, with the published "per chat" wording
   named in the test's own words.
8. **One chat, one privacy answer per window.** The privacy command answered in one topic draws
   recorded silence in another topic of the same chat inside the window — pinned.
9. **The rules reach every section.** After a pin delta, every conversation of the channel holds
   the new rules note; a section conversation created after the delta is seeded at creation with
   the newest note of each note topic; the acknowledgment is still delivered once, in the
   section where the pin happened — pinned.
10. **A section's name is recorded and superseded.** `forum_topic_created` records the name as a
    section note in that section's conversation, creating the conversation if it has none;
    `forum_topic_edited` supersedes it; an unchanged name appends nothing; a section the
    assistant never saw created carries no name note and still records, answers and delivers —
    pinned.
11. **The pseudo-reply is not stored.** A message whose `reply_to_message.message_id` equals its
    `message_thread_id` stores no reply target, while a genuine reply inside the same topic
    stores the replied-to message's id and a reply to one of the assistant's own messages still
    reports `AssistantMessage` — pinned.
12. **Two topics can compose at once.** Composing in two sections of one chat runs two typing
    refreshers, a stop in one leaves the other running, and each action carries its own
    `message_thread_id` — pinned on the refresher registry and the request bodies.
13. **Erasure spans sections.** One erasure nulls a person's messages across every section
    conversation of a group, and a direct chat still maps exactly one conversation — pinned.
14. **No management method exists.** The client carries no forum management call and the tree
    contains no reference to `createForumTopic`, `closeForumTopic`, `deleteForumTopic` or their
    General variants — pinned by a source scan, so a later addition is a deliberate act against
    decision 0070, not an accident.
15. **The privacy documents match the code.** `records-of-processing.md` D4 names the section
    name among the group facts and D5 names the section identifier in the mapping; the "per
    chat" statements at `:40`, `:149` and `:153` are re-read against the re-keyed counters and
    stand unchanged; no new recipient category, because R1 already receives "the group's
    context notes" and the section name is one.

## Notes for launch

- Branches from `main` into its own worktree; the workflow merges and deletes it as usual.
- **Core, message model** (`message.rs`): add `ChannelAddress` beside `ChannelKey` (:16), and
  move `InboundMessage.channel` (:173), `Observation.channel` (:220), `ComposingUpdate.channel`
  (:366) and `OutboundReply.channel` (:375) onto it; export it from `lib.rs:85-89`.
- **Core, mapping** (`mapping.rs`): the section joins every query — `find` (:45), `claim` (:80),
  `kind_for_conversation` (:114), `channel_for_conversation` (:136), `all` (:165),
  `delete_by_conversation` (:198, unchanged, keyed on the conversation) — plus one new
  `conversations_of_channel` for the budget and the note fan-out.
- **Core, schema** (`schema.rs`): one appended step recreating `channels` (:94-106) with
  `section TEXT NOT NULL DEFAULT ''`, `PRIMARY KEY (adapter, channel, section)`, the existing
  rows copied with `''`, and an index on `(adapter, channel)`; a second appended step widening
  the note-topic CHECK under a new frozen list, following the recreate-copy-drop-rename shape of
  `COMMAND_STAMP_MIGRATION` (:239-271); both registered in `store_config` (:376-395); the
  vocabulary pins at :399-410 re-pointed.
- **Core, budget** (`kind.rs:978-998`): `opened_debts_in_channel`, taking the channel and
  binding the window third; call site `assembly.rs:1594-1600`.
- **Core, window** (`window.rs:59-105`): `LineWindow` keys on `ChannelKey`; call site
  `assembly.rs:1296-1302`, which now resolves the conversation's channel.
- **Core, notes** (`note.rs`, `assembly.rs:930-1000`): the note append fans out over
  `conversations_of_channel`; `map_new_channel` (`assembly.rs:1416-1448`) seeds a new section
  conversation with the newest note of each note topic from its siblings, after the prompt and
  palette blocks; `NoteTopic` (`note.rs:68-80`) grows the section variant with its own lead
  line; `ObservedFact` (`message.rs:230-244`) grows the section-name fact.
- **Core, prompt retirement** (`assembly.rs:852-910`): the re-claim after a fork passes the full
  address, not the bare key.
- **Adapter, decode** (`client.rs:125-148`): `Incoming` gains `message_thread_id` and
  `is_topic_message`, both optional and leniently decoded; `forum_topic_created` and
  `forum_topic_edited` gain their two small structures (`name` only, plus `is_name_implicit`
  where it helps a log line).
- **Adapter, translate** (`translate.rs`): `channel_key` and `chat_id_of` (:312-325) gain their
  section pair, spelled decimal like the chat id and kept side by side with its inverse;
  `reply_target_of` (:454-462) drops the topic-root pseudo-reply; the forum service messages
  become observations instead of `Skip::NoText`; `Pending` (:82) carries the section.
- **Adapter, send** (`client.rs:371-460`): `send_message`, `send_body` and `send_chat_action`
  take the section and add `message_thread_id` when it is present. The section is sent
  explicitly on every call, including replies inside a topic: the platform documents no
  inference from `reply_parameters`, and an answer that arrives in General because we assumed
  one is the bug this unit exists to remove.
- **Adapter, driver** (`driver.rs`): `TypingRefreshers` (:634-680) keys on the address;
  `consume_composing` (:705-721) and `consume_replies` (:730-745) decode the section;
  `send_item` (:440, :582, :603-606) takes the address of the message that caused it;
  `first_contact` (:532), `leave` (:613), the withdrawal rest (:130-144) and the administrator
  cache (`authority.rs`) stay keyed on the chat id and must not be touched.
- **Documents:** `docs/privacy/records-of-processing.md` rows D4 and D5; decision records for
  the section address, the re-keyed counters and the note fan-out, each dated with its rejected
  alternatives, in `docs/decisions`.
- **Read but not edited:** `docs/units/telegram/02-sending-media.md:62` already lists
  `message_thread_id` among the parameters every sending method shares, so the media unit and
  this one meet at the same client signature; whichever merges second inherits the other's
  section parameter instead of adding a second one. `docs/units/13-deletion-mirror.md`'s
  mechanism is unaffected in the ordinary case, because a moderation bot's delete command
  arrives in the same topic as its target and therefore in the same conversation — but
  `deleteForumTopic` deletes every message of a topic and produces no update at all, so a topic
  deletion is invisible to the mirror and its messages stay in the ledger. That is a limitation
  of the platform's update surface, stated here instead of fixed there.
