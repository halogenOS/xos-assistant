# Unit T4 — deleting messages

Date: 2026-08-25 (revised the same day against two independent reviews). The feature most
people expect from this title cannot be built: the platform never tells a bot that a message
was deleted in a group. There is no update for it, and no method to ask. Everything else in
this unit follows from that single fact — what the assistant may delete (only its own
messages), what it must never be given the right to delete (anyone else's), and what
"deleted" can mean at all to a store that appends and never rewrites. The one capability this
unit does ship is narrow: an administrator replying with the deletion command to one of the
assistant's OWN messages makes the assistant take that message back from the chat and record
the retraction on the ledger.

**What the first draft of this unit got wrong.** Two reviews read it against the live API and
the tree and found three structural faults, each of which made a stated criterion
unreachable. They are corrected here, and the corrections changed the design, not only the
wording:

1. **Storage shape.** The first draft stored one delivery as one ordered list in one column,
   copying the tool palette's table. The retraction has to run the lookup the other way —
   from one replied-to chunk to every chunk of the same answer — and that shape has no key
   for it, so the only way through would have been a walk over every delivery row of the
   conversation on ingestion's hot path, the exact cost the tree already refuses in writing
   (`kind.rs:843-849`). One block per delivered message, keyed by origin and by delivery,
   replaces it.
2. **The command stamp.** The first draft stamped the deletion command as a command only when
   a target resolved. Every reply to the assistant arrives addressed (`translate.rs:170-178`),
   so in each case where nothing resolves the message would have summoned a model turn and put
   an answer on the wire — the opposite of the silence the unit claims. Recognition now reads
   the message alone, exactly as decision 0083 already requires, and the effect is decided
   after it.
3. **Coverage of the receipt.** The first draft put the receipt in the outbound reply consumer
   only. Deterministic messages — the rules acknowledgment, the privacy commands' answers —
   leave through a different function (`driver.rs:438-441`), so that whole class of the
   assistant's own messages would have been unretractable while the contract claimed
   otherwise. Every message the assistant sends now records its delivery.

The feature is buildable. Nothing in the corrected design is blocked on the platform beyond
the two limits named below: no deletion update exists, and a message older than 48 hours
cannot be taken back.

## Grounding

Verified against the live Bot API documentation (page fetched 2026-08-25) and against the
tree at `1891fcd`. Line anchors were re-checked one by one; the first draft's anchors had
drifted and several pointed into test modules.

**The platform, `https://core.telegram.org/bots/api`.**

- **No deletion update exists for an ordinary chat.** The `Update` object carries **27**
  optional fields (28 rows, `update_id` included), at most one per update: `message`,
  `edited_message`, `channel_post`, `edited_channel_post`, `business_connection`,
  `business_message`, `edited_business_message`, `deleted_business_messages`, `guest_message`,
  `message_reaction`, `message_reaction_count`, `inline_query`, `chosen_inline_result`,
  `callback_query`, `shipping_query`, `pre_checkout_query`, `purchased_paid_media`, `poll`,
  `poll_answer`, `my_chat_member`, `chat_member`, `chat_join_request`, `chat_boost`,
  `removed_chat_boost`, `managed_bot`, `subscription`, `stopped_message_generation`. Exactly
  one of them reports a deletion, `deleted_business_messages`, carrying
  `BusinessMessagesDeleted { business_connection_id, chat, message_ids }`, documented as
  "received when messages are deleted from a connected business account". A community
  supergroup is not a connected business account, so that update never arrives here.
- **There is no read-back either.** The API has no `getMessage` and no way to ask whether a
  message id still exists. `forwardMessage` and `copyMessage` would fail on a deleted
  message, but both SEND a message to do the asking, so neither is a probe.
- **`deleteMessage(chat_id, message_id)`** returns `True` and states its limitations
  verbatim: a message can only be deleted if it was sent **less than 48 hours ago**; service
  messages about a supergroup, channel or forum-topic creation cannot be deleted; a dice
  message in a private chat can only be deleted if it was sent MORE than 24 hours ago; **bots
  can delete outgoing messages in private chats, groups, and supergroups**; bots can delete
  incoming messages in private chats; bots granted `can_post_messages` can delete outgoing
  messages in channels; **if the bot is an administrator of a group, it can delete any message
  there**; a bot with the **`can_delete_messages`** administrator right in a supergroup or
  channel **can delete any message there**; a bot with `can_manage_direct_messages` can delete
  any message in a channel's direct-messages chat. It states no skip clause: an id the
  platform cannot find is a refusal.
- **`deleteMessages(chat_id, message_ids)`** takes "a JSON-serialized list of **1-100**
  identifiers of messages to delete", refers back to `deleteMessage` for the limitations, and
  states verbatim: "If some of the specified messages can't be found, they are skipped.
  Returns True on success." The lower bound of the range is one, so a single-message deletion
  is a legal call.
- **Inbound reactions need administrator rights.** `message_reaction` and
  `message_reaction_count` both state "The bot must be an administrator in the chat" to be
  received at all. This unit decides the bot is never made an administrator, which forecloses
  inbound reaction updates for the sibling unit on reactions; that unit is not edited here,
  and the note below records the constraint for whoever writes it.
- **Version.** The changelog's newest entry is **Bot API 10.3, 24 August 2026**; 10.2 (14 July
  2026) added `deleteEphemeralMessage`, and 10.1 is 11 June 2026. Nothing in 10.1, 10.2 or
  10.3 adds a deletion update for ordinary chats. (The brief for this unit named 10.1 as
  current; it is two releases behind.)

**Our tree.**

- The deletion mirror already exists and already reads the administrator's command:
  `mirror::mirrored_target` (`crates/core/src/mirror.rs:58-70`) returns the replied-to origin
  when the reported command is `DELETION_COMMAND` (`/del`, `mirror.rs:34`), the channel is a
  group, and the sender's resolved standing is at or above `ADMINISTRATOR_FLOOR`
  (`Authority::Moderator`, `mirror.rs:41`). It is wired into ingestion at
  `assembly.rs:687-691`, ahead of the owing-tail read at `:692`, and nulls one row through
  `kind::erase_message_named` (`kind.rs:743-786`).
