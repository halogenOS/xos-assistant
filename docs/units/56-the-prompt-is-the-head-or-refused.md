# Unit 56 — the system prompt is the head of the ledger, or it is refused

Date: 2026-09-02. A production incident of the same afternoon: every live conversation's
unattended compaction fails with "conversation already has a system prompt" and is retried
on every wake, each retry forking a temporary conversation and paying for a summary turn
over the whole first half before failing at the same statement. The cause is a forbidden
shape the store never forbade: the startup prompt fork appended the new system prompt at the
END of a cloned ledger. The user's decision, verbatim: "System prompt at the end of the
ledger is a forbidden shape and should have errored hard instead of retrying. System
messages are fine but the system PROMPT not. The fork is a simple clone with only half the
convo in the block list."

The repositories: the framework (`ronna-core`, `~/projects/agent-ledger`, HEAD `a1e41a5`;
paths rooted at `crates/agent-ledger/`) and the app (this repo, HEAD `ac88b13`; the deployed
build is `9ff182e`, whose relevant code is unchanged since). Framework first.

## What is true today

Every claim was read from the two trees at the stated heads, and the incident from the
operator's log of 13:36-13:37Z.

1. **The startup walk clones first and appends the prompt last.**
   `Sessions::forked_with_current_prompt` forks up to the last block, detaches the inherited
   prompt blocks, then inserts the current system prompt, which the append-only ledger makes
   the successor's newest block (`crates/core/src/session.rs:456-471`). `retire_stale_channels`
   runs it for every mapped conversation whose prompt moved, once at startup; last night's
   deploy moved the prompt, so every live conversation now carries the forbidden shape.
2. **Nothing in the store forbids it.** `insert_system_prompt` refuses a SECOND prompt
   (`src/store/messages.rs:355-397`, trigger `trg_unique_system_prompt`) and says nothing
   about position; a prompt appended after a thousand blocks is accepted.
3. **The compacted thread copies everything after the cut.** `Store::open_compacted_thread`
   inserts the thread's own prompt first, then `copy_junction_after` copies every junction
   row past the cut regardless of kind (`src/store/compaction.rs:236-290`,
   `src/store/conversations.rs:871-886`). The late prompt comes across and the one-prompt
   rule refuses the thread.
4. **A refused statement is classified transient and retried forever.**
   `CoreError::failure_kind` maps `StoreError::Rejected` to `FailureKind::Transient` with a
   test asserting it (`crates/core/src/error.rs:135-155, 191-192`), and the compaction driver
   retries any failure on the next wake (`session.rs:1451-1462`), every thirty seconds or on
   any block change, paying for the summary turn before reaching the refusal each time.

## The design

**The prompt is the head or it is refused.** The store's rule becomes positional: a
`system_prompt` block may be appended only to a conversation that holds no block yet. A
prompt appended anywhere else is refused with `StoreError::Rejected`, the trigger says so,
and the existing one-prompt rule stays beside it. A ledger can then never carry the
forbidden shape, and a door that tries to build one fails loudly at the door — at deploy
time, before any paid turn.

**A fork that replaces the prompt is a clone with the prompt first.** The framework's fork
door gains the prompt-replacing form: create the conversation, insert the given system
prompt, then copy the source's junction rows up to the boundary minus the blocks the caller
names — the inherited prompts. The successor's ledger reads prompt, then the shared history,
exactly as a compacted thread's does. `forked_with_current_prompt` takes that door and does
nothing after it. `open_compacted_thread` keeps its shape and, with the invariant in place,
never meets a late prompt; its post-cut copy still copies no `system_prompt` kind, so a
database written before this build compacts too.

**The databases already in the forbidden shape are repaired at startup.** Every deployed
conversation carries its prompt last. The startup walk's condition widens: a mapped
conversation is re-forked when its prompt moved OR when its prompt is not its first block,
and the fork is the prompt-first door. One walk, once, before serving; no paid turn.

