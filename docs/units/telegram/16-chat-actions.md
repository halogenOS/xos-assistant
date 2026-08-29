# Telegram unit 16 — chat actions: one honest status, and the states the platform has no word for

Date: 2026-08-27. The platform lets a bot put a status line above the chat — "typing…",
"sending photo…", "recording voice…" — through one method with eleven accepted values. This
assistant already sends one of them, on a four-second refresh, from the core's composing
signal. So the question this unit answers is not how to send an action. It is which of the
eleven values name a state this assistant is actually in, what the indicator should show when
a turn produces nothing at all, and what happens at the end of a turn given that the platform
offers no way to take the status down.

The receipts below were read on 2026-08-27 from the live Bot API pages and from this tree at
`7fb217d`.

## The four findings that shape the unit

1. **The status cannot be cleared. It can only expire, or be overwritten by a message.**
   Verbatim from the method: "The status is set for 5 seconds or less (when a message arrives
   from your bot, Telegram clients clear its typing status)." There is no `clearChatAction`,
   no empty action value, and `action` is required. So every stop this tree performs is
   advisory: it stops our refresh, and the member keeps seeing the status until the platform
   drops it. When a turn dies without sending anything, the indicator outlives the assistant's
   own giving-up by between one and five seconds. That is the platform's, it is not fixable,
   and this unit names it instead of pretending the stop is a stop.

2. **There is no action for thinking, and the platform's own answer to that is out of reach
   here.** None of the eleven values means "working on it". Bot API 9.3 added
   `sendMessageDraft` and 10.1 the thinking block, which do mean it — and both are
   private-chat only, ephemeral, and require the finished text to be re-sent as a real
   message afterwards. This assistant serves a group. The unit refuses them and makes the
   refusal a checkable property of the code instead of an accident of it.

3. **The honest indicator for a turn that says nothing is nothing, and that already holds by
   derivation — but nothing at the wire proves it.** Since unit 22 the cue begins on the
   framework's `responding` signal, raised at the first non-empty text delta, so a turn that
   finalizes empty raises no cue. The core spine pins that (`crates/core/tests/spine/helpful.rs:183-235`).
   The adapter's own battery does not: its "no action" pin covers a deterministic reply, not a
   model turn that stays silent. In helpful mode silence is the default (decision 0098), so the
   unproven case is the common one.

4. **Two decision records and one core doc comment still describe a derivation the code no
   longer has, and a sibling spec has already copied it.** Decisions 0102 and 0103 say the cue
   is on during the model's thinking window, derived from the framework's `awaiting` field.
   Unit 22 replaced that derivation on 2026-08-24 and wrote no superseding record;
   `crates/core/src/composing.rs` reads `awaiting` nowhere outside its test helper, and
   `crates/core/src/message.rs:342-357` still restates the old rule. Telegram unit 06 then
   cited decision 0103 as current (`docs/units/telegram/06-reactions.md:220-225`). One decision
   is recorded once; this unit repairs the record in the area it owns.

## Grounding

### The platform (core.telegram.org/bots/api and /bots/api-changelog, read 2026-08-27)

- **Version.** The changelog's newest entry is Bot API 10.3, dated 24 August 2026; 10.2 is
  14 July 2026 and 10.1 is 11 June 2026. A brief naming 10.1 as current is two releases behind,
  the same correction Telegram unit 02 recorded on 2026-08-25.

- **The method, verbatim.** "Use this method when you need to tell the user that something is
  happening on the bot's side. The status is set for 5 seconds or less (when a message arrives
  from your bot, Telegram clients clear its typing status). Returns True on success." The page
  adds an example — "The ImageBot needs some time to process a request and upload the image.
  Instead of sending a text message along the lines of 'Retrieving image, please wait…', the
  bot may use sendChatAction with action = upload_photo. The user will see a 'sending photo'
  status for the bot." — and one recommendation: "We only recommend using this method when a
  response from the bot will take a noticeable amount of time to arrive."

