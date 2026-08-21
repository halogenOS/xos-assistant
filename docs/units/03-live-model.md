# Unit 3 — the live model

Date: 2026-08-22. Revision 2, rewritten after the unbriefed two-reviewer probe returned
twenty-five findings against revision 1, three of them blockers proven against the
framework's code: the framework's OpenRouter module persists its key into the store,
the tail-only frontier lets an unaddressed message cancel a pending addressed one, and
nothing in scope ever unlatched an error-latched conversation. Status: settled for
implementation.

Units 1 and 2 are merged. This unit makes the assistant answer with a real model and
gives the project a runnable process — while keeping every test network-free.

## What this unit carries (from the earlier units' records)

1. **Erased blocks and role alternation** (decision 0012, OPEN) — closed here by
   normalization, not by a live probe (see the decision below).
2. **Erasure against an open stream** (decision 0012, OPEN) — ordered here.
3. **Terminal versus transient classification** (unit 2 record) — moved onto the
   core's error type here.
4. **User-facing failure behavior** (stage plan) — the failure notice, here.

## Decisions taken with this unit

- **OpenRouter through the framework's provider, with the key held in memory,
  2026-08-22.** The framework's OpenRouter module persists provider configuration —
  key included — into the store; the unit's secrets rule forbids that. The assembly
  therefore registers a thin provider wrapper whose configuration lives in process
  memory and whose persistence hooks are inert, delegating the wire entirely to the
  framework's OpenRouter binding. This is not the rejected bespoke provider: no wire
  code is duplicated, only the configuration's residence changes. The acceptance
  scan asserts the key is absent from the store file. Recorded for the framework's
  improvements list: an in-memory provider-configuration seam, so consumers stop
  needing wrappers. Rejected: relaxing the secrets rule to admit the store (the
  store file is long-lived, backed up, and outlives any key rotation).
