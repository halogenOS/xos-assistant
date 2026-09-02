# Unit 56 — a refused compaction is not retried

Date: 2026-09-02. A production incident of the same afternoon: every live conversation's
unattended compaction fails with "conversation already has a system prompt" and is retried
on every wake, each retry forking a temporary conversation and paying for a summary turn
over the whole first half before failing at the same statement. Two defects stack: the
compacted thread inherits a system prompt it must not, and a refused statement — a rule the
database will refuse again — is classified as transient and retried forever.

The repositories: the framework (`ronna-core`, `~/projects/agent-ledger`, HEAD `a1e41a5`;
paths rooted at `crates/agent-ledger/`) and the app (this repo, HEAD `515301a`; the deployed
build is `9ff182e`, whose relevant code is unchanged since). Framework first.

## What is true today

Every claim was read from the two trees at the stated heads, and the incident from the
operator's log of 13:36-13:37Z.

1. **The startup walk appends the prompt at the end.** `Sessions::forked_with_current_prompt`
   forks a conversation up to its last block, detaches the inherited prompt blocks, and
   inserts the current system prompt — which therefore becomes the successor's newest block
   (`crates/core/src/session.rs:456-471`). `retire_stale_channels` runs it for every mapped
   conversation whose prompt moved, once at startup. Last night's deploy moved the prompt.
2. **The compacted thread copies everything after the cut.** `Store::open_compacted_thread`
   inserts the thread's own system prompt first, then `copy_junction_after` copies every
   junction row past the cut with no regard to kind (`src/store/compaction.rs:236-290`,
   `src/store/conversations.rs:871-886`). A prompt sitting past the cut comes across, and
   the one-prompt rule refuses the thread: "conversation already has a system prompt".
3. **A refused statement is classified transient.** `CoreError::failure_kind` maps
   `StoreError::Rejected` to `FailureKind::Transient`, and a test asserts exactly that
   (`crates/core/src/error.rs:135-155, 191-192`). A refusal is the database applying a rule
   it will apply again; nothing about it is transient.
4. **The driver retries on every wake, whatever the class.** `unattended_compact` logs "the
   session stands and the next wake retries" for any error (`session.rs:1451-1462`); the
   wake comes every thirty seconds or on any block change. Each attempt forks a temporary
   conversation and runs the summary turn before the failing statement is reached, so the
   loop pays per cycle. Unit 51 bounded the crash loop; it did not bound this one.

## The design

**The compacted thread has exactly its own prompt.** `copy_junction_after` used by
`open_compacted_thread` copies no `system_prompt` block: the thread's prompt is the one
`open_compacted_thread` inserts, and an inherited prompt anywhere past the cut is left
behind. The temporary summary fork is untouched: it copies the first half as it stands, and
a prompt there — at block one for a conversation never re-forked, absent for one that was —
is that half's own history.

**A refused statement is terminal.** `StoreError::Rejected` classifies `FailureKind::Terminal`:
the rule that refused it stands until the code changes. `StoreError::Contended` stays
transient — a race is retried; a rule is not. The test that asserted the old mapping asserts
the new one.

**The driver stops retrying what will refuse again.** A compaction that fails with a terminal
error is not retried for that conversation for the life of the process: the driver records
the conversation in a set it consults before every attempt, logs the failure once at error
level naming the conversation and the error, and the next wake finds nothing to do. A
restart clears the set — a deploy is the one thing that can change the rule — and a
transient failure keeps today's retry. Nothing is deleted, nothing is answered in chat.

## Acceptance criteria

Framework:

1. `open_compacted_thread` on a source whose newest block is a `system_prompt` past the cut
   produces a thread carrying exactly one system prompt, the thread's own, with every other
   post-cut block copied in order. A test builds that source and asserts the thread's ledger.
2. The temporary summary fork's first-half copy is unchanged: a test asserts a block-one
   prompt still rides across and a source without one yields a fork without one.
3. Every check runs clean: `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and `cargo doc
   --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

App:

4. `StoreError::Rejected` classifies `FailureKind::Terminal`; `StoreError::Contended` stays
   `Transient`. The classification test asserts both.
5. A compaction failing with a terminal error is not attempted again for that conversation
   in the same process: a test scripts a refused statement on the first attempt, wakes the
   driver repeatedly, and asserts exactly one fork and one model request.
6. The terminal failure is logged once at error level naming the conversation and the error;
   a transient failure keeps the retry and its warning. A test asserts each.
7. A conversation re-forked by the startup walk — prompt at the end — compacts successfully
   end to end against the merged framework: a spine test runs the walk, ages the conversation
   past the threshold, and asserts the compacted thread serves with one prompt.
8. Every check runs clean in this repository, as in criterion 3.

## Rejected alternatives

- **Moving the prompt to the front during the startup fork.** Rejected: the ledger is
  append-only and a fork shares history through the junction; the new prompt can only be
  appended. The thread that owns a prompt must be the one to say which prompt it carries.
- **Retrying terminal failures with a backoff.** Rejected: the user's standing decision —
  "Failures are failures and failures just fail the compaction" — and a backoff on a rule
  refusal only slows the burn.
- **Deleting the late prompt from the source.** Rejected: nothing is deleted from a ledger
  by a compaction, and the late prompt is the source's true history.

## Decisions on record

**2026-09-02, the incident (msg 1718):** "It's wasting tokens again. What is the issue?"

**2026-09-01, the retry policy:** "I dont want the backoff. Failures are failures and failures
just fail the compaction." and "Loops are the worst kind of bug. It either wastes money,
wastes resources or spams people"

**2026-09-01, the error classes:** "There is a difference between catching an expectable query
error and a serious db failure. Something failing on a foreign key constraint is an error
while a race with another writer is expected and can be retried if it makes sense."
