# Telegram unit 09 — joining, leaving and changing standing: what the assistant may know, and what it refuses to record

Date: 2026-08-25. Three platform update types describe membership: `my_chat_member`,
`chat_member` and `chat_join_request`. This unit reads all three against the operator
contract this deployment actually runs under, and the answer is uncomfortable enough to state
first:

- **`chat_member` is unreachable.** It is delivered only to a bot that is an administrator
  of the chat, and it must additionally be named in `allowed_updates`. The operator
  contract requires the assistant to stay a NON-administrator so its reports reach the
  moderation bot. The two requirements are in direct conflict. Same shape as the reaction
  updates in telegram unit 06: specified as a blocked capability, not built, not
  subscribed to.
- **`chat_join_request` is unreachable AND unwanted.** It needs the `can_invite_users`
  administrator right, which the assistant does not have. If it were ever given, the
  capability it unlocks — `approveChatJoinRequest` / `declineChatJoinRequest`, and since
  Bot API 10.1 the `guard_bot` join-request query flow — is the assistant deciding who is
  allowed into a community, with no human between the decision and its effect. That
  contradicts decision 0070 outright. This unit refuses to specify it and records why.
- **`my_chat_member` already arrives and is already half-read.** The adapter subscribes to
  it and translates exactly one transition out of it — the assistant entering a group —
  and names everything else a skip. The assistant's own departure and its own promotion
  are on the wire today and thrown away, and both matter.

There is one path to third-party joins and leaves that does not need administrator rights:
the `new_chat_members` and `left_chat_member` service messages, which every bot receives
regardless of privacy mode. So the question "should the assistant know who joined?" is a
real choice and not a platform accident. **This unit answers no**, deliberately and with the
privacy documents cited, and makes the refusal structural instead of a convention.

Everything below is checked against the live Bot API page (Bot API 10.3, 24 August 2026,
fetched 2026-08-25) and against this tree. Every claim carries its source.

## Grounding

### What the platform actually does

**The three update types, verbatim from the `Update` table.**

- `my_chat_member` (`ChatMemberUpdated`): "*Optional*. The bot's chat member status was
  updated in a chat. For private chats, this update is received only when the bot is
  blocked or unblocked by the user." No administrator requirement, no explicit-subscription
  requirement.
- `chat_member` (`ChatMemberUpdated`): "*Optional*. A chat member's status was updated in a
  chat. **The bot must be an administrator in the chat and must explicitly specify
  `"chat_member"` in the list of *allowed_updates*** to receive these updates."
- `chat_join_request` (`ChatJoinRequest`): "*Optional*. A request to join the chat has been
  sent. **The bot must have the *can_invite_users* administrator right** in the chat to
  receive these updates."

**The default subscription excludes exactly one of them.** `getUpdates.allowed_updates`:
"Specify an empty list to receive all update types except *chat_member*,
*message_reaction*, and *message_reaction_count* (default). If not specified, the previous
setting will be used." `WebhookInfo.allowed_updates` repeats the same default set. So
`my_chat_member` and `chat_join_request` are in the default set and `chat_member` is not —
but the "must be an administrator" sentence binds independently of the subscription, so
naming `chat_member` in `allowed_updates` from a non-administrator bot buys nothing.

**`ChatMemberUpdated` carries the before and the after, and who acted.** Fields, verbatim:
`chat` "Chat the user belongs to"; `from` "Performer of the action, which resulted in the
change"; `date` "Date the change was done in Unix time"; `old_chat_member` "Previous
information about the chat member"; `new_chat_member` "New information about the chat
member"; `invite_link` "*Optional*. Chat invite link, which was used by the user to join
the chat; for joining by invite link events only"; `via_join_request` "*Optional*. *True*,
if the user joined the chat after sending a direct join request without using an invite
link and being approved by an administrator"; `via_chat_folder_invite_link` "*Optional*.
*True*, if the user joined the chat via a chat folder invite link". Note that `from` is
NOT optional — every membership change names its performer.

