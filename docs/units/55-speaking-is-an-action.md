# Unit 55 — speaking is an action

Date: 2026-09-02. The assistant stops relaying the model's text to the group. A message reaches
the chat only through two tools — `send_message`, and `reply_message` with a `reply_to`
naming the message it answers — so one turn can post several messages, answer specific
messages, or post nothing, and the model's written text becomes its own private notes. Every
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
   (`crates/core/src/composing.rs:138-161`); the framework raises no status for a tool call
   starting.
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

**The relay ends.** The outbound edge no longer classifies an assistant text block as a
reply; the model's text is stored as today and delivered nowhere. `answer_target` and the
derived threading are deleted. What the relay carried moves onto the outgoing block: the
leaked-reasoning cut, the first-interaction disclosure line (composed into the outgoing
block's stored text before the send, idempotent, on the first message to a never-introduced
co-summoner), chunking (adapter, unchanged), and the reply-command protection (an outgoing
text carrying a reply-acted shape goes out unthreaded, the same rule at the same edge).

**The reply target.** `reply_to` must name an id the conversation's ledger holds — a member
message's origin or revision, a join notice, or one of the assistant's own delivered ids —
of any age. An id the ledger does not hold is refused with a sentence saying so
(`Refusal::Failed`: the model can correct it), never sent plain, because a silently dropped
thread hides a hallucinated id. The platform's own tolerance for a vanished target
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
- the tool-choice reconcile appends a choice naming the two tools on first activity, and at
  that same append — the ledger's own record of the moment the contract changed — the app
  appends one system-voiced `contract_notice` block stating that the assistant's written
  answers above it were posted to the group as they stand, and that from there on its text
  is private and a message reaches the group only through the two tools. This is the user's
  "inject a block that tells the model the format changed", written once per conversation
  at the exact boundary, as a stored fact;
- member messages re-render under the envelope everywhere, old and new alike, because
  rendering is a request-time projection of stored columns.

Old answers do NOT render under a delivery id: the projection is per-block with no ledger
access, and receipts do not exist before 2026-08-30, so an envelope built from them would be
right for some answers and silently absent for others. The notice draws one honest line
instead.

**The caps.** Per conversation, shared by both tools, three tiers: 5 sends per minute, 30
per hour, 100 per day, counted over the conversation's outgoing blocks in the trailing span.
The check runs in the tools' admission answer over the ledger the runner already loaded — the
framework's single-tier per-name window cannot express a shared three-tier bound. A spent
tier refuses with `Refusal::Refused` and a sentence naming the tier that is spent and when it
reopens, so the model stops and resumes on a later turn; a run of five such refusals ends the
turn by the framework's rule.

**The typing cue.** The framework raises a stream status when a tool call begins streaming,
carrying the tool's name; the app lights the cue for the two sending tools and stops it on
the send's resolution. Text deltas no longer light it, because text is notes.

**The budget.** A counted debt is one whose turn produced at least one outgoing message; the
empty-answer clause is replaced, not extended. A turn of notes and no send costs the person
nothing, exactly as an empty turn does today.

**Everything that handled an answer handles the outgoing block.** Quoting a reply to the
assistant's message reads the outgoing block's own text; the take-back command resolves
through the receipt's `answer_block` to the outgoing block and strips it from view as it
strips an answer today; the erasure scrub and retraction forks treat outgoing blocks as the
assistant's words; the compaction cut's rule that a tool lifecycle is never split already
covers a send.

**The teaching.** The system prompt states the contract plainly: written text is private
notes and never reaches the group; a message reaches the group only through the two tools;
silence is still the default, and a turn ending without a send posts nothing. Every sentence
that presupposed relayed text is rewritten: the silence sentences say "end the turn without
sending"; the heads-up before slow work is sent with `send_message`; unit 54's bare-call
prohibition and the two turn-ending tools' restatement of pre-call posting are removed
because the mechanism they warned about no longer exists; the never-announce sentence stays.
The moderation and reaction teaching keep their meaning with "answer" read as "send".

