# Telegram unit 07 — offered choices: the inline keyboard, the press, and whose press it was

Date: 2026-08-25, revised the same day after two independent reviews. Until now every
instruction the assistant acts on arrived as a message somebody typed in the group. A pressed
button is different: it is an instruction that never went through the conversation, it can be
pressed by anyone who can see the message, and it can be pressed months later. This unit adds
the mechanism — an answer may carry a small set of options, and a member taps one instead of
typing a reply — and spends most of its design on the four consequences: who the option was
for, how the press is recorded on an append-only ledger, how the model learns of it without
anybody putting words in a member's mouth, and what the core has to learn before a press can
open a turn at all.

The first use is unit 21's clarifying question. When the assistant cannot tell whether a
question is about using the ROM or about building it, it asks — and now the two readings can
be two taps instead of a typed sentence.

## The finding that reshapes this unit

**A press is the second way a turn can be summoned, and the core currently encodes "a summons
is a chat message" in four separate places.** The first revision of this spec treated the
press as a new block kind that slots in beside the existing ones. It does not. Four readers of
the ledger are written against the message kind by type, and each one silently produces a
wrong answer — not an error — when the frontier of a turn is a press instead of a message:

1. **The turn's provenance reading.** `co_summoners` (`tools/provenance.rs:109-149`) returns
   `Vec<ChatMessage>`, requires the dispatch anchor to decode as `AssistantKind::ChatMessage`,
   and filters the absorbed span to the same kind. Its module documentation names the
   behaviour outright: the reading folds to the lowest authority for "a non-message frontier"
   (`provenance.rs:45-48`), and the set is "Empty for every unloadable shape"
   (`provenance.rs:104-108`). A turn opened by a press therefore reads as summoned by nobody,
   at `FLOOR` authority (`provenance.rs:63`) — so every tool declines, and the answer the
   press was meant to unlock is the degraded one.
2. **The first-interaction disclosure.** `Disclosure::first_answer_to_someone`
   (`disclosure.rs:145-147`) returns true on an empty summoner set, and the line is written
   into the stored block (`disclosure.rs:127-129`). Worse, it never converges:
   `introduction_in` (`disclosure.rs:205-213`) counts an answer as somebody's introduction
   only when that answer's own co-summoners name them, and a press-anchored answer names
   nobody. Every answer after every press would repeat the disclosure line for ever, which
   makes the AI Act record's per-person, first-interaction claim
   (`docs/compliance/ai-act.md` §3, decision 0079) untrue.
3. **The owing-tail walk.** `owing_tail_debt` (`assembly.rs:1535-1573`) matches exhaustively
   over `AssistantKind` and answers `None` for every kind that is not a chat message, and
   `DEBT_READ_THROUGH` (`assembly.rs:59-63`) lists the three kinds a debt is read through. A
   new kind appended mid-turn — the offer block — would bury an open debt behind it, the exact
   defect decision 0086 and the 2026-08-23 widening exist to prevent. A selection block as the
   tail would lose its own debt the same way.
4. **The budget counts.** `opened_debts_by_principal` and `opened_debts_in_conversation`
   (`kind.rs:950-999`) count rows of the chat-message content table. A selection recorded in
   its own table costs nothing, so the decision that a tap spends a slot cannot hold without
   touching the counts.

None of that makes the feature impossible, and none of it is fixed by adding conditionals at
the four sites — that is precisely the bolted-on shape this project refactors away from. What
it means is that **this unit is two pieces of work, and the first one touches no platform at
all**:

- **07a, the summoning seam.** A neutral reading of "what summoned this turn" that both the
  chat message and a second kind can answer, with the four readers moved onto it and every
  existing test still passing. Core only, no adapter, no new behaviour, no platform word.
- **07b, offered choices.** Everything below, which then slots into the seam without a single
  kind-specific branch outside the kind's own module.

They may run as two units or as two commits in one worktree, but 07b cannot merge before 07a.
The rest of this document specifies both: the seam in its own section, the feature in the
usual sections, and the acceptance criteria numbered across both.

## Grounding

### The platform, read 2026-08-25

The brief for this unit said Bot API 10.1 (June 2026) is current. That is two releases stale
and both of the newer ones touch this feature. The changelog's most recent entry is **Bot API
10.3, dated 24 August 2026**; 10.2 is dated 14 July 2026. Everything below is from
`core.telegram.org/bots/api` and its changelog as they read on 25 August 2026, and every
sentence in quotation marks was read from that page on that date.

- **The payload is 1–64 BYTES.** `InlineKeyboardButton.callback_data`: "Data to be sent in a
  callback query to the bot when the button is pressed, 1-64 bytes". Bytes, not characters —
  a label-sized payload does not fit, so the payload cannot carry the meaning of the option.
  This single constraint decides the shape of the whole unit.
- `InlineKeyboardMarkup.inline_keyboard` is an array of button rows. The documentation states
  **no cap** on the number of rows or buttons, and **no length limit** on
  `InlineKeyboardButton.text`. `InlineKeyboardButton` requires that "Exactly one of the
  fields other than `text`, `icon_custom_emoji_id`, and `style` must be used".
- New in 10.3: `InlineKeyboardButton.style` ("danger", "success", "primary"), and
  `InlineKeyboardButton.disabled` of the new type `DisabledButton` — "If set, then the button
  is disabled and does nothing". Also new in 10.3: `force_reply` on `InlineKeyboardMarkup`.
- **The update type is `callback_query`**, carrying a `CallbackQuery`. The `Update` table
  describes it as "New incoming callback query" and attaches no condition. Contrast the
  neighbouring entries: `chat_member`, `message_reaction` and `message_reaction_count` each
  say the bot "must be an administrator" and "must explicitly specify" the type in
  `allowed_updates`. `callback_query` requires neither, and `getUpdates` confirms it is in
  the default set — "Specify an empty list to receive all update types except `chat_member`,
  `message_reaction`, and `message_reaction_count` (default)".
- `CallbackQuery` carries: `id`, `from` (a `User`, described only as "Sender"), `message`,
  `inline_message_id`, `chat_instance`, `data`, `game_short_name`. **`message` is documented
  Optional** — "Optional. Message sent by the bot with the callback button that originated the
  query" — and the object's own preamble states the condition: "If the button that originated
  the query was attached to a message sent by the bot, the field message will be present. If
  the button was attached to a message sent via the bot (in inline mode), the field
  `inline_message_id` will be present." The same preamble states "Exactly one of the fields
  `data` or `game_short_name` will be present", so `data` is Optional too. **The `from` field
  is whoever pressed it — the platform offers no way to restrict who may press a button.** The
  API says of `data`: "Be aware that the message originated the query can contain no callback
  buttons with this data", so the payload is untrusted input even though it originated with
  the bot.
