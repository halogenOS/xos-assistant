# Unit 55 — speaking is an action

Date: 2026-09-02. The assistant stops relaying the model's text to the group. The model's
messages reach the chat only through two tools — `send_message`, and `reply_message` with a
`reply_to` naming the message it answers — so one turn can post several messages, answer
specific messages, or post nothing, and the model's written text becomes its own private
notes. Every
message the model reads carries a small envelope naming who wrote it, when the platform says
it was sent, and its id, so the model can aim a reply.

The repositories: the app (this repo, HEAD `4d56841`) and the framework (`ronna-core`,
`~/projects/agent-ledger`, HEAD `b7c0c45`; paths rooted at `crates/agent-ledger/`). The
framework gains two small doors named below and changes first; the rest is the app.

## What is true today

Every claim was read from the two trees at the stated heads.

1. **An answer is relayed text.** The framework commits a turn's text as an assistant `text`
   block (`src/store/messages.rs:302-337`); the app's outbound edge wakes on `StreamDone`,
   walks blocks past a per-conversation cursor, classifies an assistant text block as a
   reply (`crates/core/src/outbound.rs:380-511, 688-727`), cuts a leaked reasoning closer,
   composes the first-interaction disclosure line into the STORED block, decides threading
   by `answer_target` — the one absorbed message that literally addressed the assistant,
   plain send otherwise, plain send when the text carries a reply-command shape
   (`outbound.rs:755-774`) — and pushes `Outbound::Reply` to the adapter.
2. **The adapter chunks and records.** Telegram splits at 4096 UTF-16 units, threads only
   the first chunk, retries a refused thread plain once, and calls
   `Assistant::report_delivery` with every delivered platform id
   (`crates/adapters/telegram/src/client.rs:645-706`, `driver.rs:1159-1179`). The receipt is
   one `delivered` block per platform message: `origin`, `delivery`, `answer_block`
   (`crates/core/src/delivery.rs:66-161`). **Receipts exist only since unit 38, 2026-08-30**
   (`crates/core/src/schema.rs:465-501`, no backfill): an answer sent before that has no
   record, and a reply to it lands quoteless today.
3. **Text before a tool call is delivered too.** A round's text ahead of a tool call
   finalizes as its own answer and goes out (decision 0146); the search heads-up line and
   unit 54's "call the closing tool bare" prohibition both rest on that mechanism.
4. **The model names a message by a bracketed id.** User-voiced messages project as
   `[origin] speaker: text` with `(edited)` for a revision and `[message erased]` for an
   erased row (`crates/core/src/kind.rs:190-231, 702-720`). The projection is a PER-BLOCK
   reading with no ledger access; the platform send time is stored per message in `sent_at`
   as an RFC 3339 UTC string (`kind.rs:50-53`, `assembly.rs:2809`), distinct from the block
   header's store-clock `created_at`. Rendering is computed at request time and stored
   nowhere (`src/actor.rs:1292-1297`), so a format change re-renders all history.
5. **An assistant answer projects bare.** It is the framework's `Text` kind; the app cannot
   give it a per-block prefix, and the `delivered` receipt projects nothing
   (`delivery.rs:176-180`). Rendering an old answer under its platform id would need ledger
   access the projection contract does not give.
6. **Tools aim through validated sets.** The react tool accepts only a co-summoner of the
   turn (`crates/core/src/tools/mark.rs:442-466`); the report tool a wider assessment set.
   A tool posts by appending a block the outbound edge later classifies (the mark), never by
   talking to the adapter itself (`outbound.rs:486-506`).
7. **Rate bounds are one tier each.** The framework's per-tool window holds one `calls` and
   one `seconds` per tool name, unshareable across names (`src/tools/runner.rs:120-133,
   169-180`); the app's windows are single-span (`crates/core/src/window.rs`). A spent
   window is a `Refusal::Refused`; five consecutive refusals end the turn.
