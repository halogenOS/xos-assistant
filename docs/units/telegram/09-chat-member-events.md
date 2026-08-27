# Telegram unit 09 — joining, leaving and changing standing: what the assistant may know, and what it refuses to record

Date: 2026-08-25, revised the same day against two independent reviews. Three platform update
types describe membership: `my_chat_member`, `chat_member` and `chat_join_request`. This unit
reads all three against the operator contract this deployment actually runs under, and the
answer is uncomfortable enough to state first:

- **`chat_member` is unreachable.** It is delivered only to a bot that is an administrator
  of the chat, and it must additionally be named in `allowed_updates`. The operator
  contract requires the assistant to stay a NON-administrator so its reports reach the
  moderation bot. The two requirements are in direct conflict. Same shape as the reaction
  updates in telegram unit 06: specified as a blocked capability, not built, not
  subscribed to.
- **`chat_join_request` is unreachable AND unwanted.** It needs the `can_invite_users`
  administrator right, which the assistant does not have. If it were ever given, the
  capability it unlocks — `approveChatJoinRequest` / `declineChatJoinRequest`, and since
  Bot API 10.1 the join-request query flow — is the assistant deciding who is
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

## What this unit cannot do, stated before the design

Two properties an earlier draft of this specification asserted are not achievable, and the
requirements were changed rather than the claims repeated:

1. **The standing notice cannot be exactly-once while the core holds no standing state.**
   The adapter's loop is at-least-once by contract (`driver.rs:6-12`): a batch that halts
   redelivers the whole update, and the offset advances only on an acknowledged step
   (`driver.rs:321-325`). A notice computed purely from the transition carried in one update
   recomputes the same crossing on a redelivery and sends again. The concrete path exists in
   this unit's own ordering: the notice is delivered by `report` (`driver.rs:583-586`), the
   chat's lookup runs after it, and a transient core failure inside that lookup halts the
   batch (`driver.rs:551-556`). This unit accepts at-least-once delivery for the notice and
   says so in its acceptance criteria, instead of pinning a once-ness it cannot hold. The
   alternative — a persisted standing fact whose only reader is a duplicate check — is
   rejected below with its reasoning.
2. **One status-reading function cannot produce two identical readings.** The membership
   translation needs "outside the member set" as a distinct answer; the administrator cache
   needs absence to mean "an ordinary member" (`authority.rs:60-81`), and its decoded entry
   has no `is_member` field to read at all (`client.rs:242-246`). The shared function is
   specified below with a three-valued answer and two documented readings of it, and the
   acceptance criterion pins the function's answers plus each caller's derivation — not an
   agreement that cannot exist.

A third correction is smaller but changes what the unit claims for itself: revoking a
group's admission on departure closes only the case where the departure arrived and the
re-add did not. An outage longer than the platform's 24-hour retention loses both updates
and leaves the row exactly as it stands today. The revocation is still specified, with that
limit written into the decision instead of an overstated benefit.

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
the chat; for joining by invite link events only"; `via_join_request`;
`via_chat_folder_invite_link`. Note that `from`, `old_chat_member` and `new_chat_member` are
NOT optional on the wire — every membership change names its performer and both sides of the
transition.

**`ChatMember` has six subtypes** — `ChatMemberOwner` (status always "creator"),
`ChatMemberAdministrator` ("administrator"), `ChatMemberMember` ("member"),
`ChatMemberRestricted` ("Supergroups only", carrying `is_member`), `ChatMemberLeft` ("left" —
"isn't currently a member of the chat, but may join it themselves") and `ChatMemberBanned`
("kicked" — "was banned in the chat and can't return to the chat or view chat messages").
**The subtypes grow.** Bot API 10.1 added an optional `tag` "Tag of the member" to
`ChatMemberMember` AND to `ChatMemberRestricted`, and `ChatMemberMember` also carries an
optional `until_date`. A vocabulary that gained fields in the current release cycle can gain
a seventh subtype, and this unit's design has to survive that. Removal and ban are two
different status strings for the same fact from the assistant's point of view: it is outside
the member set.

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
The same release added `User.supports_join_request_queries` and `ChatFullInfo.guard_bot`
("The bot that processes join request queries in the chat"). `approveChatJoinRequest` and
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

**The service message also names the person who acted, in an ordinary field.**
`Message.from`: "Sender of the message; may be empty for messages sent to channels. For
backward compatibility, if the message was sent on behalf of a chat, the field contains a
fake sender user in non-channel chats." On a join by invite link the performer is the joiner
themselves; on a voluntary departure it is the person leaving. This matters for what the
"structurally undecodable" decision below can honestly claim, and the earlier draft claimed
more than it can.

