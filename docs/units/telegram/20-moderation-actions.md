# Telegram unit 20 — the moderation methods: none is wired, and the refusal becomes a property of the code

Date: 2026-08-27. Nine platform methods act on a person's place in a group or on their
words: `banChatMember`, `unbanChatMember`, `restrictChatMember`, `promoteChatMember`,
`setChatAdministratorCustomTitle`, `banChatSenderChat`, `unbanChatSenderChat`,
`deleteMessage` and `deleteMessages`. Decision 0070 settled the question they raise — the
assistant assesses, a human decides — and unit 15 shipped the assessment. This unit does not
reopen either. It writes the platform capability down accurately for the first time, and then
does the part no unit has done: it moves the refusal out of the group's configuration and into
the code.

That distinction is the whole reason for the unit. Seven of the nine methods fail today with a
400 because the assistant is an ordinary member, and the operator contract asks the operator to
keep it one so its reports reach the moderation bot
(`docs/reference/group-operator-contract.md:112-115`). That is the group's configuration
protecting us, not our code. An administrator who promotes the assistant one afternoon, meaning
well, turns `banChatMember` from a refusal into a working call, and nothing in this repository
would notice or object. Telegram unit 09 ships a fixed line asking a human to undo the
promotion; a message is a request, not a boundary.

The answer, then: none of the nine is wired, and after this unit none can be called without a
deliberate edit that fails a test naming decision 0070 until somebody edits the test too. The
one pair the assistant does use — `deleteMessage` and `deleteMessages` on its own messages,
telegram unit 04's subject — is bounded by provenance and not by permission, and this unit
proves that bound in the case unit 04 does not cover: an assistant somebody promoted.

## What this unit cannot promise

Three properties that look reachable and are not. They go before the design so nobody writes
criteria for them.

1. **No test prevents a capability; it makes adding one deliberate and visible.** Somebody who
   wants to call `banChatMember` can add the enumeration variant, delete the list entry and
   edit the failing assertion in one commit. What the mechanism buys is that all three edits
   appear in one diff, beside committed prose saying why the names are there. Telegram units
   04, 08, 09, 10, 11 and 19 buy the same thing with the same shape.
2. **Nothing here detects a promotion, and nothing here needs to.** The protection is that the
   callable set is closed, which holds whether or not any update arrives. Telegram unit 09 owns
   the detection and states its own bound honestly.
3. **The assistant cannot learn what an administrator did with an assessment.** The platform
   offers two routes and both are shut: `chat_member` updates need the bot to be an
   administrator *and* an explicit subscription, and `getChatMember` "is only guaranteed to
   work for other users if the bot is an administrator in the chat". No design in this
   repository may assume a report's outcome is readable.

## Grounding

### The platform, read 2026-08-27

Fetched from `https://core.telegram.org/bots/api` and the changelog at
`https://core.telegram.org/bots/api-changelog` on 27 August 2026. The changelog's newest entry
is **Bot API 10.3, dated 24 August 2026**. The brief for this series named 10.1 (11 June 2026)
as current; the published page is two releases past it, the same finding telegram units 04, 08
and 19 recorded. Every quotation below was read from those pages on that date.

**All seven standing methods require the bot to be an administrator**, in the method's own
words:

| Method | Parameters | The requirement, verbatim |
|---|---|---|
| `banChatMember` | `chat_id`, `user_id`, `until_date`, `revoke_messages` | "The bot must be an administrator in the chat for this to work and must have the appropriate administrator rights." |
| `unbanChatMember` | `chat_id`, `user_id`, `only_if_banned` | "The bot must be an administrator for this to work." |
| `restrictChatMember` | `chat_id`, `user_id`, `permissions`, `use_independent_chat_permissions`, `until_date` | "The bot must be an administrator in the supergroup for this to work and must have the appropriate administrator rights." |
| `promoteChatMember` | `chat_id`, `user_id`, and 18 optional booleans | "The bot must be an administrator in the chat for this to work and must have the appropriate administrator rights. Pass False for all boolean parameters to demote a user." |
| `setChatAdministratorCustomTitle` | `chat_id`, `user_id`, `custom_title` | "for an administrator in a supergroup **promoted by the bot**"; the title is "0-16 characters, emoji are not allowed" |
| `banChatSenderChat` | `chat_id`, `sender_chat_id` | "The bot must be an administrator in the supergroup or channel for this to work and must have the appropriate administrator rights." |
| `unbanChatSenderChat` | `chat_id`, `sender_chat_id` | "The bot must be an administrator for this to work and must have the appropriate administrator rights." |

The matching right for a ban, an unban and a restriction is `can_restrict_members`, "True, if
the administrator can restrict, ban or unban chat members, or access supergroup statistics";
for a promotion, `can_promote_members`; for deleting another person's message,
`can_delete_messages`.

**Six platform facts shape the design, and five of them are traps.**

