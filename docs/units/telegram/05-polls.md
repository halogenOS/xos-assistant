# Telegram unit 05 — polls and quizzes: the assistant asks, and no vote comes back

Date: 2026-08-25. Revision 2, rewritten after two independent reviews of revision 1. A
poll is the one message shape on this platform that collects an answer from every member
instead of from one, and the platform will hand a bot either an anonymous count or a named
list of who chose what, depending on a single boolean set at creation and unchangeable
afterwards. That boolean is the whole unit. This spec ships polls the assistant can post
at an administrator's ask, in regular and quiz form, closes them on request, records the
result once it is final, and makes the named-voter form unreachable by leaving it out of
the core's vocabulary altogether, so no member's vote can enter this system, be projected
to a model, or be turned on the person who cast it. Decision 0070 says the assistant
assesses and a human decides; a poll is the shape most likely to blur that, so the parts
of it that could produce an effect are removed instead of restrained.

## What revision 1 got wrong, and what is still not solved

Revision 1 was checked by two reviewers against the live Bot API and against both
repositories. Its platform reading survived unchanged: every version-sensitive claim in
the Grounding section below was independently confirmed twice, and a third fetch on
2026-08-25 re-confirmed the strings this design rests on. Its design did not survive. Six
statements in revision 1 were false about our own tree, and this revision states them
plainly instead of quietly correcting them, because each one is a mistake the next author
could repeat:

1. **`Authority::Admin` is the group's creator, not an administrator.** Decision 0015 and
   `crates/adapters/telegram/src/authority.rs:63-66` map `"creator"` to `Authority::Admin`
   and `"administrator"` to `Authority::Moderator`. Revision 1 required `Admin` and
   dismissed `Moderator` as "an arbitrary middle with no reason behind it". That is
   backwards: `Moderator` is exactly the rung a platform administrator occupies, with
   decision 0015 as its reason, and requiring `Admin` would have made the feature
   reachable only by the one account that created the group.
2. **Nothing in the tree could have produced `Outbound::ClosePoll`.** The outbound edge
   derives every deliverable from a stored block, `deliverable_of(&blocks[index])` inside
   an id-ordered walk (`crates/core/src/outbound.rs:322-336`, `:478-501`). Revision 1
   named two block kinds, an ask and a platform fact, and no block for a close request.
   The close would never have been sent.
3. **A failed `sendPoll` had no path.** `consume_replies` logs and drops a failed send
   (`crates/adapters/telegram/src/driver.rs:748-758`). Revision 1 projected the ask block
   to the model from the moment the tool filed it, so a poll that never reached the group
   would have kept telling the model it existed.
4. **The bounds did not deliver the guarantee attached to them.** Revision 1 claimed a
   text the core accepts is never refused on the wire, then chose 200/80/150 characters
   against platform caps of 300/100/200 counted in UTF-16 code units. A character outside
   the basic plane costs two of those units, so a 151-emoji question passes the core and
   is refused by `sendPoll`. Revision 1 also quoted the explanation's "at most 2 line
   feeds" rule and then bounded the note by length only.
5. **The quiz check was the mechanism decision 0096 explicitly rejected.** 0096's own
   rejected-alternatives list names "a mechanical 'no answer without a preceding tool
   call' check" and refuses it because it is "trivially satisfied by an irrelevant
   lookup". Revision 1 specified that check and cited 0096 as requiring it. It also could
   not have been implemented as written: `BlockKind::ToolResult` carries `tool_call_id`
   and `content` and does not name the tool
   (`/home/claude/projects/agent-ledger/crates/agent-ledger/src/agency/tool_result.rs:19-24`).
6. **`ObserveOutcome::Withdraw` carries no channel, and a poll update carries no chat.**
   `Withdraw` means "the adapter performs the withdrawal"
   (`crates/core/src/message.rs:281-284`), performed as `leave(client, chat_id, …)`
   (`driver.rs:585-588`) on a chat id the adapter computes from the observation's own
   channel key. A poll update has no channel key, so revision 1's AC9 asked for a value
   the adapter could not act on.

Two limits survive this revision and are not solved. They are limits of the deployment and
of the platform, not of the design, and they are stated here so nobody reads the
acceptance criteria as a promise they do not make:

- **In helpful answering mode the tool will often decline a legitimate ask.** The turn's
  provenance reading is a minimum over the turn's own-debt-taking messages
  (`crates/core/src/tools/provenance.rs:281-288`), and in helpful mode every group message
  is stamped summoned and therefore takes its own debt (`crates/core/src/kind.rs:53-61`,
  the recast documented at `kind.rs:53-61`). Any member message arriving between the
  administrator's request and the tool call lowers the reading to `Member` and the call is
  declined. The window is one model turn, seconds; in a quiet group the ask succeeds, in a
  busy one it may take two or three attempts. This is decision 0043's accepted
  over-declining, and this unit does not add a second admission path to escape it. What it
  does add is teaching so the assistant says why and asks the administrator to repeat the
  request, instead of failing silently. AC5 pins both directions, including the admitted
  one, so the feature's reachability is checked and not assumed.
- **An ask filed but not yet delivered when the process restarts is lost silently.** The
  outbound edge seeds its cursors from stored state and treats everything already stored
  as history (`outbound.rs:11-22`, `:160-165`). A `poll` block appended by the tool and
  not yet yielded when the process dies is never sent. The design makes this harmless
  instead of dishonest: an ask block projects nothing, so the model is never told a poll
  exists until the platform confirms it. The administrator sees no poll and asks again.

## Grounding

### The platform, read from the live documentation on 2026-08-25

Read from `https://core.telegram.org/bots/api` and `/bots/api-changelog`, fetched and
searched in full, not recalled from memory, and re-fetched for this revision. **The task
brief names Bot API 10.1 (June 2026) as current; the changelog has moved past it** — 10.2
on 14 July 2026 and 10.3 on 24 August 2026 are published. The poll surface changed
materially in 9.6 and 10.0, so an implementation written from memory of 2024's API will
fail on the wire. The specific traps:

- **`correct_option_id` no longer exists.** Bot API 9.6 (3 April 2026) "Replaced the
  parameter *correct\_option\_id* with the parameter *correct\_option\_ids* in the method
  sendPoll" and the same rename in the `Poll` class. A quiz must send
  `correct_option_ids` as a JSON array of monotonically increasing 0-based indices.
- **The option list is 1–12, not 2–10.** `sendPoll.options` is "A JSON-serialized list of
  1-12 answer options" of type `InputPollOption` (a JSON object with `text`, not a bare
  string — changed in 7.3); 10.0 "Decreased the minimum number of poll options from 2 to
  1", and the maximum reached 12 in 9.1.
- **`open_period` is 5–2628000 seconds** (9.6 raised the ceiling from 600), exclusive with
  `close_date`, which must itself be 5–2628000 seconds in the future.

The rest of the surface, as the live tables state it:

