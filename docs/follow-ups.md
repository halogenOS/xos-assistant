# Follow-ups

Work recorded, not built — each item names the unit that recorded it and
what resolving it takes. Resolved items move into a decision record.

- **The group-to-supergroup migration strands the stored channel mapping**
  (group context, 2026-08-23). The platform renumbers a chat when a group
  becomes a supergroup; the stored mapping, the authorization row and the
  conversation stay keyed to the old id, and the renumbered chat arrives as
  a stranger. The migration signal exists on the wire but is discarded (see
  the next item). Resolving this means translating the migration into a
  re-keying of the mapping and the authorization.
- **The wire client discards error response bodies on non-success status**
  (group context, 2026-08-23). A non-success status is reduced to its code,
  which hides the migration signal above and every refusal detail the
  platform states. Resolving this means decoding the refusal envelope on
  failure statuses too, without letting any token-bearing detail into the
  error text.
- **The framework superseding-block compaction** (group context,
  2026-08-23). Context notes accumulate in stream order and the newest
  wording is authoritative; a superseding-block mechanism in the framework
  — already on its improvements list — would compact the superseded ones
  out of the projection.
- **A debt behind a date crossing has no behavioral pin** (date-marker
  fallout, 2026-08-28). The consumer's debt walk now reads through the
  framework's date records at both of its blind sites, but the shape that
  would distinguish a fixed walk from a broken one — an owing message
  standing behind a date marker that is itself the tail — needs two appends
  on two different local dates, and the seams that drive the date are the
  framework's own `pub(crate)` ones. The consumer cannot construct it, so
  no fixture exists for that half here. Resolving this means a pin on the
  framework's stamped append seam, at date-crossing granularity, in the
  framework's suite.

- **The Article 50(2) due-diligence check awaits the first live turn**
  (first-interaction disclosure, 2026-08-23). The compliance record relies
  on the upstream model provider's text marking and records the presence
  check as OPEN until live output exists to inspect. Resolving it means
  running the public industry-standard detector over a real first turn's
  output and writing the result into the compliance record's marking
  section.

- ~~**Unit 7 close, 2026-08-23 — the provider module still speaks its first
  gateway's name.**~~ **Closed 2026-08-24.** The configuration key, the secret,
  the environment variable and the core's feature now name the interface:
  `chat_completions`, `chat_completions_api_key`, `CHAT_COMPLETIONS_API_KEY`.
  The framework's own `openrouter` module keeps its name where it appears —
  that is the framework's vocabulary for its shared chat-completions wire, and
  renaming another project's type in our prose would be the same misdirection
  in the other direction. The compatibility alias this item asked for was
  **rejected by the operator**: the rename is clean and the one stored
  credential is re-entered once, which fails loudly at the secrets prompt
  rather than quietly accepting a name that no longer means anything.

- **Unit 15 close, 2026-08-24 — the rules-note in-context guarantee is a
  guarantee of the request payload, not of what the model retains at scale.**
  The autonomous assessment rests on the newest rules note being in the
  model's context (decision 0094); the projection folds the whole loaded,
  never-windowed ledger into the request, so on a conversation whose history
  grows past the model's context the provider truncates and nothing pins the
  rules note as the survivor — the assessment would silently degrade to
  judging from rules it no longer sees, exactly when a busy group needs it.
  Near-term low-risk (fresh deployments, large model context, per-conversation
  ledgers), but the fix is a windowed projection that keeps the system prompt
  AND the newest rules note pinned regardless of history length, with an
  acceptance criterion proving the note survives a context-exceeding history.

- **Unit 15 close, 2026-08-24 — report filing serializes on one global lock.**
  The per-origin dedup replaced the per-channel `LineWindow` with a single
  process-wide `tokio::sync::Mutex<()>` on the shared report tool, held across
  the whole store transaction, so a slow append while filing in one channel
  blocks filing in every other channel. Reports are rare, so the impact is
  low, but the lock is broader than its stated same-origin-dedup reason.
  Closing it means keying the dedup guard per conversation (or per origin)
  rather than globally.