- **The parameter table, complete — four parameters and no others.**
  - `business_connection_id`, String, Optional: "Unique identifier of the business connection
    on behalf of which the action will be sent".
  - `chat_id`, Integer or String, Yes: "Unique identifier for the target chat or username of
    the target bot or supergroup in the format `@username`. Channel chats and channel direct
    messages chats aren't supported."
  - `message_thread_id`, Integer, Optional: "Unique identifier for the target message thread
    or topic of a forum; for supergroups and private chats of bots with forum topic mode
    enabled only".
  - `action`, String, Yes.

- **The eleven values, verbatim from the `action` row.** "Type of action to broadcast. Choose
  one, depending on what the user is about to receive: `typing` for text messages,
  `upload_photo` for photos, `record_video` or `upload_video` for videos, `record_voice` or
  `upload_voice` for voice notes, `upload_document` for general files, `choose_sticker` for
  stickers, `find_location` for location data, `record_video_note` or `upload_video_note` for
  video notes." Two further strings are accepted but undocumented: Bot API 5.2 (26 April 2021)
  "Fixed an error in sendChatAction documentation to correctly mention 'record_voice' and
  'upload_voice' instead of 'record_audio' and 'upload_audio' for related to voice note
  actions. Old action names will still work for backward compatibility."

- **When the surface arrived.** `record_video_note` / `upload_video_note` came with video notes
  in Bot API 3.0. `choose_sticker` in 5.4 (5 November 2021): "Added support for the
  choose_sticker action in the method sendChatAction." `message_thread_id` in 6.4 (30 December
  2022): "Added the parameter message_thread_id to the method sendChatAction for sending chat
  actions to a specific message thread or a forum topic." `business_connection_id` in 7.2
  (31 March 2024). 9.3 (31 December 2025): "Supported the parameter message_thread_id in
  private chats in the method sendChatAction, allowing bots to send chat actions to a specific
  topic in private chats." That 9.3 line is the last mention of the method in the whole
  changelog — every occurrence of the name was read — so 10.0 through 10.3 leave it untouched.

- **Nothing clears an action.** No `clearChatAction`, no documented empty or null value, and
  `action` is required. The complete set of endings: the platform's own expiry, a message from
  this bot arriving in that chat, or a different action replacing it.

- **`sendMessageDraft`, verbatim.** "Use this method to stream a partial message to a user
  while the message is being generated. Note that the streamed draft is ephemeral and acts as
  a temporary 30-second preview - once the output is finalized, you must call sendMessage with
  the complete message to persist it in the user's chat." Its `chat_id` is Integer, "Unique
  identifier for the target private chat"; its `text` row reads "Pass an empty text to show a
  'Thinking…' placeholder". `sendRichMessageDraft` is the same shape with `rich_message`.
  `RichBlockThinking` / `InputRichBlockThinking`: "A block with a 'Thinking…' placeholder,
  corresponding to the custom HTML tag `<tg-thinking>`. The block may be used only in
  sendRichMessageDraft, therefore it can't be received in messages." The history: 9.3 "Added
  the method sendMessageDraft, allowing partial messages to be streamed to a user while being
  generated"; 10.0 "Allowed all bots to use the method sendMessageDraft"; 10.1 "Allowed bots to
  pass an empty text in the method sendMessageDraft" and the `RichBlockThinking` class; 10.2 the
  input form of the block; 10.3 the stop button parameters. The chat's own kind never widened —
  `chat_id` is still a private chat.

- **Documented limits.** The API page states no per-method limit for chat actions. The only
  back-pressure the API documents for any method is HTTP 429 with
  `ResponseParameters.retry_after`. The FAQ's numbers are about messages, not actions: "In a
  single chat, avoid sending more than one message per second… In a group, bots are not be
  able to send more than 20 messages per minute… bots are not able to broadcast more than
  about 30 messages per second." No published number covers actions, so this unit invents none.

### Our tree, at `7fb217d`

- **One call site, one hardcoded value.** `crates/adapters/telegram/src/client.rs:401-408`:
  `send_chat_action(&self, chat_id: i64)` posts `{"chat_id": …, "action": "typing"}` to
  `sendChatAction` and discards the decoded `True`. No parameter, no map, no second value
  anywhere in the workspace.