- **The one case the mirror deliberately drops is the assistant's own message.**
  `ReplyTarget::AssistantMessage => None` (`mirror.rs:68`), because the assistant's own words
  are no person's row. That arm is where this unit's capability belongs. (The first draft
  cited `mirror.rs:81`; line 81 is inside the test module, which opens at `mirror.rs:72`.)
- **Every reply to the assistant arrives addressed.** `addressed` is
  `mentions_bot(...) || replies_to_bot(message, me) || names_bot(...)`
  (`translate.rs:170-178`), and `replies_to_bot` compares `author.id == me.id`
  (`translate.rs:440-446`). `resolved_summons` then sets `summoned: message.addressed || ...`
  (`assembly.rs:1244-1249`), and `Stamp::compose` opens a debt for a summoned message that no
  budget refused (`kind.rs:329-340`). So today a `/del` replying to one of the assistant's own
  messages summons a model turn, and only the command stamp at `assembly.rs:694` prevents one.
  The existing spine test hides this by setting the flag the adapter cannot produce:
  `crates/core/tests/spine/mirror.rs:272` — `to_assistant.addressed = false;`.
- **Every command is recorded, including every no-op.** The comment at `assembly.rs:684-686`
  says so ("the command row appended below is the lawful record of the request") and the spine
  test asserts it: `crates/core/tests/spine/mirror.rs:295` counts "the target and four
  recorded commands". No `/del` leaves the ledger untouched; only its effect varies.
- **The reply to an assistant message carries no identifier at all.** `reply_target_of`
  (`translate.rs:454-462`) returns `ReplyTarget::AssistantMessage` — a unit variant
  (`message.rs:163-167`) — for a reply whose author is the bot, and
  `ReplyTarget::Message { origin }` for everyone else's, spelled with `id.to_string()`. The
  replied-to id is itself `Option<i64>` on the wire (`client.rs:199-202`). So today the core
  cannot tell WHICH of its own messages an administrator pointed at.
- **The reply target is persisted, and the column's own documentation excludes this case.**
  `ChatMessage::stored_fields` (`kind.rs:444-470`) writes `COLUMN_REPLY_TARGET` for the
  `Message` variant and `COLUMN_REPLY_TO_ASSISTANT = true` for `AssistantMessage`, writing no
  target for it. The column's doc (`kind.rs:131-151`) states verbatim that it is "NULL for a
  non-reply, for a reply without a usable id, and for a reply to one of the assistant's own
  messages", and classifies it as "Personal data of TWO people at once";
  `erase_reply_targets_naming` (`kind.rs:799-822`) joins erasure against it.
- **The report tool does not read the reply target at all.** Its self-report refusal resolves
  the origin the MODEL passed as a tool argument against the stored row and checks the stored
  role: "the resolved row must not be the assistant's own voice (the assistant holds no
  principal row, so 'the assistant's own' is the stored role fact)" (`report.rs:283-285`,
  refusal string at `report.rs:238`). The first draft claimed the refusal "matches the
  VARIANT"; that is false, and it was offered as the safety argument for widening the variant.
  The conclusion still holds, for this reason instead.
- **The adapter throws away the identifiers of the messages it sends.**
  `BotClient::send_body` decodes the answer into `let _sent: serde_json::Value`
  (`client.rs:459`) and returns `Ok(())`. `send_message` (`client.rs:371-390`) loops over
  `chunks_within_cap` — 4096 UTF-16 units per chunk (`client.rs:34`, `client.rs:599`) — so one
  answer can already be several platform messages, and none of their ids survives the call.
- **A cut-short send reports a count, not ids.** `SendError { delivered_chunks: usize, error }`
  (`client.rs:88-97`) is returned at `client.rs:384-387` and read by the reply consumer's two
  log lines (`driver.rs:745-758`). A receipt of what actually reached the chat cannot travel on
  it as it stands.
- **The assistant's own messages leave through TWO paths.** Outbound answers, report lines and
  the failure notice ride `OutboundReply` into `consume_replies` (`driver.rs:730-760`, spawned
  in the `tokio::select!` at `driver.rs:282-286`, which holds no `Assistant` handle;
  `Arc<Assistant>` is in scope at `driver.rs:257-260`). Deterministic items — the rules
  acknowledgment and the privacy commands' answers (`message.rs:252-259`) — ride the ingestion
  call's return and are sent by `send_item` (`driver.rs:601-606`) from `driver.rs:438-441`.
  Both put messages in the chat.
- **The outbound reply is built at one site for two of its kinds.** `deliverable_of`
  (`outbound.rs:478-496`) produces `ReplyKind::Answer` for a finalized assistant text block and
  `ReplyKind::Report` for a deliverable report; both are turned into `OutboundReply` at
  `outbound.rs:365-370`. The failure notice is built separately at `outbound.rs:405-412`.
  `deliverable_of` ends in `_ => None`, so a new block kind can never leak onto the outbound
  edge.
- **Deterministic outbound already rides a call's return** (decision 0050):
  `IngestOutcome::Recorded { receipt, deliver }` (`assembly.rs:773-780`), whose receipt already
  carries the conversation id. The withdraw directive is performed the same way
  (`driver.rs:443-446`, `leave` at `driver.rs:612-627`).
- **A block kind may project nothing.** `impl Projection for Report {}` (`report.rs:161`) takes
  the default and shows the model nothing; reports are appended with role `None`
  (`report.rs:394-402`), as context notes are (`note.rs:521-527`).
- **A block's content is exactly one row.** The kind descriptor names one table and its
  columns (`report.rs:117-121`, `palette.rs:74-77`), and the store hydrates that row into the
  block's fields. A one-to-many content table is outside that contract.
- **An origin-keyed lookup is an established, bounded query.** `erase_message_named`
  (`kind.rs:743-786`) matches `COLUMN_ORIGIN` within one conversation by joining
  `conversation_blocks`, because "platform message ids are opaque and unique only per channel"
  (`kind.rs:791-798`). The opposite shape is refused in writing: "a row-by-row walk would
  stretch with the run's length into a conversation hydration on ingestion's hot path"
  (`kind.rs:843-849`).