**A refused statement is fatal.** `StoreError::Rejected` classifies `FailureKind::Fatal`: the
database applied a rule the code violated, the ledger is in a shape the code cannot
continue from, and the standing rule for a database error applies — the process ends
loudly and the supervisor restarts it, where the repair walk runs before serving.
`StoreError::Contended` stays transient: a race is retried, a rule is not. The compaction
driver treats a fatal failure as every other fatal failure — it ends the process — and no
retry path exists for it.

## Acceptance criteria

Framework:

1. Appending a `system_prompt` to a conversation that already holds any block is refused with
   `StoreError::Rejected`; appending one to an empty conversation succeeds; a second prompt is
   still refused. Tests cover all three, the first through the app's own door shape (fork,
   detach, then insert — asserted refused).
2. The prompt-replacing fork door exists: given a source, a boundary, the blocks to leave
   behind and a prompt, it yields a conversation whose first block is that prompt and whose
   remaining blocks are the source's junction rows to the boundary minus the named ones, in
   order. A test asserts the ledger shape and that the history is shared, not copied.
3. `open_compacted_thread` on a source whose newest block is a `system_prompt` past the cut
   yields a thread with exactly one prompt, its own, every other post-cut block copied in
   order. A test builds that source directly in SQL — the door can no longer build it — and
   asserts the thread's ledger.
4. Every check runs clean: `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets --all-features -- -D warnings`, `cargo test --workspace`, and `cargo doc
   --workspace --no-deps` under `RUSTDOCFLAGS="-D warnings"`.

App:

5. `forked_with_current_prompt` uses the prompt-first door; a test asserts the successor's
   first block is the prompt and the old prompt is absent.
6. The startup walk re-forks a mapped conversation whose prompt is not its first block, prompt
   moved or not; a test builds the forbidden shape directly in SQL, runs the walk, and asserts
   the channel serves a conversation with the prompt first and the same shared history.
7. `StoreError::Rejected` classifies `FailureKind::Fatal`; `StoreError::Contended` stays
   `Transient`. The classification test asserts both.
8. A compaction failing with a fatal error ends the process the way every fatal store error
   does, with no retry and no second fork: a test scripts a refused statement and asserts one
   fork, one model request, and the fatal exit signal the assembly already raises for fatal
   store errors.
9. A conversation re-forked by the walk compacts successfully end to end against the merged
   framework: a spine test runs the walk, ages the conversation past the threshold, and
   asserts the compacted thread serves with one prompt at its head.
10. Every check runs clean in this repository, as in criterion 4.

## Rejected alternatives

- **Skipping the late prompt in the compaction copy alone.** This spec's own first draft.
  Rejected on the user's decision: it would tolerate a forbidden shape instead of refusing
  it, and the shape would keep surfacing elsewhere.
- **Stopping the retry for the process lifetime and carrying on.** The first draft's second
  half. Rejected: a refused statement means the ledger is in a shape the code cannot
  continue from, and the standing rule for that is to end loudly, not to serve around it.
- **Retrying terminal failures with a backoff.** Rejected on the standing decision: "Failures
  are failures and failures just fail the compaction", and a backoff on a rule refusal only
  slows the burn.
- **Deleting the late prompt from the source in place.** Rejected: nothing is deleted from a
  ledger to change its shape; the repair is a prompt-first fork, the same door every other
  successor takes.

## Decisions on record

**2026-09-02, the shape (msg 1722, verbatim):** "System prompt at the end of the ledger is a
forbidden shape and should have errored hard instead of retrying. System messages are fine
but the system PROMPT not. The fork is a simple clone with only half the convo in the block
list."

**2026-09-02, the incident (msg 1718):** "It's wasting tokens again. What is the issue?"

**2026-09-01, the retry policy:** "I dont want the backoff. Failures are failures and failures
just fail the compaction." and "Loops are the worst kind of bug. It either wastes money,
wastes resources or spams people"

**2026-09-01, the error classes:** "There is a difference between catching an expectable query
error and a serious db failure. Something failing on a foreign key constraint is an error
while a race with another writer is expected and can be retried if it makes sense." and "a
database error should hard crash the application, not leave it running in a corrupted
state"