- **A press must be answered or the presser watches a progress bar.** The note under
  `CallbackQuery`: "After the user presses a callback button, Telegram clients will display a
  progress bar until you call `answerCallbackQuery`. It is, therefore, necessary to react by
  calling `answerCallbackQuery` even if no notification to the user is needed (e.g., without
  specifying any of the optional parameters)."
- `answerCallbackQuery` takes `callback_query_id` (required), `text` — "Text of the
  notification. If not specified, nothing will be shown to the user, **0-200 characters**" —
  `show_alert` ("an alert will be shown by the client instead of a notification at the top of
  the chat screen"), `url`, and `cache_time`. It returns True on success. **The documentation
  does not say a query may be answered only once.** The whole method section was read for such
  a sentence and there is none; the first revision of this spec asserted the once-per-query
  rule as documentation and it is not. It is widely relied on and almost certainly true, and
  the design sends exactly one answer per press regardless, so nothing here rests on it —
  recorded as unproven, not as fact.
- **The documentation states no expiry for a callback query id.** There is no published
  number and no published error text. What the server returns in practice is a refusal
  reading "query is too old and response timeout expired or query ID is invalid", widely
  reported at around ten seconds; that figure is not official and this design does not depend
  on it. The rule the design follows instead: answer as the first thing after a decision that
  needs no network call.
- **The one documented time limit near this feature is 15 seconds**, and it belongs to a
  different mechanism. Under "Ephemeral Messages and Commands" → "Reply Targets and
  Conditions": "Any bot can send an ephemeral message to a user within **15 seconds** of the
  incoming eligible action... For this the bot must provide either: The `callback_query_id`
  from a received callback query, or the `reply_parameters.ephemeral_message_id` from an
  incoming ephemeral message." Ephemeral messages (10.2, extended in 10.3 with
  `EphemeralMessageParameters` and its `replace_callback_query_message` field) are group
  messages "visible only to a specific user and the bot". The same section warns: "It is
  **not guaranteed** that the ephemeral message will be received, especially if the user is
  offline."
- `MaybeInaccessibleMessage` is either a `Message` or an `InaccessibleMessage`, the latter
  being "a message that was deleted or is otherwise inaccessible to the bot", whose `date` is
  "Always 0. The field can be used to differentiate regular and inaccessible messages." So
  the message a press names may be present and still not editable.
- `editMessageReplyMarkup` takes `business_connection_id`, `chat_id` + `message_id` (or
  `inline_message_id`) and an Optional `reply_markup` described only as "A JSON-serialized
  object for an inline keyboard". **The documentation does not state what omitting
  `reply_markup` does.** The first revision claimed omission removes the keyboard; that is an
  inference, not documentation, and it is the same class of inference sibling unit 06 marks
  as unproven pending a live call. This unit does the same: the design calls the method with
  `reply_markup` set to an inline keyboard whose rows array is empty — a documented shape, an
  `InlineKeyboardMarkup` with no buttons — and an acceptance criterion confirms against the
  live API which of the two forms actually clears the keyboard before merge. The method's
  48-hour edit ceiling applies only to business messages not sent by the bot, so it does not
  reach this path.

### Our tree

- **The consumed update types are named explicitly on every poll** and must be extended:
  `CONSUMED_UPDATE_TYPES` is `["message", "edited_message", "my_chat_member"]`
  (`client.rs:103`), passed as `allowed_updates` in `get_updates` (`client.rs:317-319`)
  because "an absent selection would inherit whatever an earlier setting left on the token".
  The decoded `Update` (`client.rs:109-123`) has no `callback_query` field; unknown fields are
  ignored, so today a press is decoded as an empty update and silently skipped.
- **The adapter throws away the identifier of the message it sends**: `send_body` ends with
  `let _sent: serde_json::Value` (`client.rs:459`). Nothing in our tree knows the platform
  identifier of the assistant's own messages.
- **Platform caps live in the adapter**: `MESSAGE_UTF16_UNIT_LIMIT = 4096` (`client.rs:34`)
  and `chunks_within_cap` (`client.rs:599`), with decision 0019 behind them. Chunked sends
  thread only the first chunk (`send_message`, `client.rs:365-390`), and a failed chunk aborts
  the remaining ones.
- **The adapter's one existing degrade path is narrow on purpose**: `rejects_formatting`
  (`client.rs:478-486`) matches five parse-complaint phrases and nothing else, because "every
  other refusal — a blocked bot, a chat that is gone, a member who left — is about the send
  itself, and retrying it unformatted would only fail again, more slowly". That narrowness is
  why this unit does not add a markup degrade path; see the decisions.
- **An inaccessible payload is already handled once in this adapter**: `PinnedContent.date`
  documents "zero is the inaccessible form" and decodes leniently so a malformed payload
  degrades to a skip instead of refusing the batch (`client.rs:152-165`). The
  `MaybeInaccessibleMessage` on a press takes the same shape, and its optionality takes the
  same lenient decode.
- **The outbound reply already carries a target and a typed kind**: `OutboundReply` has
  `channel`, `text`, `kind`, `reply_target` (`message.rs:373`); `ReplyKind` is
  `Answer | Notice | Report` (`message.rs:331`). The typed-kind rationale is written on
  `DeliveryItem` (`message.rs:252`): "typed by what it is, so an adapter can present the
  kinds differently without reading the text. The core still supplies the exact wording for
  both, because wording is behavior and behavior stays out of adapters."
- **The delivery edge reads blocks one at a time through a forward cursor**:
  `deliverable_of(block: &Block)` (`outbound.rs:478-497`) decides per block and sees no other
  block, and the loop (`outbound.rs:322-385`) walks ids past a per-conversation cursor. It
  already swallows an empty answer (`outbound.rs:341-350`) and skips a targetless report.
  Attaching options to "that turn's answer" therefore needs a stated join key; the loop cannot
  look ahead.
- **A consumer block kind carrying a deliverable line and an erasable reference already
  exists**: the report kind (`report.rs:61-130`) — a content table, a nullable
  `target_origin` that "the reported person's erasure nulls", a `reported_principal_id` stored
  "precisely so erasure can reach this block". New kinds compose into `AssistantKind`
  (`kind.rs:1118-1133`).