- **The action has its own rate-limit ceiling.** `CHAT_ACTION_WAIT_CEILING` is one refresh
  period (`client.rs:47-54`), so a stated `retry_after` past four seconds fails the call at
  once with `ClientError::RateLimitWaitOverCeiling` (`client.rs:79-84`) instead of parking the
  loop past the cadence it exists to keep. The per-caller ceiling and its reasoning are at
  `client.rs:490-527`.

- **Cadence and self-bound.** `TYPING_REFRESH` is four seconds (`driver.rs:73-79`), chosen just
  under the platform's five-second expiry. `TYPING_REFRESH_CYCLES` is the core's signal
  lifetime in refresh periods plus two (`driver.rs:89-90`) — 77 cycles against the five-minute
  lifetime at `crates/core/src/composing.rs:73`. `refresh_typing` (`driver.rs:688-699`) sends
  first and sleeps afterwards, so a begin shows the status immediately; a failed action is
  logged and the cadence continues.

- **The registry.** `TypingRefreshers` (`driver.rs:627-677`) holds one abort handle per chat:
  `begin` replaces a running loop and sweeps finished handles, `stop` aborts, `Drop` aborts
  every one. `consume_composing` (`driver.rs:705-721`) translates a transition into a begin or
  a stop and reads nothing else. `consume_replies` calls `typing.stop(chat_id)` before every
  send (`driver.rs:740`).

- **The core's cue and its true derivation.** `composing.rs:139-163` begins on
  `CoreEvent::StreamStatus` with the `RESPONDING` label — raised once per stream at the first
  non-empty text delta, never for thinking, never for a stream that finalizes empty
  (`composing.rs:5-23`). `composing.rs:171-188` stops on `StreamDone`, `StreamError` or
  `StreamClosed`. `composing.rs:195-204` answers a bus lag by stopping every open signal.
  `composing.rs:99-128` expires a signal still open at the five-minute deadline. The edge is
  live-only: nothing stored, nothing seeded, nothing owed across a restart
  (`composing.rs:27-33`).

- **The stale statements.** `crates/core/src/message.rs:342-357` says the cue is "on while the
  model is composing (its thinking and its streaming), and off during a tool call and a human
  wait", and that the stop means "the turn completed or failed". The edge's own module
  documentation says the opposite about thinking, and the stop keys on the stream's terminal
  set. `docs/decisions/0102-*.md` and `docs/decisions/0103-*.md` carry the `awaiting`
  derivation; `composing.rs` matches on `awaiting` nowhere — the only occurrences are the test
  helper's argument (`composing.rs:285,291,368,377,421`). No decision record in `docs/decisions`
  contains the word `responding`.

- **Delivery never overlaps the cue in the ordinary path.** The outbound edge delivers on
  `StreamDone`, `StreamError` and a lag (`crates/core/src/outbound.rs:195-262`) — the same
  terminal set the composing edge stops on — so by the time a reply reaches `consume_replies`
  the cue has already stopped. The `typing.stop` at `driver.rs:740` is observable only in the
  lag divergence Telegram unit 06 describes (`06-reactions.md:220-228`).

- **The existing wire pins** (`crates/adapters/telegram/tests/adapter/composing.rs`): a summoned
  model turn records at least one action before its answer and none after (`:40`); a
  deterministic reply records none (`:90`); a failed turn (`:122`) and a quiet failure (`:156`)
  stop the refresh; two chats compose independently (`:194`); a failing action leaves the
  answer untouched (`:257`). The scripted provider's turn hold
  (`tests/adapter/support.rs:166-171`) makes the first pin real. The scripted server records
  and can fail actions (`tests/adapter/server.rs:84-87,225-231,416`).

- **The adapter's provider cannot script a silent turn; the core's can.** Every non-tool,
  non-failing turn in `tests/adapter/support.rs:300-320` streams `answer_to(...)` and ends. The
  spine's provider takes a silence cue — `SILENT_CUE` at
  `crates/core/tests/spine/support.rs:118-124`, used at `:406-412` to stream no text at all —
  and `spine/helpful.rs:183-235` pins that the silent turn raises no composing transition.

