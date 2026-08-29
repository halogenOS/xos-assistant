# Telegram unit 19 — reading and changing the chat: the two reads stay, every write is refused

Date: 2026-08-27. The Bot API's chat-administration family splits in two. One half reads a
chat — `getChat`, `getChatAdministrators`, `getChatMemberCount`, `getChatMember` — and a bot
holding no administrator right at all may make three of those four calls. The other half
changes the chat — its title, description, photo, default member permissions, sticker set,
and the whole invite-link family — and every method in it requires an administrator right
the group operator contract forbids the assistant to hold. This unit reads both halves and
answers the three questions the reading forces.

The answers go first, because two are uncomfortable and one corrects a reflex:

- **The reads stay exactly as they are.** `getChat` and `getChatAdministrators` are already
  called, already narrow, and already bounded to a chat the assistant has an identifier for.
  `getChatMemberCount` and `getChatMember` are refused, with reasons this unit adds to the
  one telegram unit 09 recorded.
- **Changing a group's title or photo is not moderation.** Decision 0070 binds effects on
  people; a title is not a person. The refusal of `setChatTitle`, `setChatDescription`,
  `setChatPhoto` and `deleteChatPhoto` therefore cannot lean on 0070, and this unit does not
  pretend it can. It rests on two reasons of its own: the group's title is already an input
  to the assistant's own system voice, so an assistant that can write the title authors the
  context it later reads as the group's fact; and the replacement text would be composed by
  a model, published under the community's name, and reverted by nobody, because nothing in
  this system remembers what the title was before.
- **An invite link is a capability that outlives the turn that made it, and the assistant
  never mints one.** Decision 0070 says a human decides who stops being a member. Creating
  an invite link is that same decision pointing inward — who becomes one — and the platform
  offers no shape in which a human sees the concrete link before it works, because it works
  the instant it exists. This unit records the extension of 0070 to admission as a decision
  of its own, so the next capability does not have to derive it again.