- **The report tool is the exact precedent for a tool that writes**, and every part of its
  shape is needed here. It is not a lookup and is not in `production_lookups`
  (`tools/mod.rs:87-89`: "The report tool is not a lookup and joins at the assembly, where its
  window and the erasure fence live"); it is constructed at `assembly.rs:447`. Its
  scan-then-append pair "runs under the tool's own filing lock" because "the runner executes
  same-round calls in parallel tasks" (`report.rs:33-37`), its append holds the erasure fence
  so it "cannot re-materialize an origin an erasure just nulled" (`report.rs:36-38`), it is
  `frontier_transparent` (`report.rs:153-157`) and listed in `DEBT_READ_THROUGH`
  (`assembly.rs:59-63`), it validates its target against `co_summoners`
  (`report.rs` imports it at the top), and its erasure reach is a crate-private pass of its
  own, `erase_reported_origin` (`report.rs:171+`), composed by the erasure operation at
  `erasure.rs:141-148`.
- **The message kind is the model for a person-authored block**: `ChatMessage`
  (`kind.rs:376+`), `stored_fields` (`kind.rs:444`), `owes_answer` (`kind.rs:495`),
  `Agency::awaiting` returning `Awaiting::Model` only for a message that owes an answer
  (`kind.rs:640-648`), and `projected_text` prefixing the speaker for role user
  (`kind.rs:555-568`). `ERASED_MARKER` (`kind.rs:166`) is what an erased row projects.
- **Erasure nulls five columns, one pass per owning kind**: `erase_principal_content`
  (`kind.rs:688`) nulls text, origin, platform send time, reply target and speaker keyed on
  the principal; the three-step operation is documented at the top of `erasure.rs`, and its
  step 1 already lists the report pass beside it.
- **The stamp is composed from four inputs**: `Stamp::compose(summons, sender, limited,
  owing_tail)` (`kind.rs:328-345`), where `Summons` carries `summoned` and
  `literal_addressed`, resolved from the inbound message by `resolved_summons`
  (`assembly.rs:1244-1249`) as "addressed, or the answering mode is helpful".
- **The stamp lock is a process-wide mutex on the assistant**: `stamp_lock: Mutex<()>`
  (`assembly.rs:358`), taken at `assembly.rs:660` and `assembly.rs:974`, and it "covers
  exactly the read-then-append window" (`assembly.rs:1004`). This is the only serialization
  the write path has, and it is what a conditional append can rest on.
- **A suppressed sender's message is dropped at ingestion**: `resolve_writing_sender`
  (`assembly.rs:1138-1153`) returns `None` for a standing-flagged sender unless the message is
  a privacy command, and `suppressed_under_lock` (`assembly.rs:1261-1273`) re-reads the flag
  under the stamp lock so "from the moment it stands" holds against the race.
- **A read-only principal lookup exists that never creates a row**: `find_standing`
  (`identity.rs:101-125`), beside the creating `resolve_principal` (`identity.rs:59`). This is
  what lets a press by a stranger be refused without minting an identity for them. It returns
  the principal id and the suppression flag and nothing else — no authority.
- **The write path is one guarded sequence**: `Assistant::ingest` (`assembly.rs:623`) takes
  the erasure fence, admits the channel, resolves the sender, holds the stamp lock across the
  tail read and the budget counts, reconciles the palette, composes a `Stamp`, and appends
  one consumer block.
- **Tools are core behaviour with a declared authority bar**, admitted per conversation:
  `NAME` and `REQUIRED_AUTHORITY` (`wiki.rs:45,69`), `ToolSet::admit` (`tools/mod.rs:116`),
  the admission wrapper (`tools/admission.rs`), and the turn's provenance reading
  (`tools/provenance.rs`, decision 0043). A tool failure speaks to the model (decision 0044).
- **The co-summoner set is ordered newest first**, pinned by name:
  `the_co_summoner_set_holds_the_takers_newest_first_and_no_bystander`
  (`provenance.rs:453`) — the absorbed span's own-debt-takers newest first, then the anchor
  chain's, and never a bystander.
- **The framework's consumer seam hands a closure a raw connection**: `Store::append_consumer_block`
  (`store/descriptors.rs:1054`) is unconditional, and `domain_run` (`store/mod.rs:704`) runs
  arbitrary SQL inside the consumer's own domain. The framework's own conditional write,
  `insert_approval_decision_block` (`store/approvals.rs:82-102`) — "Append-if-undecided: the
  decision INSERT is conditional on no decision block already referencing the same request" —
  is a framework method over framework-owned kinds and is not reachable from a consumer kind,
  and `insert_approval_request_block` refuses anything that is not a tool call
  (`store/approvals.rs:28-32`). The shape is the model; the mechanism has to be ours.
- **Stored-time expiry is already tested in this tree, and not with an injected clock.** The
  core has no wall-clock seam: the assistant's test seams are the two read pauses
  (`assembly.rs:337-344`), the budget window is evaluated as SQL `datetime('now', ?2)`
  (`kind.rs:931`), and tokio's `test-util` moves in-process `Instant`s only
  (`crates/core/Cargo.toml:38-40`, `window.rs:83,123`). What the window-expiry pins actually
  use is `age_receipts` in the suite's own support module
  (`crates/core/tests/spine/support.rs:1151-1170`), which rewrites `blocks.created_at`
  backwards in the test store, plus a second helper that re-encodes the offset. Every budget
  window test drives expiry that way (`spine/protection.rs:366`, `:977`, `:1013`, `:1218`).
- **The budget counts read the header's creation time, not a copy**: the counts join `blocks`
  for "the receipt time (the header's creation time, assigned by the store at the write,
  unforgeable and never null)", and the comment states plainly that "duplicating either there
  would be a second record of a framework-owned fact" (`kind.rs:882-890`).
- **The core-vocabulary check bans platform and SDK NAMES only**: `platform-vocabulary.txt`
  holds seven entries — the two platforms and five client crates — and its header says "this
  file is the one adapters grow". `crates/core/tests/vocabulary.rs` scans every core source
  file for them as whole words. It would not notice "callback", "keyboard", "inline" or
  "markup", so passing it is not evidence for this unit's neutrality; those four words are
  added to the file by this unit, which is what makes the check mean something here.
- **Nothing about buttons exists in the code yet.** A search of `crates/` for keyboard,
  callback query, inline button and reply markup returns only unrelated hits in an HTML test
  fixture. It is no longer true of `docs/`: units 02, 03, 04 and 05 were written in parallel
  with this one and mention reply markup; none of them implements it.
- **The framework's attachment store with sparse byte ranges** (`store/attachments.rs:45-300`)
  and **the fork primitive** (`store/conversations.rs:317`) are both irrelevant here and are
  not touched: this unit moves no bytes and splits no conversation.

## 07a: the summoning seam

The seam is one idea: **a turn is opened by a summoning act, and a chat message is one kind of
summoning act.** Everything the four readers need from a summons is four facts — the acting
principal, the authority the act carries, whether the act took a debt of its own, and whether
the act's personal content has been erased. None of those is specific to a typed message.

- A `Summoning` reading is declared beside the message kind: a small owned record carrying
  `principal_id`, `authority`, and, where the reader needs it, the originating reference a
  message has and a press does not. Kinds that can open a turn produce one; every other kind
  produces none. The chat message's production is exactly its present fields, so its behaviour
  is unchanged by construction.
- `co_summoners` returns `Vec<Summoning>` instead of `Vec<ChatMessage>`, keeping its present
  order and its present folds. Its four readers change only in what they name: the authority
  fold reads `authority`, the disclosure fold reads `principal_id`, the privacy tool's
  principal resolution reads `principal_id`, and the report tool's target validation reads the
  originating reference — which a press never has, so a press can never be the aim of a
  report, correct by construction and not by a special case.
