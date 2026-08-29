# Telegram unit 26 — checklists and suggested posts: two features that cannot reach this group, and one chat shape that can

Date: 2026-08-27. This unit examines three message families the platform added in Bot API 9.1
and 9.2 — checklists, suggested posts, and posting into a channel's direct messages chat — and
finds that the assistant can create none of them. `sendChecklist` and `editMessageChecklist`
require a business connection, which unit 22 refused whole. `approveSuggestedPost` and
`declineSuggestedPost` work only in a channel's direct messages chat and only for a bot holding
an administrator right in the parent channel, which the operator contract forbids; each one is
also an editorial decision about a member's content and, where the post is paid, a movement of
that member's money.

The examination found something that does reach us, and it is the reason this unit ships code
instead of a decision record alone. **A channel's direct messages chat is a supergroup.** The
platform says so in its own changelog, and this adapter maps every supergroup to
`ChannelKind::Group` (`translate.rs:131-136`). In such a chat every message is one member's
private correspondence with the administrators of a channel; the members do not see each other.
Admitted as an ordinary group, it would put many people's private messages into one ledger
conversation, one model context and one privacy-notice window — while the assistant could not
answer a single one of them, because sending into a direct messages chat requires a parameter
this client never sends. The result would be a bot that reads private correspondence and says
nothing back.

So the unit ships three things: the recorded refusal of both feature families, named skips for
the checklist messages that can arrive in an ordinary supergroup, and a third channel kind that
makes a direct messages chat refusable fail-closed instead of admissible by accident.

## The three findings

### 1. `sendChecklist` is a business-account method, and its chat parameter has no group form

The method's own description: "Use this method to send a checklist on behalf of a connected
business account. On success, the sent Message is returned." Its first parameter,
`business_connection_id`, is marked **Yes** under Required — not optional as on `sendMessage`,
`sendPoll` and every ordinary send. Its `chat_id` reads "Unique identifier for the target chat
or username of the target bot in the format @username", where the ordinary send methods read
"target chat or username of the target bot, **supergroup or channel**". `editMessageChecklist`
carries the same required `business_connection_id` and the same narrowed `chat_id`.

Unit 22 refused business connections whole and makes the refusal checkable: no business update
type is subscribed, and one committed refusal list plus a substring scan over both crates'
sources proves no send can acquire a connection id. A method whose required parameter is on that
list cannot be called. This unit adds the two method names to the same list and adds nothing new
to the mechanism.

The consequence for this project's own question — whether a shared checklist is something this
assistant should ever create — is answered twice below, once by the platform and once by the
ledger, because a refusal that rests only on today's parameter table stops holding the day the
table changes.

### 2. A suggested post is an editorial decision, taken in a chat this assistant is not in

`approveSuggestedPost`: "Use this method to approve a suggested post in a direct messages chat.
The bot must have the 'can_post_messages' administrator right in the corresponding channel
chat." `declineSuggestedPost`: same shape, requiring `can_manage_direct_messages`, whose own
description is "True, if the administrator can manage direct messages of the channel and decline
suggested posts; for channels only".

Three separate things stop this, and any one of them is enough:

- **The chat is unreachable.** Both methods take a direct messages chat id. This assistant serves
  a community supergroup. After decision 4 below, a direct messages chat is refused before it is
  mapped, so the id never exists in this system.
- **Decision 0070.** Approving publishes a member's text to a channel's whole audience under the
  channel's name; declining rejects it, with an optional comment of 0-128 characters written back
  to that member. Both are decisions about a person's content taken by a machine, with no human
  decision point in the mechanism. Unit 19 already extended 0070 from "who stops being a member"
  to "who becomes one"; this is the same class of power pointed at what a person may publish.
- **Money.** `SuggestedPostInfo` carries an optional `SuggestedPostPrice` in Telegram Stars or
  TON grams, and `SuggestedPostApprovalFailed` exists because approval can fail from "insufficient
  user funds at the time of approval". Approving a priced post takes a member's money. Unit 24
  refused the whole commerce surface, and this is inside it.

### 3. A channel's direct messages chat looks exactly like a group to this adapter

Bot API 9.2's changelog entry, verbatim: "Added the field *is_direct_messages* to the classes
Chat and ChatFullInfo which can be used to identify supergroups that are used as channel direct
messages chats." The `Chat` field itself reads "Optional. True, if the chat is the direct
messages chat of a channel".

This adapter's translation maps `"group" | "supergroup"` to `ChannelKind::Group` in both places
it reads a chat type: the message path (`translate.rs:131-136`) and the membership path
(`translate.rs:205-209`). Nothing reads `is_direct_messages`, because the field was not decoded
when either was written.

Four consequences follow, and they compound:

- **Many people's private threads collapse into one conversation.** Each message in such a chat
  carries `direct_messages_topic`, "Information about the direct messages chat topic that
  contains the message", whose `DirectMessagesTopic.user` is "Information about the user that
  created the topic. Currently, it is always present." One topic per person. The channel-to-
  conversation mapping is keyed on the chat alone (`mapping.rs:45-69`), so every person's private
  thread would share one ledger conversation and one model context. Telegram unit 10 splits a
  forum's topics apart, but it splits on `message_thread_id`; a direct messages topic is a
  separate field with a separate send parameter, and unit 10's split would not reach it.
