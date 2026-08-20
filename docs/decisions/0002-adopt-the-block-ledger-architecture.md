# 0002 — Adopt the block ledger architecture, and the license that follows

Date: 2026-08-20

## Context

The assistant needs to store what it sees and says: group messages from several chat
platforms, its own replies, the tool calls it makes on the way, and enough history to
answer a question about last week. Two shapes were on the table — design a store for this
project from nothing, or adopt the architecture already proven in ronna-lightspeed
(https://github.com/xdevs23/ronna-lightspeed), a multi-agent orchestration application by
the same author.

The relevant properties of that architecture:

- **Blocks are the only content unit.** There is no message row. A conversation is an
  ordered list of blocks — text, thinking, tool call, tool result, status — joined to
  conversations through a junction table, so two conversations can share a block and a
  fork copies junction rows instead of content.
- **Storage is append-only.** A block is permanent and immutable once promoted. The one
  exception is the streaming block, the live tail that updates while a reply is produced;
  finalizing inserts a fresh committed block in the same transaction and never rewrites a
  committed one.
- **State is derived, not stored.** Whether a conversation owes work is read from the
  blocks themselves, so no status column can disagree with them.
- **Blocks carry their own behavior.** Each block kind answers a small uniform interface —
  who owes the next move, do my own work, am I done — and the orchestration machinery
  advances those hooks without ever branching on block kind.

## Decision

The assistant adopts this architecture, and the subsystems that implement it are brought
into this repository: the block store, the block behavior model and its orchestration
loop, the model provider modules, and stream ingestion. A chat platform message becomes a
block kind like any other.

Two consequences settle other open questions for free:

- Answering only some group messages is not a feature to build. A message the assistant
  should not answer reports no ask, which makes it invisible to the model-turn decision,
  so no reply is produced while the message stays stored and searchable as context. The
  answer-or-ignore call is stamped on the block when it is written, so the block still
  reports the same ask when history is replayed.
- After a restart the assistant does not reply to a backlog in a burst: a conversation
  starts suspended and only explicit new input resumes it.

**Sequencing.** The subsystems are copied into this repository first, and this repository
becomes the base they evolve from. Extracting them into a core library shared by both
projects — with the option of releasing that library on its own — is the intended end
state, deferred until two real consumers exist to show where the boundary belongs.

**License.** The adopted code is licensed under the GNU General Public License v3.0, so
this project is GPL-3.0 as well. The workspace declares it and the license text is
included.

## Rejected alternatives

- **Design the store from nothing.** Rejected: the questions this project would meet —
  streaming tails, tool loops, history that survives a crash, replaying old conversations
  against a new prompt — are the same ones the adopted architecture already answers, with
  the reasoning for each answer written down.
- **Depend on the upstream application as a library.** Rejected: it carries a large
  amount of code this project has no use for, including sandboxing, source-control
  integration, language-server support and document parsing. Each one would become a
  dependency to build and maintain for no benefit here.
- **Extract the shared core library first, before any feature work.** Rejected for now:
  the boundary of a shared library is chosen well when more than one consumer exists to
  test it. Copying first, extracting second, means the boundary is drawn from evidence
  instead of from a single example.
