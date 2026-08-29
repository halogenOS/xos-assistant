# Telegram unit 21 — video chats: four notices about a conversation the assistant can never enter

Date: 2026-08-27. Telegram tells a bot four things about a group's video chat: one was
scheduled, one started, one ended, and these named people were invited into it. It tells a bot
nothing else, and it offers a bot no way in. There is no method to start a call, schedule one,
join one, hear one, speak in one or end one; there is no update while a call runs; no audio
ever reaches a bot. The whole feature, from this repository's side, is four service messages
arriving about something happening beside the chat.

So this unit has two halves and both of them are refusals, for different reasons.

The **outbound** half is impossible, not declined. A member who asks the assistant to join the
call, listen in, take notes or say what was said is asking for something no bot can do. The
failure to prevent is not a wrong platform call — none can be written — but a wrong sentence:
a model asked to join a call agrees, promises notes, and then invents what it heard. This
repository already treats a false statement about the assistant's own nature as a defect
(decision 0080, whose prose sits in the conduct prompt, `prompts/30-conduct.md:1-11`). The
deliverable is the passage that stops it, written generally enough to cover the next ask of
the same shape and narrowly enough that the media work does not make it false.

The **inbound** half is a real choice, because these four messages arrive whether we want them
or not, and the answer is no. Nothing about a call is recorded: not that one started, not who
started it, not how long it ran, and above all not the list of members somebody invited into
it. That list is the sharpest case in this series. It is an array of full `User` objects —
identifier, first name, last name, username, language tag — naming people who have said
nothing, written into the chat by somebody else's action. Every route this project has to a
person begins with that person having spoken: a principal row is resolved on the ingestion path
(`identity.rs:47-59`), the first-interaction disclosure is resolved per person from the
ledger's own memory, the privacy commands are commands inside a message
(`privacy.rs:95-105`), and the suppression flag can only be set by the person themselves.
Record an invited member and the project has created a record of somebody it cannot tell,
who cannot object, and whose objection it could not have consulted because there was no way
for them to make one.

## What the platform refuses, stated before the design

Four facts, each read from the live documentation on 2026-08-27 and not recalled:

1. **There is no video chat method.** The API page defines 185 methods. None of them names a
   call, a video chat or a conference. The only method name on the page containing "call" is
   `answerCallbackQuery`, which is about inline buttons (Telegram unit 07). A bot cannot
   schedule, start, join, leave or end a video chat, and has no way to learn who is in one.
2. **There is no video chat update, and no audio.** Nothing arrives while a call runs. The four
   notices are ordinary `message` updates carrying a service field, which is why the poll's
   subscription needs no change at all.
3. **The administrator right does not unlock anything for a bot.** `can_manage_video_chats`
   exists on `ChatMemberAdministrator`, on `ChatAdministratorRights` and as a parameter of
   `promoteChatMember` — and when the right was introduced (as `can_manage_voice_chats`) the
   changelog said outright that "bots can use this privilege only for passing to other
   administrators". The current page has no method the right could be exercised through, so
   even an assistant somebody promoted gains nothing here. Telegram unit 20 makes promotion a
   non-event for the moderation methods; this unit records that promotion is a non-event here
   for a stronger reason — the method does not exist.
4. **The four notices arrive unconditionally.** The features page states, of privacy mode:
   "All bots will also receive, regardless of privacy mode: All service messages." No
   administrator standing, no `allowed_updates` change, no configuration. Whatever this
   repository decides, these messages will be delivered to it — which is exactly why the
   decision has to be written down instead of left to whichever check happens to drop them.

Nothing in this unit moves bytes. The four notices carry no file, no `file_id`, no media and
no attachment, so there is nothing to stream to disk or to the wire, and the standing streaming
constraint is untouched.

## Grounding

### The platform, read 2026-08-27

Fetched from `https://core.telegram.org/bots/api`, `https://core.telegram.org/bots/features`
and the changelog at `https://core.telegram.org/bots/api-changelog` on 27 August 2026, as raw
pages. Everything in quotation marks below was read there.

**Version.** The changelog's newest entry is **Bot API 10.3, dated 24 August 2026**; 10.2 is
14 July 2026, 10.1 is 11 June 2026, 10.0 is 8 May 2026. The brief for this series named 10.1
as current, which is two releases behind — the same correction Telegram units 04, 08, 16 and
20 recorded.

**The four `Message` fields, verbatim from the `Message` table.**