**The assistant's own standing is already readable, without an administrator right, from a
call the adapter already makes.** `getChatAdministrators`: "Use this method to get a list of
administrators in a chat. Returns an Array of `ChatMember` objects", with no administrator
requirement, and its `return_bots` parameter states verbatim: "Pass *True* to additionally
receive all bots that are administrators of the chat. **By default, bots other than the
current bot are omitted.**" The current bot is therefore in the default list whenever it is
an administrator. `getChatMemberCount` states no administrator requirement either.
`getChatMember`, by contrast: "The method is **only guaranteed to work for other users if
the bot is an administrator** in the chat" — there is no per-person standing lookup for
third parties.

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
- **The membership update is decoded minimally, and leniently on purpose.**
  `Update.my_chat_member` (`client.rs:120`), `MemberUpdate { chat, from, old_chat_member,
  new_chat_member }` (`client.rs:162-173`) with both states `Option<MemberState>`
  (`client.rs:171-172`), and `MemberState { status, is_member }` (`client.rs:174-183`). The
  struct's own doc states the reason: "The states decode leniently so a malformed update
  degrades to a skip instead of refusing the batch" (`client.rs:163-164`). The decoder
  ignores unknown fields, so `date`, `invite_link` and the two join-route flags never enter
  the process.
- **The acting person is already decoded for every membership update, including the ones
  that skip.** `MemberUpdate.from` (`client.rs:170`) is filled whatever the transition is;
  today the skip discards it a step later.
- **Only one transition is translated.** `translate_membership`
  (`crates/adapters/telegram/src/translate.rs:203-227`) yields `ObservedFact::Added { by }`
  when `!was_in && is_in`, and returns `Skip::MembershipNotAnEntry` (`translate.rs:211`)
  for everything else — including the assistant's own removal and its own promotion.
  `is_in_chat` (`translate.rs:229-236`) judges membership, not a status pair: "member",
  "administrator", "creator" are in; "restricted" is in exactly when `is_member` is true;
  **anything else is out** — an unknown status string included. That fail-safe answer is
  harmless today because its only consequence is a skip.
- **The status-to-standing mapping already exists a second time, over a narrower type.**
  `AdminCache` maps "creator" to `Authority::Admin` and "administrator" to
  `Authority::Moderator`, and omits everything else so that absence means member
  (`authority.rs:60-81`), with a one-minute cache lifetime (`authority.rs:19`). Its decoded
  entry is `ChatMember { user, status }` (`client.rs:242-246`) — no `is_member`, because
  `getChatAdministrators` never returns the restricted form. Two places read the same
  platform status vocabulary with different needs.
- **The administrator list is fetched for essentially every group message.**
  `authority_for` (`authority.rs:44-82`) is called from the message path
  (`driver.rs:405-408`), so the assistant's own standing — which the platform includes by
  default, per the `return_bots` sentence above — is already crossing the wire at most a
  minute stale.
- **The neutral observation vocabulary.** `Observation { channel, channel_kind, fact }`
  (`crates/core/src/message.rs:218-226`) and `ObservedFact { Title, PinnedAnnouncement,
  Added { by } }` (`message.rs:230-243`). `Added.by` is documented "Absence fails closed: an
  add nobody is named for is nobody's invitation." Both types derive `Debug`.
- **The core judges the add and never stores the adder.** `Assistant::observe`
  (`crates/core/src/assembly.rs:928-963`); the `Added` arm at `:949-960` compares the adder
  against the configured operator through `authorization::operator_admits`
  (`crates/core/src/authorization.rs:28-33`), returns `ObserveOutcome::Withdraw` on any
  mismatch, and otherwise writes the row through `authorization::authorize`
  (`authorization.rs:42-52`). The adder is compared and dropped; no identity row is
  resolved, no block is appended. `identity::resolve_principal`
  (`crates/core/src/identity.rs:59`) is reached only from ingestion.
- **The core cannot currently evaluate "is the report capability configured".**
  `Config.moderation_handle` (`assembly.rs:265`) is destructured in the constructor
  (`assembly.rs:404`) and moved into the report tool at registration
  (`assembly.rs:442-446`); `Assistant`'s fields (`assembly.rs:268-330`) carry `answering`
  (`assembly.rs:284`) but neither the handle nor any boolean derived from it. Any decision
  in `observe` that depends on the report capability needs a field that does not exist yet.
- **The authorization row has no removal path.** `group_authorizations` is
  `(adapter, channel)` with that pair as its primary key
  (`crates/core/src/schema.rs:218-228`); `authorize` is `INSERT OR IGNORE` and
  `is_authorized` is a presence check (`authorization.rs:42-73`). The module states
  "absence is refusal" (`authorization.rs:1-14`). Nothing ever deletes a row, so the
  admission of a group survives the assistant being thrown out of it.
- **An unauthorized group is left, on the message path as well as the observation path.**
  `IngestOutcome::Withdraw` (`assembly.rs:1200-1204`) becomes a real `leaveChat`
  (`driver.rs:444-446`). So deleting an authorization row is not an inert bookkeeping act:
  the group's next message makes the assistant leave.
- **The deterministic outbound seam already exists.** `ObserveOutcome::Observed { deliver:
  Option<DeliveryItem> }` (`message.rs:272-283`), `DeliveryItem { Acknowledgment,
  CommandAnswer }` with a shared `text()` (`message.rs:252-268`), delivered by the adapter
  through `send_item` (`driver.rs:603-609`), which reads only `item.text()` and never the
  variant. A failed send is logged and dropped (`driver.rs:604-607`).