**`ChatMember` has six subtypes** — `ChatMemberOwner` (status always "creator"),
`ChatMemberAdministrator` ("administrator"), `ChatMemberMember` ("member", plus an optional
`tag` "Tag of the member" and an optional `until_date` "Date when the user's subscription
will expire; Unix time"), `ChatMemberRestricted` ("Supergroups only", carrying `is_member`),
`ChatMemberLeft` ("left" — "isn't currently a member of the chat, but may join it
themselves") and `ChatMemberBanned` ("kicked" — "was banned in the chat and can't return to
the chat or view chat messages", `until_date` "If 0, then the user is banned forever").
Removal and ban are two different status strings for the same fact from the assistant's
point of view: it is outside the member set.

**`ChatJoinRequest` and the join-request query flow.** Fields: `chat`; `from` "User that
sent the join request"; `user_chat_id`, whose description ends "The bot can use this
identifier for **5 minutes** to send messages until the join request is processed, assuming
no other administrator contacted the user"; `date`; `bio` "*Optional*. Bio of the user";
`invite_link`; and, added in Bot API 10.1 (11 June 2026, changelog section "Join Request
Queries"), `query_id` "*Optional*. Identifier of the join request query; for bots assigned
to process join requests only. If present, then the bot must call `sendChatJoinRequestWebApp`
or directly call `answerChatJoinRequestQuery` within **10 seconds**."
`answerChatJoinRequestQuery` takes `chat_join_request_query_id` and `result`, the latter
"Must be either "approve" to allow the user to join the chat, "decline" to disallow the
user to join the chat, or "queue" to leave the decision to other administrators."
The same release added `User.supports_join_request_queries` ("*True*, if the bot supports
join request queries and can be assigned to process them. Returned only in `getMe`") and
`ChatFullInfo.guard_bot` ("The bot that processes join request queries in the chat. The
field is only available to chat administrators"). `approveChatJoinRequest` and
`declineChatJoinRequest` both state: "The bot must be an administrator in the chat for this
to work and must have the `can_invite_users` administrator right."

Read plainly: the platform now offers a bot the role of deciding, within ten seconds and
before any human sees it, whether a person may enter a community. The "queue" result is the
only outcome that hands the decision back to people.

**The service-message path needs no administrator rights.** `Message.new_chat_members`:
"*Optional*. New members that were added to the group or supergroup and information about
them (the bot itself may be one of these members)". `Message.left_chat_member`: "*Optional*.
A member was removed from the group, information about them (this member may be the bot
itself)". These arrive inside ordinary `message` updates. The bot features page states, of
privacy mode: bots receive, regardless of the setting, "All service messages." So a
non-administrator assistant with privacy mode off — which is exactly this deployment — sees
every join and every departure in the group whether it wants to or not.

**Member facts a non-administrator can still ask for.** `getChatAdministrators` states no
administrator requirement, and its `return_bots` parameter says "By default, bots other than
the current bot are omitted" — so the assistant's own administrator standing, when it has
one, is already visible in the list it fetches on every message. `getChatMemberCount`
states no administrator requirement either. `getChatMember`, by contrast: "The method is
**only guaranteed to work for other users if the bot is an administrator** in the chat" —
there is no per-person standing lookup available for third parties.

**Updates are not kept forever.** "Incoming updates are stored on the server until the bot
receives them either way, but they will not be kept longer than **24 hours**." A membership
change that happens during an outage longer than a day is lost with no replacement signal,
which is why nothing in this unit may become the sole protection for anything.

### What already exists in this tree

- **The subscription is named on every poll.** `CONSUMED_UPDATE_TYPES` is
  `["message", "edited_message", "my_chat_member"]`
  (`crates/adapters/telegram/src/client.rs:103`), passed as `allowed_updates` in
  `get_updates` (`client.rs:319`) because "an absent selection would inherit whatever an
  earlier setting left on the token". Neither `chat_member` nor `chat_join_request` is in
  it.
- **The membership update is decoded minimally.** `Update.my_chat_member`
  (`client.rs:120`), `MemberUpdate { chat, from, old_chat_member, new_chat_member }`
  (`client.rs:166-176`) and `MemberState { status, is_member }` (`client.rs:178-184`). The
  decoder ignores unknown fields, so `date`, `invite_link`, `via_join_request` and
  `via_chat_folder_invite_link` never enter the process.
- **Only one transition is translated.** `translate_membership`
  (`crates/adapters/telegram/src/translate.rs:203-227`) yields `ObservedFact::Added { by }`
  when `!was_in && is_in`, and returns `Skip::MembershipNotAnEntry` (`translate.rs:211`)
  for everything else — including the assistant's own removal and its own promotion.
  `is_in_chat` (`translate.rs:229-236`) judges membership, not a status pair: "member",
  "administrator", "creator" are in; "restricted" is in exactly when `is_member` is true;
  anything else is out.
- **The status-to-standing mapping already exists a second time.** `AdminCache` maps
  "creator" to `Authority::Admin` and "administrator" to `Authority::Moderator`, everything
  else to absence meaning member (`crates/adapters/telegram/src/authority.rs:61-67`), with
  a one-minute cache lifetime (`authority.rs:19`). Two places now read the same platform
  status vocabulary with the same intent.
- **The neutral observation vocabulary.** `Observation { channel, channel_kind, fact }`
  (`crates/core/src/message.rs:218-226`) and `ObservedFact { Title, PinnedAnnouncement,
  Added { by } }` (`message.rs:230-243`). `Added.by` is documented "Absence fails closed: an
  add nobody is named for is nobody's invitation."
- **The core judges the add and never stores the adder.** `Assistant::observe`
  (`crates/core/src/assembly.rs:928-963`); the `Added` arm at `:949-960` compares the adder
  against the configured operator through `authorization::operator_admits`
  (`crates/core/src/authorization.rs:28-33`), returns `ObserveOutcome::Withdraw` on any
  mismatch, and otherwise writes the row through `authorization::authorize`
  (`authorization.rs:42-52`). The adder is compared and dropped; no identity row is
  resolved, no block is appended. `identity::resolve_principal`
  (`crates/core/src/identity.rs:59`) is reached only from ingestion.
- **The authorization row has no removal path.** `group_authorizations` is
  `(adapter, channel)` with that pair as its primary key
  (`crates/core/src/schema.rs:218-228`); `authorize` is `INSERT OR IGNORE` and
  `is_authorized` is a presence check (`authorization.rs:60-73`). The module states
  "absence is refusal" (`authorization.rs:1-14`). Nothing ever deletes a row, so the
  admission of a group survives the assistant being thrown out of it.
- **The deterministic outbound seam already exists.** `ObserveOutcome::Observed { deliver:
  Option<DeliveryItem> }` (`message.rs:272-283`), `DeliveryItem { Acknowledgment,
  CommandAnswer }` with a shared `text()` (`message.rs:252-268`), delivered by the adapter
  through `send_item` (`crates/adapters/telegram/src/driver.rs:603-609`), which reads only
  `item.text()` and never the variant.
- **The adapter currently decides an ordering it should not own.** `observed`
  (`driver.rs:479-511`) branches on `!matches!(observation.fact, ObservedFact::Added { .. })`
  (`driver.rs:490`) to choose whether the chat's enrichment lookup runs before or after the
  fact is judged, and the comment above it (`driver.rs:474-477`) states the product reason.
  That is core knowledge living in the adapter, and any new fact adds a branch there —
  exactly the shape the project's invariant forbids.
- **Re-entry already clears the adapter's per-chat memory.** The admitted-entry path calls
  `memories.lookups.void(chat_id)` and `memories.withdrawals.forget(chat_id)`
  (`driver.rs:511-514`), and the administrator cache expires within a minute
  (`authority.rs:19`).
- **The report path's precondition is written down.** Decision 0062 and the group operator
  contract, requirement 3: "**The assistant is NOT a group administrator.** The moderation
  bot ignores administrators' reports, so an administrator assistant files into silence."
  The report tool registers only when a moderation handle is configured and answering is
  helpful — `crate::teaching::moderation_taught(true, answering)`
  (`crates/core/src/assembly.rs:442-446`, `crates/core/src/teaching.rs:33`).
- **A service message is dropped today, but anonymously.** A `new_chat_members` or
  `left_chat_member` message has no text and no caption, so `text_of`
  (`translate.rs:466-472`) answers `None` and translation returns `Skip::NoText`
  (`translate.rs:166`) — the same skip a photo without a caption draws. The pin branch
  (`translate.rs:138`) is the precedent for recognising a service message ahead of the
  on-behalf-of-chat skip (`translate.rs:159`).
- **The privacy documents already state the boundary.** The record of processing lists
  categories D1 to D9 and none of them is a membership event; its data-subject category S1
  is "Members of the project's community groups **whose messages the assistant stores** —
  includes members who never address the assistant". The impact assessment's necessity
  section closes "Nothing is collected for a purpose beyond the three named", and its
  review triggers include "Any capability that touches a person's standing in the group — a
  real moderation decision above the report relay — which also reopens the EU AI Act risk
  classification". The legitimate-interest assessment's section "What is not necessary, and
  therefore not done" lists what the project declines to collect. The published policy says
  "We store the text of each message in a group the assistant belongs to" — a join notice
  carries no text.

## Decisions taken with this unit

- **`chat_member` is not subscribed to, and is recorded as blocked, 2026-08-25.**
  `CONSUMED_UPDATE_TYPES` stays a three-element array. The update requires the assistant to
  be an administrator; the report setup requires it not to be. The conflict is not this
  unit's to resolve, and the precondition is written down so a future operator who changes
  the report arrangement knows exactly what becomes available. Nothing about the design
  depends on the update arriving, so nothing breaks if the conflict is resolved later.
  *Rejected:* naming `chat_member` in `allowed_updates` anyway "in case the assistant is
  ever promoted" — the administrator requirement binds separately from the subscription, so
  the subscription would be a statement of intent that changes no behaviour, and a promoted
  assistant is a state this unit actively reports as a fault instead of exploiting.
  *Also rejected:* asking the operator to promote the assistant. That trades a working
  report path for a member roster nothing needs.
- **`chat_join_request` is refused, not deferred, 2026-08-25.** No subscription, no decode,
  no method call to `approveChatJoinRequest`, `declineChatJoinRequest`,
  `answerChatJoinRequestQuery` or `sendChatJoinRequestWebApp`. Two independent reasons, and
  either alone is sufficient. First, the platform: the update needs the `can_invite_users`
  administrator right the assistant does not hold. Second, and the one that survives any
  change to the first: a bot that answers a join request decides whether a person may enter
  a community, and the effect lands before any human sees it. Decision 0070 requires the
  human decision point in the mechanism and rejects "bot-executed actions with post-hoc
  review" by name. The ten-second answering window makes a human-in-the-middle design
  impossible in practice — nobody reviews anything in ten seconds — and the "queue" result
  simply returns the decision to the administrators, meaning the assistant would add a
  round trip and no value. The impact assessment's review trigger for standing-touching
  capabilities would fire, and the AI Act classification would reopen with it.
  *Rejected:* an assessment-only shape, where the assistant reads the request and its `bio`
  and posts an opinion for the administrators. It requires the administrator right anyway,
  it processes the personal data of somebody who is not in the group and has never
  addressed the assistant, and it sends that person's biography to the model provider — a
  new data subject, a new data category and a new purpose, none of which any current
  privacy document covers.
- **Nothing about a third party's join or departure is recorded, 2026-08-25.** The service
  messages are received — the platform delivers them regardless of privacy mode and there
  is no way to decline them — and the adapter recognises them and stops. They become their
  own named skips instead of falling into the generic no-text skip, so an operator reading
  the log can tell "somebody joined" from "a message we could not read". No observation
  crosses the boundary, no core vocabulary grows, no identity row is created, no block is
  appended.
  Why: none of the three recorded purposes needs it. Answering a question does not need to
  know who joined, reading a thread in context does not, and the availability counters key
  on people who speak. Recording it would create personal data about a person who has never
  communicated with the assistant at all — a data subject the record of processing does not
  have a category for, since S1 is defined by messages stored and S2 by direct contact.
  Adding it would mean a new D-category, a new purpose, and a necessity test in the
  legitimate-interest assessment that it fails on its own terms, next to the sentence
  "Nothing is collected for a purpose beyond the three named". The cheapest way to keep a
  published statement true is to keep it true.
  *Rejected:* keeping a membership roster in a side table "for later". Data collected
  because it might one day be useful is the definition of what the necessity test excludes.
  *Rejected:* a member count through `getChatMemberCount` as a group context note. It is a
  group fact, not personal data, and would be permissible, but no purpose asks for it
  and every context note is a permanent line in the model's system voice.
- **The join and departure notices are structurally undecodable, 2026-08-25.** The two
  fields are added to the decoded message as `Option<serde::de::IgnoredAny>`, which records
  that the field was present and discards its contents unread. The joiner's identifier and
  username are therefore never in the process's memory, never in a debug print of the
  decoded message, and never available to a later change by accident. The data minimisation
  is a property of the type, not a rule somebody has to remember.
  *Rejected:* decoding the users properly and ignoring them at the call site. It puts other
  people's identifiers into a `Debug`-derived struct that any log line could print, and it
  leaves the field one small edit away from being stored.
- **The assistant's own membership becomes one fact with the full transition, 2026-08-25.**
  `ObservedFact::Added { by }` is replaced by
  `ObservedFact::OwnStanding { before: Option<Authority>, now: Option<Authority>, by:
  Option<SenderIdentity> }`, where `None` means outside the channel's member set and
  `Some(authority)` is the standing held, reusing the core's existing `Authority`
  vocabulary. The adapter maps the platform's status strings through one shared function —
  the same one `AdminCache` uses — so the two readings of the platform's status vocabulary
  cannot drift apart. Entry is the case `before: None, now: Some(_)`; departure, removal
  and ban are all `now: None`; a promotion is `before: Some(Member), now: Some(Moderator |
  Admin)`. The core decides per transition; the adapter reports the transition and decides
  nothing.
  Why one fact instead of three variants: three variants means the core grows one match arm
  per platform edge and the adapter grows a branch per variant, which is the bolted-on
  conditional the engineering standard forbids. One fact carrying before and after accepts a
  transition nobody has thought of yet without a new variant.
  *Rejected:* `Added`, `Removed` and `Promoted` as separate facts. *Rejected:* keeping
  `Added` and adding a second fact for everything else — the same problem, split
  differently.
- **A departure revokes the group's admission, 2026-08-25.** On `now: None` for an already
  admitted group, `authorization::revoke` deletes the row and the observation returns
  `Observed { deliver: None }` — not `Withdraw`, because the assistant is already outside
  and a leave call would fail. Re-entry then has to be admitted by the operator all over
  again. Why this matters: `is_authorized` is a presence check and the row currently
  outlives the membership, so an assistant thrown out of a group by its administrators and
  re-added a month later by anybody at all is serving a group nobody admitted the second
  time. The entry check at `assembly.rs:955` does re-run on the re-add and catches the
  ordinary case; the row's removal closes the case where the re-add's own update is lost —
  and the platform drops undelivered updates after 24 hours, so that case is real for any
  outage longer than a day.
  On the append-only record: the ledger is untouched. `group_authorizations` is derived
  state, listed as such in the record of processing under D5, and the table's own stated
  semantic is that absence is refusal — so returning it to absence is not rewriting a
  history, it is returning a derived value to its fail-closed default. Nothing about a
  person is in the row.
  *Rejected:* a `withdrawn_at` column read as `IS NULL`. It needs a migration step, keeps a
  fact no reader consumes, and expresses the same predicate as absence already does.
  *Rejected:* deleting the channel mapping, the conversation or the group's stored messages
  on departure. Leaving a group is not an erasure request; the erasure route exists and is
  the only thing that removes a person's messages.
- **An elevated standing is reported to the group in the assistant's own voice, 2026-08-25.**
  When the transition crosses from not-elevated into elevated — `now` above
  `Authority::Member`, with `before` either `None` or `Some(Member)` — and the group is
  admitted and the report capability is configured
  (`teaching::moderation_taught(handle_configured, answering)`), the observation returns
  `Observed { deliver: Some(DeliveryItem::StandingNotice(text)) }` with a fixed core
  constant. Everything else about the assistant's behaviour is unchanged.
  Why: the report path's precondition has silently broken and no other signal exists. The
  humans who can fix it are the humans in the group. The notice names nobody, states a fact
  about the assistant itself, and asks for an action a human takes — no moderation effect,
  no standing touched, decision 0070 satisfied by construction.
  Why on the crossing only: a promotion inside the elevated range says nothing new, and a
  demotion needs no announcement. The delta condition is the same on-delta admission the
  rules note uses.
  Why no repeat window: `LineWindow` was left by the rules acknowledgment because pinning is
  an administrator-only right and the spammer it was built against cannot exist
  (`crates/core/src/window.rs:6-16`). Promotion is administrator-only for the same reason,
  and the crossing condition already suppresses a repeat.
  *Rejected:* suppressing the report tool while the assistant is elevated. It would require
  the core to hold a live standing fact whose only source is an update stream that can be
  lost for 24 hours, so the suppression would be wrong exactly when it mattered, and a
  stale "not elevated" reading is the direction that does damage. *Rejected:* the assistant
  demoting itself. That is the machine taking an action on the group's own configuration.
  *Rejected:* writing the elevated standing as a context note. A note is permanent in the
  model's system voice, and this is a fault to be fixed, not a fact to be remembered.
- **The enrichment ordering moves into the core, 2026-08-25.** `ObservedFact` gains
  `enrichment(&self) -> Enrichment` over a closed `Enrichment { Before, After, Never }`:
  `Before` for the group's own text facts, whose event supersedes what the lookup would
  report; `Never` when the fact says the assistant is outside the channel, because a lookup
  against a chat it has left fails and logs a misleading warning; `After` otherwise, so an
  admission is judged before the chat's facts reach the ledger. The adapter replaces its
  variant match at `driver.rs:490` with a three-arm match on the returned value and keeps
  its own lookup scopes, which are its mechanics. This removes core knowledge that lives in
  the adapter today, and it is why the new fact needs no new adapter branch.
  Accepted limit, stated plainly: the adapter pairs `Before` with its title-only scope and
  `After` with its whole scope, so a future fact needing `Before` at whole scope would need
  the scope to move too. No such fact exists.
  *Rejected:* extending the adapter's `matches!` with one more arm. It keeps a product
  ordering rule inside a component that is required to decide nothing, and the next fact
  pays the same cost again.
- **No privacy document changes, 2026-08-25, and here is the reasoning that must be
  re-checked if the design changes.** No new category of data is stored: the transition
  facts are about the assistant itself, the acting person's identity is compared against the
  configured operator and discarded exactly as the add already does, and the departure
  removes a derived-state row instead of adding one. No new recipient: nothing new reaches
  the model provider, and the standing notice is a message in a group the assistant is
  already in. No new data subject: nothing is recorded about anyone who has not spoken. The
  service messages are received because the platform sends them and the decoder discards
  every field it is not told about — the same relationship the process already has with
  every other field of every update. Were any part of this reversed — a joiner recorded, a
  join-request read, a roster kept — the record of processing would need a new D-category
  and a new S-category note, the legitimate-interest assessment would need a fresh necessity
  test, and the impact assessment's standing-touching review trigger would fire.
  *Rejected:* shipping first and amending the documents afterwards. A published statement
  made false by a merge is a defect in that merge.
- **Nothing here moves bytes, 2026-08-25.** The unit touches no file, no media and no
  upload; the largest thing it handles is one decoded update and one fixed line of text, so
  the streaming constraint has nothing to bind. Stated so a reader does not have to check.

## The unit's contract

The assistant knows its own place in every group it belongs to and nothing about anybody
else's. Its entry, its departure and its promotion arrive as one neutral standing fact
carrying the standing before, the standing after and the acting person, and the core decides
per transition: an entry is admitted only by the configured operator exactly as before, a
departure revokes the group's admission so a re-add must be admitted again, a crossing into
elevated standing sends one fixed line into the group saying the report setup needs the
assistant to be an ordinary member, and every other transition changes nothing. The acting
person is compared and discarded, never stored. Third-party joins and departures arrive as
service messages the platform will not stop sending, and the adapter names them and drops
them with their contents never decoded; no identity, no block, no table row records that
anybody joined or left, and no privacy document changes because nothing new is stored, sent
or reached. `chat_member` and `chat_join_request` are not subscribed to, not decoded and not
called: the first is unreachable to a non-administrator assistant and the second would make
the assistant the decider of who may enter a community, which a settled decision forbids.
The enrichment ordering the adapter used to decide now comes from the core, so the next
observed fact needs no adapter branch. No new dependency.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan and the secret scan clean; no new dependency.
- **AC2** `CONSUMED_UPDATE_TYPES` is exactly `["message", "edited_message",
  "my_chat_member"]` after this unit, asserted by a test, and no source file in either
  crate names `chat_member` as a subscription, `chat_join_request`,
  `approveChatJoinRequest`, `declineChatJoinRequest`, `answerChatJoinRequestQuery` or
  `sendChatJoinRequestWebApp` — pinned by a scan test so a later change has to argue with
  the decision instead of slipping past it.
- **AC3** `ObservedFact::Added` no longer exists; `OwnStanding { before, now, by }` carries
  every transition, and translation is pinned for: entry as an ordinary member, entry
  directly as an administrator, promotion from member to administrator, promotion from
  administrator to owner, demotion, voluntary departure ("left"), removal and ban
  ("kicked"), a restricted state with `is_member` true (in, as member) and false (out), and
  an unknown status string (out). A membership update in a non-group chat is still the
  named outside-a-group skip.
- **AC4** One status-to-standing function serves both the membership translation and the
  administrator cache, and a test pins that the two readings agree for every status string
  in the platform's six-subtype vocabulary.
- **AC5** An entry behaves exactly as before this unit: the configured operator's add
  authorizes the group and the chat's enrichment follows; an add by anybody else, or with
  no operator configured, or naming nobody, returns the withdraw directive and writes
  nothing. The existing group-context tests pass unchanged in behaviour.
- **AC6** A departure revokes: after an `OwnStanding` fact with `now: None` for an admitted
  group, `is_authorized` answers false, a later message from that group is refused with the
  withdraw directive, and a re-add by a stranger stays refused. The observation returns the
  observed outcome, not the withdraw directive, and the adapter makes no leave call for it —
  pinned at the adapter with the fake server asserting no `leaveChat` request.
- **AC7** A departure enriches nothing: the adapter issues no `getChat` for a fact whose
  enrichment answer is never — pinned against the fake server's recorded requests.
- **AC8** The elevated notice fires once on the crossing and only then: member to
  administrator sends the fixed line; administrator to owner sends nothing; owner to member
  sends nothing; entry directly as an administrator sends it once; a repeat of the same
  update sends nothing. With no moderation handle configured, or in addressed answering,
  nothing is sent in any of those cases. An unadmitted group gets the withdraw directive
  and no line. All pinned.
- **AC9** The notice's text is a named core constant, delivered through the existing
  delivery item seam, and the adapter reads only the item's text — pinned by the adapter
  test asserting the sent body without the adapter naming the variant.
- **AC10** A join service message and a departure service message each translate to their
  own named skip, ahead of the on-behalf-of-chat skip, and produce no observation, no
  ledger write and no identity row — pinned with a payload carrying a full user object, and
  a second pin asserting that the decoded message's `Debug` output contains neither the
  joiner's identifier nor their username.
- **AC11** No acting person reaches storage: after an entry, a departure and a promotion,
  the identity table holds no row for the acting account and the ledger holds no block
  naming it — pinned.
- **AC12** The enrichment ordering comes from the core: `ObservedFact::enrichment` is
  exhaustively pinned over every fact, and the adapter's ordering is a match on that value
  with no reference to a fact variant.
- **AC13** The privacy documents are unchanged by this unit, and a reviewer confirms
  against the four of them that no statement became false — recorded as a checked fact in
  the merge, not as an assumption.

## Notes for launch

- Branches from `main` into its own worktree; the unit is self-contained in the consumer
  repository and needs no framework change.
- Adapter sites: `client.rs:103` (unchanged, pinned by AC2), `client.rs:166-184` (the
  membership structs; `MemberState` already carries what the mapping needs),
  `Incoming` at `client.rs:125-144` (the two `Option<serde::de::IgnoredAny>` fields),
  `translate.rs:40-76` (two new skips), `translate.rs:138-166` (the service-message
  recognition sits immediately after the pin branch and before the on-behalf-of-chat skip),
  `translate.rs:203-236` (`translate_membership` and `is_in_chat` become the full
  transition plus the shared standing function), `authority.rs:58-70` (the administrator
  cache calls the shared function), `driver.rs:479-511` (the ordering match).
- Core sites: `message.rs:230-243` (`OwnStanding`, `Enrichment`, `enrichment()`),
  `message.rs:252-258` (`DeliveryItem::StandingNotice`), `assembly.rs:949-960` (the
  transition arms), `authorization.rs` (add `revoke` beside `authorize`), and the notice
  constant beside the existing fixed lines. `schema.rs` is untouched — the revocation is a
  delete against the existing table.
- Documentation to update in the same merge: the group operator contract gains a short
  section saying that removing the assistant from a group cancels its admission and that
  re-adding it needs the operator's own account again, and that promoting the assistant
  breaks the report setup and produces the notice. Decisions 0052 and 0062 are refined by
  new records, not edited: 0052 gains the revocation, 0062 gains the detection of its own
  broken precondition.
- Do not resolve the recorded follow-up about the group-to-supergroup migration here. It
  travels on the same service-message path this unit touches and it deserves its own
  reasoning about re-keying the mapping and the authorization; this unit only adds two skips
  beside it.
- Telegram unit 06 is the precedent for the blocked-capability sections above and should not
  be edited by this unit's implementer. If its author disagrees with how this unit states
  the administrator conflict, that belongs in a review, not in an edit to their spec.
