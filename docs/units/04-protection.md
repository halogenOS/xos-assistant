# Unit 4 — protection

Date: 2026-08-22. Revision 2, rewritten after the unbriefed two-reviewer probe returned
twenty findings against revision 1, two of them blockers: the limited stamp as first
written cancelled an innocent sender's propagated debt, and the channel count cannot
run in the kind's content table alone. Both reviewers probed the schemas and query
plans; every resolution below is grounded in what the tables actually hold. Status:
settled for implementation.

Units 1 through 3 are merged. This unit keeps the assistant standing in a public
group: flood protection on the answering path, and the authority groundwork the tool
unit enforces against.

## The stage, re-sliced

The stage plan's fourth entry bundled protection, the one-authority turn, the feature
tools and spam reporting. Re-sliced 2026-08-22, because one-authority enforcement has
no observable effect until tool admission exists: unit 4 is protection plus the
recorded authority fact; unit 5 is the tools with admission (where enforcement bites)
and spam reporting.

## Decisions taken with this unit

- **Protection limits answering, never recording, 2026-08-22.** The record-all policy
  is the product; no protection mechanism may drop a message from the ledger. What a
  flood can exhaust is the answering path — model spend and group attention — so the
  limits act at the write, on the stamp. The budget refuses only the debt the message
  itself would open, never a debt it propagates: the stamp rule is answer-due =
  (addressed and not limited) or tail-owes. An over-limit addressed message is
  recorded with its addressed fact true, a limited fact naming the refusing budget,
  and an answer-due fact that still carries a propagated debt forward unchanged — a
  flooder can be refused their own answer but can never cancel someone else's. The
  limited fact reads as "this message's own debt was refused"; a true answer-due
  beside it is a propagated debt, not a contradiction. Rejected: dropping over-limit
  messages at the adapter (breaks record-all, pushes policy into the adapter);
  limiting at the provider (spends the turn the limit exists to save); a flat
  answer-due false on limited messages (cancels a propagated debt — the lost answer
  decision 0021 forbids).