8. **The typing cue keys on text.** `RESPONDING` fires at the first non-empty text delta
   (`crates/core/src/composing.rs:138-161`). The framework raises `RUNNING_TOOLS` when a
   stream STOPS for tool use — execution beginning, after the whole call streamed
   (`src/ingestion.rs:1015-1021`) — and handles the per-call moment a tool call begins
   streaming in `tool_use_start`, which inserts the streaming call block and raises no
   status (`src/ingestion.rs:1110-1136`). Nothing names the tool while its arguments stream.
9. **The budget refund keys on an empty answer.** `COUNTED_DEBT_SQL` counts a debt unless
   the anchored assistant text is empty (`kind.rs:1322-1334`).
10. **A changed prompt reaches old sessions by a startup fork.** `retire_stale_channels`
    forks every mapped conversation onto the current prompt once at startup
    (`assembly.rs:1304-1374`); the tool-choice reconcile appends a fresh choice on a
    registered-set delta (unit 52).
11. **The teaching speaks of writing.** The silence sentences say "end your turn without
    writing any text"; the search teaching carries a heads-up that exists because pre-call
    text is delivered; unit 54's closing prohibitions and both turn-ending tools' descriptions
    restate the pre-call-posting mechanism (`crates/core/src/teaching.rs:128-134, 206-212,
    352-353, 361-363`; `prompts/30-conduct.md:19, 88-96`).

## The design

**Two tools, one outgoing record.** `send_message(text)` posts to the conversation's channel;
`reply_message(text, reply_to)` posts threaded onto the message whose id `reply_to` names.
Each call appends one consumer block, `outgoing_message` (table `block_outgoing_message`:
`text`, `reply_to` nullable), and returns `ToolOutcome::Pending`. The outbound edge classifies
that block exactly as it classifies a mark today and hands it to the adapter; the adapter
reports the delivery through the existing receipt door, which now also RESOLVES the pending
call: success completes it with the platform ids it was posted under, failure fails it with
the adapter's reason. The model learns the real id of what it sent, and a turn holding an
unresolved send stays open until the send is settled, by the framework's standing rule for a
system-owed call. Both tools require member authority through the admission macro and ride
the recorded tool choice like every tool.

**A send never outlives the process that made it.** The outgoing block records the call
block it answers (`call_block`), and the tool body is idempotent on it: a re-run of the same
call after a restart finds its outgoing block and appends no second one. A pending send the
process died with is not delivered late: at startup, before serving, every outgoing block
whose call is still unresolved is failed with a recorded sentence saying the process
restarted before the platform confirmed the send, so the model may send again on its next
turn — the same trade decision 0014 made for a redelivered update, a possible duplicate over
a possible silence. The outbound edge's startup seed therefore never meets an undelivered
outgoing block. Retiring a conversation settles it first, and the settle fails any pending
send it holds with a sentence naming the retirement.

**A send that posted partly is a failure that names what posted.** The adapter reports every
delivery with its outcome — whole, or cut short after some chunks — and the receipt door
records the delivered ids either way. A whole send completes the call with its ids; a
cut-short send fails the call with a sentence carrying the ids that did post and the
platform's reason for the rest, so the model can answer a member replying to the chunk that
posted.

