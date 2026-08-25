# Telegram unit 11 — pinned messages: the assistant reads the pin and never writes it

Date: 2026-08-25. Pinning is not a new capability here. It is already the mechanism the
whole rules contract stands on: the group's rules are whatever text sits in its pinned
announcement (decision 0048), the rules note is written into the assistant's system voice
from that text, and the autonomous assessment judges every group message against it
(decision 0094). The reading half shipped on 2026-08-23 and has never been examined against
the platform's actual pin surface since. This unit does that examination, adds the two small
mechanisms the examination shows are missing, and refuses the writing half outright.

The refusal is the important part, so it goes first. **The assistant cannot pin, unpin or
clear pins in a group, and must not be given the ability.** All three methods require the
bot to be an administrator holding `can_pin_messages`, and the operator contract requires
the assistant to stay an ordinary member so its reports reach the moderation bot
(`docs/reference/group-operator-contract.md:112-115`). That alone makes every call a
guaranteed refusal in the shipping configuration. The stronger reason survives even if the
operator ever reversed that instruction: the pin right *is* the trust boundary that lets a
group govern its assistant (decision 0049), so an assistant that can pin can write its own
governing input, and an assistant that can unpin can remove the rules it is judged by. This
unit therefore ships no writing path at all — not behind a permission check, not behind a
configuration key.

What the examination did find is three ways the rules the assistant follows can quietly stop
matching the rules the group actually pinned, and one case where an administrator cannot tell
whether a pin worked. Two of the three are closed here; the third is closed in the operator
contract, because the platform gives no mechanism that could close it in code.

Everything below is checked against the live Bot API page (Bot API 10.3, 24 August 2026,
fetched 2026-08-25) and against this tree at `1891fcd`. Every claim carries its source.

## Grounding

### What the platform actually does

- **`pinChatMessage`** — parameters `business_connection_id` (String, optional), `chat_id`
  (Integer or String, required), `message_id` (Integer, required), `disable_notification`
  (Boolean, optional). The description, verbatim: "Use this method to add a message to the
  list of pinned messages in a chat. In private chats and channel direct messages chats, all
  non-service messages can be pinned. Conversely, **the bot must be an administrator with the
  'can_pin_messages' right** or the 'can_edit_messages' right to pin messages in groups and
  channels respectively. Returns *True* on success."
