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

- **The Article 50(2) due-diligence check awaits the first live turn**
  (first-interaction disclosure, 2026-08-23). The compliance record relies
  on the upstream model provider's text marking and records the presence
  check as OPEN until live output exists to inspect. Resolving it means
  running the public industry-standard detector over a real first turn's
  output and writing the result into the compliance record's marking
  section.

- **Unit 7 close, 2026-08-23 — the provider module still speaks its first
  gateway's name.** The configuration key, the endpoint override and the
  provider crate are named for OpenRouter while the deployed gateway is any
  OpenAI-compatible endpoint (Requesty today). Invisible to users and
  harmless at runtime — the endpoint override carries the truth — but the
  vocabulary misleads a reader of the configuration. Closing it means a
  neutral provider name across config, crate and docs with a compatibility
  alias for the old key.

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
