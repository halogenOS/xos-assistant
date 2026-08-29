# Telegram unit 23 — boosts and giveaways: the half that cannot arrive, and the half that arrives naming a hundred strangers

Date: 2026-08-27. This family reads like one feature with one answer, and it is two features
with opposite shapes. Everything below is checked against the live Bot API page and its
changelog (Bot API 10.3, 24 August 2026, both fetched 2026-08-27) and against this tree at
`7fb217d`. The brief for this unit named Bot API 10.1 as current; the live changelog's first
entry is 10.3, and nothing in 10.2 or 10.3 touches boosts or giveaways, so the difference
changes no claim here — it is recorded because a spec that quietly corrects its own brief is
a spec nobody can check.

Four findings, and the first two invert the reading most people arrive with.

**The two boost updates cannot reach this assistant, and the reason is a settled decision, not
an oversight.** `Update.chat_boost` and `Update.removed_chat_boost` both carry the sentence
"The bot must be an administrator in the chat to receive these updates", and this deployment's
assistant must stay an ordinary member: the group's moderation bot ignores administrators'
reports, so an administrator assistant files into silence
(`docs/reference/group-operator-contract.md:112-115`). The same wall stands in front of the
family's only method. `getUserChatBoosts` says "Requires administrator rights in the chat", and
the right that carries it — `can_manage_chat`, described as the right to "access the chat event
log, get boost list, see hidden supergroup and channel members" — is "Implied by any other
administrator privilege". There is no partial version of this: an assistant that can read boosts
is an assistant whose reports are ignored.

**And yet the boost fact arrives anyway, twice, needing no rights at all.** A boost in a
supergroup produces the `boost_added` service message (`ChatBoostAdded`, one field,
`boost_count`), which rides the ordinary `message` update this adapter already subscribes to.
Worse for us, `Message.sender_boost_count` — "If the sender of the message boosted the chat, the
number of boosts added by the user" — rides *every ordinary message a booster sends*, including
the ones this assistant records and sends to the model provider. So the question this unit
actually has to answer is not "can we see boosts" but "what happens to the part that turns up
uninvited, attached to content we do keep".

**Giveaways are not channel-only, and their winners list is the sharpest personal-data payload
in this whole series.** Telegram's own protocol documentation states it plainly: "Telegram
channel and supergroup administrators with any set of rights may launch giveaways", and on
completion "a `messageActionGiveawayResults` will be sent to the channel/supergroup"
(`https://core.telegram.org/api/giveaways`, fetched 2026-08-27). In Bot API terms that means all
four giveaway carriers can land in the very group this assistant serves, and one of them,
`GiveawayWinners`, holds `winners`: "List of up to 100 winners of the giveaway", an array of
full `User` objects. Every `User` carries a mandatory `first_name` — the field decision 0077
removed from this system, column and decoder both — naming up to a hundred people who may never
have written a word to the assistant, in a message nobody addressed to it.

**Nothing here can be done, only received.** The page defines 588 named methods and types; the
only one in this family is `getUserChatBoosts`. There is no method to create, schedule, cancel,
end or draw a giveaway, and none to grant, buy or read a boost level. A member or an
administrator who asks the assistant to run a giveaway, pick a winner, or check who boosted is
asking for something no bot on this platform can do, and the failure that costs us is not a
rejected API call — no such call can be written — but a model that says "sure, I'll draw the
winner" and then invents one.

The unit is therefore a refusal on both halves, and the deliverable is the set of mechanisms
that make the refusal checkable instead of accidental. It is accidental today: all five
carriers already die in translation, but they die on the "no text" and "on behalf of a chat"
skips, and two sibling units are in the middle of lifting exactly those floors.

## What a bot in this supergroup actually receives, stated before the design

Six platform facts, each verified against the current documentation instead of recalled.

1. **`chat_boost` and `removed_chat_boost`: never, at this deployment.** Administrator rights
   required, by the field descriptions on `Update`. They are also in the *default* subscription
   set — `getUpdates`'s `allowed_updates` says "Specify an empty list to receive all update types
   except `chat_member`, `message_reaction`, and `message_reaction_count`" — so the only thing
   standing between a future promoted assistant and a stream of named people's payment events is
   the explicit list this adapter sends on every poll (`client.rs:103`, `client.rs:319`).
2. **`getUserChatBoosts`: never, at this deployment.** Administrator rights required. It answers
   per user, not per chat: `UserChatBoosts` is one field, `boosts`, an array of `ChatBoost`, each
   carrying `boost_id`, `add_date`, `expiration_date` and a `source`. There is no method that
   reports a chat's boost level to a bot at all.
3. **`boost_added`: yes, with no rights, as an ordinary message update.** Added in Bot API 7.1
   (16 February 2024) alongside `sender_boost_count` and `Chat.unrestrict_boost_count`, the
   release that brought boosts to supergroups. The service message carries `ChatBoostAdded` with
   `boost_count`. This design does not depend on who its `from` names: recognition happens before
   the sender check, so a boost notice is refused whether the platform names the booster on it or
   not.
4. **`sender_boost_count`: yes, with no rights, on ordinary conversation.** It is a field of
   `Message`, not of a service message, and it appears on messages this assistant records.
