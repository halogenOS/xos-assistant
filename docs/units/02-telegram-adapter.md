# Unit 2 — the Telegram adapter

Date: 2026-08-21. Revision 2, rewritten after the unbriefed two-reviewer probe returned
twenty findings (one blocker: messages sent on behalf of a chat; the rest gaps in the
configuration surface, batch failure semantics, startup ordering and checkability), all
verified against the official Bot API documentation and both trees. The core spine is
merged: the core exposes the ingestion entry point (returning a receipt), the
per-adapter outbound subscription edge, and the erasure operation, all platform-neutral.
This unit gives the assistant its first platform.

## The invariant this unit lives under

An adapter contains no behavior. It translates between the platform's API and the
core's message model — inbound updates into inbound messages, outbound replies into
send calls — and decides nothing about conversations, answers, policy or storage. If a
choice feels like behavior, it belongs in the core or in a later unit's contract, not
here.

## Decisions taken with this unit

- **The adapter speaks the Bot API directly, 2026-08-21.** A thin client over an HTTP
  library: long polling with `getUpdates`, sending with `sendMessage`. Rejected: a
  Telegram SDK crate (a large dependency tree to audit for what is, for this unit, two
  endpoints and a JSON model; and SDK update types must not cross into the core anyway,
  so the SDK would be wrapped as thoroughly as the raw API); webhooks (require a public
  HTTPS endpoint and certificate wiring — deployment surface this project does not have
  or need yet; long polling works from anywhere).
- **The update offset is persisted beside the process, 2026-08-21.** `getUpdates`
  acknowledges by offset; the state file holds the next offset to send (the highest
  acknowledged update id plus one) at a path the embedder supplies, written after the
  batch's messages are ingested — so a crash between ingest and write redelivers (and
  duplicates) instead of dropping. An absent, empty or malformed state file is treated
  as absent, logged, and the redelivered updates are the accepted duplicates.
  Rejected: offset only in memory (the wire itself confirms on the next poll, so the
  loss window is a crash between ingest and that poll — similar in size, but implicit;
  the file makes the redelivery window explicit and testable); deduplicating in the
  core (the core has no platform vocabulary and no uniqueness contract on origin).
- **Authority translates from the chat's member status, 2026-08-21.** The platform's
  `creator` maps to admin, `administrator` to moderator, everything else to member; a
  direct chat's sender is a member. The adapter resolves status from a per-chat
  administrator list fetched via `getChatAdministrators` and cached with a short
  time-to-live, which is what "resolved live by the adapter at receipt" (decision
  0008) can mean against a rate-limited API. Rejected: `getChatMember` per message
  (one API round-trip per group message, against the same authority data); trusting a
  status carried on the update (updates do not carry it).
- **Messages on behalf of a chat are skipped, 2026-08-21.** A group message carrying
  `sender_chat` — an anonymous administrator posting as the group, or a linked
  channel's auto-forward — has no resolvable person behind its fake sender: recording
  it would mint one shared principal aggregating several real people (wrong identity,
  wrong erasure scope) at member authority (wrong standing). Skipped as a named case
  beside the channel-broadcast skip. Rejected: recording the fake sender as-is (a
  principal that is not a person corrupts both the identity model and erasure);
  resolving the real author (the platform deliberately withholds it).