- **Appending a non-message block into a conversation is a solved seam.** `DEBT_READ_THROUGH`
  (`assembly.rs:59-63`) lists the kinds the owing-tail walk reads through — context notes, the
  tool palette, reports — each "appended by an independent path at an arbitrary moment", and
  `kind::newest_block_id_past_erased` (`kind.rs:843+`) implements the walk past them.
- **Adding a table is an appended migration step.** `store_config` (`schema.rs:372-395`) lists
  the three creating steps and then every appended step in order, per decision 0026; the report
  table (`schema.rs:296-310`) and the palette table (`schema.rs:180-192`) are the two nearest
  precedents, and `PROTECTION_STAMP_MIGRATION` (`schema.rs:160-172`) shows an index created in
  the same step.
- **The framework has a `detach_block`**
  (`agent-ledger/crates/agent-ledger/src/store/conversations.rs:359-372`) that removes a
  block's membership of a conversation. Its own documentation calls it "deliberately narrow",
  built so a fork can differ from its source, and warns that the block itself is never removed.
  Erasure calls `store.gc_orphan_blocks()` in its **third** step (`erasure.rs:154`, after the
  reply-target scrub at `:140`, the content nulls at `:141`, the report pass at `:148` and the
  per-conversation deletes at `:151-153`; the module doc names "three idempotent steps" at
  `erasure.rs:3`). The first draft cited step 2.
- **Erasure nulls, it does not remove.** `kind::erase_principal_content` (`kind.rs:688-742`)
  sets text, origin, send time, reply target and speaker to NULL on the person's rows;
  `erase_reply_targets_naming` (`kind.rs:799-822`) nulls the copies other people's rows hold.
  Block headers are never touched.