- **Delivery is at-least-once and the halt path is real.** The loop's own doc states it
  (`driver.rs:6-12`); the offset advances only on `Step::Acknowledged` (`driver.rs:321-325`);
  `first_contact`'s doc states "A halted batch records nothing: the update redelivers whole"
  (`driver.rs:530-531`) and a transient core failure inside `report` halts
  (`driver.rs:594-597`).
- **The adapter currently decides an ordering it should not own.** `observed`
  (`driver.rs:479-520`) branches on `!matches!(observation.fact, ObservedFact::Added { .. })`
  (`driver.rs:490`) to choose whether the chat's enrichment lookup runs before or after the
  fact is judged, and the comment above it (`driver.rs:474-477`) states the product reason.
  That is core knowledge living in the adapter, and any new fact adds a branch there —
  exactly the shape the project's invariant forbids.
- **The lookup's own reports bypass that branch entirely.** `first_contact`
  (`driver.rs:532-570`) calls `report` directly for each `Title` and `PinnedAnnouncement`
  the lookup produced (`driver.rs:549-551`). Only a translated update reaches `observed`.
  Any ordering rule attached to a fact must therefore be consulted at `observed` and nowhere
  else, or a lookup-produced `Title` would ask for a lookup that produces another `Title`.
- **Re-entry clears the adapter's per-chat memory, and does it inside the entry branch.**
  The admitted-entry path calls `memories.lookups.void(chat_id)` and
  `memories.withdrawals.forget(chat_id)` (`driver.rs:517-518`) after the core accepted the
  entry; the administrator cache expires within a minute (`authority.rs:19`).
- **The report path's precondition is written down.** Decision 0062 and the group operator
  contract, requirement 3: "**The assistant is NOT a group administrator.** The moderation
  bot ignores administrators' reports, so an administrator assistant files into silence."
  The report tool registers only when a moderation handle is configured and answering is
  helpful — `crate::teaching::moderation_taught` (`assembly.rs:442-446`,
  `crates/core/src/teaching.rs:33-36`). Decision 0062 also rejects "the handle as a core
  constant" by name: "The moderation bot is deployment wiring, not product truth."
- **A service message is dropped today, but after its sender is read.** A
  `new_chat_members` or `left_chat_member` message has no text and no caption, so `text_of`
  (`translate.rs:466-472`) answers `None`; translation reaches that check only past the
  on-behalf-of-chat skip (`translate.rs:160`) and the sender check (`translate.rs:163`), and
  returns `Skip::NoText` (`translate.rs:166`). The pin branch (`translate.rs:138`) is the
  precedent for recognising a service message ahead of both.
- **The platform-vocabulary scan matches whole words only.** `carries_word`
  (`crates/core/tests/vocabulary.rs:62-67`) splits a line on every non-alphanumeric
  character, so a needle containing `_` can never match, and the scan reads its own crate's
  `src` and `tests` directories (`vocabulary.rs:33-45`). Any new scan in this unit has to
  choose its matching rule explicitly and keep its own needles out of the scanned set.
- **The privacy documents already state the boundary.** The record of processing lists
  categories D1 to D9 (`docs/privacy/records-of-processing.md:61-69`) and none of them is a
  membership event; D5, "Derived state", names "group authorization" among its contents
  (`records-of-processing.md:65`). Its data-subject category S1 is "Members of the project's
  community groups whose messages the assistant stores — includes members who never address
  the assistant" (`records-of-processing.md:50`). The impact assessment's necessity section
  closes "Nothing is collected for a purpose beyond the three named" (`dpia.md:340`), and
  its review triggers include "Any capability that touches a person's standing in the group
  — a real moderation decision above the report relay" (`dpia.md:578`). The
  legitimate-interest assessment's section "What is not necessary, and therefore not done"
  (`lia.md:106`) lists what the project declines to collect. The published policy says "We
  store the text of each message in a group the assistant belongs to"
  (`bot-assistant-privacy-policy.md:20`) — a join notice carries no text.

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
  a community, and the effect takes hold before any human sees it. Decision 0070 requires the
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
- **The join and departure member lists are structurally undecodable — and that is all the
  type can promise, 2026-08-25.** `new_chat_members` and `left_chat_member` are added to the
  decoded message as `Option<serde::de::IgnoredAny>`, which records that the field was
  present and discards its contents unread. The people an administrator added, and the
  person an administrator removed, are therefore never in the process's memory and never one
  small edit away from being stored.
  What this does NOT cover, stated because an earlier draft claimed it did: when somebody
  joins by invite link or leaves of their own accord, the platform names them in
  `Message.from`, which this decoder already reads for every message (`client.rs:131`) and
  which `Incoming`'s derived `Debug` would print. That person's identifier is in the decoded
  update exactly as any sender's is, and the service-message skip discards it a few lines
  later without the core ever seeing it. The honest claim is: the member lists are never
  decoded, and the acting person is handled exactly as every other unrecorded sender is —
  read by the decoder, dropped at the skip, never stored.
  *Rejected:* decoding the member lists properly and ignoring them at the call site. It puts
  other people's identifiers into a `Debug`-derived struct that any log line could print.
  *Rejected:* suppressing `Message.from` for service messages specifically. It would mean the
  decoder branching on which service message it holds, to hide a field the same decoder reads
  for every other message and drops just as reliably.