- **Unit 15 close, 2026-08-24 — report filing makes two linear passes over the
  never-windowed ledger.** Each filing scans the full conversation ledger
  twice (the dedup check and the append's own load). Rare-path, so low impact,
  but it grows with the retention-free history. Closing it means a bounded
  lookup for the prior-report check instead of a full scan.

- **Unit 16 close, 2026-08-24 — a silent unaddressed miss still spends its
  budget slot.** The counting SQL excludes only the abstention sentinel, so an
  unaddressed miss that delivers nothing still counts its opened debt. This errs
  in the limiting direction (a miss spent a real lookup worth bounding) and the
  budget spine was held unchanged this unit, so it is deliberate and pinned; a
  later change to extend the exclusion to the silent miss would be a considered
  budget decision, not a bug fix.

- **Unit 16 close, 2026-08-24 — the addressed-miss first-interaction path
  writes the block twice.** Delivering the fixed don't-know line to a first-time
  addressed asker issues one update to rewrite the miss sentinel to the line and
  a second to prepend the disclosure. Correct but two writes where one composed
  write would do; closing it means folding the disclosure prepend into the same
  rewrite.

- **Unit 16 close, 2026-08-24 — the grounding discipline rests on the model
  emitting the miss sentinel.** By design (the mechanical gate was rejected), a
  substantive answer that does not ground its claim and does not emit the miss
  sentinel is delivered as written. The teaching governs this and a scripted
  turn pins the intended path; a live-transcript replay of a real failed lookup
  would raise confidence that the model complies in practice.

- **Unit 17 close, 2026-08-24 — the two endpoint resolvers are duplicated.**
  resolve_wiki_index_endpoint in the binary's config is byte-for-byte the
  trim/refuse-empty logic of resolve_wiki_endpoint, with a parallel start-error
  variant. Correct but duplicated; closing it means one shared resolver
  parameterized by the endpoint name.

- **Unit 20 (rules-ack) close, 2026-08-24 — the one-shot acknowledgment call
  blocks the observation path.** The rules-delta observe path awaits the bounded
  model completion inline (up to the 10s timeout), so a rules change delays that
  update's batch processing until the ack returns or falls back. Bounded and on a
  rare admin-only path (a pin), so accepted; if rules-change responsiveness ever
  matters, spawn the generation and deliver the ack via a background task rather
  than the ObserveOutcome return value.
- **Unit 20 close, 2026-08-24 — the pinned rules text rides the model request
  undelimited.** The (admin-controlled) rules text is the user message with no
  wrapping. Admin-only and the ack is a trivial confirmation, so low risk; a
  fenced/delimited wrapping would harden it against a crafted rules pin if ever
  wanted.

- **Unit 21 close, 2026-08-24 — the addressed-mode clarifying-question note
  explains turn mechanics to the model.** The Addressed branch tells the model
  "only a message that addresses you reaches you, so a plain follow-up would
  otherwise go unseen" — a bit of internal reach mechanics in a model-facing
  prompt. It gives the model the reason to invite a reply (arguably helpful) and
  only affects addressed mode (not the helpful-mode deployment), so it is a nit;
  a tighter wording would state the instruction without the mechanism rationale.

- **Unit 26 (threaded replies) fix pass, 2026-08-25 — the never-threaded guard
  knows one of the two reply-invoked command shapes.** An answer is never
  threaded when its prose carries `REPORT_LINE_LEAD`, per decision 0108. The
  core records a second shape acted on from a reply: `mirror::DELETION_COMMAND`,
  which an administrator invokes by replying with it, so the same hazard ends in
  a deletion instead of a report — while the assistant itself holds
  administrator standing, which is a deployment's choice. The unit's spec scopes
  the guard to the report lead deliberately, so widening it widens 0108 and is
  not the fix pass's to take. Resolving it means either amending 0108 to cover
  every reply-invoked command shape — with the shapes coming from one record
  that `mirror.rs` and the guard both read, instead of a second literal in the
  guard — or recording in 0108 why `/del` stays out. The disagreement is pinned:
  `crates/core/tests/spine/threading.rs`,
  `an_answer_carrying_the_deletion_command_shape_delivers_plainly`, ignored and
  failing by design until 0108 answers.
  **Resolved 2026-08-27:** the guard was widened. Decision 0108 carries the dated
  amendment, the shapes come from the one list in `crates/core/src/reply_commands.rs`
  that the guard reads, and the pin is un-ignored and green.

- **A dated ledger sends the model two adjacent system messages** (date-marker
  fallout, 2026-08-28). Open question, recorded with what is known, not settled
  here. A production request now opens with the prompt's system message and the
  date marker's dated line as a second system message directly behind it. The
  framework would have joined them: `blocks_to_messages` groups a run of blocks
  sharing a projection role into one message. The consumer's `tool_palette` sits
  between them and projects no role at all, and a role-less block ends the open
  run — so the group closes at the prompt and a fresh system group opens at the
  marker. Same-role adjacency is what strict vendors reject, and the suite
  already pins against it for exactly that reason
  (`crates/core/tests/spine/projection.rs`, `assert_alternation_holds`) — over
  synthetic ledgers, which carry no marker, so the shape the runtime actually
  sends is outside that pin. The scripted provider the suite runs on accepts
  anything, so no test here can fail on it either;
  `crates/core/tests/spine/date_marker.rs` records the two-message shape as the
  observed fact. The question is where the join belongs: the framework's render,
  grouping a same-role run across role-less blocks, or the deployment's vendor
  module, folding the leading system messages at the wire. It is not the
  consumer's to answer by giving the palette a role, which would put the tool
  list in the model's system voice. Whichever answer, it must be settled before
  the deployment's framework pin moves onto a date-marking framework, because
  the first live request after that carries the adjacency to a real vendor.