- **The vocabulary check and its reach.** `docs/platform-vocabulary.txt` holds platform, SDK
  and (since unit 02) method names. The scan (`crates/core/tests/vocabulary.rs:60-88`)
  lowercases each file and matches whole words, where a word is one run of letters and digits
  and `_` separates runs. So `sendchataction` is expressible as one term and `upload_photo` is
  not: it would have to be listed as `upload` or `photo`. Verified today: `sendchataction`
  and `chat_action` appear nowhere under `crates/core`, while the word `typing` appears three
  times in core test prose (`crates/core/tests/spine/audience.rs:225,228`,
  `crates/core/tests/spine/helpful.rs:183`).

- **What the neighbouring units already decided, and what this unit therefore does not
  respecify.** Telegram unit 10 gives `send_chat_action` the section parameter, re-keys
  `TypingRefreshers` on the address and pins that two topics of one chat compose at once, each
  action carrying its own `message_thread_id` (`10-forum-topics.md:55-57,160-162,381-383,437`).
  Telegram unit 22 refuses the whole business surface, including a connection identifier on any
  outbound body (`22-business-accounts.md:265-269,326,411`). Telegram unit 02 defines the
  outbound attachment vocabulary — `OutboundAttachment { address, media_type, filename }` on a
  `ReplyKind::Illustration` variant — and the adapter's media-type map: `image/gif` to
  `sendAnimation`, any other `image/*` to `sendPhoto`, anything else to `sendDocument`
  (`02-sending-media.md:385-407`), and states that its code arrives with its first caller
  (`:272-280`). Telegram unit 13 refuses to send a position or a place; unit 14 refuses to send
  a sticker or a dice roll (`14-stickers-and-dice.md:615-620`).

- **Channels never arise.** `CONSUMED_UPDATE_TYPES` is `["message", "edited_message",
  "my_chat_member"]` (`client.rs:103`), so no channel post reaches this process and the
  method's refusal of channel chats cannot be hit.

## Decisions taken with this unit

- **The cue's real derivation is recorded once, in a new decision that supersedes 0102 and
  0103, 2026-08-27.** A new record states what the code does: the cue begins on the framework's
  `responding` signal at the first non-empty text delta and stops on the stream's terminal set,
  so the pre-text thinking window is dark, a turn that says nothing raises no cue, and a
  tool-bearing turn raises one begin/stop pair per text-bearing stream. It names 0102 and 0103
  as superseded and states which of their claims survives: the outcome "the cue is off during a
  tool call" still holds, because no text flows during one, but the mechanism they name is gone.
  *Rejected:* editing 0102 and 0103 in place — a decision log that rewrites its own past is the
  mutation the ledger rule exists to prevent, and the dates are how a reader tells which
  statement was current when a sibling spec cited it. *Rejected:* leaving the records and
  repairing only the code comment — the records are what unit 06 read, so the stale statement
  would keep spreading.

- **The core's type documentation points at the edge instead of restating it, 2026-08-27.**
  `message.rs:342-357` loses its summary of when the cue is on and says only what the type
  means — a live presence cue, never stored, owing nothing across a restart — with the
  authoritative statement named as the composing edge. One decision, one place. *Rejected:*
  updating the summary to the new derivation, which is the same duplication that went stale
  within a day the first time.

- **A turn that produces nothing shows nothing, and the wire proves it, 2026-08-27.** The
  adapter's scripted provider gains a silence cue in the shape the spine's already has: a turn
  whose newest projected message carries the cue streams no text and ends, the framework
  commits the empty answer block, the outbound edge delivers nothing (unit 22), and no
  `responding` ever fires — so the recorded action count for that chat stays at zero. This is
  the honest answer to "what does the indicator do when the answer is silence": in a group
  where silence is the default (decision 0098), a status line promising an answer that never
  comes is a lie the assistant tells fifty times a day. *Rejected:* showing `typing` from the
  moment a turn is owed — the operator asked for the opposite on 2026-08-24 and unit 22 built
  it; it would also light the cue for every unaddressed message the model reads and declines to
  answer. *Rejected:* a short delay before the first action, so that only turns which take long
  enough show a status — the platform's recommendation invites it, but it adds a timer that
  estimates what `responding` already states exactly, and a fast turn would then show no
  status at all while its answer still took a second to arrive. *Rejected:* leaving the
  property to the spine pin alone — the spine proves a transition is absent, not that no call
  reaches the platform, and the adapter is where a stray refresher would live.