- **The assistant's own membership becomes one fact with the full transition, 2026-08-25.**
  `ObservedFact::Added { by }` is replaced by
  `ObservedFact::OwnStanding { before: Option<Authority>, now: Option<Authority>, by:
  Option<SenderIdentity> }`, where `None` means outside the channel's member set and
  `Some(authority)` is the standing held, reusing the core's existing `Authority`
  vocabulary. Entry is `before: None, now: Some(_)`; departure, removal and ban are all
  `now: None`; a promotion is `before: Some(Member), now: Some(Moderator | Admin)`. The core
  decides per transition; the adapter reports the transition and decides nothing.
  Why one fact instead of three variants: three variants means the core grows one match arm
  per platform edge and the adapter grows a branch per variant, which is the bolted-on
  conditional the engineering standard forbids. One fact carrying before and after accepts a
  transition nobody has thought of yet without a new variant.
  *Rejected:* `Added`, `Removed` and `Promoted` as separate facts. *Rejected:* keeping
  `Added` and adding a second fact for everything else — the same problem, split
  differently.
- **A state the adapter cannot read is a skip, never a departure, 2026-08-25.** This is the
  correction that matters most in this revision. Today `is_in_chat` answers `false` for an
  unknown status string and for an absent state, and the only consequence is
  `Skip::MembershipNotAnEntry` — harmless. Under the transition fact, "out" stops being
  harmless: it revokes the group's admission, and the group's next message then makes the
  assistant leave (`assembly.rs:1200-1204`, `driver.rs:444-446`). A seventh `ChatMember`
  subtype — and the subtypes gained fields in the current release cycle — or one leniently
  decoded malformed payload would silently cost the deployment its group.
  So the platform's status vocabulary is read three ways, not two:

  ```
  Standing::Inside(Authority)   "creator" → Admin, "administrator" → Moderator,
                                "member" → Member, "restricted" with is_member = Some(true)
  Standing::Outside             "left", "kicked", "restricted" with is_member = Some(false)
  Standing::Unreadable          any other status string, and "restricted" with no flag
  ```

  If either side of the transition is `Unreadable`, or either state is absent from the
  update, the whole update is `Skip::MembershipUnreadable`: no fact, no revocation, nothing
  changed, and a warning in the log naming the status string so an operator learns the
  platform grew a shape. The failure direction is deliberate. An unreadable transition that
  was really a departure leaves a row standing that should have gone, which is the state the
  tree is in today; an unreadable transition treated as a departure destroys an admission
  nobody withdrew. Only the first is recoverable without the operator.
  The same rule covers an unreadable `old_chat_member` on what may have been an entry: no
  fact, so no authorization. That is fail-closed in the direction decision 0052 already
  chose — absence is refusal — and it writes nothing it would have to undo.
  *Rejected:* keeping the two-valued reading and treating "unknown" as out. That is exactly
  the inversion above: a fail-safe predicate reused for a destructive effect.
  *Rejected:* treating an unreadable state as "no change" and synthesising `before == now`.
  It invents a fact the update did not carry, and a synthesised "no change" would suppress a
  real crossing.
- **One function reads the platform's status vocabulary, with two documented readings of its
  answer, 2026-08-25.** `standing_of(status: &str, is_member: Option<bool>) -> Standing` is
  the only place in the adapter that matches on a status string. The membership translation
  reads all three answers as above. The administrator cache reads it for a narrower question
  — "which elevated standing, if any" — passing `is_member: None`, because
  `getChatAdministrators` returns only the owner and administrator forms, and keeping an
  entry only for `Inside(Admin)` and `Inside(Moderator)`; `Inside(Member)`, `Outside` and
  `Unreadable` alike leave no entry, and absence continues to mean member exactly as it does
  today (`authority.rs:79-81`). The cache's behaviour is unchanged for every status string
  the platform can put in that list.
  Why not an identical reading in both places: they ask different questions of the same
  vocabulary. The translation must distinguish "outside the member set" from "an ordinary
  member"; the cache must not, because its input can never contain a non-member and its
  absence-means-member rule is what the message path depends on. Sharing the match and
  documenting the two derivations is what stops the vocabulary drifting; pretending the
  derivations are the same would be a claim nothing could falsify.
  *Rejected:* one function returning `Option<Authority>` for both callers. `None` would have
  to mean "outside" for one and "ordinary member" for the other, which is how the two
  readings drift apart in the first place.
  *Rejected:* giving the cache its own copy of the match with a comment asking future
  editors to keep them in step. That is the state this unit is fixing.