1. **A ban deletes the member's whole history, and the 48-hour window does not bind it.**
   `revoke_messages` is "Pass True to delete all messages from the chat for the user that is
   being removed. If False, the user will be able to see messages in the group that were sent
   before the user was removed. **Always True for supergroups and channels.**" A community group
   of any size is a supergroup, so the parameter is not a choice there. Banning a member wipes
   every message that member ever sent, however old. The platform's own answer to "remove a
   message older than two days" is therefore "ban its author" — the heaviest act available, and
   a reason no machine should be the one choosing.
2. **A short ban is a permanent ban.** `until_date`: "If user is banned for more than 366 days
   or less than 30 seconds from the current time they are considered to be banned forever.
   Applied for supergroups and channels only." `restrictChatMember` carries the same sentence.
   A ten-second mute produces a permanent one and the API answers `True`.
3. **The undo is itself a removal.** `unbanChatMember`: "By default, this method guarantees that
   after the call the user is not a member of the chat, but will be able to join it. So if the
   user is a member of the chat they will also be removed from the chat. If you don't want this,
   use the parameter `only_if_banned`." There is no safe reversal, only a correctly
   parameterised one, so "it can always be undone" is not available as an argument.
4. **A restriction is a sixteen-field object whose fields imply one another.**
   `restrictChatMember` requires a `ChatPermissions`: `can_send_messages`, `can_send_audios`,
   `can_send_documents`, `can_send_photos`, `can_send_videos`, `can_send_video_notes`,
   `can_send_voice_notes`, `can_send_polls`, `can_send_other_messages`,
   `can_add_web_page_previews`, `can_react_to_messages`, `can_edit_tag`, `can_change_info`,
   `can_invite_users`, `can_pin_messages`, `can_manage_topics`. Unless
   `use_independent_chat_permissions` is passed, "the `can_send_other_messages` and
   `can_add_web_page_previews` permissions will imply the `can_send_messages`,
   `can_send_audios`, `can_send_documents`, `can_send_photos`, `can_send_videos`,
   `can_send_video_notes`, and `can_send_voice_notes` permissions; the `can_send_polls`
   permission will imply the `can_send_messages` permission." A partial mute is not what the
   field names suggest.
5. **`banChatMember` has a live alias.** Bot API 5.3 (25 June 2021): "Renamed the method
   `kickChatMember` to `banChatMember`. The old method name can still be used." A list of
   forbidden names that omits `kickChatMember` misses the ban entirely — the single strongest
   argument in this unit for a positive list of what may be called over a negative list of what
   may not.
6. **The assistant cannot read a moderation outcome back.** `chat_member` is "A chat member's
   status was updated in a chat. The bot must be an administrator in the chat and must
   explicitly specify `chat_member` in the list of `allowed_updates` to receive these updates",
   and `getChatMember` "is only guaranteed to work for other users if the bot is an
   administrator in the chat". Both routes need the standing this project refuses, so a report's
   effect is invisible to us by construction, not by omission.

**The delete pair is different in kind.** `deleteMessage(chat_id, message_id)` lists its own
limitations, verbatim: "A message can only be deleted if it was sent less than 48 hours ago";
"Bots can delete outgoing messages in private chats, groups, and supergroups"; "If the bot is
an administrator of a group, it can delete any message there"; "If the bot has
`can_delete_messages` administrator right in a supergroup or a channel, it can delete any
message there". The outgoing clause needs no right at all; every other clause is an
administrator clause, and note the asymmetry the second and third sentences carry — in a basic
group, being an administrator is enough; in a supergroup, the named right is required.
`deleteMessages(chat_id, message_ids)` takes "a JSON-serialized list of 1-100 identifiers of
messages to delete. See deleteMessage for limitations", and "If some of the specified messages
can't be found, they are skipped." So the delete pair is the one place where a promotion
silently widens what a call can reach, which is exactly why its bound has to sit in the code
that chooses the identifiers.

**Self-demotion is not available.** `promoteChatMember`'s `can_promote_members` is "Pass True
if the administrator can add new administrators with a subset of their own privileges or demote
administrators that they have promoted, directly or indirectly". A bot promoted by somebody
else cannot demote itself even holding the right. Unit 09's line asking a human to remove the
rights is not a matter of manners; it is the only route the platform leaves.

**The family keeps growing.** `setChatMemberTag` and the `can_manage_tags` right arrived in Bot
API 9.5 (1 March 2026); `can_send_welcome_messages` was added to `promoteChatMember` in 10.3 (24
August 2026); `deleteMessages` arrived in 7.0, `deleteMessageReaction` and
`deleteEphemeralMessage` in later releases. Any protection written as a list of rights or a list
of methods to avoid needs editing every release; a list of the calls the assistant makes needs
editing only when the assistant changes.

**Three adjacent methods are named once and left with their owners.**
`setChatMemberTag(chat_id, user_id, tag)` — "set a tag for a regular member … must have the
`can_manage_tags` administrator right", tag "0-16 characters" — labels a named person in the
group and belongs on the refusal list with the seven. `setChatPermissions` restricts every
ordinary member at once and is already refused, with its reasoning, by telegram unit 19.
`deleteMessageReaction` — "The bot must have the `can_delete_messages` administrator right" —
removes another person's reaction and belongs to telegram unit 06. This unit re-specifies none
of the three and edits neither sibling.

