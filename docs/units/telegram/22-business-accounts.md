# Telegram unit 22 — business connections: refused whole, and the refusal made checkable

Date: 2026-08-27. This is the largest capability the platform hands a bot and the furthest from
what this project is. A business connection lets an account holder attach this bot to their
personal account, after which the bot receives the private correspondence of a set of chats it
cannot enumerate, may send messages that appear under that person's own name, and — where the
person ticked the boxes — may rename them, rewrite their bio, replace their profile photo, read
their gift inventory, and move their Telegram Stars onto the bot's own balance.

The unit examines the whole surface and declines it: the four inbound update types, the
`business_connection_id` parameter on every sending and editing method, `getBusinessConnection`,
the fourteen account-management and asset methods, and the four story methods the same rights
object carries. It then spends its implementation making the decline a property of the code.
Four things stop this feature independently, and any one of them would be enough. The connection
is created by a stranger and cannot be ended from this side, so decision 0052's admission model
has nothing to check and no call to enforce it with. The platform's own developer terms forbid
disclosing business message content to a third-party API, which is exactly what answering a
message means in this architecture. The deployment's published notice tells members that direct
chats are not served at all. And the product is a community assistant for one project's groups;
a personal secretary for arbitrary strangers is a different product wearing the same token.

There is also a defect this unit found while reading, and it matters beyond business accounts:
the channel key the adapter mints is the chat id in decimal, and the platform documents that a
business chat's identifier may collide with the identifier of a chat the bot already knows. That
is recorded below as the reason the key's contract holds today and the one thing that would
break it.

## The finding

**A business connection is reach without admission.** Decision 0052 makes the operator's
invitation a durable fact: written exactly one way, from a membership observation whose adder
matches the configured operator (`authorization.rs:28-33`); checked before anything touches the
ledger; enforced by leaving the chat when the row is missing. A business connection inverts
every part of that.

1. **The person who admits is not the operator.** Any account holder who finds the bot can
   attach it. The bot's side has no approval step, no allowlist and no callback: the whole flow
   is described from the connecting person's side, and nothing in it gives the bot a veto. A
   business connection produces no membership observation and names no operator, so there is
   nothing decision 0052's one write path could accept and nothing it could refuse.
2. **The admitted scope cannot be read.** `BusinessConnection` carries an id, the owning user,
   that user's private chat id, a date, a rights object and an enabled flag, and no list of
   chats. The platform states that the account owner chooses which chats the bot can access; the
   bot learns the scope only by observing which messages arrive. A bot that cannot enumerate its
   own reach cannot state it to anyone, which is the first thing the developer terms demand of a
   business bot.
3. **The refusal cannot be enforced by leaving.** Decision 0052 heals itself because the
   withdraw directive becomes a `leaveChat` call (`driver.rs:444-446`, `driver.rs:613-625`), so a
   lost leave is repaired at the group's next contact. The whole API reference was read for a
   counterpart and there is none: no method ends a business connection, and `is_enabled` is a
   field the bot reads, never one it writes. Only the account owner can disconnect. The one
   refusal available to this side is to never subscribe and never call, which is what this unit
   builds.
4. **The core's least-protected door is the one this would enter.** `is_authorized` runs only
   for group channels (`assembly.rs:1201-1204`); a direct channel is checked against the
   `DirectChats` switch and nothing else (`assembly.rs:1206-1208`). A business message is a
   private-chat message. So the fail-closed admission that protects every group would not apply
   to it, and on a deployment with direct chats on, a stranger's private correspondence would be
   recorded and answered by the same path that serves an ordinary direct message, at
   `Authority::Member` (`translate.rs:129-131`).

None of that is repaired by a branch in the core. The neutral vocabulary for the thing would be
a **delegated channel** — a channel the assistant reaches on another person's behalf, admitted
by that person instead of by the operator — and naming it is the easy half. The hard half is
that admission, authority, disclosure, protection and erasure would each need a second answer
for the delegated case, and the erasure half is not extra work but a conflict: one person's
erasure request would reach into another person's correspondence (below). That is the structural
reason for the refusal. The developer-terms reason and the published-notice reason are
independent of it and each sufficient alone.

## Grounding

### The platform, read 2026-08-27

Fetched from `core.telegram.org/bots/api`, `core.telegram.org/bots/features` and the changelog
at `core.telegram.org/bots/api-changelog` on 27 August 2026, plus the Bot Developer Terms at
`telegram.org/tos/bot-developers` the same day. The brief for this series named Bot API 10.1
(11 June 2026) as current; the changelog's newest entry is **Bot API 10.3, dated 24 August
2026**, with 10.2 on 14 July 2026 and 10.0 on 8 May 2026. Every sentence in quotation marks was
read from those pages on that date.

**The feature arrived in two halves, four years apart.** Bot API 7.2 (31 March 2024),
"Integration with Business Accounts", added `BusinessConnection`, the four update types,
`getBusinessConnection`, `business_connection_id` on sixteen send methods and on
`sendChatAction`, and the `business_connection_id` and `sender_business_bot` fields on `Message`.
Bot API 9.0 replaced `BusinessConnection.can_reply` with a `rights` object of type
`BusinessBotRights` and added the whole account-management family: `readBusinessMessage`,
`deleteBusinessMessages`, `setBusinessAccountName`, `setBusinessAccountUsername`,
`setBusinessAccountBio`, `setBusinessAccountProfilePhoto`, `removeBusinessAccountProfilePhoto`,
`setBusinessAccountGiftSettings`, `getBusinessAccountStarBalance`, `transferBusinessAccountStars`,
`getBusinessAccountGifts`, `convertGiftToStars`, `upgradeGift`, `transferGift`, `postStory`,
`editStory` and `deleteStory`. Bot API 9.3 added a fourth story method, `repostStory`, "allowing
bots to repost stories across different business accounts they manage".

**The four inbound update types, and the fact that none of them is excluded by default.**

- `business_connection`, carrying `BusinessConnection` — "The bot was connected to or
  disconnected from a business account, or a user edited an existing connection with the bot".
- `business_message`, carrying a full `Message` — "New message from a connected business
  account".
- `edited_business_message` — "New version of a message from a connected business account".
- `deleted_business_messages`, carrying `BusinessMessagesDeleted { business_connection_id, chat,
  message_ids }` — "Messages were deleted from a connected business account". Its `chat` field is
  documented as "Information about a chat in the business account. The bot may not have access to
  the chat or the corresponding user."
- `getUpdates.allowed_updates`: "Specify an empty list to receive all update types except
  `chat_member`, `message_reaction`, and `message_reaction_count` (default). If not specified,
  the previous setting will be used." None of the four business types is in that exclusion set,
  so a token that never named its selection receives all four. The same paragraph carries the
  transition warning this unit's fail-safe exists for: "Please note that this parameter doesn't
  affect updates created before the call to getUpdates, so unwanted updates may be received for a
  short period of time."

