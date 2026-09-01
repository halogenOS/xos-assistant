# Unit 52 — the session records which tools it has

Date: 2026-09-01. A conversation's tool choice becomes a fact recorded in its own ledger, and
the framework reads it: at the dispatch that decides what a turn is offered, and at the runner
that decides what a tool call resolves against. An empty choice is the whole answer for a
conversation that has no tools, and the compaction fork records exactly that, written by the
library that owns the fork.

The repositories: the framework (`ronna-core`, checkout `~/projects/agent-ledger`, HEAD
`8f8a2d1`; paths below rooted at `crates/agent-ledger/`) and the app (this repo, HEAD
`713d68c`). Both change. The framework changes first; the app consumes it by path from the
sibling checkout.

## What is true today

Every claim was read from the two trees at the stated heads.

1. **The framework records no tool choice anywhere.** The `conversations` table carries `id`,
   `parent_id`, `model_id`, `reasoning`, `last_processed_block_id`, `last_processed_metadata_id`
   and `created_at` (`src/store/migrations.rs:87-97`). The only per-conversation settings are
   the model and the reasoning level.
2. **What a turn is offered is a hidden boolean on the block kind.** The dispatch reads
   `K::from_block(tail).offers_tools()` and sends either every registered definition or none
   (`src/actor.rs:1284-1311`). The hook defaults to `true` (`src/agency/mod.rs:199-201`) and
   exactly one kind answers `false`: `HarnessMessage` (`src/agency/harness_message.rs:62-64`),
   written from one place, `Store::fork_temporary` (`src/store/compaction.rs:206-215`), and
   reachable by no consumer.
3. **The runner resolves a call name against the whole process registry.** On a miss it hands
   the model `unknown tool: {name}. The registered tools are: {every registered name}` and
   records the outcome `Refusal::Failed` (`src/tools/runner.rs:495-519`). `Failed` counts
   toward no bound; only `Refusal::Refused` feeds the trailing refusal run that ends a turn
   (`src/agency/tool_call.rs:285-327`, limit read at `src/actor.rs:1396-1399`). So a turn shown
   no schemas invents a name, is handed the entire registry, and spends the rest of its rounds
   on it.
4. **The registry is process-wide.** One `ToolRunner` per application over one `ToolRegistry`,
   shared across conversations by `Arc` (`src/actor.rs:97-126`). Only the rate windows are
   folded per conversation.
5. **The runner already loads the conversation's ledger once per call**, before every check
   (`src/tools/runner.rs:455-465`), and holds the snapshot through the admission pass.
6. **The consumer's per-call check has no seam with ledger access.** `ToolHandler::gate`
   receives the input string alone — no store, no conversation id, no ledger
   (`src/tools/mod.rs:213-234`) — and its refusal is recorded `Refusal::Failed`
   (`src/tools/runner.rs:652-664`).
7. **The app implements the recorded tool choice one repository too high.** `ToolPalette`
   (`crates/core/src/tools/palette.rs`) is a consumer block kind carrying a JSON list of tool
   names in its own content table `block_tool_palette`. It is written at a channel's first
   contact (`crates/core/src/session.rs:389-400`), superseded on a registered-set delta by
   `reconcile_palette` (`crates/core/src/assembly.rs:2546-2581`, called from three
   channel-serving paths at `:923`, `:1419`, `:1553`), and written EMPTY into both temporary
   compaction forks (`crates/core/src/session.rs:575-585` and `:1401-1413`).
8. **The app enforces it by wrapping every handler.** `AdmittedTool`
   (`crates/core/src/tools/admission.rs`) loads the ledger a SECOND time inside every tool's
   `execute`, checks the newest palette, then checks the authority the tool requires against
   the turn's provenance reading, and returns `ToolOutcome::Refused` for either.
9. **That wrapper silently drops a framework capability.** It forwards six of the trait's seven
   methods by hand and omits `ends_turn` (`crates/core/src/tools/admission.rs:166-211`), which
   the framework's own trait documentation names as the failure this shape produces
   (`src/tools/mod.rs:204-211`). No app tool declares `ends_turn` today, so the omission is
   currently inert; the first one that does would compile and never fire.
10. **A compacted thread carries no tool record.** `open_compacted_thread` writes the system
    prompt, the ancestor reference and the compaction message, then the source's junction rows
    past the cut (`src/store/compaction.rs:236-241`); the palette sits before the cut and does
    not come across, and nothing writes one into the new thread. The thread is covered only
    because `reconcile_palette` runs on the three channel-serving paths before any turn.
11. **A registered descriptor cannot be withdrawn.** `check_registry` runs before the
    consumer's migrations and fails the open with `StoreError::MissingDescriptors` when a table
    the database registered is absent from the supplied set (`src/store/descriptors.rs:843-880`).
    A consumer that stops declaring a kind cannot reopen its database.

## The design

**The recorded choice.** A framework block kind, stored string `tool_choice`, carries the list
of tool names a conversation has. It is a library kind, stored in the library's own table
`block_tool_choice` through an appended core migration step and read through the blocks query,
the same way the other nineteen library kinds are. It projects nothing to the model and awaits
nobody. The newest one in a conversation's ledger speaks; a later one supersedes an earlier one
by being appended.