- **`sendPoll`** — required: `chat_id`, `question` (1–300 characters), `options`. Optional
  and relevant here: `is_anonymous` ("True, if the poll needs to be anonymous, defaults to
  True"), `type` ("Poll type, "quiz" or "regular", defaults to "regular""),
  `allows_multiple_answers`, `allows_revoting` ("Pass True if the poll allows to change
  chosen answer options, defaults to False for quizzes and to True for regular polls"),
  `shuffle_options`, `allow_adding_options` ("not supported for anonymous polls and
  quizzes"), `hide_results_until_closes`, `correct_option_ids` ("required for polls in
  quiz mode"), `explanation` ("Text that is shown when a user chooses an incorrect answer
  or taps on the lamp icon in a quiz-style poll, 0-200 characters with at most 2 line
  feeds after entities parsing"), `explanation_parse_mode`, `open_period`, `close_date`,
  `is_closed`, `description` (0–1024 characters), `message_thread_id`, `protect_content`,
  `disable_notification`, `reply_parameters`, `reply_markup`. `members_only` and
  `country_codes` are "for channel chats only" and do not reach a supergroup. `chat_id`'s
  own note: "Polls can't be sent to channel direct messages chats."
- **The platform counts text in UTF-16 code units**, which is what our own client already
  records: `MESSAGE_UTF16_UNIT_LIMIT` is documented as "The platform's cap on one
  message's text, in UTF-16 code units — the unit the platform measures text in"
  (`crates/adapters/telegram/src/client.rs:31-34`). One character costs one or two of
  those units. This is the arithmetic revision 1 asserted away and this revision designs
  around.
- **Formatting is nearly absent.** `question_parse_mode` and
  `InputPollOption.text_parse_mode` both say "Currently, only custom emoji entities are
  allowed" — a poll question and its options cannot carry bold, italics, links or code.
  Only `explanation_parse_mode` is a full parse mode.
- **`stopPoll`** — "Use this method to stop a poll which was sent by the bot. On success,
  the stopped Poll is returned." Parameters: `chat_id`, `message_id` ("Identifier of the
  original message with the poll"), optional `reply_markup`, optional
  `business_connection_id`. A poll the bot did not send cannot be stopped. There is no
  `editMessagePoll`: a posted poll's question, options and marked answer are fixed for its
  whole life, correctable only by stopping it.
- **There is no `getPoll`.** The API exposes no read of a poll's current standing. What a
  bot knows is what an update told it.
- **A bot cannot vote.** No method casts a vote.
- **`Update.poll`** — "New poll state. Bots receive only updates about manually stopped
  polls and polls, which are sent by the bot." The `Poll` object carries `id`, `question`,
  `question_entities`, `options`, `total_voter_count`, `is_closed`, `is_anonymous`,
  `type`, `allows_multiple_answers`, `allows_revoting`, `members_only`, `country_codes`,
  `correct_option_ids`, `explanation`, `explanation_entities`, `explanation_media`,
  `open_period`, `close_date`, `description`, `description_entities`, `media`. **It
  carries no chat and no message id**: an arriving tally names only the poll's own
  identifier, so nothing but a stored mapping can say which conversation it belongs to.
- **The update's cadence is not documented, and the design must not assume one per poll.**
  The phrase is "new poll state", and a vote changes a poll's state, so a poll with
  `allows_revoting` — the platform default for a regular poll — can produce an unbounded
  number of updates for one poll. The documentation promises no rate and no coalescing.
  This shapes the volume decision below.
- **`Update.poll_answer`** — "A user changed their answer in a non-anonymous poll. Bots
  receive new votes only in polls that were sent by the bot itself." `PollAnswer` carries
  `poll_id`, `user` ("if the voter isn't anonymous"), `voter_chat` ("if the voter is
  anonymous", the channel-as-voter case), `option_ids` and `option_persistent_ids`, both
  of which "may be empty if the vote was retracted". **In an anonymous poll this update
  does not exist**, which is the mechanism this unit rests on.
- **`allowed_updates`** defaults to "all update types except *chat_member*,
  *message_reaction*, and *message_reaction_count*" — but only when the list is empty. A
  named list, which is what this adapter sends, receives exactly what it names.
- **Permission**: `ChatPermissions.can_send_polls` — "True, if the user is allowed to send
  polls and checklists". A supergroup that restricts it makes every `sendPoll` fail.
- **Since 9.6 an option can be added by a member** (`allow_adding_options`,
  `PollOption.added_by_user`, the `PollOptionAdded`/`PollOptionDeleted` service messages).
  The same table says it is "not supported for anonymous polls and quizzes", so an
  anonymous-only assistant cannot have member-authored text appended to its own message.

### Our tree

- **Poll updates do not arrive today.** `CONSUMED_UPDATE_TYPES` is
  `["message", "edited_message", "my_chat_member"]`, named explicitly on every poll
  (`crates/adapters/telegram/src/client.rs:103`, sent at `:313-320`) precisely so the
  selection is stated instead of inherited. Without `"poll"` in that array no tally can
  ever reach the process.
- **A member's poll message is already skipped.** `text_of` reads text or caption only
  (`translate.rs:466-472`), so a poll message translates to `Skip::NoText`
  (`translate.rs:165-167`) and never reaches the ledger.
- **Outbound is one-way and text-only.** `OutboundReply` carries channel, text, kind and
  an optional reply target (`crates/core/src/message.rs:373-394`); `deliverable_of` maps a
  finished assistant text block or a report block to it (`outbound.rs:478-501`); the
  driver's `consume_replies` sends and discards (`driver.rs:730-760`), and `send_body`
  throws the platform's returned `Message` away
  (`client.rs:459`, `let _sent: serde_json::Value = self.decode(response).await?`).
  Nothing in the tree can learn the identifier of a message the assistant sent.
- **The outbound edge maps blocks, one at a time, in id order.** `deliver_answers_and_reports`
  walks the conversation's loaded blocks, skips everything at or below the per-conversation
  cursor, and calls `deliverable_of` on each (`outbound.rs:322-336`). `deliverable_of` sees
  one block and nothing else (`outbound.rs:478-501`), so anything the adapter must be told
  has to be stored on the block that produces the item. Delivery is at-least-once from
  stored state, and what is stored when the edge is taken is history (`outbound.rs:11-22`).
- **`ObserveOutcome::Withdraw` carries no payload**, and its contract sentence is "the
  adapter performs the withdrawal" (`message.rs:281-284`). The adapter performs it as
  `leave(client, chat_id, …)` (`driver.rs:585-588`, `:613-623`), with the chat id computed
  at `driver.rs:479-482` from the observation's own channel key.
- **The observation surface is the existing inbound path for platform facts.**
  `Assistant::observe` (`assembly.rs:928`) takes an `Observation { channel, channel_kind,
  fact }` (`message.rs:218-240`), checks admission fail-closed, appends on delta under
  `stamp_lock` (`assembly.rs:503`, the lock's reasoning at `:348-358`) and answers
  `ObserveOutcome::{Observed, Withdraw}`. Authorization is keyed on the channel throughout
  (`authorization::is_authorized`, `assembly.rs:936-947`).
- **Group authorization is written once and never removed.** `group_authorizations` is
  `INSERT OR IGNORE` (`authorization.rs:46`) and `SELECT` (`:65`); no code path deletes a
  row. The reachable refusal is therefore "never admitted", not "revoked", and the
  module's own doc explains why a lost withdrawal needs no delivery guarantee: "a lost
  leave call is healed by the group's next contact, which is refused all over again"
  (`authorization.rs:7-11`).
- **A block kind that carries a group fact already exists, with the lifecycle a poll
  needs.** `ContextNote` is appended only when the observed text differs from the newest
  stored note of its topic, is `frontier_transparent` so it never buries an unanswered
  message (`note.rs:256-267`), projects in the system voice with supersession wording
  (`note.rs:281-293`, leads at `:41-48`), and refuses an over-bound text whole instead of
  truncating. Its bounds are **byte** bounds — `RULES_TEXT_MAX_BYTES` and
  `TITLE_TEXT_MAX_BYTES` (`note.rs:54-65`) — a detail revision 1 got wrong when it claimed
  to follow this module "exactly" while specifying character bounds.
- **A tool that appends a block for the outbound edge to deliver already exists.**
  `ReportTool` files a block naming a target the model must have seen this turn, validated
  against the turn's co-summoner set (`tools/report.rs:1-42`, `provenance::co_summoners`
  at `tools/provenance.rs:109-140`), projects nothing because the filing is machinery
  (`report.rs:17-19`), refuses a non-group conversation through
  `mapping::channel_for_conversation` (`report.rs:371-372`, `GROUP_ONLY_ERROR`), holds its
  own filing mutex across the scan-and-append pair because "the runner executes same-round
  calls in parallel tasks" (`report.rs:33-37`, `:335-341`, `:364-365`), stores the reported
  principal id "precisely so erasure can reach the block" through `erase_reported_origin`
  (`report.rs:38-40`, `:172-177`), and is registered at the assembly with the erasure fence
  (`assembly.rs:446-447`).
- **Tool admission is one path with an authority check, over one ledger load that is not
  shared.** Every handler is wrapped by `AdmittedTool::admit`, which loads the ledger
  itself (`tools/admission.rs:121-127`), checks the palette, then the anchor check of
  decision 0043, and **drops the vector before the inner handler runs**
  (`admission.rs:186-190`). The report tool re-loads independently (`report.rs:378`). A
  tool that needs the ledger loads it again; revision 1's "the vector it already loads for
  admission" described something that does not exist. Every shipped tool today requires
  `Authority::Member` (`commit.rs:33`, `release.rs:42`, `wiki.rs:69`, `rights.rs:56`,
  `report.rs:198`). `own_debt_taken` is `addressed && limited.is_none()`
  (`kind.rs:352-354`, `:536-538`).
- **The palette supersedes on activity, so an existing conversation gains new tools.**
  `reconcile_palette` appends a fresh palette block whenever the stored list differs from
  the registered one, once per conversation per process, on ingestion and on observed
  activity alike (`assembly.rs:1463-1493`). A conversation created before this unit
  therefore admits `post_poll` and `close_poll` from its next contact onward, with no
  migration.
- **`AssistantKind` is matched exhaustively in the provenance module.** `turn_reading`'s
  span filter lists every variant by name (`provenance.rs:88-94`), so a new variant is a
  compile error there until it is classified. `chain_step` restates its catch-all for the
  report kind on purpose, because that kind "is written INTO a live turn's window by the
  report tool itself, so its classification decides admission on the very turn that wrote
  it" (`provenance.rs:243-249`). A poll ask block is written into a live turn's window in
  exactly the same way.
- **A tool result does not name its tool.** `BlockKind::ToolResult` carries
  `tool_call_id` and `content`
  (`/home/claude/projects/agent-ledger/crates/agent-ledger/src/agency/tool_result.rs:19-24`);
  the name lives on `ToolCall` (`agency/tool_call.rs:30-31`) and is reachable only by
  joining on `tool_call_id`. A lookup that found nothing also returns a `ToolResult`, by
  decision 0097's miss sentinel. Any rule phrased as "a successful lookup happened" is
  therefore neither one field nor one predicate.
- **The framework has a human-clearance primitive already.** `BlockKind::ApprovalRequest`
  / `ApprovalDecision` park a tool call on `Awaiting::OutOfBand` until a human's verdict is
  submitted, and a denial resolves the call with a tool error
  (`/home/claude/projects/agent-ledger/crates/agent-ledger/src/agency/approval_request.rs:21-27`,
  `agency/approval_decision.rs:18-25`). Nothing in this repository uses it: `grep Approval
  crates/` finds nothing.
- **Content tables key on `block_id` and may join the framework's own tables.** The report
  table is `block_id INTEGER PRIMARY KEY REFERENCES blocks(id) ON DELETE CASCADE`
  (`schema.rs:296-309`), and the note module's reads "join the framework's `blocks` and
  `conversation_blocks` tables by name" as a recorded, deliberate coupling
  (`note.rs:295-300`). A conversation is therefore reachable from a content row by one
  join, which is what makes a reference-keyed lookup possible at all.
- **Schema growth is an appended migration step**, each one quoting its own frozen
  vocabulary (`schema.rs:289-309` for the report table, the ordered list at `:373-396`).
  `AssistantKind` composes each consumer kind so one parse path reads every block
  (`kind.rs:1118-1134`).
- **Erasure nulls text and projects a marker.** An erased message reads `None` for its text
  and projects `ERASED_MARKER`, `"[message erased]"` (`kind.rs:166`, `:379-382`, `:540`).
- **The chunking, retry and formatting machinery is text-specific.** `chunks_within_cap`
  (`client.rs:599`) and `MESSAGE_UTF16_UNIT_LIMIT` (`client.rs:34`) belong to
  `sendMessage`; the markdown-to-HTML renderer (`formatting.rs:54`) and its
  refused-formatting retry (`client.rs:478-503`) belong to it too. The rate-limit retry
  (`client.rs:29,45`) is per request and applies to any method.
- **The framework's attachments store moves no bytes.** Its own header: "No file bytes
  pass through here. This module tracks what an attachment is and which parts of it are
  already on disk; the caller owns the file itself"
  (`/home/claude/projects/agent-ledger/crates/agent-ledger/src/store/attachments.rs:1-4`).
  Revision 1 told a future author to "stream bytes through" it, which the module does not
  do.
- **The adapter's test fixture answers unknown methods with `{"ok":true,"result":true}`**
  (`crates/adapters/telegram/tests/adapter/server.rs:450`), so a wire test for `sendPoll`
  or `stopPoll` that does not add a real handler passes without proving anything.

## Decisions taken with this unit

- **The assistant posts anonymous polls only, and the core has no anonymity field at all,
  2026-08-25.** `is_anonymous` is never sent, so the platform's default — anonymous —
  stands, and the core's poll vocabulary carries no switch that could ask for anything
  else. The consequence is guaranteed by the platform, not by our care: `poll_answer`
  "receive[s] new votes only in polls that were sent by the bot itself" and only in
  non-anonymous ones, so no per-person vote can reach this process even through a defect,
  a misconfiguration or a future edit that forgets why. A member's vote is therefore never
  personal data this system holds, never text sent to the model provider, and never
  something that can be turned on the person who cast it. *Rejected:* a configuration key
  defaulting to anonymous — a default is a thing that gets changed, and the whole safety
  argument collapses the moment the field exists; *rejected:* a non-anonymous poll for
  "administrator-only" questions — the voter list is shown to every member of the group by
  the platform's own interface, so the exposure is to the room, not to us, and no
  administrator can consent on the voters' behalf.

- **`poll_answer` is not subscribed, 2026-08-25.** `CONSUMED_UPDATE_TYPES` gains `"poll"`
  and nothing else. Not asking for data we have decided never to hold is one line of code
  and one pinned assertion; it also means a future non-anonymous poll cannot start silently
  working. *Rejected:* subscribing and discarding in the adapter — that puts a decision in
  the adapter, where no decision belongs, and the discarded payload would still have
  crossed the process boundary.

- **The two tools require `Authority::Moderator`, which is what a platform administrator
  maps to, 2026-08-25.** Revision 1 required `Authority::Admin` on the reading that it
  means "administrator". It does not: decision 0015 and `authority.rs:63-66` map
  `"creator"` to `Admin` and `"administrator"` to `Moderator`, so `Admin` names the single
  account that created the group. `Moderator` is the rung the group's administrators
  actually occupy, and it is the correct requirement for a tool that must be above every
  member and below nothing else. The existing admission wrapper enforces it through the
  provenance fold with no new path. A poll interrupts everyone in the group, is uneditable
  once posted, and is decision-shaped; making it reachable to any member turns the
  assistant into a way to hold a vote about anything, including about a person.
  *Verified consequence, accepted:* the fold is a minimum over co-summoners
  (`provenance.rs:281-288`), and in helpful mode every group message takes its own debt, so
  a member speaking during the administrator's turn lowers the reading to `Member` and the
  call is declined for that turn. This is 0043's stated over-declining, quantified in the
  limits section at the top of this document, and it is preferred to a second admission
  path. *Rejected:* `Authority::Admin` (creator-only, as above — it would make the unit's
  own contract sentence false); *rejected:* reading the dispatch anchor's own sender
  authority instead of the fold — `admission.rs:16-22` says plainly that one admission path
  exists so no second one can drift from it; *rejected:* a new authority rung between
  `Moderator` and `Admin` — the platform expresses two elevated statuses and the core
  already models both.

- **A decline the administrator can act on: the tool result names the cause and the
  teaching turns it into a sentence, 2026-08-25.** The authority decline closes with the
  existing `NO_RETRY` line (`admission.rs:47-52`), which correctly stops the model from
  retrying inside the same turn. On its own that produces silence, and the administrator
  learns nothing. The teaching therefore carries one rule: when a poll request is declined
  for authority, say so plainly and ask the administrator to send the request again. The
  cause is real and explainable — another message arrived while the assistant was working
  — and repeating the request in a quieter moment succeeds. *Rejected:* suppressing the
  decline and posting anyway (it discards the check); *rejected:* an automatic retry in the
  core (nothing in this tree re-runs a turn, and a retry loop against a busy group is a
  loop with no end).

- **Quiz mode ships as a marked answer, and the requirement that the mark be verified stays
  teaching, 2026-08-25.** A quiz is a poll carrying `answer_option` and an optional
  `answer_note`; the adapter maps them to `type: "quiz"`, `correct_option_ids: [i]` and
  `explanation`. A quiz publishes a claim about the project that the platform marks as
  correct, to the whole group, uneditable for the life of the poll. Revision 1 tried to
  enforce decision 0096 mechanically, by declining a quiz whose turn held no lookup result.
  That is withdrawn, for three reasons, each checked: 0096's own rejected-alternatives list
  names that mechanism and refuses it as "trivially satisfied by an irrelevant lookup", so
  the spec was citing a decision that had already decided against it; the check cannot be
  implemented from the block it named, because `ToolResult` does not carry the tool's name
  and a miss returns a `ToolResult` like a hit; and the runner executes same-round calls in
  parallel (`report.rs:33-37`), so a model emitting the lookup and the quiz in one round
  would race the check for no reason it could understand. The rule stays exactly where 0096
  put it, in the teaching, with the miss handling of 0097 behind it. *Rejected:* the
  mechanical check, as above; *rejected:* dropping quiz mode (a knowledge quiz off the wiki
  is a genuinely useful thing in a community group, and refusing it wholesale costs more
  than the honest teaching); *rejected:* requiring `answer_note` to be non-empty as a proxy
  for having checked (a required field is filled, not verified, and a wrong note is worse
  than none).

- **The core's poll vocabulary is small on purpose, 2026-08-25.** A poll is a question, two
  to twelve options, an optional marked answer with an optional note, and a duration. The
  platform's other switches — `allows_multiple_answers`, `allows_revoting`,
  `shuffle_options`, `allow_adding_options`, `hide_results_until_closes`, `description`,
  `media`, `protect_content`, `members_only`, `country_codes`, `reply_parameters` — are not
  modelled and not sent. Mirroring them would make the core's vocabulary a copy of one
  platform's parameter table, which is the invariant's exact failure mode; each ships when a
  real need names it, in neutral words. Two of them are unavailable anyway: `members_only`
  and `country_codes` are documented "for channel chats only", and `allow_adding_options`
  is "not supported for anonymous polls and quizzes", so an anonymous poll can never grow
  member-authored option text.

- **The core bounds text in characters at half the platform's cap, refuses whole, and
  refuses a line feed in the note, 2026-08-25.** A question is 1–150 characters, an option
  1–50, an answer note 0–100, the option count 2–12, the duration 300–604800 seconds. The
  reason for the halving is arithmetic, and revision 1 asserted it away: some platforms
  count text in 16-bit units, where one character costs up to two, so a character bound set
  at half the smallest platform cap is the largest bound that cannot be exceeded on the
  wire by any input. 150 × 2 = 300, 50 × 2 = 100, 100 × 2 = 200 — each exactly the
  platform's own cap, so the guarantee is a computation and not a hope. The answer note
  additionally refuses any line feed: the platform's explanation is documented as "0-200
  characters with at most 2 line feeds after entities parsing", and a note that is one
  sentence needs none. An over-bound or line-feed-carrying field is refused whole with the
  reason returned to the model, never truncated: a cut question is a different question.
  The refuse-whole discipline follows `note.rs`; the *unit* of the bound does not — the note
  module bounds bytes (`note.rs:54-65`) and this one bounds characters, because the
  guarantee above is a character-to-code-unit ratio and bytes do not express it.
  *Rejected:* passing the platform's own numbers into the core (leaks one platform's
  arithmetic into shared behaviour); *rejected:* counting UTF-16 code units in the core
  (the guarantee would then be stated in one platform's counting unit, which is the
  vocabulary invariant's failure mode in a different disguise); *rejected:* the 200/80/150
  bounds of revision 1 with a note that astral text may fail (a bound whose stated
  guarantee is false is worse than a smaller bound that holds); *rejected:* truncating to
  fit (changes what was asked); *rejected:* a minimum of one option, which the platform now
  allows — a one-option poll is not a question.

- **A poll's text is sent plain; the markdown renderer is not applied, 2026-08-25.** The
  platform allows only custom emoji entities in a question and in an option, so bold, links
  and code cannot be expressed there at all; the explanation would accept them, and a poll
  formatted in one field out of three reads as a defect. Sending plain text also removes the
  whole refused-formatting retry path (`client.rs:478-503`) from this method: plain text
  cannot be rejected for bad markup, and with no parse mode sent there are no entities, so
  the explanation's "after entities parsing" bound is its raw length. *Rejected:* rendering
  the explanation only (inconsistent, and it re-imports the retry for a 100-character
  field).

- **A poll is three block kinds: two asks and one platform fact, 2026-08-25.** The outbound
  edge derives every item from one stored block (`outbound.rs:478-501`), so every request
  the adapter must perform needs a block of its own; revision 1's two kinds left the close
  request with no producer, and it would never have been sent. The kinds are: `poll`, the
  ask to post, carrying question, options, marked answer, note, duration and the asking
  member's principal id; `poll_close`, the ask to close, carrying the `poll` block's id and
  the platform references copied from the posting fact at filing time, so the edge can build
  the request from this block alone; and `poll_state`, one appended row per platform fact
  about an ask, carrying the ask block's id, the platform's opaque poll reference, the
  message origin, an outcome from a closed vocabulary (`posted`, `unsent`, `closed`), the
  counts, a copy of the question for projection, and a reason for a refusal. The ledger is
  append-only, so a block written before a request cannot later acquire what the request
  returned; a fact is a new row. *Rejected:* one kind with a mutable identifier column
  (rewrites history); *rejected:* two kinds, as revision 1 had it (no producer for the close
  — the defect this decision exists to repair); *rejected:* one fact kind per event
  (posting, refusal and closing are the same shape of statement about the same object,
  distinguished by one closed-vocabulary column with a CHECK constraint, which is this
  repository's established way to close a vocabulary).

- **An ask block projects nothing; the platform's answer is what the model reads,
  2026-08-25.** Revision 1 projected the `poll` block from the moment the tool filed it,
  which told the model a poll existed before it was sent and kept telling it so forever when
  the send failed — and `sendPoll` does fail, on a supergroup that restricts
  `can_send_polls`, on an exhausted rate-limit retry, or on any refusal, all of which
  `consume_replies` currently logs and drops (`driver.rs:748-758`). Both ask kinds therefore
  project nothing, following the report block's precedent (`report.rs:17-19`), and the
  `poll_state` row projects instead: a `posted` fact projects the mark, the question and the
  options in the system voice ("Poll 14: <question> — options A, B, C"), an `unsent` fact
  projects the refusal ("Poll 14 was not posted: <reason>"), and a `closed` fact projects the
  result under supersession wording ("Poll 14 closed: A 3, B 7"). The mark is the ask
  block's id, the same way the projected origin mark lets the model name a message to report
  (`kind.rs:545-554`). The question text is copied onto the `posted` and `unsent` rows when
  the core appends them, because projection reads one block and cannot join
  (`outbound.rs:478-501` shows the same one-block limit on the delivery side). *Rejected:*
  projecting the ask and appending a retraction on failure (the model would have read the
  poll as real for the whole turn that asked for it); *rejected:* a cross-block projection
  that reads the ask through its fact (the `Projection` trait's methods take `&self` and no
  ledger — see the note kind's own implementation at `note.rs:283-293`).

- **The adapter reports the platform's refusal as a fact, and decides nothing about it,
  2026-08-25.** After `sendPoll` or `stopPoll` returns, the adapter reports the outcome to
  the core through the observation surface: success as `Posted` or `Standing`, failure as
  `Unsent { ask, reason }` where the reason is the client's own bounded error text. The
  adapter classifies nothing — a returned error is a fact about the platform's answer, which
  is precisely what an adapter translates — and the core decides what to record and what to
  project. This is what makes the failure path exist at all. *Rejected:* logging the failure
  in the adapter as today (the ledger and the model would never learn of it, which is the
  defect); *rejected:* the adapter retrying the send itself (a decision, and the client's
  own rate-limit retry at `client.rs:29,45` already covers the one retryable case).

- **The outbound edge carries typed items on the existing channel, 2026-08-25.**
  `Assistant::replies` yields an `Outbound` enum: `Reply(OutboundReply)` unchanged, plus
  `PostPoll` (the ask block's id as the key, the channel, the question, the options, the
  marked answer, the note, the duration) and `ClosePoll` (the ask block's id, the channel,
  the stored platform references). One channel keeps the order of a turn's own output. The
  ordering that results must be stated correctly, because revision 1 stated it backwards:
  the edge walks blocks in ledger id order (`outbound.rs:322-336`) and the tool files its
  ask block before the model's answer text is committed, so **the poll arrives first and the
  assistant's sentence follows it**. That is a fixed, checkable order, not the one revision 1
  claimed, and the teaching is written to match it. The adapter's `consume_replies` matches
  on the item and decides nothing. *Rejected:* a second channel for polls, mirroring the
  composing edge — it loses ordering between an answer and its poll, which is visible to
  every member; *rejected:* encoding a poll into `OutboundReply.text` — the adapter would
  have to parse behaviour out of a string.

- **A poll fact reaches its conversation by a reference lookup in the core, 2026-08-25.**
  The platform's poll update carries no chat, so a `Standing` fact names only the poll's own
  identifier. The core resolves it with one query on its own domain tables: the
  `poll_state` row whose reference matches, joined to `blocks` for the conversation, the
  same deliberate cross-table join the note module's reads already document
  (`note.rs:295-300`). The reference column carries an index, and the `posted` row is unique
  per reference by the at-most-one rule below, so the query answers zero or one row. An
  adapter that remembered the mapping itself would be holding state and making a decision.
  *Rejected:* a mapping table of its own (the fact is already on the row that records the
  posting; a second copy is a second thing to keep true); *rejected:* the adapter caching
  reference-to-chat (state and a decision in the adapter, and it would not survive a
  restart).

- **`ObserveOutcome::Withdraw` gains the channel to withdraw from, 2026-08-25.** Today the
  adapter recomputes the chat from the observation it passed in (`driver.rs:479-482`), which
  works only because every observation carries a channel. A poll observation does not, and
  revision 1's answer — a `Withdraw` the adapter could not act on — was a dead value. The
  outcome therefore carries the channel key the core resolved, and the adapter translates
  that key instead of recomputing one: for a channel observation the core hands back the key
  it was given, and for a poll observation the key it resolved through
  `mapping::channel_for_conversation` (already used at `outbound.rs:316`). This is a
  correction of a seam, not a new mechanism: the outcome now states what the adapter must
  act on. A poll fact whose reference resolves to no conversation, or whose conversation has
  no mapping, is dropped as unknown and never produces a withdrawal. *Rejected:* leaving
  the enum unchanged and answering `Observed` for an unadmitted group's poll (the ledger
  stays clean, but the withdrawal silently stops happening for a whole class of contact);
  *rejected:* a separate `observe_poll` entry point with its own outcome type (it duplicates
  the fail-closed check and invites the two to drift, which is exactly what
  `admission.rs:16-22` warns against for the tool path).

- **`Observation` becomes a two-variant input so one surface keeps one admission path,
  2026-08-25.** `Assistant::observe` takes `ObservationInput::Channel(Observation)` —
  today's struct, unchanged — or `ObservationInput::Poll(PollObservation)`. Both run the
  same fail-closed authorization check and both append under `stamp_lock`. *Rejected:* a
  `subject` field beside the existing `fact` field, which would make illegal pairs
  representable — a title fact about a poll.

- **Only a final tally is recorded, and a running count is refused before any store work,
  2026-08-25.** A `Standing` fact with `closed = false` is dropped with a debug line, in the
  core, before the reference lookup and before `stamp_lock` is taken. The placement matters
  and revision 1 left it unstated: the platform documents no cadence for `Update.poll`, a
  vote changes a poll's state, and `allows_revoting` defaults to True for a regular poll, so
  one member toggling a vote can produce an unbounded stream of updates. Every one reaches
  the core, because the drop is behaviour and behaviour is not the adapter's; the cost of
  each is one enum match and a log line, and none of them touches the store or the lock that
  serializes ingestion (`assembly.rs:348-358`). The protection counters of decisions 0030
  and 0034 count messages and do not see observations, so nothing else bounds this: the
  cheap early exit is the bound. The ledger records what was said, and nobody said "seven":
  a running count is a counter the platform maintains and re-renders, changing without any
  act being recorded. Two facts about a poll are statements in the honest sense and both are
  recorded: the assistant asked this question with these options at this time, and the poll
  closed with these counts. *Rejected:* appending every differing snapshot and projecting
  none (an unbounded number of blocks whose only reader is somebody reading the database by
  hand); *rejected:* recording the last snapshot before closing (nothing can know which
  snapshot is the last, because there is no `getPoll` and no delivery guarantee);
  *rejected:* filtering in the adapter to save the call (a decision in the adapter, and the
  saving is one function call).

- **At most one posting fact and at most one closing fact per ask; the first of each wins,
  2026-08-25.** Revision 1 said `poll_state` records "one row per fact, superseding by being
  later" and then required a second closing fact to append nothing, which are two different
  rules; and the case that actually arises is not a byte-identical duplicate but a
  near-duplicate — `stopPoll`'s response and a `poll` update for the same close, one of them
  counting a vote the other missed. The rule is therefore stated as a property of the ask,
  not as a similarity test: an ask that already carries a `posted` or `unsent` row takes no
  second one, and an ask that already carries a `closed` row takes no second one. The first
  closing fact is the record because it is what the assistant learned when the poll closed;
  a later reading of an already-closed poll is not a new statement by anyone. The scan and
  the append run under the observation's `stamp_lock`, so two concurrent facts cannot both
  find the ask empty — the same read-then-append serialization the note path already relies
  on (`assembly.rs:974-1008`). *Rejected:* superseding by being later (it makes the record
  depend on delivery order, which nothing controls, and it appends a block per redelivered
  update); *rejected:* a duplicate test on the counts (it answers only the case that does
  not happen).

- **An explicit close is the reliable path to a result, and the timer is the backstop,
  2026-08-25.** `stopPoll` is documented to return the stopped `Poll`, so an administrator
  asking the assistant to close a poll yields a final tally from the method's own response,
  with no dependence on an update arriving. Every poll is also sent with an `open_period`,
  so a forgotten poll closes on the platform's side even if this process is down. *Named
  residual:* the documentation does not promise a `poll` update when a poll auto-closes, and
  there is no way to read a closed poll back, so a poll that runs out its timer while nobody
  closes it may leave no result on the record. The assistant then says the result is
  unknown; it does not guess and does not invent a count. *Rejected:* a scheduler in the
  core that closes polls at their deadline (no durable timer machinery exists, and adding one
  for this is a unit of its own); *rejected:* omitting `open_period` to force the explicit
  path (a poll nobody closes stays open in the group forever).

- **A poll fact never opens a turn, 2026-08-25.** All three kinds are
  `frontier_transparent` and agency-inert, exactly like a context note (`note.rs:256-267`),
  so a closing result buries no unanswered message and owes no turn. In the provenance
  module all three classify as `ChainStep::Extends`, and each arm is written out instead of
  falling to the catch-all, following the report kind's own reasoning at
  `provenance.rs:243-249`: an ask block is written into a live turn's window by the tool
  that filed it, so its classification decides admission on the turn that wrote it. The
  assistant does not announce a result on its own; it states the counts when someone asks,
  which is what decision 0098's silence default already says. *Rejected:* making a close owe
  a turn — a poll's timer would then make the assistant speak unprompted, which is a machine
  deciding to talk; *rejected:* letting the three new variants fall to `chain_step`'s
  catch-all (the classification would be invisible at the point that decides it).

- **`close_poll` names a poll the model can see, validated against this conversation's open
  polls, 2026-08-25.** The tool takes the projected poll mark, loads the conversation's
  ledger itself — `AdmittedTool` drops its own load before the handler runs
  (`admission.rs:186-190`), so this is a second read and the spec says so instead of
  pretending otherwise — and scans for a `poll` block in this conversation carrying a
  `posted` fact and no `closed` fact. An unknown mark, another conversation's poll, an
  unsent poll or an already-closed poll is declined with the reason. The scan and the append
  of the `poll_close` block run under the tool's own filing mutex, following
  `report.rs:335-341` and for the same reason: the runner executes same-round calls in
  parallel, so two calls naming one poll must not both find it open. *Rejected:* closing
  "the most recent poll" implicitly (ambiguous the moment two polls are open, and it hides a
  mistake instead of refusing it).

- **Both tools are group-only, 2026-08-25.** `Assistant::observe` returns early for a
  non-group channel — "a direct-channel observation observes nothing" (`assembly.rs:944-947`)
  — so a poll posted in a direct chat would record no posting fact and could never be closed
  or reported. Direct chats are a live switch (decision 0069), so this is reachable. Both
  tools therefore refuse a non-group conversation the way the report tool already does,
  through `mapping::channel_for_conversation` and a fixed group-only error
  (`report.rs:371-372`). *Rejected:* letting the observation path grow a direct-chat branch
  (it would add a second lifecycle for a surface nobody asked for); *rejected:* leaving it
  unstated, as revision 1 did (the failure is silent and permanent).

- **Erasure reaches the poll record, keyed on the asking member's principal id,
  2026-08-25.** Revision 1 borrowed decision 0055's conclusion — an unreachable record,
  recorded as a gap — while its premise did not transfer: 0055 rests on the note quoting the
  group's own published governance, and a poll question is generated inside a turn whose
  context is members' messages. The sibling precedent it should have cited is decision 0063,
  which states that "the report block stores the reported message's principal id precisely
  so erasure can reach it" and refuses the open-gap shrug where a principal reference is
  available. One is available here: the ask is made in a turn with a resolvable summoner.
  The `poll` block therefore stores the asking member's principal id, and that person's
  erasure nulls the question, the options and the answer note on the ask block and the
  copied question on its `poll_state` rows, through a crate-private pass the erasure
  operation composes, exactly as `erase_reported_origin` is composed (`report.rs:172-177`).
  A nulled poll projects the erased marker in place of its text (`kind.rs:166`), and
  `close_poll` can still name it by its mark, because the mark is a block id and not prose.
  The references and the counts stay: they identify nobody. *Named residual, and it is the
  one 0055 already records:* a question can name a third person, and no key reaches that.
  This does not become a third recorded gap — it widens the second one, whose subject is
  exactly "governance prose no principal id reaches", and the privacy documents are edited
  to say so in every place the gap is described. *Rejected:* the open-gap shrug of revision
  1 (0063 refuses it where a key exists, and one exists); *rejected:* keeping the text and
  nulling only the principal id (the id is the key; keeping the prose is the thing erasure
  is for); *rejected:* recording a third gap (there is no third unreachable surface — the
  poll text is reached, and its third-person case is the second gap's own subject).

- **Nothing streams here, and the reason is stated, 2026-08-25.** A poll is a small JSON
  body in one request with no upload and no download; a poll cannot be chunked, because its
  limits are per-field and hard, so the text-chunking path (`client.rs:599`) is not on this
  method. Poll media — `InputPollMedia`, `InputPollOptionMedia`, the photo, video and
  sticker forms 10.0 added — is out of scope. When it ships it moves bytes, and it must
  stream them from the platform to disk and from disk into a multipart body chunk by chunk,
  never buffered whole. The framework's attachments store is the right place to record
  *what* an attachment is and which byte ranges are already on disk, and it is explicitly
  not the path the bytes travel: "No file bytes pass through here … the caller owns the file
  itself" (`crates/agent-ledger/src/store/attachments.rs:1-4`). Revision 1 told the next
  author to stream bytes through that module, which it does not do.

- **What stays teaching, stated plainly, 2026-08-25.** No mechanism can read a question's
  intent, so the rule that a poll is never a vote about a person lives in the teaching,
  where it binds the model's judgment and permits the assistant to refuse the ask. What the
  mechanisms deliver instead is that such a poll would be ineffective: nobody is identifiable
  in the result, no tool takes a poll result as an input, a poll fact never opens a turn, and
  the report tool's target must be a message in the turn's co-summoner set, so a tally can
  never become a report target. Decision 0070 is satisfied structurally — there is no path
  from a count to an effect on a person — and the teaching covers the part that is about
  taste and decency instead of about power. Because AC12 asks for a verbatim check, the four
  sentences are written here and the implementation copies them:
  1. "A poll is never a vote about a person, and never asks the group whether someone should
     be removed, muted, warned or punished."
  2. "The assistant cannot see the results of a poll it did not post itself."
  3. "A poll's result is what the group said when it was asked, not an instruction to do
     anything."
  4. "When a poll closed and no result reached the assistant, the assistant says the result
     is unknown and does not estimate it."

- **The framework's approval primitive is not used here, and this records when it becomes
  mandatory, 2026-08-25.** `ApprovalRequest`/`ApprovalDecision` would park the tool call
  until a named human approves the exact question and options. It is not used, for one
  reason: submitting a decision needs a human-facing surface this consumer does not have,
  and building that surface is a unit of its own. It is not needed here either, because
  posting a poll produces no effect on any person — decision 0070 binds moderation effects,
  and a question is speech. The moment a poll result is proposed as an input to anything that
  touches a person's standing, that unit ships the approval surface first; this spec names
  the primitive so the next author does not invent a second one. *Rejected:* a home-grown
  confirmation exchange ("reply YES to post") — it reinvents an existing seam and would be
  the assistant deciding what counts as a confirmation.

- **A member's own poll stays invisible, and the skip says so, 2026-08-25.** The platform
  gives a bot no way to read a poll it did not send: `Update.poll` arrives only for the
  bot's own polls and manually stopped ones, `stopPoll` refuses another sender's poll, and
  there is no `getPoll`. A member's poll message therefore records nothing, as today — but
  `Incoming` gains the `poll` field so the skip is named `Skip::PollMessage` instead of the
  misleading `Skip::NoText`, and the log line says why the assistant cannot see it.
  *Rejected:* recording the question text of a member's poll (the counts would always be the
  zero snapshot the message carried, which reads as data and is not).

- **A tally for an unknown reference is dropped, not stored, 2026-08-25.** The
  documentation's phrase "manually stopped polls and polls, which are sent by the bot"
  leaves open which polls beyond the bot's own can produce an update, and a restart between
  a send and its posting fact leaves a real poll with no stored reference. Both cases resolve
  the same way: a `Standing` fact whose reference matches no `posted` row is logged and
  dropped. *Named residual:* a crash between `sendPoll` returning and the posting
  observation being appended leaves a poll live in the group that this system cannot close or
  record; an administrator closes it in the client, and its `open_period` closes it anyway.
  *Rejected:* creating a poll record from an unmatched tally (it would invent a poll nobody
  asked for and could be produced by a stranger's stopped poll).

- **An undelivered ask is not recovered after a restart, and the design makes that
  harmless, 2026-08-25.** The outbound edge treats what is stored when it is taken as
  history (`outbound.rs:11-22`, `:160-165`), so an ask filed by the tool and not yet yielded
  when the process dies is never sent. It is tempting to re-offer, at edge seeding, any ask
  carrying no `poll_state` row. That is refused: the same condition also describes the crash
  window above, where the poll *was* posted and only its confirmation was lost, so
  re-offering would post a second identical poll into the group with no way to tell the two
  apart. Instead the ask projects nothing until the platform confirms it, so the model never
  claims a poll that does not exist, and the tool result says the ask is filed and not yet
  posted. The administrator sees no poll and asks again, which is the same recovery as any
  other lost message. *Rejected:* the seeding recovery, as above; *rejected:* an idempotency
  token on `sendPoll` (the API offers none for this method).

## The unit's contract

An administrator — the platform status decision 0015 maps to `Authority::Moderator` — can
ask the assistant to put a question to the group, and the assistant posts an anonymous
poll, regular or quiz, through the `post_poll` tool, admitted only when the turn's whole
provenance reaches that authority and refused in a group-less conversation. The ask is
recorded as a `poll` block that projects nothing; the outbound edge delivers it as a typed
item ahead of the turn's answer text, and the platform's answer comes back through the
observation surface as a `poll_state` row that projects instead — the question and options
when the poll was posted, the refusal when it was not, the counts when it closed. Closing
is a second ask, a `poll_close` block naming a poll the model can see and carrying the
references the edge needs, so `stopPoll` is issued and its returned tally is what gets
recorded. At most one posting fact and at most one closing fact exist per ask, the first of
each winning, and a running count is refused in the core before any store work. Every poll
carries a duration, so a forgotten one closes itself. No vote is attributable to anyone at
any point: the core has no anonymity field, `poll_answer` is not subscribed and could not
arrive for an anonymous poll, running counts are never stored, and a closed poll's counts
project as an aggregate. A poll fact never opens a turn, never reaches a moderation path,
and never becomes an input to anything that touches a person; the assistant states a result
when asked and says the result is unknown when a poll closed without one arriving. The
asking member's erasure nulls the question, the options and the note. A member's own poll
remains invisible, named as such in the skip and in the teaching. The core gains no
platform vocabulary — bounds, wording and the decision to post are all its own — the
adapter gains no decision, and the privacy documents are updated in the same commit,
including every place that counts the recorded gaps or lists the bounded system-voice
surfaces, so nothing they state becomes false.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt and doc under denied
  warnings; the platform-vocabulary scan and the secret scan clean; no new dependency.
- **AC2** `CONSUMED_UPDATE_TYPES` is exactly `["message", "edited_message", "my_chat_member",
  "poll"]` — pinned by an assertion naming `"poll_answer"` as deliberately absent, with the
  anonymity reasoning in the comment.
- **AC3** The wire shape is pinned against the fixture server, with real `sendPoll` and
  `stopPoll` handlers added — the fixture answers an unknown method with
  `{"ok":true,"result":true}` (`tests/adapter/server.rs:450`), so a test without them proves
  nothing. `sendPoll` carries `chat_id`, `question`, `options` as an array of objects with a
  `text` field, `open_period`, and for a quiz `type: "quiz"`, `correct_option_ids` **as an
  array** and `explanation`; it carries no `is_anonymous`, no `parse_mode` of any kind, no
  `correct_option_id` singular, and none of the unmodelled switches. `stopPoll` carries
  `chat_id` and `message_id` only.
- **AC4** No anonymity switch exists, checked two ways because the property is a negative:
  the wire assertion of AC3 pins that no `is_anonymous` key is serialized, and a source scan
  over `crates/core` finds no identifier matching `anonym` in any poll type, tool schema or
  stored column. The scan is a proxy for the broader property and the test says so in a
  comment, so a later edit that adds such a field fails here and is forced to re-read this
  unit.
- **AC5** Authority is enforced through the existing admission path at `Authority::Moderator`:
  `post_poll` and `close_poll` are admitted for a turn summoned by a member whose stored
  authority is `Moderator`, admitted for one summoned by `Admin`, declined for a
  member-summoned turn, and declined for a moderator's turn that absorbed a member's
  own-debt message — each pinned, including the declined case's recorded reading and its
  no-retry wording. The admitted moderator case is pinned explicitly so the feature's
  reachability is checked and not assumed.
- **AC6** Both tools decline in a direct-channel conversation with the group-only reason and
  issue no request — pinned, mirroring the report tool's own group-only test.
- **AC7** Every bound refuses whole and never truncates, and the bounds are measured in
  characters: a 151-character question, a 51-character option, a 101-character note, a note
  containing one line feed, a 1-option list, a 13-option list, a 299-second duration and a
  604801-second duration each decline with the field named, and no `sendPoll` is issued.
  Additionally pinned: a 150-character question composed entirely of non-BMP characters is
  accepted by the core and its serialized `question` is exactly 300 UTF-16 code units, which
  is the platform's cap — the arithmetic that makes the bound's guarantee true instead of
  merely asserted.
- **AC8** The round trip is pinned end to end against the fixture server: a moderator's ask
  files a `poll` block that projects nothing, the outbound edge issues `sendPoll`, the
  returned `Message.poll.id` and `Message.message_id` arrive as a `posted` `poll_state` which
  projects the mark, question and options, `close_poll` files a `poll_close` block, the edge
  issues `stopPoll`, and the response's counts are appended as a `closed` `poll_state` which
  projects the counts. Ordering is pinned too: the poll request reaches the fixture before
  the turn's answer text.
- **AC9** At most one fact of each sort per ask: a second `posted` fact for the same ask
  appends nothing, and a second `closed` fact — whether byte-identical or carrying a
  different count, which is the case that actually arises when `stopPoll`'s response and a
  `poll` update race — appends nothing, and both log. Pinned with differing counts, not only
  with identical ones.
- **AC10** The failure paths are pinned: a `sendPoll` that the fixture refuses produces an
  `unsent` `poll_state` whose projection names the poll and the reason, and no `posted` row;
  a `stopPoll` that the fixture refuses produces an `unsent` fact against the close ask and
  leaves the poll open; a `poll` update with `is_closed: false` appends nothing, logs at
  debug and is refused before any store read; a closing update for a reference no `posted`
  row names appends nothing and logs; a closing update for a poll in a group carrying no
  authorization row answers `Withdraw` **carrying that group's channel key**, appends
  nothing, and the adapter leaves that chat. The last case is constructed as a
  never-authorized fixture, because no code path removes an authorization row
  (`authorization.rs:46`, `:65`).
- **AC11** Projection and frontier behaviour are pinned: `poll` and `poll_close` project
  nothing; a `posted` `poll_state` projects the mark, question and options in the system
  voice; an `unsent` one projects the reason; a `closed` one projects the counts under
  supersession wording. All three kinds are frontier-transparent — a poll fact appended over
  an unanswered message leaves the debt owing — and no poll fact opens a turn, pinned as no
  dispatch on an observation-only ledger. All three classify as `ChainStep::Extends` in
  `provenance.rs`, each by its own named arm.
- **AC12** Erasure reaches the poll record: erasing the asking member nulls the question, the
  options and the answer note on the `poll` block and the copied question on its
  `poll_state` rows, leaves the references and the counts intact, and the nulled block
  projects the erased marker while remaining nameable by `close_poll` — pinned.
- **AC13** A member's poll message records nothing and reports `Skip::PollMessage` — pinned,
  with the message carrying a poll object and no text.
- **AC14** The teaching carries the four sentences written verbatim in this unit's teaching
  decision above, pinned by exact string equality in both answering modes, plus the
  declined-request rule: when a poll request is declined for authority, the assistant says so
  and asks the administrator to send the request again.
- **AC15** The documentation edits are made in the same commit and pinned in
  `crates/assistant/tests/docs.rs` the way every prior unit's edits are: the policy's poll
  sentence; the record of processing's new data-category row; the record's second recorded
  gap widened to name poll text beside note text at `records-of-processing.md:126`; its
  system-voice safeguard row at `:150` restated to name three bounded surfaces, with the
  poll question marked as one the assistant writes and the group does not control; the
  legitimate-interest assessment's new safeguard; the impact assessment's R6 paragraph at
  `dpia.md:449` widened the same way as the record's gap 2; a dated addendum to the impact
  assessment with the review trigger "a non-anonymous poll, or any use of a poll result as an
  input to a path that touches a person"; and this unit's decision records. The two places
  that state the gap **count** — `records-of-processing.md:154` and `dpia.md:554` — are
  checked and left unchanged, because the count stays two: the poll record is reached by
  erasure, and its third-person residual is the second gap's own subject.

## Notes for launch

- Branches from `main`; builds against the current agent-ledger checkout with no framework
  change — every primitive this unit needs is already there.
- Core sites: new module `crates/core/src/poll.rs` for the three kinds, their bounds, their
  erasure pass and their projection, modelled on `note.rs` for the refuse-whole discipline
  and on `tools/report.rs` for the erasure key; `crates/core/src/tools/poll.rs` for
  `post_poll` and `close_poll`, modelled on `tools/report.rs` including its own ledger load,
  its filing mutex, its group-only refusal and the erasure fence handle;
  `kind.rs:1118-1134` gains three `AssistantKind` variants; `provenance.rs:88-94` gains
  three arms in the exhaustive span match and `provenance.rs:226-249` gains three named
  `ChainStep::Extends` arms; `schema.rs` gains three appended migration steps after
  `LITERAL_ADDRESSED_MIGRATION`, with the outcome column's frozen vocabulary in a CHECK and
  an index on the reference column, plus their entries in `store_config()` at `:373-396`;
  `message.rs:218-286` gains `ObservationInput` and `PollObservation`, `ObserveOutcome::Withdraw`
  gains its channel field, and `:373` gains the `Outbound` enum; `assembly.rs:928` splits
  `observe` on the input and `:446` registers the two tools at `Authority::Moderator`;
  `outbound.rs:322-336` and `:478-501` gain the poll deliverables and the channel type
  widens from `OutboundReply` to `Outbound` through `spawn_edge` and `replies`; the erasure
  operation composes the new pass beside `erase_reported_origin`; `teaching.rs` gains the
  poll rules.
- Adapter sites: `client.rs:103` for the update selection; new `send_poll`/`stop_poll`
  returning the decoded `Message`/`Poll` instead of discarding it as `:459` does now — the
  poll reference is `Message.poll.id` on the `sendPoll` response and the message origin is
  `Message.message_id`, and a response whose `poll` field is absent is reported as `Unsent`
  with that as the reason, because a poll whose reference is unknown can never be closed or
  matched; `Update` at `:109-123` gains `poll`, `Incoming` at `:125-150` gains `poll`;
  `translate.rs:120-193` gains the poll-message skip and the poll-update translation;
  `driver.rs:479` routes the poll observation without calling `chat_id_of` or `first_contact`
  — a poll observation has no chat to look up and its own resolution happens in the core, so
  it takes its own arm ahead of the existing chat-keyed path — and acts on the channel the
  `Withdraw` outcome now carries; `driver.rs:730` matches the outbound item and reports each
  send's outcome back through the observation surface, which means `consume_replies` gains
  the assistant handle it does not have today.
- Documentation sites, exactly: `docs/privacy/bot-assistant-privacy-policy.md` under
  Processing/Messages and the purpose line; `docs/privacy/records-of-processing.md` section 5
  (a new row for the poll record: the question and options, which can name a person the way a
  pinned rules text can, plus the platform's poll identifiers and the closing counts, which
  identify nobody), the gap-2 wording at `:126`, and the safeguard row at `:150`;
  `docs/privacy/lia.md` section 7 as a ninth safeguard (anonymous polls only, no per-vote
  data, no path from a count to an effect); `docs/privacy/dpia.md` at `:449` and as a dated
  addendum in the established form; `docs/compliance/ai-act.md` gains one sentence recording
  that no new claim is made — the poll adds no new interaction shape beyond the disclosure
  that already stands, and takes no decision about any person.
- Two things this unit deliberately leaves for a later one, so the next author does not
  re-derive them: poll media, which is the first feature in this area that moves bytes and
  must stream them from the platform to disk and from disk into the request without
  buffering; and the approval surface, which is the precondition for any poll result ever
  reaching a decision about a person, using the framework's existing
  `ApprovalRequest`/`ApprovalDecision` blocks instead of a new mechanism.
- One observation about a neighbouring spec, recorded here instead of edited there: unit
  `telegram/03-editing-messages.md` and this unit both need the identifier of a message the
  assistant sent, which `client.rs:459` currently discards. Whichever merges first should
  make `send_body` return the decoded `Message` and the other should use it, instead of two
  private paths appearing beside each other.