- The chain walk's frontier condition widens from "the anchor decodes as a chat message" to
  "the anchor decodes as a summoning act". `chain_step`'s read-through set is unchanged.
- `owing_tail_debt` reads the tail's summoning production instead of matching the message kind
  by name, so a tail that owes an answer hands over its debt whatever kind it is. The
  exhaustive match over `AssistantKind` stays exhaustive — a new kind that summons nothing
  still answers `None` — but it stops being the place a new kind has to be remembered.
- The two budget counts take the counted-debt fragment they already share and apply it to
  every table that records a summoning act, summing the counts. The fragment stays one
  definition, which is the reason it exists (`kind.rs:921-926`).

That is the whole of 07a. It adds no behaviour, no platform word, and no new kind: after it,
the only kind producing a summoning reading is still the chat message, and every existing test
passes unchanged. It is specified as its own piece because a refactor with no behaviour change
can be reviewed for exactly that, and because 07b is then a kind that implements a declared
reading instead of a kind that four readers must each be taught about.

*Rejected:* teaching the four readers about the selection kind directly — four conditionals
against one type, in four modules that have no reason to know it, and a fifth reader added
later would need a fifth. *Rejected:* recording the selection as a chat message with invented
text, which would make all four readers work with no change and make the ledger's record of
what a person said untrue; the reason it is rejected is stated again under the decisions.
*Rejected:* leaving the readers alone and accepting the degraded behaviour — it is not
degradation, it is four wrong answers, one of which makes a published compliance statement
false.

## Decisions taken with this unit

- **The payload is an opaque token; the meaning lives on the ledger, 2026-08-25.** Sixty-four
  bytes cannot hold an option's meaning, and a payload that carried an instruction would be an
  instruction the assistant received from outside the conversation with no record of having
  offered it. So the core mints one token per option — the offer block's row id, a separator
  and the option index, ASCII, bounded by construction at well under sixty-four bytes — and
  the token means nothing except as a key into the offer the core itself appended. On a press
  the token is parsed, never interpreted: it must resolve to an offer block, or the press is
  refused. *Rejected:* encoding the option's label or an action name in the payload — it does
  not fit, and where it did fit it would make the button the authority for what happens, which
  is exactly the hazard. *Rejected:* a random opaque token in a side table — a second lookup
  structure for a key the block row id already provides.
- **An offer and a selection are two consumer block kinds, in the shape of the framework's
  approval pair, 2026-08-25.** `choice_offer` records what was offered: the addressee's
  principal and the option labels as offered. `choice_selection` records one press: the offer
  it answers, the option index, the label as it was offered, the acting principal and the
  speaker's handle. Neither is ever rewritten. Neither stores a time of its own: the block
  header's creation time is the framework's own unforgeable receipt, and the budget counts
  already state that copying it into a content table would be a second record of a
  framework-owned fact (`kind.rs:882-890`). The offer's closure is not a field that flips — it
  is derived, exactly as the framework derives a call's resolution: an offer is closed when a
  selection block referencing it exists, and expired when its header's creation time is older
  than the window. That is how an append-only record expresses a fact that changes.
  *Rejected:* reusing `ApprovalRequest`/`ApprovalDecision` — the framework's request write
  refuses anything that is not a tool call, and its decision projects invisibly on purpose,
  while ours must reach the model. *Rejected:* a mutable `decided` column on the offer — a
  rewrite, forbidden. *Rejected:* an offer time column of its own, which would be a second
  copy of the header's time and would need its own erasure treatment for no gain.
