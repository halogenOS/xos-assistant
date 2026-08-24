# Telegram unit 06 — reactions: the assistant may place one, and cannot see the group's

Date: 2026-08-25, revised the same day after two independent reviews. A reaction is the
cheapest thing a chat platform offers: an acknowledgement that costs no message, makes no
notification storm, and does not push the conversation down the screen. For an assistant
whose default is silence (decision 0098) that is a real capability — "I read this, I owe you
no words" without spending a message on saying so.

The unit ships **one half of the feature and refuses the other, on purpose**. Placing a
reaction works: `setMessageReaction` needs no administrator rights and the assistant is an
ordinary member. **Reading other people's reactions does not work and cannot be made to
work under the current operator contract**: both reaction update types are delivered only to
a bot that is an administrator of the chat, and the operator contract requires the assistant
to stay a non-administrator so its reports reach the moderation bot. The two requirements
are in direct conflict, the conflict is not ours to resolve here, and the receiving half is
therefore specified as a blocked capability with its exact preconditions written down — not
built, not stubbed, not subscribed to.

Two things in this unit stay unproven and no shipped code depends on either: whether an
empty `reaction` array withdraws a placed reaction, and whether `setMessageReaction` behaves
in one-to-one chats. Both are inferences from wording the platform never states directly,
this repository has no path for a live call against the real endpoint (the whole adapter
suite runs against a loopback fake, `crates/adapters/telegram/tests/adapter/server.rs:1-11`),
and the sections below say plainly which design choices rest on them and which do not.

Everything else is checked against the live Bot API page (Bot API 10.3, 24 August 2026,
fetched 2026-08-25) and against this tree. Every claim carries its source.

## Grounding

### What the platform actually does

- **`setMessageReaction`** — parameters `chat_id` (Integer or String, required),
  `message_id` (Integer, required), `reaction` (Array of `ReactionType`, optional), `is_big`
  (Boolean, optional). The description, verbatim: "Use this method to change the chosen
  reactions on a message. Service messages of some types can't be reacted to. Automatically
  forwarded messages from a channel to its discussion group have the same available
  reactions as messages in the channel. Bots can't use paid reactions. Returns *True* on
  success." No administrator right is required and none is mentioned.
- On `message_id`, verbatim: "Identifier of the target message. If the message belongs to a
  media group, the reaction is set to the first non-deleted message in the group instead."
- The `reaction` parameter's description is the operative limit: "A JSON-serialized list of
  reaction types to set on the message. **Currently, as non-premium users, bots can set up to
  one reaction per message.** A custom emoji reaction can be used if it is either already
  present on the message or explicitly allowed by chat administrators. Paid reactions can't
  be used by bots."
  - Note the current wording allows custom emoji **conditionally**. Older summaries of this
    method say a bot cannot use custom emoji at all; that is out of date. Paid reactions
    (`ReactionTypePaid`, added in Bot API 7.9, 14 August 2024) are excluded outright.
  - The parameter is described as the list to *set*. Setting an empty list therefore sets no
    reactions, which is how a placed reaction would be withdrawn. **The documentation does
    not say this in words. It is an inference, it is not proven, and nothing this unit ships
    depends on it** — see the supersession decision below, which now says what a withdrawal
    would cost instead of assuming the mechanism.
  - The description names no restriction by chat type. It also confirms none. **That the
    method works in a one-to-one chat is an inference from the absence of a restriction**,
    not a documented fact, and the decision below that ships marks in direct chats is written
    to survive being wrong.
- **`ReactionTypeEmoji.emoji`** — "Reaction emoji. Currently, it can be one of …" followed
  by a closed list of **73 emoji**, rendered on the page as images. Extracted from those
  images' alt text and counted: ❤ 👍 👎 🔥 🥰 👏 😁 🤔 🤯 😱 🤬 😢 🎉 🤩 🤮 💩 🙏 👌 🕊 🤡 🥱
  🥴 😍 🐳 ❤‍🔥 🌚 🌭 💯 🤣 ⚡ 🍌 🏆 💔 🤨 😐 🍓 🍾 💋 🖕 😈 😴 😭 🤓 👻 👨‍💻 👀 🎃 🙈 😇 😨 🤝
  ✍ 🤗 🫡 🎅 🎄 ☃ 💅 🤪 🗿 🆒 💘 🙉 🦄 😘 💊 🙊 😎 👾 🤷‍♂ 🤷 🤷‍♀ 😡.
  **A hazard the implementer must not skip:** several of these are ambiguous as byte
  sequences. The alt text gives ❤ as U+2764 alone, ✍ as U+270D alone, ☃ as U+2603 alone,
  🤷‍♂ as U+1F937 U+200D U+2642 — all without the U+FE0F variation selector that a text
  editor or a copy-paste from a chat client will silently add. The page states no wire form
  in prose. Whichever emoji this unit sends must be pinned as an explicit escape sequence in
  source, never as a pasted literal, and its bytes asserted independently in a test.
- **`ChatFullInfo.available_reactions`** — "*Optional*. List of available reactions allowed
  in the chat. If omitted, then all emoji reactions are allowed." A group can restrict the
  set, so a reaction that is globally legal can still be refused in one particular group.
- **`ChatFullInfo.max_reaction_count`** — "The maximum number of reactions that can be set on
  a message in the chat" (added Bot API 7.3). Not binding here: a bot may set one anyway.
- **The two `can_react_to_messages` fields differ, and the difference matters.** Both were
  added in Bot API 10.0, 8 May 2026, and only one of them carries a default:
  - `ChatPermissions.can_react_to_messages`, Boolean: "*Optional*. True, if the user is
    allowed to react to messages. If omitted, defaults to the value of `can_send_messages`."
  - `ChatMemberRestricted.can_react_to_messages`, Boolean, **required, with no default
    clause**: "True, if the user is allowed to react to messages."

  A group that restricts this permission can switch the capability off for the assistant
  without telling it, and the assistant learns about it only from a refused call.
- **The two update types differ in who they are about, not in degree.**
  - `MessageReactionUpdated` — "This object represents a change of a reaction on a message
    performed by a user." Fields: `chat` (Chat), `message_id` (Integer), `user` (User,
    "*Optional*. The user that changed the reaction, if the user isn't anonymous"),
    `actor_chat` (Chat, "*Optional*. The chat on behalf of which the reaction was changed, if
    the user is anonymous"), `date` (Integer), `old_reaction` and `new_reaction` (Array of
    `ReactionType`). It is **per person and per change**, and it carries the before and after
    state, so an add and a removal are the same update shape with different arrays.
  - `MessageReactionCountUpdated` — "This object represents reaction changes on a message
    with anonymous reactions." Fields: `chat`, `message_id`, `date`, `reactions` (Array of
    `ReactionCount`, each `{type, total_count}`). It is **aggregate and anonymous**: totals
    only, nobody named, no before-state. Which of the two a chat produces is the platform's
    own choice based on whether that message's reactions are anonymous; the documentation
    states no rule for when that happens, so the spec claims none.
- **Neither update arrives by default and neither arrives to a non-administrator.**
  `Update.message_reaction`: "*Optional*. A reaction to a message was changed by a user. **The
  bot must be an administrator in the chat** and must explicitly specify `"message_reaction"`
  in the list of *allowed_updates* to receive these updates. **The update isn't received for
  reactions set by bots.**" `Update.message_reaction_count` says the same about the
  administrator requirement and the explicit subscription, and adds "The updates are grouped
  and can be sent with delay up to a few minutes." `getUpdates.allowed_updates`: "Specify an
  empty list to receive all update types except *chat_member*, *message_reaction*, and
  *message_reaction_count* (default)."