- **A departure revokes the group's admission, with a stated limit, 2026-08-25.** On
  `now: None`, `authorization::revoke` deletes the row for that channel and the observation
  returns the observed outcome with nothing to deliver — never the withdraw directive,
  because the assistant is already outside and a leave call would fail. Revoking a channel
  that holds no row is a no-op that deletes zero rows and returns the same outcome, which is
  the common case: the assistant's own withdrawal from an unadmitted group
  (`assembly.rs:1200-1204` → `driver.rs:444-446`) makes the platform send exactly this
  update, and it must not produce a second leave call for a chat already left.
  Why revoke at all: `is_authorized` is a presence check and the row currently outlives the
  membership, so an assistant thrown out of a group by its administrators and re-added later
  by anybody at all is serving a group nobody admitted the second time.
  What it does not achieve, stated plainly because the rule above says nothing here may
  become the sole protection for anything: the entry check at `assembly.rs:955` already
  refuses a stranger's re-add whenever the entry update arrives, so revocation closes only
  the narrower case where the departure was delivered and the re-add was not. An outage
  longer than the platform's 24-hour retention loses both updates, and the row then survives
  exactly as it does today.
  On decision 0052, which this refines rather than contradicts: 0052 called the operator's
  invitation "a durable fact, not a fleeting event", meaning the admission must not depend on
  an event arriving in order to STAY in force. It still does not. What now depends on an
  event arriving is the admission's END, and the failure direction of a lost departure is the
  state 0052 already accepts — the row stands, and the next contact from a stranger-added
  group is refused by the entry check.
  On the append-only record: the ledger is untouched. `group_authorizations` is derived
  state, listed as such in the record of processing under D5
  (`records-of-processing.md:65`), and the table's own stated semantic is that absence is
  refusal — so returning it to absence is not rewriting a history, it is returning a derived
  value to its fail-closed default. Nothing about a person is in the row.
  *Rejected:* a `withdrawn_at` column read as `IS NULL`. It needs a migration step, keeps a
  fact no reader consumes, and expresses the same predicate as absence already does.
  *Rejected:* deleting the channel mapping, the conversation or the group's stored messages
  on departure. Leaving a group is not an erasure request; the erasure route exists and is
  the only thing that removes a person's messages.
  *Rejected:* not revoking at all, on the argument that the benefit is narrow. The narrow
  case is real and the safe direction is cheap; what was wrong in the earlier draft was the
  claim, not the mechanism.
- **A transition other than a departure, in a group with no admission, draws the withdraw
  directive, 2026-08-25.** A promotion, a demotion or a rights change observed in a group the
  operator never admitted is treated exactly as `Title` and `PinnedAnnouncement` already are
  (`assembly.rs:961-963`): the authorization check refuses and the adapter performs the
  rested leave. The departure arm is the single exception, above, because leaving a chat one
  has already left is a call that cannot succeed.
  *Rejected:* letting an unadmitted group's promotion pass silently. It is a group the
  assistant should not be in, and every other observation from it already ends in a leave.
- **An elevated standing is reported to the group in the assistant's own voice, 2026-08-25.**
  When the transition crosses from not-elevated into elevated — `now` above
  `Authority::Member`, with `before` either `None` or `Some(Member)` — and the group is
  admitted (for an entry, admitted by this very observation) and the report capability is
  configured, the observation returns a delivery item carrying a fixed core constant:

  > I have been given administrator rights in this group. Reports from an administrator are
  > ignored, so I cannot pass rule violations on while I hold them. Please take the rights
  > away and leave me an ordinary member.

  It names nobody, states a fact about the assistant itself, asks for an action a human
  takes, and does not name the moderation bot — decision 0062 rejected putting deployment
  wiring into a core constant, and this constant honours that. No moderation effect, no
  standing touched, decision 0070 satisfied by construction.
  Why on the crossing only: a promotion inside the elevated range says nothing new, and a
  demotion needs no announcement. The delta condition is the same on-delta admission the
  rules note uses.
  What the notice is NOT: a protection. It rides `my_chat_member`, which the platform drops
  after 24 hours, so a promotion during a long outage is never observed and the notice never
  fires. Nothing depends on it; the report path being broken while the assistant is elevated
  is a fault a human fixes, and this line is a prompt for that human, delivered when the
  signal happens to arrive. The non-lossy detection — the assistant's own entry in the
  administrator list the adapter already fetches — is recorded as a follow-up below rather
  than built here, because it fires on every cache refill and needs a repetition policy the
  core would have to own.
  Delivery is at-least-once, like every other item on this seam: a redelivered promotion
  update recomputes the same crossing and sends the line again, and a send that fails is
  logged and dropped (`driver.rs:604-607`) with no later retry. Repeating a true request to
  humans who have not yet acted is a tolerable cost; losing one is a tolerable cost too,
  because nothing rests on it.
  *Rejected:* a repeat suppressor, in any of its three shapes. A `LineWindow` bounds a line
  per channel per window and would silence a genuine second promotion after a genuine
  demotion; a stored standing fact would add persisted state whose only reader is a duplicate
  check; a per-process memory in the adapter would put "how often to warn" — a product
  decision — inside the component required to decide nothing.
  *Rejected:* suppressing the report tool while the assistant is elevated. The earlier draft
  rejected it for a reason that is not true — that no other reading of the assistant's own
  standing exists — and the `return_bots` sentence above shows one does. The real reasons:
  the tool is registered per process at assembly and named in each conversation's recorded
  palette (`assembly.rs:442-446`), so live suppression means a palette that changes under a
  running conversation; and a report filed into silence costs nothing, while a capability
  that removes itself quietly costs the humans the one chance of noticing. The notice tells
  somebody; suppression tells nobody.
  *Rejected:* the assistant demoting itself. That is the machine acting on the group's own
  configuration.
  *Rejected:* writing the elevated standing as a context note. A note is permanent in the
  model's system voice, and this is a fault to be fixed, not a fact to be remembered.
