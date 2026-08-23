# Unit 5 — the tools, admitted

Date: 2026-08-22. Revision 2, rewritten after the unbriefed two-reviewer probe returned
twenty findings against revision 1, including two facts about the real world revision 1
had wrong: the project site is a static application with no wiki and no server-side
search, and the project runs two forges with different API dialects — the canonical
self-hosted one, and a mirror that is nonetheless the only home of releases. The
framework probes were equally decisive: no palette surface, no context-bearing
admission gate, no consumer fact on tool inserts. Every mechanism below is stated
against what exists. Status: settled for implementation.

## The stage, re-sliced again

Unit 5 was tools plus spam reporting; split 2026-08-22. The tool layer and the spam
policy are separable concerns; unit 6 is spam detection and reporting, the last build
unit.

## Decisions taken with this unit

- **Two lookups, not three; the wiki tool waits for a wiki, 2026-08-22.** The project
  site is a static single-page application: no wiki, no server-side search, nothing a
  bounded GET can query. The third tool is therefore dropped from this unit rather
  than pointed at a backend that does not exist; when the project stands up a
  searchable docs backend, a wiki tool follows as its own small unit. Rejected:
  scraping the site bundle (client-side assets, hash-named, no stable contract);
  fetch-and-search over a fixed URL list (a hand-kept index that drifts, answering
  worse than the two real lookups).
- **Each lookup names its real backend and dialect, 2026-08-22.** Commit lookup
  speaks the Forgejo v1 API of the project's canonical self-hosted forge,
  unauthenticated (it answers so today, and commits are public data); its parameters
  are the repository name within the project organization and a commit hash or
  reference. Release lookup speaks the GitHub v3 API against the project's builds
  repository on the mirror organization — the only place releases exist, and the
  data the project site itself reads; its parameter is an optional tag, defaulting
  to the latest release. Two base URLs in configuration with those real hosts as
  defaults; one HTTP client, one decoder per dialect. An optional mirror API token
  (a secret, referenced indirectly) raises the mirror's rate limit from sixty to
  five thousand requests per hour; the canonical forge needs none. Rejected: one
  forge for both tools (releases exist only on the mirror; the canonical forge is
  the truth for code); shelling out to clones (the APIs answer).
- **Tools are product behavior and live in the core, 2026-08-22.** The
  no-platform-vocabulary invariant bans chat-platform vocabulary; the project's own
  forge and releases are the product. The tools sit in the core's tool module tree.
  The core gains its first network dependency for them: the HTTP client at the
  framework's own major version — one HTTP stack, one TLS story across the
  workspace — recorded per the supply-chain rule before the manifest names it.
  Rejected: a separate tools crate (a boundary with no consumer behind it); tools
  in the adapter (the adapter translates, it does not act).
- **The palette is a consumer block kind, and it gates admission, not exposure,
  2026-08-22** (recording the 2026-08-20 fail-closed settlement against the
  framework as probed). The framework has no palette surface: tool definitions go
  to the model registry-wide on every turn, and nothing per-conversation filters
  them. The palette is therefore the assistant's own leaf kind — one durable block
  naming the admitted tools, written at every conversation's creation (direct and
  group alike) beside the system prompt, under the same winner-only rule; it
  projects nothing to the model and awaits nothing. No palette block means no
  tools. What the palette cannot do today is stated plainly: the model may still be
  OFFERED a tool the palette will decline, so the decline wording teaches the model
  not to retry. The per-conversation definitions filter joins the framework
  improvements list. Conversations created before this unit have no palette and
  admit nothing; no backfill — no production store predates deployment, so the
  case is a test fixture, not an operational path. Rejected: fail-open with
  run-level checks (an operator-session model; a public group is a different
  threat model); a registry-side filter in the assistant (the registry is
  framework machinery; the fact belongs on the ledger).
- **Admission is a consumer check at the top of execute, 2026-08-22.** The
  framework's admission chain offers no consumer seam with ledger access: its gate
  hook receives the input string alone, and tool-call inserts are framework-
  internal with fixed columns, so provenance cannot be stamped at insert. The
  mechanism that exists: one admission wrapper shared by every tool handler, whose
  execute first reads the palette block and resolves the provenance through the
  tool context's ledger access, and declines — returning the recorded tool error,
  no network touched — before the tool body runs. "Declined, never executed" means
  the tool's body; the wrapper is technically entered. Both missing seams join the
  framework improvements list: a context-bearing gate, and a consumer fact on the
  tool-call insert. Rejected: waiting for the framework seams (the gate would ship
  ungated); per-tool hand-written checks (one rule, one place — the wrapper).
- **Provenance is the minimum over the turn's chat run, 2026-08-22.** Mid-turn
  absorption makes "the frontier chat message at decision time" the wrong anchor:
  an absorbed message carries its own fresh stamp, and reading it could escalate a
  member-summoned turn to admin — the exact escalation 0036 forbids. The rule that
  cannot escalate: walking back from the tool call, the provenance is the MINIMUM
  authority over the contiguous chat messages since the previous assistant answer
  (the summoning message and everything absorbed since), reading each message's
  debt stamp with its sender authority as the pre-migration fold. The minimum can
  over-decline when a lower-authority bystander chimes in mid-turn — accepted and
  stated: a declined lookup is a degraded answer, an escalated tool is a broken
  access model. With two member-level tools this gate cannot decline today; it
  exists so unit 6's report tool inherits enforcement. Admitted calls record no
  provenance fact (the framework's tool blocks carry no consumer field — the
  improvements item covers the durable form); a decline records the reading in
  its recorded error text. Rejected: the decision-time frontier read (escalates
  under absorption); a consumer admission-record block per admitted call (ledger
  noise recording a derivable behavioral fact).