- **The budget clock is the receipt time, 2026-08-22.** The window counts against the
  block header's creation time — assigned by the store at the write, unforgeable,
  never null, uniform across adapters — anchored at the stamp's own wall clock. The
  platform send time is rejected as the clock: it is platform-asserted (unforgeable
  on Telegram, where the server assigns it, but not guaranteed unforgeable on every
  future adapter — a federated platform's origin timestamps are peer-asserted), it
  is nullable under erasure, and a backlog replayed after downtime carries old send
  times that would open the window exactly when the queue is longest. With receipt
  time, a replayed backlog meets the budgets like live traffic. Rejected: the send
  time (above); a hybrid (two clocks is two answers to one question).
- **The counts join the framework's tables, sanctioned and recorded, 2026-08-22.**
  The content table carries neither the conversation id nor the receipt time; both
  live in the framework's own tables (the junction and the block header). The budget
  queries therefore join them by name from the kind's module — framework vocabulary
  is not platform vocabulary, and the no-vocabulary invariant binds the core against
  platforms, not against the library it consumes. The coupling is real and is
  recorded here with its risk: the framework does not contract those table names.
  Surfaced to the framework's improvements list: exported schema-name constants or a
  counting read seam, so consumers stop naming internals. Rejected: a conversation
  column on the content table (a second record of the junction's fact, drifting on
  fork); loading whole ledgers through the public read path per stamp (the exact
  full-materialization the outbound edge was already cured of).
- **Limits are derived from the ledger at the write, 2026-08-22.** No counter table,
  no in-memory tally: the entry point runs two bounded counts inside the existing
  stamp serialization. The counted predicate is messages that opened debt —
  addressed, not limited — younger than the window by receipt time, by principal
  globally (spend is global, so heavy direct-chat use and group use share one
  budget) and by conversation. Propagated stamps are not counted, and a multi-
  message turn still counts each opened debt: a debt opened is a spend intent, and
  the over-count against absorbed turns is accepted and stated. The appended
  migration adds the index the principal count runs on (principal id, addressed);
  the channel count rides the framework's existing junction index. Rejected: a
  counter table (a second record of a derivable fact, drifting on every erasure);
  in-memory counters (reset on restart, and unreadable in audits). The stamp
  serialization is in-process; single-process deployment is already the assembly's
  stated assumption.
- **Over-limit is silent in the chat, 2026-08-22.** A rate-limited addressed message
  draws no answer and no notice: a notice per flood message is a flood amplifier a
  hostile sender controls. The limited fact in the ledger is the audit trail; the
  behavior is documented in the repository. The unlatch intent follows the same
  line: only a message whose own debt is taken (addressed, not limited) is
  re-engagement per decision 0022 — a refused debt neither answers nor unlatches,
  so a limited flood cannot wake an error-latched conversation. Rejected: a notice
  per limited message (hands the flooder the assistant's voice); a one-per-window
  notice (still triggerable on schedule); limited messages unlatching (re-engagement
  by a message the budget just refused).
- **Budgets live in the configuration file, defaults stated here, 2026-08-22.**
  Defaults: principal, 6 answers per 600 seconds; channel, 20 answers per 600
  seconds. A window of zero disables that budget explicitly; a count of zero is
  refused at parse (an assistant configured to answer no one is a misconfiguration,
  not a policy). A partial protection table takes per-field defaults; unknown keys
  are refused like everywhere else in the file. Rejected: hardcoded budgets (a
  product knob in code); per-channel-kind budgets (direct chats end at the principal
  budget anyway); count-zero-as-disable (inverts the natural reading).
- **The turn's authority is stamped at the write, minimum rule, 2026-08-22.** Every
  message already records its sender's authority; this unit adds the debt's. A
  message with no owing tail opens its debt (if addressed and not limited) at its
  own authority. Whenever the tail owes, the minimum rule applies regardless of the
  incoming message's own addressed fact: the carried debt authority is the minimum
  of the tail's debt authority and the incoming sender's authority. The frontier
  message's debt authority is therefore the lowest authority that contributed to
  summoning the turn, recorded before the turn exists — the fact unit 5's tool
  admission reads: provenance stamped at insert, policy read live at admission. A
  pre-migration owing tail carries a null debt authority; the fold reads that as
  the tail's own stored sender authority, which every pre-migration row carries.
  Rejected: deriving the turn's authority at admission by folding history (the
  per-block hook cannot fold; admission reads one stamped fact); the maximum rule
  (a member's question riding an admin's debt would gain admin standing — the
  escalation the access model forbids).

## The unit's contract

### The limited stamp

The kind's content table gains two columns by the first appended migration step
(decision 0026's discipline): the limited fact (text, closed vocabulary `principal`
or `channel`, null when no budget refused) and the debt authority (text, the
authority vocabulary, null when no debt). Pre-existing rows read null in both. Both
are structure, not personal data — erasure leaves them. The stamp order: addressing
first; budgets only for addressed messages (principal before channel — the first
refusing budget names the limited fact); then answer-due per the composition rule;
then the debt authority per the minimum rule. The awaiting hook keeps reading
answer-due alone.

### The budget queries

Two counts in the kind's module through the domain seam, each joining the
framework's junction and block-header tables for the conversation id and the
receipt time, counting opened debts (addressed, not limited) younger than the
window, anchored at the stamp's wall clock. The principal count is global across
conversations and runs on the new (principal id, addressed) index; the channel
count rides the junction's conversation index. Both run inside the existing stamp
serialization, so two racing messages cannot both take the last budget slot.

### Configuration

The configuration file gains a protection table per the budget decision: four
fields, per-field defaults, window-zero disables, count-zero refused, unknown keys
refused.

### Scope fence

No tools, no spam detection, no admission (unit 5). No framework changes. The
adapter is untouched — protection is core policy at the write, invisible to the
adapter by construction. The unlimited paths stay unlimited: recording, erasure,
the failure notice.

## Acceptance criteria

- **AC1** The workspace builds; the suite passes in parallel and single-threaded
  identically; loopback-only traffic.
- **AC2** The principal budget: within one window, addressed messages up to the
  budget are answered and the next is recorded addressed-but-limited with the
  `principal` fact and no own answer; once the window passes (receipt times aged by
  a test seam or short configured windows), the same principal is answered again —
  pinned end to end over a scripted provider.
- **AC3** The channel budget: two principals exhausting the channel budget
  together; the over-limit message carries the `channel` fact; another channel is
  unaffected; the limited principal's direct chat still answers under its own
  principal budget — pinned.
- **AC4** Composition: an over-limit addressed message arriving behind an
  unanswered answer-due tail carries the propagated debt (answer-due true, limited
  set) and the earlier sender's answer still arrives; a budget-exhausted channel
  still records unaddressed messages with a null limited fact and consults no
  budget for them — pinned.
- **AC5** The race: two messages arriving concurrently for the last budget slot
  yield exactly one taken debt and one limited, under the stamp serialization;
  the test states its repeated-run probabilistic nature and runs the interleaving
  enough times to make a broken serialization fail in practice — pinned.
- **AC6** Debt authority: an admin's addressed message opens an admin debt; a
  member's unaddressed message propagating it stamps member (minimum); a member's
  ADDRESSED message behind the same admin debt also stamps member; a fresh member
  debt stays member; a pre-migration-shaped owing tail with null debt authority
  reads as its stored sender authority; erased tails still owe nothing — each
  pinned block by block.
- **AC7** Derived, never stored: no new table, no in-memory counter; the budget
  outcome changes when the ledger's recent history changes and only then — pinned
  by aging receipt times through the test seam and observing the budget release.
- **AC8** Configuration: absent table takes defaults; partial table takes per-field
  defaults; window zero disables; count zero refuses at parse naming the field;
  unknown keys refused; the process-level test drives one budget through the
  binary.
- **AC9** Clippy denied-warnings, fmt, doc under denied warnings (workspace and per
  package), the vocabulary scan and the secret scans are clean; any new dependency
  is recorded before a manifest names it. The expectation is none.
- **AC10** The decisions above are recorded, dated, with rejected alternatives; the
  stage re-slice is recorded; the framework-improvement item (schema-name constants
  or a counting read seam) is on the improvements list; the migration is the first
  appended step per decision 0026.