**The sender-chat pair aims at a subject this core does not record.** `banChatSenderChat` stops
"the owner of the banned chat" sending "on behalf of any of their channels". Decision 0016 skips
every message sent on behalf of a chat, because recording one would mint a shared principal
standing for several real people. The assistant therefore holds no assessment, no message row
and no vocabulary for a sender chat, and could form no judgement about one even if the method
were reachable.

### Our tree, at `7fb217d`

- **Every Bot API call leaves through one function.** `BotClient::post`
  (`crates/adapters/telegram/src/client.rs:532-545`) takes `method: &str` and builds the URL at
  `client.rs:538`: `format!("{}/bot{}/{method}", self.root, self.token)`. Its wrapper `request`
  (`client.rs:505-530`) adds the rate-limit handling. `self.token` is read at two places today,
  the URL at `client.rs:538` and the redaction at `client.rs:591`. One seam, and the method name
  arrives as a free string.
- **The seven callers are the whole outbound surface.** `getMe` (`client.rs:304`), `getUpdates`
  (`client.rs:313`), `getChat` (`client.rs:334`), `leaveChat` (`client.rs:348`),
  `sendChatAction` (`client.rs:401`), `sendMessage` through `send_body` (`client.rs:439`) and
  `getChatAdministrators` (`client.rs:464`). The module's own summary calls itself "three
  methods over two endpoints and the administrator list" (`client.rs:269-270`), which is four
  calls out of date — evidence that this surface grows without anyone counting it.
- **The poll subscribes to three update types**, `["message", "edited_message",
  "my_chat_member"]` (`client.rs:103`), so no `chat_member` update reaches the process even if
  the assistant were promoted. Telegram unit 09 owns that refusal and its reasoning.
- **The adapter cannot see which rights it holds.** The administrator list decodes into
  `ChatMember { user, status }` (`client.rs:243-248`), so no field of
  `ChatMemberAdministrator` enters the process. `AdminCache` maps "creator" to `Authority::Admin`
  and "administrator" to `Authority::Moderator` and drops every other status
  (`crates/adapters/telegram/src/authority.rs:60-84`); `authority_for` runs for essentially every
  group message (`driver.rs:405-408`).
- **Every directive the core hands an adapter today acts on the assistant itself.**
  `OutboundReply { channel, text, kind, reply_target }` (`message.rs:373`) with `ReplyKind
  { Answer, Notice, Report }` (`message.rs:331`) — the assistant's own words. `DeliveryItem
  { Acknowledgment, CommandAnswer }` (`message.rs:252`) — the assistant's own words.
  `ObserveOutcome::Withdraw` (`message.rs:272`) and `IngestOutcome::Withdraw`
  (`message.rs:289`) — the assistant's own membership. `ComposingUpdate { channel, state }`
  (`message.rs:364`) — the assistant's own presence cue. No directive names a person, and
  nothing in the core says the set is closed.
- **The performing sites in the adapter are equally enumerable**, and each one matches:
  `driver.rs:438-451` and `:580-586` deliver the core's own text, `:604` sends it, `:622` leaves
  the chat, `:716-717` sets the presence cue, `:745` sends the reply. Not one of them names
  another person.
- **The assessment path, end to end.** The prompt teaches the model to judge each group message
  against the pinned rules and file a clear violation
  (`crates/core/src/teaching.rs:48`), composed only when a moderation handle is configured and
  answering is helpful (`teaching.rs:33`). The tool is `report_spam`
  (`crates/core/src/tools/report.rs:192`) at `REQUIRED_AUTHORITY = Authority::Member`
  (`report.rs:198`), taking one parameter — `message_id`, "the violating message, named by the
  id the projection shows in brackets ahead of it" (`report.rs:200-202`) — validated against the
  turn's own assessment set. Its target vocabulary is a message, never a person: the declines
  name messages (`report.rs:218-256`), and the stored principal exists so erasure can reach the
  report row, not so anything can act on the member. The output is one line,
  `REPORT_LINE_LEAD` = `/report@` plus the configured handle (`report.rs:207`, `:265-268`),
  delivered threaded onto the reported message.
- **The one deterministic path with an administrator in it stops at a stored row.**
  `mirror::mirrored_target` (`crates/core/src/mirror.rs:58-70`) recognises the moderation bot's
  `/del` from a sender at or above `ADMINISTRATOR_FLOOR` (`mirror.rs:41`) and returns the
  replied-to origin; the effect is one nulled row and no platform call. The module documentation
  names the administrator as "the human decision of decision 0070" (`mirror.rs:24`).
- **Telegram unit 04 owns the delete pair.** It decides that the assistant deletes only messages
  it sent itself, is never made an administrator and is never granted `can_delete_messages`, and
  that "the bound lives in the code and not in the permission: the origins the core may name for
  deletion come only from its own recorded deliveries"
  (`docs/units/telegram/04-deleting-messages.md:249-263`). What it does not cover is the same
  code with the assistant promoted.
- **Telegram unit 19 already created the shared refusal list.** `docs/telegram-refused-methods.txt`,
  in the form of `docs/platform-vocabulary.txt` — one name per line, `#` comments — scanned over
  both crates' `src` directories, whole alphanumeric runs only, with the merge rule stated:
  "Whichever unit merges first creates the file and the test; the later ones add their names to
  it and delete their own scan in the same commit"
  (`docs/units/telegram/19-chat-administration.md:410-425`). Telegram units 08, 09 and 10 each
  specify a scan of their own and are covered by that same rule.