**`BusinessConnection`** carries `id` (String), `user` (a full `User`), `user_chat_id`
("Identifier of a private chat with the user who created the business connection"), `date`
("Date the connection was established in Unix time"), an Optional `rights` of type
`BusinessBotRights`, and `is_enabled` ("True, if the connection is active"). There is no list of
admitted chats and no field the bot can write.

**`BusinessBotRights`** is fourteen Optional flags, each of them the account owner's grant:
`can_reply` — "True, if the bot can send and edit messages in the private chats that had incoming
messages **in the last 24 hours**"; `can_read_messages`; `can_delete_sent_messages`;
`can_delete_all_messages` — "True, if the bot can delete **all** private messages in managed
chats"; `can_edit_name`; `can_edit_bio`; `can_edit_profile_photo`; `can_edit_username`;
`can_change_gift_settings`; `can_view_gifts_and_stars`; `can_convert_gifts_to_stars`;
`can_transfer_and_upgrade_gifts`; `can_transfer_stars` — "True, if the bot can transfer Telegram
Stars received by the business account **to its own account**, or use them to upgrade and
transfer gifts"; `can_manage_stories` — "True, if the bot can post, edit and delete stories on
behalf of the business account".

**A documentation inconsistency, recorded because an implementer would trip on it.** The rights
object spells three flags `can_edit_name`, `can_edit_bio` and `can_edit_username`, while the three
methods that consume them say "Requires the `can_change_name` business bot right", "the
`can_change_bio` business bot right" and "the `can_change_username` business bot right". Both
spellings were read from the same page on the same day. Nothing in this unit depends on which is
authoritative, and this document does not resolve it; anyone reopening the question has to
establish it against a live connection first.

**The account-management surface, method by method.** Each takes `business_connection_id` as a
required String and each names the right it requires.

- `getBusinessConnection(business_connection_id)` — "get information about the connection of the
  bot with a business account", returns `BusinessConnection`. It needs no right and no
  subscription; it needs an id, which is the one thing this unit makes unobtainable.
- `readBusinessMessage(business_connection_id, chat_id, message_id)` — requires
  `can_read_messages`; the chat "must have been active in the last 24 hours".
- `deleteBusinessMessages(business_connection_id, message_ids)` — "1-100 identifiers of messages
  to delete. All messages must be from the same chat." Requires `can_delete_sent_messages` for
  the bot's own messages, `can_delete_all_messages` for anyone's.
- `setBusinessAccountName(business_connection_id, first_name, last_name)` — first name "1-64
  characters", last name "0-64 characters".
- `setBusinessAccountUsername(business_connection_id, username)` — "0-32 characters".
- `setBusinessAccountBio(business_connection_id, bio)` — "0-140 characters".
- `setBusinessAccountProfilePhoto(business_connection_id, photo, is_public)` — `photo` is an
  `InputProfilePhoto`, either `InputProfilePhotoStatic` (".JPG") or `InputProfilePhotoAnimated`
  ("MPEG4"), and the static form states: "Profile photos can't be reused and can only be uploaded
  as a new file, so you can pass `attach://<file_attach_name>` if the photo was uploaded using
  multipart/form-data". `is_public` sets "the public photo, which will be visible even if the main
  photo is hidden by the business account's privacy settings".
- `removeBusinessAccountProfilePhoto(business_connection_id, is_public)`.
- `setBusinessAccountGiftSettings(business_connection_id, show_gift_button, accepted_gift_types)`.
- `getBusinessAccountStarBalance(business_connection_id)` — returns `StarAmount { amount,
  nanostar_amount }`.
- `transferBusinessAccountStars(business_connection_id, star_count)` — "Transfers Telegram Stars
  from the business account balance to the bot's balance", `star_count` "1-10000".
- `getBusinessAccountGifts(business_connection_id, …)` — returns `OwnedGifts`, filtered by five
  `exclude_*` booleans and paginated by an `offset` string and a `limit`.
- `convertGiftToStars(business_connection_id, owned_gift_id)`.
- `upgradeGift(business_connection_id, owned_gift_id, keep_original_details, star_count)`.
- `transferGift(business_connection_id, owned_gift_id, new_owner_chat_id, star_count)` — the new
  owner's "chat must be active in the last 24 hours".
- `postStory`, `repostStory`, `editStory` and `deleteStory` take the same connection id under
  `can_manage_stories`; `postStory` takes an `InputStoryContent`, a photo or a video, and
  `repostStory` "Reposts a story on behalf of a business account from another business account.
  Both business accounts must be managed by the same bot".