- **The core gains a boolean, not the handle, 2026-08-25.** `observe` cannot evaluate
  "is the report capability configured" today: `Assistant` holds `answering` but nothing
  derived from `moderation_handle`. The constructor computes
  `teaching::moderation_taught(moderation_handle.is_some(), answering)` once, stores the
  result on `Assistant`, and the tool registration at `assembly.rs:442-446` reads the same
  value instead of re-deriving it, so the notice and the tool can never disagree about
  whether reporting exists.
  *Rejected:* storing the handle itself on `Assistant`. Nothing in the observation path needs
  the text, and decision 0062 keeps deployment wiring out of the core's product surface.
  *Rejected:* passing the answer in on the observation. Configuration is not a channel fact,
  and `Observation` is documented as read from the channel itself, never from configuration.
- **The enrichment ordering moves into the core, and the seam is named, 2026-08-25.**
  `ObservedFact` gains `enrichment(&self) -> Enrichment` over a closed `Enrichment { Before,
  After, Never }`: `Before` for the group's own text facts, whose event supersedes what the
  lookup would report; `Never` when the fact says the assistant is outside the channel,
  because a lookup against a chat it has left fails and logs a misleading warning; `After`
  otherwise, so an admission is judged before the chat's facts reach the ledger. The adapter
  replaces its variant match at `driver.rs:490` with a three-arm match on the returned value
  and keeps its own lookup scopes, which are its mechanics.
  The seam is `observed` — the arrival of a translated update — and only there. The lookup's
  own reports go straight to `report` (`driver.rs:549-551`) and must keep doing so; consulting
  the enrichment answer inside `first_contact` would make a lookup-produced `Title` ask for
  another lookup. The consequence, stated so the pin does not claim more than it proves:
  `Title`'s answer is unreachable in production today, since no `Title` ever arrives as an
  update. It answers `Before` anyway, because a title-change event, if the platform ever
  delivered one, would supersede the lookup exactly as the pin event does — and because the
  function is total over the enum by construction.
  Accepted limit: the adapter pairs `Before` with its title-only scope and `After` with its
  whole scope, so a future fact needing `Before` at whole scope would need the scope to move
  too. No such fact exists.
  *Rejected:* extending the adapter's `matches!` with one more arm. It keeps a product
  ordering rule inside a component that is required to decide nothing, and the next fact
  pays the same cost again.
  *Rejected:* consulting `enrichment()` at the single `report` call instead of at `observed`.
  Fewer call sites, but it recurses.
- **The admission says so in its own outcome, 2026-08-25.** The adapter's per-chat
  housekeeping — `memories.lookups.void` and `memories.withdrawals.forget`
  (`driver.rs:517-518`) — runs today inside the entry branch, which is about to disappear.
  Hanging it on the `After` answer instead would run it for promotions, demotions and every
  future non-departure fact, voiding the lookup memory and re-fetching a chat nobody's
  admission changed. So the core states the fact instead: `ObserveOutcome::Observed` carries
  `admission: Admission` over a closed `Admission { Unchanged, Recorded }`, and only the arm
  that wrote an authorization row answers `Recorded`. The adapter clears its memory on
  `Recorded` and on nothing else — a core statement performed, not a variant recognised.
  A redelivered entry re-answers `Recorded` (the write is `INSERT OR IGNORE`, so it is
  idempotent) and costs one repeated lookup, exactly as today.
  *Rejected:* riding the `After` answer. Described above: it widens silently.
  *Rejected:* a second outcome variant `Admitted { deliver }`. It duplicates the delivery
  field and its handling at every call site for one bit of information.
  *Rejected:* leaving the housekeeping keyed to the fact variant in the adapter. That is the
  knowledge this unit is moving out.