- `video_chat_scheduled`, type `VideoChatScheduled`: "*Optional*. Service message: video chat
  scheduled"
- `video_chat_started`, type `VideoChatStarted`: "*Optional*. Service message: video chat
  started"
- `video_chat_ended`, type `VideoChatEnded`: "*Optional*. Service message: video chat ended"
- `video_chat_participants_invited`, type `VideoChatParticipantsInvited`: "*Optional*. Service
  message: new participants invited to a video chat"

**The four objects, verbatim and complete.**

- `VideoChatScheduled` — "This object represents a service message about a video chat scheduled
  in the chat." One field: `start_date`, Integer, "Point in time (Unix timestamp) when the
  video chat is supposed to be started by a chat administrator".
- `VideoChatStarted` — "This object represents a service message about a video chat started in
  the chat. Currently holds no information." No fields at all; on the wire it is `{}`.
- `VideoChatEnded` — "This object represents a service message about a video chat ended in the
  chat." One field: `duration`, Integer, "Video chat duration in seconds".
- `VideoChatParticipantsInvited` — "This object represents a service message about new members
  invited to a video chat." One field: `users`, Array of `User`, "New members that were invited
  to the video chat".

A caution for whoever re-checks this: a summarising fetch of the API page returned a fabricated
field, `video_chat_event_id`, for `VideoChatScheduled`. The field is `start_date`, read from
the page's own table. Re-verify against the raw page, not a summary.

**`User` is the full object** — "id", "is_bot", "first_name", Optional "last_name", Optional
"username", Optional "language_code", Optional "is_premium", and the `getMe`-only flags. So
`video_chat_participants_invited` carries, per invited member, the identifier this project
stores as D2 *and the first and last name that decision 0077 deliberately stopped storing*.