**`business_connection_id` on the sending and editing side.** On `sendMessage` it is documented
as "Unique identifier of the business connection on behalf of which the message will be sent".
This one parameter is what makes a message appear under the account owner's name. Since 7.2 it is
on the sixteen send methods and `sendChatAction`; later versions added it to the five edit methods
and `stopPoll` (7.5, 18 June 2024), to `pinChatMessage` and `unpinChatMessage` (7.8, 31 July
2024), to `sendPaidMedia` (7.9), to `createInvoiceLink` (8.0, "For payments in Telegram Stars
only"), to `sendChecklist` and `editMessageChecklist`, and to `sendRichMessage`, whose parameter
adds: "Bot can send rich messages on behalf of a business account only if the corresponding user
can send rich messages."

**The editing window a business bot does not control.** Every edit method carries the same
sentence: "Note that business messages that were not sent by the bot and do not contain an inline
keyboard can only be edited within 48 hours from the time they were sent." Unit 03 already
recorded that sentence when it specified editing (`03-editing-messages.md:59-63`); it is repeated
here only because it shows that even the reply half of this surface has a platform-imposed clock
the assistant would have to model.

**What the platform documents about the appearance, and what it does not.** `Message` gained
`business_connection_id` — "Unique identifier of the business connection from which the message
was received. If non-empty, the message belongs to a chat of the corresponding business account
**that is independent from any potential bot chat which might share the same identifier**" — and
`sender_business_bot`, "The bot that actually sent the message on behalf of the business account.
Available only for outgoing messages sent on behalf of the connected business account." That
second field is documented as something the **bot** receives about its own outgoing messages. The
API reference says nothing about what the human on the other side of the chat sees. Recorded as
undocumented, not as a claim in either direction. `Message.is_from_offline` is the nearest thing
to a marker on the receiving side, and it says only "True, if the message was sent by an implicit
action, for example, as an away or a greeting business message, or as a scheduled message".

**Several affordances this project already uses are unavailable under a business connection.**
`ReplyKeyboardMarkup` and `ReplyKeyboardRemove` are "Not supported in channels and for messages
sent on behalf of a business account"; `InlineKeyboardButton.web_app` and the three
`switch_inline_query*` fields carry the same exclusion; `ReplyParameters.chat_id` is "Not
supported for messages sent on behalf of a business account"; and
`ReplyParameters.allow_sending_without_reply` is "Always True for messages sent on behalf of a
business account". That last one is the only place where the platform's business behaviour and
this adapter's behaviour agree by accident: `send_body` already sets
`allow_sending_without_reply` on every threaded send (`client.rs:450-455`).

**The switch is in BotFather, and the platform now calls it Secretary Mode.** The features page:
"Bots can enable **Secretary Mode**, allowing users to connect the bot to their account so it can
process incoming messages and, where permitted, respond on their behalf", with the quick-start
step "Enable Secretary Mode for your bot in @BotFather." No API method turns it on or off. The
terminology has drifted: the page and the 10.0 changelog entry say Secretary Bots and Secretary
Mode, while every object, field and method name still says Business. Both names describe one
switch, and this document uses the API's spelling for identifiers and the platform's current
prose for the setting.

**Whether the flag is on is readable from `getMe`.** `User.can_connect_to_business` — "True, if
the bot can be connected to a user account to manage it. Returned only in getMe."

**Bot API 10.0 widened who may connect.** Under General: "Allowed Secretary Bots to manage
accounts of users **without a Telegram Premium subscription**." The developer terms still
describe the feature as one "Telegram Business subscribers may choose"; both sentences were read
on 27 August 2026 and both are reproduced here without resolving them. The practical reading is
that the population able to attach a bot is now every account holder, not a paying subset.

**Every connecting person sends the bot an ordinary private message.** The features page: "Users
who connect your bot to their account will see a quick action bar at the top of each managed chat
– tapping on 'Manage Bot' will redirect them to your bot, which will receive a deep link message
in the format `/start bizChat<user_chat_id>`." That is a message on the subscribed `message`
type, not a business update.

**The Bot Developer Terms, section 5.4, bind this directly.** "If your TPA supports being
designated as a Chatbot under Telegram Business", the developer additionally agrees to, among
others:

> (i) Clearly and truthfully represent the full extent of the services provided by your bot
> through Telegram Business;
>
> (ii) If applicable and without limiting 4.2., clearly state what private user data you will
> retain, how long you will store it and for what purpose;
>
> (iii) Never use message contents, files or other data you obtained or processed through
> Telegram Business for any other purpose than providing your services as a business Chatbot;
>
> (iv) Without limiting 4.2., never disclose message contents, files or other data you obtained
> or processed through or in connection with Telegram Business to third parties (including
> third-party APIs) without the user's authorization;
>
> (v) Never attempt to conceal any activity carried out by your TPA from the business account
> owner;

Section 5 adds that a bot "cannot use its privileges (moderation permissions, private chat
management via Telegram Business, etc.) to carry out actions that diverge from the purposes under
which the privileges were originally obtained", and the stated consequence of a breach is "the
immediate and permanent ban of your TPA and your account from the Telegram platform".

**One platform inference this document does not prove.** The API says only "True, if the bot can
be connected to a user account to manage it", never what `getMe` answers on a token where
Secretary Mode was never enabled. Nothing merged depends on the answer: a token that never
reports the flag produces no notice line, which is the same outcome as the setting being off.
Marked in the shape unit 06's AC13 and unit 08's AC16 use, with a named post-merge check.

### Our tree, at `7fb217d`

- **The poll names its update types on every request, which is the only reason none of the four
  arrives.** `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]`
  (`client.rs:103`), passed as `allowed_updates` in `get_updates` (`client.rs:317-320`) because
  "an absent selection inherits whatever an earlier setting left on the token"
  (`client.rs:99-102`). Since all four business types are in the platform's default set, an
  adapter that had left the selection unstated would be receiving them today. The existing wire
  test pins the exact array (`tests/adapter/group_context.rs:22-35`).
- **A business update that arrived anyway is already skipped, by accident and not by design.**
  The decoded `Update` carries three optional payload fields beside its id, and "Unknown fields
  are ignored by the decoder, so the model stays exactly as small as the translation needs"
  (`client.rs:105-107`). An update carrying only `business_message` decodes with all three
  absent, falls through `translate` to `Translation::Skip(Skip::NonMessage)`
  (`translate.rs:126-128`), and `process` logs it at debug, acknowledges it and advances the
  offset (`driver.rs:367-371`). Nothing is fetched, nothing is written, nothing is answered. That
  is the behaviour this unit wants, and nothing proves it.
- **If the decode ever learned the field, a business chat would be treated as an ordinary direct
  chat.** `translate` reads `message.chat.kind`, and `"private"` yields `(ChannelKind::Direct,
  Some(Authority::Member))` (`translate.rs:129-131`); a direct channel is addressed
  unconditionally (`translate.rs:171-173`), so every message in it would summon a turn. The
  distance between "refused" and "answering a stranger's private correspondence as a member" is
  one decoded field, which is why the refusal is checked instead of assumed.
- **The channel key is the chat id in decimal, and the platform warns that the identifier can
  collide.** `channel_key` builds `ChannelKey { adapter, channel: chat_id.to_string() }` and its
  comment calls the decimal form "a durable contract — it keys the channel mappings"
  (`translate.rs:309-317`). `Message.business_connection_id` documents that a business chat "is
  independent from any potential bot chat which might share the same identifier". Feeding both
  through `channel_key` would map two conversations onto one key: `mapping::find` would return the
  existing direct conversation and a stranger's correspondence would be appended into it, on an
  append-only ledger, with no way to separate the two afterwards. The mapping module is "the one
  place a channel key is stored" (`mapping.rs:1-8`) and has one key column, so the injectivity of
  `channel_key` is a real invariant this surface would break.
- **Erasure would reach across controllers.** `direct_conversations_of` walks every mapping row of
  kind `Direct` and collects the conversations carrying the principal's messages
  (`erasure.rs:166-175`); the module removes a person's direct conversations whole. If business
  chats were mapped as direct channels, one customer's erasure request would delete the account
  owner's correspondence with that customer — data the owner controls, not the operator. The
  conflict is structural, not a matter of care.
- **Admission is checked for group channels only.** `is_authorized` runs when
  `message.channel_kind == ChannelKind::Group` (`assembly.rs:1201-1204`), and its module states
  "Absence is refusal" (`authorization.rs:54-56`), with "Direct channels never touch this table"
  (`authorization.rs:14`). The direct branch reads the `DirectChats` switch and nothing else
  (`assembly.rs:1206-1208`, the switch at `assembly.rs:194-208`).
- **The withdraw directive is a leave call.** `IngestOutcome::Withdraw` becomes `leave(client,
  pending.chat_id, …)` (`driver.rs:444-446`), which calls `leave_chat` under a per-chat rest
  (`driver.rs:613-625`). There is no analogue for a business connection, so the outcome vocabulary
  has nothing to express a refusal with even if the core learned the case.
- **The first-contact lookup never runs for a private chat.** `first_contact` is called only when
  `pending.channel_kind == ChannelKind::Group` (`driver.rs:377-378`), and `ChatInfo` decodes a
  title and a pinned message and nothing else (`client.rs:189-192`). So the three business profile
  fields the platform puts on a private chat's `ChatFullInfo` — `business_intro`,
  `business_location` and `business_opening_hours` — are unreachable twice over: no call is made,
  and the decoder would drop them if one were.