- **The acting person keeps riding the fact, and the core drops it outside the entry arm,
  2026-08-25.** `OwnStanding.by` is filled for every transition, not only entries. Nothing
  new enters the process: `MemberUpdate.from` is already decoded for every membership update
  including the ones that skip (`client.rs:170`), so the remover's or promoter's identifier
  is in memory today. The core compares it in the entry arm and drops it everywhere else;
  no identity row is resolved and no block is appended, pinned below.
  Why not blank it outside the entry arm: choosing which transitions carry a performer would
  put a data rule inside the adapter, which decides nothing, and the field is the transition's
  own performer — the same category the add already carries under decision 0052, a person
  acting on the assistant's own membership rather than a bystander. The strict standard
  applied to `new_chat_members` above is about people who are not party to the assistant's
  membership at all.
  *Rejected:* dropping `by` from the fact and passing the adder separately. Two shapes for one
  performer.
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
per transition: an entry is admitted only by the configured operator exactly as before and
says in its outcome that the admission was recorded, a departure revokes the group's
admission without a leave call and does nothing when there was no admission to revoke, a
crossing into elevated standing sends one fixed line into the group saying the report setup
needs the assistant to be an ordinary member, any other transition in an unadmitted group
draws the withdraw directive, and everything else changes nothing. A transition the adapter
cannot read — an unknown status string, an absent state — is a named skip that changes
nothing at all, so a platform that grows a seventh member shape costs a log line and not a
group. The acting person is compared and discarded, never stored. Third-party joins and
departures arrive as service messages the platform will not stop sending; their member lists
are never decoded, the adapter names the message and drops it, and no identity, no block and
no table row records that anybody joined or left. No privacy document changes because
nothing new is stored, sent or reached. `chat_member` and `chat_join_request` are not
subscribed to, not decoded and not called: the first is unreachable to a non-administrator
assistant and the second would make the assistant the decider of who may enter a community,
which a settled decision forbids. The enrichment ordering and the per-chat memory reset that
the adapter used to decide now come from the core, so the next observed fact needs no adapter
branch. No new dependency.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings;
  the platform-vocabulary scan and the secret scan clean; no new dependency.
- **AC2** The refusals are pinned by two objective checks. First, an equality assertion that
  `CONSUMED_UPDATE_TYPES` is exactly `["message", "edited_message", "my_chat_member"]`, which
  by construction contains no `"chat_member"` element. Second, a substring scan over the
  `src` directories of both crates for the six names `chat_join_request`,
  `approveChatJoinRequest`, `declineChatJoinRequest`, `answerChatJoinRequestQuery`,
  `sendChatJoinRequestWebApp` and `supports_join_request_queries`, finding none. The scan
  lives in a `tests/` directory so it never reads itself, and it does NOT search for
  `chat_member`: `my_chat_member`, `old_chat_member` and `new_chat_member` are required
  identifiers that contain it, and the repository's own scanner matches whole words only
  (`vocabulary.rs:62-67`), which a needle containing an underscore can never satisfy.
- **AC3** `ObservedFact::Added` no longer exists; `OwnStanding { before, now, by }` carries
  every readable transition, and translation is pinned for: entry as an ordinary member,
  entry directly as an administrator, promotion from member to administrator, promotion from
  administrator to owner, demotion, voluntary departure ("left"), removal and ban ("kicked"),
  and a restricted state with `is_member` true (in, as an ordinary member) and false (out). A
  membership update in a non-group chat is still the named outside-a-group skip.
- **AC4** An unreadable transition changes nothing: an unknown status string on either side,
  a "restricted" state with no `is_member` flag, and an absent `old_chat_member` or
  `new_chat_member` each translate to the new unreadable skip — no observation, and in an
  adapter-level test against an admitted group, no revocation, no leave call and no delivery.
  The retired `Skip::MembershipNotAnEntry` is gone from the enum, its `Display` arm and its
  existing pin (`translate.rs:75`, `:489`, `:844`).
- **AC5** One function matches the platform's status vocabulary. A table test pins
  `standing_of`'s answer for "creator", "administrator", "member", "restricted" with the flag
  true, false and absent, "left", "kicked" and an unknown string. A second test pins the
  administrator cache's derived standing for the same inputs and shows it unchanged from
  today's mapping: elevated for "creator" and "administrator", ordinary member for
  everything else. A reviewer confirms the two elevated status literals appear in exactly one
  place in the adapter's sources.
- **AC6** An entry behaves as before this unit, with one added effect: the configured
  operator's add authorizes the group, its outcome reports the admission as recorded, the
  adapter clears the chat's lookup memory and withdrawal rest, and the chat's enrichment
  follows at whole scope; an add by anybody else, or with no operator configured, or naming
  nobody, returns the withdraw directive and writes nothing. The one added effect is the
  standing notice when the entry is directly into an elevated standing, covered by AC9. The
  existing group-context tests pass with no behavioural change.
- **AC7** A departure revokes: after an `OwnStanding` fact with `now: None` for an admitted
  group, `is_authorized` answers false, a later message from that group is refused with the
  withdraw directive, and a re-add by a stranger stays refused. The observation returns the
  observed outcome with the admission unchanged, not the withdraw directive, and the adapter
  makes no `leaveChat` call for it — pinned at the adapter against the fake server's recorded
  requests. A departure for a group with no authorization row returns the same outcome, makes
  no call and raises no error.
- **AC8** A departure enriches nothing: the adapter issues no `getChat` for a fact whose
  enrichment answer is never — pinned against the fake server's recorded requests.
- **AC9** The elevated notice fires on the crossing and only on it: member to administrator
  sends the fixed line; administrator to owner sends nothing; owner to member sends nothing;
  entry directly as an administrator, admitted by the operator, sends it once. With no
  moderation handle configured, or under addressed answering, nothing is sent in any of those
  cases. A promotion in a group with no authorization row returns the withdraw directive and
  no line. All pinned. Delivery is at-least-once and the criterion says so: a redelivered
  identical update sends the line again, and a test pins that behaviour as accepted rather
  than pinning a suppression that does not exist.