**What a turn is offered.** The dispatch reads the newest recorded choice for the conversation
and offers exactly the definitions those names resolve to. An empty list offers nothing. No
recorded choice at all offers the registry, which is what every conversation is offered today.

**What a call resolves against.** The runner resolves a call name against the same recorded
choice instead of the whole registry. A name the choice does not carry does not reach a handler.
The sentence the model reads names the tools THIS conversation has, never the process registry,
and the outcome's classification follows from whether a next round could succeed: with names to
offer it is `Refusal::Failed`, because the model can correct itself; with none it is
`Refusal::Refused`, because nothing it calls can resolve, and a run of those ends the turn.

**The compaction.** `fork_temporary` records the empty choice into the temporary conversation
itself, before the instructions block. The library owns that fork, so the library states its
choice; no consumer supplies it and no consumer can forget it. `open_compacted_thread` carries
the source conversation's newest recorded choice into the new thread, because a compacted thread
continues the same session.

**The hook that goes away.** `Agency::offers_tools` is deleted, along with its `HarnessMessage`
override. The fact it carried is now data in the ledger, stated once, read by both the dispatch
and the runner.

**The consumer's own admission.** `ToolHandler` gains one hook, consulted inside the runner's
admission pass over the snapshot that pass already loaded: it receives the tool context and the
ledger, and answers admit or refuse with a sentence. A refusal is recorded `Refusal::Refused`.
`gate` keeps its present meaning, the human's clearance, and is unchanged.

**The app.** `ToolPalette`, `newest_tools`, `reconcile_palette` and `AdmittedTool` are deleted.
The app records the framework's `tool_choice` at a channel's first contact and appends a fresh
one when the registered set differs, on the same three paths as today. The authority check of
decision 0043 moves into the new hook, keeps its wording and its `Refusal::Refused`
classification, and reads the ledger the runner already loaded instead of loading a second one.
The two palette declines are deleted: the framework now refuses a tool the conversation does not
have, before any handler is reached. The app's `block_tool_palette` table and its descriptor are
withdrawn.

**Withdrawing a descriptor.** The framework gains a stated way for a consumer to withdraw a
content descriptor: the withdrawal is declared with the descriptor set at open, `check_registry`
accepts the absence of a withdrawn table, and the domain migration that drops the table also
clears its registry row. Without this the app cannot delete its palette kind, because a database
that registered it refuses to reopen without it.

## Acceptance criteria

Framework:

1. A block kind with stored string `tool_choice` exists, carries a list of tool names, is stored
   in a library table created by an appended core migration step, and is read back through the
   blocks query with its names intact. It projects nothing to the model and awaits nobody.
2. `Store` exposes a way to append a tool choice to a conversation and a way to read the newest
   one, both covered by tests.
3. The dispatch offers exactly the definitions named by the conversation's newest recorded
   choice. A test asserts a two-name choice against a three-tool registry offers two.
4. An empty recorded choice offers no definitions. A test asserts it.
5. A conversation with no recorded choice is offered every registered definition, unchanged from
   today. A test asserts it.
6. The runner resolves a call name against the conversation's newest recorded choice. A name the
   choice does not carry never reaches its handler, even when the registry holds it. A test
   asserts the handler body did not run.
7. The sentence the model reads on an unresolved name lists the tools the CONVERSATION has and
   no others. A test asserts a name outside a two-name choice yields a sentence naming those two
   and not the third registered tool.
8. An unresolved name in a conversation whose choice is empty yields a sentence naming no tool
   and is recorded `Refusal::Refused`. A test reads the classification off the stored row.
9. An unresolved name in a conversation that HAS tools is recorded `Refusal::Failed`, unchanged
   from today. A test reads the classification off the stored row.
10. Consecutive unresolved calls in a conversation with an empty choice reach the forced turn end
    at the configured consecutive limit. A test asserts the turn stands down.
11. `Agency::offers_tools` no longer exists, and neither does the `HarnessMessage` override or
    the tests that read it. A grep for the identifier across both repositories returns nothing.
12. `fork_temporary` records an empty tool choice into the temporary conversation, before the
    instructions block, with no consumer input. A test reads the temporary conversation's ledger
    and finds it.
13. `open_compacted_thread` records the source conversation's newest tool choice into the new
    thread. A test asserts the new thread's newest choice equals the source's.
14. `ToolHandler` has one hook that receives the tool context and the ledger snapshot the
    admission pass loaded, and answers admit or refuse with a sentence. Its refusal resolves the
    call with that sentence and the classification `Refusal::Refused`, and the handler body does
    not run. A test asserts all three.
15. The admission pass loads the conversation's ledger exactly once per call, and the new hook
    receives that snapshot. A test with a counting store asserts one load.
16. A consumer can withdraw a content descriptor: a database that registered a table reopens
    after the consumer declares that table withdrawn and its domain migration drops it, and the
    registry row is gone afterwards. A test opens, writes, withdraws, reopens and asserts.
17. Withdrawing a table that was never registered, and reopening twice after a withdrawal, both
    succeed unchanged. A test asserts both.