- **The outbound body has no place for a connection id and gains none.** `send_body` builds
  `{ "chat_id", "text" }` with an optional `parse_mode` and `reply_parameters`
  (`client.rs:446-455`); `send_chat_action` builds `{ "chat_id", "action": "typing" }`
  (`client.rs:401-402`). Every outbound call in the adapter is keyed on a chat id the adapter
  minted itself.
- **The core-side word list cannot carry the word "business".** The scan lowercases both the list
  and every line and matches whole runs of letters and digits (`crates/core/tests/vocabulary.rs:23`,
  `:61-67`, `:76`), and the core already uses the ordinary English word four times in neutral
  prose (`composing.rs:149`, `composing.rs:444`, `outbound.rs:299`, `outbound.rs:390`). A camelCase
  method name is one run, so an entry `getbusinessconnection` would match — but the scan reaches
  the core crate only (`vocabulary.rs:33-45`), where no HTTP call exists to name, so such entries
  would be inert.
- **The series has converged on one refusal list, and the units disagree about its name.** Unit
  09's AC2 introduced a substring scan over both crates' `src` directories for six join-request
  names. Unit 19 turns that into "a file `docs/telegram-refused-methods.txt`, in the same form as
  `docs/platform-vocabulary.txt` (one name per line, `#` comments, blank lines ignored)"
  (`19-chat-administration.md:410-412`) and states the ownership rule: the first unit to merge
  creates the file and the test, the later ones add their names and delete their own scan in the
  same commit (`:421-424`). Unit 20 specifies the same mechanism under a different filename,
  `docs/administrative-methods.txt` (`20-moderation-actions.md:276`, `:447`). Unit 25 says plainly
  that unit 09 owns the mechanism and that it adds names, not a scan
  (`25-web-apps-and-keyboards.md:333-335`). Unit 08 predates the convergence and specifies its own
  adapter-only scan with its own list file. Unit 24 specifies a third: adapter-only, case-sensitive
  substring, carrying seven of this unit's method names already
  (`24-payments-stars-and-gifts.md:427-433`). Unit 16's AC11 claims the literal
  `business_connection_id` for a scan of its own (`16-chat-actions.md:412-413`).
- **The `getMe` capability reading already exists as a specified mechanism.** Unit 08 adds a named
  tolerant `Option<bool>` reader to `BotIdentity` for `supports_inline_queries` and
  `supports_guest_queries`, a pure `capability_notice(&BotIdentity) -> Option<String>`, and one
  error-level line written by `poll_loop` after `fetch_identity` returns. `BotIdentity` today has
  an `id` and two Optional strings and no capability field at all (`client.rs:226-239`).
  `can_connect_to_business` is the same kind of flag from the same `getMe` answer.
- **The scripted server has no live endpoint.** It binds loopback and "Nothing here leaves the
  machine" (`tests/adapter/server.rs:1-11`), and it selects and acknowledges updates by
  `update_id` (`server.rs:509`, `:518`), so a scripted update in this unit carries an `update_id`
  plus one payload object and nothing else. Tests can read the store directly through
  `fixture.store.run` (`tests/adapter/offset.rs:149-157`).
- **The published notice describes a bot that does not serve private chats at all.** "We store the
  text of each message in a group the assistant belongs to"
  (`docs/privacy/bot-assistant-privacy-policy.md:20-21`), "We do not serve direct chats: a direct
  message is rejected and not stored" (`:22-23`), and "We take nothing about you from anywhere
  else" (`:50`). The record of processing describes the activity as "A bot in the halogenOS
  community groups stores the groups' messages" (`records-of-processing.md:30`) and names two
  categories of data subject, both attached to the project's own surfaces (`:48-52`). The
  legitimate-interests assessment rests its balancing on "Messages people chose to post to an open
  community group, in front of every other member. This is not private correspondence"
  (`lia.md:117-118`) and "A person joining a project's support group where an announced assistant
  answers questions expects to be read by that assistant. Nobody is surprised" (`lia.md:131-132`).
- **The processor named in the record is a third-party API.** Recipient R1 is Requesty Ltd,
  receiving "The conversation's text and the public username of each speaker, plus the system
  prompt and the group's context notes" (`records-of-processing.md:82`), with sub-processors under
  R2. Answering a business message means sending its text there, which is what developer term
  5.4(iv) forbids without the user's authorization — and a business chat has two people in it,
  only one of whom connected the bot.
- **The disclosure duty has no honest discharge under someone else's name.** The AI Act record
  states the line "Hi, I'm <name>, an AI system, made to assist members of the community", "stored
  into the answer itself, resolved per person from the ledger's own memory"
  (`docs/compliance/ai-act.md:72-79`). Sent with `business_connection_id`, that sentence would
  appear under the account owner's name to a person who believes they are writing to the owner.
  Either the line is omitted, leaving Article 50(1) undischarged, or it is sent and misdescribes
  who is speaking. There is no third option this unit could specify.
- **There is no start command to receive the deep link.** The core's whole command vocabulary is
  the five privacy commands (`privacy.rs:97-101`). `/start bizChat<user_chat_id>` would be
  recorded verbatim as what the person said (decision 0017) — text a button inserted and the
  person never typed. With `DirectChats::Off` it is disregarded before any write
  (`assembly.rs:1206-1208`), which is the correct outcome and the one this deployment has.
- **The impact assessment's review triggers are the test for whether a document changes.**
  `dpia.md:557-586` lists them, including "Any new path that sends message content off the
  machine" and "A change to what is collected". This unit fires none of them, because it receives
  nothing new and sends nothing new.

## Decisions taken with this unit

- **The whole business surface is refused: no connection is served, no business update is
  consumed, no business method is called, 2026-08-27.** Four independent reasons, each
  sufficient. First, admission: the connection is created by a stranger, covers chats the bot
  cannot enumerate, and cannot be ended from this side, so decision 0052's three properties — the
  operator's own invitation, a durable row to check, a leave call to enforce it — are all absent,
  and the surface would enter through the direct-channel path, which has no authorization table at
  all. Second, the platform's terms: answering means sending the message text to the model
  processor, and term 5.4(iv) forbids disclosing business message content "to third parties
  (including third-party APIs) without the user's authorization", while the second party to every
  such chat authorized nothing and has no relationship with the operator. Third, the published
  statements: the privacy notice says direct chats are not served and that nothing is taken from
  anywhere else, and the legitimate-interests assessment rests explicitly on messages that are
  "not private correspondence"; shipping this would make three published documents untrue on the
  day it merged, which is a defect and not a follow-up. Fourth, purpose: this assistant answers
  questions about one open-source project in its own community groups. *Rejected:* accepting
  connections but reading only, never replying — the reading is the processing that the terms, the
  record of processing and the legitimate-interests assessment all turn on, and a bot that stores
  private correspondence without answering has the whole exposure and none of the use. *Rejected:*
  accepting connections from the operator's own account only — the platform gives the bot no way
  to refuse the others, so the check would run after the private messages had already been
  delivered into this process, and it would still need every mechanism the reopening list names.
  *Rejected:* leaving the question open as a follow-up — follow-ups record accepted shortfalls in
  shipped work; this is a decision, and an unrecorded one gets re-derived or, worse, shipped.