- **The selection is its own kind and never speaks in the member's voice, 2026-08-25.** The
  message kind records what a person typed, verbatim, and a press is not typing. So a
  selection is not stored as a message with invented text. It projects to the model through
  the same speaker prefix the message kind uses, with a body that reads as an act, not a
  quote — `ada: (chose: use it on my device)` — so the model can never mistake the
  assistant's own label for the member's words. An erased selection projects the shared erased
  marker, like an erased message. *Rejected:* synthesising a message ("I want to use it on my
  device") — putting words in a member's mouth, and it would make the ledger's record of what
  was said untrue. *Rejected:* feeding the press to the model only as a tool result — the
  press is a person's act in the conversation and belongs in the conversation's own record.
- **The offer is a tool the model calls; the offer block projects invisibly, 2026-08-25.** The
  model offers options by calling `offer_choices` with two to four labels, and then writes its
  answer as usual. The model already sees its own tool call and result in context, so the
  offer block itself adds nothing and stays invisible to the projection — the same reasoning
  the framework's approval decision states for its own invisibility, applied for the opposite
  cause: there the model must not learn, here it already knows. The tool declares
  `REQUIRED_AUTHORITY = Member` and is admitted through the existing palette, so a conversation
  that never admitted it cannot be handed buttons. Like the report tool it is not a lookup: it
  is constructed at the assembly, where the erasure fence and its own filing lock live, not in
  `production_lookups`. *Rejected:* a deterministic core path that attaches options behind the
  model's back — the model would then answer a selection it has no record of having offered.
  *Rejected:* letting the model emit a marker inside its answer text — a magic string in prose,
  the pattern unit 22 removed. *Rejected:* registering it beside the three lookups, which
  `tools/mod.rs:87-89` already explains is the wrong place for a tool that writes.
- **The offer's addressee is the newest own-debt-taker in the turn's co-summoner set, and an
  empty set refuses the tool, 2026-08-25.** A group button is visible to everyone, so the core
  binds the offer to a single principal at the moment it is appended. The co-summoner set is
  plural — decision 0010 absorbs mid-turn messages — and it is ordered newest first, pinned by
  its own test (`provenance.rs:453`). The addressee is its first entry: the most recent person
  whose own ask this turn is answering. When the set is empty the tool declines and says so to
  the model (decision 0044), which is the same downward fold every other reader of the set
  performs. *Rejected:* accepting any member's press — the loudest tapper would then steer an
  answer to someone else's question. *Rejected:* offering to every co-summoner — two people
  tapping different options on one question would leave the single-use rule deciding the
  conversation by whoever's update arrived first. *Rejected:* the dispatch anchor's own sender,
  which `provenance.rs:22-26` warns may be a bystander whose line merely became the frontier.
  *Rejected:* letting the model name the addressee the way the report tool names its target —
  the model knows handles, not principals, and a mis-named addressee would be a button offered
  to the wrong person with no way for anyone to notice.
- **A press by anyone but the addressee is refused without touching the identity tables,
  2026-08-25.** The core resolves the presser through the read-only lookup, never the creating
  one, so somebody who has only ever tapped a button never becomes a stored person. A refused
  press appends no block and writes no row; the presser is told, briefly, that the choice was
  not theirs, and that is the end of it. *Rejected:* recording every press for an audit trail —
  it would create personal data about people who never spoke to the assistant, to protect
  against a tap that has no effect.
- **A press by a person whose objection stands is refused and recorded nowhere, 2026-08-25.**
  The privacy documents state that from the moment an objection stands the person's new
  messages are dropped at ingestion, and decisions 0071 and 0072 exempt only the privacy
  command family from that. A press is not a message and is not a privacy command, so it takes
  the plain form of the rule: the select path re-reads the standing flag under the stamp lock,
  exactly as `suppressed_under_lock` does for a message, and a flagged presser's press appends
  nothing. Because such a person may still be the addressee of an offer made before they
  objected, the refusal is the same brief notice as any other refusal — the assistant does not
  tell a room why. *Rejected:* recording the selection and suppressing it later, which would
  make the record of processing false the day it merged; *rejected:* treating a press as
  exempt like a privacy command, which would let a stopped collection restart with a tap.
- **One `answerCallbackQuery`, sent after the core's verdict, and the verdict costs no network
  call, 2026-08-25.** The platform shows a progress bar until an answer arrives and the id's
  expiry is undocumented. The design makes the verdict fast instead of racing it: a selection
  is decided entirely from the local store, so no administrator fetch, no chat lookup and no
  rate-limited call sits between the press and the answer. The adapter then answers once —
  with no text when the selection was taken, so nothing is shown, and with the core's brief
  notice and `show_alert` when it was refused. *Rejected:* answering immediately with nothing
  and delivering the notice as an ephemeral message keyed by `callback_query_id` — two calls, a
  fifteen-second budget, and delivery the platform explicitly does not guarantee, to save a
  local database read. It is the right shape the day a verdict genuinely needs the network, and
  the reasoning is recorded here so that day does not have to rediscover it.
- **The press names no channel; the offer does, 2026-08-25.** `CallbackQuery.message` is
  documented Optional and the assistant never needs it to decide. The token resolves to the
  offer block, the offer block's conversation is the conversation, and the mapping module says
  which channel that conversation is. So the neutral inbound selection carries the token, the
  presser's identity and nothing else, and the verdict is reachable even when the platform
  sends no message at all. Two consequences follow and both are wanted: the press does not run
  channel admission, so a stale button in a chat the assistant has left cannot make it fetch an
  administrator list or issue a withdrawal — a press whose conversation is no longer mapped for
  this adapter is simply refused; and an absent or inaccessible `message` costs only the
  keyboard removal, which is skipped. *Rejected:* routing the press through the channel
  admission the message path uses, which would put a network call before the verdict and could
  make a year-old tap trigger a departure from a chat.
- **A press carries no fresh standing; it inherits the standing recorded on the offer's own
  turn, 2026-08-25.** Resolving authority means fetching the chat's administrator list, which
  is a network call with a one-minute wait ceiling — precisely what the previous decisions
  forbid. So the authority is recorded once, at the moment the offer is appended: the offer
  block stores the authority of the co-summoner it is addressed to, taken from the same
  provenance reading that chose the addressee. On a press the selection copies it. This is
  honest as well as fast: standing is a property of the person at the moment they asked, and a
  press is not a new claim to it. *Rejected:* re-resolving standing per press; *rejected:*
  reading "the authority recorded on the original ask" at press time, which the first revision
  specified and which nothing can do — a press knows the offer, and the offer knew no message;
  *rejected:* storing no authority at all, which would leave the stamp with nothing to compose
  from; *rejected:* the presser's most recent stored authority, which is a different fact that
  happens to be nearby.
- **A press opens a turn and spends a budget slot, 2026-08-25.** A selection is a summoning
  act in the sense 07a defines: it sets the same answer-owed stamp a summoning message sets, so
  the model answers the now-disambiguated question in its own turn, and it passes through the
  same budget check — a tap costs a model turn, so it costs what a model turn costs. Its
  summons is not resolved from text: a press is addressed by construction, so `summoned` is
  true and the literal-addressed fact is false, which is the honest reading — nobody typed the
  assistant's name. A tail debt carried into a press propagates through it exactly as it
  propagates through a message. *Rejected:* treating a press as free because it is cheap to
  send — free taps are exactly how a bounded budget stops bounding anything. *Rejected:*
  resolving the summons from the answering mode the way a message does, which would make a tap
  in a silent-by-default conversation open no turn and leave the presser with a button that
  visibly does nothing.
- **A selection made by a silenced presser is recorded and answers nothing, and that closes
  the offer, 2026-08-25.** The budget path treats it exactly as a silenced message: the block
  is recorded with the refusing budget named, no answer is owed, and the chat stays quiet.
  Because the offer is closed by the existence of a selection, the presser cannot retry once
  their budget frees — the button is spent. That is stated plainly and not smoothed over: it is the
  same outcome as a silenced typed question, which is also not retried for the asker, and the
  alternative would be a button that quietly does nothing and can be pressed for ever.
  *Rejected:* leaving a silenced selection unrecorded so the offer stays open, which would make
  the tap a free probe of the budget's state.
- **One offer, one selection; a repeat of the same press is idempotent, and the mechanism is
  the stamp lock, 2026-08-25.** The framework's approval write is conditional inside one
  transaction, but that is a framework method over framework-owned kinds and a consumer kind
  cannot reach it: consumers append through the unconditional consumer seam. The precedent for
  a consumer facing exactly this is the report tool, whose scan-then-append pair "runs under
  the tool's own filing lock" (`report.rs:33-37`). The select path takes the same shape and
  needs no lock of its own: it already holds the assistant's stamp lock across the read and the
  append, the same process-wide mutex that serialises every ingestion, and it is the only
  writer of selection blocks. So the scan for an existing selection and the append of a new one
  are serialised against every other write in the process. A second press by the same person
  naming the same option appends nothing and answers taken, so a redelivered update after a
  lost answer is harmless. A second press naming a different option, and any press by anyone
  else, is refused. The guarantee is in-process, which is exactly as strong as the guarantee
  every budget count already has, and the assistant is one process by design. *Rejected:*
  letting a member change their mind by pressing again — it would mean two contradicting
  selections against one offer, and the model would have to work out which one counts.
  *Rejected:* claiming transaction semantics the consumer seam does not offer, which the first
  revision did.
- **An offer expires twenty-four hours after its block's creation time, as a fixed property,
  2026-08-25.** A button sits in the group's history forever, and a tap on a year-old message
  would open a model turn about a conversation nobody remembers. The window is a constant in
  the core with its reasoning beside it, and the age it compares against is the block header's
  creation time — the same unforgeable receipt the budget windows use. *Rejected:* no expiry at
  all — the message history is a permanent instruction surface. *Rejected:* a configuration
  entry — decision 0024 keeps configuration to one file, and every entry added there is a
  support surface nobody will ever tune. *Rejected:* an injected clock, which the first
  revision's acceptance criterion demanded and which does not exist anywhere in this core;
  expiry is proven the way every budget window in this suite is proven, by ageing the stored
  receipt times through the suite's own helper.
- **The keyboard is cleared with an empty keyboard, not by omission, 2026-08-25.** On a taken
  selection the adapter clears the markup on the message the press named, so the answer stays
  in the history without live controls; on a refusal the markup stays, since the offer may
  still be takeable by the person it was for. The documentation does not say what omitting
  `reply_markup` does, so the design sends the documented shape instead — an inline keyboard
  whose rows array is empty — and an acceptance criterion confirms against the live API before
  merge which form actually clears it, exactly as sibling unit 06 does for its own unproven
  inference. If the message is absent from the query or decodes as the inaccessible form —
  date zero, the shape this adapter already recognises for a pinned message — the clearing is
  skipped and nothing else changes. *Rejected:* omitting `reply_markup` on an inference;
  *rejected:* recording the assistant's own message identifier so an expired offer could be
  cleared later — it would add a delivery-to-core feedback path and a new stored platform
  identifier, to tidy up buttons whose press is refused anyway. *Rejected:* rendering the
  closed options with 10.3's `DisabledButton`, which keeps the labels visible and would read
  better: it shipped on 24 August 2026, one day before this spec, and client rollout is
  unknown. Revisit it once it is in released clients.
- **Every input is bounded in the core, so the adapter has nothing to degrade from,
  2026-08-25.** The core caps what an offer may contain: two to four options, each label
  non-empty after trimming, at most sixty-four characters, and no two labels equal after
  trimming and case folding — bounds chosen for a person reading a row of buttons on a phone,
  not from any platform number, and each one refuses the tool call with a sentence the model
  can act on. The token is bounded by construction and the adapter asserts its byte length
  before every send. With the input bounded there is no known option set the platform refuses,
  because the documentation states no cap on rows, buttons or label text — so this unit adds no
  markup-specific degrade path. That is deliberate: the adapter's one existing degrade path is
  narrow because "every other refusal ... is about the send itself, and retrying it unformatted
  would only fail again, more slowly" (`client.rs:478-486`), and a markup refusal has no
  documented error text to match on, so any classifier written for it would be invented. A send
  that fails while carrying markup fails like any other send, through the path that already
  exists. *Rejected:* retrying every failed send without markup, which would retry blocked
  bots and departed chats one bounded wait at a time; *rejected:* an invented list of markup
  refusal phrases, which is unsourced and would silently stop matching.
- **The core carries the option vocabulary; the adapter carries the platform's caps, and the
  adapter's own suite proves the caps hold, 2026-08-25.** The neutral vocabulary is
  `ChoiceOption { token, label }`, an optional set of them on `OutboundReply`, an
  `InboundSelection` on the way in, and a `SelectOutcome` on the way back — no platform word
  anywhere, and a future Matrix adapter renders the same options however that platform can, or
  as a numbered list a member answers by typing. The sixty-four-byte payload cap, the
  two-hundred-character notice cap and the markup itself live in the adapter, where the
  four-thousand-and-ninety-six character message cap already lives. A notice longer than the
  platform's notice cap is not truncated — the adapter answers with no text and logs an error,
  because half a sentence is worse than silence. The proof that no notice ever reaches that
  path is an ADAPTER test asserting each of the core's notice constants against the adapter's
  own cap: the dependency runs adapter to core, so the adapter can read both numbers, and the
  core learns neither. *Rejected:* letting the core know the platform's numbers; *rejected:* a
  core test asserting the notice length, which the first revision specified and which would put
  the platform's two hundred inside the core — the exact leak the same decision rejects, and
  one the vocabulary check would not catch; *rejected:* truncating a notice at the boundary,
  which would let the adapter edit the core's words.
- **The options attach to the first answer of the turn that offered them, joined by the
  dispatch anchor, 2026-08-25.** The delivery loop reads one block at a time through a forward
  cursor and cannot look ahead, so "that turn's answer" needs a key it can evaluate on the
  block in front of it. Every block a turn writes carries the id of the summoning frontier, so
  the offer block and the turn's answers share a dispatch anchor: the loop attaches an offer's
  options to the first assistant answer block it reaches whose anchor matches and whose id is
  greater than the offer's. With decision 0103 a tool call ends a round, so the model's closing
  text is the next block after the offer and that first answer is the intended one. A second
  `offer_choices` call in the same turn is declined by the tool, under the same lock that
  makes the selection append safe — the runner executes same-round calls in parallel tasks, so
  without the decline two offers could reach one answer and only one set of buttons could be
  rendered. *Rejected:* attaching to the last answer of the turn, which the loop cannot
  identify without buffering the turn — and buffering an answer is what this project's
  streaming constraint forbids; *rejected:* letting the offer block itself be deliverable,
  which would send the buttons as a separate message under the answer and make the answer's
  own reply threading meaningless.
- **An offer whose buttons never reach anyone simply expires, 2026-08-25.** Three ordinary
  outcomes leave an offer on the ledger that nobody can press: the turn abstains and the
  delivery loop swallows the empty answer, the turn ends on the miss sentinel, or a chunked
  send fails before the chunk carrying the markup. In all three the offer sits for twenty-four
  hours addressed to a person and is then expired by age like any other. Nothing cleans it up
  and nothing needs to: no button exists, so no press can arrive, and the record of processing
  says plainly that an offer may be recorded that was never shown. *Rejected:* a compensating
  write that closes an unsent offer, which would need the delivery edge to report failure back
  into the core — a new feedback path for a row that already expires on its own.
- **This unit's choices are about the assistant's own answer and nothing else, 2026-08-25.**
  A one-tap moderation control — warn, mute, ban, delete — is deliberately out of scope, and
  this mechanism must not be used to build one without a decision that reopens 0070. The
  reason is not that a tap is not a human decision; it is that 0070 places the effect with the
  group's administrators through their own moderation bot, and a button that made the
  assistant the actor would move it. The mechanism as specified gives the assistant no power
  over any person: the only thing a press can do is tell the assistant which question to
  answer. And because a press produces no originating reference, the report tool's target
  validation can never aim at one — a tap is not reportable, by construction and not by
  rule. *Flagged, not specified*, per the standing invariant.
- **Nothing streams, and the one place a future stream touches this is named, 2026-08-25.**
  This unit moves no bytes: the payload is capped at sixty-four bytes by the platform, an
  offer holds at most four short labels, and no file, no media and no upload is involved. The
  answer's existing path — finalise, then chunk on the way out — is unchanged, and the options
  attach to the final chunk of the answer they belong to, so the buttons sit under the end of
  that answer. OPEN, for whoever makes the answer stream: the final chunk is only knowable at
  the end of the stream, so a streaming delivery must hold the options until the stream closes,
  never attaching them to the first piece it sends.

## The unit's contract

After 07a, the core reads "what summoned this turn" through one neutral reading that any
turn-opening kind answers, and the authority fold, the disclosure resolution, the owing-tail
walk and the two budget counts are all written against that reading and not against the
chat message by name; no behaviour changes and every existing test passes. After 07b, an
answer may carry a small set of offered options. The model offers them by calling
`offer_choices`, a palette-admitted core tool constructed at the assembly, which validates its
labels, refuses a second call in the same turn, and appends one `choice_offer` block naming the
newest own-debt-taker of the turn's co-summoner set, that person's authority as it stood on
that turn, and the labels as offered; the delivery loop attaches the options to the first
answer of that same turn and the adapter renders them as an inline keyboard on that answer's
final chunk. When a member taps one, the adapter decodes the `callback_query` update into a
neutral `InboundSelection` carrying the token and the presser alone, and the core decides from
the local store: the token must resolve to an offer, the offer's conversation must still be
mapped for this adapter, the presser must be the person it was offered to and must not have an
objection standing, the offer must be undecided and its block younger than twenty-four hours.
A taken selection appends one `choice_selection` block under the stamp lock, conditional on no
selection for that offer already existing, carrying the authority recorded on the offer,
spending a budget slot and owing the model a turn — so the assistant answers the disambiguated
question next, with its tools admitted at the inherited authority and without repeating its
introduction. The adapter then answers the press once, silently for a taken selection and with
the core's brief notice for a refusal, and clears the keyboard on the message the press named
when the platform gave it one. A press by anyone else records nothing and creates no identity
row for the presser. No block is ever rewritten: an offer's closure and its expiry are both
derived from what is on the ledger, and both new kinds are read through by the debt walk so an
open debt behind them still owes. The core gains no platform word, the adapter gains no
decision and no user-visible wording of its own, and four platform words the vocabulary check
did not know about are added to it. No new dependency, no new configuration.

## Acceptance criteria

- **AC1** Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the
  platform-vocabulary scan and the secret scan clean; no new dependency, no new configuration
  entry.
- **AC2** *(07a)* The seam changes nothing: after the summoning reading replaces the message
  kind at all four readers, the entire existing suite passes without a test being edited, and
  the co-summoner order pin and every budget-window pin are among the tests run unchanged.
- **AC3** *(07a)* The seam is the only reader: no module outside the choice module and the
  message kind's own module matches on a summoning kind by name — pinned by a source scan in
  the core's test suite, in the shape of the existing vocabulary scan.
- **AC4** The poll asks for the press update: the consumed update types include the callback
  query type, the decoder reads the query's id, sender, and — leniently, each Optional as the
  API documents it — its data and its originating message; a query carrying no data is
  answered with no text and skipped, and a query carrying no message is decoded and processed
  normally — pinned.
- **AC5** The offer tool appends its block and the options reach the wire: a scripted turn
  calling `offer_choices` with two labels appends one `choice_offer` naming the newest
  own-debt-taker of the turn's co-summoner set and that person's authority, and that turn's
  first answer is delivered with two options whose tokens are each at most sixty-four bytes —
  pinned, with the byte bound asserted, never assumed, and with a second pin over a turn
  carrying an absorbed mid-turn ask proving the addressee is the newer of the two askers.
- **AC6** The tool refuses what it cannot honour, and every refusal speaks to the model: one
  label, five labels, an empty label, a label over the core's character cap, two labels equal
  after trimming and case folding, a second call in the same turn, and a turn whose
  co-summoner set is empty — each returns a tool failure naming the reason and appends no
  block — pinned, one case each.
- **AC7** The intended person's press is taken: a selection naming a live offer by its
  addressee appends exactly one `choice_selection` carrying the option index, the label as
  offered and the authority recorded on the offer; the press is answered with no text; the
  keyboard is cleared on the message the press named — pinned.
- **AC8** Somebody else's press is refused and leaves no trace: a selection on the same offer
  by a different sender appends no block, creates no principal row for that sender (the
  identity tables are unchanged), and answers with the core's brief notice — pinned, with the
  identity-table assertion explicit.
- **AC9** A press by a person whose objection stands appends nothing and answers with a
  refusal notice, including when that person is the offer's own addressee — pinned, so the
  record of processing's statement that a standing objection stops collection stays true.
- **AC10** A press whose conversation is no longer mapped for this adapter is refused, and no
  administrator list is fetched and no withdrawal is issued on the press path — pinned, with
  the absence of the network call asserted against the scripted client.
- **AC11** The offer is single use and the repeat is idempotent: a second press by the same
  person naming the same option appends nothing and answers taken; a second press naming a
  different option is refused; across all three presses the offer's stored row is unchanged,
  asserted column by column over its named columns and its block header's creation time —
  pinned.
- **AC12** An expired offer is refused: a press against an offer whose block has been aged past
  the window through the suite's existing receipt-ageing helper appends nothing and answers
  with the expiry notice, while a press one second inside the window is taken — pinned that
  way and not on real time, matching how every budget window in this suite is proven.
- **AC13** A taken selection opens a turn and spends a slot: the selection sets the answer-owed
  stamp with the summons addressed and not literally addressed, the model answers next, a
  carried tail debt propagates through it, the two budget counts each count it, and a selection
  by a sender the budgets have silenced is recorded with the refusing budget named, answers
  nothing, and still closes its offer — pinned.
- **AC14** A press-opened turn is a full turn: its tools are admitted at the authority recorded
  on the offer, not at the floor, and its answer does not repeat the first-interaction
  disclosure line for a person already introduced — pinned, one test each, because these are
  the two regressions the seam exists to prevent.
- **AC15** The debt walk reads through both new kinds: with a debt open, a turn that appends a
  `choice_offer` as the conversation's tail still hands that debt to the next message's stamp,
  and the same holds behind a `choice_selection` that owes nothing — pinned, and both kinds
  appear in the read-through set and declare themselves frontier-transparent.
- **AC16** The model sees an act, not a quotation: the projection of a selection renders through
  the shared speaker prefix as a parenthesised choice, never as prose in the member's voice,
  the offer block projects nothing in either mode, and an erased selection projects the shared
  erased marker — pinned.
- **AC17** Erasure reaches both new kinds, symmetrically with the message pass: erasing a
  person nulls the chosen label and the speaker on their selections and the addressee reference
  on offers made to them, so an unpressed offer becomes unpressable and a pressed one reads
  back erased; the block header rows, their creation times included, are untouched — pinned,
  with the erasure module's own step-one documentation naming the new pass beside the report
  pass, and with the pass composed at the erasure operation the way the report pass is.
- **AC18** The adapter carries no wording and no decision: the answered text for each outcome
  is identical to the core constant for that outcome, an adapter test asserts every one of
  those constants against the adapter's own notice cap, a notice that exceeded the cap would
  answer with no text and log an error, and an absent or inaccessible originating message skips
  the keyboard clearing without changing any other outcome — pinned.
- **AC19** The core carries no platform vocabulary and the scan can see it: callback, keyboard,
  inline and markup are added to the platform-vocabulary file and the core scan passes with
  them present — so the check is evidence for this unit instead of silent about it.
- **AC20** The keyboard-clearing inference is either proven or replaced: before merge, a live
  call against a real bot token confirms which form of `editMessageReplyMarkup` clears an
  inline keyboard — an empty rows array, an omitted `reply_markup`, or neither — and the
  spec's decision and the adapter are corrected to whichever it is. Recorded here as unproven,
  in the shape unit 06 uses for its own withdrawal inference.
- **AC21** The privacy and compliance documents are true on the day this merges: the record of
  processing, the impact assessment, the legitimate-interests assessment, the public privacy
  policy and the AI Act record each gain exactly what the launch notes below specify, checked
  against that list — not against a reader's judgement, and not after the merge.

## Notes for launch

- Branches from `main` into its own worktree; merges back and the worktree is deleted, as every
  unit does. 07a merges before 07b, as its own commit at least.
- 07a's sites: the summoning reading declared beside the message kind in `kind.rs`, produced by
  `ChatMessage` and by nothing else at first; `co_summoners` and `turn_reading` in
  `tools/provenance.rs` returning and folding over it, with the frontier condition widened;
  `Disclosure::first_answer_to_someone` and `introduction_in` (`disclosure.rs:145`, `:205`)
  reading its principal; the privacy tool's principal resolution and the report tool's target
  validation reading its principal and its originating reference; `owing_tail_debt`
  (`assembly.rs:1535`) reading it instead of matching the message kind; the two counts
  (`kind.rs:950`, `:971`) applying the shared counted-debt fragment across every summoning
  table.
- 07b's core sites: a `choice` module holding both kinds, their content descriptors, their
  stored shapes, their projections and the selection's summoning production, composed into
  `AssistantKind` beside the report kind (`kind.rs:1118-1133`), both kinds declared
  frontier-transparent and added to `DEBT_READ_THROUGH` (`assembly.rs:59`); the `offer_choices`
  tool with its `NAME`, its `REQUIRED_AUTHORITY`, its label validation, its filing lock and its
  erasure fence, constructed at the assembly beside the report tool (`assembly.rs:447`) and
  deliberately not in `production_lookups` (`tools/mod.rs:76-116`); the neutral types in
  `message.rs` (`ChoiceOption`, the option set on `OutboundReply`, `InboundSelection`,
  `SelectOutcome`) beside `DeliveryItem` (`message.rs:252`) and `ReplyKind` (`message.rs:331`);
  `Assistant::select` in `assembly.rs`, following `ingest` (`assembly.rs:623`) for the erasure
  fence, the stamp lock and the budget check, using `find_standing` (`identity.rs:101`) where
  `ingest` uses `resolve_principal` and re-reading the standing flag under the lock the way
  `suppressed_under_lock` (`assembly.rs:1261`) does, and resolving its conversation through the
  mapping module and not through channel admission; the anchor-joined option attachment in
  the delivery loop (`outbound.rs:322-385`) and in `deliverable_of` (`outbound.rs:478`); the
  selection's erasure pass in the choice module, composed at `erasure.rs:141-148` the way
  `erase_reported_origin` is, with the module documentation at the top of `erasure.rs` updated;
  the teaching in `teaching.rs` telling the model when offering options helps and that a member
  may ignore them and type instead.
- 07b's adapter sites: the callback query type added to `CONSUMED_UPDATE_TYPES`
  (`client.rs:103`) and to the decoded `Update` (`client.rs:109`), with `data` and `message`
  both decoded leniently as the API documents them; an `answer_callback_query` and an
  `edit_message_reply_markup` beside the existing client methods, both under the same
  rate-limit contract and both with the send ceiling, since they run inside the sequential
  batch; the markup on the final chunk in `send_message` (`client.rs:365`) and `send_body`
  (`client.rs:459`), with the token's byte length asserted before the send; the press branch in
  `translate.rs` and its handling in `process` (`driver.rs:359`), acknowledging or halting the
  batch exactly as an ingest does; the adapter-side test asserting every core notice constant
  against the notice cap.
- Accepted risk, stated plainly: the poll processes one batch sequentially, so a press can wait
  behind a slower update in the same batch and its answer can arrive after the platform has
  expired the query id — the presser sees a progress bar that gives up, while the selection
  itself is recorded correctly. The refusal is logged and skipped. Reordering the batch to serve
  presses first is not done here: it would break the update-offset discipline the poll depends
  on. If this proves common in the live group, the fix is to move the press onto its own path
  instead of reordering the batch.
- New decisions to write up, continuing from 0105: the summoning seam itself, which is the one
  with the widest reach; the opaque token and the ledger-held meaning; the offer bound to the
  newest own-debt-taker; a stranger's press touching no identity row; the objection's plain
  application to a press; the single answer after a local-only verdict; the press that names no
  channel; the inherited standing recorded on the offer; the twenty-four-hour window measured on
  the block header; the anchor join for attaching options; the moderation scope-out that leaves
  0070 intact.
- Privacy and compliance documents, changed with the code and not after it. The record of
  processing gains a data category for the offer and the selection — the assistant's own option
  labels, the addressee's and the presser's internal identifiers, the option chosen, and the
  block creation times that stand in for the offer's and the press's own times — and states
  that no new platform identifier is stored, that a refused press is recorded nowhere, that a
  press by a person whose objection stands is recorded nowhere, and that an offer may be
  recorded that was never shown to anyone because the answer carrying it never went out; its
  processor entry gains the selection line, since the projected selection is a new fact about a
  named person sent to the processor; its erasure section gains the new pass and names the two
  columns it nulls. The impact assessment gains an addendum dated 2026-08-25 covering one new
  processing act, an interaction that is not a message, and one path that touches a person who
  never spoke and deliberately stores nothing about them. The legitimate-interests assessment
  gains a sentence in its processing description and its necessity test: the interest and the
  necessity are unchanged, the interaction widens from a typed message to a typed message plus
  a tapped option. The public privacy policy gains plain sentences under its messages heading —
  when the assistant offers a choice and you tap it, it keeps which option you chose and when,
  deleted with everything else; if you tap a choice offered to somebody else, nothing about you
  is kept. The AI Act record's disclosure section gains a sentence confirming that an answer
  following a tap resolves the first-interaction line per person exactly as an answer following
  a typed message does, which is what AC14 proves.
- The operator contract gains a note: the assistant may attach options to an answer, only the
  person who asked can take them, a member may always ignore the options and type instead, and
  the options stop working after a day.
- Other specs in this series were written in parallel with this one, and two of them collide
  with this unit. Unit 05 states its second acceptance criterion as the consumed update types
  being *exactly* a four-element list; this unit adds a fifth, so after both merge one of the
  two fails. This unit's wording — the list *includes* the type it needs — is the form that
  survives further additions, and unit 05's should be relaxed to match, in unit 05, not here.
  Separately, where another spec assumes the assistant knows the platform identifier of its own
  sent messages, note that this unit deliberately does not record it and explains why in its
  decisions; that assumption should be reconciled in whichever unit needs it.