- **The core's own list is checked separately.** `crates/core/tests/vocabulary.rs` reads
  `docs/platform-vocabulary.txt`, walks the core crate's `src` and `tests`, matches
  case-insensitive whole alphanumeric runs and reports `file:line` (`vocabulary.rs:15`
  `forbidden_words`, `:35` `scanned_files`, `:64` `carries_word`). The list holds platform and
  SDK names today; telegram units
  11 and 19 both add method names to it.
- **The documents already state the boundary and none needs changing.** The public notice: "The
  assistant does not moderate: it cannot warn, remove or ban anybody"
  (`docs/privacy/bot-assistant-privacy-policy.md:75`). The impact assessment's review triggers
  include "Any capability that touches a person's standing in the group — a real moderation
  decision above the report relay" (`docs/privacy/dpia.md:578`), and both moderation addenda
  close with "The warn and ban lines remain held out, and their shipping remains a review
  trigger" (`dpia.md:666-667`, `:729-730`). The AI Act record: "Its one standing-adjacent
  capability, the report relay, is an assessment a human judges"
  (`docs/compliance/ai-act.md:41-44`).
- **The operator contract is where the rights live.** Requirement 3: "**The assistant is NOT a
  group administrator.** The moderation bot ignores administrators' reports, so an administrator
  assistant files into silence. Keep the assistant an ordinary member and turn its privacy mode
  off instead of promoting it" (`group-operator-contract.md:112-115`). One reason is given and
  it is operational. After this unit there is a second that does not depend on how the
  moderation bot behaves.
- **Documentation pins have an established home.** `crates/assistant/tests/docs.rs` reads
  committed files and asserts named substrings (`docs.rs:713-765` for the deletion mirror's
  three document updates).
- **A platform refusal arrives without its text.** `BotClient::decode` reduces a non-success
  HTTP status to its code (`client.rs:565-570`), and these methods refuse with a 400, so the
  "not enough rights" description is lost until the recorded follow-up is resolved
  (`docs/follow-ups.md:13`). Nothing in this unit needs the description and this unit does not
  fix it.

## Decisions taken with this unit

- **None of the nine methods is wired, and the callable set is closed at the type level,
  2026-08-27.** Decision 0070 forbids a moderation effect without a human decision point in the
  mechanism, and names "any future administrative tool — a warn, a ban, a mute" as shipping only
  behind an approval that precedes the effect. No such mechanism exists here and none is built.
  What is built is the property that makes the refusal survive the next person: `post`
  (`client.rs:532-545`) stops taking a `&str` and takes a closed enumeration of the calls the
  assistant makes, one variant per call, each carrying its wire name and a documentation line
  naming the unit that added it. An arbitrary method name stops being one typo away and becomes
  a type error. *Rejected:* a comment above `post` listing forbidden methods — the "enforcement
  by prompt alone" shape decision 0070 rejects, moved one level down. *Rejected:* a list of
  forbidden names as the only protection — the platform still accepts `kickChatMember`, added
  `setChatMemberTag` in 9.5 and a new right in 10.3, so a negative list ages out of date every
  release while a positive one ages only when the assistant changes. *Rejected:* a configuration
  key switching moderation off — a switch that can be flipped is not a boundary, and it would put
  a product decision inside the component required to decide nothing.
- **The set is pinned by an exact-list assertion whose failure message quotes the decision,
  2026-08-27.** The enumeration makes crossing the line deliberate; it does not make it visible.
  A `Method::ALL` slice and a test asserting its wire names exactly turn a new variant into a
  failing test whose message states that the set is closed by decision 0070 and that widening it
  means reopening the decision first. The next person meets the reasoning at the moment they
  would not otherwise look for it. Each sibling unit in this series that adds a call updates
  that list in its own commit; the churn is the mechanism working, because a new outbound call is
  exactly what a reviewer should be made to see. *Rejected:* asserting the number of variants — a
  swap keeps the count. *Rejected:* asserting only that the nine names are absent — that says
  nothing about a tenth method nobody has thought of, and the closed list says the stronger thing
  for the same cost.
