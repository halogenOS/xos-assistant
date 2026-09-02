# Unit 53 — quiet conversations expire

Date: 2026-09-02. The assistant gains its one retention rule: a conversation whose newest
ledger entry is older than ninety days is deleted — the conversation, the blocks nothing else
still holds, and the identity rows nothing anywhere names any more. Activity refreshes the
whole conversation, so a living group never loses history; what expires is what nobody has
touched in three months. The privacy policy's new retention wording ships in this unit,
because a policy must describe only what the code does. Stored media files are OUT of this
unit's scope for the same reason: file reception is a committed spec with no implementation,
so there are no files to collect and no file promise may be committed.

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
4. **A missing ancestor is already survivable.** `block_ancestor_reference` carries no foreign key
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
   The app's only wall-clock read is the framework's `ClockReading::now_local()`, which
   carries a LOCAL date and minute-level time and no UTC instant; a test bans every clock
   crate from the app. Block rows carry `created_at` written by SQLite's `datetime('now')`,
   UTC. So the one place that can compare a stored timestamp against now correctly is the
   store itself — the same clock wrote both sides.
8. **The config file is where deployment numbers live.** `Configuration` (TOML,
   `deny_unknown_fields`) already carries `context_window` and the protection windows in
   seconds, each with a default and zero disabling (`crates/assistant/src/config.rs`).

## The design

**The rule.** A conversation expires when its newest ledger entry — the newest block its
junction holds, by that block's stored UTC `created_at` — is older than the configured
retention span. The comparison runs INSIDE the store, in SQL, against the store's own
`datetime('now')`: the clock that wrote every `created_at` answers the age question, the app
reads no wall clock, and no clock crate enters the tree. Every conversation is measured by
the same rule: serving, replaced, ancestor, direct. There is no second clock and no per-kind
carve-out; a compacted ancestor stops growing at its cut, so it expires ninety days later
while its living descendant, refreshed by every message, does not. A conversation whose
junction holds no blocks never expires by this rule — emptiness is a transient creation
state, raced by the mapping claim, and sweeping it would delete a conversation mid-birth;
crash residue in that shape is bounded and already cleaned by the lost-claim and startup
paths.

**The span is configuration.** `retention_days` in the TOML, default 90, zero disables the
sweep entirely. The default is the decision; the field exists so a deployment can be told
apart from the code, the shape `principal_window_seconds` already set.

**The sweep.** A spawned task beside the compaction driver, on a one-hour
`tokio::time::interval` with missed ticks skipped and the first tick at spawn — a boot is a
tick like any other, and every tick is idempotent, so the cadence carries no meaning beyond
freshness of enforcement. Each tick it asks the store for the expired conversations and, for
each one: delete the channel mapping row if one names it, then retire through the one
existing door — settle, delete, forget. After the conversations, one `gc_orphan_blocks`
pass; then the unflagged principals nothing names any more. The collections are owed on
EVERY tick, expired conversations or none — an earlier tick's failed collection must not
leave orphaned rows waiting for something else to expire — so a tick that finds nothing
expired deletes no conversation and still collects. Failures follow the standing rule: a store error inside one conversation's
retirement fails that conversation's deletion and the sweep moves on, logging it; the next
tick retries it, because the conversation is still expired. Nothing about a failed sweep
touches serving.

**The sweep and an erasure never race.** The whole tick — the expiry reading, the
retirements and the collections — runs under the erasure fence, the arbiter the erasure and reset paths already hold; whichever holds the
fence runs whole, and the other waits its turn. A deletion request never waits for the
schedule in the other direction either: erasure takes the fence on demand and the sweep's
next tick simply finds less to do.

**Which rows name a principal.** The erasure module already enumerates every row family that
reaches a principal — messages, join notices, report copies, mark blocks, reaction records —
and that enumeration is the ONE answer. The sweep's principal collection reads the same
enumeration from the same place erasure does, factored so a family added later is added
once; a principal named by any row in any surviving conversation is kept, and flagged
(opted-out) principals are kept unconditionally, exactly as erasure keeps them.