- **No thinking indicator, and no message drafts, 2026-08-27.** `sendMessageDraft`,
  `sendRichMessageDraft` and the thinking block are refused as a class, on three independent
  reasons, any one of which is sufficient: they take a private chat and this assistant answers
  in a group; they put text in a chat that no ledger block holds, so an erasure could never
  reach it and the published record would describe a store that does not hold everything the
  assistant said; and the finished text must be sent again as a real message, so the draft is a
  preview layered over the send this tree already performs and adds a second write path to the
  same answer. The refusal is checkable by the adapter source scan unit 08
  introduces, extended with the two method names. *Rejected:* using a draft in direct chats
  only — a mechanism that exists in one channel kind and not another is a platform branch
  growing inside the send path, and direct chats are a configuration switch (decision 0069),
  not a second product. *Rejected:* `find_location` as a stand-in for "the assistant is looking
  something up" — it tells the client that location data is coming, unit 13 refuses to send
  any, and a status naming a category the member will never receive is a lie with a plausible
  excuse.

- **The lingering status is accepted, named, and not papered over, 2026-08-27.** At the moment
  the assistant stops, the last action is between zero and four seconds old, so the member sees
  the status for a further one to five seconds. Nothing in the API removes it. The arithmetic
  runs against intuition and is written down here so nobody re-derives it wrongly: each action
  sets a fresh five-second status, so a *shorter* refresh cadence lengthens the expected tail
  instead of shortening it, and only a cadence approaching five seconds would shorten it, at
  the cost of a visible gap mid-turn. The cadence therefore stays at four seconds.
  *Rejected:* sending a message to clear the status — the platform clears on any message from
  the bot, so this would work, and it would put a message in the group that answers nothing,
  pings members and has no block behind it. *Rejected:* dropping the cadence to one second to
  shrink the tail, which spends four times the calls and makes the tail worse.

- **An upload shows the platform's upload status, derived with the method in one match,
  2026-08-27.** Unit 02's adapter already turns a neutral media type into a method. That match
  becomes one that produces a pair — the method and the action the platform documents for it —
  so the two facts are decided in one place and cannot drift: `sendPhoto` with `upload_photo`,
  `sendDocument` with `upload_document`, `sendAnimation` with `upload_video`. The core learns
  nothing: the status a method shows is a fact about the platform, on the same footing as the
  method name, and unit 02's contract already allows the adapter exactly this translation.
  `upload_video` for an animation is the honest one of two imperfect choices — the platform
  lists no animation action and `sendAnimation` delivers a video file, so "sending video" is
  what is literally on the wire. *Rejected:* a new `ComposingState` variant such as `Uploading`
  — the composing edge is keyed on the model's text stream, an upload is not a stream, and the
  variant would make a general mechanism know about one kind of reply. *Rejected:* a neutral
  "what is being prepared" field on the outbound reply — it is the media type spelled a second
  way, and two places would decide one thing. *Rejected:* the core passing the action string,
  which is unit 02's rejected "core passes the method name" with a different noun. *Rejected:*
  showing `typing` during an upload, which tells the member the assistant is writing while a
  picture arrives.

- **The upload status runs on the existing refresher for as long as the send takes, and no
  size is measured, 2026-08-27.** In the reply consumer's attachment arm the chat's refresher
  is begun with the upload action instead of being stopped, the send runs, and the refresher is
  stopped when the send returns. One registry, one place deciding when an indicator runs; the
  registry gains the action as a parameter and nothing else. Every attachment shows a status,
  whatever its size: under unit 02's address form the platform fetches the file itself and our
  wait is one small request, and under the stored-file form the wait is the upload — in neither
  case is the duration knowable in advance, and measuring it would mean reading the file, which
  the streaming rule forbids and unit 02's existence check already refuses to do. *Rejected:* a
  single action call before the send — the platform expires the status in five seconds, so a
  fifteen-second upload would show a status for its first third. *Rejected:* a size threshold
  under which no status is shown, which needs the size the design deliberately never has.
  *Rejected:* a second registry for upload loops, which is two mechanisms deciding one thing.
  *Rejected:* restoring the typing cue after the upload finishes — the adapter would have to
  remember a cue it does not own, and the only case where this is observable is the lag
  divergence unit 06 analysed, whose cost is a missing status and never a wrong one, the
  direction `composing.rs:195-204` already chose.