- **`unpinChatMessage`** — `business_connection_id` (String, optional), `chat_id` (Integer or
  String, required), `message_id` (Integer, **optional**: "Identifier of the message to
  unpin. Required if *business_connection_id* is specified. **If not specified, the most
  recent pinned message (by sending date) will be unpinned**"). Same administrator sentence
  as above.
- **`unpinAllChatMessages`** — `chat_id` only. "In private chats and channel direct messages
  chats, no additional rights are required to unpin all pinned messages. Conversely, the bot
  must be an administrator with the 'can_pin_messages' right … in groups and channels
  respectively."
- **A chat holds a *list* of pinned messages, and the bot can see exactly one of them.**
  `pinChatMessage` "add[s] a message to the list"; `unpinAllChatMessages` "clear[s] the
  list". The only read the API offers is `ChatFullInfo.pinned_message`, described verbatim as
  "*Optional*. The most recent pinned message (by sending date)" — **by sending date, not by
  pin order**, and one message, not a list. There is no enumeration of pins anywhere in the
  API. Decision 0048 recorded this correctly in 2026-08-23 and it still holds.
- **`Message.pinned_message`** is the service message the bot receives: type
  `MaybeInaccessibleMessage`, "*Optional*. Specified message was pinned. Note that the
  Message object in this field will not contain further *reply_to_message* fields even if it
  itself is a reply." `InaccessibleMessage` is "a message that was deleted or is otherwise
  inaccessible to the bot", and its `date` field is "Always 0. The field can be used to
  differentiate regular and inaccessible messages."
- **There is no unpin update, and no update at all for an edited pin.** The complete list of
  `Update` fields is `message`, `edited_message`, `channel_post`, `edited_channel_post`,
  `business_connection`, `business_message`, `edited_business_message`,
  `deleted_business_messages`, `guest_message`, `message_reaction`,
  `message_reaction_count`, `inline_query`, `chosen_inline_result`, `callback_query`,
  `shipping_query`, `pre_checkout_query`, `purchased_paid_media`, `poll`, `poll_answer`,
  `my_chat_member`, `chat_member`, `chat_join_request`, `chat_boost`, `removed_chat_boost`,
  `managed_bot`, `subscription`, `stopped_message_generation`. The only pin-related member is
  the service message inside `message`, and its documented trigger is a message being pinned.
  Nothing reports an unpin, and nothing reports that an already-pinned message's text changed
  other than the ordinary `edited_message` update for that message.
- **Undelivered updates expire in a day.** Verbatim: "Incoming updates are stored on the
  server until the bot receives them either way, but they will not be kept longer than 24
  hours." A pin event that happens while the process is down for longer than that is gone,
  and no later update replaces it.
- **The permission names, exactly.** `ChatMemberAdministrator.can_pin_messages` and
  `ChatAdministratorRights.can_pin_messages` are both "*Optional*. True, if the user is
  allowed to pin messages; for groups and supergroups only".
  `ChatMemberRestricted.can_pin_messages` is required, "True, if the user is allowed to pin
  messages". `ChatPermissions.can_pin_messages` is "*Optional*. True, if the user is allowed
  to pin messages. **Ignored in public supergroups.**" — so in a public supergroup the pin
  right is an administrator matter regardless of the chat-wide permission, which is exactly
  the trust boundary decision 0049 assumed.
- **The methods have been stable for years.** `unpinAllChatMessages` and the `message_id`
  parameter on `unpinChatMessage` arrived with "Multiple Pinned Messages" on 4 November 2020
  (Bot API 5.0); `business_connection_id` was added to both pin methods on 31 July 2024 (Bot
  API 7.8). Bot API 10.0 through 10.3 (8 May, 11 June, 14 July, 24 August 2026) change
  nothing about pinning. There is no rate limit documented for any of the three methods, and
  none is needed here because none of them is called.
- **Forum topics have their own pinned lists and their own methods** —
  `unpinAllForumTopicMessages` and `unpinAllGeneralForumTopicMessages`, both requiring the
  `can_pin_messages` administrator right — and `pinChatMessage` has no `message_thread_id`
  parameter. A rules pin inside a forum topic is therefore outside everything described here;
  the operator contract's instruction to pin the rules in the group is what keeps that case
  out of scope, and it is now stated there instead of assumed.

### What already exists in this tree

- **The rules contract is a pure function over one pinned text.** `read_rules`
  (`crates/core/src/note.rs:130-146`) returns `RulesReading` (`:104-122`) with four
  outcomes: `Rules(text)`, `NotRules`, `RefusedEmpty`, `RefusedOverBound { bytes }`. The
  prefix is `Rules:` (`:52`), the bound is 4096 bytes (`:58`), and an over-bound text is
  refused whole because "a cut rule is a different rule".
- **Three of those four outcomes are thrown away one function later.** `note_of`
  (`note.rs:154-190`) maps `Rules` to a note and every other reading to `None`, logging the
  two refusals at info level (`:174-186`). The caller cannot tell a refused rules pin from an
  ordinary announcement, so the chat cannot be told either.
- **The observation path is where a note is appended and where anything deterministic is
  delivered.** `Assistant::observe` (`crates/core/src/assembly.rs:928-1033`): the erasure
  fence is taken at `:932`, the note branch checks authorization at `:962`, calls `note_of`
  at `:965` and returns silently when it yields nothing (`:965-967`), takes the stamp lock at
  `:974`, resolves or creates the mapping, reconciles the palette (`:986`), reads the newest
  note of the topic (`:987`), returns without appending when the text is identical
  (`:991-993`), appends (`:994-1003`), drops both locks (`:1008-1013`) and returns the
  acknowledgment for a rules delta (`:1024-1029`). The refusal check at `:965` sits **before**
  any lock is taken, which is why the new refusal line costs no lock and no model call.
- **The acknowledgment is a bounded one-shot model call with a fixed fallback.**
  `crates/core/src/acknowledgment.rs:53-58`, ten-second timeout (`:38`), 600-code-point
  output cap (`:48`), falling back to `RULES_ACKNOWLEDGMENT`
  (`crates/core/src/outbound.rs:142-151`).
- **The acknowledgment deliberately carries no rate window.** `crates/core/src/window.rs:6-16`
  records why: "The rules acknowledgment left this bound on 2026-08-23: pinning is an
  administrator-only right, so the pin-toggling spammer the window was built against cannot
  exist, and the window only silenced legitimate rules edits". Decision 0051's second
  refinement is the operator's own decision to that effect.
- **The adapter translates a pin event without reading anything else about it.**
  `translate.rs:138-158`: the pin arm runs ahead of the on-behalf-of-chat skip so an
  anonymous administrator's pin still counts, refuses a non-group chat (`Skip::PinOutsideGroup`),
  refuses the inaccessible form by its zero date (`Skip::InaccessiblePin`), falls back from
  `text` to `caption`, refuses a pin with neither (`Skip::TextlessPin`), and reports
  `ObservedFact::PinnedAnnouncement(text)`. `PinnedContent` (`client.rs:146-160`) decodes
  three fields only: `date`, `text`, `caption`. **The pinned message's identifier is not
  decoded at all**, here or in `ChatInfo` (`client.rs:185-192`).
- **The lookup half reads the same two fields.** `lookup_observations`
  (`translate.rs:254-283`) reports the title, and — only in `LookupScope::Whole` — the
  exposed pin when its date is non-zero and it carries text (`:267-281`). When
  `ChatFullInfo.pinned_message` is absent, **nothing is reported at all**: the vocabulary has
  no way to say "this group exposes no pinned announcement". `LookupScope::TitleOnly`
  (`:242-248`) exists because a pin event outranks the lookup's by-sending-date pin.
- **The zero-date check on the lookup path is inert, and stays.** `ChatFullInfo.pinned_message`
  is typed `Message`, not `MaybeInaccessibleMessage`, so it never carries the zero-date
  discriminator; the check at `translate.rs:269` is only meaningful on the service-message
  path. It is harmless, it costs one comparison, and this unit leaves it alone instead of
  removing a check whose absence would matter if the platform ever widened that field.
- **The lookup runs once per chat per process, and never again.** `LookupMemory`
  (`driver.rs:148-195`): `answered` is a `HashSet<i64>` (`:155`), `skips` returns true
  forever once a chat is in it (`:170-172`), and only an admission voids it
  (`driver.rs:517`, `LookupMemory::void` at `:191-194`). The only other way a chat leaves the
  set is `ANSWERED_MEMORY_CAP` (`:111`) clearing the whole set at 4096 entries
  (`:178-181`) — an accident, not a refresh. `ChatRest` (`:113-146`) is the existing
  expiring-memory structure beside it, used for the failed-lookup rest (`:98`,
  `LOOKUP_RETRY_REST`, one minute) and the withdrawal rest (`:105`).
- **A pin event can be dropped for good.** `report` (`driver.rs:572-599`) treats a terminal
  refusal from the core as handled and acknowledges the update past (`:590-593`), which is
  correct for the batch and permanent for that pin.
- **An edit to the pinned message is skipped by design.** `translate.rs:123-125` returns
  `Skip::EditedMessage`; decision 0017 keeps the ledger as first seen. Combined with the
  platform fact above — no update reports the new text of an already-pinned message — an
  administrator who edits the pinned rules in place changes the group's rules and the
  assistant never learns.
- **The rests measure the standard-library clock.** `ChatRest` holds
  `HashMap<i64, std::time::Instant>` (`driver.rs:56`, `:118`), so no test can advance past a
  rest; the existing tests pin only the inside-the-window half
  (`crates/adapters/telegram/tests/adapter/group_context.rs:239-262`). The core's own windows
  already use the other clock: `crates/core/src/window.rs:33` imports `tokio::time::Instant`
  and the window tests advance it (`window.rs:184-191`).
- **The deterministic return channel exists and is typed.** `DeliveryItem`
  (`crates/core/src/message.rs:246-268`) is "typed by what it is, so an adapter can present
  the kinds differently without reading the text. The core still supplies the exact wording
  for both, because wording is behaviour and behaviour stays out of adapters"; its two
  variants are `Acknowledgment` and `CommandAnswer`. `ObserveOutcome::Observed { deliver }`
  carries it, and `report` sends it (`driver.rs:580-584`).
- **The platform-vocabulary scan is a whole-word check over `crates/core`.**
  `crates/core/tests/vocabulary.rs:64-67` splits each lowercased line on non-alphanumeric
  characters and compares runs against `docs/platform-vocabulary.txt`, whose header invites
  each adapter to add its platform's names. No API method name is on the list today; unit 04
  decided to add `deleteMessage` and `deleteMessages` for exactly this reason
  (`docs/units/telegram/04-deleting-messages.md:445-456`).
- **The record of processing already covers what a pin produces.**
  `docs/privacy/records-of-processing.md:64` — "D4 | Group facts | Channel title, pinned
  rules text, stored as context notes | Note table" — with the retention row at `:112`: "Kept
  while the group is served. A note is superseded when the group's rules are pinned anew."
- **Erasure does not reach a context note, on purpose and with the reasoning recorded.**
  Decision 0055, OPEN: the note quotes the group's published governance, and giving the note
  table an author column would store more personal data in order to make erasure apply.

### The three holes, stated plainly

1. **An in-place edit of the pinned rules is invisible.** No update carries it, the
   `edited_message` update for that message is skipped by decision 0017, and the once-per-
   process lookup never runs again. The assistant keeps judging messages against the previous
   text, indefinitely, with nothing anywhere saying so.
2. **A pin event lost while the process is down beyond 24 hours is lost for good**, as is a
   pin event dropped by the terminal-refusal path at `driver.rs:590-593`. Same consequence.
3. **An unpin produces nothing, and absence cannot be read as a retraction.** There is no
   unpin update, and the lookup's absence of an exposed pin does not mean the rules were
   withdrawn — the exposure is by sending date, so a still-pinned rules message is absent
   from it whenever a newer-sent announcement is also pinned. Any mechanism that treated
   absence as retraction would silently delete a group's rules the moment an administrator
   pinned a newer notice. The operator contract already says "replace, never merely unpin"
   (`group-operator-contract.md:64-69`); what it does not say is how to retire rules
   entirely.

And one smaller thing, which is what an administrator actually notices first: **a rules pin
that declares itself and is refused is answered with silence.** Pin `Rules:` followed by 5000
bytes and the outcome — no note, no acknowledgment, one info line in a log the group cannot
see (`note.rs:179-185`) — is byte-for-byte the outcome of re-pinning the identical rules.
The administrator has no way to tell "already current" from "refused".

## Decisions taken with this unit

- **The assistant never pins, never unpins and never clears pins; no such call exists in the
  tree, 2026-08-25.** `pinChatMessage`, `unpinChatMessage` and `unpinAllChatMessages` are not
  added to the client, and the core gains no vocabulary that could ask for them. Three
  independent reasons, any one of which is sufficient. The operator contract requires the
  assistant to stay a non-administrator (`group-operator-contract.md:112-115`) and all three
  methods require the `can_pin_messages` administrator right in a group, so every call would
  be a refusal. The pin right is the trust boundary the rules contract rests on (decision
  0049): an assistant that can pin can author the rules it is governed by, and a pin it
  performed would come straight back to it as a pin service message and be read as an
  observed fact (`translate.rs:138-158`) — a machine writing its own system voice through the
  platform. And unpinning is an effect on the group's shared surface with no human decision
  point in the mechanism, which decision 0070 forbids; `unpinAllChatMessages` is that with
  the whole list at once.
  *Rejected:* adding the methods behind an administrator check, so a group that promotes the
  assistant gains them. That trades the report path for a capability nobody asked for, and it
  puts the self-authoring loop one configuration change away.
  *Rejected:* pinning in direct chats, where the platform needs no right at all ("In private
  chats … all non-service messages can be pinned"). Pinning the disclosure or the privacy
  answer in someone's private chat is clutter in a space that is theirs, and it buys the
  assistant nothing it cannot say in a message.
  *Rejected:* pinning the rules acknowledgment so the group can find it. The rules are
  already pinned; a second pin of a confirmation is noise, and it needs the administrator
  right anyway.

- **The pinned message's identifier stays undecoded and unstored, 2026-08-25.**
  `PinnedContent` and `ChatInfo` keep their three and two fields; the note row keeps its
  topic and text. The on-delta text comparison (`assembly.rs:987-993`) is the only identity
  the contract needs, and it is the right one: "a cut rule is a different rule" is a
  statement about text, not about which message carried it.
  *Rejected:* storing the pin's origin on the note row so a later mechanism could tell a
  re-pin from a new pin. It stores a pointer to a member-authored message in a table with no
  principal id, which decision 0055 deliberately keeps free of author data, and outside the
  reach of the deletion mirror, which only ever touches the message table
  (`crates/core/src/kind.rs:743-784`). It would also grow the record of processing's D4 entry
  for a fact nothing reads.
  *Rejected:* keeping the id in adapter memory so an `edited_message` update naming it could
  be re-reported as a pin. It is the cheaper fix for hole 1 and it is a real option — but it
  is the adapter deciding that a particular edit becomes an observed group fact, and the
  refresh below closes the same hole with no new state anywhere. Recorded as the named
  follow-up if the delay ever matters.

- **A rules pin that declares itself and is refused says so in the chat, deterministically,
  2026-08-25.** `note_of` stops flattening `RulesReading` into `Option`: it returns a reading
  the caller can act on, and `observe` turns the two refusals into a returned
  `DeliveryItem::Notice` carrying one of two fixed lines. `RefusedEmpty` and
  `RefusedOverBound` only — a pin without the prefix never claimed to be rules, and a pin
  with no text at all is not a claim either. No note is appended, nothing is stored, no lock
  is taken and no model call is made: the refusal is decided at `assembly.rs:965`, before the
  stamp lock at `:974`. The reason to speak is that silence here is indistinguishable from
  "your rules are already current", and the administrator's next action depends on which one
  it was.
  *Rejected:* leaving the refusals in the log. The log belongs to whoever runs the process,
  and the person who needs the answer is the administrator in the chat.
  *Rejected:* generating the line through the model the way the acknowledgment is generated.
  It spends a bounded model call to state a mechanical fact, and the input it would have to
  be given is the over-bound pinned text — the exact untrusted, undelimited input the unit 20
  close already recorded as a rough edge.
  *Rejected:* quoting the pin back ("Rules: … was refused"). The refused text is up to
  whatever the platform accepted as a message; echoing it puts unbounded administrator-
  authored text into the chat under the assistant's name for no gain. The lines name the
  cause and the remedy and quote nothing.

- **That line carries no rate window, 2026-08-25.** It follows the acknowledgment's own rule,
  which the operator decided on 2026-08-23 (decision 0051, second refinement, restated at
  `window.rs:6-16`): pinning is an administrator-only right, the pin-toggling spammer the
  window was built against cannot exist, and the window's only observed effect was silencing
  legitimate rules edits. The refusal line is the case where that harm bites hardest — an
  administrator fixing an over-long rules pin is very likely to pin the corrected version
  within the same five minutes, and a window would answer the first attempt and swallow the
  second.
  *Rejected:* a `LineWindow` over `ACKNOWLEDGMENT_WINDOW`, keyed per channel
  (`window.rs:63-90`). It is the reflex, it is the mechanism the repository already retired
  on this exact path, and it recreates the documented failure.
  *Rejected:* appending a refusal note so the on-delta comparison suppresses repeats. It puts
  a system-voiced row on the ledger stating that something is not the rules, which is a
  sentence the model has no use for and cannot be superseded cleanly.

- **The group's facts are re-read on the group's own activity, at most once every six hours
  per chat, 2026-08-25.** `LookupMemory::answered` becomes a `ChatRest` over a new
  `FACTS_REFRESH_INTERVAL` beside `LOOKUP_RETRY_REST` (`driver.rs:98`), so a chat whose
  lookup answered longer ago than the interval is looked up again on its next update, with
  `LookupScope::Whole`, exactly as first contact does today. Nothing else changes: the same
  `get_chat` call, the same two observations, the same on-delta rule in the core, so a
  refresh that finds the same text appends nothing and says nothing. This closes holes 1 and
  2 with one mechanism and no new state. Six hours because undelivered updates expire at 24
  ("they will not be kept longer than 24 hours"), so the interval bounds the blind window to
  a quarter of the platform's own loss horizon, at a cost of one `getChat` per active group
  per six hours. A silent group draws nothing: the refresh rides an update that was arriving
  anyway, so the assistant never calls into a chat that is not talking.
  *Rejected:* a background timer per chat. It calls into silent chats forever, and it puts a
  second scheduler inside an adapter that has exactly one loop.
  *Rejected:* refreshing on every message, or on a short interval such as an hour. Six times
  the calls, for a fact that changes a few times a year.
  *Rejected:* a core-to-adapter query surface that asks for a refresh when the core thinks
  its note is stale. Decision 0054 rejected exactly that boundary, and the core has nothing
  to base staleness on anyway.
  *Rejected:* making the interval a configuration key. An operator knob nobody asked for; the
  constant sits beside the two existing rests and can be promoted if a deployment ever needs
  it.
  *Accepted consequence, stated:* the refresh reads by sending date. If a group has two rules
  messages pinned and the one with the later sending date is not the one the last pin event
  named, the refresh moves the note to the later-sent one and acknowledges the change. That
  is one flip, it is stable afterwards, and it reports the pin state the platform actually
  exposes. The operator contract's "post fresh, then pin" instruction (`:55-62`) is what
  keeps a group out of that situation, and it now says so for the refresh too.

- **Absence still means nothing, and rules are retired by replacement, 2026-08-25.** A lookup
  that finds no exposed pinned message reports no observation, exactly as today
  (`translate.rs:267-281`), and the refresh does not change that. A group that wants to
  retire its rules pins a fresh `Rules:` message whose text says the group has no written
  rules; the assistant then quotes that to the model, which is true. This is documentation,
  not code.
  *Rejected:* a "no pinned announcement" observed fact that retracts the rules note. Absence
  of an *exposed* pin is not absence of a pin: the exposure is one message chosen by sending
  date, so a rules pin sitting behind a newer announcement is absent from it while being
  perfectly pinned. The mechanism would delete a group's rules the first time an
  administrator pinned a notice, silently, and the assessment would keep running against
  nothing.
  *Rejected:* reading `Rules:` with an empty body as a retraction. That is the exact shape of
  a truncated or half-typed pin, and it now draws the refusal line instead — inventing a
  meaning for it would make the accident destructive.

- **The three method names join `docs/platform-vocabulary.txt`, 2026-08-25.** `pinChatMessage`,
  `unpinChatMessage` and `unpinAllChatMessages` go on the list, so "the core never asks for a
  pin" is checked by the existing scan instead of asserted in prose. Each is a single
  alphanumeric run, none is an English word, and none appears in the core today, so the scan
  stays green and gains real reach. Unit 04 set the precedent with the two deletion methods.
  *Rejected:* adding `pin` or `pinned`. Both are ordinary English and both appear in true
  neutral prose in the core already (`note.rs:48-51`, `:104`, `:124-128`), so the scan would
  fail on correct statements.
  *Rejected:* claiming the invariant without a check. A criterion whose named check cannot
  fail for the property it claims is worse than no criterion.

- **`ChatRest` measures the runtime clock, 2026-08-25.** It switches from
  `std::time::Instant` to `tokio::time::Instant`, the clock the core's own windows already
  use (`window.rs:33`). Every use of `ChatRest` is inside the adapter's async loop, so the
  runtime context is always present, and the switch makes all three rests — the refresh, the
  failed-lookup rest and the withdrawal rest — testable on a paused clock instead of only
  half-testable as they are today.
  *Rejected:* a test-only shortened interval injected through the config. It tests a
  different constant from the one that ships.
  *Rejected:* leaving the clock alone and pinning only the inside-the-interval half, as the
  existing rest tests do. The half that matters for a refresh is the expiry, and a mechanism
  whose expiry is never exercised is a mechanism nobody has run.

## The unit's contract

The assistant reads pinned messages and never writes them: no pin, unpin or clear-pins call
exists in the tree, the three method names are on the forbidden list the core is scanned
against, and the assistant is never made an administrator, so the trust boundary of decision
0049 stays where it was — whoever holds the group's pin right steers the assistant, and the
assistant does not hold it. A pin event still yields one observed fact carrying the pinned
text and nothing else, and the pinned message's identifier is still neither decoded nor
stored. A pinned text that reads as rules and differs from the newest stored note still
appends a note and draws the acknowledgment; an identical one still says nothing; an
announcement without the prefix still supersedes nothing, because the platform keeps a list
of pins and a new announcement removes no rules. What changes is that a pinned text which
declares itself rules and is refused — empty after trimming, or past the 4096-byte bound —
now says so in the chat with one of two fixed lines that quote none of the refused text,
append no block and call no model, while a textless or inaccessible pin stays silent because
it claimed nothing. And the group's facts are re-read from the platform on the group's own
next update once six hours have passed since the last successful lookup, with the same scope,
the same call and the same on-delta rule, so an in-place edit of the pinned rules and a pin
event lost to downtime are both picked up within that window instead of never. Nothing is
rewritten: a refreshed rules text is a new note superseding the old one under the existing
supersession wording, and a refusal writes nothing at all. No schema change, no new block
kind, no new dependency, no new data category, no new recipient, and no bytes: this unit
moves no files, no media and no uploads, and the only unbounded input it touches — the
pinned text — is still refused whole above 4096 bytes and never truncated, before it
reaches storage or the model.

## Acceptance criteria

1. Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the secret
   scan clean; no new dependency and no schema migration.
   `docs/platform-vocabulary.txt` gains `pinChatMessage`, `unpinChatMessage` and
   `unpinAllChatMessages`, and `crates/core/tests/vocabulary.rs` is green with them on the
   list.
2. No pin call exists anywhere in the workspace: a source scan over `crates/` — in the shape
   of the vocabulary scan, `crates/core/tests/vocabulary.rs:64-67` — asserts that none of the
   three method names appears in any source file, including the adapter's client. The test
   states in its own documentation that it is the check behind the "the assistant never pins"
   decision, so deleting it deletes a stated guarantee, not a redundant test.
3. A rules pin refused as empty and one refused as over-bound each return
   `ObserveOutcome::Observed { deliver: Some(DeliveryItem::Notice(line)) }` with the line
   pinned verbatim against its constant; each appends no block (the conversation's block
   count is identical before and after), leaves the newest rules note unchanged, and makes no
   provider request — pinned by the scripted provider recording zero calls, which also proves
   the refusal path never reaches the acknowledgment generation.
4. Neither line contains any fragment of the refused pin: the fixture's pinned text carries a
   marker string, and the delivered line is asserted not to contain it.
5. The over-bound line's number cannot drift from the bound: a test asserts the line contains
   `RULES_TEXT_MAX_BYTES` rendered in decimal.
6. Silence stays silence where the pin claimed nothing: a pinned text without the prefix, a
   pin with neither text nor caption, an inaccessible pin (zero date) and an identical re-pin
   each deliver nothing and append nothing — the first three pinned in the adapter's
   translation tests where they already live (`translate.rs:928-958`), the last in the spine.
7. The refresh's expiry is pinned in `driver.rs`'s own `mod tests` on a paused clock: a
   `LookupMemory` that recorded a chat as answered skips it, still skips it just before
   `FACTS_REFRESH_INTERVAL` has passed, and stops skipping it once the clock is advanced past
   it; a recorded failure keeps its own one-minute rest independently; and an admission still
   voids both.
8. The refresh's wiring is pinned end to end in the adapter suite: two messages from the same
   group inside the interval draw exactly one `getChat` (the shape
   `group_context.rs:239-262` already uses), and a group whose refreshed lookup returns a
   different pinned rules text appends a second rules note and delivers a second
   acknowledgment, with the first note left standing on the ledger — the append-only reading
   asserted, not assumed.
9. A refreshed lookup that returns the same title and the same rules text appends nothing and
   delivers nothing, so an active group pays one platform call per interval and no chat
   noise.
10. A failed refresh changes nothing: the `getChat` error is logged, the update batch
    continues, the previous notes stand, and the chat's next update past the failure rest
    retries — pinned in the adapter suite.
11. The document changes ship with the code and are pinned by `crates/assistant/tests/docs.rs`
    in its existing shape: the operator contract's four new statements (the assistant never
    pins, how to retire rules, what the refusal lines say, and that the group's facts are
    re-read on activity), the record of processing's amended D4 retention row, and the unit's
    decision records.

## Notes for launch

Exact sites, from the reading above. All anchors verified at `1891fcd`.

- **Core, the note reading** (`crates/core/src/note.rs`): `note_of` (`:154-190`) returns a
  reading instead of an `Option` — a note with its topic and text, a refusal with its cause,
  or nothing — and the two `tracing::info!` calls at `:174-186` move to the one caller, so
  the log line and the delivered line are decided in the same place. `read_rules`, the
  prefix, the bound and `RulesReading` are untouched; this is plumbing that was already
  computed and discarded.
- **Core, the two fixed lines** (`crates/core/src/outbound.rs:142-151`): two constants beside
  `RULES_ACKNOWLEDGMENT`, in its documented shape. Drafts, to be confirmed by the copy pass
  the repository applies to member-facing text:
  - empty — "That pinned message starts with \"Rules:\" but has no rules text under it, so
    the rules did not change. Pin a message with the rules written under that first line."
  - over-bound — "That pinned message starts with \"Rules:\" but the rules text is longer
    than the 4096-byte limit, so the rules did not change. Pin a shorter version."
- **Core, the delivery vocabulary** (`crates/core/src/message.rs:246-268`): `DeliveryItem`
  gains `Notice(String)` and `text` gains its arm. Note the contention: unit 04
  (`docs/units/telegram/04-deleting-messages.md:558-562`) also adds a variant here and changes
  `text` to return `Option<&str>`; whichever unit merges second takes the other's signature,
  and `Notice` returns `Some` under either. This file is not edited by this unit's spec.
- **Core, the observation path** (`crates/core/src/assembly.rs:961-967`): the `let … else` at
  `:965-967` becomes a match over the new reading. The refusal arm returns
  `Observed { deliver: Some(Notice(…)) }` immediately, before the stamp lock at `:974` — no
  lock, no mapping resolution, no palette reconciliation, no model call. The note arm is
  unchanged from `:974` onward.
- **Adapter, the rest structure** (`crates/adapters/telegram/src/driver.rs:113-146`):
  `ChatRest` holds `tokio::time::Instant`; the import at `:56` splits accordingly.
  `LookupMemory` (`:148-195`) replaces `answered: HashSet<i64>` with
  `answered: ChatRest::new(FACTS_REFRESH_INTERVAL)`; `skips` (`:170-172`) reads
  `answered.resting(chat_id) || failed.resting(chat_id)`; `record_answered` (`:176-183`)
  records into the rest and keeps its cap by asking the rest for its size and clearing it at
  `ANSWERED_MEMORY_CAP`, which needs two one-line methods on `ChatRest`; `void` (`:191-194`)
  forgets both. The new constant sits beside `LOOKUP_RETRY_REST` (`:98`) with its own
  reasoning, naming the platform's 24-hour update retention.
- **Adapter, the module documentation** (`driver.rs:34-51`): the paragraph that says "once per
  group chat per process" becomes the refresh's description, and the sentence about the
  answered memory being "once per cap epoch" is restated. `first_contact`'s own doc
  (`:524-531`) follows. Unit 05 adds a branch to `observed` (`:479-522`) for its own
  observation kind (`docs/units/telegram/05-polls.md:849`); the two changes do not touch the
  same lines, and this unit does not edit that file.
- **Documentation, the operator contract** (`docs/reference/group-operator-contract.md`): the
  rules-pin section (`:30-53`) gains what the assistant says when a pin declares itself rules
  and is refused; "First setup: post fresh, then pin" (`:55-62`) gains the sentence that the
  periodic re-read also follows sending date; "Replace rules, never merely unpin them"
  (`:64-69`) gains how to retire rules entirely and gains the fact that an in-place edit of
  the pinned message is picked up on the next re-read and not at once; the trust-boundary
  section (`:71-78`) gains one sentence saying the assistant holds no pin right, performs no
  pin, unpin or clear, and is never promoted to gain one — beside the existing
  non-administrator instruction at `:112-115`. One sentence names forum topics: the rules pin
  belongs to the group's own pinned list, because a pin inside a topic is a separate list the
  assistant does not read.
- **Documentation, the record of processing**
  (`docs/privacy/records-of-processing.md:112`): the D4 retention row's second sentence
  becomes true of both paths — a note is superseded when the group's rules are pinned anew,
  or when the periodic re-read of the group's facts finds the exposed pinned rules changed.
  D4 itself (`:64`) is unchanged: same category, same content, same table, same recipients.
  The DPIA and the LIA need no edit and the reason is stated here instead of left implied —
  no new category of data is collected, nothing new is sent to the model provider (the rules
  text already rides the acknowledgment generation, `acknowledgment.rs:53-58`), no new
  recipient exists, and the refusal lines carry no member data at all.
- **Decision records** (`docs/decisions/`): one record per decision above, written from the
  next free number at merge time — the sibling specs in this folder claim 0106 onward
  (`01-receiving-media.md:236`, `02-sending-media.md:643`, `03-editing-messages.md:611`), so
  the numbers are assigned when this unit merges, not now. Decision 0054 gains a dated
  refinement: "once per channel per process" becomes "once per channel per refresh interval",
  with the reason. Decision 0048 gains a dated refinement recording that an in-place edit of
  the pinned message is invisible on the wire and is picked up by the re-read, and that
  retirement is by replacement text.
- **Follow-up** (`docs/follow-ups.md`): the faster path for hole 1, recorded and not built —
  the adapter remembering the identifier of the pin its own lookup exposed, and re-reporting
  the text when an `edited_message` update names that identifier, which would cut the pickup
  delay from hours to seconds at the cost of one decoded field, one per-chat memory and one
  more place where the adapter decides that an update becomes an observation.