**The relay ends.** The outbound edge no longer classifies an assistant text block as a
reply; the model's text is stored as today and delivered nowhere. `answer_target` and the
derived threading are deleted. What the relay carried moves onto the outgoing block: the
leaked-reasoning cut, which narrows the WIRE text only and leaves the stored text as the
model wrote it (unit 43's contract, unchanged); the first-interaction disclosure line,
composed into the outgoing block's STORED text before the send, idempotent, on the first
message to a never-introduced co-summoner; chunking (adapter, unchanged); and the
reply-command protection (an outgoing text carrying a reply-acted shape goes out unthreaded,
the same rule at the same edge). The report tool's fixed line keeps its own arm of the edge:
it is the tool's deterministic effect, like a reaction, and never the model's text.

**The reply target.** `reply_to` must name an id the conversation's ledger holds — a member
message's origin or revision, a join notice, or one of the assistant's own delivered ids —
of any age. An id the ledger does not hold is refused with a sentence saying so
(`Refusal::Failed`: the model can correct it), never sent plain, because a silently dropped
thread hides a hallucinated id. "Holds" means the serving conversation's own ledger: an id
compacted below the cut, or one whose message an erasure nulled, is no longer held and is
refused with the same sentence — the model never saw either under an envelope it could
still name. The platform's own tolerance for a vanished target
(`allow_sending_without_reply`) stays as it is.

**The envelope.** A user-voiced message projects as a YAML front matter and the text:

```
---
from: @handle
date: 2026-08-31T22:33:46Z
msgid: 12345
---
The actual message
```

`from` is the stored speaker and is omitted when none is stored; `date` is the stored
platform send time verbatim, never a store or framework clock; `msgid` is the id the model
names the message by — a revision shows under the revised message's id, as today, with an
`edited: true` line; an erased row projects the fixed erased marker and no envelope. Join
notices carry the same envelope. The assistant's own text projects bare: it is notes. The
model's sends appear to it as what they are — a tool call carrying the text and the target,
and a result carrying the ids the platform assigned.

**Old sessions read consistently.** Three mechanisms, all existing or ledger-recorded, none
a fork of the ledger and none a rewrite:

- the startup walk forks every mapped conversation onto the new system prompt;
- the tool-choice reconcile appends a choice naming the two tools on first activity. When
  that append is a DELTA — the conversation's newest prior choice existed and lacked the two
  tools — the app appends, in the same act, one system-voiced `contract_notice` block
  stating that the assistant's written answers above it were posted to the group as they
  stand, and that from there on its text is private and a message reaches the group only
  through the two tools. A first recording (a conversation born under this build) is not a
  delta and gets no notice. This is the user's "inject a block that tells the model the
  format changed", written once per conversation at the exact boundary, as a stored fact.
  Under compaction the notice sits after every raw answer it explains, so any cut that keeps
  a raw pre-contract answer in view keeps the notice with it, and a cut that summarizes the
  notice summarizes those answers too;
- member messages re-render under the envelope everywhere, old and new alike, because
  rendering is a request-time projection of stored columns.

Old answers do NOT render under a delivery id: the projection is per-block with no ledger
access, and receipts do not exist before 2026-08-30, so an envelope built from them would be
right for some answers and silently absent for others. The notice draws one honest line
instead.

**The order.** The two sending tools run in order: a call of either runs only after every
earlier call of either in the same conversation has resolved, so the messages reach the
group in the order the model issued them, and a pending send never has a sibling in flight.
The framework carries the rule as a tool hook, `runs_in_order` (default false): its runner
parks a ready call of an in-order tool while an earlier in-order call of the conversation is
unresolved and re-emits it the way it re-emits a latched one, and parallel calls stay
parallel for every other tool. The app's two sending tools answer the hook true. No filing
lock exists beside it: the order is the one mechanism.

**The pairing.** A call and its resolution are paired by the call's block id. The result and
error rows already record `source_block_id` (the framework's schema states that a model's
`tool_call_id` can repeat and the block id cannot); the framework's result and error kinds
carry it, and the framework's own predicate (`ToolCall::resolved_in` and the outcome reading
beside it) pairs by it. The app reads a send's state (delivered, failed, pending) through
that framework reading and holds no pairing walk of its own.

**The caps.** Per conversation, shared by both tools, three tiers: 5 sends per minute, 30
per hour, 100 per day, counted over the conversation's outgoing blocks in the trailing span
whose call completed with delivered ids or is still pending — a failed send posted nothing
and burns no tier.
The check runs once, in the tools' admission answer over the ledger the runner already
loaded — the framework's single-tier per-name window cannot express a shared three-tier
bound — and because the calls run in order, that ledger holds every earlier send and the
count is exact. A spent
tier refuses with `Refusal::Refused` and a sentence naming the tier that is spent and when it
reopens, so the model stops and resumes on a later turn; a run of five such refusals ends the
turn by the framework's rule. The three numbers are constants of the code today and become
configuration when the web UI arrives (decision below).