**A swept serving channel.** Deleting a mapped conversation unmaps the channel; the next
message finds no mapping and creates a fresh session, the path that already exists and is
already raced correctly. The group loses nothing it used in ninety days, because using it is
what refreshes it.

**The policy and the records.** The policy's retention section states the rule in member
words: messages are kept while the conversation they sit in stays in use; a conversation
untouched for ninety days is deleted whole; a deletion request never waits for the schedule.
The no-expiry statement has five committed homes and every one moves: the policy's retention
section, the records-of-processing rows D1/D3 and D11, the impact assessment's expiry line,
and the legitimate-interests assessment's scheduled-deletion reasoning — plus a new dated
decision refining 0003 with the rejected alternatives; the old decision is not rewritten.
The parked working-tree draft is split: its retention paragraph is superseded by this
unit's wording, and its file-storage paragraphs are NOT committed, because they promise a
subsystem that has no implementation. The withdrawal is documented, never silent: the new
decision document records that the draft carried file-storage wording, why it was held back
— the media intake is a committed spec with no code — and that the wording returns with the
unit that builds it. The docs test moves with what ships.

**First activation.** The user's requirement, verbatim: "Nothing should be deleted on the
first boot with the policy actice", given with its own reasoning: "It hasn't been 90 days
yet so we are still in the clear any way." At this activation the requirement holds by that
calendar fact — no stored conversation is near the threshold — and durably by the sweep
having no catch-up behaviour: a boot is a tick like any other, and a tick deletes only what
the rule names. Stated plainly for the record: a future activation over a store that already
holds conversations past the span would delete them on its first tick, because the rule is
the only mechanism and that is what the rule says.

## Acceptance criteria

1. `retention_days` exists in the configuration with default 90; zero disables the sweep and
   spawns no task. Tests cover the default, an explicit value, and zero.
2. The expiry reading is the newest junctioned block's `created_at` per conversation,
   compared inside the store against the store's own `datetime('now')`; a conversation with
   an empty junction is never named. A test builds conversations either side of the
   threshold, plus an empty one, and asserts exactly the stale one is named.
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
6. After the conversations, the sweep runs one orphan collection and deletes every unflagged
   principal that no row of the shared reference enumeration names; the enumeration is the
   same one erasure reads, recorded once. Tests cover a collected principal, a principal
   kept because only a mark or join notice in a surviving conversation names it, and a
   flagged principal kept unconditionally.
7. A swept serving channel's next message creates a fresh session and is answered. A spine
   test sweeps a mapped conversation and sends the next message.
8. A transient store failure inside one conversation's retirement leaves that conversation
   for the next tick and does not stop the sweep or the process; a fatal one — a refused
   statement, an unusable store — ends the sweep and raises the process's exit, per unit 56.
   A test scripts a transient failure on the first of two expired conversations and asserts
   the second is swept and the first survives to be swept when the failure clears.
9. A database whose every conversation is fresher than the span sweeps nothing: no deletion,
   no mapping change, no principal removed. A test asserts it against a populated store —
   the mechanical half of the first-boot requirement; the calendar half is recorded in the
   design.
10. The tick runs under the erasure fence, the expiry reading included, so what the reading
    names is what stands when the deletions run. A test holds the fence and asserts the
    tick's deletions wait for its release.
11. The app reads no wall clock for the sweep: the store answers the expiry question with
    its own clock, and the existing clock-source test still passes unchanged.
12. Every committed home of the no-expiry statement moves: the policy's retention section,
    records-of-processing D1/D3 and D11, the impact assessment's expiry line, the
    legitimate-interests reasoning, and a new dated decision refining 0003 with rejected
    alternatives. The policy test asserts the new wording. The parked draft's file-storage
    paragraphs are not committed, and a grep of the committed documents finds no promise of
    stored files or of a no-expiry rule.
13. Every check runs clean: `cargo fmt --all -- --check`, `cargo clippy --workspace
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