- **The eight names join telegram unit 19's list; no second list is created, 2026-08-27.**
  Unit 19 specifies `docs/telegram-refused-methods.txt` and one scan over both crates, and states
  the merge rule for the units that also refuse names (`19-chat-administration.md:410-425`). A
  second file for the same class of property is one decision recorded twice, and the second copy
  is the one that will be forgotten. This unit contributes eight names — `banChatMember`,
  `kickChatMember`, `unbanChatMember`, `restrictChatMember`, `promoteChatMember`,
  `setChatAdministratorCustomTitle`, `banChatSenderChat`, `unbanChatSenderChat` — plus
  `setChatMemberTag`, each with a comment naming this unit. `setChatPermissions` is unit 19's and
  is not claimed twice. `deleteMessage` and `deleteMessages` are deliberately absent, and the
  file says so where the names would sit: unit 04 ships `deleteMessages` for the assistant's own
  messages, so a needle for it would fail on a sibling's own diff, and their bound is provenance,
  written in code, not a name that must not appear. *Rejected:* a list of this unit's own —
  see above. *Rejected:* omitting `kickChatMember` as an obsolete spelling — the platform still
  accepts it, so a list without it does not refuse banning at all. *Rejected:* substring
  matching — `getChatMember` would match `getChatMemberCount`, and unit 19 already decided
  whole-run matching for that reason.
- **The enumeration and the list are both kept, because each catches what the other cannot,
  2026-08-27.** They are not two mechanisms for one property. The enumeration prevents the call:
  a method name that is not a variant cannot reach `post`. The list prevents the name: it fails on
  any occurrence in either crate, including a fixture, a comment or a test that would normalise
  the vocabulary. Together they interlock — adding `banChatMember` as a variant puts the literal
  in `client.rs`, where the scan fails, so the person must also delete the list entry, beside a
  committed file that says why it was there. *Rejected:* the enumeration alone — it says nothing
  about a second HTTP client built beside `BotClient`. *Rejected:* the list alone — it catches
  names, not intent, and nothing about a method assembled at run time.
- **The core states, in neutral words, that no directive it hands an adapter acts on another
  person — and pins it, 2026-08-27.** The adapter is not where a future change would really
  cross this line. The realistic path is a core directive — a `DeliveryItem::Silence
  { principal }`, an `IngestOutcome::Restrict` — that reads as neutral, passes the
  platform-vocabulary scan, and leaves an adapter author with no honest choice but to call
  `restrictChatMember`. So the invariant lives where the vocabulary lives, in the core, and it is
  stated as a property of the directive types: **the core may ask an adapter to make the
  assistant speak, mark or take back its own words, leave a channel, or show its presence cue; it
  may never ask an adapter to change what another person may do, may see, or whether they stay in
  the channel, and no directive names a person as the object of an act.** It goes on the
  `message.rs` module documentation and is pinned by an exact-variant assertion over
  `ReplyKind`, `DeliveryItem`, `ObserveOutcome` and `IngestOutcome` whose failure message states
  the invariant and names the decision. Unit 04's retraction satisfies it — the assistant taking
  back its own message. Unit 06's reaction satisfies it — the assistant's own mark, placed on a
  message it can see, changing nothing a person may do. A variant naming a principal does not.
  *Rejected:* a marker trait implemented by each directive type — machinery for a property that
  is four short enumerations, and a trait can be implemented for the wrong type as easily as a
  variant can be added. *Rejected:* documenting the invariant without the pin — it would hold
  until the first person who did not read the module documentation. *Rejected:* checking it in
  the adapter — the adapter decides nothing, and an invariant about the core's vocabulary that
  lives in one adapter is untrue the moment a second adapter exists. *Rejected:* a looser pin
  that only forbids a person identity as a field — it would pass a `Restrict { channel }` that
  silences everyone.
- **The delete pair keeps unit 04's bound, and this unit pins the case unit 04 leaves open,
  2026-08-27.** Unit 04 decides that the bound is provenance and not permission, and its criteria
  prove the ordinary case: an administrator's `/del` on a member's message nulls a stored row and
  issues no platform request. What none of them proves is the same case with the assistant
  promoted, which is precisely when `deleteMessage`'s administrator clause would widen the reach.
  This unit adds that assertion and nothing else on the delete path: with the scripted
  administrator list naming the assistant itself, an administrator's `/del` replying to a
  member's message produces the same nulled row and the same absence of any delete request. The
  behaviour does not change with the group's configuration, which is the entire claim.
  *Rejected:* refusing the deletion when the assistant is an administrator — behaviour that
  changes with the group's configuration is worse than behaviour that does not, and unit 04's
  capability would vanish silently on the day somebody promotes the bot. *Rejected:*
  re-specifying the retraction path here — it is unit 04's, and a second description of one
  mechanism is how two descriptions drift apart.