**The typing cue.** The framework raises a stream status at `tool_use_start`, once per
recorded call, carrying the tool's name — when the reader records the call's start, which
is as early as the wire allows; on the shipped wires the arguments have already arrived by
then, so the cue precedes the send by little, and that is the honest bound. `RUNNING_TOOLS`
keeps its own meaning (execution began) and is untouched. The
app lights the cue on that status for the two sending tools. The cue stops when the send is
done, whichever way it ended: the receipt door raises the stop for a delivered send, a failed
send and a cut-short send alike, and a call refused before anything was filed (a spent tier,
an unknown target, missing text) raises the same stop from the tool. The composing edge's
lifetime sweeper stays the backstop. The adapter stopping its own refresher ahead of the
platform call is the adapter's bookkeeping, not a carrier of the cue. Text deltas no longer
light it, because text is notes.

**The budget.** A counted debt is one whose turn DELIVERED at least one message — an
outgoing block whose call completed with ids; the empty-answer clause is replaced, not
extended. A turn of notes and no send costs the person
nothing, exactly as an empty turn does today.

**Everything that handled an answer handles the outgoing block.** Quoting a reply to the
assistant's message reads the outgoing block's own text; the take-back command resolves
through the receipt's `answer_block` to the outgoing block and strips it from view as it
strips an answer today; the erasure scrub and retraction forks treat outgoing blocks as the
assistant's words; the compaction cut's rule that a tool lifecycle is never split already
covers a send.

**The teaching.** The system prompt states the contract plainly: written text is private
notes and never reaches the group; the model's messages reach the group only through the two
tools (the report tool's fixed line and a reaction stay what they are, the tools' own
effects);
silence is still the default, and a turn ending without a send posts nothing. Every sentence
that presupposed relayed text is rewritten: the silence sentences say "end the turn without
sending"; the heads-up before slow work is sent with `send_message`; unit 54's bare-call
prohibition and the two turn-ending tools' restatement of pre-call posting are removed
because the mechanism they warned about no longer exists; the never-announce sentence stays.
The moderation and reaction teaching keep their meaning with "answer" read as "send".

## Acceptance criteria

Framework:

1. `tool_use_start` raises a stream status carrying the tool's name, once per call, never
   for text or reasoning; `RUNNING_TOOLS` and `RESPONDING` are unchanged. A test asserts the
   new status, its name, and that the two existing statuses still fire where they did.
2. A consumer can resolve a pending tool call out of band by its block id, completing it with
   a result or failing it with an error, through a public store door; the resolution carries
   the handler's ends-turn stamp the same way the runner's does. A test covers both outcomes
   and a resolution against an already-resolved call being a no-op.

App:

3. `send_message` and `reply_message` exist, carry the admission macro at member authority,
   append one `outgoing_message` block each recording `call_block`, and return pending. A
   re-run of the same call appends nothing and returns pending again. Tests read the
   definitions, the appended block, and the idempotent re-run.
4. The outbound edge delivers an outgoing block and never an assistant text block; the
   delivery receipt records `answer_block` as the outgoing block and resolves the pending
   call with the platform ids; a failed send fails the call with the reason; a cut-short send
   fails it with a sentence carrying the ids that posted. Spine tests cover a send, a reply,
   a failed send, a cut-short send, and a turn whose text is non-empty and delivers nothing.
   A startup test: an outgoing block with an unresolved call at process start is failed with
   the restart sentence before serving and is never delivered; a retire test: a pending send
   in a retired conversation is failed with the retirement sentence.
5. `reply_to` accepts any id the ledger holds — a member message by origin or revision, a
   join notice, an assistant delivered id — and refuses an id it does not hold with the
   recorded sentence, `Refusal::Failed`. Tests cover each accepted kind and the refusal.
6. `answer_target` and the derived threading no longer exist; a grep returns nothing.
7. The leaked-reasoning cut narrows the wire text of an outgoing block and leaves its stored
   text untouched; the disclosure line is composed into the stored text once; the
   reply-command protection sends such a text unthreaded. Tests cover each on an outgoing
   block, asserting the stored text after each.
8. The envelope renders exactly as specified for a plain message, a message without a
   stored speaker, a revision (`edited: true` under the revised id), an erased row (marker
   only), and a join notice; the `date` is the stored `sent_at` byte for byte. A test per
   shape, and a test that a member message carrying a `---` line renders without breaking
   the envelope.
