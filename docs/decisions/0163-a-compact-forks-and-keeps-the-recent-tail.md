# 0163 — A compact forks the conversation and keeps only the recent tail

Date: 2026-08-30, with unit 45.

**Superseded 2026-08-31, by decision 0185 (unit 48).** The tail keep is gone whole: a
compaction now summarizes the first half of the ledger and carries the second half forward
verbatim, so nothing is discarded and the older conversation reaches the model as prose
instead of not at all. Everything below is the record of what unit 45 shipped and why, kept
because the reasoning it states about detaching, about the bulk door, and about what a reset
costs a pending report is the reasoning the mechanism that replaced it inherited. The kept
set, the kept bound, the nothing-to-cut check and the invoking row's own place in the count
no longer exist in the code.

## Context

The model reads the WHOLE conversation every turn: there is no read window, no floor and no
summary anywhere in the framework or in this consumer. So a conversation can come to carry a
thousand-row tool flood, and a model reading one continues the pattern it reads. What
`/compact` has to produce is a conversation that reads like the recent conversation and
nothing else.

## Decision

`/compact` runs the retire walk's shape with one addition. The conversation is forked with
its full inherited history, the fork takes the current binding and the current prompt in
place of the inherited ones, and then every block of the PRE-FORK snapshot is detached from
the FORK except the kept set. The snapshot is the enumeration basis on purpose: the fork's
own fresh prompt is outside the sweep structurally, not by a filter that could be edited
away.

The kept set is:

- the trailing chat rows, up to a bound of twenty — a member's recorded message, one of the
  assistant's own text blocks, or a stored quote, counted alike, with their stamps and their
  debts riding along, so an unanswered question inside the tail still owes its turn;
- every date record standing among them, plus the newest one before the oldest kept row, so
  the kept rows keep their own day;
- the newest tool palette, which is configuration and not traffic: without it a fork whose
  first wake is a turn would run with no tool admitted at all;
- the newest context note per observed fact — the group's title and its rules — so the model
  keeps what the group is with no enrichment round trip.

Everything else is cut, and each class for its own reason. Tool traffic is defined exactly —
a call, its result, its error — and none of it survives, not even inside the kept tail: it is
the poison the command exists to remove, and because no call crosses, the fork can never open
on a call with no result behind it. Join notices go because a reset session owes no memory of
who walked in. Delivery records go because they project nothing. Filed reports go because
their deliveredness lives only in the outbound edge's process memory: the core cannot tell a
delivered report from a pending one, and keeping both would re-deliver, which the delivery
contract refuses above all. A report still pending at a compact is therefore lost, accepted
under this tree's own recorded process-death precedent, and with eyes open — the turn an
unattended compact follows was looping, and its report is the least trustworthy thing it
produced.

The bound has one home in the code, read by both the sweep and the check below, and the
invoking command's own row counts INSIDE it: the row arrived to ask for the compact, so it is
not what makes a session uncompact.

Which row that is, is CARRIED rather than assumed. The command trigger names the invoking
row's block id, and the count reads the chat rows older than it; ids ascend in ledger order,
so "older" is a fact the reading checks instead of an assumption about the tail that a later
append could quietly break.

A conversation with nothing to cut is left alone entirely — no fork, no mapping write — and
answers so. Nothing to cut means: no tool traffic stored, and no more chat rows than the
bound, counting only the rows older than the invoking row. The unattended path has no row of
its own and counts the whole readable set.

Nothing is deleted. Detaching removes a junction row from the fork and never a block, so the
source conversation keeps every one of them and, because it still references them, the orphan
collector cannot reach any of them. A debt older than the kept tail is cut with the context
that poisoned it — stated so nobody calls it a burial: the reset was asked for, and the source
still shows the unanswered message to a human reader.

The sweep's cost is bounded rather than accepted: the whole detach list goes through the
framework's bulk door in ONE round trip and one transaction, so a thousand-row flood holds
the ingestion lock for one commit instead of a thousand. The per-row door is the wrong shape
for a set, and using it here would have made the ingestion pause scale with the very flood
the command exists to cut.

One cost stands: a conversation predating the first-contact notes carries none to keep, so
its compacted fork stays note-less until the platform fact next changes or the process
restarts.

`/compact` needs no channel-reset directive: the kept notes carry the group knowledge across,
so there is nothing for the adapter to forget.

## Rejected alternatives

- **A summary block in place of the cut history.** The framework has no summarization
  capability, and having the model write summaries of members' messages is new personal-data
  processing that would not be smuggled in through a command about context size.
- **Projecting per position — a marker that hides what sits below it.** Projection is per
  kind with no position context, and the verified burial defect is this tree's standing
  warning against new read-through shapes.
- **Keeping tool blocks that happen to fall inside the kept tail.** The kept tail has to be
  free of the poison by construction; a bound that sometimes keeps a call is a bound whose
  behaviour depends on where the flood stopped.
