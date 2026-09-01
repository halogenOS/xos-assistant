# Unit 53 — quiet conversations expire

Date: 2026-09-02. The assistant gains its one retention rule: a conversation whose newest
ledger entry is older than ninety days is deleted — the conversation, the blocks nothing else
still holds, the files nothing else still references, and the identity rows no message names
any more. Activity refreshes the whole conversation, so a living group never loses history;
what expires is what nobody has touched in three months. The privacy policy's new retention
wording ships in this unit, because a policy must describe only what the code does.

The repository: this one, at `9ff182e` (spec commits sit above it; no source has moved). The
framework at `b7c0c45` is consumed by path and does not change in this unit.

## What is true today

Every claim was read from the trees at the stated heads.

1. **Nothing expires.** The committed policy says "We set no automatic expiry, deliberately",
   decision 0003 (2026-08-20) records it, and no code deletes anything on a schedule. The
   working tree carries an uncommitted policy draft promising a 90-day expiry for replaced
   conversations — wording ahead of reality, which this unit makes true and widens per the
   decision below.
2. **Replaced conversations accumulate.** A compaction swaps the channel to the new thread
   and leaves the source "on record" (`crates/core/src/session.rs:857-863`); the startup
   staleness walk forks and remaps without deleting. Nothing ever removes them.
3. **Deletion is one door.** `Sessions::retire` settles, then `Store::delete_conversation`,
   then the context forget (`session.rs:262-292`). The framework's delete cascades junction
   rows, drafts and metadata, nulls unshared dispatch anchors, and leaves block headers and
   content for `gc_orphan_blocks`, which deletes blocks nothing references, looping until a
   pass deletes nothing (`agent-ledger/src/store/messages.rs:1085-1154`).
4. **A missing ancestor is already survivable.** `ancestor_reference` carries no foreign key
   on purpose (framework migrations v5); `serving_lineage` and `stripped_lineage` stop the
   walk at an ancestor whose ledger reads empty, log it, and treat the hop as the root
   (`crates/core/src/lineage.rs:78-95`, `crates/core/src/erasure.rs:430-457`). Quoting is
   scoped to the serving conversation alone and never reaches an ancestor.
5. **The mapping is separate.** `channels` maps one channel to one conversation; erasure
   already deletes the mapping row before the conversation (`erasure.rs:350-356`), and an
   unmapped channel's next message creates a fresh session inside the stamp lock
   (`assembly.rs:1191-1205`).
6. **Identity outlives its purpose.** `principals` rows are deleted on an erasure request
   and nowhere else; a principal whose every message is gone keeps a stored username.
   Flagged rows (the opt-out stubs) must survive any sweep — erasure empties their username
   and keeps the flag (`crates/core/src/identity.rs:207-225`).
7. **One periodic job exists as the precedent.** The compaction driver: a spawned task on a
   30-second `tokio::time::interval` with skipped missed ticks (`session.rs:1508-1585`).
   Wall-clock time comes only from the framework's `ClockReading`; a test enforces that the
   app holds no clock of its own. Block rows carry `created_at` written by SQLite's
   `datetime('now')`, UTC.
8. **The config file is where deployment numbers live.** `Configuration` (TOML,
   `deny_unknown_fields`) already carries `context_window` and the protection windows in
   seconds, each with a default and zero disabling (`crates/assistant/src/config.rs`).

## The design

**The rule.** A conversation expires when its newest ledger entry — the newest block its
junction holds, by that block's stored UTC `created_at` — is older than the configured
retention span. Every conversation is measured by the same rule: serving, replaced, ancestor,
direct. There is no second clock and no per-kind carve-out; a compacted ancestor stops
growing at its cut, so it expires ninety days later while its living descendant, refreshed by
every message, does not.

**The span is configuration.** `retention_days` in the TOML, default 90, zero disables the
sweep entirely. The default is the decision; the field exists so a deployment can be told
apart from the code, the shape `principal_window_seconds` already set.

**The sweep.** A spawned task beside the compaction driver, on an interval, reading the span
from configuration and the current moment from the framework's clock. Each tick it lists the
expired conversations and, for each one: delete the channel mapping row if one names it, then
retire through the one existing door — settle, delete, forget. After the conversations, one
`gc_orphan_blocks` pass; then the media files whose stored references are gone; then the
unflagged principals no message row names any more. A tick that finds nothing does nothing.
Failures follow the standing rule: a store error inside one conversation's retirement fails
that conversation's deletion and the sweep moves on, logging it; the next tick retries it,
because the conversation is still expired. Nothing about a failed sweep touches serving.