- **Text is what this unit records, 2026-08-21.** An update's message text, or its
  caption when the message is media with a caption, becomes the inbound text. Updates
  with neither, and non-message updates, are skipped. Edited messages are skipped too:
  the recorded ledger keeps the message as first seen, and an edit kind — appending
  the revision as its own block — is a later unit's decision, taken when the acting
  policy exists to read it. Rejected: recording edits as fresh messages (two blocks
  claiming to be the person's one statement, with no marking).
- **Replies send plainly, 2026-08-21.** The outbound reply carries a channel key and
  text; the adapter sends the text to the chat. Reply threading onto the origin
  message is deferred until the outbound edge carries the origin (the column is
  stored for exactly that); wiring a guess now would thread every reply onto the
  newest message, which is wrong in a busy group. Rejected: extending the outbound
  edge in this unit (a core change the adapter invariant says an adapter must never
  need).
  Amended 2026-08-21, after review: the platform caps one message's text, and a
  finalized answer carries no length bound, so "plainly" is refined — a reply longer
  than the cap goes out as consecutive in-order chunks, and a chunk that fails ends
  the reply at the last delivered chunk, and no tail is sent past a lost
  middle. This is translation of a documented wire constraint, not behavior: the
  adapter decides nothing about the text, it fits the platform's message unit.
  Decision record 0019 carries the rule and the rejected alternatives (dropping the
  whole over-cap reply; truncation; language-aware split points).

## The unit's contract

### The embedder contract

The adapter is a library with one constructor and one run entry. Its configuration:
the bot token, the API root (defaulting to the Bot API host; tests supply the
loopback server's address), the state-file path, and nothing else. The adapter's
registered name is the constant `telegram`, pinned in the crate: it keys the channel
mappings and principals durably, so it is a permanent contract, not a parameter. The
adapter runs against a shared reference to the started assembly and consumes the two
public edges and the receipt — nothing deeper.

### The client

One module owns the Bot API wire: request building, the bot token, JSON decoding into
the adapter's own minimal update model (update id, message id, chat id, chat type,
sender id and display fields, sender-chat presence, text or caption, member
statuses). The token arrives as configuration and appears in no logged output and no
error text. HTTP details — timeouts, the long-poll timeout parameter, the rate-limit
retry — live here and nowhere else, with the retry wait taken through an injectable
sleep so tests pin it without waiting it out. A send answered with the rate-limit
reply honors the stated wait and retries up to three attempts, then logs and drops;
a stated wait past a named ceiling fails the send at once, because the outbound
consumer is sequential and a flood wait would park every later reply behind it. The
poll and the administrator fetch park no queue and honor stated waits in full —
re-asking a limiter early amplifies the load being limited.

### The loop

The adapter takes its outbound edge before the first poll — the core treats answers
stored before the subscription as history, so the order is part of the contract, not
an implementation detail. Then it long-polls, translates each update per the
decisions above, hands each inbound message to the core's entry point, persists the
offset after the batch, and concurrently sends each reply from the edge to its chat.
Chat type `private` maps to the direct channel kind; `group` and `supergroup` map to
group; `channel` broadcasts carry no conversation this assistant serves and are
skipped. The platform's message id becomes the origin reference; the platform's send
date becomes the message timestamp. The per-chat administrator cache's time-to-live
is a named constant in the adapter (about a minute); tests may run with the cache
cold or the time-to-live at zero.

### Failure behavior

The batch discipline: on the first failed ingest the batch stops — the offset is
persisted up to the last success, the loop backs off and re-polls, and the failed
update and its successors redeliver, the same at-least-once outcome the offset
decision accepts. A deterministic refusal from the core (a channel-kind mismatch, as
opposed to a transient store error) is terminal for that update: logged and
acknowledged past, because retrying it forever would wedge every later message in
the chat behind it — this data-loss rule is the spec's, stated here so no implementer
has to invent it. A failed administrator-list fetch fails that message's ingest the
same way a transient ingest failure does: the update is not acknowledged and the next
poll retries; authority is never silently defaulted into the ledger. A failed send
follows the bounded rate-limit retry, then is logged and dropped. Two loss surfaces
are named plainly: a stream error yields no reply (the core's recorded OPEN), and an
answer finalized while the adapter is down is history to the next subscription and
is not sent — accepted for this unit, revisited when user-facing failure behavior
arrives with the live-model unit. A network error backs off and re-polls; the loop
never busy-spins.

### Scope fence

No core changes, no live provider, no acting policy, no new outbound-edge fields. The
tests drive the adapter against a scripted Bot API server on the loopback interface —
no traffic leaves the machine — and against the real core assembly with a scripted
provider written in this unit's test code against the framework's public provider
traits, per decision 0009's reasoning; like unit 1's, it must answer the metadata
worker's title-derivation request deterministically.

## Acceptance criteria

- **AC1** The workspace builds; the suite passes in parallel and single-threaded
  identically. That no test reaches beyond the loopback interface is review-verified
  (nothing in a suite can assert its own absence of traffic).
- **AC2** The end-to-end test: a scripted Bot API server serves one group-message
  update; the adapter ingests it through the real core, the scripted provider answers,
  and the adapter delivers the reply to the scripted server's `sendMessage` — asserted
  on the server's recorded requests and on the ledger.
- **AC3** Translation is pinned per decision: private/group/supergroup chat kinds,
  creator/administrator/member authority, caption fallback, and the skip cases
  (channel posts, sender-chat messages, edits, text-less updates) each have a test.
- **AC4** Offset persistence is pinned: a restarted adapter (same state file) does not
  re-ingest acknowledged updates; a crash simulated between ingest and offset write
  redelivers and the test states the duplicate as the accepted outcome; a batch that
  fails mid-way persists the offset up to the last success; a malformed state file
  reads as absent.
- **AC5** The rate-limit reply is honored: a scripted rate-limit response with a
  stated wait makes the send hand that wait to the injectable sleep and retry, up to
  the three-attempt bound and the drop past it — pinned through the sleep seam, no
  real waiting.
- **AC6** The token appears in no log line and no error string, pinned by a test that
  forces both paths and scans the output.
- **AC7** The core crate still passes its platform-vocabulary scan untouched; that
  the adapter reaches nothing beyond the core's public API is enforced by visibility
  and confirmed in review.
- **AC8** Clippy with warnings denied, fmt, and doc builds are clean across the
  workspace.
- **AC9** Every new external dependency is recorded in the dependency review document
  — current version from its registry, advisory database consulted — before a
  manifest names it.
- **AC10** The decisions above appear in the repository's decision records, dated,
  with their rejected alternatives.