- **The wire client reduces a failure status to its code.** `BotClient::decode`
  (`client.rs:561-585`) returns `ClientError::Status { status }` for any non-success HTTP
  status and only decodes `description` from a 200 answer carrying `ok: false`. The Bot API
  answers "message to delete not found" with HTTP 400, so the reason will not be visible until
  the recorded follow-up ("the wire client discards error response bodies on non-success
  status", `docs/follow-ups.md:12-17`) is resolved.
- **The vocabulary scan proves less than the first draft claimed.** `vocabulary.rs:68-90`
  matches whole alphanumeric runs, case-insensitively, against `docs/platform-vocabulary.txt`,
  over `crates/core` only (`vocabulary.rs:34-45`). The file's entire content is seven platform
  and SDK names; no API method name is on it, so running the scan green says nothing about
  method names. The file's own header invites adapters to grow it.
- **The token scan owns its own test binary.** `crates/adapters/telegram/tests/token_scan.rs`
  installs a process-wide capture subscriber and states in its header why it "shares its
  process with nothing else". A claim about how many lines something logs can only be ruled
  inside that binary.
- **The stub server exists and scripts sends.** `crates/adapters/telegram/tests/adapter/server.rs`
  binds a loopback listener per test (`:105-130`) and routes `sendMessage` (`:398`) through a
  scripted outcome queue (`:41`).
- **The published documents already speak on this.** The privacy policy states "Deleting a
  message in your chat app does not reach us" and names the mirror as the one exception
  (`docs/privacy/bot-assistant-privacy-policy.md:113-121`). It does NOT state the converse —
  that an erasure request does not remove anything from the group's chat. The DPIA carries the
  mirror at `docs/privacy/dpia.md:638-654` and its storage-and-deletion section at §3.6
  (`dpia.md:255-268`). The operator contract explains the mirror and its bounds at
  `docs/reference/group-operator-contract.md:137-158`. The docs test that pins them is
  `crates/assistant/tests/docs.rs:715-760`, which asserts exact substrings. Decision 0070
  (`docs/decisions/0070-the-assistant-assesses-a-human-decides.md`) forbids any moderation
  effect without a human decision point in the mechanism, and names "any future administrative
  tool" as shipping only behind human approval. Decision 0083
  (`docs/decisions/0083-non-administrators-deletion-commands-mirror-nothing.md`) settles which
  `/del` shapes are recognized, and states the principle this unit leans on: recognition
  "reads the message alone and never the store".
- **The command token's own documentation calls the assistant's part bookkeeping.**
  `mirror.rs:27-33`: "The assistant piggybacks on this token instead of owning a command: the
  admin asks the moderation bot, and the assistant's part is bookkeeping." The operator
  contract says the same at `group-operator-contract.md:139-146`. After this unit the
  assistant performs a platform action on that token, so both statements need changing.

## Decisions taken with this unit

- **The store cannot follow the chat, and this unit does not pretend otherwise, 2026-08-25.**
  A member deleting their own message, an administrator deleting one through the client, and
  the moderation bot deleting one through its own API call all produce **nothing the assistant
  can observe** — the `Update` list above has no field for it. The reply-command mirror of unit
  13 works because it reads a COMMAND, not a deletion, and that stays the only path. This unit
  therefore adds no reconciliation of any kind and the published bound stays exactly as
  written. *Rejected:* polling for existence (there is no `getMessage`, and a
  `forwardMessage`/`copyMessage` probe sends a message to ask a question — abusive at group
  scale and useless besides, since the copy would have to go somewhere); opening a Telegram
  Business connection to obtain `deleted_business_messages` (a group is not a business account;
  the update is scoped to the connected account's own chats, it would restructure who the data
  flows through, and it would still not see a member's own client-side deletion in a group);
  asking the moderation bot to notify us out-of-band (a second system's promise we do not
  control, and it still cannot see a member's own client-side deletion).
- **The assistant deletes only messages it sent itself, and is never made an administrator,
  2026-08-25.** `deleteMessage`'s own text is the fork in the road: "bots can delete outgoing
  messages in private chats, groups, and supergroups" needs no right at all, while deleting
  anyone else's message needs the bot to be an administrator, or to hold `can_delete_messages`
  in a supergroup. The second is a standing capability over a person's words — precisely the
  shape decision 0070 refuses — and it duplicates the moderation bot, which already holds that
  right and already answers to the administrators. The bound lives in the code and not in the
  permission: the origins the core may name for deletion come only from its own recorded
  deliveries, so promoting the bot to administrator for some unrelated reason still cannot turn
  into power over a member. *Rejected:* granting `can_delete_messages` and deleting a member's
  message on an administrator's command (the administrator would satisfy 0070's human decision
  point, but the capability is standing, it fires the impact assessment's review trigger for
  standing-touching capabilities, and it re-implements the moderation bot inside a bot whose
  whole posture is bystander); an assistant-side "delete this" the model can call (the model
  deciding to remove a person's message is the exact prohibition).
- **The administrator's `/del` on an assistant message is a human decision point, and the
  assistant's act is never wider than what was asked, 2026-08-25.** One review asked the fair
  question: the token belongs to the moderation bot, so does the assistant infer consent for
  its own act from a command aimed elsewhere? Three facts answer it. The act touches no
  person: the only message deleted is one the assistant itself wrote, so there is no moderation
  effect on anybody for 0070 to protect. The command is unambiguous in its own vocabulary — an
  administrator replying `/del` to a message is asking for that message to go — and the
  administrator is the human decision point the mechanism requires. And the moderation bot,
  which holds `can_delete_messages`, can act on the same command against the same message
  anyway, so the assistant's deletion is at most a duplicate of a deletion the administrator
  already commanded, never a broader one. What does change is the framing: the assistant's part
  is no longer only bookkeeping, and the sentence that says so must change with it, in the code
  comment and in the operator contract. *Rejected:* a separate consent step (an administrator
  answering a bot's "shall I really?" for the bot's own message is noise, and the silence rule
  of decision 0082 exists to avoid exactly that); treating the assistant's own message as
  outside the administrator's reach (the administrators run the group; a bot that refuses to
  take back its own message on their command is worse behaved than one that does).
- **A retraction is an appended fact; the answer it retracts keeps its text and its place,
  2026-08-25.** The ledger appends and never rewrites, so "the assistant took that message
  back" is a NEW block that supersedes, not an edit of the old one. The retracted answer stays
  in the conversation and stays projected: it was said, the group read it, and the honest
  record is "this was said, then taken back". *Rejected:* nulling the answer's text the way the
  mirror nulls a person's row (the nulling path exists for personal data in a table of its own,
  which is the carve-out decision 0003 bought so that a person's rights never break the
  append-only rule; the assistant's own prose is not that, and reusing the erasure path for the
  assistant's convenience would blur the one distinction the whole storage design rests on);
  `detach_block` (the framework's fork primitive, whose own documentation says it removes a
  membership for a fork's sake — here it would leave the answer held by no conversation and the
  next `gc_orphan_blocks` would collect it, which is rewriting history through a seam built for
  something else).
- **Erasure never reaches the chat; retraction never reaches a person's row, 2026-08-25.**
  These are the two halves that must not blur. Erasure nulls stored columns and touches no
  platform message; retraction deletes a platform message the assistant sent and nulls nothing.
  Neither borrows the other's mechanism. *Rejected:* extending a person's erasure to delete
  their messages from the group (it needs administrator rights, so decision 0070 again; the
  48-hour limit would make the promise true only for the newest messages and false for
  everything older, which is worse than not promising; and it removes text other members are
  reading, which is not what a person asking us to delete OUR copy asked for); extending
  retraction to null the retracted answer's stored text (see the previous decision).
- **Every message the assistant sends records its delivery, as one block per delivered
  platform message, 2026-08-25.** It cannot delete its own message without knowing the id, and
  today the id is discarded at `client.rs:459`. After each successful send the adapter reports
  what reached the chat, and the core appends one `Delivered` block per platform message,
  holding two values: the platform's id for that message, and the **delivery key** that ties
  the messages of one send together — the first id of that send, which is already unique
  within the channel and mints no new identity. Every path is covered: outbound answers, report
  lines, the failure notice, the rules acknowledgment and the privacy commands' answers. A
  uniform rule needs no exception list and cannot silently omit a class the way the first draft
  did. Blocks, not a side table, so they cascade with a deleted conversation exactly like every
  other content row and need no cleanup pass of their own; they project nothing and join
  `DEBT_READ_THROUGH`, the seam `assembly.rs:59-63` already describes. *Rejected:* one block
  per send holding the ordered ids in one column, following the tool palette (a block's content
  is one row by the descriptor's contract, and that shape has no key for the lookup the
  retraction actually needs — from one replied-to chunk to all of them — so resolving it would
  read every delivery row of the conversation on ingestion's hot path, the cost `kind.rs:843-849`
  refuses in writing); one block per send plus a second, index-only table beside it (a second
  place for one fact, outside the kind system, for no gain over one block per message); an
  adapter-local map from answer to ids (lost on restart, and it makes the adapter hold state it
  would then have to reason about — behaviour in an adapter); a side table keyed on the answer's
  block id (it does not cascade with a deleted conversation, so erasure would need an extra pass
  for it, and the deterministic items have no answer block to key on); resolving "my latest
  answer" without an id (ambiguous the moment a second answer exists, and wrong exactly when it
  matters).
- **The retraction is keyed on the delivery, not on the chunk, 2026-08-25.** A `Retraction`
  block records one delivery key. An administrator who replies to the third chunk of an answer
  and an administrator who replies to the fifth are asking for the same thing, and a
  chunk-keyed record would treat them as two different retractions — appending a second block
  for one act, and defeating the idempotence this unit claims. Keying on the delivery makes any
  chunk of an answer name the whole answer. *Rejected:* recording the replied-to origin alone
  (two records for one act on a chunked answer, and no way to ask whether the answer was
  already taken back).
- **The reply-to-assistant target carries the assistant message's origin, and that origin is
  never stored, 2026-08-25.** `ReplyTarget::AssistantMessage` becomes
  `AssistantMessage { origin: Option<String> }`, filled at `translate.rs:454-462` from the same
  `reply_to_message.message_id` the other variant already reads — optional because the wire
  field is optional. The value is consumed during ingestion, before the row is appended, and
  `ChatMessage::stored_fields` keeps writing exactly what it writes today for this variant:
  `COLUMN_REPLY_TO_ASSISTANT = true` and no reply target. The report tool is unaffected, but not
  for the reason the first draft gave: it never reads the reply target at all, and resolves the
  model-supplied origin against the stored role instead (`report.rs:283-285`). *Rejected:*
  storing the origin in `COLUMN_REPLY_TARGET` (its documentation states verbatim that the
  column is NULL for this case and classifies it as two people's personal data, and
  `erase_reply_targets_naming` joins erasure against it — a column that sometimes holds the
  assistant's own id makes both statements false); a third column for it (a stored fact nothing
  reads); keeping the variant empty and adding a parallel field on `InboundMessage` (two ways to
  say one thing); keeping it empty and resolving the target by recency (the wrong message,
  silently).
- **Recognition reads the message; the store decides only the effect, 2026-08-25.** The mirror
  gains one function that classifies the deletion command into an ask — a reply naming a
  person's message, or a reply naming one of the assistant's own — from the message and the
  sender's standing alone. Recognition is what stamps the row `LimitedBy::Command` and takes
  the turn away, and it must not depend on anything the store holds: decision 0083 already
  requires this ("the trigger reads the message alone and never the store"), and without it
  every unresolvable case would summon a model turn, because a reply to the assistant always
  arrives addressed. The effect — which row is nulled, which delivery is retracted, or nothing
  at all — is decided afterwards. This **amends decision 0083** in one respect and the
  amendment is deliberate: 0083 records that an administrator's `/del` replying to the
  assistant's own message "mirrors nothing" and records as an ordinary message. It now has an
  effect, so it is a recognized command and records with the command stamp, exactly as 0083's
  own reasoning would have it. A NON-administrator's `/del` is unchanged in every case: not
  recognized, recorded as an ordinary message, and — when it replies to the assistant — still
  addressed and still answered, exactly as today. *Rejected:* keeping the stamp tied to a
  resolved target (silent on the wire only when the store happened to hold a delivery, chatty
  otherwise — two behaviours for one command, and the first draft's own criteria contradicted
  each other over it); stamping every `/del` from anyone (it would silence a member's ordinary
  message to the assistant, which 0083 deliberately keeps ordinary).
- **The retraction rides the ingestion call's return, 2026-08-25.** It is deterministic,
  immediate and takes no model turn, so decision 0050 applies as written: `DeliveryItem` gains a
  `Retraction { origins: Vec<String> }` variant and `DeliveryItem::text` becomes
  `Option<&str>`, with the driver matching on the variant instead of calling `.text()` blindly.
  The two are mutually exclusive by construction — the deletion command is not a privacy
  command, and the privacy family is the only source of the other variants — so one resolution
  point produces one item. The adapter performs it; it decides nothing. *Rejected:* a second
  outbound subscription for retractions (a whole async path for an act that is already known by
  the time `ingest` returns); adding a `retract: Vec<String>` field beside `deliver` on
  `IngestOutcome::Recorded` (a bolted-on second channel for the same "here is what to do on the
  chat" idea — the enum is the structure that already accepts it).
- **One command, two effects, decided by what the reply names, 2026-08-25.** The trigger stays
  `DELETION_COMMAND` (`/del`) under the same three conditions the mirror already applies —
  group channel, administrator floor, reported invoked command. A reply naming a person's
  message erases that row (unit 13, unchanged); a reply naming one of the assistant's messages
  retracts that delivery. Administrators learn one command, and the operator contract's existing
  bounds carry over unchanged, `/del@othermoderationbot` included: a token aimed at another bot
  by name is not an invocation here and does nothing. *Rejected:* a separate `/retract` command
  owned by the assistant (a second vocabulary for one human intent, and a command the moderation
  bot's administrators would have to be taught separately).
- **A delivery is retracted whole, through `deleteMessages` in batches of at most 100, even for
  a single message, 2026-08-25.** One answer over 4096 UTF-16 units is already several platform
  messages (`client.rs:34`, decision 0019), and taking back only the first would leave the group
  reading the remainder of a retracted answer. All the recorded origins of the delivery are
  therefore deleted. One method does it for every case: `deleteMessages` accepts "1-100"
  identifiers, so a single-message delivery is a legal one-element call. That matters beyond
  tidiness — `deleteMessages` skips ids it cannot find and still succeeds, while `deleteMessage`
  refuses with a 400. Since the most common way an id goes missing is the moderation bot
  deleting the same message on the same command a moment earlier, choosing per size would make
  the one-message case a logged failure and the two-message case a silent success for the same
  event. Past 100 the adapter issues successive calls, walking the recorded origins in batches
  without building any larger request. *Rejected:* `deleteMessage` for a single origin
  (a branch that buys nothing and splits the behaviour of one act along an accident of length);
  deleting only the first chunk (leaves a truncated answer standing).
- **A repeat command re-issues the call but appends no second block, 2026-08-25.** The two
  halves of idempotence are not the same half. On the ledger, a delivery already carrying a
  `Retraction` gets no second one: the recorded fact is that the administrator asked for this
  delivery to go, and asking twice is one fact. On the wire the call is issued again, because
  the first one may have failed — a spent rate-limit ceiling, a dropped connection — and an
  administrator who sees the message still standing and types the command again is telling us
  the first attempt did not work. Re-issuing costs one call that either succeeds or is skipped
  as not-found; refusing to re-issue would make one dropped request permanently foreclose the
  retraction while the ledger claimed success. *Rejected:* no second call ever (the first
  draft's rule: one transient failure becomes permanent, and the administrator's retry is
  documented as a no-op); recording success on the block and keying the repeat on it (the block
  is appended during ingestion and the outcome is only known later in the adapter, so this needs
  a second receipt path for a fact nothing else wants); appending a second `Retraction` per
  repeat (a stream of blocks for a repeatedly failing call).
- **Failure is best-effort, logged, and never rewrites the record, 2026-08-25.** The 48-hour
  limit is the platform's, not ours: an answer older than that cannot be taken back. The
  recorded fact is the administrator's ASK, which certainly happened, so the `Retraction` block
  is appended whether or not the platform obeys, and a refusal is logged and dropped exactly as
  a failed send is (`driver.rs:745-758`). The call uses the same `MAX_RATE_LIMIT_WAIT` ceiling
  as the leave directive (`client.rs:348-352`), because it runs inside the sequential update
  batch. *Rejected:* retrying past the client's existing rate-limit contract (a message that
  cannot be deleted will not become deletable within one batch; the administrator's own repeat
  is the retry, per the decision above); appending the retraction only on success (the ledger
  would then record the administrator's ask as never having happened).
- **The retraction shows the model nothing, 2026-08-25.** `Retraction`, like `Delivered`, takes
  the default projection and contributes no message to the model's context, joining
  `DEBT_READ_THROUGH` with it. The first draft projected a fixed line, and two problems came
  with it. The line could not name the answer it retracted, because assistant messages do not
  project their origins — so it would say only that something had been taken back. And it would
  be appended at the tail, next to whatever the newest answer happened to be, which in a live
  conversation is usually a different answer entirely: the model would read a retraction
  attached to the wrong message. A fixed line also has to declare a role, and an assistant-voiced
  line arriving straight after the administrator's command reads as an answer to a command the
  unit deliberately answers with silence. Showing nothing is the honest option: the retracted
  answer keeps projecting, which is true — it was said — and nothing false is added.
  *Rejected:* the unnamed fixed line (says something misleading in exchange for saying
  something); a line naming the answer (needs the assistant's own origins in the projection,
  which is a separate change with its own privacy reading — recorded as a follow-up).
- **The silence rule stands, 2026-08-25.** No reply, no acknowledgment — the mirror's rule
  (unit 13, decision 0082) for the same reason: the administrator issued a command, and a bot
  answering it with prose is noise.
- **The two method names join the core's forbidden vocabulary, 2026-08-25.** The scan over
  `crates/core` checks a word list, and no API method name is on it, so the first draft's claim
  that a green scan proved "no Bot API method name appears outside the adapter's client" was
  empty. Adding `deleteMessage` and `deleteMessages` to `docs/platform-vocabulary.txt` makes the
  core half of that claim real and checkable, which is the half the invariant is about; the
  file's own header invites each adapter to grow it. Neither token can appear in neutral core
  prose, so no false positive is created. Which adapter file names them stays a review matter,
  as it already is for every other method. *Rejected:* leaving the list untouched and dropping
  the claim (cheaper, but it leaves the one enforceable part unenforced); a second scan over the
  adapter crate asserting only one file names the methods (asymmetric — every existing method
  name lives by convention alone — and it pins a file layout instead of an invariant).
- **The documents move, 2026-08-25.** Five edits across four documents, each stated below in
  "Notes for launch", and all of them ship with the code because a published statement that the
  code makes false is a defect, not a follow-up. One code comment moves with them, for the same
  reason.

## The unit's contract

The assistant never learns that anyone deleted a message, because the platform has no such
update for a group, and this unit adds no mechanism that pretends it does. It gains exactly one
deletion capability. After a successful send — of any message it sends, an answer, a report
line, the failure notice, an acknowledgment or a command's answer — it records one `Delivered`
block per platform message that reached the chat, each holding that message's id and the key of
the send it belonged to. When a group administrator replies to any of those messages with the
deletion command, the command is recognized from the message alone: the row records with the
command stamp, no model turn is summoned and nothing is said. If the reply names a recorded
delivery, the assistant appends a `Retraction` block for it unless one already stands, and
deletes every message of that delivery from the chat through `deleteMessages`, in batches of at
most 100. It never deletes a message it did not send, is never granted `can_delete_messages`,
and is never made an administrator. The retracted answer keeps its text, its block and its
place on the ledger; the retraction supersedes it and nothing is rewritten, and neither the
delivery record nor the retraction shows the model anything. A person's erasure still nulls
columns and still reaches no chat; a retraction still deletes a chat message and still nulls
nothing. No new dependency.

## Acceptance criteria

1. Workspace suite green in both modes; clippy, fmt and doc under denied warnings; the secret
   scan clean; no new dependency. `docs/platform-vocabulary.txt` gains `deleteMessage` and
   `deleteMessages`, and the scan over `crates/core` is green with them on the list — the new
   core words (`delivered`, `retraction`, `origin`, `delivery`, `directive`) are neutral and
   name no platform.
2. A successful send records its delivery, pinned against the stub server returning ids: an
   answer delivered as one message appends one `Delivered` block whose origin is that id and
   whose delivery key is the same id; an answer whose text exceeds the chunk cap appends one
   block per chunk, in send order, all carrying the first chunk's id as the delivery key; a
   send that fails on its first chunk appends nothing; a send cut short after some chunks
   appends exactly the blocks for the chunks that reached the chat. The same is pinned for a
   deterministic item — the privacy command's answer — which leaves through the other send
   path.
3. A `Delivered` block and a `Retraction` block each project nothing to the model, are skipped
   by the outbound delivery loop, and are members of `DEBT_READ_THROUGH`: a debt owed behind a
   run of them still owes, pinned exactly as the report kind's membership is pinned.
4. An administrator's reply `/del` to one of the assistant's own messages retracts the whole
   delivery: a `Retraction` block naming the delivery key is appended, and the adapter issues
   `deleteMessages` carrying every origin of that delivery — pinned over the wire for a
   one-message delivery (a one-element call, not `deleteMessage`) and for a chunked one.
   Replying to the third message of a chunked delivery produces the same request as replying to
   the first.
5. A delivery of more than 100 messages is deleted in successive calls of at most 100 ids each,
   with no larger list assembled — pinned from a ledger state built directly by the test
   fixture, holding 101 `Delivered` blocks under one delivery key. The fixture is stated as
   constructed on purpose: reaching 101 chunks through the send path would mean pushing more
   than 400,000 characters through it, which pins the batching no better and the suite far
   slower.
6. The retracted answer is untouched: its stored text, its block header and its position are
   identical before and after, and it still projects to the model, with the projection carrying
   no retraction line anywhere — pinned block by block.
7. The recognized-but-ineffective commands are pinned. In each case the row records with
   `LimitedBy::Command`, summons no model turn, and produces **no `Retraction` block, no nulled
   column and no request on the stub**: an administrator's `/del` replying to an assistant
   message whose id the platform did not supply; one replying to an assistant message whose
   delivery the store never recorded; and — for the ledger half only — one replying to a
   delivery that already carries a `Retraction`, which appends no second block while still
   issuing the delete request, pinned as exactly one further request on the stub. "The ledger
   alone" is deliberately not claimed anywhere: every `/del` appends its command row, as unit
   13 already pins.
8. The unrecognized cases stay unrecognized, pinned: a non-administrator's `/del` replying to an
   assistant message records as an ordinary message with no command stamp, retracts nothing and
   issues no request — and, being a reply to the assistant, still summons a turn exactly as it
   does today; a `/del` with no reply, a direct channel, and `/del` suffixed with another bot's
   handle each retract nothing. Unit 13's own no-ops are re-pinned unchanged, and its spine
   fixture stops setting `addressed = false` by hand, so the pins run against the flag the
   adapter actually produces.
9. A person's message and an assistant message are told apart by one command: the same `/del`
   reply nulls a person's row through the existing mirror and retracts a delivery through the
   new path, and neither ever runs the other's mechanism — pinned by asserting that a
   retraction nulls no column and that a mirror issues no platform request.
10. Failure is contained: a `deleteMessages` refused by the platform (a 400 from the stub, the
    48-hour and already-deleted cases) leaves the `Retraction` block standing, logs a warning,
    drops the directive and lets the update batch continue with the next message — pinned by the
    following update being processed normally. That no log line carries the token is covered by
    the existing token-scan binary, extended to force this failure path; no claim about how many
    lines are emitted is made outside that binary.
11. The two ordering cases are pinned, not assumed. A `/del` naming a message whose
    `Delivered` block has not been appended yet resolves nothing and is one of criterion 7's
    cases — the administrator's repeat, once the block stands, retracts normally. A retraction
    performed while a model turn is in flight on the same conversation leaves that turn alone:
    the turn's answer still delivers, still records its own delivery, and is itself retractable
    afterwards. Unlike the mirror, a retraction nulls nothing, so no in-flight answer can carry
    text a retraction removed — the timing window the impact assessment records for the mirror
    has no counterpart here, and the assessment says so.
12. A direct conversation's deletion during erasure removes its `Delivered` and `Retraction`
    blocks with it, leaving no orphan rows — pinned. A person's erasure leaves both kinds
    untouched, because neither holds anything of theirs — also pinned.
13. The five document changes and the one code-comment change ship, and the docs test asserts a
    named substring for each of the five, in the shape the existing deletion-mirror docs test
    uses.

## Notes for launch

Exact sites, from the reading above. All anchors re-verified at `1891fcd`.

- **Core, message vocabulary** (`crates/core/src/message.rs`): `ReplyTarget::AssistantMessage`
  (`:163-167`) gains `{ origin: Option<String> }`, and its doc comment loses the sentence that
  says no origin rides it; `DeliveryItem` (`:252-259`) gains `Retraction { origins: Vec<String> }`
  and `DeliveryItem::text` (`:261-268`) returns `Option<&str>`; a `DeliveryHandle` type joins the
  module beside `IngestReceipt` — opaque to adapters, naming the conversation the sent message
  belongs to — together with the receipt the adapter reports back.
- **Core, delivery module** (new, `crates/core/src/delivery.rs`): owns both new kinds the way
  `note.rs` and `tools/report.rs` own theirs — the kind constants, `stored_fields`, the
  descriptors, `impl Projection for … {}` taking the default for both, and the three queries:
  the delivery key for one origin, the origins of one delivery, and whether a retraction
  already stands for one delivery. Every query is conversation-scoped by joining
  `conversation_blocks`, for the reason `kind.rs:791-798` states — ids are unique per channel
  only — and each is one bounded statement, the standard `kind.rs:843-849` sets.
- **Core, kind composition** (`crates/core/src/kind.rs`): the two kinds join `AssistantKind`
  (`:1118-1134`) so one parse path reads every block. `ChatMessage::stored_fields` (`:444-470`)
  changes in exactly one way: the `AssistantMessage` arm at `:466-468` binds the new field and
  deliberately does not store it, and a comment says why. `COLUMN_REPLY_TARGET`'s
  documentation (`:131-151`) stays true and unedited — that is the point of not storing it.
- **Core, schema** (`crates/core/src/schema.rs`): one appended migration step per decision 0026,
  added to the list in `store_config` (`:372-395`) after the newest step, creating both content
  tables and the indexes the lookups need — the delivery table keyed by block id with an origin
  column and a delivery column, indexed on each, and the retraction table keyed by block id with
  a delivery column, indexed on it. The report table (`:296-310`) is the shape to follow, and
  `PROTECTION_STAMP_MIGRATION` (`:160-172`) shows an index created in the same step. Both tables
  are structure, not personal data: erasure leaves them, and a conversation's deletion removes
  them through the block cascade.
- **Core, mirror** (`crates/core/src/mirror.rs`): `mirrored_target` (`:58-70`) is replaced by one
  function that returns the recognized ask — the person's-message origin, or the assistant's-
  message origin (itself optional) — or nothing. Recognition keeps the same three conditions and
  reads the message alone. The module's own documentation changes twice: the constant's doc
  (`:27-33`) no longer says the assistant's part is bookkeeping, and the new function's doc
  states which of its outcomes are store-independent and why.
- **Core, ingestion** (`crates/core/src/assembly.rs`): the ask is resolved once where
  `mirrored_target` is called today (`:687-691`), ahead of the owing-tail read at `:692`; the
  command stamp at `:694` reads the ask's presence, not its effect; the effect is a match over
  the ask's variants — null the row, or resolve and append the retraction. The retraction's
  directive is the deterministic item the call returns, resolved at the one place `deliver` is
  resolved today (`:760-767`) and returned in `IngestOutcome::Recorded` (`:773-780`). The two new
  kinds join `DEBT_READ_THROUGH` (`:59-63`).
- **Core, delivery receipt** (`crates/core/src/assembly.rs`): a new entry point in the shape of
  `ingest` and `observe` that takes the handle and the reported origins and appends one
  `Delivered` block per origin. It tolerates a conversation that no longer exists — erasure can
  delete a direct conversation between the send and the receipt — by logging and returning
  without error; a failed append is logged the same way, and the consequence, a message that
  cannot be retracted, is stated in the entry point's documentation.
- **Core, outbound** (`crates/core/src/outbound.rs`): `OutboundReply` carries the handle, set at
  the one construction site (`:365-370`, which covers both the answer and the report kinds) and
  at the failure notice (`:405-412`). The notice gets a handle like everything else; the first
  draft made it permanently unretractable for no stated reason.
- **Adapter, client** (`crates/adapters/telegram/src/client.rs`): `send_body` (`:439-460`)
  decodes the sent message's id instead of discarding it at `:459` and returns it; `send_message`
  (`:371-390`) collects the ids and returns them, and `SendError` (`:88-97`) carries the
  delivered ids in place of `delivered_chunks` — the count the two log lines at `:745-758` read
  is then the list's length, so the existing log shapes stand. Add one `delete_messages` beside
  `leave_chat` (`:348-352`) with the same `MAX_RATE_LIMIT_WAIT` ceiling, taking at most 100 ids
  per call. No `delete_message` is added: one method serves both sizes.
- **Adapter, driver** (`crates/adapters/telegram/src/driver.rs`): `consume_replies` (`:730-760`)
  gains the `Assistant` handle — `Arc<Assistant>` is in scope at `:257-260` and the `select!` at
  `:282-286` passes it — and reports the receipt after each send, including a cut-short one.
  `send_item` (`:601-606`) does the same, with the handle taken from the ingest receipt the
  driver already receives at `:437-441` but currently discards through `..`. The directive match
  replaces the blind `item.text()` call at `:438-441`, sending text items and performing the
  retraction through batched `delete_messages` calls.
- **Adapter, translate** (`crates/adapters/telegram/src/translate.rs`): `reply_target_of`
  (`:454-462`) fills the new origin from `replied.message_id`. The decimal naming rule gets its
  missing half — a named encoder beside `message_id_of` (`:326-334`), whose own documentation
  already says "both directions of the naming rule live beside each other" — used by the
  receipt, by `Pending::origin` (`:192`) and by the reply target (`:460`).
- **Test fixtures** (`crates/core/tests/spine/mirror.rs`, `crates/adapters/telegram/tests/`):
  the spine fixture's hand-set `addressed = false` (`:272`) goes, so the mirror's no-ops run
  against the flag the adapter produces; the stub server (`tests/adapter/server.rs`) returns
  message ids from `sendMessage` (`:398`) and routes `deleteMessages`, recording each request's
  ids for the batching pins; the token-scan binary forces the new failure path.
- **Documents.** The privacy policy's retention and deletion section
  (`docs/privacy/bot-assistant-privacy-policy.md:106-125`) gains the converse of the sentence
  already there — asking us to delete does not remove anything from the group's chat, because
  the assistant can only delete its own messages and only within the platform's 48-hour window —
  plus one sentence that an administrator can make the assistant take back its own message.
  The record of processing (`docs/privacy/records-of-processing.md`) gains a data category for
  the identifiers of the assistant's own sent messages, with its retention stated and justified:
  kept for as long as the conversation, removed with it, and not reached by a person's erasure,
  because the identifier names a message the assistant wrote and carries nothing of anyone else.
  The DPIA's storage-and-deletion section (`dpia.md:255-268`) gains the distinction between
  erasure and retraction, and answers the storage-limitation question this unit creates: the
  identifier's only use expires after 48 hours, when the platform stops allowing the deletion,
  while the store carries no expiry timer at all (decision 0003) — the identifier stays because
  it is not personal data, an expired one is inert, and adding a timer for a structural field
  would be new machinery with no benefit to anyone's rights. The DPIA's moderation section
  (`dpia.md:638-654`) records that the assistant now holds a platform deletion capability
  bounded to its own messages, that the bound is in the code and not in the granted rights, that
  the standing-capability review trigger therefore does not fire, and that no timing window like
  the mirror's exists here because a retraction nulls nothing. The operator contract
  (`docs/reference/group-operator-contract.md:137-158`) gains the second effect of `/del`, drops
  the sentence saying the assistant adds nothing because the administrator addressed the
  moderation bot (`:139-146`), and states plainly that the assistant must not be granted
  `can_delete_messages` or made an administrator.
- **Streaming.** Nothing here moves bytes, so the standing constraint applies to the request
  side and to the record: the recorded origins are read and deleted in batches of at most 100,
  one call at a time, and no larger request body is ever assembled; the send path keeps its
  chunk-by-chunk shape, and each delivered id becomes its own block, so the record grows with the
  send instead of waiting for a whole answer to be collected.
- **Two limits an implementer will meet.** The wire client reduces a 400 to its status code
  (`client.rs:561-585`), so the platform's reason for refusing a deletion will not appear in the
  log until the recorded follow-up on error bodies (`docs/follow-ups.md:12-17`) is resolved;
  this unit does not need the reason and does not fix it. And the model is shown nothing about a
  retraction, because a truthful line would have to name the answer, and naming it means
  projecting the assistant's own message origins — a change with its own privacy reading.
  Record both as follow-ups; do not widen the unit.
- **One constraint for a sibling unit, not edited here.** The decision that the bot is never
  made an administrator forecloses inbound reaction updates: `message_reaction` and
  `message_reaction_count` are only delivered to an administrator bot. Outbound reactions need
  no right. The reactions unit's author should read this before designing around inbound
  reaction events; this unit does not touch that file.
- Branches from `main` into its own worktree, merges back on completion, and the worktree is
  deleted.