- **The account-management and asset methods are refused as a class, and the reason is recorded
  separately from the messaging refusal, 2026-08-27.** Renaming a person, rewriting their bio,
  replacing their profile photo, changing their gift privacy settings, converting or transferring
  their gifts, posting a story under their name or moving up to 10000 of their Stars onto the
  bot's own balance are effects on a person, decided by whatever the model emitted. This project's
  standing rule is that the assistant assesses and a human decides, with the decision point in the
  mechanism instead of in a convention (decision 0070). That decision is written about moderation
  effects and does not literally cover a person's own account, so this unit does not claim it as
  settled coverage; it applies the same principle by analogy and records the extension here. The
  asset methods are worse than the moderation case in one respect: `transferBusinessAccountStars`
  moves value to the bot's own balance, so a defect in this project would enrich this project,
  which is a shape no amount of review makes acceptable. *Rejected:* refusing the money-moving
  methods and allowing the cosmetic ones — a bot that can rewrite a person's name and bio can
  impersonate them to everyone they know, and "cosmetic" describes the API call, not the effect.
  *Rejected:* allowing them behind an operator confirmation — the confirming human would be the
  operator, not the account owner whose account is being changed, so the mechanism would put the
  decision in the wrong person's hands, which is the failure decision 0070 exists to prevent.
- **The four business update types are never subscribed, and the refusal is asserted on the wire,
  2026-08-27.** `CONSUMED_UPDATE_TYPES` gains nothing; one new assertion states that the array
  sent as `allowed_updates` contains none of `"business_connection"`, `"business_message"`,
  `"edited_business_message"` or `"deleted_business_messages"`, with the reason in the assertion's
  own message so a later change has to read it before removing it. It is a containment check, not
  an equality check: units 05, 07 and 09 each assert something about the same constant and two of
  them already collide with each other, and an exact-list assertion here would join that collision
  for no gain, since this unit's claim is about absence. *Rejected:* an exact-list assertion, for
  that reason. *Rejected:* saying nothing on the wire on the grounds that the constant is visibly
  three elements long — the platform's default set includes all four business types, so the
  explicit naming is the whole protection, and an unasserted protection is one refactor from gone.
- **An arriving business update is pinned as a skip for all four types, and none of them gets a
  decode path, 2026-08-27.** The documented `allowed_updates` transition window means a token that
  polled without an explicit selection can deliver business updates into this adapter's first
  polls. Today each of the four decodes with every known field absent and is skipped anonymously
  as `Skip::NonMessage` — correct behaviour that nothing proves. This unit pins all four: a
  scripted update carrying an `update_id` and one such payload is acknowledged, advances the
  offset, writes no row and causes no outbound request. *Rejected:* adding the fields to the
  decoded `Update` with named `Skip` reasons, the way `edited_message` earns one
  (`client.rs:116-118`) — that field exists because edits arrive constantly on a subscribed type,
  whereas these would decode a stranger's private message text on a path that runs only in a
  misconfiguration. Unit 06's reasoning against decode paths that never execute applies unchanged,
  and here the decoded value would be personal data the project has decided not to receive.
  *Rejected:* pinning `business_message` alone as representative — `business_connection` is the
  one that would tell a future reader a connection exists, and its skip is the fact most likely to
  be reversed by somebody trying to be helpful.
- **Secretary Mode is read from `getMe` and reported at startup through unit 08's reader and
  notice, not through a second mechanism, 2026-08-27.** `can_connect_to_business` joins
  `supports_inline_queries` and `supports_guest_queries` as a third Optional boolean on
  `BotIdentity`, read by the same named tolerant deserializer, with an unreadable value reading as
  set for the same reason: the notice stops nothing, so a false alarm costs one log line the
  operator checks in BotFather in seconds, while a missed alarm leaves a token that strangers can
  attach to their accounts with nobody told. The text names the finding and directs the operator
  to Secretary Mode in BotFather; the platform documents no API method or BotFather command that
  reverses it, so none is invented. *Rejected:* a second reader and a second startup line — the
  two units describe the same hazard through the same field of the same answer, and two mechanisms
  for one decision is the duplication this repository refactors away from. *Rejected:* refusing to
  start when the flag is set — an adapter taking the community's assistant offline over a
  BotFather setting costs the community more than the setting does, given that no business update
  is subscribed and no business method is called. *Rejected:* calling `getBusinessConnection` at
  startup to discover live connections — it takes an id the bot learns only from the update this
  unit refuses, so there is nothing to pass it, and the call would itself be the first business API
  call this unit exists to prevent.
- **The outbound refusal joins the one committed refusal list the series converged on, and the
  overlap with three sibling units is settled here by naming who owns which literal, 2026-08-27.**
  The names go in `docs/telegram-refused-methods.txt` — unit 19's filename, one name per line with
  `#` comments — scanned by one substring scan over both crates' `src` directories, the mechanism
  unit 09 introduced and unit 19 generalised. This unit's entries are the twelve method names no
  other unit claims — `getBusinessConnection`, `readBusinessMessage`, `deleteBusinessMessages`,
  `setBusinessAccountName`, `setBusinessAccountUsername`, `setBusinessAccountBio`,
  `setBusinessAccountProfilePhoto`, `removeBusinessAccountProfilePhoto`, `postStory`, `repostStory`,
  `editStory`, `deleteStory` — plus the request-body key `business_connection_id`, which is the
  single literal that would turn an ordinary send into a send under someone's name. The seven
  gift and Stars methods that also take a connection id (`getBusinessAccountGifts`,
  `getBusinessAccountStarBalance`, `transferBusinessAccountStars`, `convertGiftToStars`,
  `upgradeGift`, `transferGift`, `setBusinessAccountGiftSettings`) belong to unit 24's list, which
  already carries them; this unit does not repeat them, and whichever of the two merges second
  adds only the names the file lacks. `business_connection_id` is claimed by unit 16's AC11 as
  well; this unit owns it, because the business refusal is this unit's subject and unit 16's
  interest in it is one consequence of it. *Rejected:* extending unit 08's separate adapter-only
  scan, which is what a first reading of this unit's dependencies suggests — three scans reading
  three lists is three places where one decision is recorded, and unit 25 already decided that
  question the same way for the same reason. *Rejected:* unit 20's filename
  `docs/administrative-methods.txt` — this file will carry method names from at least five units
  and only the general name stays true; the disagreement is named in the launch notes so whichever
  unit merges first settles it instead of each unit assuming its own. *Rejected:* adding the names
  to `docs/platform-vocabulary.txt` — that scan reads the core crate only, the core has no HTTP
  client and never will, so the entries would be inert, and the one word that would bite,
  "business", already appears four times in ordinary core prose and would fail the scan on merge.
  *Rejected:* forbidding the four inbound update-type names in the same scan — the wire assertion
  and the constant's comment must name them, so the scan would fail on this unit's own diff or need
  exceptions that blunt it. Inbound is checked on the wire, outbound is checked in the source, and
  neither mechanism is asked to do the other's job.