- **The assistant could not reply.** `sendMessage`'s `direct_messages_topic_id` is documented
  "Identifier of the direct messages topic to which the message will be sent; **required if the
  message is sent to a direct messages chat**". `send_body` sends `chat_id`, `text`, an optional
  `parse_mode` and optional `reply_parameters`, and nothing else (`client.rs:549-571`). Every
  send would be refused by the platform, logged and dropped (`driver.rs:732-760` reasoning, per
  unit 05's reading of the same path).
- **A published statement would stop being true in substance.** The privacy notice says "We
  store the text of each message in a group the assistant belongs to" and "We do not serve direct
  chats: a direct message is rejected and not stored"
  (`bot-assistant-privacy-policy.md:20-24`). A member writing privately to a channel's
  administrators is writing a direct message by every reading a member would give the word. The
  impact assessment's criterion 4 already names direct chats as "data of a highly personal
  nature" and records that they are switched off (`dpia.md:70-72`).
- **The withdrawal path would never fire.** `Assistant::observe` refuses every non-group
  observation with a bare `!=` check that answers `Observed` and appends nothing
  (`assembly.rs:943-946`), and `admit_channel` decides on two sequential `if`s
  (`assembly.rs:1201-1209`). Neither is a `match`, so a third channel kind would be swallowed by
  a catch-all instead of forcing a classification at the two sites that decide admission.

Reachability is the one part this unit does not claim to have proven. The platform documents no
mechanism by which a bot is added to a channel's direct messages chat, and group admission is
fail-closed on the operator's own invitation (`authorization.rs:28-33`, `:60-71`), so the
plausible path is an operator adding the assistant there by hand. The refusal ships anyway: it
costs one decoded field and one enum variant, and being wrong about it costs members' private
correspondence.

## Grounding

### The platform, read from the live documentation on 2026-08-27

Fetched in full from `https://core.telegram.org/bots/api` and `/bots/api-changelog` and searched
locally, not recalled. **The task brief names Bot API 10.1 (June 2026) as current; the changelog
has moved past it** — 10.2 on 14 July 2026 and 10.3 on 24 August 2026 are published. Neither
touches any of the three families below.

**Which version added what, from the changelog's own section headings:**

- **Bot API 9.1, 3 July 2025**, under "Checklists": the classes `ChecklistTask`, `Checklist`,
  `InputChecklistTask`, `InputChecklist`; "Added the field *checklist* to the classes Message and
  ExternalReplyInfo"; "Added the class ChecklistTasksDone and the field *checklist\_tasks\_done*
  to the class Message, describing a service message about status changes for tasks in a
  checklist (i.e., marked as done/not done)"; "Added the class ChecklistTasksAdded and the field
  *checklist\_tasks\_added* to the class Message"; "Added the method sendChecklist, allowing bots
  to send a checklist **on behalf of a business account**"; and the same sentence for
  `editMessageChecklist`. The same version's "General" section — not its checklist section —
  added `DirectMessagePriceChanged`.
- **Bot API 9.2, 15 August 2025**, under "Checklists": `ReplyParameters.checklist_task_id` and
  `Message.reply_to_checklist_task_id`. Under "Direct Messages in Channels":
  `Chat.is_direct_messages`, `ChatFullInfo.parent_chat`, `DirectMessagesTopic` with
  `Message.direct_messages_topic`, and the `direct_messages_topic_id` parameter on twenty send,
  copy and forward methods. Under "Suggested Posts": `SuggestedPostParameters` with the
  `suggested_post_parameters` parameter on eighteen methods, `approveSuggestedPost`,
  `declineSuggestedPost`, `can_manage_direct_messages` on `ChatMemberAdministrator`,
  `ChatAdministratorRights` and `promoteChatMember`, `Message.is_paid_post`,
  `SuggestedPostPrice`, `SuggestedPostInfo`, and the five service-message classes
  `SuggestedPostApproved`, `SuggestedPostApprovalFailed`, `SuggestedPostDeclined`,
  `SuggestedPostPaid`, `SuggestedPostRefunded`.
- **Bot API 9.3, 31 December 2025**: "Added the field *completed\_by\_chat* to the class
  ChecklistTask" — the only checklist change in that version.
- **Bot API 9.6, 3 April 2026**: "Allowed “date\_time” entities in checklist title, checklist
  task text, TextQuote, ReplyParameters quote, sendGift, and giftPremiumSubscription."

**Two corrections to the way these features are usually summarised, both material here:**

1. **`checklist_tasks_done` and `checklist_tasks_added` are not update types.** The changelog
   adds them as fields "to the class Message", and the live `Update` class carries neither. They
   arrive inside an ordinary `message` update as service messages. The same holds for all five
   `suggested_post_*` service messages. **No change to `allowed_updates` can turn either family
   on or off**, so `CONSUMED_UPDATE_TYPES` (`client.rs:170`) is not a mechanism that touches this
   unit at all, and a spec that added names there would be adding names the platform rejects.
2. **`DirectMessagePriceChanged` arrived in 9.1, not 9.2 or 9.3.** It sits in 9.1's "General"
   section, one heading below the checklist block. The class describes "a change in the price of
   direct messages sent to a channel chat" and carries `are_direct_messages_enabled` and
   `direct_message_star_count`.

**The method surfaces, as the live reference states them:**

- **`sendChecklist`** — "Use this method to send a checklist on behalf of a connected business
  account. On success, the sent Message is returned." Parameters: `business_connection_id`
  (String, **Required: Yes**), `chat_id` (Integer or String, Yes, "Unique identifier for the
  target chat or username of the target bot in the format @username"), `checklist`
  (`InputChecklist`, Yes), and the optional `disable_notification`, `protect_content`,
  `message_effect_id`, `reply_parameters`, `reply_markup`. There is no `message_thread_id`, no
  `direct_messages_topic_id` and no supergroup form of `chat_id`.
- **`editMessageChecklist`** — "Use this method to edit a checklist on behalf of a connected
  business account. On success, the edited Message is returned." Required:
  `business_connection_id`, `chat_id`, `message_id`, `checklist`. Optional: `reply_markup`.
- **`approveSuggestedPost`** — "Use this method to approve a suggested post in a direct messages
  chat. The bot must have the 'can\_post\_messages' administrator right in the corresponding
  channel chat. Returns True on success." Parameters: `chat_id` (Integer, Yes, "Unique identifier
  for the target direct messages chat"), `message_id` (Integer, Yes), `send_date` (Integer,
  Optional, "must be not more than 2678400 seconds (30 days) in the future").
- **`declineSuggestedPost`** — "Use this method to decline a suggested post in a direct messages
  chat. The bot must have the 'can\_manage\_direct\_messages' administrator right in the
  corresponding channel chat. Returns True on success." Parameters: `chat_id`, `message_id`,
  `comment` (String, Optional, "Comment for the creator of the suggested post; 0-128
  characters").
- **There is no read-back method.** No `getChecklist`, no `getSuggestedPost`. What a bot knows
  about either object is what an update told it, the same limit unit 05 recorded for polls.

**The object shapes that decide the design:**

- **`InputChecklist`** — `title` "1-255 characters after entities parsing", `parse_mode`,
  `title_entities`, `tasks` ("List of 1-30 tasks in the checklist"), `others_can_add_tasks`
  ("Pass True if other users can add tasks to the checklist"), `others_can_mark_tasks_as_done`.
  **`InputChecklistTask`** — `id` ("must be positive and unique among all task identifiers
  currently present in the checklist"), `text` "1-100 characters after entities parsing",
  `parse_mode`, `text_entities` limited to bold, italic, underline, strikethrough, spoiler,
  custom\_emoji and date\_time. **Neither carries media**: a checklist is text and integers, and
  nothing in this family moves a byte.
- **`ChecklistTask`** — `id`, `text`, `text_entities`, `completed_by_user` (User), and since 9.3
  `completed_by_chat` (Chat), plus `completion_date` ("0 if the task wasn't completed"). **Task
  completion is attributed to a named person.**
- **`ChecklistTasksDone`** — `checklist_message` ("**Optional.** Message containing the checklist
  whose tasks were marked as done or not done"), `marked_as_done_task_ids`,
  `marked_as_not_done_task_ids`. **`ChecklistTasksAdded`** — `checklist_message` (Optional again),
  `tasks`. Both deltas name only what changed; the whole-state snapshot beside them is optional
  and its freshness is not documented.
- **`ChatPermissions.can_send_polls`** and **`ChatMemberRestricted.can_send_polls`** both read
  "True, if the user is allowed to send polls **and checklists**". `ChatPermissions` "Describes
  actions that a non-administrator user is allowed to take in a chat", and `setChatPermissions`
  applies to supergroups. **The platform therefore models checklist-sending as an ordinary
  supergroup member permission**, which is what makes the inbound half of this unit reachable
  while the outbound half is not. Unit 05 already quoted this sentence for its poll half.
- **`SuggestedPostInfo`** — `state`, "Currently, it can be one of “pending”, “approved”,
  “declined”"; `price`; `send_date`. `Message.suggested_post_info`'s own note: "Information about
  suggested post parameters if the message is a suggested post **in a channel direct messages
  chat**. If the message is an approved or declined suggested post, then it can't be edited."
- **`SuggestedPostParameters`** on the send methods is documented "**for direct messages chats
  only**. If the message is sent as a reply to another suggested post, then that suggested post
  is automatically declined."
- **`Message.is_paid_post`** — "True, if the message is a paid post. Note that such posts must
  not be deleted for 24 hours to receive the payment and can't be edited."
- **`SuggestedPostPrice`** — "price in Telegram Stars must be between 5 and 100000, and price in
  nanograms must be between 10000000 and 10000000000000". **`SuggestedPostRefunded.reason`** is
  "post\_deleted" or "payment\_refunded".
- **`DirectMessagesTopic`** — `topic_id` ("This number may have more than 32 significant bits …
  at most 52 significant bits"), `user` ("Optional. Information about the user that created the
  topic. Currently, it is always present.").
- **`leaveChat`** — "Use this method for your bot to leave a group, supergroup or channel.
  Returns True on success." A direct messages chat is a supergroup with its own chat id, distinct
  from the parent channel's, so leaving it cannot leave the channel.

### Our tree

- **Every supergroup is a group.** `translate.rs:131-136` maps `"private"` to
  `ChannelKind::Direct`, `"group" | "supergroup"` to `ChannelKind::Group`, `"channel"` to
  `Skip::ChannelBroadcast`, and anything else to `Skip::UnknownChatKind`. The membership path
  repeats the pair at `translate.rs:205-209`. `Chat` decodes `id` and `type` and nothing else
  (`client.rs:273-278`).
- **A checklist message already records nothing, under a misleading name.** `text_of` reads text
  or caption only (`translate.rs:487-493`), so a message carrying `checklist`,
  `checklist_tasks_done` or `checklist_tasks_added` and no text reaches `Skip::NoText`
  (`translate.rs:167-169`) — the same shape unit 05 found for a member's poll message and renamed
  to `Skip::PollMessage`. The service messages are ordered after the `sender_chat` skip
  (`:161-163`) and the `from` check (`:164-166`), so a service message with no sender reaches
  `Skip::NoSender` first.
- **`Incoming` decodes nine fields and ignores the rest** (`client.rs:192-211`): the decoder's
  own comment says "Unknown fields are ignored by the decoder, so the model stays exactly as
  small as the translation needs". A `checklist_tasks_done`'s nested `checklist_message`, a
  `suggested_post_info`, a `direct_messages_topic` — none of them enters the process as a typed
  value today.
- **`ChannelKind` has two variants and its vocabulary is generated into the schema.**
  `message.rs:29-61` defines `Direct` and `Group`, `ALL` in "stored-encoding order — what closes
  the vocabulary in the schema's CHECK constraint, so the constraint and this enum cannot drift
  apart", and `schema.rs:99-103` quotes `ChannelKind::ALL` into `kind TEXT NOT NULL CHECK (kind
  IN (…))` on the `channels` table. `schema.rs:232` records that "a column CHECK cannot be altered
  in place, so the step recreates the table", with a worked precedent.
- **The two channel-kind decisions are not matches.** `assembly.rs:943-946` is
  `if observation.channel_kind != ChannelKind::Group { … return Observed }`, and `admit_channel`
  (`assembly.rs:1187-1209`) decides on `if … == Group && !is_authorized` then
  `if … == Direct && direct_chats == Off`. A third variant compiles cleanly through both and is
  admitted by the second.
- **Refusal is fail-closed and self-healing.** A group with no authorization row answers
  `IngestOutcome::Withdraw` / `ObserveOutcome::Withdraw`, touching nothing
  (`assembly.rs:1201-1205`, `:956`, `:963`); the driver performs it as `leaveChat` behind a
  per-chat rest window, and a failed leave is "left to the authorization check's self-healing —
  the chat's next contact past the rest re-directs it" (`driver.rs:587`, `:608-625`).
  `IngestOutcome::Disregarded` is the other refusal shape: "Nothing touched the ledger or the
  identity tables, and nothing is delivered — there is no directive to perform"
  (`message.rs:298-309`).
- **Sending carries no topic parameter of any sort.** `send_body` builds
  `{"chat_id", "text"}` plus an optional `parse_mode` and optional `reply_parameters`
  (`client.rs:549-571`).
- **The reply target is the replied-to message id.** `reply_target_of` stores
  `reply_to_message.message_id` (`translate.rs:475-483`, `client.rs:266-269`); nothing reads a
  sub-message identifier of any kind.
- **The refusal list and its scan are unit 19's and unit 22's, and they already exist as
  specified.** `docs/telegram-refused-methods.txt` carries the names, one substring scan runs over
  both crates' `src` directories with the test file outside them, and a negative fixture proves
  the matcher can fail (`19-chat-administration.md:497-508`, `22-business-accounts.md:605-618`).
- **The privacy notice's scope sentences.** "We store the text of each message in a group the
  assistant belongs to" and "We do not serve direct chats: a direct message is rejected and not
  stored" (`bot-assistant-privacy-policy.md:20-24`). The impact assessment's scope is "the
  halogenOS community groups on the platforms it supports (Telegram today), plus direct chats
  between a person and the assistant" (`dpia.md:104-106`), with criterion 4 met for direct chats
  and recorded as switched off (`dpia.md:70-72`).

## Decisions taken with this unit

- **The assistant never creates a checklist, and the refusal rests on the ledger as well as on
  the platform, 2026-08-27.** `sendChecklist` and `editMessageChecklist` are unreachable twice
  over — a required `business_connection_id`, which unit 22 refused whole, and a `chat_id`
  documented without a supergroup form. Neither reason would survive a parameter table changing,
  so the decision also rests on what a checklist is. A checklist is one message whose contents
  other people change: `others_can_add_tasks` and `others_can_mark_tasks_as_done` hand every
  member the ability to edit the assistant's own published message, and the assistant learns of
  the change only through a delta that names the changed task ids, beside an **optional**
  snapshot of undocumented freshness. There is no read-back method. The append-only ledger can
  hold a history of deltas and fold a current state from it — that is what the ledger is for and
  it is not the problem. The problem is that the fold's inputs are not guaranteed: one service
  message missed while the process is down leaves the fold permanently wrong with nothing able to
  detect it, and the assistant would then state a checklist's contents confidently and
  incorrectly. This is unit 05's poll reasoning applied to an object that changes continuously
  instead of once. *Rejected:* creating a read-only checklist with both `others_can_*` flags
  false — the platform still gives no read-back, the assistant would still be publishing a
  message it cannot re-read, and a list nobody may tick is a numbered paragraph, which
  `sendMessage` already delivers; *rejected:* holding the checklist's state in a content table
  and superseding it on each delta — the supersession would be honest about what arrived and
  dishonest about what is true, because a missed delta is invisible; *rejected:* deferring the
  question until business connections are reconsidered — a refusal that is only a consequence of
  another refusal disappears silently when that one is revisited.

- **A member's checklist message and both checklist service messages record nothing, and each
  gets its own named skip, 2026-08-27.** The platform makes checklist-sending an ordinary
  supergroup member permission ("allowed to send polls and checklists"), so all three shapes can
  arrive in the group the assistant serves. All three already record nothing, because decision
  0017 records text and a checklist is not text — but they record nothing under `Skip::NoText`,
  which tells a reader of the log that the member sent an empty message. Three named skips
  replace it: `ChecklistMessage`, `ChecklistTasksDone`, `ChecklistTasksAdded`. This follows unit
  05's `Skip::PollMessage` exactly and for the same reason: a skip should name the case. Nothing
  new is decoded to distinguish them, so the decode stays as narrow as it is today.
  *Rejected:* recording the checklist's title and task texts as message content — it adds a data
  category the record of processing does not carry, the stored copy starts going stale the moment
  a member ticks a task, and the ledger cannot correct it; *rejected:* recording the completion
  events — `ChecklistTask.completed_by_user` and `completion_date` are a per-person activity
  record, a new category of personal data and a new transfer to the model provider, for a feature
  the assistant does not otherwise touch; *rejected:* leaving all three under `Skip::NoText` — it
  is what the tree does today and it misreports what happened.

- **`reply_to_checklist_task_id` is not decoded, and a reply to a task threads to the checklist
  message, 2026-08-27.** Bot API 9.2 lets a member reply to one task inside a checklist. Such a
  message still carries `reply_to_message` pointing at the checklist message, which is what
  `reply_target_of` reads (`translate.rs:475-483`), so the assistant's answer threads to the
  checklist and not to the task. That is the correct behaviour for a system that reads no
  checklists: the assistant has no way to say which task it means, and threading to the whole
  message says exactly as much as it knows. *Rejected:* decoding the field and storing it beside
  the reply target — it would be a stored value with no reader, and unit 03's reading of the
  reply reference already narrowed what is kept.

- **`approveSuggestedPost` and `declineSuggestedPost` are refused for three independent reasons,
  2026-08-27.** The chat is unreachable after the fourth decision below; decision 0070 forbids a
  machine deciding what a person may publish and what comment is written back to them when it
  refuses; and an approved post can move a member's Telegram Stars or TON, which unit 24's
  refusal of the whole commerce surface already covers. Three reasons are recorded instead of one
  because each survives the others being revisited. *Rejected:* a tool that files an assessment
  and lets an administrator approve through the platform's own interface — the administrator
  already has that interface, the assessment would need the post's text, which lives in a chat
  this system refuses, and the assistant would have read private correspondence to produce it;
  *rejected:* refusing `approveSuggestedPost` on money alone and allowing `declineSuggestedPost`
  as harmless — declining is the refusal of a person's contribution, taken by a machine, with a
  comment written in the assistant's voice, which is decision 0070's exact subject.

- **A channel's direct messages chat is a third channel kind, `ChannelKind::PrivateThreads`,
  refused fail-closed with the withdraw directive, 2026-08-27.** The adapter decodes
  `Chat.is_direct_messages` and translates a supergroup carrying it to the new kind on both the
  message path and the membership path. The core refuses that kind everywhere: an observation
  answers `Withdraw`, an ingestion answers `Withdraw`, nothing is mapped, nothing is authorized,
  no principal is resolved and no block is appended. This is modelling and not a platform branch:
  the three kinds are a real domain distinction — one person; many people who see each other; many
  people who do not see each other, each speaking privately to the administrators of the chat —
  and the core's own doc comment states it in those words, with no platform vocabulary in it. The
  cost is named honestly: `ChannelKind::ALL` is quoted into the `channels` table's CHECK
  constraint (`schema.rs:99-103`), and SQLite cannot alter a CHECK in place, so widening the
  vocabulary is an appended migration step that recreates that table, in the form `schema.rs:232`
  already establishes. The value is never written, since the refusal happens before mapping; the
  migration exists so the generated schema and the enum do not drift, which is the property
  `message.rs:36-40` says the CHECK is there to hold. *Rejected:* leaving the mapping at
  `Group` and adding an authorization rule — it fails open by default, since the operator who
  added the assistant is exactly the person whose invitation authorizes a group; *rejected:* a
  boolean field on `InboundMessage` beside the kind — a general mechanism would then carry a flag
  that only one platform's chat shape sets, and illegal pairs become representable; *rejected:*
  answering `Disregarded` instead of `Withdraw` — the assistant cannot reply in such a chat at all
  (`direct_messages_topic_id` is required and `send_body` has no such parameter), so staying gives
  the members nothing while a bot keeps receiving their private correspondence, and leaving is the
  only honest state; *rejected:* refusing in the adapter — an adapter contains no behaviour, and
  the refusal is a decision about what this assistant serves. *Named residual:* whether `leaveChat`
  succeeds in a direct messages chat is not documented. It does not matter to correctness: the
  refusal touches nothing whether or not the leave lands, and a failed leave is re-directed on the
  next contact past the rest window (`driver.rs:613-625`), the same self-healing the authorization
  module already relies on.

- **The two channel-kind decisions become exhaustive matches, 2026-08-27.** `assembly.rs:943-946`
  and `assembly.rs:1201-1209` decide admission with `!=` and with sequential `if`s. Adding a
  variant to a `!=` check silently classifies the new kind with the old catch-all, which is the
  precise mechanism by which a direct messages chat would be admitted anyway. Both sites become a
  `match` over `ChannelKind` with one arm per variant, so the compiler refuses to build until a
  fourth kind is classified at each place that decides who is served. This is the same reasoning
  unit 05 recorded for `provenance.rs:243-249`: a classification that decides admission must be
  visible at the point that decides it. *Rejected:* a single `if kind == PrivateThreads` added
  ahead of each existing check — it fixes today and leaves the same trap for the next kind;
  *rejected:* a helper predicate such as `kind.is_served()` — one boolean over three kinds hides
  which arm each site actually needs, and the two sites do need different answers today
  (`Disregarded` for a direct chat with the switch off, `Withdraw` for an unadmitted group).

- **No update subscription changes, because none of these are update types, 2026-08-27.**
  `checklist_tasks_done`, `checklist_tasks_added` and all five `suggested_post_*` service messages
  are fields on `Message`; the `Update` class carries none of them. `CONSUMED_UPDATE_TYPES`
  (`client.rs:170`) is untouched, and the acceptance criteria assert that it is, with the reason
  written into the assertion so nobody adds a name the platform would reject.
  *Rejected:* naming them in `allowed_updates` defensively — `getUpdates` validates the list, so a
  name outside the vocabulary is a startup failure, not a harmless extra.

- **Four method names join the existing refusal list; no new mechanism is built, 2026-08-27.**
  `sendChecklist`, `editMessageChecklist`, `approveSuggestedPost` and `declineSuggestedPost` go on
  `docs/telegram-refused-methods.txt` with their reasons in the file's comment section, checked by
  the one substring scan units 19 and 22 already specify. *Rejected:* a second scan for this unit
  (two scans over the same sources drift); *rejected:* adding the four names to
  `docs/platform-vocabulary.txt` — that file's own header says a term is "one word of letters and
  digits, exactly as the scan splits source lines", so a compound method name can never match. The
  same objection applies to unit 19's AC4, which is recorded in the notes below instead of edited
  there.

- **Nothing here streams, and the reason is stated, 2026-08-27.** No object in any of these three
  families carries a file: `InputChecklist` and `InputChecklistTask` are text and integers, the
  suggested-post classes are prices, dates and states, and `DirectMessagesTopic` is an identifier
  and a user. The refusals move no bytes either. The streaming constraint binds this unit only as
  a statement of fact, and the fact is recorded so a later author who adds checklist media — if
  the platform ever adds it — knows the constraint applies from the first line: bytes move from
  the platform to disk and from disk into the request in pieces, and the framework's attachments
  store records what an attachment is and which ranges are on disk without carrying the bytes
  itself (`agent-ledger/src/store/attachments.rs:1-4`).

- **No privacy or compliance document changes, and four statements are re-checked, 2026-08-27.**
  This unit adds no data category, no recipient and no stored field; it removes a way for one to
  appear. The four statements re-read against the merged code and left unchanged: the notice's
  "We store the text of each message in a group the assistant belongs to" and "We do not serve
  direct chats: a direct message is rejected and not stored"
  (`bot-assistant-privacy-policy.md:20-24`), which become structurally true instead of
  incidentally true; the impact assessment's scope sentence (`dpia.md:104-106`); and its criterion
  4 record that direct chats are switched off (`dpia.md:70-72`). The operator contract gains one
  paragraph, because the operator is the only person who can put the assistant into a direct
  messages chat and should know it will leave. *Rejected:* adding a risk row for direct messages
  chats to the impact assessment — a refused surface is not a residual risk, and the assessment's
  own review triggers cover a future reversal.

## What this unit examined and deliberately leaves alone

- **`direct_messages_topic_id` and `suggested_post_parameters` on the twenty and eighteen send
  methods.** Both are documented "for direct messages chats only". `send_body` sends neither and
  gains neither; after the fourth decision no such chat is ever mapped, so no send can reach one.
- **`ChatFullInfo.parent_chat` and `ChatFullInfo.is_direct_messages`.** The chat lookup decodes
  `title` and `pinned_message` only (`client.rs:256-259`), and unit 19's AC5 pins that narrowness
  against a full payload. The refusal reads `Chat.is_direct_messages` from the message and the
  membership update, which arrive without an extra call, so the lookup needs no widening. The
  refusal also fires before any lookup would run.
- **`can_manage_direct_messages` on `promoteChatMember` and the two rights classes.** Unit 19
  refuses every write to a chat's administration and names the rights the assistant does not hold;
  this one joins that list by the same reasoning and needs no separate mechanism.
- **`DirectMessagePriceChanged`.** A service message about a channel's own pricing, delivered in a
  channel chat, which `Skip::ChannelBroadcast` already refuses (`translate.rs:134`).
- **The five `suggested_post_*` service messages.** They occur only in a direct messages chat.
  After the refusal, no such chat reaches translation's service-message paths at all, because the
  chat kind is decided first (`translate.rs:131-136`). No named skip is added for them, because a
  skip nothing can reach is a claim the tests cannot check.
- **`Message.is_paid_post`.** Not decoded, and unit 24's refusal of the commerce surface owns the
  question of whether this assistant ever touches a paid message.

## What would have to be true before this is reopened

Nothing below is a decision deferred into legitimacy. The decision is no, today, for the reasons
above. This list exists so a future yes pays its price in the open.

1. **For checklists**, the platform would have to offer a way to read a checklist's current state
   on demand, and `sendChecklist` would have to accept a supergroup without a business connection.
   Until both hold, any stored state is a fold over deltas that cannot be repaired after a gap.
2. **The record of processing would gain two categories** — checklist task text, and the
   attribution of a completion to a named person with its timestamp — and the impact assessment's
   trigger for "a change to what is collected" would fire before the code merges, not after.
3. **For suggested posts**, decision 0070 would have to be revisited on what a machine may decide
   about a person's published contribution, and unit 24's refusal of money would have to be
   revisited separately, because approval and payment are the same act.
4. **For direct messages chats**, five things would each have to be solved: the conversation split
   would need `direct_messages_topic.topic_id` as its address, which is a different field from the
   one telegram unit 10 splits on; sending would need `direct_messages_topic_id` threaded from that
   address to `send_body`; erasure would have to reach one person's thread inside a chat holding
   many people's threads; the privacy notice would have to stop saying direct messages are not
   served, because a member writing privately to channel administrators is writing one; and the
   impact assessment's criterion 4, already met for direct chats, would have to be answered for a
   surface where the assistant reads correspondence addressed to somebody else.

## The unit's contract

After this unit the repository's answer to "can this assistant create a checklist, approve or
decline a suggested post, or serve a channel's direct messages chat" is a recorded no with its
reasoning, and each no is a property of the code instead of an accident of it. The four method
names sit on the one committed refusal list, checked by the one substring scan over both crates'
sources, so none of them can appear in a source file without a check failing with a file and a
line. The three checklist message shapes a member can send into an ordinary supergroup —
a checklist, a task-status service message and a task-added service message — record nothing, as
they do today, and each now reports its own named skip instead of claiming the member sent an
empty message; nothing new is decoded to tell them apart, and the assistant's reply to a message
that replied to a single task threads to the checklist message, which is as much as the assistant
knows. `ChannelKind` carries a third variant for a chat holding many separate private
correspondences, the adapter translates a supergroup marked as a channel's direct messages chat
into it on both the message path and the membership path, and the core refuses that kind
fail-closed with the withdraw directive before anything is mapped, authorized, resolved or
appended — so a chat full of private messages the assistant could never answer is left instead of
read. The two sites that decide which channel kinds are served are exhaustive matches, so a
fourth kind is a compile error at both instead of a silent admission at one. No update
subscription changes, because none of these message shapes is an update type. No privacy or
compliance document changes; four of their statements are re-read and stay true, two of them more
firmly than before. The operator contract gains one paragraph naming the one chat shape the
assistant will leave on its own.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes — `AnsweringMode::Helpful` and the
  addressed-only mode; clippy, fmt and doc under denied warnings; the platform-vocabulary scan and
  the secret scan clean; no new dependency.
- **AC2** `CONSUMED_UPDATE_TYPES` is unchanged and asserted so, with the reason in the assertion's
  comment: `checklist_tasks_done`, `checklist_tasks_added` and the five `suggested_post_*` shapes
  are fields on `Message`, not update types, so no subscription can turn them on or off.
- **AC3** The refusal list carries the four names. `docs/telegram-refused-methods.txt` gains
  `sendChecklist`, `editMessageChecklist`, `approveSuggestedPost` and `declineSuggestedPost`, each
  with its reason in the file's comment section, and units 19's and 22's single substring scan over
  both crates' `src` directories finds none of them. The scan's negative fixture already proves the
  matcher can fail; this unit adds no second scan.
- **AC4** A member's checklist message records nothing and reports `Skip::ChecklistMessage` —
  pinned with a scripted `message` update carrying a `checklist` object, a sender, and no `text`
  and no `caption`. The store holds no block afterwards and no outbound request is made.
- **AC5** Both checklist service messages record nothing and report their own skips: a scripted
  update carrying `checklist_tasks_done` with `marked_as_done_task_ids` reports
  `Skip::ChecklistTasksDone`, and one carrying `checklist_tasks_added` with a `tasks` array
  reports `Skip::ChecklistTasksAdded`. Both fixtures include the optional nested
  `checklist_message`, and the captured log lines for both contain none of the task texts, none of
  the task identifiers and no username from the nested message.
- **AC6** A member's message that replies to a single checklist task threads to the checklist
  message: a scripted update carrying `reply_to_checklist_task_id`, a `reply_to_message` naming
  the checklist message and ordinary text records with its reply target equal to the checklist
  message's identifier, and no field derived from the task identifier is stored — pinned by the
  stored row, not by a claim.
- **AC7** `ChannelKind` has exactly three variants, `ALL` lists them in stored-encoding order, and
  `parse` round-trips each one and answers `None` outside the vocabulary — the existing
  round-trip test at `message.rs:453-456` extended, with `"broadcast"` still refused.
- **AC8** A supergroup marked as a channel's direct messages chat is refused on the message path:
  a scripted `message` update whose `chat` carries `"type": "supergroup"` and
  `"is_direct_messages": true` produces `IngestOutcome::Withdraw`, appends no block, writes no
  `channels` row, writes no `group_authorizations` row, creates no principal, and the adapter
  issues exactly one `leaveChat` and no `sendMessage`. Constructed against the fake server so the
  requests are counted, not asserted about.
- **AC9** The same chat is refused on the membership path, which is the earlier one: a
  `my_chat_member` update adding the assistant to such a chat, sent by the configured operator's
  own account, produces `ObserveOutcome::Withdraw`, writes no `group_authorizations` row, and the
  adapter leaves. The criterion names the operator explicitly, because the failure this prevents is
  precisely the operator's own invitation authorizing the chat.
- **AC10** The refusal is not a per-message skip in the adapter: the same scripted message is
  proven to reach `Assistant::ingest` — the core is what refuses it — by asserting on the returned
  outcome, and the adapter's translation is asserted to yield a `Record`/`Observe` carrying
  `ChannelKind::PrivateThreads` and not a `Skip`.
- **AC11** An ordinary supergroup is unaffected: the same fixtures without `is_direct_messages`,
  and the same fixtures with `"is_direct_messages": false`, translate to `ChannelKind::Group` and
  follow today's admitted path unchanged — the existing group tests pass without modification, and
  one new case pins the explicit-false form, because a decoder that treats `false` as present would
  make the assistant leave the group it serves.
- **AC12** Both channel-kind decisions are exhaustive matches: `assembly.rs`'s observation refusal
  and `admit_channel` each `match` over `ChannelKind` with one arm per variant and no wildcard arm,
  and each arm's outcome is pinned — `Direct` observes nothing, `Group` checks authorization,
  `PrivateThreads` withdraws, on the observation side; `Group` withdraws when unauthorized,
  `Direct` disregards when the switch is off, `PrivateThreads` withdraws unconditionally, on the
  ingestion side. The absence of a wildcard is checked by a source scan over the two functions, so
  a later edit cannot reintroduce one quietly.
- **AC13** The schema migration is appended and idempotent: a fresh database's `channels` table
  CHECK quotes all three kinds; a database created before this unit is migrated by the appended
  step, which recreates the table in the form `schema.rs:232` establishes, preserves every existing
  row, and leaves the widened CHECK; running the migration twice changes nothing. The step is
  registered in `store_config()` in its place in the ordered list.
- **AC14** No source file names a refused method or a refused parameter: the scan of AC3 covers
  `sendChecklist`, `editMessageChecklist`, `approveSuggestedPost`, `declineSuggestedPost`, and the
  criterion states in its own comment what the scan cannot prove — a method name assembled at run
  time from fragments would pass it, which is the same limit unit 24's AC9 records.
- **AC15** No file under `docs/privacy/` or `docs/compliance/` is modified by this unit's diff —
  `git diff` over both directories is empty — and the four statements named in this unit's
  documentation decision are re-read against the merged code and recorded as checked in the merge,
  by file and line.
- **AC16** The operator contract carries the new paragraph: the assistant serves community groups,
  it will leave a channel's direct messages chat on its own if added to one, and the reason is that
  it cannot answer there and will not read correspondence addressed to somebody else. Checked by
  reading the merged document.
- **AC17** Two decision records are written in `docs/decisions/`, each with its date and its
  rejected alternatives: the refusal of checklists and suggested posts, citing 0070 for the
  editorial decision, unit 22 for the business connection and unit 24 for the money; and the third
  channel kind with its fail-closed refusal, citing 0052's admission model and naming the schema
  cost.

## Notes for launch

- Branches from `main`; builds against the current agent-ledger checkout with no framework change.
- Core sites: `message.rs:29-61` gains `ChannelKind::PrivateThreads` with its stored encoding
  `"private_threads"`, its `ALL` entry and its `parse` arm, and the doc comment stating the domain
  distinction in neutral words — one person; many people who see each other; many people who do not
  see each other, each speaking privately to the administrators of the chat.
  `assembly.rs:943-946` and `assembly.rs:1201-1209` become exhaustive matches.
  `schema.rs:99-103`'s generated CHECK widens by construction, with an appended migration step
  after the newest one, modelled on the table-recreation step at `schema.rs:232` and registered in
  the ordered list at `schema.rs:374-396`.
- Adapter sites: `client.rs:273-278` — `Chat` gains `is_direct_messages: Option<bool>` with
  `#[serde(default)]`, read as `== Some(true)` so an absent field and an explicit `false` behave
  identically; `client.rs:192-211` — `Incoming` gains `checklist`, `checklist_tasks_done` and
  `checklist_tasks_added` as presence-only markers, decoded as `Option<serde_json::Value>` or as
  empty structs, whichever keeps the decode narrower, so no task text and no nested message enters
  the process as a typed value; `translate.rs:131-136` and `translate.rs:205-209` — the supergroup
  arm splits on the new field; `translate.rs:42-78` — three new `Skip` variants with their
  comments; `translate.rs:167-169` — the three checklist checks sit ahead of the `text_of` check so
  the named skip wins over `Skip::NoText`, and behind the `sender_chat` and `from` checks, which
  are earlier decisions.
- Documentation sites: `docs/telegram-refused-methods.txt` gains four names and their reasons;
  `docs/reference/group-operator-contract.md` gains the paragraph of AC16; `docs/decisions/` gains
  the two records of AC17.
- Test fixture: the adapter's fake server answers an unknown method with `{"ok":true,"result":true}`
  (`tests/adapter/server.rs:485`, as unit 05 records), so AC8's and AC9's `leaveChat` assertions
  must read the recorded requests and not merely the absence of a failure.
- Two observations about neighbouring specs, recorded here instead of edited there. First, unit
  19's AC4 asks for fifteen method names to be added to `docs/platform-vocabulary.txt` so the core
  scan proves the core names none of them; that file's own header defines a term as "one word of
  letters and digits, exactly as the scan splits source lines", so a compound name like
  `setChatTitle` can never match and the criterion would pass without proving anything. Whichever
  unit merges first should either narrow that criterion to the refused-methods scan, which does
  match substrings, or change the vocabulary scan's matching rule deliberately. Second, telegram
  unit 10 splits a forum's topics into separate conversations on `message_thread_id`; a direct
  messages chat's topics live in a different field with a different send parameter, and this unit's
  refusal is what makes that difference harmless. If the refusal is ever reversed, unit 10's
  address seam is where the second topic namespace belongs, and it needs the `send_body` half as
  well, which unit 10 does not build for it.