5. **All four giveaway carriers: possible in this group.** `Message.giveaway_created`
   (`GiveawayCreated`), `Message.giveaway` (`Giveaway`), `Message.giveaway_winners`
   (`GiveawayWinners`) and `Message.giveaway_completed` (`GiveawayCompleted`). They also arrive
   second-hand: a giveaway posted in a channel that this group is the discussion group for
   auto-forwards into the group, and any member can forward one in. In the group's own giveaway
   the messages are posted by the chat itself, which is the case this adapter already treats as
   "on behalf of a chat".
6. **The boost is a real power in the chat, which is the part that makes it dangerous to us.**
   `Chat.unrestrict_boost_count` is "For supergroups, the minimum number of boosts that a
   non-administrator user needs to add in order to ignore slow mode and chat permissions". A
   booster can post what an ordinary member cannot. That is exactly the fact somebody will
   eventually want the assistant to weigh, and decision 3 below refuses it.

`ChatBoostSource` repays reading even though this deployment can never hold one, because it
says what a boost record *is*: `ChatBoostSourcePremium` names the `user` who boosted;
`ChatBoostSourceGiftCode` names the `user` a gift code was created for; `ChatBoostSourceGiveaway`
names the `user` who won, plus `giveaway_message_id`, `prize_star_count` and `is_unclaimed`.
Every variant names a person and how they paid. A stored boost is not a fact about a chat with a
person's identifier attached to it; it is a fact about a person's money, with the chat attached.

## Grounding

### The platform, read 2026-08-27

Sources: `https://core.telegram.org/bots/api` and `https://core.telegram.org/bots/api-changelog`,
both fetched on the date of this spec, plus `https://core.telegram.org/api/giveaways` for the
protocol-level statement about which chat types may run a giveaway. Claims are cited by named
API method, type and field, which is the stable way to check them.

- `Update.chat_boost` → `ChatBoostUpdated` (`chat`, `boost`); `Update.removed_chat_boost` →
  `ChatBoostRemoved` (`chat`, `boost_id`, `remove_date`, `source`). Both field descriptions end
  "The bot must be an administrator in the chat to receive these updates."
- Neither is named in `getUpdates`'s default exclusion list, which is exactly
  `chat_member`, `message_reaction`, `message_reaction_count`.
- `getUserChatBoosts(chat_id, user_id) → UserChatBoosts`. "Requires administrator rights in the
  chat."
- `ChatAdministratorRights.can_manage_chat` / `ChatMemberAdministrator.can_manage_chat`: "…get
  boost list… Implied by any other administrator privilege."