- **Streaming: the status loop is concurrent with the send future, which is what makes it
  work at all, 2026-08-27.** The refresher runs as its own task while the send is awaited, so
  bytes move from disk to the socket through reqwest's streaming body (unit 02's upload form)
  with the status refreshed alongside and nothing buffered to decide anything. The action value
  is chosen from the media type the core already resolved, so no byte is read to select it.
  *Rejected:* sending the action between chunks of the upload, which would require owning the
  transfer loop and reading it into pieces this adapter has no reason to see.

- **Three action values ship and eight do not, 2026-08-27.** `typing` on the composing cue, and
  the three upload values on unit 02's methods. `choose_sticker` and `find_location` have
  producers only if units 14 and 13 are reversed. The `record_*` family means the bot is
  capturing audio or video, which this assistant has no means to do and no reason to claim.
  `upload_voice` waits for the media subsystem, which sends no voice notes today. The two
  legacy names appear nowhere. *Rejected:* writing all eleven "for completeness", which is the
  unreachable code unit 02's first decision forbids, and which invites a later reader to reach
  for a status the assistant cannot honestly show.

- **The refusal is made checkable as far as the instrument reaches, and stated plainly where it
  does not, 2026-08-27.** `docs/platform-vocabulary.txt` gains `typing` and `sendchataction`,
  and the three occurrences of "typing" in core test prose are reworded to the core's own word,
  composing — the core naming the platform's rendering is exactly the leak the list exists to
  catch. The ten action values containing an underscore cannot be listed, because the scan
  splits on it; that half is a review criterion, called that instead of called pinned, following
  unit 02's treatment of platform numbers. *Rejected:* listing `upload` or `photo` as bare
  words, which matches ordinary prose and trains people to widen an ignore list until the check
  means nothing. *Rejected:* leaving `typing` off the list because the word also means type
  theory — the answer to a false positive is to reword the core's prose, and the core has no
  reason to discuss static typing.

- **Nothing here changes a privacy or compliance document, 2026-08-27.** A chat action carries
  a chat identifier and a fixed word to the platform that already carries every message in that
  chat; no new category of data goes to the model provider, nothing new is stored, and no new
  recipient is reached — the recipient table's R1 through R5 are untouched
  (`docs/privacy/records-of-processing.md:80-86`). The cue itself is live-only and lands on no
  ledger row, so erasure has nothing to reach. Decision 0070 is untouched as well: a status
  line files nothing, hides nothing and restricts nobody.

## The unit's contract

The composing cue's derivation is recorded once, in a decision that supersedes 0102 and 0103,
and the core's type documentation points at the edge instead of restating it. The wire proves
what the spine already proves: a model turn that produces no text draws no chat action at all,
and the spoken turn beside it draws one — the assistant never tells a group that an answer is
coming when none is. Nothing else about the typing path changes: the same four-second cadence,
the same self-bound, the same swallowed failures, the same per-chat keying, and the same
acceptance that the platform's status outlives our stop by up to five seconds because no method
removes it. Message drafts and the thinking block are refused, and the refusal is asserted over
the adapter's own source. When unit 02's send path is built, an attachment send shows the
platform's upload status for the whole duration of the send, on the existing refresher, with
the action derived in the same match that picks the method, and no size is measured to decide
it. The core gains no platform vocabulary: `typing` and the method name join the committed word
list, and the values the scan cannot express are named here as a review criterion. No new
dependency, no configuration change, and no privacy or compliance document changes.

## Acceptance criteria

1. Workspace suite green in both modes; clippy, fmt and doc under denied warnings; vocabulary
   and secret scans clean; no new dependency.