**"video chat" occurs fourteen times on the whole page** and nowhere else: the four `Message`
field descriptions, the four object introductions, the three field descriptions above, and the
three occurrences of `can_manage_video_chats` ("*True*, if the administrator can manage video
chats" on `ChatMemberAdministrator` and `ChatAdministratorRights`, "Pass *True* if the
administrator can manage video chats" on `promoteChatMember`). "voice chat", "group call" and
"live stream" occur zero times.

**The methods.** 185 in total; the only ones whose names contain "video" are `sendVideo` and
`sendVideoNote` (Telegram unit 02's territory), and the only one containing "call" is
`answerCallbackQuery`. There is no method to add to any refusal list, because there is no
method.

**The history, from the changelog.** Bot API 5.1 (9 March 2021) added `VoiceChatStarted`,
`VoiceChatEnded`, `VoiceChatParticipantsInvited` and the right, with the sentence "Added the
new administrator privilege can_manage_voice_chats to the class ChatMember and parameter
can_manage_voice_chats to the method promoteChatMember. For now, bots can use this privilege
only for passing to other administrators." Bot API 5.2 (26 April 2021) added
`VoiceChatScheduled`. Bot API 6.0 (16 April 2022) renamed all four fields and the right from
voice to video. Nothing since — 10.0 through 10.3 mention video chats nowhere. This corner of
the API has been static for four years, which is a reason to write the refusal down once rather
than to keep re-deciding it.

**Privacy mode.** The features page: "All bots will also receive, regardless of privacy mode:
All service messages. All messages from private chats. All messages from channels where they
are a member."

**The subscription.** `getUpdates.allowed_updates`: "Specify an empty list to receive all update
types except *chat_member*, *message_reaction*, and *message_reaction_count* (default)." The
notices are `message` updates, which this adapter already names
(`client.rs:103`, `CONSUMED_UPDATE_TYPES = ["message", "edited_message", "my_chat_member"]`).

### This tree, at `7fb217d`

- **The notices already arrive and are already dropped, by accident of another check.**
  `Incoming` decodes `message_id`, `date`, `chat`, `from`, `sender_chat`, `text`, `caption`,
  `reply_to_message` and `pinned_message` (`client.rs:125-144`) and nothing else; unknown
  fields are ignored by the decoder (`client.rs:105-107`). A video chat notice therefore
  reaches `translate` as a message with no text and no caption, and leaves it as
  `Skip::NoText` (`translate.rs:165-167`), whose display string is "a message with neither text
  nor caption" (`translate.rs:481`). A notice posted by an anonymous administrator has
  `sender_chat` set and leaves one check earlier, as `Skip::OnBehalfOfChat`
  (`translate.rs:159-161`). Both outcomes are correct today and neither says what was refused.
- **`from` is documented in this tree as absent on service messages** (`client.rs:130-131`),
  which is the platform's older behaviour; a modern video chat notice does carry the person who
  acted. The refusal must not depend on either reading.
- **The generic drop is about to change meaning.** Telegram unit 01 renames `Skip::NoText` to
  `Skip::NothingToRecord` (`01-receiving-media.md:249`) and makes a captionless media message
  a recorded block. After that unit, "no text" is no longer a refusal at all — it is a branch
  into media handling. A refusal that lives inside it is a refusal held by whoever edits that
  branch next.
- **Every skip in this adapter restates a recorded decision.** The module documentation says
  so: "every branch here restates a recorded decision, never invents one"
  (`translate.rs:1-3`), and each variant's doc comment names its decision (`translate.rs:40-77`).
  A drop with no name and no decision is the exception in this file, not the rule.
- **A skip is logged with the update id and the reason phrase, and nothing else**
  (`driver.rs:366-372`). No chat id, no person, no text. Whatever this unit names, the name is
  what appears in the log.
- **The core's observation vocabulary has three facts** — `Title`, `PinnedAnnouncement` and
  `Added { by }` (`message.rs:228-244`) — and each has a consumer. There is no neutral fact for
  "something happened in this channel that nobody needs".
- **Group facts that are recorded become context notes, which the processor receives forever
  and erasure never reaches.** The record of processing states that R1 receives "the
  conversation's text and the public username of each speaker, plus the system prompt and the
  group's context notes" (`records-of-processing.md:82`), and decision 0055 records that
  erasure does not reach context notes. A note is superseded, never removed.
- **The identity row is created on the ingestion path.** `resolve_principal` "Resolve or create
  the principal for a sender on one adapter" (`identity.rs:47-59`); the read-only lookup beside
  it exists precisely so an exempted command does not write (`identity.rs:92-97`).
- **Erasure keys on a principal and reaches messages, direct conversations and identity rows**
  (`erasure.rs:1-40`). It has no concept of a person who appears only as a name inside somebody
  else's event.
- **The suppression flag is set only by the person themselves** — the five privacy commands
  (`privacy.rs:95-105`) and the plain-language tool the conduct prompt teaches
  (`prompts/30-conduct.md:49-58`), which "acts on whoever asked and only for themselves".
- **The conduct prompt is where prose about what the assistant is already lives**
  (`prompts/30-conduct.md:1-11`), while the core's composed sections carry what depends on
  configuration — the name, the answering mode, the moderation capability (`teaching.rs:1-13`,
  `teaching.rs:90-96`). Decision 0046 settled that split.
- **A prompt edit reaches every channel at the next start.** `retire_stale_prompts`
  (`assembly.rs:852`) retires every mapped channel whose recorded system prompt differs from
  the current one, and the binary calls it during startup (`crates/assistant/src/main.rs:454`).
  So a passage added to a prompt file is not "new conversations only" in practice; it takes
  effect for every channel after the next restart, at the documented cost of that channel
  starting fresh (`assembly.rs:835-844`).
- **Prompt and document prose is pinned by a test that reads the repository's own files.**
  `crates/assistant/tests/docs.rs` reads prompts through `repo_prompt` (`:53`) and documents
  through `repo_file` (`:39`), with `flattened` (`:78`) for prose that wraps.
- **The outbound refusal is already structural, from Telegram unit 20.** That unit closes the
  callable platform surface to an enumerated `Method::ALL` list pinned by an exact-list test,
  and adds `docs/administrative-methods.txt` scanned over the adapter crate. A call-related
  method could not be reached without a deliberate, reviewed edit that fails a named test — and
  there is no such method to reach.
- **The adapter's tests drive raw update JSON through a fake platform.**
  `BotApiServer::push_update` takes a `serde_json::Value` (`tests/adapter/server.rs:150`), and
  the translation pins assert on the ledger and the persisted offset
  (`tests/adapter/translation.rs:1-9`). A notice with three named invitees can be pushed
  verbatim.

## Decisions taken with this unit

- **Nothing about a video chat is recorded, observed, projected or answered, 2026-08-27.** The
  four notices are dropped at translation. No block, no note, no observation, no principal row,
  no reply. The reasoning is not squeamishness: each recording route breaks something specific.
  A **context note** ("a video chat started here") would be sent to the processor with every
  later request and could never be erased (decision 0055) — a permanent record of one
  afternoon's activity, for no reader. A **message block** would need a speaker, so it would
  resolve or refresh an identity row for whoever pressed the button, and would store as message
  content something nobody wrote (decision 0017 records text as what is stored). The
  **invited list** would create identity rows for people who never spoke, adding a category of
  data subject the record of processing does not have: everyone in the record's §4 is there
  because they wrote something or administered a group. *Rejected:* recording the notices but
  keeping them out of the model's context — the storage is the part that carries the
  obligations, so this pays every cost and buys nothing. *Rejected:* recording only the two
  integers, `start_date` and `duration`, as "not personal data" — a call's duration in a group
  is a fact about what the members did with an evening, and stored beside the channel it is a
  fact about identifiable people. *Rejected:* a group fact carrying only "a call is scheduled
  for a time" so the assistant can answer "when is the call?" — see the scheduled-time decision
  below.
- **The refusal is named at translation, ahead of the sender and text checks, 2026-08-27.**
  `translate` recognises the four service fields immediately after the pin branch and before
  the `sender_chat` check (`translate.rs:159`), returning a new
  `Skip::VideoChatNotice(VideoChatNotice)` over a closed `Scheduled | Started | Ended |
  ParticipantsInvited`. Placement is the whole point: after this unit the refusal does not
  depend on the message having no text, on it having no sender, or on it being sent on behalf
  of a chat — three properties that other units are actively changing. *Rejected:* leaving the
  four in the generic no-content skip. That is today's state, and it is why this unit exists:
  Telegram unit 01 turns that branch into media handling, and a refusal nobody named is a
  refusal nobody will notice they removed. *Rejected:* four separate skip variants — one
  decision spelled four times. *Rejected:* one flat variant with no payload — the log could
  then not distinguish the invited-participants notice, which is the one case an auditor would
  ask about; the closed sub-enumeration costs four lines.
- **The notice is decoded as presence only: no field in this process can hold an invited
  person, 2026-08-27.** `Incoming` gains four fields typed `Option<serde::de::IgnoredAny>` —
  the deserialiser consumes the value and stores nothing — plus a small predicate that answers
  which of the four arrived. Serde treats a missing `Option` field as absent without an
  attribute, exactly as `text` and `caption` already rely on (`client.rs:135-137`). The claim
  this makes is precise and not larger than it is: the names exist in the response body while
  the JSON is parsed, because the poll decodes a whole response; after the parse there is no
  field, no variable and no log line that holds them, and nothing is written or forwarded.
  *Rejected:* decoding `users` into a typed list "in case a later unit wants it" — that is a
  retained copy of personal data about non-speakers, built for a consumer this repository has
  decided not to have. *Rejected:* a `serde_json::Value` catch-all — same objection, less
  visibly. *Rejected:* not decoding the fields at all and inferring the notice from the absence
  of everything else — that is the accidental refusal again, in a new spelling.
- **The prompt teaches one capability rule about live calls, scoped so the media work cannot
  make it false, 2026-08-27.** `prompts/30-conduct.md` gains a short passage beside the
  AI-honesty prose it matches in kind: the assistant reads and writes text messages in the
  chat; it cannot join, hear or speak in a voice or video call, cannot listen to anything live,
  and cannot see anyone's screen; asked to join a call, take notes on one, or report what was
  said in one, it says plainly that it cannot, and never claims a capability it does not have.
  The scope is deliberate. A broader sentence — "you cannot hear audio" — would be false the
  day the media work ships transcription of voice messages, and a prompt that has gone false is
  worse than no prompt, because the model has been taught to deny something it can do. The rule
  is written about *live participation*, which no roadmap in this repository changes.
  *Rejected:* naming video chats specifically, in the platform's words — the same ask arrives
  as a phone call, a screen share or a Matrix call, and one rule that covers the shape beats
  four that chase the vocabulary. *Rejected:* saying nothing and relying on the sourcing
  discipline — that discipline is about substantive claims backed by lookups
  (`teaching.rs:143-159`); "sure, I'll join" is a claim about the assistant itself, which no
  lookup covers.
- **The passage lives in the prompt file, not in the composed teaching, 2026-08-27.** Decision
  0046 keeps the maintainer's prose in the prompt files and the core's composition for what
  depends on configuration (`teaching.rs:1-13`). This passage depends on nothing configurable,
  and putting it in the core would import call vocabulary into a crate with no mechanism to go
  with it. *Rejected:* extending `identity_section` (`teaching.rs:90-96`) because it is also
  about honesty — that section exists to interpolate the configured name, and this text has no
  name in it. *Rejected:* a tool that refuses the request — a tool in the palette teaches the
  model that joining is a thing it might do, and its refusal would be a machine deciding
  something the model can simply say.
- **Nothing is added to Telegram unit 20's method refusal list, 2026-08-27.** That list names
  methods this assistant must never call. There is no video chat method to name. Adding an
  invented name would produce a check that can never fail and a reader who believes the
  platform offers something it does not. Unit 20's closed `Method::ALL` enumeration already
  makes any future call a reviewed edit. *Rejected:* adding `can_manage_video_chats` to the
  list as a capability flag — unit 09 already scans for the capability flags that matter, and
  this right unlocks no method for a bot at all.
- **The record of processing gains one sentence; nothing else in the privacy documents
  changes, 2026-08-27.** No new data is stored, none is sent to a new recipient, and no
  published statement becomes false, so no assessment is reopened. One sentence is still added,
  to §5 beside the existing "Anonymous administrator posts and automatic channel forwards are
  not stored at all" (`records-of-processing.md:75-76`): service notices about video chats,
  including the list of members invited to one, are not stored either. The record already
  answers this exact question for the other case where the platform hands over something the
  project declines; a member who was named in an invite has a plain reason to ask, and the
  answer costs one line. *Rejected:* leaving the documents untouched — defensible, since
  nothing new is processed, but it leaves the one thing a reader would look for unwritten.
  *Rejected:* a new D-row in §5 — a row describes a category of data the project holds, and
  this is a category it does not.
- **The scheduled time is not remembered, and the honest route is the pinned announcement,
  2026-08-27.** A member asking "when is the community call?" gets an "I don't know" unless the
  answer is in the group's pinned statement or the wiki. That cost is stated instead of
  designed around: `start_date` is documented as when the call "is supposed to be started by a
  chat administrator", which is a plan and not a fact, and a stored plan goes stale silently
  when the call is rescheduled or never happens. Telegram unit 11 already reads the pinned
  announcement as a group fact, so a group that wants the assistant to answer that question has
  a route that is already built, already visible to members and already documented as D4.
  *Rejected:* storing the scheduled time as a context note with a supersede on the started
  notice — machinery to keep one integer fresh, whose failure mode is the assistant confidently
  announcing a call that was cancelled.
- **A call notice draws no reply, 2026-08-27.** The assistant does not post "I can't join
  calls" when a call starts. Nobody asked it, silence is the default in helpful mode (decision
  0098), and a message on every call is noise in a community group. The refusal is spoken only
  when a member actually asks. *Rejected:* a one-time notice the first time a call starts in a
  group — a deterministic reply that answers an unasked question, and the state to make it
  once-only would be a persisted fact whose only reader is a duplicate check.

## The unit's contract

After this unit, a video chat notice of any of the four kinds is recognised for what it is at
translation, ahead of every check whose meaning other units are changing, and is dropped by
name: nothing is stored, no principal row is created, no observation reaches the core, no
context note is written, nothing is sent to the model provider, nothing is delivered to the
chat, and the poll's offset advances exactly as it does for any other skip. The invited
members' names have no field, variable or log line in this process that can hold them past the
response parse. The core gains no vocabulary and no code, the poll's subscription is unchanged,
no platform method is called and none exists to call. The conduct prompt teaches the assistant
that it cannot join, hear or speak in a live call and that it says so plainly instead of
agreeing, scoped to live participation so that transcription work cannot make the sentence
false. The record of processing states that these notices are not stored. No new dependency.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt and doc under denied
  warnings; the platform-vocabulary scan and the secret scan clean; no new dependency.
- **AC2** Each of the four notices is refused by name. Four fixture updates, one per service
  field, each in an admitted group, translate to `Skip::VideoChatNotice` carrying `Scheduled`,
  `Started`, `Ended` and `ParticipantsInvited` respectively, and each display string names the
  notice — pinned in the adapter's translation tests.
- **AC3** The refusal does not depend on the checks around it. A notice whose message also
  carries `sender_chat` is the video chat skip and **not** `Skip::OnBehalfOfChat`; a notice
  with no `from` is the video chat skip and not `Skip::NoSender`; a notice carrying a caption
  field is still the video chat skip. A notice in a channel remains `Skip::ChannelBroadcast`,
  unchanged — pinned.
- **AC4** No invited person is representable. `Incoming`'s four new fields are typed so that no
  name can be held: a `video_chat_participants_invited` update carrying three users with first
  names, last names and usernames decodes successfully, translates to the skip, and the struct
  has no field of a type that could carry them — pinned by the type and by the wire case
  together.
- **AC5** Nothing reaches storage. After the four updates are delivered to an admitted group,
  the ledger holds no block, the note table holds no new note, and the identity tables hold no
  principal for any invited user or for the acting member — pinned through the store, as the
  translation suite already asserts absence.
- **AC6** Nothing reaches the wire. The same four updates produce no `sendMessage`, no
  `sendChatAction` and no other recorded platform call beyond the poll itself — pinned against
  the fake platform's recorded calls.
- **AC7** The poll is unaffected. The four updates are acknowledged, the persisted offset
  advances past them, and a message sent after them in the same batch is recorded normally —
  pinned, so a refusal can never halt the loop.
- **AC8** The core is untouched: the unit's diff contains no change under `crates/core/src`,
  and the core's neutral vocabulary gains no fact, directive or variant for calls.
- **AC9** The model is taught. The conduct prompt carries the capability passage, it states
  that the assistant cannot join, hear or speak in a live voice or video call and that it says
  so plainly when asked, and it does not deny any capability outside live participation — the
  prose pinned in `crates/assistant/tests/docs.rs` beside the existing prompt pins, and the
  AI-honesty prose at `prompts/30-conduct.md:1-11` retained verbatim.
- **AC10** The refusal is recorded where the next implementer will look: a decision file per
  the repository's convention, dated, carrying the rejected alternatives above, numbered from
  the next free number at merge — pinned in the documentation test with the other decision
  records.
- **AC11** The record of processing carries the not-stored sentence for video chat notices,
  including the invited list, and no other privacy document changes — pinned in the
  documentation test, and checkable as an empty diff over the remaining three documents.

## Notes for launch

- Sites, from the reading above:
  - `crates/adapters/telegram/src/client.rs` — four presence-only fields on `Incoming`
    (`:125-144`) and the predicate that answers which arrived; `CONSUMED_UPDATE_TYPES`
    (`:103`) unchanged.
  - `crates/adapters/telegram/src/translate.rs` — the `Skip::VideoChatNotice(VideoChatNotice)`
    variant in the enum (`:40-77`), the recognition placed after the pin branch and before the
    `sender_chat` check (`:159`), and the four display strings (`:474-496`).
  - `crates/adapters/telegram/tests/adapter/translation.rs` — the wire pins for AC2 to AC7,
    driven through `BotApiServer::push_update` (`tests/adapter/server.rs:150`).
  - `prompts/30-conduct.md` — the capability passage, beside the AI-honesty prose it matches
    in kind (`:1-11`).
  - `docs/privacy/records-of-processing.md:75-76` — the added sentence, in the existing
    not-stored paragraph.
  - `docs/decisions/` — one record, next free number at merge; the sibling specs in this folder
    claim numbers from 0106 onward, so the implementer takes what is unclaimed and says so in
    the commit.
  - `crates/assistant/tests/docs.rs` — the prompt, record and decision pins.
- Dependencies, named instead of re-specified:
  - **Telegram unit 01** renames `Skip::NoText` to `Skip::NothingToRecord` and turns that branch
    into media handling. Whichever unit merges second moves the other's check; this unit's
    recognition must stay ahead of the media branch, and unit 01's specification is not edited
    here.
  - **Telegram unit 11** owns the pinned announcement as a group fact, which is this unit's
    answer to "when is the call?".
  - **Telegram unit 20** owns the closed callable-method enumeration and the refusal list;
    this unit adds nothing to either and says why.
  - **Telegram unit 09** owns the promotion notice. A promotion is a non-event for this
    feature: the right it would carry unlocks no method for a bot.
- The platform reading is complete and dated; do not re-derive it from a summarising fetch,
  which fabricated a field name for `VideoChatScheduled` on 2026-08-27. Read the page's own
  table.
- Nothing here touches files, media or uploads, so the streaming constraint has nothing to
  bind: the four notices are small JSON objects on the poll's existing response.