- **Addressing is translated by the adapter, decided at the write, 2026-08-22.**
  What "addressed" means on a platform — a direct chat, a mention of the bot's
  username, a reply to one of the assistant's messages — is platform knowledge; the
  adapter resolves it (bot identity from `getMe`, fetched before the first poll with
  the poll's own backoff, no message translated before it succeeds) and the inbound
  message carries the neutral fact. The kind stores it. Because the framework owes a
  turn from the newest block alone, the stored fact that drives the turn is stamped
  by the entry point at the write: a message's answer-due fact is true when the
  message is addressed or when the block behind it carries an unanswered
  answer-due — so an unaddressed message arriving on the heels of an addressed one
  propagates the debt instead of cancelling it. The stamp is a decision recorded
  once at insert, in the access-model tradition of provenance stamps; it is not a
  derivable-fact column, because the per-block hook that consumes it cannot fold
  history. Both columns are structure, not personal data — erasure leaves them.
  This replaces the unit-1 stopgap (decision 0007) where every message awaited.
  Rejected: the core parsing mentions (platform vocabulary in the core); the
  adapter dropping unaddressed messages (record all, answer some — the group's
  memory is the product); a framework change to fold history at the frontier (the
  write-time stamp expresses the policy in the seams that exist).
- **Re-engagement is the next addressed message, 2026-08-22.** A stream error
  latches the conversation (the framework's rule). The recovery surface is the
  ingestion path itself: an addressed message always emits the unlatch intent — a
  person addressing the assistant IS the deliberate re-engagement — and an
  unaddressed message never does. This also retires the per-process unlatched set
  entirely (and with it the id-reuse bookkeeping): the intent is idempotent, so
  emitting it on every addressed write is one decision in one place. Against an
  empty wallet each addressed message costs one refused pre-stream attempt, which
  spends nothing. Rejected: an operator unpause surface (right for unit 4's
  toolset, wrong as the only recovery); unlatch-at-restart only (turns every
  transient provider error into an outage until someone restarts the process).
- **The system prompt lives in its own files, pinned per conversation,
  2026-08-22.** The prompt is prompt files in the repository under a named
  directory; the assembly loads them at start and records them through the
  framework's system-prompt kind at each conversation's creation (the mapping
  winner only). The framework records a conversation's prompt exactly once, so an
  edited prompt reaches new conversations only. OPEN, surfaced to the framework's
  improvements list: a long-lived group conversation never receives a prompt
  update; the superseding-prompt block is framework work. Rejected: a constant in
  code (prose in code, and a prompt edit becomes a code change); deployment-
  supplied prompt files (the assistant's voice belongs to its public repository).
- **Configuration is one file the process reads, 2026-08-22.** TOML, located by the
  binary's single command-line argument. It names: the store path, the Telegram
  state-file path, the prompt directory, the log destination, the model id, and the
  endpoint overrides (Telegram API root, OpenRouter base URL — both defaulting to
  the real hosts; tests point them at loopback servers). Secrets — the bot token
  and the OpenRouter key — are named indirectly: an environment variable name or a
  file path per secret. Secrets never appear in the configuration file, the store,
  or any tracked file. Rejected: flags (a growing surface wrapped in a script
  anyway); environment-only configuration (paths and model choice deserve a
  reviewable file).
- **A failed turn tells the chat once, at most once, 2026-08-22.** On a stream
  error the conversation latches and the edge yields a failure notice — marked as a
  notice, one per failed turn, no model prose — and the adapter sends one short
  plain line. The notice is derived from the bus event, and the bus is lossy: the
  notice is at-most-once by construction, stated here plainly — a lagged edge may
  drop it, and a late error from a torn-down predecessor stream may produce a
  spurious one; both are accepted for a courtesy line. The durable record of failed
  turns is framework work, already on the improvements list as recording dead
  turns. One uniform notice text: no distinct budget wording, because the wire
  flattens the refusal to prose before the core sees it — the latch already stops
  spend, which is the substance. Rejected: silent failure (the status quo, now
  user-visible); blind automatic retry (spends without consent and can loop); a
  notice classified by string-matching provider prose (a coupling to wording
  nobody owns).
- **The last in-place schema edit, 2026-08-22.** No durable store predates this
  unit's binary, so the shipped CREATE TABLE gains the two addressing columns by
  direct edit one final time. From this unit on — the first deployable process —
  every schema change is an appended, versioned migration step. Rejected: starting
  the append-only migration discipline one unit early (a ceremony for stores that
  cannot exist).

## The unit's contract

### The addressing seam

The inbound message gains the addressed fact; the kind's content table gains
`addressed` and `answer_due` (both booleans, structure not personal data); the entry
point stamps `answer_due` per the decision above, reading only the tail block; the
awaiting hook reads `answer_due`. An addressed message also always emits the unlatch
intent; the per-process unlatched set and its erasure bookkeeping are removed. The
mid-turn absorption semantics are unchanged.

### The provider wiring

The assembly accepts a provider registration whose configuration lives in memory
(key, base URL, model), delegating the wire to the framework's OpenRouter module;
its persistence hooks are inert. The scripted-provider test path stays as it is; the
new seams are tested over scripted providers. One assembly-level test drives the
real OpenRouter module against a scripted OpenRouter-shaped server on the loopback
interface, patterned on the adapter's Telegram loopback server — the loopback
property is the test's own base-URL configuration, and the test asserts its server
was actually hit.

### Role alternation, closed by normalization

An erased message currently ends the projection's contiguous run. The unit closes
0012's first OPEN deterministically, model-independently and network-free: the
projected request never carries two same-role messages in a row and never opens
with the assistant's voice. The preferred seam is the kind's own projection shape,
if a probe of the framework's fold shows a shape that preserves run continuity
while contributing nothing; if no kind-level shape can express it, the minimal fold
amendment in the framework is authorized for exactly this, follows that
repository's own rules, and is recorded there. Either way the outcome is pinned by
tests on the projected request and recorded in the decision record that closes the
OPEN. The live-probe branch of revision 1 is dropped: wire acceptance is a
per-model fact and cannot evidence a configurable binding.

### Erasure and open streams

The assembly tracks per-conversation streaming state from the bus events it already
consumes. Erasing a principal whose direct conversation shows an open stream emits
the interrupt, awaits the stream-closed signal for that conversation, then confirms
settle with a bounded re-read (no streaming tail) before deleting; a conversation
with no observed open stream is erased directly. The bound is a named constant; on
timeout the erasure fails loudly and deletes nothing. The interrupt's own status
append racing the delete is closed by the settle re-read ordering. Pinned by a test
that erases during a held scripted stream, and one that erases an idle principal
without paying any wait.

### Error classification

The core's error type states terminal-or-transient for the message that caused it;
the adapter's batch discipline reads the statement, not variant names. Conservative:
only provably deterministic errors are terminal.

### The runnable process

A new binary crate (`crates/assistant`, workspace member) assembles the pieces:
read the TOML configuration, resolve secrets, open the store with the schema, load
the prompt files, start the assembly with the wrapped OpenRouter provider, run the
Telegram adapter. It logs startup facts (never secrets), stops on SIGTERM — the
run future is selected against the signal and abandoned, with an in-flight send
possibly cut short, stated here as accepted — and exits nonzero on a configuration
it cannot read. Deployment wiring stays outside the repository; the group-privacy
platform setting the record-all policy depends on is an operational fact for the
untracked local notes, referenced here only as a dependency.

### Scope fence

No rate protection, no tools, no spam detection (unit 4). Framework changes only if
the role-alternation seam forces the fold amendment, scoped to exactly that. The
Telegram adapter changes only for: the addressing seam (getMe, mention and
reply-to-self resolution), the failure-notice send, the classification read, and
the shutdown seam the binary's stop requires.

## Acceptance criteria

- **AC1** The workspace builds with the OpenRouter feature enabled; the suite passes
  in parallel and single-threaded identically; every test's traffic stays on
  loopback by its own endpoint configuration.
- **AC2** Addressing end to end over the adapter and a scripted provider: a direct
  message, a mention, and a reply-to-assistant each get answered; an unaddressed
  group message is recorded, rests, and appears in the next turn's context; an
  unaddressed message arriving after an addressed one before the turn fires does
  not cancel the answer — each pinned, with the getMe-before-first-poll fixture.
- **AC3** The system prompt: loaded from the prompt files, recorded at conversation
  creation through the framework's system-prompt kind, present in the first turn's
  projected request — pinned.
- **AC4** The failure notice: a scripted stream error yields exactly one notice on
  the edge, marked as a notice; the adapter sends the plain line; the next
  addressed message unlatches and the conversation answers again — pinned end to
  end, with the at-most-once nature stated in the test doc.
- **AC5** Role alternation: the projected request of a ledger holding the two
  erased shapes carries no same-role adjacency and no leading assistant message —
  pinned by tests; the closing decision record states the seam that achieved it.
- **AC6** Erasure during a held stream interrupts, settles, then deletes; the
  timeout path fails loudly deleting nothing; an idle erasure pays no wait — all
  pinned.
- **AC7** Classification: the adapter names no core error variants; a terminal
  refusal and a transient failure drive the two batch outcomes through the stated
  classification — pinned.
- **AC8** The binary: starts against a configuration file pointing at scripted
  loopback endpoints and answers a message; refuses a malformed configuration with
  a nonzero exit; exits cleanly within a stated bound on SIGTERM; never logs a
  secret, and the key is absent from the store file — all pinned by process-level
  tests extending the token scan.
- **AC9** Clippy denied-warnings, fmt, doc, the vocabulary scan and the token scan
  are clean; every new dependency is recorded before a manifest names it.
- **AC10** The decisions above are recorded, dated, with rejected alternatives; the
  0012 OPEN items this unit closes are marked closed with pointers; the framework
  improvement items this unit surfaced (in-memory provider configuration, durable
  failed-turn record, superseding system prompt) are recorded on the improvements
  list.