2. A silent turn draws no action at the wire: with the adapter's scripted provider given a
   silence cue in the shape `crates/core/tests/spine/support.rs:118-124` uses, an addressed
   message that summons a turn producing no text records zero `sendChatAction` calls and zero
   `sendMessage` calls for that chat, proven past a barrier — a second message recorded through
   the whole pipeline — exactly as the deterministic-reply pin at
   `tests/adapter/composing.rs:90` proves silence today.
3. The spoken turn in the same test still records at least one `sendChatAction` whose `action`
   is `typing`, so criterion 2 cannot pass by breaking the cue.
4. The six existing composing pins pass unchanged, including the refresher's self-bound test at
   `driver.rs:774-783`.
5. The core's `ComposingState` documentation states no derivation and names the composing edge
   as the authority; a new decision record supersedes 0102 and 0103, names them, states the
   `responding` derivation and records what of their claims survives. A scan of `docs/decisions`
   finds no other record stating the `awaiting` derivation as current.
6. `docs/platform-vocabulary.txt` contains `typing` and `sendchataction`, and the core scan is
   green — which requires the three reworded comments and proves the core carries neither word.
7. The draft methods are refused structurally: the adapter source scan introduced by unit 08's
   AC3 also fails on `sendMessageDraft`, `sendRichMessageDraft` and `tg-thinking`, with its
   comment naming this unit's decision.
8. The action set is closed and checkable: a test over the adapter's media-type match asserts
   the produced pairs are exactly `sendPhoto`/`upload_photo`, `sendDocument`/`upload_document`
   and `sendAnimation`/`upload_video`, and that the match is total over the media types unit 02
   defines. Review criterion beside it: no other action value literal appears in the adapter.
9. (Only when unit 02's send path is built.) An attachment send shows its own status: the
   recorded call sequence for an illustration is at least one `sendChatAction` carrying the
   method's action before the media call, no `typing` action during it, and no action after the
   media message is on the wire — the same barrier shape as pin 2. With the send held open past
   two refresh periods, the action count keeps moving, proving the status is refreshed rather
   than sent once.
10. (Only when unit 02's send path is built.) A failing upload action leaves the send untouched,
    pinned with the scripted server's action failure (`tests/adapter/server.rs:225-231`), and a
    failing send stops the refresher — no loop outlives a send that returned.
11. No behaviour changes for the deterministic-reply path, the withdraw path or the first-contact
    lookup: their recorded call sequences are identical before and after.

## Notes for launch

- Branches from `main` at `7fb217d`. Sites, all of them named: the adapter's scripted provider
  and its silence cue in `crates/adapters/telegram/tests/adapter/support.rs` beside the turn
  hold at `:166-171` and the answer script at `:300-320`; the new pin in
  `crates/adapters/telegram/tests/adapter/composing.rs`; the documentation repair at
  `crates/core/src/message.rs:342-357`; the reworded prose at
  `crates/core/tests/spine/audience.rs:225,228` and `crates/core/tests/spine/helpful.rs:183`;
  the two words in `docs/platform-vocabulary.txt`; one decision record continuing the numbering
  after whatever is unclaimed when this merges — the highest on `main` today is 0105.
- Do not touch `crates/adapters/telegram/src/client.rs:401-408`, `driver.rs:627-721` or
  `crates/core/src/composing.rs` in this unit. The typing path is correct as it stands; the
  action parameter on `send_chat_action` and the re-keying of `TypingRefreshers` belong to
  Telegram unit 10, and the attachment arm of the reply consumer belongs to Telegram unit 02.
  If either merges first, criteria 8 through 10 are written against the shape that exists.
- Criteria 9 and 10 are written for whoever builds unit 02's send path. That implementer reads
  this unit's upload decisions and folds them into that build; nothing in this unit ships an
  attachment arm on its own, because a capability with no caller is the dead code unit 02's
  first decision forbids.
- Read but not edited: `docs/units/telegram/06-reactions.md:220-225` cites decision 0103 as the
  current statement of when the cue is off. Its conclusion survives — the cue is dark during a
  tool call — but the mechanism it quotes does not, and the superseding record written here is
  what a later reader of that spec needs.