- **A promotion is reported and never used; no detection is built here, 2026-08-27.** Telegram
  unit 09 sends one fixed line into the group when the assistant crosses into an elevated
  standing and records the honest bound: it rides `my_chat_member`, so it is a prompt for a human,
  not a protection. That division is right and this unit keeps it — the protection is the closed
  set, which depends on no update arriving. Two platform facts are recorded here because they
  bear on it and are not in unit 09: the assistant cannot demote itself, since
  `promoteChatMember` only demotes administrators the caller promoted, so asking a human is the
  only route the platform leaves; and the adapter cannot tell which rights a promotion carried,
  because the administrator list decodes as `ChatMember { user, status }` (`client.rs:243-248`).
  *Rejected:* decoding `ChatMemberAdministrator`'s rights so the notice could name what was
  granted — the field set gained members twice in six months, so the decode would need editing
  every release, and the answer changes nothing anybody does: any elevated standing is the fault
  to fix, whichever rights came with it. *Rejected:* a second notice from this unit when a
  moderation right is seen — the same line from two units, one of them firing on a decode this
  unit just refused.
- **What an administrator can act on from an assessment is written into the operator contract,
  2026-08-27.** The report line is the capability's whole output, and every method above belongs
  to the moderation bot's token, not the assistant's. The contract's requirement 3 gains a second
  reason that does not depend on the moderation bot's behaviour, and a named list of rights the
  assistant must never be granted: `can_restrict_members`, `can_delete_messages`,
  `can_promote_members`, `can_invite_users` and `can_manage_tags`. It also gains the one
  consequence an operator most needs before acting on an assessment: **banning a member in a
  supergroup removes every message that member ever sent from the chat, because `revoke_messages`
  is always true there, and removes nothing at all from the assistant's store** — no deletion
  update reaches a bot in a group (telegram unit 04), so the divergence is silent, and clearing
  the store is the erasure route's job. *Rejected:* leaving the mass revoke unwritten — the
  operator is the person who acts on our assessments, this is the largest effect one act can have,
  and it is on no screen they will see. *Rejected:* putting it in the public notice — the notice
  already tells members the assistant cannot ban anybody and that a deletion in the chat does not
  reach the store; this is operational wiring for the person holding the rights.
- **No privacy or compliance document changes, and four published statements are re-read instead,
  2026-08-27.** Nothing new is processed, stored, sent to the model provider or disclosed to a
  recipient: the unit adds a type, list entries, tests and two documents, and removes no
  capability. None of the impact assessment's review triggers fires, because no capability
  touching a person's standing ships — the trigger is written for a capability, and an absence
  made checkable is not one. The four statements that move from true-by-absence to
  true-by-construction are named and re-read against the merged code: the public notice's "The
  assistant does not moderate: it cannot warn, remove or ban anybody"
  (`bot-assistant-privacy-policy.md:75`); the impact assessment's standing-capability trigger
  (`dpia.md:578`) and its twice-stated "The warn and ban lines remain held out" (`dpia.md:666-667`,
  `:729-730`); and the AI Act record's standing-adjacent paragraph (`ai-act.md:41-44`). The ban's
  mass revoke is checked against the impact assessment's existing sentence on the moderation
  bot's bulk purges and found covered, which is why that document is read and not edited.
  *Rejected:* amending the impact assessment to record that the refusal is now structural — that
  document describes processing and its risks, and a note that a test exists for an absence is
  the padding that makes real entries harder to audit. *Rejected:* rewriting the "held out"
  sentences as "refused" — later statements refine earlier ones and these need no refining; the
  unit's job is to keep them true, not to restate them more loudly.
- **Nothing streams, and the append-only record is untouched, 2026-08-27.** No bytes move: the
  unit adds no file handling, no upload and no download, and the one place the streaming
  constraint touches this subject is unit 04's batched deletion, which walks recorded identifiers
  in batches of at most 100 without assembling a larger body — unit 04's decision, restated here
  only as the reason the enumeration would carry the batch method and not the single one. Nothing
  is appended and nothing supersedes: refusing to call a method is not a fact about a
  conversation. One consequence of the platform reading does belong on the record and is already
  expressed the way unit 04 chose — after a ban the chat and the ledger diverge wholesale and
  permanently, and the ledger records what was said, never what is still visible.

## What would have to be true before this is reopened

Refusing without naming what could work is refusing without examining. Decision 0070 leaves one
door open by name — "the moderation bot's review queue is the known shape" — and this is the
checklist for anyone walking through it.

1. **A human approval that precedes the effect, in the mechanism.** Not a notification, not an
   undo, not a review afterwards: decision 0070 rejects "bot-executed actions with post-hoc
   review" in terms. The concrete artefact is a queue an administrator reads and confirms, with
   the platform call issued from that confirmation and from nowhere else.
2. **The rights granted deliberately, and the report path replaced.** Every method above needs
   the assistant to be an administrator, and requirement 3 of the operator contract says an
   administrator assistant's reports are ignored by the moderation bot. Granting the rights
   breaks the assessment path that would feed the queue, so the two cannot both be true as they
   stand.
3. **The five traps designed for explicitly**, not discovered in production: `revoke_messages`
   always true in supergroups, `until_date` under 30 seconds meaning forever, `unbanChatMember`
   removing a member unless `only_if_banned` is passed, `ChatPermissions` fields implying one
   another unless `use_independent_chat_permissions` is set, and `kickChatMember` still working
   as a name.
4. **An answer to the read-back problem.** Neither `chat_member` nor `getChatMember` works for
   other users without the standing this project refuses, so any design needing a member's
   current state must say where that knowledge comes from.