- `ChatBoost`: `boost_id`, `add_date`, `expiration_date`, `source`.
- `ChatBoostSourcePremium`: `source` = "premium", `user`. `ChatBoostSourceGiftCode`: `source` =
  "gift_code", `user`; the type note records that each code "boosts the chat 4 times".
  `ChatBoostSourceGiveaway`: `source` = "giveaway", `giveaway_message_id` ("the message could
  have been deleted already. May be 0 if the message isn't sent yet"), optional `user`, optional
  `prize_star_count`, optional `is_unclaimed`.
- `Message.boost_added` → `ChatBoostAdded` (`boost_count`). Changelog, Bot API 7.1, 16 February
  2024: "Added the class ChatBoostAdded and the field boost_added to the class Message for
  service messages about a user boosting a chat", "Added the field sender_boost_count to the
  class Message", "Added the field unrestrict_boost_count to the class Chat".
- `Message.sender_boost_count`: "If the sender of the message boosted the chat, the number of
  boosts added by the user."
- `GiveawayCreated`: optional `prize_star_count` only.
- `Giveaway`: `chats` (array of `Chat`), `winners_selection_date`, `winner_count`, optional
  `only_new_members`, `has_public_winners`, `prize_description`, `country_codes` (ISO 3166-1
  alpha-2), `prize_star_count`, `premium_subscription_month_count`.
- `GiveawayWinners`: `chat`, `giveaway_message_id`, `winners_selection_date`, `winner_count`,
  `winners` — "List of up to 100 winners of the giveaway", array of `User` — plus optional
  `additional_chat_count`, `prize_star_count`, `premium_subscription_month_count`,
  `unclaimed_prize_count`, `only_new_members`, `was_refunded`, `prize_description`.
- `GiveawayCompleted`: `winner_count`, optional `unclaimed_prize_count`, optional
  `giveaway_message` (a nested full `Message`), optional `is_star_giveaway`.
- `User` always carries `id`, `is_bot` and `first_name`; `last_name`, `username` and
  `language_code` are optional. `first_name` is mandatory, which is why the winners array cannot
  be decoded "just for the identifiers".
- Changelog, Bot API 7.0, 29 December 2023: the two boost updates, the three
  `ChatBoostSource` variants, `getUserChatBoosts`, and all four giveaway classes arrived
  together. Bot API 7.10, 6 September 2024, added `prize_star_count` to `GiveawayCreated`,
  `Giveaway`, `GiveawayWinners` and `ChatBoostSourceGiveaway`, and `is_star_giveaway` to
  `GiveawayCompleted`. Nothing in 8.x, 9.x, 10.0, 10.1, 10.2 or 10.3 changes any of it.
- Method count: the page's anchor list holds 588 named methods and types; the only anchors
  matching "boost" are the seven types above plus `getuserchatboosts`, and the only anchors
  matching "giveaway" are the four `Giveaway*` classes and `ChatBoostSourceGiveaway`. There is no
  giveaway method of any kind.
- `https://core.telegram.org/api/giveaways`, fetched 2026-08-27: "Telegram channel and supergroup
  administrators with any set of rights may launch giveaways"; "If `winners_are_visible` flag is
  set while starting a giveaway, giveaway winners are public and will be listed in a
  `messageMediaGiveawayResults` message"; on completion "a `messageActionGiveawayResults` will be
  sent to the channel/supergroup". Those two protocol constructors are what the Bot API surfaces
  as `Message.giveaway_winners` and `Message.giveaway_completed`.

### Our tree, at `7fb217d`

- **The poll names its update types on every call.** `CONSUMED_UPDATE_TYPES` is
  `["message", "edited_message", "my_chat_member"]` (`client.rs:103`) and is sent in the
  `getUpdates` body every time (`client.rs:319`), with the reason written into the constant's
  own comment: "an absent selection inherits whatever an earlier setting left on the token, so
  the selection is stated instead of assumed" (`client.rs:100-102`). Neither boost update can
  arrive through this poll even if the assistant were promoted tomorrow.
- **The decoder holds only what translation reads.** `Incoming` (`client.rs:125-144`) decodes
  eleven fields and no more; unknown keys are ignored by serde, which is stated on the type
  (`client.rs:105-107`). `boost_added`, `sender_boost_count` and all four giveaway fields are
  therefore discarded at the wire today — by a default, not by a decision.
- **A person's name is already refused at the decoder.** `User` decodes `id` and `username` only,
  and the comment says why: "The platform's name fields are not decoded at all, so a display name
  never enters the process as a typed value" (`client.rs:212-220`, decision 0077). The winners
  array would walk straight past that, in bulk.
- **Where these five carriers die today, and why that is not good enough.** Translation matches
  the chat kind (`translate.rs:127-133`), handles the pin service note
  (`translate.rs:135-158`), skips a message sent on behalf of a chat (`translate.rs:159-161`),
  skips a message with no sender (`translate.rs:162-165`), then skips a message with neither text
  nor caption (`translate.rs:166-169`, `text_of` at `translate.rs:466-472`). So the group's own
  giveaway messages die as `Skip::OnBehalfOfChat` and the service messages die as
  `Skip::NoText` — both of which are about to change underneath us. Telegram unit 01 renames
  `Skip::NoText` to `Skip::NothingToRecord` and permits an empty text column
  (`docs/units/telegram/01-receiving-media.md:249`), and telegram unit 14 records a wordless
  sticker as a turn in the conversation. A refusal that rests on "there was no text" is a
  refusal with a scheduled expiry date.
- **Every skip is a named case with a `Display` string, logged by the driver.** The `Skip` enum
  and its documentation are at `translate.rs:37-76`, the `Display` impl at `translate.rs:474-495`,
  and the driver logs `%reason` for every skipped update (`driver.rs:368-369`). Adding a case
  here costs one variant, one `Display` arm and one branch.
- **The service-message shape this unit copies already exists.** Telegram unit 09 decided that
  `new_chat_members` and `left_chat_member` are decoded as `Option<serde::de::IgnoredAny>` —
  presence recorded, contents discarded unread — precisely so other people's names are never in
  the process's memory and never one small edit away from being stored
  (`docs/units/telegram/09-chat-member-events.md:343-361`). Telegram unit 21 took the same shape
  for the video-chat invited list. This unit adds the fifth and sixth users of that pattern
  instead of inventing a sixth spelling of it.
- **Standing comes from the administrator list and nothing else.** `authority.rs:1-7` states
  decision 0015: `creator` → admin, `administrator` → moderator, everyone else → member, resolved
  from a per-chat administrator list with a one-minute cache (`authority.rs:19`, `:44-82`). There
  is no other input to a sender's standing, and this unit adds none.
- **Every route this project gives a person keys on that person having spoken.** An identity row
  is resolved only from ingestion (`identity.rs:60`); the standing opt-out is read from the
  *sender* of the message being recorded (`assembly.rs:1143-1151`); the privacy commands are
  messages in a conversation. A record about a giveaway winner would be a record about somebody
  who cannot be told it exists, cannot object, and whose already-raised objection would not even
  be consulted, because they are not the sender of the message that carries their name.
- **The core has no vocabulary for any of this and needs none.** `ObservedFact`
  (`message.rs:230-244`) carries a title, a pinned announcement and the assistant's own
  admission. `DeliveryItem` (`message.rs:251-258`) carries an acknowledgment and a command
  answer. Nothing in this unit proposes a new variant of either, so the platform-vocabulary check
  (`crates/core/tests/vocabulary.rs`) has nothing new to catch.
- **The prompt is where a refusal of this kind is taught.** `prompts/30-conduct.md` (74 lines)
  already carries the AI-honesty passage and the handle rules, and `crates/assistant/tests/docs.rs`
  pins prompt text against the composed prompt (`repo_prompt()` at `:53`).
- **A source scan is an established test shape here.** `crates/adapters/telegram/tests/token_scan.rs`
  owns its own test binary to scan for a forbidden string; telegram unit 24 specifies a scanner
  with a committed list of forbidden method names for the payments family. This unit's one
  forbidden method joins that list instead of starting a second scanner.

### What the privacy documents say today

- Data subjects are defined by communication: S1 is "Members of the project's community groups
  whose messages the assistant stores", S2 is "People who write to the assistant directly"
  (`records-of-processing.md:50-51`). A giveaway winner is neither. Recording one would require a
  new subject category, which is a change to the record of processing, not a footnote.
- D1 is message content, "The text of a message, including the caption of a media message"
  (`records-of-processing.md:61`). A boost count and a winners list are neither text nor caption.
- The data-minimisation line is a list of what is not taken: "Text only, no media, no files, no
  voice, no stickers, no edits (decision 0017). Anonymous stand-in senders skipped (decision
  0016). No profiling, no scoring, no secondary use." (`records-of-processing.md:144`). "No
  profiling, no scoring" is the clause a booster flag would falsify.
- The legitimate-interest assessment's necessity section reads "What is not necessary, and
  therefore not done… Anonymous administrator posts and channel forwards are skipped… No profile
  of any member is built" (`lia.md:106-109`). The channel-forward sentence already covers the
  auto-forwarded giveaway from a linked channel; this unit keeps it true and makes it deliberate.
- The impact assessment's review triggers include "A change to what is collected: media, edits,
  reactions, membership events" and "Any moderation capability shipping"
  (`dpia.md:559-575`). This unit collects nothing new and ships no capability, so neither fires —
  which is a fact to check and record, not to assume.

## Decisions taken with this unit

- **Neither boost update is subscribed to, and the refusal is written into the assertion rather
  than into a comment, 2026-08-27.** `CONSUMED_UPDATE_TYPES` stays exactly as it is, and a test
  asserts both that it equals the three current entries and that it contains neither
  `chat_boost` nor `removed_chat_boost`, with the reason in the assertion message: these updates
  need administrator rights the assistant must not hold, and they are in the platform's default
  set, so a future edit that drops the explicit list turns them on silently.
  Why an assertion and not a comment: the list is one line and the platform's default is the
  opposite of it. The failure mode is somebody simplifying `"allowed_updates": CONSUMED_UPDATE_TYPES`
  away as redundant, which is a plausible cleanup and a silent subscription to a stream of named
  people's payment events.
  *Rejected:* subscribing to both "since they cannot arrive anyway". A subscription that is inert
  only because of a separate operational choice is one promotion away from being live, and the
  promotion has its own reason to happen (somebody wanting the assistant to pin, or to delete).
  *Rejected:* promoting the assistant to receive them. It breaks the report setup outright — the
  moderation bot ignores administrators' reports
  (`docs/reference/group-operator-contract.md:112-115`) — and telegram unit 09 already ships a
  notice for exactly this mistake.

- **`getUserChatBoosts` is never called, and a source scan proves it, 2026-08-27.** It is the
  family's only method and it is on the forbidden-method list telegram unit 24 introduces, with
  its own one-line reason beside it.
  Why a scan and not "we simply did not write it": the method is per-user and takes exactly
  the two things the adapter has in hand at every group message — a chat id and a sender id — so
  it is the easiest possible addition for an implementer who thinks a booster flag would be
  useful. The scan is what makes "we do not do this" survive a future contributor's good idea.
  *Rejected:* calling it to enrich the sender's standing. That is decision 3 below, and it is
  refused on its own terms.
  *Rejected:* calling it to answer "who boosted the group". It answers per user, so it cannot
  answer that question at all, and answering it would mean the assistant naming people's payments
  on request.
  *Rejected:* calling it once at first contact to learn the group's boost level. There is no such
  call: no method reports a chat's boost level to a bot, and this one needs a `user_id` and
  administrator rights.

- **A boost never touches standing, and the assistant never weighs one in an assessment,
  2026-08-27.** Standing comes from the administrator list and nothing else (decision 0015,
  `authority.rs:1-7`), and no assessment input keys on payment, in either direction.
  Why this needs to be a written decision and not an absence: the platform makes the idea
  concrete and almost reasonable. `Chat.unrestrict_boost_count` means a sufficiently-boosted
  member can "ignore slow mode and chat permissions" — that is, a booster genuinely can post what
  an ordinary member cannot. From there it is one short step to "so treat their messages as
  trusted", or the mirror-image "so watch them harder, they bought their way past the limits".
  Both make a person's money an input to a judgement about them, and both fall under decision
  0070's binding: any path that could touch a person's standing carries the human decision point
  in its mechanism. A payment-weighted assessment does not have one; it has a price.
  *Rejected:* a "trusted booster" exemption from assessment. It sells a moderation outcome, and
  it is invisible to the administrators who are supposed to be the deciders.
  *Rejected:* the inverse suspicion rule. Same defect, same direction, opposite sign.
  *Rejected:* passing the boost count to the model as neutral context and "letting it decide".
  That is the same decision, moved somewhere it cannot be reviewed.

- **Five carriers become one named skip over a closed enum, 2026-08-27.** The adapter gains
  `Skip::BoostOrGiveawayNotice(BoostOrGiveaway)` over
  `BoostOrGiveaway { BoostAdded, GiveawayCreated, Giveaway, GiveawayWinners, GiveawayCompleted }`
  — one refusal, one match arm, five `Display` strings ("a boost service message", "a giveaway
  creation notice", "a scheduled giveaway message", "a giveaway winners message", "a giveaway
  completion notice"). The five presence flags are read in that order and the first present one
  answers; the platform sends one carrier per message, and the nested `Message` inside
  `GiveawayCompleted.giveaway_message` is nested, not a sibling field, so the order is a
  formality, not a precedence rule.
  Why one variant carrying which, and not five variants or one blanket skip: the log line is
  the only place an operator ever learns that the group ran a giveaway at all, and "a message
  with neither text nor caption" tells them nothing. One variant keeps the `Skip` enum from
  growing five near-identical entries while still naming the case.
  *Rejected:* leaving all five to the existing skips. They already work today and they are both
  scheduled to change: unit 01 turns `Skip::NoText` into `Skip::NothingToRecord` with an empty
  text column permitted, and unit 14 makes a wordless message a recordable turn. A refusal of a
  hundred people's names should not depend on which sibling merges first.
  *Rejected:* one blanket `Skip::UnwantedServiceMessage` shared with units 09, 21 and 24. It
  would produce a log an operator cannot act on and a test suite that cannot tell the four
  families apart, and the next family added to it would silently inherit four other units'
  reasoning.

- **The recognition sits immediately after the pin branch, ahead of the on-behalf-of-chat, sender
  and text checks, 2026-08-27.** Same placement telegram units 09 and 21 take, for the same
  reason and one more of its own: the group's own giveaway is posted by the chat, so without this
  placement a giveaway would be logged as an anonymous administrator's post and a boost notice
  would be logged as a message with no text — both true, both useless.
  The cost, stated here instead of discovered in review: if the platform ever attached a member's own
  words to one of these five carriers, this placement would drop those words. It cannot today —
  the three service messages have no text field in use, the giveaway message is composed by the
  platform, not by a person, and forwarding preserves the original message without
  offering a caption — and if it ever could, the refusal would still be right: a caption cannot
  make an undecoded hundred-name winners list acceptable to record, and a member's question is
  one ordinary message away, where it is recorded and answered normally.
  *Rejected:* recognising the carriers only when there is nothing else to record. It puts the
  refusal back underneath the text floor that two sibling units are lifting, which is the whole
  problem this decision exists to solve.
  *Rejected:* placing it before the chat-kind match so channel posts are named too. Channel posts
  are already refused wholesale (`translate.rs:132`), and a broadcast channel is not a
  conversation this assistant serves.

- **None of the five fields is decoded — all are `Option<serde::de::IgnoredAny>` — and the
  winners array is the reason, 2026-08-27.** Presence is everything the skip needs.
  `GiveawayWinners.winners` is up to 100 `User` objects, each with a mandatory `first_name` and
  an optional `last_name`, `username` and `language_code`. Decoding it would re-admit through a
  side door, in bulk, exactly the field decision 0077 removed from this system, about people who
  have not spoken. Three consequences follow, any one sufficient: those people have no route to
  the assistant, because every route keys on having spoken (`identity.rs:60`,
  `privacy.rs:95-105`); they could not be told the record existed; and the standing opt-out could
  not bind, because the suppression check reads the *sender* of the message being recorded
  (`assembly.rs:1143-1151`) and a winner is not the sender. A member who has already objected
  under Article 21 would get a fresh record of them written by somebody else's giveaway, with
  their flag never consulted.
  The honest limit, stated because unit 09 had to state it too: this claims nothing about
  `Message.from`, which the decoder reads for every message (`client.rs:130-131`) and which
  `Incoming`'s derived `Debug` would print. A booster who is named in `from` on their own boost
  notice is read by the decoder exactly as any sender is, and dropped at the skip a few lines
  later, never reaching the core.
  *Rejected:* decoding the arrays properly and ignoring them at the call site. It puts a hundred
  strangers' names into a `Debug`-derived struct (`client.rs:124`) that any log line could print.
  *Rejected:* decoding only `winner_count` and `boost_count`. Counts are not personal data and
  would be permissible, and no purpose asks for them; a stored or logged fact with no reader is
  what the necessity test excludes (`lia.md:106-109`).
  *Rejected:* decoding `Giveaway.country_codes` "since it is about the giveaway, not a person".
  It is a list of the countries eligible members must be in, which is a fact about the people who
  can enter, and it has no reader either.

- **`sender_boost_count` is never decoded, and that is pinned by a test instead of left to the
  decoder's default, 2026-08-27.** It is the one field in this family that rides messages this
  assistant *does* record and *does* send to the model provider, so "unknown fields are ignored"
  is too quiet a guarantee for it. The pin is a decode test over a realistic recorded group
  message plus a scan asserting the field name appears nowhere in the adapter's source outside
  that test.
  Why it matters more than its size suggests: adding it to `Incoming` costs one line, it looks
  like harmless metadata, and it would immediately be available to the projection that builds the
  model's view of a conversation. That is a paid-status flag beside a named person's words, sent
  to a model provider, contradicting "No profiling, no scoring" (`records-of-processing.md:144`)
  and requiring a new D-category in the record of processing.
  *Rejected:* decoding it and simply not using it. Unused, it is one edit from used; the field
  exists to be read.
  *Rejected:* relying on the type-level comment at `client.rs:105-107`. That comment explains a
  serde default; it does not fail a build.

- **The assistant never acknowledges a boost, and the refusal is taught, not merely
  absent, 2026-08-27.** No thank-you line, no reaction, no note, no mention of a member's boost
  in any answer.
  Four reasons, and the weakest one is the privacy one. First, it could not be consistent: the
  boost notice rides an update stream the platform drops after 24 hours, so an outage, a restart
  or a rate-limit pause loses boosts silently — and an assistant that thanked one member and said
  nothing to the next has not been neutral, it has visibly played favourites for reasons nobody
  can see. Second, silence is already this system's default in helpful mode (decision 0098); an
  event nobody addressed to the assistant is the clearest possible case of a message that
  warrants no reply. Third, a thank-you is an assistant-initiated public statement about a
  person's payment; that the platform already announced it does not make the assistant's
  repetition of it necessary, and necessity is the test the legitimate-interest assessment
  applies. Fourth, thanking for money is the first half of a relationship in which not thanking
  means something — the moment it exists, its absence becomes a message too.
  *Rejected:* a fixed thank-you line on `boost_added`, the shape the acknowledgment and the
  promotion notice already use. Those two fire on facts about the assistant itself — a rules
  delta it read, its own standing changing — and they are answers the operator contract promises.
  A boost is a fact about a member, and nothing promised a response to it.
  *Rejected:* a "current boosters" context note. Every context note is a permanent line in the
  model's system voice (`note.rs:154-188`), and this one would be a permanent, unerasable list of
  who paid.
  *Rejected:* a reaction instead of a message, on the grounds that it is smaller. Smaller, and
  the same decision.

- **The model is taught the exact shape of its blindness here, 2026-08-27.** A passage in
  `prompts/30-conduct.md`, pinned sentence for sentence, saying: it is not told who boosted the
  group and cannot look it up; it cannot start, run, judge, cancel or end a giveaway, and no
  amount of asking changes that; it cannot tell anyone whether they won, and does not guess; a
  member's statement that they boosted or won is that member's account, taken as such and never
  restated as the assistant's own knowledge; and a boost buys nothing from the assistant — no
  priority, no leniency, no special treatment.
  Why the prompt and not only the mechanism: decision 0096 already says substantive answers come
  only from tool lookups, but an unaided model asked "did I win?" does not experience itself as
  making a substantive claim — it summarises the message it thinks it can see. It cannot see it,
  because this unit skips it, so what it would produce is an invention about a prize. Telegram
  unit 21 hit the identical failure with video calls and took the identical remedy.
  *Rejected:* leaving it to the mechanism. The mechanism guarantees the assistant does not
  *know*; only the prompt stops it *claiming*.
  *Rejected:* a stock "I can't help with that" refusal. It is false in the useful direction: the
  assistant can perfectly well explain what a boost is or where the giveaway rules are; what it
  cannot do is see one.

- **One paragraph is added to the operator contract, and no privacy document changes,
  2026-08-27.** The contract gains: boosting the group has no effect on the assistant, in either
  direction; the assistant cannot see who boosted and will not thank anyone for it; it cannot run
  or judge a giveaway; and promoting it so that it could read boosts breaks the report setup, so
  the answer to "can it read the boost list" is permanently no.
  The privacy documents are unchanged because nothing new is received into the core, stored, sent
  or reached, and the check is recorded as performed: S1 and S2 still describe every subject
  (`records-of-processing.md:50-51`), D1 still describes every category (`:61`), the minimisation
  line stays literally true including "No profiling, no scoring" (`:144`), the
  legitimate-interest necessity section keeps its channel-forward sentence (`lia.md:106-109`),
  and neither the "change to what is collected" nor the "moderation capability shipping" trigger
  fires (`dpia.md:559-575`).
  *Rejected:* leaving the operator contract silent. The operator is the person who would
  otherwise promote the assistant to fix the "it doesn't thank boosters" complaint, and the
  contract is where they look.
  *Rejected:* adding a "not collected" line about boosts to the record of processing. That
  document records what is processed; a catalogue of everything declined would grow without
  bound, and the decision record is where a refusal belongs.

- **Nothing streams, and this is stated so nobody has to check, 2026-08-27.** The standing
  streaming constraint is not engaged by this unit: no method is called, no file is fetched, no
  byte moves. The largest object involved, a hundred-winner array, is refused at the decoder
  instead of buffered, which is the strictest possible answer to "does it whole-buffer".

## The unit's contract

The assistant is blind to boosts by construction and refuses giveaways by name. The two boost
updates are not subscribed to and could not arrive if they were, and the subscription is pinned
by an assertion carrying its own reason; the family's only method, `getUserChatBoosts`, appears
nowhere in the source and a scan proves it. The five carriers that do arrive over the subscribed
message type — the boost service message and the four giveaway messages — are recognised
immediately after the pin branch and ahead of the on-behalf-of-chat, sender and text checks, so a
giveaway posted by the group itself is named a giveaway instead of an anonymous post, and no
sibling unit's rewrite of the text floor can turn any of them into a recorded turn. None of the
six fields in the family is decoded: all are `Option<serde::de::IgnoredAny>`, so the winners
array's hundred names, the boost counts, the prize descriptions and the eligible countries never
enter this process, and `sender_boost_count` never rides along beside content that is stored and
sent onward. Nothing is recorded, no identity row is created, no note is appended, no block is
written, no core type grows a variant, no schema step is added and `docs/platform-vocabulary.txt`
is unchanged, because the core learned no word from this unit. A boost changes nobody's standing
and enters no assessment, so decision 0070 is untouched and no human decision point was moved.
The assistant never acknowledges a boost and never claims to know a giveaway's outcome; the
prompt says so plainly, pinned sentence for sentence, so an unaided model cannot invent a winner.
Two documents change — a decision record and one paragraph of the operator contract — and the
privacy documents change not at all, with the five statements that could have been falsified
re-checked by line and recorded as checked. Nothing streams, because nothing here carries a byte.
No new dependency, no new configuration key, no new update subscription, and no behaviour a
member can observe except the one they could already observe: silence.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt and doc under denied
  warnings; the platform-vocabulary check and the token scan clean; no new dependency in any
  manifest.
- **AC2** The subscription is pinned with its reason: a test asserts `CONSUMED_UPDATE_TYPES`
  equals `["message", "edited_message", "my_chat_member"]` and separately asserts it contains
  neither `"chat_boost"` nor `"removed_chat_boost"`, the second assertion's message stating that
  both are in the platform's default set and both require administrator rights the assistant must
  not hold. A second pin reads the recording server's captured `getUpdates` bodies across a
  multi-poll run and asserts every one of them carried the list — an empty or absent
  `allowed_updates` fails the test.
- **AC3** Each carrier names itself: five fixture updates, one per field, translate to
  `Skip::BoostOrGiveawayNotice` carrying `BoostAdded`, `GiveawayCreated`, `Giveaway`,
  `GiveawayWinners` and `GiveawayCompleted` respectively, and each `Display` string is pinned
  character for character. A `giveaway_created` payload of `{}` and one carrying unexpected keys
  translate identically.
- **AC4** The placement holds against all three later checks: a carrier whose message also
  carries `sender_chat` reaches the new skip and **not** `Skip::OnBehalfOfChat`; one with no
  `from` reaches it and not `Skip::NoSender`; and one that also carries a non-empty `text` and a
  non-empty `caption` reaches it and is **not** recorded. The third case is the one that survives
  telegram units 01 and 14, and its test comment says so and names them.
- **AC5** The winners array is structurally undecodable: a `giveaway_winners` payload carrying
  100 winners, each with `id`, `first_name`, `last_name`, `username` and `language_code` all
  distinct from the message's own `from`, decodes without error, and the resulting `Incoming`'s
  `Debug` output contains none of those 500 values. The test states unit 09's honest limit
  verbatim in a comment: it claims nothing about `Message.from`, which the decoder reads for
  every message and which the skip discards a few lines later.
- **AC6** `sender_boost_count` never enters the process: an ordinary group message that is
  recorded normally, carrying `sender_boost_count: 7`, produces a stored row identical in every
  column to the same message without the field, and the decoded `Incoming`'s `Debug` output
  contains no `7` other than ones the message's own recorded fields explain. A source scan over
  `crates/adapters/telegram/src` finds the string `sender_boost_count` nowhere.
- **AC7** Nothing reaches storage and nothing reaches the wire: after the five updates are
  delivered to an admitted group against the loopback server, the identity table holds no new
  row, the ledger holds no new block, no chat message row exists, and the recording server saw no
  `sendMessage`, no `sendChatAction`, no `getChat`, no `getChatAdministrators` and no
  `leaveChat` — asserted by reading the store and the request log directly, before and after. The
  persisted offset advances past all five, so a group that runs giveaways cannot stall the poll.
- **AC8** The method is unreachable: a source scan over the adapter crate finds
  `getUserChatBoosts` in no source file, driven by the committed forbidden-method list telegram
  unit 24 introduces, with this unit's one-line reason beside the entry. If unit 24 has not
  merged, this unit ships the scanner and the list file, and unit 24 adds to it.
- **AC9** The core is untouched: the merge's diff contains no change under `crates/core/src`, and
  a substring scan under `crates/core` finds none of `boost_added`, `sender_boost_count`,
  `chat_boost`, `giveaway`, `Giveaway` or `ChatBoost`. The scan's comment states why it is not
  the existing vocabulary check: that one matches whole alphanumeric runs, so a needle containing
  `_` can never match it (`crates/core/tests/vocabulary.rs:62-67`).
- **AC10** The model is taught: `prompts/30-conduct.md` carries the passage, and
  `crates/assistant/tests/docs.rs` pins it against `repo_prompt()` claim by claim — not told who
  boosted and cannot look it up; cannot start, run, judge, cancel or end a giveaway; cannot say
  who won and does not guess; a member's claim of boosting or winning is that member's account; a
  boost buys no priority, no leniency and no special treatment.
- **AC11** The refusals are recorded where the next implementer will look: one decision file per
  the repository's convention, its number taken at merge and named in the commit, carrying the
  four decisions with standing beyond this unit — the boost updates and the boost method are
  refused because the assistant must remain a non-administrator; a boost never touches standing
  or an assessment; the family's fields are structurally undecodable; the assistant never
  acknowledges a boost. Each names its rejected alternatives.
- **AC12** The privacy documents are unchanged — `git diff docs/privacy/` is empty — and five
  statements are re-checked against the merged code and recorded in the merge as checked, by
  line: the S1 and S2 definitions (`records-of-processing.md:50-51`), D1's content line (`:61`),
  the data-minimisation line including "No profiling, no scoring" (`:144`), the
  legitimate-interest necessity paragraph (`lia.md:106-109`), and the two review triggers that
  did not fire (`dpia.md:559-575`).
- **AC13** The operator contract carries the new paragraph, and it states all four points: a
  boost changes nothing about the assistant, the assistant cannot see who boosted and will not
  thank them, it cannot run or judge a giveaway, and promoting it to read the boost list breaks
  the report setup.

## Notes for launch

- Branches from `main` into its own worktree; self-contained in the consumer repository. No
  framework change, no schema step, no migration.
- **Sites, exactly:**
  - `crates/adapters/telegram/src/client.rs` — `Incoming` (`:125-144`) gains six
    `Option<serde::de::IgnoredAny>` fields (`boost_added`, `sender_boost_count`,
    `giveaway_created`, `giveaway`, `giveaway_winners`, `giveaway_completed`) under one shared
    comment stating that presence is all the skip needs, that no value is read, and that
    `sender_boost_count` is listed here despite riding ordinary messages precisely because it
    does. `CONSUMED_UPDATE_TYPES` (`:103`) is unchanged and pinned by AC2.
  - `crates/adapters/telegram/src/translate.rs` — the `BoostOrGiveaway` enum beside `Skip`; the
    `Skip::BoostOrGiveawayNotice(BoostOrGiveaway)` variant in the enum (`:37-76`); the
    recognition immediately after the pin branch (`:158`) and before the on-behalf-of-chat skip
    (`:159`); the five `Display` arms (`:474-495`); the pins in the inline test module (`:498`
    onward, beside the existing skip pins).
  - `crates/adapters/telegram/tests/adapter/` — the end-to-end pins of AC4, AC6 and AC7 against
    the loopback and recording servers, and the decode pins of AC5 and AC6.
  - `crates/adapters/telegram/tests/` — the source scans of AC6, AC8 and AC9, each in a file that
    excludes itself from its own scan.
  - `prompts/30-conduct.md` — the passage, placed beside the AI-honesty prose it matches in kind.
  - `crates/assistant/tests/docs.rs` — the prompt pin of AC10.
  - `docs/decisions/` — the decision record of AC11.
  - `docs/reference/group-operator-contract.md` — the paragraph of AC13, in the report-setup
    section, where the non-administrator requirement already lives (`:112-115`).
- **Ordering against the siblings that touch the same lines.** Four specs written in this batch
  edit `Incoming` and the `Skip` enum, and whichever merges second adapts instead of re-deciding:
  - Telegram unit 01 renames `Skip::NoText` to `Skip::NothingToRecord` and permits an empty text
    column. This unit's skip is independent of that rename and its AC4 exists to prove it; if
    unit 01 has merged, only the unrelated `Display` arm's wording differs.
  - Telegram unit 14 makes a wordless message a recordable turn. Same relationship: AC4 is the
    pin that keeps a giveaway from becoming one.
  - Telegram units 09, 21 and 24 all insert a service-message recognition immediately after the
    pin branch. The presence flags are disjoint — no message carries a membership list, a video
    chat notice, a payment notice and a giveaway at once — so the arms may sit in any order, and
    whichever implementer arrives second appends instead of reordering.
  - Telegram unit 24 owns the forbidden-method scanner and its list file; AC8 adds one entry to
    it, or ships it, depending on merge order. Coordinate on the file name at merge; do not
    create a second scanner.
- Do not edit the sibling specs. Telegram unit 24 covers Stars, gifts and the payment refusal,
  and `ChatBoostSourceGiveaway.prize_star_count` and `GiveawayCompleted.is_star_giveaway` sit on
  the seam between that unit and this one: the rule is that any *value transfer* belongs to unit
  24 and any *chat boost or giveaway carrier* belongs here, and the two units' skips can coexist
  because a message carries at most one of them. If unit 24's author disagrees with that line,
  it belongs in a review, not in an edit to either spec.
- **Record one follow-up in `docs/follow-ups.md`** (boosts and giveaways, 2026-08-27): the
  giveaway that arrives *as text* is not covered here and cannot be. A member who types out the
  winners of a giveaway, or pastes a list of handles, writes an ordinary message that is recorded
  and answered like any other, and this unit does not and should not change that. Resolving it,
  if it ever needs resolving, means deciding what the assistant does about members publishing
  other members' names in the ordinary course of conversation, which is a far larger question
  than this family and must not be settled as a side effect of it.
- **Do not build the thing this unit deliberately did not build.** If a later unit wants the
  assistant to acknowledge boosters, it starts by disagreeing in writing with the decision record
  of AC11, and it must answer the consistency argument first: the platform drops updates after 24
  hours, so any acknowledgement scheme silently thanks some members and not others.