- **AC10** The notice's text is a named core constant, delivered through the existing
  delivery item seam, and the adapter reads only the item's text — pinned by an adapter test
  asserting the sent body without the adapter naming the variant. The constant names no
  handle and no bot.
- **AC11** A join service message and a departure service message each translate to their
  own named skip, ahead of the on-behalf-of-chat skip and the sender check, and produce no
  observation, no ledger write and no identity row — pinned with a payload whose
  `new_chat_members` list carries a full user object DIFFERENT from the message's `from`, and
  a second pin asserting that the decoded message's `Debug` output contains neither that
  listed user's identifier nor their username. The criterion deliberately does not claim
  anything about `from`, which the decoder reads for every message.
- **AC12** No acting person reaches storage: after an entry, a departure and a promotion
  performed by an account that has never sent a message, the identity table holds no row
  created by this path and the ledger holds no block naming it — pinned by asserting the
  table is empty before and after.
- **AC13** The enrichment ordering comes from the core: `ObservedFact::enrichment` is a total
  function pinned over every fact — including the `Title` arm, recorded in the test as
  unreachable in production and pinned for totality — and the adapter's ordering is a match
  on that value with no reference to a fact variant. A second pin: the lookup's own reports
  do not consult it, asserted by a first-contact test that produces a `Title` and shows
  exactly one `getChat` call.
- **AC14** The per-chat memory reset follows the core's statement, not the ordering: a
  promotion in an already-looked-up admitted group triggers no `getChat` (the lookup memory
  survives), while an admitted entry does — both pinned against the fake server's recorded
  requests.
- **AC15** The privacy documents are unchanged by this unit — `git diff docs/privacy/` is
  empty — and four named statements are re-checked against the merged code and remain true:
  the S1 definition (`records-of-processing.md:50`), D5 already covering group authorization
  (`records-of-processing.md:65`), "Nothing is collected for a purpose beyond the three
  named" (`dpia.md:340`), and the stored-message sentence in the published policy
  (`bot-assistant-privacy-policy.md:20`). Recorded as a checked fact in the merge, naming
  those four lines, not as an assumption.

## Notes for launch

- Branches from `main` into its own worktree; the unit is self-contained in the consumer
  repository and needs no framework change.
- Adapter sites: `client.rs:103` (unchanged, pinned by AC2), `client.rs:162-183` (the
  membership structs; `MemberState` already carries what the mapping needs), `Incoming` at
  `client.rs:124-149` (the two `Option<serde::de::IgnoredAny>` fields), `translate.rs:40-76`
  (the two service-message skips and the unreadable skip in, `MembershipNotAnEntry` out),
  `translate.rs:138-166` (the service-message recognition sits immediately after the pin
  branch and before the on-behalf-of-chat skip), `translate.rs:203-236`
  (`translate_membership` and `is_in_chat` become the full transition plus `standing_of`),
  `authority.rs:58-81` (the cache calls `standing_of` and keeps its absence-means-member
  rule), `driver.rs:479-520` (the ordering match and the admission-driven memory reset),
  `translate.rs:489` and `:844` (the retired skip's `Display` arm and its pin).
- Core sites: `message.rs:230-243` (`OwnStanding`, `Enrichment`, `enrichment()`),
  `message.rs:252-258` (the notice's delivery item), `message.rs:272-283`
  (`Observed` gains `admission`; every construction of it in `observe` names one),
  `assembly.rs:268-330` and `:404-446` (the derived report-capability boolean on `Assistant`,
  computed once and read by both the registration and the notice), `assembly.rs:949-963` (the
  transition arms), `authorization.rs` (add `revoke` beside `authorize`, documented as a
  no-op on an absent row), and the notice constant beside the existing fixed lines in
  `outbound.rs:114-155`. `schema.rs` is untouched — the revocation is a delete against the
  existing table.
- Documentation to update in the same merge: the group operator contract gains a short
  section saying that removing the assistant from a group cancels its admission, that
  re-adding it needs the operator's own account again, and that promoting the assistant
  breaks the report setup and produces the notice. Decisions 0052 and 0062 are refined by
  new records, not edited: 0052 gains the revocation and the fail-closed reading of an
  unreadable transition, 0062 gains the detection of its own broken precondition.
- Record one follow-up in `docs/follow-ups.md`: the assistant's own elevated standing is
  visible in the administrator list the adapter already fetches for every group message
  (`authority.rs:44-82`, and the platform's `return_bots` default includes the current bot),
  which is a detection that survives a 24-hour outage where `my_chat_member` does not.
  Resolving it means deciding, in the core, how often a standing read may speak — the reason
  it is not built here.
- Do not resolve the recorded follow-up about the group-to-supergroup migration here. It
  travels on the same service-message path this unit touches and it deserves its own
  reasoning about re-keying the mapping and the authorization; this unit only adds skips
  beside it.
- Telegram unit 06 is the precedent for the blocked-capability sections above and should not
  be edited by this unit's implementer. If its author disagrees with how this unit states
  the administrator conflict, that belongs in a review, not in an edit to their spec.