5. **The documents moved before the code merges.** The impact assessment's standing-capability
   trigger fires and reopens the AI Act classification with it (`dpia.md:578`,
   `ai-act.md:41-52`); the public notice's sentence at `bot-assistant-privacy-policy.md:75`
   becomes false on the day of the merge and is the first thing rewritten, because it is the
   promise made to members, not to an auditor.

Nothing above defers the decision. The decision is no, today, for the reasons in the previous
section; this list exists so a future yes pays its price in the open.

## The unit's contract

The assistant calls no platform method that acts on a person's standing or on another person's
words, and after this unit it cannot begin to without a deliberate edit that fails a test naming
decision 0070. The adapter's one outbound seam stops accepting a method name as a string and
accepts a closed enumeration of the calls the assistant makes, pinned by an exact-list
assertion; eight administrative method names and `setChatMemberTag` join the shared refusal list
telegram unit 19 specifies, scanned over both crates, with `kickChatMember` among them because
the platform still answers to it, and with the delete pair deliberately absent and the reason
written in the file; the same test states the named sites where the token-bearing URL is built,
so a second route to the platform is an edit somebody has to make on purpose. The core states on
its message module, and pins by exact variant sets, that it may ask an adapter to make the
assistant speak, mark or take back its own words, leave a channel or show its presence cue, and
may never ask it to change what another person may do, may see, or whether they stay — which is
the shape a future standing capability must break before any adapter could carry it. The delete
pair keeps the bound telegram unit 04 gave it, provenance and not permission, proved in the one
case unit 04 does not cover: a promoted assistant still deletes nothing but its own messages, and
answers, assesses and reports exactly as an ordinary member. The assessment path is unchanged —
the model judges, the report line reaches the moderation bot, the administrators decide with
their own token. The operator contract gains the rights that must never be granted and the
consequence of a ban: every message the banned member ever sent leaves the chat, and none of it
leaves the store. No privacy or compliance document changes, because nothing new is processed;
four published statements are re-read and recorded as still true. No new dependency, no new
configuration entry, and no behaviour any member can observe.

## Acceptance criteria

1. **AC1** Workspace suite green in both answering modes; clippy, fmt and doc under denied
   warnings; the platform-vocabulary scan and the secret scan clean; no new dependency and no new
   configuration entry. The only change under `crates/core/src/` is documentation on
   `message.rs`; every other core change is a test.
2. **AC2** The callable set is closed at the type level: `post` (`client.rs:532-545`) takes a
   closed enumeration instead of `method: &str`, each variant carrying its wire name and a
   documentation line naming the unit that added it, and no call site passes a string. Adding a
   call requires adding a variant.
3. **AC3** The set is pinned exactly: a test asserts the wire names of `Method::ALL`, equal to
   the calls the adapter makes at merge time — `getMe`, `getUpdates`, `getChat`, `leaveChat`,
   `sendMessage`, `sendChatAction`, `getChatAdministrators`, plus whatever a sibling unit merged
   before it added. The assertion's message states that the set is closed by decision 0070 and
   that widening it means reopening the decision.
4. **AC4** The nine names are on the shared refusal list and the scan finds none of them:
   `docs/telegram-refused-methods.txt` carries `banChatMember`, `kickChatMember`,
   `unbanChatMember`, `restrictChatMember`, `promoteChatMember`, `setChatAdministratorCustomTitle`,
   `banChatSenderChat`, `unbanChatSenderChat` and `setChatMemberTag`, each with a comment naming
   this unit, and the file states beside them why `deleteMessage` and `deleteMessages` are
   deliberately absent. If telegram unit 19, 09 or 10 merged first, this unit adds its names to the
   existing file and test and creates neither; if none has, it creates both in unit 19's specified
   form — whole alphanumeric runs, both crates' `src`, the test file outside the scanned
   directories, and a negative check proving the scan can fail.