App:

18. `ToolPalette`, its content table, its descriptor, `newest_tools`, `reconcile_palette` and
    `AdmittedTool` no longer exist. A grep for each identifier returns nothing.
19. A channel's first contact records a framework tool choice naming exactly the registered set,
    in the position the palette held. A test reads the created conversation's ledger.
20. A registered-set delta appends a fresh tool choice on the same three paths that reconciled
    the palette, once per process per conversation, with the same memory bound. Tests cover each
    path.
21. Neither compaction fork writes a tool choice of its own: the framework's fork writes the
    empty one. A test asserts the temporary conversation carries exactly one, and it is empty.
22. The authority check of decision 0043 runs through the framework's new hook, refuses with the
    same sentence today's `authority_decline` produces, byte for byte, and is recorded
    `Refusal::Refused`. A test asserts the sentence and the classification.
23. The two palette declines are gone, and a tool the conversation does not have is refused by
    the framework before the handler. A test asserts the handler body did not run and the
    sentence names the conversation's own tools.
24. A domain migration drops `block_tool_palette` and its registry row, and a database written
    by the previous build opens, serves and keeps its message history. A test opens a fixture
    database carrying palette rows, migrates, and reads the conversations back.
25. Every existing app test that named the palette either names the tool choice or is deleted
    with its subject. No test asserts a behaviour this unit removed.

Both:

26. Every check runs clean in both repositories: `cargo fmt --all -- --check`, `cargo clippy
    --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and
    `cargo doc --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

## Open decisions

Both are marked OPEN and neither is built on until decided. Each names what it forbids.

**OPEN-1 — a conversation with no recorded choice.** The design above offers it the registry,
which is today's behaviour for every conversation and every framework consumer. The alternative
is to offer it nothing, which is today's APP behaviour: a conversation with no palette admits no
tool. Offering the registry forbids the app's present protection, that a conversation whose
record was never written cannot reach a tool. Offering nothing forbids a consumer from using
tools without recording a choice, and changes what every existing framework conversation is
offered.

**OPEN-2 — withdrawing the app's palette table.** The design above adds descriptor withdrawal to
the framework so the app can delete its kind. The alternative is to leave `block_tool_palette`
registered and its descriptor declared, written by nothing, so the database keeps opening.
Leaving it forbids the deletion this unit exists to make, and leaves a dead kind in a public
repository. Adding withdrawal enlarges the unit by one framework capability.

## Rejected alternatives

- **A column on the `conversations` row.** Rejected: the choice is a decision the session made
  and a decision belongs in the ledger, where it is dated, superseded by appending, and carried
  by a fork the same way every other block is. A column would be mutable state beside an
  append-only record, and a fork would have to copy it by hand.
- **Keeping `offers_tools` and adding the recorded choice beside it.** Rejected: two answers to
  one question, one hidden on a kind and one in the ledger, and nothing keeping them agreed.
- **A compaction-specific block kind that disables tools.** Rejected: the general form covers it
  exactly, an empty list, and a specific one would put a consumer's word for a situation into a
  mechanism that must not know it. The empty list also covers a consumer that wants a toolless
  conversation for its own reasons.
- **Filtering the definitions at the dispatch but leaving the runner resolving against the
  registry.** Rejected: it fixes what the model is SHOWN and leaves what it is TOLD, which is
  the defect — an invented name still draws the whole registry back.
- **Moving the authority check into `gate`.** Rejected: `gate` is the human's clearance check,
  runs only for a handler that declares itself checked and only while no request stands, and its
  refusal is recorded `Refusal::Failed`, which counts toward no bound. Unit 51 made the app's
  decline `Refusal::Refused` on purpose so a run of them ends the turn.
- **Leaving the authority check as a wrapper around every handler.** Rejected: the wrapper is
  what this unit deletes. It loads the ledger a second time per call and it silently drops any
  trait method added after it was written.
- **Deleting the app's palette without withdrawing its descriptor.** Rejected: the production
  database registered the table, and `check_registry` refuses to reopen without it. The app
  would not start.

## Decisions on record

The user's words, verbatim.

**2026-09-01, on the architecture.** "This is asking for an architecture solution. The hollow
lattice approach. You're trying to rush something past me and bolting on random shit. Dont do
that. Architecture it well. This has one simple solution that doesnt need all your yapping:

Let the session choose its own tools. Record that decision in the ledger. The compaction is its
own session type. It is a new session anyway because of how we reuse only half the blocks. So
the framework just reads off the ledger and sees that the choice was made to have no tools
because the array is empty. And since the compaction is its own type of thing, the framework
knows and responds accordingly. It could even just be a compaction type block that disables
tools entirely, and thus the framework answers accordingly."

**2026-09-01, on the shape of the work.** "Delete and move into framework, CLEANLY, and
MODULARLY, ROBUSTLY and respecting all the rules."

**2026-09-01, on the compaction turn's tools.** The compaction turn is offered nothing and
admits nothing; the requirement stands from unit 51.

**2026-08-27, decision 0043.** The authority a tool requires is enforced at the call, against the
turn's provenance reading, and never at registration.