9. PROJECTION EQUIVALENCE: a committed fixture database at
   `crates/core/tests/fixtures/previous-build.sqlite`, generated once by a committed example
   binary run at the previous build's commit (`4d56841`) and carrying real member rows with
   speaker, origin, revision, erasure and `sent_at`, opens under this build's migrations and
   renders byte-identical to the same messages freshly ingested by this build. The test
   asserts the equality row by row and asserts the fixture's recorded domain version
   predates this unit, so a regenerated fixture cannot silently become a new-build one. The
   example binary and the generating command are committed beside the fixture.
10. The `contract_notice` block is appended exactly once per conversation, in the same act
    as a tool-choice DELTA whose prior choice lacked the two tools, projects in the system
    voice with the recorded sentence, and never appears in a conversation whose first
    recorded choice already names them. Tests cover an old conversation's first activity, a
    fresh conversation, and a second activity appending no second notice.
11. The caps: 5 per minute, 30 per hour, 100 per day per conversation, shared across both
    tools, counted over outgoing blocks that delivered or are pending — a failed send counts
    for nothing; the sixth send in a minute is refused
    `Refusal::Refused` with the recorded sentence naming the tier and its reopening; the
    tiers are checked once, from the loaded ledger inside the admission answer, and no
    second check or filing lock exists. Tests cover each tier's edge and the sentence, and
    one test drives the registered tools through the framework's runner against a real
    store: five calls in one minute file, the sixth is refused with the sentence.
12. The typing cue lights on a sending tool's call-start status and stops when the send is
    done: on a delivered, a failed and a cut-short send through the receipt door, and on a
    refusal before filing; a text delta no longer lights it. Tests cover the light, each
    stop, and the text delta.
13. `COUNTED_DEBT_SQL` counts a debt when its turn holds an outgoing block that delivered,
    and not otherwise; the per-person and per-channel budgets read it. Tests cover a turn of
    notes only, a turn with a delivered send, and a turn whose only send failed.
14. Quoting, the take-back command, the erasure scrub and the retraction fork all reach an
    outgoing block as they reached an answer. One test each.
15. The teaching contains the contract sentences, no sentence presupposing relayed text (a
    test asserts the removed sentences are gone and the rewritten ones present), the
    never-announce sentence stays, and the two turn-ending tools' descriptions no longer
    restate pre-call posting. The docs test moves with it.
16. Every check runs clean in both repositories: `cargo fmt --all -- --check`, `cargo clippy
    --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and
    `cargo doc --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

Framework, the order and the pairing:

17. `ToolHandler::runs_in_order` exists with default false. A ready call of an in-order tool
    is parked while an earlier in-order call of the same conversation is unresolved and runs
    once that call resolves, in call order; calls of every other tool stay parallel. A test
    issues three in-order calls in one round with the first body held and asserts the second
    body starts only after the first resolves and the third after the second; a test asserts
    a parallel tool's call beside them is not held.
18. The tool result and tool error kinds carry the call's block id, and `ToolCall::resolved_in`
    and the outcome reading beside it pair by it, never by the provider echo. A test records
    two calls carrying one echo and asserts each resolves only through its own result; a test
    asserts a legacy resolution row carrying no call block id (if the schema admits one) is
    handled as the framework decides and states, never by the echo.

App, the order and the pairing:

19. Both sending tools answer `runs_in_order` true; the app holds no filing lock and no
    pairing walk of its own: `outgoing::send_state`'s echo comparison is gone and the send
    state is read through the framework's outcome reading. A grep of the app for a
    `tool_call_id` comparison returns nothing.

## Rejected alternatives

- **A second cap check under a filing lock.** The review found the admission check alone
  lets parallel sibling calls pass the cap together. Rejected as the fix: it answers one
  question twice, and it leaves the messages' order to whichever body files first. The order
  is the fix, and with it the admission check is exact.
- **Ordering the sends inside the app by reading the ledger for unresolved earlier calls.**
  Rejected: the runner already knows which call is ready and which are unresolved; a second
  reading of the same fact in the consumer is the fact recorded twice.