5. **AC5** The token-bearing URL is built only at named sites: the same test asserts that
   `self.token` occurs in the adapter crate's `src` exactly at the sites the test names — the URL
   construction and the redaction today — with a message stating that any other occurrence is a
   second route to the platform that the closed set does not cover, and that a unit adding one
   (telegram unit 01's file download, `https://api.telegram.org/file/bot<token>/<file_path>`)
   names it here in the same commit.
6. **AC6** The core's directive vocabulary is closed and self-directed: a core test pins the exact
   variant sets of `ReplyKind`, `DeliveryItem`, `ObserveOutcome` and `IngestOutcome`, and its
   failure message states the invariant — the core may ask an adapter to make the assistant speak,
   mark or take back its own words, leave a channel or show its presence cue, and never to change
   what another person may do, may see, or whether they stay in the channel. The same sentence is
   on the `message.rs` module documentation and the test names it as its source.
7. **AC7** A promoted assistant deletes nothing but its own messages: with the scripted
   `getChatAdministrators` answer listing the assistant itself as an administrator, an
   administrator's `/del` replying to a **member's** message nulls that row exactly as it does
   today and produces no delete request of any kind, asserted against the scripted server's
   recorded requests by method name.
8. **AC8** A promoted assistant behaves as an ordinary member end to end: with the same scripted
   administrator list, a helpful-mode turn over a rule-violating group message files the report and
   issues exactly the requests it issues when the assistant is an ordinary member — asserted by
   comparing the recorded method names between the two runs, so a future capability that fires only
   when the assistant is elevated fails here.
9. **AC9** The decision record and the operator contract section ship, pinned in
   `crates/assistant/tests/docs.rs` in the existing shape (`docs.rs:713-765`): the decision file
   carries its date and its rejected alternatives; the operator contract names the five rights that
   must never be granted, states that every standing method belongs to the moderation bot's token,
   and states that banning a member in a supergroup removes every message that member ever sent
   from the chat and nothing from the assistant's store.
10. **AC10** No privacy or compliance document is modified — the unit's diff touches no file under
    `docs/privacy/` or `docs/compliance/` — and the four named statements are re-read against the
    merged code and recorded as checked in the merge: `bot-assistant-privacy-policy.md:75`,
    `dpia.md:578`, `dpia.md:666-667` with `:729-730`, and `ai-act.md:41-44`.
11. **AC11** The nine names are also on `docs/platform-vocabulary.txt` and the core scan stays
    green, proving the core names none of them. Each is a single alphanumeric run, none is an
    English word, and none appears in `crates/core` today (verified 2026-08-27).

## Notes for launch

- Branches from `main` into its own worktree, merges back on completion, and the worktree is
  deleted. The diff is small; the document is the deliverable that matters most.
- **Adapter sites**, all in `crates/adapters/telegram/src/client.rs`:
  - A closed `Method` enumeration beside `CONSUMED_UPDATE_TYPES` (`client.rs:103`), one variant
    per call, with a `wire_name` reading and a `Method::ALL` slice. Its type documentation carries
    decision 0070's sentence and states that the set is the whole outbound surface.
  - `post` (`client.rs:532-545`) and `request` (`client.rs:505-530`) take the enumeration; the
    seven call sites (`client.rs:304`, `:313`, `:334`, `:348`, `:401`, `:439`, `:464`) pass a
    variant. The URL construction at `client.rs:538` is otherwise unchanged.
  - The module summary "three methods over two endpoints and the administrator list"
    (`client.rs:269-270`) is corrected; it undercounts by four today, and the enumeration is what
    makes the count checkable.
- **Adapter test sites**: AC4 and AC5 belong in the shared refusal-scan test under
  `crates/adapters/telegram/tests/`, reading the committed list so the needles are not in a
  scanned file, in the shape `crates/core/tests/vocabulary.rs:15`, `:35` and `:64` use. AC3 sits beside
  it or in a `#[cfg(test)] mod tests` in `client.rs` — the crate has no test module there today
  and telegram unit 08 adds one; either home is fine and the choice is the implementer's. AC7 and
  AC8 go in `crates/adapters/telegram/tests/adapter/`, reusing the scripted server's
  administrator-list arm; scripting the assistant's own id into that list is the one fixture
  addition this unit needs.
- **Core sites**: the `crates/core/src/message.rs` module documentation gains the invariant
  sentence; a new test under `crates/core/tests/spine/` pins the four variant sets for AC6. No
  behaviour in the core changes and no file under `crates/core/src/` changes except that
  documentation.
- **Document sites**: `docs/telegram-refused-methods.txt` (created or extended, per AC4);
  `docs/platform-vocabulary.txt`; one decision record in `docs/decisions/` at the next free number
  when the unit merges — `0105` is the highest on `main` today and telegram units 07 and 08 each
  reserve a series above it; a paragraph in `docs/reference/group-operator-contract.md` beside
  requirement 3 (`group-operator-contract.md:112-115`); and the pins in
  `crates/assistant/tests/docs.rs`.
- **Sibling dependencies, named and not acted on.** Telegram unit 04 owns the delete pair; this
  unit re-specifies none of it, and if unit 04 has not merged then AC3's list omits
  `deleteMessages` and AC7 covers only the member-message case, which is the case that matters
  here. Telegram unit 09 owns the promotion notice, the `chat_member` refusal and the join-request
  refusal. Telegram unit 19 owns the refusal list, the scan and `setChatPermissions`. Telegram
  unit 06 owns `deleteMessageReaction`, which needs `can_delete_messages` and removes another
  person's reaction — if that unit's examination does not refuse it, this unit's invariant is the
  reason it must, and the point belongs in that review, not in an edit to its specification.
  Telegram unit 08 specifies an outbound scan of its own over the same crate; unit 19's merge rule
  covers it, and whoever merges last should fold the lists together instead of leaving three
  scans over one crate.
- **One thing an implementer will meet.** A refused call returns a bare 400 through
  `decode` (`client.rs:565-570`), so a rights refusal arrives without the platform's own
  description. Nothing here needs it, and the recorded follow-up (`docs/follow-ups.md:13`) keeps
  the item.