- **The deep-link message needs no new handling and gets none, 2026-08-27.** `/start
  bizChat<user_chat_id>` arrives on the subscribed `message` type as an ordinary private message,
  so it is already covered: with `DirectChats::Off` it is disregarded before any write
  (`assembly.rs:1206-1208`), and with the switch on it is an ordinary direct message recorded
  verbatim under decision 0017. This unit adds no start command and no special case. *Rejected:*
  a start command that answers the connecting person with the refusal — it would put a
  first-contact surface in front of strangers, it would need the disclosure work unit 12 governs,
  and it would record as a person's own words a message a button composed. *Rejected:* filtering
  the literal `bizChat` prefix in the adapter — the adapter decides nothing, and a text filter on
  a message body is behaviour by any reading.
- **Nothing streams, because nothing here moves a byte, 2026-08-27.** Two of the refused methods
  would upload. `setBusinessAccountProfilePhoto` takes an `InputProfilePhoto` whose own
  documentation states that "Profile photos can't be reused and can only be uploaded as a new
  file", so it could not use the identifier-reuse path unit 02 relies on and would need a fresh
  multipart body on every call; `postStory` takes an `InputStoryContent`, a photo or a video, with
  the same consequence. Recorded because the streaming constraint binds every spec, and recorded
  explicitly so that a reopening cannot treat the upload half as solved by unit 02.
- **No privacy or compliance document changes with this unit, and the reversal list is written
  down instead, 2026-08-27.** The assistant receives no business update, calls no business method,
  emits no business affordance and stores nothing new, so there is no new category of data, no new
  recipient and no new storage; none of the impact assessment's review triggers
  (`dpia.md:557-586`) fires. An amendment saying that a thing was considered and not done would
  put a non-event into a register of processing activities. *Rejected:* a note in the record for
  completeness — a record of processing describes processing, and padding it with refusals makes
  the real entries harder to audit.
- **The channel key's injectivity is recorded as an invariant with its named threat, comment only,
  2026-08-27.** `channel_key`'s comment already calls the decimal form a durable contract; this
  unit adds one sentence naming what would break it — a chat identifier that is not unique across
  surfaces, which the platform documents for business chats — so the next person to widen the
  decode reads the hazard at the site instead of rediscovering it after the ledger has merged two
  people's conversations. *Rejected:* changing the key's shape now to make it collision-proof, for
  instance by prefixing a surface name — the collision cannot occur while the adapter mints keys
  only for chats it was admitted to, the change would migrate every stored mapping row against a
  hazard that does not exist yet, and the right time to pay that cost is the unit that actually
  introduces a second surface, if one ever does.

## What this unit examined and deliberately leaves alone

**The rights-flag naming inconsistency is reported, not resolved.** The platform spells three
rights `can_edit_*` in `BusinessBotRights` and `can_change_*` in the three method descriptions.
This unit calls none of those methods and decodes none of those flags, so nothing here depends on
the answer. It is recorded so that whoever reopens the question knows to establish it against a
live connection before writing a decoder, instead of assuming the object's spelling is the one the
server enforces.

**Guest mode stays where unit 08 left it.** `guest_message` and `answerGuestQuery` name a
neighbouring hazard — a bot acting in a chat it never joined — and unit 08 reads
`supports_guest_queries`, pins the guest-message skip, and says plainly that deciding what guest
mode should do belongs to its own examination. That division is unchanged. This unit adds the four
business types to the same fail-safe and the third flag to the same startup notice, and decides
nothing about guest mode.

**`Message.via_bot` remains unread, as unit 08 recorded.** Nothing in this unit changes the
message decode, and the provenance question that field raises belongs to whichever unit next opens
it.

**`getUserPersonalChatMessages` is not part of this surface and is not decided here.** Bot API
10.0 added it: "get the last messages from the personal chat (i.e., the chat currently added to
their profile) of a given user", taking a `user_id` and a `limit` of 1-20, with no business
connection and no permission the documentation names. It reads a person's messages without any
admission at all, which makes it a serious question — and a different one, since it involves no
connection, no rights object and no acting under anyone's name. It belongs to whichever unit
examines the reads the platform grants over a person, and it is named here so that the next reader
finds it already noticed. This unit neither calls it nor lists it.

**Unit 04's rejected alternative is confirmed from the other side.** Unit 04 considered and
rejected opening a business connection to obtain `deleted_business_messages` as a way to learn
about deletions in the group, on the reasoning that a group is not a business account and the
update is scoped to the connected account's own chats (`04-deleting-messages.md:243-246`). That
reading is correct, and this unit adds the second half: even if the update delivered something
useful, the connection could not be admitted, could not be ended, and could not be answered
without breaching the developer terms. No edit is made to that spec.

**Unit 16's AC11 overlaps this unit's scan and is not edited.** Its wording asks for "a source
scan finds no `business_connection_id` in any request body built by the adapter"
(`16-chat-actions.md:412-413`), which this unit's list entry satisfies and covers more widely,
since the list is scanned over both crates' sources. The overlap is named in the launch notes so
whichever unit merges second drops its own copy instead of adding a second scan.

## What would have to be true before this is reopened

Refusing without naming what could work is refusing without examining. The narrowest shape that
could conceivably be defensible is a **separate bot, a separate token, a separate deployment and a
separate controller relationship** — not this assistant with a setting flipped. Even that shape is
blocked today, and the checklist is the useful part.

1. **The developer terms would have to be satisfied in writing, not in spirit.** Term 5.4(iv)
   requires the user's authorization before message content reaches a third-party API. The
   connecting person could give it. The person on the other side of each chat, whose messages are
   the ones being sent, has no relationship with the operator at all — so either the design never
   sends their text anywhere, which rules out this architecture, or the authorization problem is
   unsolved. Terms 5.4(i) and (ii) require the bot to state the full extent of its services and
   what it retains, which requires knowing the admitted chats, and `BusinessConnection` does not
   carry them.
2. **The controller question would have to be answered before any code.** The account owner is the
   controller of their own correspondence; the operator would be their processor, needing an
   Article 28 contract with each connecting person, a lawful basis for the other party in each
   chat, and an answer to what happens when the two people's rights conflict. Today's documents
   describe one controller and one activity.
3. **Erasure would have to stop being a cross-controller act.** The direct-conversation walk
   removes a person's conversations whole (`erasure.rs:166-175`). Under a business connection, one
   person's Article 17 request would reach another person's stored correspondence. That is a design
   change in the erasure module, not a configuration.