- **Deleting the failure stop and relying on the adapter's refresher stop.** Proposed by the
  cold-alternatives seat. Rejected by the user's answer: the cue stops when the send is done
  whichever way it ended, which makes the receipt door the one carrier for every outcome.
- **The contract notice as a standing prompt sentence.** Proposed by the same seat. Rejected:
  the decision on record is a recorded notice at the moment the tools joined an old
  conversation's choice, and a standing sentence would tell a fresh conversation about a
  history it does not have.

- **Rendering old answers under their delivery id.** Rejected: the projection is per-block
  with no ledger access, and receipts begin on 2026-08-30, so the envelope would be present
  for some old answers and absent for others with no way to tell the reader which. The
  contract notice draws one line at the recorded moment of the change.
- **Forking old conversations onto the new contract.** Rejected: the startup fork already
  carries the prompt; a second fork for a notice that is one appended block is machinery
  for a fact the ledger can simply record.
- **Sending from inside the tool body.** Rejected: a tool that talks to the adapter puts
  platform delivery inside the core's tool path; the mark's shape — append a block, let the
  edge deliver — keeps the core platform-free and gives the send its receipt.
- **Completing the call before the send.** Rejected: "sent" would be a lie when the platform
  refuses, and the model would not learn the id it needs to reply to its own message. Pending
  is the framework's native form for a call a backing system settles.
- **A framework multi-tier per-tool window.** Rejected for this unit: the bound is shared
  across two names and read per conversation; the admission answer over the loaded ledger
  expresses it in one place without widening a framework type for one consumer.
- **Silently sending plain on an unknown `reply_to`.** Rejected: it hides a hallucinated id
  from the model; the refusal is the honest answer and costs one round.

## Decisions on record

The user's words, verbatim.

**2026-09-01, the idea (msg 1669):** "Let's remove this reply to mechanic, and allow the model
to decide on its own who to reply to. It sees all the messages as usual but my idea is:
Instead of relaying text output to the group, stop relaying it, and introduce a send message
tool and a reply message tool with a reply to parameter that allows replying to a specific
message. This way the bot can also reply to multiple messages in a single turn or not reply
to any at all without being forced to keep its mouth shut on non important messages."

**2026-09-01, the envelope and the notes (msg 1671):** "The syntax could be yaml just like
harness envelopes --- from: @handle date:2026-08-31T22:33:46Z msgid: themsgidfromtg --- The
actual message. Then the model can just use the id. Yes internal notes. The model is then
allowed to yap as much as it wants. We just tell in the system prompt that its text output
never goes anywhere. But that also means we should either convert existing sessions via
custom fork on load or inject a block that tells the model that the format changed."

**2026-09-01, the worry (msg 1673):** "what i mean are the assistant messages · the model
might get confused and think that its messages never reached the group"

**2026-09-01, the caps (msg 1675):** "The message cap is 5 per 1 minute, and then another one
30 per 1 hour, and 100 per day (for busy days). On ratelimit the model gets a clear error
message and asks for a retry later. So it can resume responding on the next turn." — and,
asked per group or bot-wide (msg 1705): "Per group chat"

**2026-09-02, the order, the caps' home and the cue's stop (msg 1785):** "The message
sending needs to be serialized anyway because the messages must be in the intended order in
the group. · Q1: for now hardcoded, and configurable once the web ui hits. Record this · Q2:
it should stop typing when the send is done, regardless of its success. · Q3: go, take the
above into accounr" — Q1 was whether the caps stay constants or become configuration fields;
Q2 was whether the failure-only stop channel goes; Q3 the fix round.

**2026-09-01, the go (msg 1677):** "Alright then ensure through tests that the old db entries
work on the new format properly and are the same as the new entries on projection. Then you
can implement it"

**2026-09-01, platform dates (msg 1685):** "when a message was sent is tg's metadata and it
can be any time. The bot could be resumed at a later date or messages come in delayed. They
should say the real date as tg records it, not be made factually false by the framework.
What the framework and bot code records for its own bookkeeping of course has to use one and
the same clock."