**A swept serving channel.** Deleting a mapped conversation unmaps the channel; the next
message finds no mapping and creates a fresh session, the path that already exists and is
already raced correctly. The group loses nothing it used in ninety days, because using it is
what refreshes it.

**The policy and the records.** The policy's retention section states the rule in member
words: messages are kept while the conversation they sit in stays in use; a conversation
untouched for ninety days is deleted whole, with its files; a deletion request never waits
for the schedule. The records-of-processing row and the docs test move with it. Decision
0003's no-expiry statement is refined by a new dated decision recording the ninety-day rule
and what it beat; the old decision is not rewritten.

**First activation.** The user's requirement, verbatim: "Nothing should be deleted on the
first boot with the policy actice". It holds by the rule itself — the oldest data any
deployment carries is weeks old, so no conversation reaches the threshold — and the sweep
has no catch-up behaviour to make first boot special: a boot is a tick like any other, and a
tick deletes only what the rule names.

## Acceptance criteria

1. `retention_days` exists in the configuration with default 90; zero disables the sweep and
   spawns no task. Tests cover the default, an explicit value, and zero.
2. The expiry reading is the newest junctioned block's `created_at` per conversation,
   compared in UTC against the framework clock's now. A test builds conversations either
   side of the threshold and asserts exactly the stale one is named.
3. The sweep retires an expired conversation through the existing door: mapping row deleted
   first when one exists, then settle-delete-forget. A test asserts the conversation, its
   junction rows and its mapping are gone.
4. A conversation with any entry inside the span is untouched, however old its oldest entry
   is. A test puts ninety-one-day-old and fresh blocks in one conversation and asserts it
   survives.
5. An expired ancestor of a living thread is swept, the living thread survives, and the
   walkers behave as already specified: the lineage ends at the deleted hop with the warning,
   and a retraction aimed above the cut resolves as it does today for a pre-cut message. A
   test compacts, ages the ancestor past the span, sweeps, and asserts all three.
6. After the conversations, the sweep runs one orphan collection, deletes every stored media
   file whose referencing rows are gone, and deletes every unflagged principal no message row
   names; flagged principals survive. Tests cover a collected file, a surviving shared file,
   a collected principal and a surviving flagged one.
7. A swept serving channel's next message creates a fresh session and is answered. A spine
   test sweeps a mapped conversation and sends the next message.
8. A store failure inside one conversation's retirement leaves that conversation for the next
   tick and does not stop the sweep or the process. A test scripts the failure on the first
   of two expired conversations and asserts the second is swept and the first survives to be
   swept when the failure clears.
9. A database whose every conversation is fresher than the span sweeps nothing: no deletion,
   no mapping change, no file removed. A test asserts it against a populated store — the
   first-boot requirement, held by rule.
10. The sweep uses the framework clock for now and the stored UTC timestamps for age; no new
    clock enters the app. The existing clock-source test still passes.
11. The policy's retention section states the any-conversation rule, the policy test asserts
    the new wording, the records-of-processing row matches, and a new dated decision refines
    0003 with the rejected alternatives. The uncommitted draft's replaced-only paragraph is
    replaced by this wording; no committed document promises what the code does not do.
12. Every check runs clean: `cargo fmt --all -- --check`, `cargo clippy --workspace
    --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and `cargo doc
    --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

## Rejected alternatives

- **Replaced conversations only.** The first draft's shape. Rejected by the user: any
  conversation, because the assistant works in fresh sessions and freshness is the latest
  ledger entry.
- **A per-message expiry.** Rejected in decision 0003 for good reason and still rejected: a
  schedule that deletes individual messages guts a conversation the group still uses. The
  unit expires whole quiet conversations, which is the shape 0003's objection does not reach.
- **A special first-boot grace period.** Rejected: the rule already protects first boot, and
  a mechanism whose only job is to protect a moment the rule protects is a second answer to
  one question.
- **Sweeping inside the compaction driver's loop.** Rejected: the driver's tick is
  thirty seconds of monotonic time serving context pressure; retention is wall-clock days.
  Folding them couples two unrelated cadences and two clock kinds in one loop.
- **Deleting the group's authorization with its conversation.** Rejected: the authorization
  is the operator's admission of a channel, not member data, and a quiet group that speaks
  again should be served without re-admission.

## Decisions on record

The user's words, verbatim.

**2026-09-01, the rule (msg 1657):** "Let's just say any, because the bot still works even in
fresh sessions. But the freshness is determined by the latest ledger entry."

**2026-09-01, the go and the first boot (msg 1659):** "It hasn't been 90 days yet so we are
still in the clear any way. Let's just implement it and activate it. Nothing should be
deleted on the first boot with the policy actice"

**2026-08-20, decision 0003:** message history kept with no scheduled expiry; refined by this
unit's decision document, not reversed.