4. **The channel key would need a surface dimension.** Two chats with the same identifier must not
   map to one conversation, and the platform documents that they can. This is the change most
   likely to be missed, because nothing fails loudly: the ledger simply becomes wrong.
5. **Admission would need a shape that fits a stranger's invitation.** Decision 0052's model has no
   place for an admission granted by somebody other than the operator, no withdrawal mechanism for
   a connection the platform will not let the bot end, and no way to record a scope the platform
   will not disclose.
6. **Protection would need a second half.** The per-conversation debt count binds a conversation,
   and a business connection can open an unbounded number of them at once, none of which the
   operator chose. What bounds a surface the operator cannot see would have to be decided, not
   inherited.
7. **The disclosure duty would need an honest discharge.** Article 50(1) applies to the person on
   the other side of the chat, who believes they are writing to a human. A line sent under that
   human's name saying "I'm an AI system" is either absent or misleading, and nothing in the
   guidelines contemplates borrowing a third party's name for it.
8. **Five documents would change before the code merges**, not after: the public privacy notice
   first, because it is the promise made to people and it currently says direct chats are not
   served at all; the record of processing, which gains a category of data subject and a category
   of recipient; the impact assessment, whose triggers for "a change to what is collected" and "any
   new path that sends message content off the machine" both fire; the legitimate-interests
   assessment, whose §4.1 and §4.2 rest on messages posted openly in a community group; and the AI
   Act record, for the disclosure question above.

Nothing above is a decision deferred into legitimacy: the decision is no, today, on the reasoning
in the previous section. This list exists so that a future yes has to pay the price in the open.

## The unit's contract

After this unit the repository's answer to "can a person attach this assistant to their personal
account and have it write in their name" is a recorded no with its reasoning, and the no is
checkable on both sides instead of assumed. Inbound: the poll's `allowed_updates` is asserted to
contain none of the four business update types, with the reason written into the assertion, and an
update delivered inside the platform's documented transition window — `business_connection`,
`business_message`, `edited_business_message` or `deleted_business_messages` — is proven to be
acknowledged and skipped without a single outbound request, without a stored row, and without the
message text being decoded at all. Outbound: the repository's one committed refusal list gains
this unit's twelve method names and the `business_connection_id` request key, checked by the one
substring scan over both crates' sources, so no send can acquire a connection id and no
account-management call can appear without failing a check that reports file and line. The
adapter's `getMe` decode gains a third tolerantly-read Optional boolean, `can_connect_to_business`,
read by the reader unit 08 adds and reported by the notice function unit 08 adds, so a token with
Secretary Mode enabled produces one error-level line at poll start naming the setting and where it
is reversed, while the process continues polling exactly as before. `channel_key`'s comment gains
one sentence naming the identifier collision its contract depends on not existing. The core is
untouched: no new entry point, no new vocabulary, no new channel kind, no new table, and
`docs/platform-vocabulary.txt` is unchanged because nothing in the core learned a platform word. No
privacy or compliance document changes, because nothing new is received, stored or sent anywhere.
No new dependency, no new configuration entry, and no change to any behaviour a member can observe.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes — `AnsweringMode::Helpful` and
  `AnsweringMode::Addressed` (`assembly.rs:180-188`); clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan and the secret scan clean; no new dependency and no new
  configuration entry; the diff touches no file under `crates/core/`.
- **AC2** The poll subscribes to no business update type: an adapter test asserts that the
  `allowed_updates` array sent on the wire contains none of `"business_connection"`,
  `"business_message"`, `"edited_business_message"` or `"deleted_business_messages"`, with the
  assertion's message naming this unit's decision and stating that the platform's default set
  includes all four. Written as a containment check so it survives whatever the sibling units add
  to the list, and placed beside the existing wire assertion
  (`tests/adapter/group_context.rs:22-35`) without editing it.
- **AC3** Each of the four business updates is acknowledged and ignored, pinned as four separate
  cases: with the scripted server pushing an update carrying an `update_id` and exactly one
  business payload — a `business_connection` with an id, a user and an enabled flag; a
  `business_message` whose inner message names a private chat, a sender, a text and a
  `business_connection_id`; an `edited_business_message` of the same shape; and a
  `deleted_business_messages` with a connection id, a chat and two message ids — the adapter
  acknowledges it, the next poll's offset is past it, and the recorded requests for that batch
  contain no `sendMessage`, no `sendChatAction`, no `getChatAdministrators` and no `getChat`. Four
  cases and not one representative, because `business_connection` is the one a later reader is
  most likely to decide is harmless.
- **AC4** Nothing about those four updates reaches the ledger or the identity tables: after AC3 the
  store holds no new block, no new conversation, no new channel mapping and no new principal row
  for the user named in them. Asserted by counting rows through `fixture.store.run` in the shape
  `tests/adapter/offset.rs:149-157` uses, before and after.
- **AC5** The Secretary Mode flag decodes tolerantly through unit 08's reader, pinned in the
  `client.rs` test module unit 08 creates: a `getMe` result omitting `can_connect_to_business`, one
  carrying `null`, one carrying `false`, one carrying `true`, and one each carrying a string, a
  number and an object, all decode into a `BotIdentity` without error. The wrong-typed cases read
  as `Some(true)`, and the assertion's message states why an unreadable flag counts as set.
- **AC6** The startup notice covers the third flag: `capability_notice` returns `None` when no flag
  is set; with `can_connect_to_business` alone set it returns a text containing the substrings
  `can_connect_to_business` and `Secretary Mode`, containing no BotFather command name because the
  platform documents none for this setting, and containing no part of the token, in keeping with
  the client module's no-token-in-any-string rule (`client.rs:1-9`). A further case pins that all
  three flags set produce one text naming all three field names, so the readings compose instead of
  replacing one another.
- **AC7** A token with Secretary Mode enabled does not stop the adapter: with the scripted `getMe`
  answering `can_connect_to_business: true`, the adapter completes startup, polls, ingests a group
  message and answers it exactly as with the flag absent. Pinned, because the decision that a
  misconfiguration is reported and not enforced is the one a future reader is most likely to
  reverse by accident. Uses the `set_me_result` fixture unit 08 adds.
- **AC8** The assistant emits no business affordance: `docs/telegram-refused-methods.txt` carries
  `getBusinessConnection`, `readBusinessMessage`, `deleteBusinessMessages`,
  `setBusinessAccountName`, `setBusinessAccountUsername`, `setBusinessAccountBio`,
  `setBusinessAccountProfilePhoto`, `removeBusinessAccountProfilePhoto`, `postStory`, `repostStory`,
  `editStory`, `deleteStory` and `business_connection_id`, and the single substring scan over both
  crates' `src` directories finds none of them, failing with file and line on any occurrence. The
  scan is case-sensitive, because these are unique camelCase identifiers and a scanner copied
  from the lowercasing vocabulary test would compare `getBusinessConnection` against a lowercased
  line and never match — a check that passes for the wrong reason.