- **Removing somebody else's reaction is an administrator power, and only in groups.**
  `deleteMessageReaction` (chat_id, message_id, optional user_id, optional actor_chat_id) —
  "Use this method to remove a reaction from a message in a group or a supergroup chat. The
  bot must have the 'can_delete_messages' administrator right in the chat."
  `deleteAllMessageReactions` (chat_id, optional user_id, optional actor_chat_id) — "Use this
  method to remove up to 10000 recent reactions in a group or a supergroup chat added by a
  given user or chat", same administrator right. Both added in Bot API 10.0, 8 May 2026.
- **Rate limiting**: the API page documents no numeric ceiling for this method. The only
  documented signal is `ResponseParameters.retry_after` — "In case of exceeding flood
  control, the number of seconds left to wait before the request can be repeated" — which
  this adapter already honours (`crates/adapters/telegram/src/client.rs:57` `TOO_MANY_REQUESTS`,
  `:549` `stated_wait`, `:29` `RATE_LIMIT_ATTEMPTS`).
- Reactions arrived in Bot API 7.0 (29 December 2023); reacting to most service messages was
  allowed in Bot API 8.3 (12 February 2025). Nothing in Bot API 10.0 through 10.3 changes the
  method's shape.

### What already exists in this tree

- **The adapter names its update subscription explicitly.**
  `crates/adapters/telegram/src/client.rs:103` —
  `pub(crate) const CONSUMED_UPDATE_TYPES: [&str; 3] = ["message", "edited_message", "my_chat_member"];`
  — sent on every poll at `:319`. Neither reaction update type is in it, which matches the
  platform default and this unit's decision.
- **The outbound edge is a single typed channel of one item type.**
  `Assistant::replies` (`crates/core/src/assembly.rs:1048-1051`) returns
  `mpsc::UnboundedReceiver<OutboundReply>`; the adapter takes it once at
  `crates/adapters/telegram/src/driver.rs:262`, selects over it at `driver.rs:283-287`, and
  drains it in `consume_replies` (`driver.rs:730-760`). `OutboundReply`
  (`crates/core/src/message.rs:373-397`) is
  `{channel, text, kind: ReplyKind, reply_target: Option<String>}` — every item on the edge
  is prose today.
- **`message.rs` already carries a differently-purposed item enum.**
  `DeliveryItem { Acknowledgment(String), CommandAnswer(String) }`
  (`crates/core/src/message.rs:246-258`, re-exported at `crates/core/src/lib.rs:85-89`) is
  what a **deterministic call returns synchronously** on its receipt, never what the
  asynchronous replies channel yields. The two are separate paths and the new enum must say
  so on its face; see the decision below.
- **The reply target already threads.** `OutboundReply::reply_target` is translated at
  `driver.rs:742-745` through `translate::message_id_of` and handed to
  `BotClient::send_message`, which sets `reply_parameters` with
  `allow_sending_without_reply: true` on the first chunk only (`client.rs:451-455`,
  `:371-390`). The origin string a reaction needs to name a target is the same string, and
  the same `message_id_of` parses it.
- **The report tool is the exact structural precedent for "the model names a member's
  message and the core acts on it".** `crates/core/src/tools/report.rs` gives the whole
  pattern: a block kind with a nullable `target_origin` and a NOT NULL
  `reported_principal_id` (`:69-74`, schema at `crates/core/src/schema.rs:296-308`); a tool
  whose one parameter is the projected message id (`report.rs:200-202`); validation of that
  id against the turn's own assessment set through `co_summoners`
  (`crates/core/src/tools/provenance.rs:109`, used at `report.rs:283-320`); a per-origin
  duplicate check (`report.rs:309-318`); a filing lock plus the shared erasure fence
  (`report.rs:334-346`); delivery by the outbound edge, not by the tool
  (`crates/core/src/outbound.rs:489-495`); and an erasure pass keyed on the stored principal
  (`report.rs:172-184`, composed at `crates/core/src/erasure.rs:148`).
- **The report's per-origin check is an existence check, not a newest-wins read.**
  `report.rs:309-318` is `ledger.iter().any(|block| … report.target_origin.as_deref() ==
  Some(origin))`. Any design that wants a later block to supersede an earlier one for the
  same origin needs a different read, not this one.
- **The model can already name a message.** Every user-voiced live block with a stored origin
  projects with `projected_origin_mark` (`crates/core/src/kind.rs:181`, applied at `:565`),
  so a tool taking a message id needs no new projection work.
- **Blocks compose through one kind enum.** `AssistantKind`
  (`crates/core/src/kind.rs:1117-1134`) already carries `ChatMessage`, `ToolPalette`,
  `ContextNote` and `Report` beside the framework delegate; a new kind joins there and its
  table joins the appended migration list at `crates/core/src/schema.rs:376-392`.
- **Four readers decide what a new block kind means, and only two of them are caught by the
  compiler.** This is the most dangerous part of adding a kind to this tree:
  - `Agency::frontier_transparent` defaults to **false — opaque**
    (`/home/claude/projects/agent-ledger/crates/agent-ledger/src/agency/mod.rs:150-167`,
    documented as the fix for "the verified burial defect"). `Report` overrides it to `true`
    with its reason at `crates/core/src/tools/report.rs:150-157`: "the block is written INTO
    a live turn's window by the tool, so the owed-turn decision must read through it — a
    report over an unanswered message buries nothing." A defaulted implementation is silent.
  - `DEBT_READ_THROUGH` (`crates/core/src/assembly.rs:59-63`) is the consumer's own list of
    kinds the owing-tail walk reads through: `CONTEXT_NOTE_KIND`, `TOOL_PALETTE_KIND`,
    `report::REPORT_KIND`. It is a string list, so a missing entry is silent.
  - The tail-debt match at `crates/core/src/assembly.rs:1558-1571` is exhaustive over the
    named variants with no catch-all, so a new variant is compile-caught there and the author
    is forced to place it.
  - `provenance.rs::chain_step` (`crates/core/src/tools/provenance.rs:226-249`) ends in
    `_ => ChainStep::Extends`, so a new variant falls through **silently**. `Report` names
    itself at `:246` deliberately, with the reason inline. `origin_reading`'s span filter
    (`provenance.rs:88-93`) is exhaustive and compile-caught.
  - `outbound.rs::deliverable_of` (`crates/core/src/outbound.rs:471-500`) ends in
    `_ => None`, so a new kind is silently undeliverable until an arm names it.