## Acceptance criteria

Framework:

1. A tool call that begins streaming raises a stream status carrying the tool's name, once
   per call, never for text or reasoning. A test asserts it and asserts the text status is
   unchanged.
2. A consumer can resolve a pending tool call out of band by its block id, completing it with
   a result or failing it with an error, through a public store door; the resolution carries
   the handler's ends-turn stamp the same way the runner's does. A test covers both outcomes
   and a resolution against an already-resolved call being a no-op.

App:

3. `send_message` and `reply_message` exist, carry the admission macro at member authority,
   append one `outgoing_message` block each, and return pending. A test reads the
   definitions and the appended block.
4. The outbound edge delivers an outgoing block and never an assistant text block; the
   delivery receipt records `answer_block` as the outgoing block and resolves the pending
   call with the platform ids; a failed send fails the call with the reason. Spine tests
   cover a send, a reply, a failed send, and a turn whose text is non-empty and delivers
   nothing.
5. `reply_to` accepts any id the ledger holds — a member message by origin or revision, a
   join notice, an assistant delivered id — and refuses an id it does not hold with the
   recorded sentence, `Refusal::Failed`. Tests cover each accepted kind and the refusal.
6. `answer_target` and the derived threading no longer exist; a grep returns nothing.
7. The leaked-reasoning cut, the disclosure line and the reply-command protection apply to
   the outgoing block's text; the disclosure line is composed into the stored block once.
   Tests cover each on an outgoing block.
8. The envelope renders exactly as specified for a plain message, a message without a
   stored speaker, a revision (`edited: true` under the revised id), an erased row (marker
   only), and a join notice; the `date` is the stored `sent_at` byte for byte. A test per
   shape, and a test that a member message carrying a `---` line renders without breaking
   the envelope.
9. PROJECTION EQUIVALENCE: a fixture database written by the previous build — real member
   rows with speaker, origin, revision, erasure and `sent_at` — renders, under the new
   format, byte-identical to the same messages freshly ingested by this build. A test
   asserts the equality row by row over the fixture.
10. The `contract_notice` block is appended exactly once per conversation, at the tool-choice
    append that first names the two tools, projects in the system voice with the recorded
    sentence, and never appears in a conversation created under this build. Tests cover an
    old conversation's first activity and a fresh conversation.
11. The caps: 5 per minute, 30 per hour, 100 per day per conversation, shared across both
    tools, counted over outgoing blocks; the sixth send in a minute is refused
    `Refusal::Refused` with the recorded sentence naming the tier and its reopening; the
    tiers are checked from the loaded ledger inside the admission answer. Tests cover each
    tier's edge and the sentence.
12. The typing cue lights on a sending tool's call start and stops on its resolution; a text
    delta no longer lights it. Tests cover both.
13. `COUNTED_DEBT_SQL` counts a debt when its turn holds an outgoing block and not otherwise;
    the per-person and per-channel budgets read it. Tests cover a turn of notes only and a
    turn with a send.
14. Quoting, the take-back command, the erasure scrub and the retraction fork all reach an
    outgoing block as they reached an answer. One test each.
15. The teaching contains the contract sentences, no sentence presupposing relayed text (a
    test asserts the removed sentences are gone and the rewritten ones present), the
    never-announce sentence stays, and the two turn-ending tools' descriptions no longer
    restate pre-call posting. The docs test moves with it.
16. Every check runs clean in both repositories: `cargo fmt --all -- --check`, `cargo clippy
    --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and
    `cargo doc --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

## Rejected alternatives

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

**2026-09-01, the go (msg 1677):** "Alright then ensure through tests that the old db entries
work on the new format properly and are the same as the new entries on projection. Then you
can implement it"

**2026-09-01, platform dates (msg 1685):** "when a message was sent is tg's metadata and it
can be any time. The bot could be resumed at a later date or messages come in delayed. They
should say the real date as tg records it, not be made factually false by the framework.
What the framework and bot code records for its own bookkeeping of course has to use one and
the same clock."