- **AC9** The scan can fail, proved: a negative check matches one of the list's entries against a
  fixture line the scan is given deliberately, so an implementation that collected no files or
  compared the wrong case cannot pass vacuously.
- **AC10** The list carries its reasons in place: the entries this unit adds sit under a comment
  naming the unit and the decision, and stating that the four inbound update-type names are
  deliberately absent from the file because the wire assertion and the constant's comment must name
  them. Checked by reading the diff.
- **AC11** The list holds each name once: after this unit merges, no entry in
  `docs/telegram-refused-methods.txt` appears twice, and the seven gift and Stars methods that take
  a connection id appear under unit 24's section, not this unit's. Asserted by the scan's own list
  loader, which fails on a duplicate entry with the duplicated name in the message.
- **AC12** The constant's comment carries the reason in place: `CONSUMED_UPDATE_TYPES`
  (`client.rs:99-103`) is unchanged as a value, and its doc comment gains a sentence naming the four
  business types as deliberately absent, with the decision's number and the fact that the
  platform's default set includes them, so the next person adding an update type reads it there.
- **AC13** The channel key's comment names the threat: `channel_key` (`translate.rs:309-317`) gains
  one sentence stating that the decimal chat id is unique only across the surfaces the adapter was
  admitted to, and that a business chat's identifier is documented to be independent of a bot chat
  that may share it. Comment only; no behaviour changes. Checked by reading the diff.
- **AC14** The decision is recorded and the operator is told: a file in `docs/decisions/` records
  this unit's refusal with its date and its rejected alternatives, and
  `docs/reference/group-operator-contract.md` gains a section stating that the assistant is not a
  business bot, that Secretary Mode must stay off in BotFather, that any account holder can attach
  a bot whose Secretary Mode is on, and that a connection once made cannot be ended by the bot.
  Both files are named in the diff.
- **AC15** No file under `docs/privacy/` or `docs/compliance/` is modified by this unit's diff, and
  the reopening list above names the public privacy notice, the record of processing, the impact
  assessment, the legitimate-interests assessment and the AI Act record explicitly, so a future
  unit reopening the question cannot claim the documents were never mentioned.
- **AC16** The two unproven platform inferences are marked as unproven here and nothing merged
  depends on either, in the shape unit 06's AC13 uses. First: that `getMe` omits
  `can_connect_to_business`, or answers it false, on a token where Secretary Mode was never
  enabled. Second: which of `can_edit_name` and `can_change_name` the server enforces. Nothing
  depends on the first, because a token that never reports the flag produces no line, which is the
  same outcome as the setting being off; nothing depends on the second, because no flag is decoded
  and no method is called. The named post-merge check for the first is one `getMe` against the real
  token, read once by whoever deploys, with the result written into the operator contract.

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. The diff is small on purpose; the document is the deliverable that matters most.
- **Sequencing: this unit depends on unit 08 and should merge after it.** Three of its mechanisms
  are unit 08's — the named tolerant `Option<bool>` reader and the `client.rs` test module (AC5),
  `capability_notice` and its startup line (AC6, AC7), and the `set_me_result` fixture (AC7). If
  the order is reversed, this unit builds them to unit 08's specified shapes and unit 08 extends
  them; whichever merges second adds only its own flag and its own cases.
- **The refusal list and its scan are shared, and the series has not yet settled the filename.**
  Unit 19 names `docs/telegram-refused-methods.txt`, unit 20 names
  `docs/administrative-methods.txt`, unit 09 introduced the scan without a list file, and units 24
  and 08 each specify a scan of their own. This unit takes unit 19's filename and the general
  ownership rule stated there: the first unit to merge creates the file and the test, and every
  later unit adds names and deletes its own scan in the same commit. If unit 20 merges first under
  its own filename, this unit adds its names to that file and the rename is a one-line change
  somebody makes when the third unit arrives — the property being checked matters, the filename
  does not, and the one thing to avoid is two files.
- Adapter sites, all in `crates/adapters/telegram/src/`:
  - `BotIdentity` (`client.rs:226-239`) gains `can_connect_to_business: Option<bool>` with the same
    `#[serde(default, deserialize_with = "…")]` pair unit 08 puts on the other two flags. No new
    deserializer.
  - `capability_notice` gains the third flag's sentence. The text names Secretary Mode and
    BotFather's settings for the token, and states that the platform documents no command to
    reverse it, so no command name is invented.
  - `CONSUMED_UPDATE_TYPES` (`client.rs:99-103`): comment only, per AC12.
  - `channel_key` (`translate.rs:309-317`): comment only, per AC13.
  - Nothing else in the adapter changes. No new struct, no new field on `Update`, no new `Skip`
    variant, no new request body.
- Adapter test sites:
  - The wire assertion for AC2 as a new test in `tests/adapter/group_context.rs`, beside
    `the_poll_names_the_update_types_it_consumes`, without editing it.
  - The four scripted-update skips (AC3) and their store assertions (AC4) in
    `tests/adapter/offset.rs`, which already exercises acknowledgement and the offset contract
    through the scripted server and already reads the store directly.
  - AC5 and AC6 in the `#[cfg(test)] mod tests` unit 08 adds at the end of `client.rs`;
    `BotIdentity` is `pub(crate)`, so the test lives in-crate.
  - AC7 in `tests/adapter/end_to_end.rs`, using `set_me_result`.
  - AC8 to AC11 extend the shared scan test and its list file; no new test binary.
- Documentation sites: `docs/telegram-refused-methods.txt` (new, or extended if a sibling merged
  first), one decision file continuing the numbering after whatever is unclaimed at merge — taken
  at merge time and not fixed here, since several sibling units reserve numbers — and a section in
  `docs/reference/group-operator-contract.md` per AC14. No change to `docs/follow-ups.md`, per the
  decision above.
- Sibling collisions, stated and not acted on. Units 05, 07 and 09 each assert something about
  `CONSUMED_UPDATE_TYPES` and two of them assert exact lists that will collide once both merge;
  unit 08 records this in its own launch notes. This unit's assertion is a containment check and
  collides with none, and the exact-list wordings should be relaxed in their own units, not here.
  Unit 16's AC11 and unit 24's scan overlap this unit's list as described above.
- A naming hazard for whoever reads the two series side by side: "unit 22" in the top-level
  `docs/units/` series is the drop-sentinels unit, which several Telegram specs cite by that
  short name. This document is Telegram unit 22 and is unrelated to it.
- One thing to watch after merge: if Secretary Mode is ever enabled for an unrelated reason, the
  startup line appears and stays until the setting is reversed, and any connection made in the
  meantime keeps existing — the bot cannot end it, and this unit's refusal means the bot will not
  even see it. The operator contract says so plainly, because that is the one state where the
  refusal is complete on this side and a person is still left believing they attached a working
  secretary.