- **Tool admission and teaching share one predicate, by rule.** `crates/core/src/teaching.rs:29-36`:
  "One predicate for the teaching and the registration, so the prompt can never instruct a
  tool the palette does not carry, and a registered tool is never left untaught." The report
  tool's registration is inside `if let Some(handle) = moderation_handle && crate::teaching::
  moderation_taught(true, answering)` (`crates/core/src/assembly.rs:442-450`); the privacy
  tool's, three lines below at `:456-465`, is unconditional. `ToolSet::admit` is at
  `crates/core/src/tools/mod.rs:116`. A conversation's palette is reconciled on its first
  activity per process (`assembly.rs:671`), so a newly registered tool reaches existing
  conversations without a migration.
- **The protection budgets already bound anything keyed on the co-summoner set.** A
  budget-refused message is stamped `limited = Some(…)` (`assembly.rs:1246`, `kind.rs:336`),
  `ChatMessage::own_debt_taken` is `addressed && limited.is_none()` (`kind.rs:536-538`), and
  `co_summoners` filters on exactly that (`provenance.rs:127`). A message from a person
  over budget is therefore not in the set at all.
- **In helpful answering every message is a co-summoner, including one merely overheard.**
  `resolved_summons` stores `summoned = message.addressed || answering == Helpful`
  (`assembly.rs:1246`) into the same `addressed` column the co-summoner filter reads
  (`kind.rs:290-294`, `:336`). So in the shipping mode the aiming check does **not** rule out
  a message that never spoke to the assistant, and the "where a mark is noise" teaching below
  is the only thing that does.
- **A block appended and not yet delivered is lost across a restart.** `seed_cursors`
  (`crates/core/src/outbound.rs:273-295`) starts every conversation's cursor at its newest
  stored block, so nothing already stored when the edge is taken is ever delivered. The file
  states the same consequence for reports at `:85-89`: "a report undelivered when the process
  dies is LOST".
- **The outbound edge wakes on three things only.** `CoreEvent::StreamDone`,
  `CoreEvent::StreamError` and a bus lag (`crates/core/src/outbound.rs:186-262`). Nothing is
  delivered while the model is still writing.
- **The composing cue is off during a tool call and stops on the same terminal set.**
  Decision 0103: "The cue begins when the model starts, stops when a tool call goes out,
  resumes when its result returns the turn to the model's thinking and streaming."
  `crates/core/src/message.rs:346-350` restates it. `crates/core/src/composing.rs:164-187`
  stops on `StreamDone | StreamError | StreamClosed` — the same events that wake the outbound
  edge. `consume_replies` calls `typing.stop(chat_id)` before every send at `driver.rs:740`.
  The two edges only disagree under a lag: the outbound edge re-reads stored state
  (`outbound.rs:250-256`) while the composing edge stops every open signal instead
  (`composing.rs:195-204`).
- **The client's rate-limit ceiling is per caller, and the reason is written down.**
  `client.rs:490-504`: "The ceiling applies only where a caller asks for it: the send holds a
  queue of pending replies behind it … so for those a stated wait past the caller's ceiling
  fails the call at once." `MAX_RATE_LIMIT_WAIT` is one minute (`client.rs:40-45`),
  `CHAT_ACTION_WAIT_CEILING` is one `TYPING_REFRESH`, four seconds (`client.rs:47-54`,
  `driver.rs:79`), and `RATE_LIMIT_ATTEMPTS` is 3 (`client.rs:29`). The outbound consumer is
  sequential, so a parked call delays every later reply.
- **The deletion mirror reaches the message table only.** `kind::erase_message_named`
  (`crates/core/src/kind.rs:743-784`) nulls the target row's five columns and then every other
  row's reply reference, both inside `CHAT_MESSAGE_TABLE`. The report's `target_origin` is not
  reached, so any new table holding a message identifier is outside the mirror's reach unless
  this unit puts it there.
- **The platform-vocabulary scan cannot see an emoji, and cannot be made to.**
  `crates/core/tests/vocabulary.rs:63-67` matches whole runs of `is_alphanumeric` characters
  against `docs/platform-vocabulary.txt`; an emoji is not alphanumeric, so it is a separator
  and never a token. The list holds seven platform and SDK names and its own maintenance rule
  keeps it to those. Adding the common English word `reaction` would also fail on an
  unrelated line, `crates/core/tests/spine/support.rs:549`. A different check is needed, and
  the tree makes one easy: the core's sources today contain exactly four non-ASCII
  codepoints — U+2014, U+2500, U+2026, U+2013 — all punctuation and box drawing.
- **A log-line count needs its own test binary.** `crates/adapters/telegram/tests/token_scan.rs:6-12`:
  the capture subscriber is installed as the process-wide default, "so a process-wide default
  can only be owned by a test that shares its process with nothing else." The `adapter`
  integration target runs many tests in one process, so no assertion there can count log
  lines. The loopback fake does record every request
  (`crates/adapters/telegram/tests/adapter/server.rs:1-11`), so wire effects are assertable
  there and log volume is not.
- **Sibling Telegram specs contend for the same seam.** `docs/units/telegram/05-polls.md:248-257`
  proposes exactly this change to `Assistant::replies` — an item enum with
  `Reply(OutboundReply)` unchanged plus its own arms — and names it `Outbound`.
  `docs/units/telegram/07-buttons-and-callbacks.md:528` adds an option set to
  `OutboundReply`, and `04-deleting-messages.md:314` adds an opaque handle to it. This unit
  does not edit those files; it adopts the name unit 05 chose so the two converge on one type.

### The conflict, stated plainly

`docs/reference/group-operator-contract.md:112-115` states, as an operator instruction:

> **The assistant is NOT a group administrator.** The moderation bot ignores administrators'
> reports, so an administrator assistant files into silence. Keep the assistant an ordinary
> member and turn its privacy mode off instead of promoting it.

The Bot API requires administrator status in the chat for both `message_reaction` and
`message_reaction_count`. There is no partial version of this: the requirement is on the
update itself, not on a right that can be granted separately. So under the shipping
configuration the assistant will receive **zero** reaction updates, however it is
subscribed. Subscribing anyway would add two names to `CONSUMED_UPDATE_TYPES`, two decode
paths and two translation branches that never execute — dead code with a privacy notice
attached to it. This unit does not write them.

Two further facts shrink the receiving half below what it first looks like, even if the
operator ever reversed that instruction: the assistant would not see reactions set by bots
at all ("The update isn't received for reactions set by bots"), and in any chat whose
reactions are anonymous it would receive only totals with nobody named, which is a fact
about a message and not about a person — useful for nothing the assistant does today.

## Decisions taken with this unit

- **Ship the placing half, refuse the receiving half, 2026-08-25.** The assistant may place
  a reaction on a message; it subscribes to neither reaction update type and decodes
  neither. The receiving half is blocked by the platform's administrator requirement against
  the operator contract's non-administrator instruction, and the conflict is the operator's
  to resolve, not this unit's.
  *Rejected:* promoting the assistant to administrator so the updates flow — that breaks the
  report path the moderation unit depends on (`group-operator-contract.md:112`), trading a
  shipped capability for a speculative one.
  *Rejected:* subscribing to the updates anyway "so they are ready" — the platform sends
  nothing to a non-administrator, so every line of that path would be untested by
  construction and would still need the privacy documents to describe collection that never
  happens.
  *Rejected:* holding the whole unit until the conflict is settled — the placing half needs
  no administrator right, is independently useful, and its absence is what makes the
  assistant spend a whole message on "noted".

- **The core learns the word "mark", not the word "emoji", 2026-08-25.** The neutral
  vocabulary is a closed enum `Mark` in `crates/core/src/mark.rs`, one variant in this
  unit: `Mark::Seen` — "the assistant read this and owes no words". The core decides *which
  mark*; the adapter holds the one-line table from `Mark` to its platform's token. An emoji
  list is a platform fact (Telegram's 73, Matrix's free-form strings, another platform's own
  set), and translating a neutral enum into a platform token is translation, not behaviour —
  the same shape as `ReplyKind` today, which lets an adapter present kinds differently
  without reading the text (`message.rs:331-346`).
  *Rejected:* the core storing the emoji literal, on the precedent that "the core still
  supplies the exact wording, because wording is behavior" (`message.rs:246-250`). That
  precedent is about prose. A fixed-vocabulary token is a representation, and putting
  Telegram's 73-emoji list in the core would import the platform's vocabulary wholesale under
  a different spelling.
  *Rejected:* a free-form string mark chosen by the model. It hands the model the entire
  reaction vocabulary, including 🤡, 💩, 🖕 and 👎, and pushes validation into the adapter,
  which is where decisions must not live.

- **"The core carries no emoji" gets a check that can actually fail, 2026-08-25.** The
  platform-vocabulary scan is structurally unable to see an emoji, so this unit does not
  claim it as evidence. Two mechanisms replace the empty claim:
  - a new test beside the vocabulary scan asserting that every non-ASCII character in
    `crates/core/src` is one of a short committed allowlist — today U+2014, U+2500, U+2026,
    U+2013, the four the sources already use — so any glyph entering the core fails loudly
    and a new punctuation mark is a deliberate one-line addition;
  - four Bot API names added to `docs/platform-vocabulary.txt` — `setmessagereaction`,
    `reactiontypeemoji`, `messagereactionupdated`, `messagereactioncountupdated` — each a
    single alphanumeric run, each a platform identifier and not an English word, and none
    of them present in the core today, so the existing scan stays green and gains real reach.

  *Rejected:* adding the word `reaction` to the list. It is ordinary English, it would fail
  on `crates/core/tests/spine/support.rs:549` ("the taught reaction to a result"), and the
  list's own maintenance rule keeps it to platform and SDK names.
  *Rejected:* leaving the claim in the acceptance criteria as it stood. A criterion whose
  named check cannot fail for the property it claims is worse than no criterion, because a
  green run reads as proof.

- **One mark now, and a second one costs an appended migration either way, 2026-08-25.** The
  unit ships `Mark::Seen` alone. The stored block carries the mark's own column from the
  first migration, with the vocabulary frozen in a CHECK the way the authority vocabulary is,
  because **the record must say which mark was placed**: the record of processing describes a
  stored mark name, and a table that stored only "a mark happened" would make the second mark
  unreadable in the history the first one wrote.
  The earlier draft of this decision claimed the column made a second mark cheap — "no schema
  change". That was wrong, and this tree says so: a column CHECK cannot be altered in place,
  so a widening step recreates the table (`schema.rs:229-239`), every widening is appended to
  `store_config()` (`schema.rs:376-392`), and the pin test at `schema.rs:397-432` fails on
  purpose the moment the enum grows. A second mark is an appended migration whether or not the
  column exists. The column is here for the record's sake, and the honest cost is written down.
  *Rejected:* hard-coding a single mark with no column. Cheaper today by one migration step,
  and it loses the fact the privacy documents describe.
  *Rejected:* shipping three marks now (seen / agreed / working). "Working" duplicates the
  composing cue that unit 18 and unit 22 already deliver (`crates/core/src/composing.rs`), and
  "agreed" is an endorsement the assistant has no basis to give.

- **The glyph is 👀 (U+1F440), pinned as an escape sequence, 2026-08-25.** It is in the
  platform's allowed list, it is a single codepoint with no variation-selector ambiguity (the
  hazard named above), and it reads as "seen" without claiming approval.
  *Rejected:* 👍 — it reads as endorsement of the message's content, which is a judgement the
  assistant is not making and, on a rule-breaking message, would be an actively wrong one.
  *Rejected:* ✍ and ☃-class glyphs — their documented form lacks U+FE0F and their wire form
  is unproven, which is a needless risk for a cosmetic choice.
  *Rejected:* making the glyph configurable per deployment in this unit. It widens the
  operator surface for no asked-for benefit; the constant sits alone in the adapter and a
  later unit can promote it if a group asks.

- **No negative, mocking or judging mark exists, structurally, 2026-08-25.** The `Mark` enum
  contains no such variant and never will without a decision of its own. A 👎, 🤡, 💩 or 🖕
  placed by the assistant on a member's message is a public judgement of that person,
  authored by a machine, with no human decision point anywhere in the mechanism — which is
  precisely what decision 0070 forbids. The assistant assesses and files a report;
  administrators decide. A reaction that judges is a moderation effect wearing a cheaper
  costume, and it is worse than the report because it is visible to the whole group instead
  of to the administrators.
  *Rejected:* teaching the model not to use negative marks while leaving them reachable. A
  structural absence cannot be prompted around; a taught rule can.
  *Rejected:* a "flagged" mark that quietly signals the moderation queue. Same objection, plus
  it would leak the assessment to everyone who can see the message.

- **The model places the mark through a tool; nothing places one mechanically, 2026-08-25.**
  A new tool `mark_seen`, one parameter `message_id`, modelled on the report tool: the id is
  validated against the turn's own co-summoner set (`provenance.rs:109`), so the model can
  mark a message it is actually reading and nothing else — not an old message, not an
  invented id, not another channel's. A mark is a judgement about whether words are owed, and
  unit 22 already moved that judgement from the machinery to the model.
  *Rejected:* a mechanical rule ("mark every message that produced no answer"). That marks
  every passing line in a busy group, which is the noise this unit is supposed to avoid, and
  it is the machine deciding again.
  *Rejected:* marking as a substitute for the failure notice (`FAILURE_NOTICE`,
  `crates/core/src/outbound.rs:114`). A failed turn owes the asker words, not a glyph.

- **The tool registers unconditionally, and its teaching composes unconditionally,
  2026-08-25.** One predicate for both, as `teaching.rs:29-36` requires — and here the
  predicate is "always", so the two cannot drift. Registration sits beside the privacy tool's
  at `assembly.rs:456-465`, not beside the report's at `:442-450`. The report's condition of a
  moderation handle and helpful answering is about a capability that needs a moderation bot
  to receive it; a mark needs nothing but a chat. Making the mark inherit that condition would
  remove it from the addressed-only mode, which is the mode where a short acknowledgement is
  most often the whole of what is owed.
  *Rejected:* registering beside the report under the moderation predicate. It would tie a
  cosmetic acknowledgement to a moderation deployment for no reason anyone could state.
  *Rejected:* conditioning registration on the answering mode alone. Both modes benefit and the
  condition would have to be re-read by every later change.

- **Where a mark fits, and where it is noise, 2026-08-25** — carried in the tool description
  and the teaching, because it is behaviour, and because in helpful answering the mechanism
  cannot narrow it: every unlimited message is a co-summoner there (`assembly.rs:1246`), so
  the aiming check admits messages the assistant merely overheard and only the teaching keeps
  it off them. It fits when a member speaks *to* the assistant and no answer is owed: a
  thank-you, a "got it, that worked", a correction the assistant accepts, a request it has
  already answered in the same thread. It is noise on a message that was not speaking to the
  assistant, on any message in a burst of several, and on a message the assistant is also
  answering in words, because the answer already acknowledges it. One mark per message, ever.
  *Rejected:* marking every addressed message as a read receipt. In a group of any size that
  is an eye on every third line, which is the visual equivalent of the chatter the silence
  default exists to prevent.
  *Rejected:* dropping the overheard clause as already enforced by the mechanism. It is not
  enforced in helpful answering, and a review that assumed otherwise read
  `own_debt_taken` as "literally addressed"; `assembly.rs:1246` folds the mode in before the
  column is written.

- **A mark and a report never sit on the same message, and the refusal points one way,
  2026-08-25.** `mark_seen` refuses an origin that already carries a report in this
  conversation, the same existence check the report uses against itself
  (`report.rs:309-318`), with its own decline text. The reverse order — mark first, report
  later in the same turn — stays taught, not enforced: at the moment the mark is
  resolved the report does not exist, so no check could see it, and suppressing an
  already-filed mark would mean the delivery edge reading other blocks to decide one block's
  fate, which nothing on that edge does today.
  *Rejected:* refusing a report whose origin already carries a mark. That would let a
  cosmetic acknowledgement suppress a moderation assessment, which is the wrong direction on
  every reading of decision 0070.
  *Rejected:* calling the overlap a leak. The report's own line is a public reply in the same
  chat (`report.rs:200-202`, delivered threaded at `outbound.rs:489-495`), so a 👀 beside it
  reveals nothing the group cannot already see. It is noise, and it is treated as noise.

- **The outbound edge becomes an item enum, named the way unit 05 names it, 2026-08-25.**
  `Assistant::replies` (`assembly.rs:1048-1051`) changes its element type from `OutboundReply`
  to `Outbound { Reply(OutboundReply), Mark(OutboundMark) }`, where
  `OutboundMark { channel: ChannelKey, mark: Mark, target_origin: String }`. The adapter's
  `consume_replies` (`driver.rs:730`) matches the two arms. The name is unit 05's
  (`05-polls.md:248-251`) because that spec proposes the identical change to the identical
  function on the same day; two names for one seam would be a merge conflict invented on
  purpose.
  `Outbound` and the existing `DeliveryItem` (`message.rs:246-258`) stay separate types and
  each one's doc comment names the other: `DeliveryItem` is what a deterministic call returns
  synchronously on its receipt, `Outbound` is what the asynchronous replies channel yields
  from a model turn. Merging them would put a synchronous return value and a channel element
  in one enum whose arms are unreachable from half its call sites.
  *Rejected:* a `ReplyKind::Mark` variant on `OutboundReply` with empty `text` and the glyph
  smuggled somewhere. That is the bolted-on conditional the project's standard says to
  refactor away from: every consumer of `OutboundReply` would have to remember that one kind
  means "do not send this text".
  *Rejected:* a second, separate receiver beside replies and composing. Ordering against the
  answer would then be undefined, and the driver's `select!` (`driver.rs:283-287`) would gain
  a fourth branch for something that belongs in the stream it is ordered against.
  *Rejected:* folding the mark arm into `DeliveryItem`. Different path, different lifetime,
  and the enum is publicly re-exported (`lib.rs:85-89`) with a documented meaning that does
  not cover this.

- **A mark is transparent to every debt reading, at all four sites, 2026-08-25.** This is the
  one part of the unit that changes what members see if it is got wrong, so it is specified
  site by site instead of being left to a trait list:
  - `impl Agency for MessageMark { fn frontier_transparent(&self) -> bool { true } }`, for
    the reason `Report` gives at `report.rs:149-157`: the block is written into a live turn's
    window by a tool, so the owed-turn decision must read through it. A mark is placed
    precisely on turns that answer nothing, so it is the ledger tail more often than a report
    is. Taking the default (`agency/mod.rs:165-167`) re-creates the burial defect that
    default is documented as having fixed.
  - `MESSAGE_MARK_KIND` joins `DEBT_READ_THROUGH` (`assembly.rs:59-63`). Without this the
    consumer's own owing-tail walk stops at a mark and a real question behind it stops owing
    an answer.
  - The exhaustive tail-debt match (`assembly.rs:1558-1571`) folds `MessageMark` into the
    `None` arm: a mark answers no debt and owes none. The compiler forces this one; it is
    named here so nobody folds it in without the entry above, which is exactly the burial.
  - `chain_step` (`provenance.rs:226-249`) names `AssistantKind::MessageMark(_) =>
    ChainStep::Extends` explicitly instead of reaching the `_` catch-all, for the reason
    `Report` is named at `:246`: the kind is written into a live turn's window by the tool, so
    its classification decides tool admission on the very turn that wrote it.

  *Rejected:* listing the traits and letting the implementer follow the report. The report's
  own transparency is an override with a comment, not a default, and one of the four sites is
  a string list nothing checks.

- **A mark's delivery does not stop the typing cue, for a reason that survives the tree,
  2026-08-25.** `consume_replies` calls `typing.stop(chat_id)` before every send
  (`driver.rs:740`) because an answer is the visible end of the composing it announced. A
  mark is not that. The earlier draft justified this by saying a mark "can be appended
  mid-turn while the model is still writing"; that is false here and the correction matters.
  The outbound edge delivers only on `StreamDone`, `StreamError` or a lag
  (`outbound.rs:186-262`), the composing edge stops on the same terminal set
  (`composing.rs:164-187`), and decision 0103 already has the cue off for the whole tool-call
  window in which a mark is appended. In the ordinary path the two edges terminate together
  and the difference is invisible.
  The reachable case is the divergence: the outbound edge lags and recovers by re-reading
  stored state (`outbound.rs:250-256`) while the composing edge answers a lag by stopping
  every open signal and letting the next turn re-begin (`composing.rs:195-204`). A mark
  recovered from an earlier turn then arrives while a later turn is composing, and stopping
  the cue there would extinguish an indicator that later turn is still earning.
  *Rejected:* stopping the cue for both arms uniformly. Simpler to read, and wrong in the one
  case where the two arms differ at all.
  *Rejected:* claiming this is "the easy thing to get wrong" and testing it end to end. There
  is no reachable production state where a mark is delivered with a cue running except the lag
  divergence, and a test that pretends otherwise pins a fixture instead of a behaviour.

- **A rate-limited mark is dropped at once, not waited on, 2026-08-25.** The placement uses a
  ceiling of its own, `MARK_WAIT_CEILING = Duration::ZERO`, so any stated `retry_after` — the
  platform always states one, and the fallback is a second (`client.rs:36-38`) — exceeds it
  and the call fails immediately with `RateLimitWaitOverCeiling` (`client.rs:517-521`),
  logged and dropped. No new code path: this is the existing per-caller ceiling mechanism, and
  the reason is the one the mechanism was built on (`client.rs:490-504`). The outbound
  consumer is sequential, so a mark honouring `MAX_RATE_LIMIT_WAIT` like a send could park
  every later answer behind a cosmetic call for up to two minutes. The tree already refused
  that for the other cosmetic call, giving the typing action its own four-second ceiling
  (`client.rs:47-54`).
  A mark also has no value late: it says "I read this", and arriving minutes after the
  conversation moved on it is noise, not an acknowledgement. Zero is therefore the
  honest ceiling and not a tuned number.
  *Rejected:* `MAX_RATE_LIMIT_WAIT`, "exactly like a send", which the earlier draft chose.
  The send holds that ceiling because a reply that arrives late is still the answer somebody
  asked for. A mark is not.
  *Rejected:* a small non-zero ceiling. Every value would be invented, and the argument for
  any of them is the argument for zero.

- **A failed placement is logged and dropped, never retried and never converted, 2026-08-25.**
  Anything that fails past the ceiling above — the group restricted `available_reactions`, the
  assistant's `can_react_to_messages` is off, the target is a service message that cannot be
  reacted to, the message was deleted, the chat is a type the method refuses — is one warning
  line and nothing else.
  *Rejected:* pre-flighting `getChat` for `available_reactions` before each mark. It is a
  second network call per mark, the answer can go stale between the read and the set, and the
  refusal it would prevent is already harmless.
  *Rejected:* falling back to a text message when the mark fails. The whole point of the mark
  is that it costs no message; a fallback would spend one at the worst possible moment.

- **The ledger records the model's decision to mark, and delivery is a separate fact,
  2026-08-25.** The earlier draft said "the ledger keeps the record that a mark was placed;
  the platform simply did not show it". That is not true in every case, and the difference
  matters: three things can leave a `MessageMark` block with no reaction anywhere.
  - The platform refused the call, per the decision above.
  - The process died between the append and the edge's wake. `seed_cursors`
    (`outbound.rs:273-295`) makes every pre-existing block history, so the mark is lost the
    way an undelivered report is lost (`outbound.rs:85-89`) — and because the per-origin
    check refuses a second call naming the same origin, that message becomes permanently
    unmarkable. The residual is one missing 👀 on one message and nothing else.
  - Erasure or the deletion mirror nulled the target origin before delivery, per the two
    decisions below.

  The block is still the truthful record: it says the model decided this message needed no
  words. That is what the record of processing describes, and it is what the history is for.
  *Rejected:* re-delivering marks from history after a restart. The report path already
  refused that direction for a stronger reason (`outbound.rs:85-89`), and for a cosmetic
  acknowledgement the safer direction is fewer, never more.
  *Rejected:* recording delivery success back onto the block. The ledger is append-only, so
  that is a second block per mark to carry a fact nothing reads.

- **The append-only record expresses a mark as a fact, and a withdrawal would cost a
  redesign this unit does not ship, 2026-08-25.** The ledger stores one `MessageMark` block
  per placement, appended, never updated. The per-origin check is an existence check, exactly
  the report's shape (`report.rs:309-318`), and it is what makes "one mark per message, ever"
  true.
  The earlier draft claimed that reading "the mark that stands on message X" as the newest
  such block was "the same reading the report's duplicate check already performs". It is not:
  that check is `any`, not newest-wins, and the two readings are mutually exclusive. Under an
  existence check a withdrawal block for an already-marked origin can never be written. So a
  withdrawal is not a small later addition; it needs the per-origin check relaxed to a
  newest-wins read, a second `Mark`-adjacent variant or a withdrawal kind, and the empty-array
  behaviour proven live first. That cost is recorded here so a later unit budgets for it
  instead of discovering it.
  *Rejected:* a mutable "current mark" row keyed by origin. It rewrites history, which the
  storage decision of 2026-08-20 forbids.
  *Rejected:* shipping the newest-wins read now, unused. It would weaken the check that makes
  the contract's "at most once ever" true, in exchange for a capability nobody asked for and
  whose platform mechanism is unproven.

- **Erasure nulls the stored target and does not chase the platform copy, 2026-08-25.** The
  mark block stores `target_origin` (nullable, for exactly this reason) and
  `marked_principal_id` (NOT NULL), and a new pass `erase_marked_origin` nulls the origin for
  a principal, composed into `erasure::execute` beside the report's pass (`erasure.rs:141-148`).
  The reaction already visible in the chat is not removed: withdrawing it would require
  issuing a network call from inside an operation that is store-only by design, would have to
  read the very column the same pass is nulling, and would depend on the unproven empty-array
  behaviour. The residual is stated instead of hidden: the visible reaction is a fact about
  *the assistant* — that it read something — attached to a message the platform and the group
  already hold as their own; it names nobody and reveals nothing the group cannot see. This
  residual belongs in the record of processing's own erasure section, not only in this file.
  *Rejected:* clearing the platform reaction during erasure, for the ordering, separation and
  proof reasons above.
  *Rejected:* storing no principal id and thereby putting the block out of erasure's reach.
  That is the exact gap decision 0003 exists to prevent, and the report tool already refuses
  a target it cannot reach (`UNRECORDED_TARGET_ERROR`, `report.rs:248-254`).

- **A mark whose origin was nulled is skipped at the edge, not sent with a hole,
  2026-08-25.** `OutboundMark.target_origin` is a plain `String` because an item on the edge
  always names its target. The stored column is nullable, so `deliverable_of`
  (`outbound.rs:471-500`) returns `Deliverable::Skipped` for a mark whose origin is NULL,
  exactly as it already does for a targetless report (`outbound.rs:465-468`, its arm at
  `:375-381`), with its own debug line naming the mark. Reachable when an erasure or the deletion mirror
  lands between the tool's append and the edge's wake, and on every later lag re-read.
  *Rejected:* an `Option<String>` on the outbound item, pushing the absent case into the
  adapter. The adapter would then decide what an item with no target means, which is a
  decision, and adapters decide nothing.

- **The deletion mirror reaches the mark's target origin, 2026-08-25.** When an administrator
  deletes a message through the moderation bot's reply command, `erase_message_named`
  (`kind.rs:743-784`) gains a third pass nulling `target_origin` on every mark naming that
  origin in that conversation, keyed on the origin the command hands in like the other two,
  and counted in `MirrorNulls` for the same log line (`assembly.rs:1220-1234`). A mark's
  stored origin is a verbatim copy of a deleted message's identifier, which is what decision
  0085's second pass exists to scrub, and a mark pointing at a message that no longer exists
  can only fail if it is still undelivered.
  *Rejected:* leaving the mark outside the mirror out of symmetry with the report, whose
  `target_origin` the mirror does not reach today. Whether the report should be reached is
  that unit's question and this spec does not answer it; symmetry with an unexamined case is
  not a reason to leave a fourth stored copy of a deleted identifier unscrubbed.

- **The protection budgets already bound marks, and nothing new is needed, 2026-08-25.**
  A message from a person over a per-principal or per-channel budget is stamped `limited`
  (`assembly.rs:1246`, `kind.rs:336`), which makes `own_debt_taken` false (`kind.rs:536-538`),
  which removes it from the co-summoner set (`provenance.rs:127`), which makes the mark
  tool refuse it with the anti-aiming decline. A flooding member cannot be marked, by the same
  mechanism that stops them being answered, and a mark writes no counter of its own so it
  neither consumes nor bypasses a budget. This is recorded because the silence rule of
  decision 0034 is about words, and a reader could reasonably wonder whether a wordless
  acknowledgement slips past it. It does not.
  *Rejected:* a budget of its own for marks. It would count something already counted, one
  layer down.

- **Marks are attempted in direct chats as well as groups, and the platform decides,
  2026-08-25.** The report tool is group-only because the moderation bot lives in groups
  (`GROUP_ONLY_ERROR`, `report.rs:244-246`). A mark has no such dependency: "read, nothing
  owed" is as useful in a one-to-one chat. The method's description names no chat-type
  restriction, but it confirms none either, so this is an inference. It is a safe one to act
  on: if a one-to-one call is refused, the failure rule above applies unchanged — one warning
  line, no retry, no text fallback, and the mark block stays the truthful record of the
  model's decision.
  *Rejected:* copying the report's group-only restriction out of symmetry. Symmetry is not a
  reason.
  *Rejected:* asserting private-chat support as a platform fact. The page does not say it,
  and this file does not claim what the page does not say.

- **A mark filed in a turn that then failed is delivered anyway, 2026-08-25.** The failure
  wake runs the same stored-state read as a completion before the notice
  (`outbound.rs:213-235`), so a 👀 and "I could not finish that answer"
  (`FAILURE_NOTICE`, `outbound.rs:114`) can reach the chat seconds apart. Both are true: the
  model read the message, and the turn later died. Suppressing the mark would mean the
  delivery function branching on which event woke it, a condition every later change to that
  function would carry, for a rare cosmetic overlap that misleads nobody.
  *Rejected:* passing the wake cause into `deliver_answers_and_reports` so marks are held on a
  failure. The report deliberately goes out on that path and the function's whole point is
  that the two wakes read stored state identically.

- **Nothing here streams, and the spec says so instead of inventing a stream, 2026-08-25.**
  A mark moves no bytes: one small JSON POST with a chat id, a message id and a one-element
  array, and a `True` back. There is no file, no media, no upload, no download and nothing to
  buffer, so the standing streaming constraint has no surface to bind to in this unit. It is
  named here so a reviewer does not have to wonder whether it was forgotten.

## The unit's contract

The assistant can place exactly one kind of reaction, 👀, on a message it is currently
reading, by calling the `mark_seen` tool with that message's projected id; the id is
validated against the turn's own co-summoner set, an origin that already carries a mark or a
report is refused, and the mark is recorded as an appended `MessageMark` block carrying the
target origin, the marked person's internal identifier and the mark's own name — a block that
is transparent to the owed-turn frontier, to the consumer's owing-tail walk and to the tool
admission chain, so a mark never buries an unanswered question. The core's vocabulary is the
closed `Mark` enum and never an emoji, checked by a scan that fails on any glyph in
`crates/core/src`; the adapter holds the single mapping from `Mark::Seen` to the pinned
U+1F440 escape and calls `setMessageReaction` with a zero rate-limit ceiling, so a
flood-controlled mark is dropped at once instead of delaying an answer. The outbound edge
carries an `Outbound` enum whose two arms are a reply and a mark; a mark's delivery does not
stop the typing cue, a mark whose origin was nulled is skipped at the edge, and a failed
placement is one warning line with no retry and no text fallback. No negative or judging mark
exists in the enum, so the assistant cannot publicly judge a member — assessment still goes to
the moderation bot's human administrators. The adapter subscribes to neither
`message_reaction` nor `message_reaction_count`, because a non-administrator bot receives
neither, and the operator contract keeps the assistant a non-administrator; that half of the
feature is documented as blocked, with its preconditions written down, and no code for it
exists. Erasure nulls a mark's target origin by the marked principal and the deletion mirror
nulls it by the deleted message's origin. The record of processing, the impact assessment and
the public privacy policy are updated in the same change. No new dependency.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings;
  the platform-vocabulary check clean (`docs/platform-vocabulary.txt`), with the four Bot API
  names added and the file's maintenance note widened to cover platform method and type
  names; secret scan clean; no new dependency.
- **AC2** No glyph in the core: a new test beside `crates/core/tests/vocabulary.rs` scans
  every `.rs` file under `crates/core/src` and fails on any non-ASCII character outside a
  committed allowlist, with the allowlist holding exactly U+2014, U+2500, U+2026 and U+2013
  at merge. The test's own failure message names the file, the line and the codepoint. A
  fixture proves it fails: a string containing U+1F440 is rejected by the same predicate the
  scan uses.
- **AC3** The glyph is pinned by its bytes, not by a literal: the adapter's constant is
  written as `"\u{1F440}"` and a test asserts, independently of that constant's spelling,
  that it is exactly one `char` whose value is `0x1F440` — no U+FE0F, no zero-width joiner,
  no surrounding whitespace. Asserting the constant equals its own escape proves nothing and
  does not satisfy this.
- **AC4** `mark_seen` places a mark: given a turn reading a member's message, a call naming
  that message's projected id appends one `MessageMark` block with the target origin, the
  member's principal id and the stored mark name, and returns the filed result text verbatim.
  The wire form is pinned against the loopback server, which records a `setMessageReaction`
  request whose body carries `chat_id`, `message_id` and `reaction` as a one-element array of
  `{"type":"emoji","emoji":"\u{1F440}"}` — the assertion spells the escape, never a pasted
  glyph.
- **AC5** The aiming checks hold, each pinned with its own decline text: an id outside the
  turn's co-summoner set is refused; a second call naming an already-marked origin is refused;
  a call naming an origin that already carries a report is refused; a call naming no id is
  refused; a call naming a message with no recorded principal is refused; a call naming the
  assistant's own message is refused. Each decline ends with the shared `NO_RETRY` close
  (`crates/core/src/tools/admission.rs:45-50`), and the filed result does **not** contain it
  but ends with its own instruction not to mark that message again — the exact split the
  report pins at `report.rs:583-592`, because a turn reading several messages may mark more
  than one, each through its own call. A transient failure names the moment and teaches no
  never-again, mirroring `transient_error` (`report.rs:258-262`, pinned at `:593-597`). These
  texts are behaviour under decision 0044 — the tool result is the model's only channel — so
  they are written as named constants in the unit, not invented at implementation time.
- **AC6** A mark is transparent at all four sites: `frontier_transparent` returns `true` for
  `MessageMark` and a test asserts it; `MESSAGE_MARK_KIND` is in `DEBT_READ_THROUGH`; a stored
  ledger with an unanswered addressed message followed by a mark still reads a tail debt
  (`assembly.rs:1558-1571` through the owing-tail walk); and `chain_step` returns
  `ChainStep::Extends` for a `MessageMark` block through an arm that names the variant, pinned
  by a test that would still pass if the catch-all were deleted.
- **AC7** The outbound edge carries both arms and the adapter routes them: a reply arm sends a
  message and stops the chat's typing refresher; a mark arm calls `setMessageReaction` and
  does not touch the refresher. Pinned as a unit test over `consume_replies` with a recording
  typing handle, and the test's comment states that in production both edges terminate on the
  same event, so this difference is observable only under an outbound lag.
- **AC8** A refused placement is contained, pinned through the loopback server and not
  through log volume: a `setMessageReaction` answering `429` with any stated wait produces
  exactly one recorded request and no sleep, a `setMessageReaction` answering a non-rate-limit
  error produces no `sendMessage` on that chat, and in both cases the next item queued on the
  edge is delivered normally. Counting log lines is not asserted here and cannot be: the
  capture subscriber is process-wide and needs its own binary
  (`crates/adapters/telegram/tests/token_scan.rs:6-12`).
- **AC9** A mark with no target is skipped, not sent: a stored `MessageMark` whose
  `target_origin` is NULL yields `Deliverable::Skipped` from `deliverable_of` and nothing
  reaches the channel, with the cursor advanced so a re-read does not meet it again — pinned
  beside the report's existing targetless test.
- **AC10** Erasure reaches the mark: after erasing a principal, every `MessageMark` block
  whose `marked_principal_id` is that principal has a NULL `target_origin`, the block header
  row is untouched, and a repeat erasure is idempotent — pinned beside the report's existing
  erasure test.
- **AC11** The deletion mirror reaches the mark: an administrator's deletion command naming a
  message nulls the `target_origin` of every mark on that origin in that conversation, the
  count appears in `MirrorNulls`, an unknown origin stays a full no-op, and a second run is
  idempotent — pinned beside the mirror's existing tests.
- **AC12** The adapter subscribes to no reaction update: `CONSUMED_UPDATE_TYPES`
  (`client.rs:103`) is asserted to contain neither `"message_reaction"` nor
  `"message_reaction_count"`, with the assertion's comment naming the administrator
  requirement — pinned, so a future change has to read the reason before removing it.
- **AC13** The two unproven platform inferences are marked as unproven in this file and
  nothing merged depends on either: no code sends an empty `reaction` array, and the
  direct-chat path degrades to the ordinary failure rule if the platform refuses it. A grep
  of the diff for an empty reaction array returns nothing. This replaces the earlier
  criterion that required a live call before merge: the adapter suite has no live-endpoint
  path (`crates/adapters/telegram/tests/adapter/server.rs:1-11`), so such a criterion left no
  artifact a reviewer could inspect and could not fail.
- **AC14** The privacy documents are true again in the same change, each named site edited:
  - a new row **D10** in `docs/privacy/records-of-processing.md` section 5 (which ends at D9
    today, `:69`) describing the mark record — the marked message's platform identifier, the
    marked person's internal identifier and the mark's name, written when the assistant marks
    a message it read;
  - a new **D10** row in that document's section 8, the erasure and time-limits table
    (`:110-115`, which today carries a `| D7, D8 |` row), stating that the marked person's
    erasure empties the stored message reference, that the deletion mirror empties it too, and
    that the reaction already visible in the chat is not withdrawn, with the reason;
  - an addendum in `docs/privacy/dpia.md` under its own review trigger — section 10 already
    names "a change to what is collected: media, edits, **reactions**, membership events"
    (`dpia.md:566`) — recording that the assistant now places a reaction, that it collects
    nobody else's, why (the administrator requirement), that no new data reaches the model
    provider, that no new recipient exists, that the mark is not a moderation effect because
    no negative mark exists, and the erasure residual named above;
  - a plain-language line in `docs/privacy/bot-assistant-privacy-policy.md` telling members
    the assistant may put a 👀 on a message to say it read it, **and** a clause in that
    document's "Retention and deletion" paragraph (`:106-113`, which today enumerates "a
    report with its message reference emptied when the reported person is deleted; lookup
    records with no time limit") covering the mark record on the same terms.

  Recipients are unchanged and nothing new travels to the processor: both are stated
  explicitly in the record of processing, not left to inference. The mark block projects
  nothing (`impl Projection for MessageMark {}`, the report's shape at `report.rs:161`) and
  the bracketed origin the tool names is already projected (`kind.rs:181`, applied at `:565`).
- **AC15** `docs/reference/group-operator-contract.md` gains a short section stating that
  reaction updates require administrator status, that the assistant deliberately stays a
  non-administrator, and therefore that the assistant sees no member's reactions — so an
  operator who wonders why never has to read this spec to find out.
- **AC16** A decision file per the repository's convention (numbering runs to `0105` today)
  records the two decisions with standing beyond this unit: the mark vocabulary lives in the
  core as a closed enum with the glyph in the adapter, and no negative mark exists (decision
  0070's structural extension).

## Notes for launch

- **Sites, exactly:**
  - Core, new: `crates/core/src/mark.rs` — the `Mark` enum with `as_str`/`parse`/`ALL` in the
    shape of `Authority` (`message.rs:87-131`), the `MessageMark` block kind (`LeafKind`;
    `Agency` **overriding `frontier_transparent` to `true`**; `Projection` projecting
    nothing), its `MESSAGE_MARK_KIND`/`MESSAGE_MARK_TABLE`/column constants and
    `erase_marked_origin`, modelled on `crates/core/src/tools/report.rs:60-184`.
  - Core, new: `crates/core/src/tools/mark.rs` — the `MarkTool`, modelled on
    `report.rs:326-470`: the filing lock, the shared erasure fence taken as the bare
    `Arc<RwLock<()>>`, the co-summoner validation, the per-origin duplicate check, the
    already-reported check, the append, and every result and decline text as a named constant.
  - Core, new test: the non-ASCII scan beside `crates/core/tests/vocabulary.rs`, with its
    committed allowlist.
  - Core, edited: `crates/core/src/kind.rs:1117-1134` (a `MessageMark` variant on
    `AssistantKind`) and `:743-784` (`erase_message_named` gains the mark pass and
    `MirrorNulls` a field); `crates/core/src/schema.rs` (a `MESSAGE_MARK_MIGRATION` appended
    last at `:376-392`, the mark vocabulary frozen through `quoted_list` the way the authority
    vocabulary is, and its pin in the vocabulary tests at `:397-432`);
    `crates/core/src/message.rs` (the `Outbound` and `OutboundMark` types beside
    `OutboundReply` at `:373`, and the cross-reference added to `DeliveryItem`'s doc at
    `:246-258`); `crates/core/src/outbound.rs` (a `Deliverable::Mark` arm at `:457-476`, its
    reading in `deliverable_of` at `:478-500` naming the kind instead of reaching `_ => None`,
    the `Skipped` reading for a nulled origin, and the send at `:333-374`);
    `crates/core/src/assembly.rs:59-63` (`DEBT_READ_THROUGH`), `:1048-1051` (the edge's
    element type), `:456-465` (unconditional registration beside the privacy tool's),
    `:1220-1234` (the mirror's log line) and `:1558-1571` (the tail-debt arm);
    `crates/core/src/tools/provenance.rs:88-93` (the compile-caught span filter) and
    `:226-249` (the named `chain_step` arm); `crates/core/src/erasure.rs:148` (the new pass,
    beside `erase_reported_origin`); `crates/core/src/teaching.rs` (the mark teaching,
    composed unconditionally the way the base sections are, beside `MODERATION_TEACHING` at
    `:33-48`); `crates/core/src/lib.rs:85-89` (the re-exports).
  - Adapter, edited: `crates/adapters/telegram/src/client.rs` — a `set_message_reaction`
    method beside `send_chat_action` (`:401`), using
    `request(..., Some(MARK_WAIT_CEILING))`; the new `MARK_WAIT_CEILING` constant beside
    `CHAT_ACTION_WAIT_CEILING` (`:47-54`) with its reason; the pinned glyph constant; **no
    change to `CONSUMED_UPDATE_TYPES` (`:103`)**, with a comment saying why.
    `crates/adapters/telegram/src/driver.rs:730-760` — `consume_replies` becomes a match over
    `Outbound`, with the mark arm skipping `typing.stop` (`:740`).
  - Docs: `docs/platform-vocabulary.txt` (four Bot API names and the widened maintenance
    note); the four privacy and reference documents named in AC14 and AC15; the decision file
    of AC16.
- **The sibling seam, and who reconciles it.** `docs/units/telegram/05-polls.md:248-257`
  proposes the same change to `Assistant::replies` under the same name, `Outbound`;
  `07-buttons-and-callbacks.md:528` and `04-deleting-messages.md:314` both add fields to
  `OutboundReply`. Whichever unit merges first defines the enum and the later ones add arms.
  This spec does not edit those files. Reconciling the element type is a merge task, not a
  redesign, and adopting unit 05's name is what keeps it that way.
- **The blocked half, written down so nobody re-derives it.** If the operator ever decides
  the assistant should read the group's reactions, all of the following must change together
  and none of them is optional: the assistant is promoted to administrator (which breaks the
  report path at `group-operator-contract.md:112` and needs its own decision);
  `CONSUMED_UPDATE_TYPES` gains both names; the adapter decodes `MessageReactionUpdated` with
  its anonymous-actor case (`actor_chat`) skipped exactly as decision 0016 skips anonymous
  posts today; `MessageReactionCountUpdated` is either ignored or handled as an aggregate
  that names nobody; a member's reaction becomes a new stored category of personal data with
  its own erasure reach; and the impact assessment is taken again under the trigger at
  `dpia.md:566`. That is a unit of its own, and it starts with a product decision this spec
  does not make.
- **What stays unproven after merge, and what it would take to prove it.** Two platform
  inferences, neither of which any merged line depends on:
  - whether an empty `reaction` array clears a placed reaction. Proving it needs a real token
    and a real chat, which the publishable-repository rule keeps out of tracked files, so it
    is an operator observation against the deployed bot and a note back into this file — not
    a merge check. A withdrawal feature must prove it first and must also pay the redesign
    cost recorded in the supersession decision above.
  - whether `setMessageReaction` works in a one-to-one chat. The same operator observation
    settles it. If it does not, nothing breaks: the call fails, one warning line is logged,
    and a later unit adds the group-only refusal to the tool.