Two methods in the brief are already settled elsewhere and are not re-specified here.
`approveChatJoinRequest` and `declineChatJoinRequest` are refused by telegram unit 09
(`docs/units/telegram/09-chat-member-events.md`, the decision "`chat_join_request` is
refused, not deferred"). `setChatPermissions` is refused by telegram unit 20
(`docs/units/telegram/20-moderation-actions.md:255-263`), which carries it on the committed
list of methods the assistant must never call. This unit states what it learned about both
where that changes an argument, and edits neither specification.

Everything below is checked against the live Bot API page (Bot API 10.3, 24 August 2026,
fetched 2026-08-27) and against this tree. The brief named Bot API 10.1 as current; the
published page has moved two releases past it, which is why the version is stated with its
fetch date. Every claim carries its source.

## What this unit cannot promise

Four properties that look achievable and are not. They are stated before the design so no
implementer writes criteria for them.

1. **A source scan cannot prevent a capability; it makes adding one deliberate.** Somebody
   who wants `createChatInviteLink` can add the method to the client and delete the list
   entry in the same commit. What the list buys is that the deletion is visible in the diff,
   beside a committed file whose contents say why the name was there. That is the same value
   telegram units 04, 11 and 20 already buy with the same shape.
2. **The narrow decode does not stop the group's primary invite link from entering the
   process.** `ChatFullInfo.invite_link` is in the body of a call the adapter already makes,
   and `decode` reads the whole body into memory (`response.json()`,
   `crates/adapters/telegram/src/client.rs:571`) before serde drops every key `ChatInfo` does
   not name (`client.rs:189-192`). The honest property, and the one the criteria pin, is
   narrower: the link never becomes a typed value, never reaches the ledger, never reaches
   the model provider, and never reaches a chat. The transient body is handled exactly as
   every other undecoded field of every other answer already is.
3. **Nothing here detects an administrator quietly granting the assistant a right.** The
   assistant's own elevated standing is telegram unit 09's subject — it sends a fixed line
   asking to be demoted, and records the non-lossy detection as its own follow-up. This unit
   adds no detection and depends on none. Holding a right whose methods the binary never
   calls changes no behaviour, which is the reason the refusals live in the code and not in
   the configuration.
4. **The lookup's answer cannot be bounded to the fields the adapter wants.** `getChat` has
   no field selector; the platform decides what it sends. What can be bounded is what the
   process turns into values, and that is what this unit checks.

## Grounding

### What the platform actually does

**The right each method needs.** Quoted from the method descriptions on the live page. Where
a description says only "the appropriate administrator rights", the matching field of
`ChatAdministratorRights` is named in the third column and the platform's own wording for
that field is given, so the inference is visible instead of assumed.

| Method | The platform's words on the requirement | Right |
|---|---|---|
| `getChat` | "Use this method to get up-to-date information about the chat. Returns a `ChatFullInfo` object on success." No administrator sentence. | none |
| `getChatAdministrators` | "Use this method to get a list of administrators in a chat. Returns an Array of `ChatMember` objects." No administrator sentence. | none |
| `getChatMemberCount` | "Use this method to get the number of members in a chat. Returns *Integer* on success." No administrator sentence. | none |
| `getChatMember` | "The method is only guaranteed to work for other users if the bot is an administrator in the chat." | none for the bot itself; administrator for anybody else |
| `setChatTitle` | "Titles can't be changed for private chats. The bot must be an administrator in the chat for this to work and must have the appropriate administrator rights." `title`: "New chat title, 1-128 characters" | `can_change_info` — "*True*, if the user is allowed to change the chat title, photo and other settings" |
| `setChatDescription` | "Use this method to change the description of a group, a supergroup or a channel." Same administrator sentence. `description`: "New chat description, 0-255 characters", optional | `can_change_info` |
| `setChatPhoto` | "Photos can't be changed for private chats." Same administrator sentence. `photo`: `InputFile`, "New chat photo, uploaded using multipart/form-data" | `can_change_info` |
| `deleteChatPhoto` | "Photos can't be changed for private chats." Same administrator sentence. | `can_change_info` |
| `setChatPermissions` | "The bot must be an administrator in the group or a supergroup for this to work and must have the `can_restrict_members` administrator rights." | `can_restrict_members` — refused by unit 20 |
| `setChatStickerSet`, `deleteChatStickerSet` | Same administrator sentence, plus "Use the field `can_set_sticker_set` optionally returned in `getChat` requests to check if the bot can use this method." | The platform names no right. `can_set_sticker_set` is **not** a member of `ChatAdministratorRights`; it is a `ChatFullInfo` field, "*True*, if the bot can change the group sticker set". `can_change_info` is the rights field whose wording covers "other settings". |
| `exportChatInviteLink` | "generate a new primary invite link for a chat; any previously generated primary link is revoked. The bot must be an administrator in the chat for this to work and must have the appropriate administrator rights." | `can_invite_users` — "*True*, if the user is allowed to invite new users to the chat" |
| `createChatInviteLink` | "create an additional invite link for a chat." Same administrator sentence. `name` 0-32 characters; `expire_date` optional; `member_limit` "1-99999"; `creates_join_request` "If *True*, `member_limit` can't be specified." | `can_invite_users` |
| `editChatInviteLink` | "edit a non-primary invite link **created by the bot**." Same administrator sentence. | `can_invite_users` |
| `revokeChatInviteLink` | "revoke an invite link created by the bot. If the primary link is revoked, a new link is automatically generated." Same administrator sentence. | `can_invite_users` |
| `createChatSubscriptionInviteLink` | "create a subscription invite link **for a channel chat**. The bot must have the `can_invite_users` administrator rights." `subscription_period` "Currently, it must always be 2592000 (30 days)."; `subscription_price` "The amount of Telegram Stars a user must pay initially and after each subsequent subscription period to be a member of the chat; 1-10000" | `can_invite_users`, channels only |
| `editChatSubscriptionInviteLink` | "edit a subscription invite link created by the bot. The bot must have the `can_invite_users` administrator rights." | `can_invite_users` |
| `approveChatJoinRequest`, `declineChatJoinRequest` | "The bot must be an administrator in the chat for this to work and must have the `can_invite_users` administrator right." | `can_invite_users` — refused by unit 09 |

**What a bot with no right at all can still read.** Three of the four reads carry no
administrator sentence, so a plain member bot may call them: the chat's full information, the
administrator list, and the member count. `getChat` answers with `ChatFullInfo`, and exactly
three of its fields state a restriction in their own description —
`has_aggressive_anti_spam_enabled` and `guard_bot` ("The field is only available to chat
administrators") and `has_visible_history` ("available only to chat administrators").
`invite_link` ("Primary invite link, for groups, supergroups and channel chats"),
`description`, `permissions`, `sticker_set_name`, `can_set_sticker_set`, `linked_chat_id` and
`slow_mode_delay` state no restriction, so a design must assume a non-administrator bot may
receive all of them. The one read that is genuinely closed is per-person: `getChatMember` is
"only guaranteed to work for other users if the bot is an administrator".

**The administrator list already includes the assistant itself.** `getChatAdministrators`'s
`return_bots` parameter, verbatim: "Pass *True* to additionally receive all bots that are
administrators of the chat. **By default, bots other than the current bot are omitted.**" So
the default answer names the calling bot whenever it is an administrator, and names no other
bot. This is the sentence telegram unit 09 relies on for its follow-up, re-verified here.

**A bot can only work with its own invite links.** The Note under `exportChatInviteLink`,
verbatim: "Each administrator in a chat generates their own invite links. Bots can't use
invite links generated by other administrators. If you want your bot to work with invite
links, it will need to generate its own link using `exportChatInviteLink` or by calling the
`getChat` method. If your bot needs to generate a new primary invite link replacing its
previous one, use `exportChatInviteLink` again." `editChatInviteLink` and
`revokeChatInviteLink` both say "created by the bot" in their first sentence. So even a
promoted assistant could not revoke a link an administrator made; it could only make and
unmake its own.

**`exportChatInviteLink`'s destructiveness is ambiguous in the platform's own words.** The
method says "any previously generated primary link is revoked"; the Note's last sentence
says "a new primary invite link replacing its previous one", which reads per-bot. Both
readings are available on the page and the page settles neither. The wider reading — the
chat's published primary link stops working — is the one a design has to survive, because
the harm if it is the true one is a dead link on a project website and no signal inside this
system that anything happened. Nothing below depends on which reading is right.

**`setChatPermissions` takes the whole set, never a delta.** The parameter is "A
JSON-serialized object for new default chat permissions", and no partial form exists.
`ChatPermissions` carries sixteen optional booleans — `can_send_messages`,
`can_send_audios`, `can_send_documents`, `can_send_photos`, `can_send_videos`,
`can_send_video_notes`, `can_send_voice_notes`, `can_send_polls`, `can_send_other_messages`,
`can_add_web_page_previews`, `can_react_to_messages`, `can_edit_tag`, `can_change_info`,
`can_invite_users`, `can_pin_messages`, `can_manage_topics` — and
`use_independent_chat_permissions` only decides which of them imply which others *within the
object that was passed*. A call naming three fields therefore states a whole permission set
in which the other thirteen are absent: the group's ordinary members lose whatever the caller
forgot to mention. Unit 20 refuses the method; this paragraph exists because the whole-set
shape is a fact about this family that the read side has to know as well.

**The subscription link charges the joiner.** `createChatSubscriptionInviteLink` mints a link
whose holder pays 1 to 10000 Telegram Stars for each 30-day period. It is channel-only, so it
cannot apply to a community supergroup at all — both facts are recorded, because the second
alone would be mistaken for the reason.

**Updates expire after 24 hours.** "Incoming updates are stored on the server until the bot
receives them either way, but they will not be kept longer than 24 hours." The same sentence
unit 09 relies on; it bounds what any event-shaped detection can promise, including the
promotion notice this unit depends on and does not own.

**Bot API 10.3 added one right to this neighbourhood and no method.**
`can_send_welcome_messages` joined `ChatAdministratorRights` and `ChatMemberAdministrator` —
"*True*, if the administrator can manage chat welcome messages or directly send them in the
case of bots" — and `promoteChatMember` gained the matching parameter. No method in the
current API is named for sending one, so there is nothing here to refuse; the right is
recorded so the operator contract's list of rights is complete on the day it is written.

### What already exists in this tree

- **`getChat` is called for group chats only, lazily, once per chat per process.**
  `first_contact` (`crates/adapters/telegram/src/driver.rs:532-566`) skips on the per-chat
  memory (`driver.rs:539-541`), calls `client.get_chat` (`client.rs:334-340`) and reports
  what translation makes of the answer (`driver.rs:550-563`). The message path enters it only
  for a group (`driver.rs:377-385`), the pin path at title-only scope (`driver.rs:490-501`),
  and the admitted entry at whole scope (`driver.rs:519`). Every observation this adapter
  builds is a group one (`translate.rs:131`, `:155`, `:215`, `:263`, `:278`), so no private
  chat is ever looked up and none of `ChatFullInfo`'s private-chat fields — `bio`,
  `birthdate` and the rest — is ever in an answer this process receives.
- **The lookup addresses a chat by number, never by name.** `get_chat` builds
  `{"chat_id": chat_id}` from an `i64` (`client.rs:334-335`), and every caller passes an
  identifier that came out of an update. The platform's `chat_id` also accepts "@username of
  the target supergroup or channel", so the call *could* read a public chat the assistant has
  never been in; the adapter's signature makes that unreachable. Nothing in the tree says so
  yet.
- **The answer is decoded into two fields.** `ChatInfo { title, pinned_message }`
  (`client.rs:185-192`) with `PinnedContent { date, text, caption }` (`client.rs:151-160`).
  Serde ignores unknown keys, so `invite_link`, `description`, `permissions`,
  `sticker_set_name` and the rest are dropped at the decoder. Nothing in the tree asserts
  that this stays true.
- **The lookup yields at most two neutral facts.** `lookup_observations`
  (`translate.rs:254-283`) produces `ObservedFact::Title` and, at `LookupScope::Whole`
  (`translate.rs:243-248`), `ObservedFact::PinnedAnnouncement` when the pin is accessible and
  carries text.
- **The title becomes a line in the model's system voice.** `note_of` turns
  `ObservedFact::Title` into a `NoteTopic::Title` note (`crates/core/src/note.rs:154-171`),
  refused whole above `TITLE_TEXT_MAX_BYTES` = 512 (`note.rs:60-65`), projected to the model
  as `TITLE_NOTE_LEAD` — "The group's title is now: " — (`note.rs:42`, `:217-223`), appended
  on-delta under the stamp lock (`crates/core/src/assembly.rs:965-1008`). The group's title
  is an input the assistant reads about itself, on the same seam as the rules.
- **Authority comes from the administrator list, cached per chat for one minute.**
  `AdminCache::authority_for` (`crates/adapters/telegram/src/authority.rs:46-85`) calls
  `chat_administrators` (`client.rs:464-471`), maps "creator" to `Authority::Admin` and
  "administrator" to `Authority::Moderator` (`authority.rs:60-70`), and treats absence as
  `Authority::Member` (`authority.rs:79-84`); `ADMIN_CACHE_TTL` is one minute
  (`authority.rs:19`). The module's own documentation states that a failed fetch is the
  caller's transient failure and authority is never defaulted (`authority.rs:6-7`). The
  decoded entry is `ChatMember { user, status }` (`client.rs:243-246`) over
  `MemberUser { id }` (`client.rs:250-252`) — no name, no title, no rights.
- **One function reaches the platform.** `post` interpolates the method name into the
  token-bearing URL (`client.rs:532-545`), and the client exposes seven calls: `get_me`
  (`client.rs:304`), `get_updates` (`:313`), `get_chat` (`:334`), `leave_chat` (`:348`),
  `send_message` (`:371`), `send_chat_action` (`:401`), `chat_administrators` (`:464`).
  Telegram unit 20 pins that set as a closed enumeration
  (`docs/units/telegram/20-moderation-actions.md:441`); this unit does not restate the pin.
- **A committed list of methods the assistant must never call is already specified.**
  Telegram unit 20 creates `docs/administrative-methods.txt` with nine names — the seven
  standing moderation methods plus `setChatPermissions` and `setChatMemberTag` — scanned over
  the adapter crate's `src` and `tests`, failing with `file:line`, with the list living
  outside the scanned files so the needles do not match themselves
  (`20-moderation-actions.md:255-284`). It states the merge rule for unit 09's join-request
  names on the same file. Its rejected alternatives include putting method names on
  `docs/platform-vocabulary.txt`, "that list is scanned over `crates/core` only … where the
  names cannot appear anyway".
- **The core cannot ask the adapter for anything.** Decision 0054 rejects "a core-to-adapter
  query surface — a new boundary for what a push solves"
  (`docs/decisions/0054-observations-may-open-a-conversation-a-lookup-feeds-them.md`), and
  every group fact the core holds arrived as a pushed `Observation`. A tool answering "how
  many members are in this group?" needs exactly that boundary, plus platform vocabulary in a
  core tool module.
- **The forbidden-word scan is whole-word over alphanumeric runs.** `carries_word`
  (`crates/core/tests/vocabulary.rs:64-67`) splits each line on every non-alphanumeric
  character and compares tokens case-insensitively; the scan reads the core crate's `src`,
  `tests` and manifest (`vocabulary.rs:35-46`, `:69-90`). The list it reads is documented as
  the platform names and SDK crate names an adapter contributes
  (`docs/platform-vocabulary.txt`, header).
- **None of the nineteen method names in the table above appears anywhere in `crates/`
  today.** Verified 2026-08-27 by a case-insensitive grep over the workspace for
  `setChatTitle`, `setChatDescription`, `setChatPhoto`, `deleteChatPhoto`,
  `setChatStickerSet`, `deleteChatStickerSet`, every `InviteLink` name, `getChatMember` and
  `invite_link`: no hit in any crate.
- **The operator contract requires an ordinary member.** Requirement 3: "**The assistant is
  NOT a group administrator.** The moderation bot ignores administrators' reports, so an
  administrator assistant files into silence. Keep the assistant an ordinary member and turn
  its privacy mode off instead of promoting it."
  (`docs/reference/group-operator-contract.md:112-115`.)
- **The privacy documents state what a group fact is and who receives it.** D4 is "Group
  facts — Channel title, pinned rules text, stored as context notes"
  (`docs/privacy/records-of-processing.md:64`); D5 is derived state including group
  authorization (`:65`); S1 is members whose messages are stored, "Includes members who never
  address the assistant" (`:50`); R1 receives "the conversation's text and the public username
  of each speaker, plus the system prompt and the group's context notes" (`:82`), and R2 is
  the sub-processor layer where the zero-retention promise ends (`:83`). The impact
  assessment closes its necessity section "Nothing is collected for a purpose beyond the three
  named" (`docs/privacy/dpia.md:340`) and lists among its review triggers "Any capability that
  touches a person's standing in the group" (`dpia.md:578`). The legitimate-interest
  assessment has a section "What is not necessary, and therefore not done" (`lia.md:106`).
  Decision 0055 keeps erasure away from context notes.
- **Neighbouring specifications this unit depends on and does not repeat.** Unit 09 owns the
  join-request refusal and the assistant's own-standing detection. Unit 11 refuses the
  pin-writing methods and adds a six-hourly re-read of the chat's facts on the group's own
  activity (`11-pinning.md:296-300`), so `getChat` stops being once-per-process; this unit's
  decode pin holds under that change unaltered. Unit 20 owns the moderation methods, the
  closed client enumeration and the committed list file. Unit 02 owns the streaming shape for
  outbound media.

## Decisions taken with this unit

- **The two reads stay exactly as they are, and no third is added, 2026-08-27.** `get_chat`
  and `chat_administrators` keep their bodies and their call sites. No parameter is added to
  either; in particular `return_bots` stays unset, because the platform already includes the
  current bot by default and setting the flag would pull other bots' identifiers into the
  cache for no reader.
  *Rejected:* fetching every admitted group's chat at startup. It calls into groups that are
  not talking — the shape decision 0054 and unit 11 both refused — and the authorization
  table is not a list of chats the assistant is currently in.
  *Rejected:* changing `ADMIN_CACHE_TTL` while reading the surrounding methods. It is a
  shipped constant with a stated reason (`authority.rs:16-19`), nothing here learned anything
  about it, and editing a live constant inside a refusal unit is unexamined drift.

- **The chat lookup is bounded to chats the assistant has an identifier for, and that becomes
  a stated property, 2026-08-27.** `get_chat` keeps its `i64` parameter and gains one
  sentence of documentation saying why the type is the bound: the platform's `chat_id`
  accepts a public `@username`, so a `String` parameter would let one careless caller read a
  community the assistant has never been in. The signature is the check; the sentence is what
  stops somebody widening it for convenience.
  *Rejected:* a runtime check that the identifier belongs to a known chat. It re-decides at
  run time what the type already decides at compile time, and the adapter has no list of
  known chats to check against — the core holds the authorizations.
  *Rejected:* leaving it undocumented because the type already holds. The next person adding
  a lookup by username would be adding a parameter, not removing a check, and would meet no
  reason not to.

- **`getChatMemberCount` is refused a second time, and the second reason is the ledger's,
  2026-08-27.** Unit 09 already rejected it: permissible, not personal data, and no purpose
  asks for it. The reason this unit adds binds the storage side. A member count changes
  several times a day in a live community, and the only place a group fact can live is a
  context note appended on-delta under the stamp lock (`assembly.rs:965-1008`). Storing it
  would append a superseding note on most lookups, so the ledger would accumulate a stream of
  stale counts that erasure does not reach (decision 0055), and the model's system voice
  would carry a number already wrong when it is read. A fact whose truth expires faster than
  the record holding it does not belong in an append-only record.
  *Rejected:* reading the count without storing it, for a log line. A platform call for an
  operator-facing curiosity, and the operator has the group open in front of them.
  *Rejected:* reading it once at first contact as a one-off note. The first day's number,
  kept forever, is worse than no number.

- **`getChatMember` is not added, for a third party or for the assistant itself, 2026-08-27.**
  For a third party the platform gives no answer a non-administrator can rely on, and the
  answer it would give an administrator is a person's standing, their custom title or tag,
  their restriction expiry and eleven per-person permission booleans — a per-person profile
  read about somebody who may never have addressed the assistant. Authority resolution needs
  none of it: the administrator list answers the only question the core asks, for every
  sender, in one cached call per chat per minute, and the decoded entry carries an
  identifier and a status string and nothing else (`client.rs:243-252`).
  For the assistant itself the call would work, and it is a real way to detect an unnoticed
  promotion — but that detection is unit 09's recorded follow-up, and resolving it means
  deciding in the core how often a standing read may speak. This unit does not resolve
  another unit's follow-up in passing.
  *Rejected:* replacing the administrator-list cache with a per-sender `getChatMember`. One
  call per distinct sender instead of one per chat per minute, against a rate-limited API,
  for an answer the platform does not guarantee to a non-administrator.
  *Rejected:* calling it for the bot's own identifier on every cache refill. That is the
  follow-up's question with its repetition policy left unanswered, which is why unit 09 did
  not build it.
  *Rejected:* reading `ChatFullInfo.can_set_sticker_set` from the lookup as a cheap promotion
  signal. It arrives free in an answer the adapter already receives, and it is false for an
  ordinary member — but it states whether the bot may change the sticker set, not whether the
  bot is an administrator, and making one field stand for a fact it does not state is how a
  check quietly becomes wrong when the platform re-scopes the field.

- **The narrow decode becomes a checked property instead of a habit, 2026-08-27.** `ChatInfo`
  keeps its two fields and gains a documentation line saying the narrowness is deliberate and
  checked. A test decodes a full `ChatFullInfo` payload — carrying `invite_link`,
  `description`, `permissions`, `sticker_set_name`, `can_set_sticker_set`, `linked_chat_id`,
  `slow_mode_delay`, `bio` and `guard_bot` with distinctive values — and asserts that the
  decoded value's `Debug` output contains none of them. This is the shape unit 09 used to pin
  that a service message's member list never enters the process, and it is the one mechanism
  here that catches the likely accident: somebody adding `description` to `ChatInfo` next
  year because it was already in the answer.
  *Rejected:* `#[serde(deny_unknown_fields)]` on `ChatInfo`. It inverts the failure
  direction — the next field the platform adds to `ChatFullInfo` would break every lookup in
  production — and lenient decoding is a stated property of this adapter (`client.rs:105-107`,
  `:146-150`, `:162-164`).
  *Rejected:* asserting the narrowness in prose and trusting review. That is the state the
  tree is in today, and it is why this decision exists.

- **The assistant never changes the group's title, description or photo — and the reason is
  not decision 0070, 2026-08-27.** `setChatTitle`, `setChatDescription`, `setChatPhoto` and
  `deleteChatPhoto` are not added to the client and gain no core vocabulary. The non-reason
  goes first, because a refusal resting on the wrong rule falls over when somebody checks it:
  a title is not a person, changing it touches nobody's standing, and 0070 does not reach it.
  An administrator who typed a command asking for a rename would even satisfy 0070's human
  decision point. The refusal rests on three reasons of its own.
  First, the right. All four need `can_change_info`, and the operator contract requires the
  assistant to stay an ordinary member (`group-operator-contract.md:112-115`), so every call
  is a refusal in the shipping configuration.
  Second, and the reason that survives a promotion: the group's title is already an input to
  the assistant's own system voice (`note.rs:154-171`, `:217-223`), reported to the core as
  an observed fact by the same lookup. An assistant that can set the title writes the note it
  later reads as the group's own fact. That is the self-authoring loop unit 11 refused for the
  pin and decision 0049 named as the trust boundary, one field over.
  Third, the composition. The text would come from a model: unbounded prose squeezed into
  1-128 characters, published under the community's name on every member's chat list, and
  reverted by nobody, because nothing in this system remembers the previous title. The chat
  has no undo and the ledger has no reverse.
  *Rejected:* the methods behind an administrator check, so a promoted assistant gains them.
  It trades the report path for a capability nobody asked for and puts the self-authoring
  loop one configuration change away.
  *Rejected:* an administrator-commanded rename, where a human types the new title and the
  assistant performs it. 0070 is satisfied and the model composes nothing, so this is the
  strongest form of the idea. It still needs `can_change_info`, it still closes the
  self-authoring loop the moment the right is held, and the administrator who typed the title
  can set it themselves in two taps. The assistant would add a round trip and a permission.
  *Rejected:* the description alone, on the argument that no note reads it. Same right, and
  the next decision says why the description stays out of the system voice in both
  directions.

- **The group's description is not read into a context note either, 2026-08-27.** The lookup
  keeps reporting the title and the exposed pinned announcement and nothing else. The
  description is tempting: public group-authored text, often carrying house rules, arriving
  free in an answer the adapter already receives. Refused, because the rules contract has one
  source by decision 0048 and the pin right is the trust boundary by decision 0049. A second
  source of governing text, writable by anybody holding `can_change_info` instead of
  `can_pin_messages`, would put two differently-permissioned surfaces into one system voice,
  and the assistant would judge messages against a blend of them. One decision, recorded
  once.
  *Rejected:* the description as a plain informational note, distinct from rules. Still
  unbounded member-authored text in the model's system voice, still a new `NoteTopic` variant
  and a migration of the topic vocabulary (`note.rs:67-98`), and no recorded purpose asks for
  it.
  *Rejected:* reading it only when no rules pin exists. A conditional source of governing
  text, where what governs depends on what is absent, is the shape nobody can reason about
  later.

- **The sticker-set methods are refused, 2026-08-27.** `setChatStickerSet` and
  `deleteChatStickerSet` need administrator standing, and the platform points at
  `ChatFullInfo.can_set_sticker_set` as the check. A group's sticker set is a shared surface
  every member sees; no purpose asks for it; and unit 14 already settles that the assistant
  sends no stickers. Choosing the group's sticker set is the larger version of the same taste
  decision, made for everybody at once.
  *Rejected:* reading `sticker_set_name` from the lookup so the model can mention the group's
  sticker set. A fact nothing answers with, in a note that lives forever.

- **The invite-link family is refused, and decision 0070's principle is extended to admission
  to say why, 2026-08-27.** None of `exportChatInviteLink`, `createChatInviteLink`,
  `editChatInviteLink`, `revokeChatInviteLink`, `createChatSubscriptionInviteLink` or
  `editChatSubscriptionInviteLink` is added to the client, and the core gains no vocabulary
  that could ask for one. Four reasons, in the order they survive changes to the deployment.
  First, the reading of 0070 this unit records as a decision of its own. 0070 binds "every
  path that could touch a person's standing" and names warns, bans and mutes — every example
  points outward. An invite link points inward: it decides that somebody who is not a member
  may become one. The community's composition is the same property in both directions, and a
  mechanism letting a machine widen it without a human seeing the concrete act is the same
  defect as one letting a machine narrow it. Unit 09 reached this conclusion for join
  requests, in the words the platform forced on it; this unit writes it down as the general
  rule so the next capability does not have to re-derive it.
  Second, the capability outlives the turn. A created link has no expiry unless one is
  passed and no member limit unless one is passed; once it exists it can be forwarded,
  screenshotted or published where this system will never see it.
  `ChatInviteLink.pending_join_request_count` is the only feedback the API offers, and only
  for links the bot itself made. Revocation is a separate deliberate act by somebody who
  remembers the link exists. A turn ends; the link does not. There is no shape in which a
  human reviews the link before it works, because it works the instant it is created — which
  is exactly what "a human decides" cannot be reduced to.
  Third, the platform's own destructiveness, on the reading a design has to survive.
  `exportChatInviteLink` revokes the previously generated primary link and the page settles
  neither reading of "previously generated". One call, from a model that decided somebody
  "needed an invite", can on the wider reading break the link the project publishes on its
  website, with no signal inside this system that anything happened.
  Fourth, the subscription variants create a payment obligation for whoever joins — 1 to
  10000 Stars per 30-day period — which is the assistant charging a stranger for entry. They
  are channel-only, so they cannot apply to the community supergroup at all; both facts are
  stated, because the second alone would be mistaken for the reason.
  *Rejected:* a single-use, short-expiry link on an administrator's explicit command. It
  satisfies 0070's human decision point and is the strongest form of the idea. It still needs
  `can_invite_users`, which the operator contract forbids; the administrator issuing the
  command already holds the right and can make the link in their own client; and a link the
  assistant made is a link the assistant is answerable for, in a system that keeps no record
  of it surviving the process.
  *Rejected:* revoking a link on a report of abuse. `revokeChatInviteLink` works only on
  links the bot created, so the assistant could revoke nothing it had not first minted — the
  capability is useless without the one refused hardest.
  *Rejected:* recording the refusal in prose alone. The list entry makes it checkable, which
  is the difference between a decision and an intention.

- **The chat's primary invite link is never decoded, never stored and never spoken,
  2026-08-27.** `ChatFullInfo.invite_link` arrives in an answer the adapter already receives,
  and `ChatInfo` will not name it. Reading it needs no right at all, which is precisely why
  the refusal must be written down instead of inferred from the permission table. Three
  consequences, if it were read.
  It is a bearer capability: anybody holding the string can enter the community. Putting it
  in a context note sends it to R1 with every request and onward to R2's model-provider
  layer, where the zero-retention promise ends (`records-of-processing.md:82-83`). An access
  credential for a community would leave the deployment's control as ordinary prose.
  It cannot be un-said. Notes live on the append-only ledger, a newer note supersedes an
  older one without removing it, and erasure does not reach context notes (decision 0055). A
  link revoked tomorrow stays in the record and in the model's projected history.
  It would be quotable. The model composes text, and a note in its system voice is material
  it may repeat. An invite link in the context is one plausible turn away from being handed
  to whoever asked, including somebody the administrators removed an hour earlier.
  *Rejected:* storing the link with a "never repeat this" instruction in the teaching. A
  prompt is advice to a model, not a bound on a system — 0070's own rejected alternative, in
  its own words.
  *Rejected:* answering "how do I invite someone?" from the platform at all. Substantive
  answers come from tool lookups by decision 0096; if the project publishes an invite route
  it is on the wiki, and the wiki lookup already finds it. That is the neutral answer and it
  needs no new capability.

- **The refused names go on unit 20's committed list, and this unit creates no second list,
  2026-08-27.** `docs/administrative-methods.txt` already exists in specification
  (`20-moderation-actions.md:255-284`) with the same purpose, the same matching rule and the
  same reason. This unit contributes fourteen names — `setChatTitle`, `setChatDescription`,
  `setChatPhoto`, `deleteChatPhoto`, `setChatStickerSet`, `deleteChatStickerSet`,
  `exportChatInviteLink`, `createChatInviteLink`, `editChatInviteLink`,
  `revokeChatInviteLink`, `createChatSubscriptionInviteLink`,
  `editChatSubscriptionInviteLink`, `getChatMemberCount`, `getChatMember` — each with a
  comment naming this unit as the refusing one. `setChatPermissions` is not among them: unit
  20 contributes it. `getChat` and `getChatAdministrators` are not on the list and the test
  asserts their absence, so nobody breaks the two calls the adapter needs by adding a name
  that reads like the others.
  Matching is whole-word over runs of letters and digits, the rule `carries_word` already
  uses (`vocabulary.rs:64-67`), for a specific reason: a substring rule would make
  `getChatMember` match `getChatMemberCount` and would make any future `getChat` entry match
  the two calls that must keep being made.
  On merge order: whichever of units 09, 19 and 20 merges first creates the file; the later
  ones add their names and delete any scan of their own in the same commit. No unit's
  specification is edited to arrange this.
  *Rejected:* a second file, `docs/telegram-refused-methods.txt`, owned by this unit. Two
  lists of platform methods the assistant must never call is one decision recorded twice, and
  the second copy is the one that gets forgotten.
  *Rejected:* adding the fourteen names to `docs/platform-vocabulary.txt`. That file is
  documented as the platform names and SDK crate names an adapter contributes, its scan reads
  `crates/core` only (`vocabulary.rs:35-46`), and the names cannot appear there anyway — unit
  20 rejected the same idea for the same reason, and two units agreeing is not a reason to do
  it twice.
  *Rejected:* re-pinning the client's closed method enumeration here. Unit 20 pins it
  (`20-moderation-actions.md:441`); a second assertion of the same set is a second place to
  update when a legitimate method is added.

- **Nothing here moves bytes, and the one method that would is refused, 2026-08-27.**
  `setChatPhoto` is the only member of this family carrying a file, and the platform accepts
  it only as `InputFile` "uploaded using multipart/form-data" — no URL form, no `file_id`
  form. Since the method is refused, this unit adds no upload path and the standing streaming
  constraint has nothing to bind. Recorded with what it would take if a later unit reversed
  the refusal: the image would have to stream from the framework's attachment store into a
  multipart field chunk by chunk, on the shape unit 02 owns for outbound media, never
  assembling in memory. The one whole-buffer this unit touches is existing and bounded — the
  JSON answer body at `client.rs:571` — and it is a lookup answer, not media.

- **No privacy document changes, 2026-08-27, and here is the reasoning to re-check if the
  design moves.** No new category of data is stored: the lookup keeps reporting the title and
  the exposed pin, which is D4 as written (`records-of-processing.md:64`). No new recipient:
  nothing new reaches R1 or R2, because nothing new is decoded. No new data subject: no
  per-person read is added, so nobody enters the record who was not already there through
  their own messages. No new purpose. Each refused capability is recorded with what it would
  have cost: a member count or a permissions snapshot would extend D4 and need a necessity
  test against "What is not necessary, and therefore not done" (`lia.md:106`); a
  `getChatMember` read would create a new data-subject category and fire the impact
  assessment's standing trigger (`dpia.md:578`); an invite link in a note would put an access
  credential into R1's and R2's hands and into an append-only record erasure does not reach.
  *Rejected:* shipping first and amending the documents afterwards. A published statement
  made false by a merge is a defect in that merge.

- **The operator contract names the rights the assistant must never be granted, 2026-08-27.**
  Requirement 3 says "not a group administrator" and gives the report-path reason
  (`group-operator-contract.md:112-115`). It gains a short paragraph naming the rights this
  family turns on — `can_change_info`, `can_invite_users`, `can_restrict_members`, and
  `can_send_welcome_messages`, added in Bot API 10.3 — saying that the assistant calls none
  of the methods they unlock, that granting one therefore changes no behaviour, and that the
  only observable effect of promoting the assistant is the demotion request unit 09 sends.
  Documentation, not code.
  *Rejected:* a configuration key listing permitted capabilities. An operator control for a
  set of methods absent from the binary, whose only honest value is the empty one.

## The unit's contract

The assistant reads a group's title and its exposed pinned announcement, and reads the chat's
administrator list to resolve a sender's standing. It reads nothing else about a chat and
nobody's membership individually: `getChatMemberCount` and `getChatMember` are not called,
the lookup addresses a chat only by an identifier that arrived in an update, and the answer
is decoded into two fields — checked by a test that feeds a full platform payload through
the decoder and shows that the group's primary invite link, its description, its default
permissions, its sticker set and its slow-mode delay never become typed values. It changes
nothing about a chat. The title, description, photo and sticker set are not writable by this
assistant in any configuration: the methods are absent from the client, absent from the
core's vocabulary, and named on the committed list of methods a scan over the adapter crate
checks — the title and photo because the group's identity is not the machine's to compose and
the title is an input to the assistant's own system voice, the sticker set because a shared
surface chosen for everybody is nobody's to choose here. No invite link is created, edited,
revoked, exported, subscribed or read: minting one decides who may enter a community, which
is decision 0070's principle pointing inward, and a link keeps working long after the turn
that made it has ended. The default member permissions stay unit 20's refusal and the join
requests stay unit 09's. Nothing new is stored, nothing new is sent to the model provider,
no privacy document changes, and no new dependency is added.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt and doc under denied
  warnings; the platform-vocabulary scan and the secret scan clean; no new dependency and no
  schema migration.
- **AC2** The fourteen names this unit refuses are on `docs/administrative-methods.txt`, each
  with a comment naming this unit, and the scan over the adapter crate's `src` and `tests`
  finds none of them. `setChatPermissions` is on the file under unit 20's name, not this
  unit's, and appears exactly once. `getChat` and `getChatAdministrators` are absent from the
  file, asserted by the test, so the two calls the adapter needs cannot be refused by a later
  careless edit.
- **AC3** The scan is provably able to fail: a fixture string containing one of the fourteen
  names is detected by the same matcher the test uses, and a fixture containing
  `getChatMemberCounted` is not — the whole-word rule, pinned in both directions. If unit 20
  merged first, this criterion re-runs its test unchanged with the longer list; if this unit
  merges first, it creates the file and the test in unit 20's specified shape.
- **AC4** The chat lookup's decode stays narrow, pinned against a payload and not against a
  claim: a `ChatFullInfo` JSON fixture carrying distinctive values for `invite_link`,
  `description`, `permissions`, `sticker_set_name`, `can_set_sticker_set`, `linked_chat_id`,
  `slow_mode_delay`, `bio` and `guard_bot`, plus a title and a pinned message, decodes into
  `ChatInfo`; the title and the pinned message decode correctly; and the decoded value's
  `Debug` output contains none of the nine distinctive values. The criterion states in the
  test's own words that it claims nothing about the raw response body, which `decode` reads
  whole (`client.rs:571`).
- **AC5** The lookup still yields at most the two facts: the same payload through
  `lookup_observations` at `LookupScope::Whole` produces exactly one `Title` observation and
  one `PinnedAnnouncement` observation, and nothing derived from the description, the
  permissions, the sticker set or the invite link.
- **AC6** `get_chat` still takes an `i64` and its documentation says why. A reviewer confirms
  no call site passes a name; the type carries the check and the criterion records that the
  documentation sentence merged with it.
- **AC7** No new platform call is made on any path: an adapter run over a group entry, a
  first message, a pin event and a promotion, against the fake server, records requests whose
  method names are all drawn from the seven the client exposes. The harness gains a reader
  for the whole recorded list beside `recorded` (`tests/adapter/server.rs:249-258`) so the
  assertion is over every request made, not over a list of names somebody remembered to
  check.
- **AC8** Authority resolution is untouched: the existing administrator-cache tests pass
  unchanged, `return_bots` appears in no request body — asserted against the recorded
  `getChatAdministrators` bodies — and the one-minute cache lifetime is the shipped constant.
- **AC9** The operator contract carries the new paragraph naming the four rights, stating
  that the assistant calls none of the methods they unlock and that granting one changes no
  behaviour. Read against the merged document, and consistent with requirement 3 instead of
  replacing it.
- **AC10** A decision record is written for the extension of 0070's principle to admission —
  that deciding who may enter a community is the same class of power as deciding who may
  stay, and that an invite link is refused on that basis as well as on the administrator
  right. It cites 0070, cites unit 09's join-request refusal as the case that forced the
  question, and carries the rejected alternatives from the decision above.
- **AC11** The privacy documents are unchanged by this unit — `git diff docs/privacy/` is
  empty — and four named statements are re-read against the merged code and remain true: D4's
  contents (`records-of-processing.md:64`), R1's "what it receives" (`:82`), "Nothing is
  collected for a purpose beyond the three named" (`dpia.md:340`), and "What is not
  necessary, and therefore not done" (`lia.md:106`). Recorded in the merge as a checked fact
  naming those four lines, not as an assumption.

## Notes for launch

- Branches from `main` into its own worktree. The unit is self-contained in the consumer
  repository and needs no framework change. It adds no runtime behaviour: what ships is two
  tests, fourteen list entries, two documentation sentences, a contract paragraph and a
  decision record. Expect a small diff and treat the reasoning as the deliverable.
- Adapter sites, all unchanged in behaviour: `crates/adapters/telegram/src/client.rs:185-192`
  (`ChatInfo`, gaining the sentence that its narrowness is deliberate and checked, pinned by
  AC4), `client.rs:334-340` (`get_chat`, gaining the sentence about the numeric bound, pinned
  by AC6), `client.rs:464-471` (`chat_administrators`, pinned by AC8),
  `crates/adapters/telegram/src/translate.rs:254-283` (`lookup_observations`, pinned by AC5),
  `crates/adapters/telegram/src/authority.rs:46-85` (pinned by AC8). New files are tests
  only, under `crates/adapters/telegram/tests/`, plus the harness accessor named in AC7.
- Core sites: none. `crates/core/src/note.rs`, `message.rs` and `assembly.rs` are untouched,
  and `docs/platform-vocabulary.txt` is untouched as well — the decision above says why.
- Documents: `docs/administrative-methods.txt` (created by whichever of units 09, 19 and 20
  merges first, extended by the rest), `docs/reference/group-operator-contract.md` (the
  paragraph after requirement 3), and one decision record in `docs/decisions` taking the next
  free number at merge time. Several units in flight already claim numbers from 0106, so the
  number is chosen when this merges and is not reserved here.
- Order against the neighbouring units: this unit assumes unit 20's list file and its closed
  client enumeration instead of restating either. If unit 20's shape changes in review, this
  unit's AC2, AC3 and AC7 follow it; they are written to read the file and the recorded
  requests, not to duplicate the enumeration.
- Unit 11 turns the once-per-process lookup into a six-hourly re-read
  (`11-pinning.md:296-300`). Nothing in this unit assumes once-per-process: AC4 and AC5 are
  decode and translation pins, and AC7 asserts which methods are called, not how often.
- Do not edit telegram units 09, 11 or 20. This unit disagrees with none of them today: 09
  owns the join-request refusal and the own-standing follow-up, 11 owns the pin refusal and
  the re-read, 20 owns the moderation methods, the enumeration and the list file, and this
  unit's arguments are the same arguments applied to different methods.
- One judgement recorded for the implementer instead of left to be rediscovered: if a
  reviewer argues that an administrator-commanded rename, or an administrator-commanded
  invite link, satisfies decision 0070 and should therefore ship, they are right about 0070
  and wrong about the unit. Both were considered and refused above on the right the assistant
  must not hold, the self-authoring loop, and the fact that the administrator giving the
  command already holds the right themselves. Answer with those reasons; do not reopen 0070.