- **Tool failures speak to the model, not the chat, 2026-08-22.** A lookup that
  fails — network, rate limit, not found, timeout — returns its error as the tool
  result the model sees, and the model answers with what it has; the chat never
  receives a raw error. A turn where the model narrates before calling a tool
  sends both texts to the chat — both are the assistant speaking, and the platform
  already receives multi-message replies under the chunking rule; accepted and
  pinned. Timeouts are per-tool construction parameters with named-constant
  defaults, so tests construct short ones instead of waiting production bounds.
  Rejected: tool errors as failure notices (the notice is for a failed turn);
  suppressing pre-tool narration (an outbound rule against the assistant's own
  words).
- **Erasure does not reach tool blocks yet — recorded OPEN, 2026-08-22.** A tool
  call's input and result may quote a person's prose, and they live in framework-
  owned tables erasure cannot reach today. Accepted for this unit with its
  reasoning: the group tools are project lookups whose inputs are overwhelmingly
  technical, and the erasure write path for tool blocks is framework work — filed
  on the improvements list beside the other seams. OPEN, in the 0012 lineage,
  revisited when the framework offers the path.

## The unit's contract

### The tools

Each tool is one module: name, model-facing description, parameter schema, required
authority (member), and an execute performing one bounded HTTP GET against its
configured base URL, returning a compact result — commit: subject, author, date,
link; release: version, date, link, per-device assets summarized. Result-size
bounds are named constants; timeouts per the decision above. No tool writes
anywhere.

> Amended 2026-08-23 (unit 8, decision 0060): the no-write rule above gains its
> dated exception — a tool may append blocks of kinds that exist for tool-driven
> delivery, which the report tool's filed block is. Lookups still write nothing.

### The palette kind and registration

The new leaf kind composes into the existing enum through the derive, with its own
content table via the next appended migration step. The assembly registers the two
wrapped handlers and writes the palette block at conversation creation naming
exactly those tools. A conversation without a palette admits nothing — pinned via
a store-direct fixture modeling a pre-unit conversation.

### The admission wrapper

One wrapper type implementing the framework's handler trait around each tool:
palette read, provenance walk per the minimum rule, decline with a recorded error
naming the rule and the reading (worded so the model does not retry), else the
tool body. The unlatch, budget and notice rules are untouched: a turn that calls
tools is one turn and one answer-budget slot.

### Test support

The scripted provider grows a tool-call script: a first request answered with
tool-use events carrying non-empty input, a second request answered with text —
patterned on the framework's own tool-call event shapes. The scripted forge and
mirror servers follow the loopback pattern of the existing scripted wires.

### Configuration

Two base URLs (real-host defaults), the optional mirror token secret reference
joining the secret scans. Unknown keys refused.

### Scope fence

No spam detection, no report tool, no admin tools (unit 6). No framework changes;
the probed missing seams are improvements-list items, not blockers, per the
mechanisms above. The adapter is untouched.

## Acceptance criteria

- **AC1** The workspace builds; the suite passes in parallel and single-threaded
  identically; loopback-only traffic.
- **AC2** End to end over the adapter, the tool-scripted provider and the scripted
  forge: an addressed question makes the scripted model call the commit lookup,
  the tool executes against the loopback forge, the result reaches the model's
  second request, and the answer reaches the chat — asserted on the ledger block
  by block including the tool call and result blocks; a variant with narration
  text before the tool call delivers both texts to the chat.
- **AC3** Each tool pinned: happy path (scripted response decoded to the compact
  result) and failure paths (error status, and a timeout under a short
  constructed bound) return a tool error the model sees while the chat receives
  only the model's answer.
- **AC4** The palette fails closed: the store-direct pre-unit conversation admits
  no tool call (declined, recorded, the turn completes); a created conversation's
  palette block names exactly the two tools; direct and group conversations get
  the same palette — pinned.
- **AC5** (amended 2026-08-22, after the second verification refuted the
  ledger-shape walk) Admission enforces the palette; authority enforcement is
  structural: tool registration refuses any tool whose required authority is
  above member, pinned by a test that attempts to register an admin-level tool
  and asserts the refusal names the floor and decision 0043's closure. The
  refused registration provably never reaches the registry.
- **AC6** (amended likewise) The interim provenance walk is REMOVED, not
  fenced: no walk code remains, the stamp keeps unit 4's tail-only read, and
  the unit-4 protection suite passes unchanged. The framework's dispatch
  anchor is recorded as the mechanism that lifts the floor.
- **AC7** Configuration: base URLs default to the real hosts and override for
  tests; the optional token flows to the mirror requests' authorization header
  (absent token sends no header) and joins the secret scans including the
  process-level test.
- **AC8** Budgets and tools compose: a turn that calls tools consumes one answer
  slot; a limited message summons no tools — pinned.
- **AC9** Clippy denied-warnings, fmt, doc under denied warnings (workspace and
  per package), the vocabulary scan and all secret scans are clean; the HTTP
  client dependency is recorded before the manifest names it.
- **AC10** The decisions above are recorded, dated, with rejected alternatives;
  the re-slice and the erasure OPEN are recorded; the framework improvement items
  (context-bearing gate, consumer fact on tool inserts, palette-filtered
  definitions, erasure path for tool blocks) are on the improvements list.
