# 0043 — Provenance is the minimum over the turn's chat run

Date: 2026-08-22

## Context

Decision 0036 stamps every message's debt authority at the write. Mid-turn
absorption makes "the frontier chat message at decision time" the wrong anchor
for admission: an absorbed message carries its own fresh stamp, and reading it
alone could escalate a member-summoned turn to admin — the exact escalation
0036 forbids.

## Decision

Walking back from the tool call, the provenance is the MINIMUM authority over
the contiguous chat messages since the previous assistant answer — the
summoning message and everything absorbed since — reading each message's debt
stamp with its sender authority as the fold for a null stamp (the pre-migration
shape, and every message that carries no debt). The minimum cannot escalate. It
can over-decline when a lower-authority bystander chimes in mid-turn — accepted
and stated: a declined lookup is a degraded answer, an escalated tool is a
broken access model. With two member-level tools this gate cannot decline
today; it exists so unit six's report tool inherits enforcement. Admitted calls
record no provenance fact — the framework's tool blocks carry no consumer
field, and the improvements-list item covers the durable form; a decline
records the reading in its recorded error text.

Closed 2026-08-22, at the unit's second verification: the ledger-shape walk is
refuted as a mechanism. Three adversarially proven escalations — an absorbed
message after a finalized narration, the whole post-narration tool phase, and
the window behind a failed round's error block — plus a cap rule that invented
debts for bystanders, establish that stored shape cannot tell a turn's own
voice from a turn boundary, and every refinement moved the leak instead of
closing it. The rule of this record stands as the INTENT; its mechanism is the
framework's dispatch anchor — the turn's summoning frontier recorded onto the
tool call at insert — which is the improvements batch's first item. Until that
anchor exists, the gate is floored structurally instead: tool registration
refuses any tool whose required authority is above member, so no walk runs and
nothing above the floor is expressible. The interim walk and its cap are
removed rather than shipped fenced; a mechanism that failed verification three
times is not a safety layer.

Closed again 2026-08-22, the same day: the mechanism arrived and the floor
lifted. The framework's dispatch anchor ships — every block a turn writes
carries its summoning frontier's id, inherited across continuation rounds, and
one `find_block` on the call's anchor loads the summoner. Registration accepts
any authority again, and the admission wrapper enforces this record's intent
over a NARROWER interval than the decision paragraph above states: the anchor
gate reads the minimum over the summoner's folded debt stamp and the sender
authorities of every chat message recorded between the summoner and the call —
the interval the refuted walk had no lower bound for, sound now that the
anchor is one. Narrower, stated plainly: the paragraph above folds the whole
contiguous run since the previous assistant answer, including every no-debt
message before the summons; the anchor interval starts at the summons, so a
pre-summons author whose debt the summoner's stamp does not carry — a resting
author's answered message, a limited author's refused one — lies outside the
interval and is not folded in, even when the dispatched request carries that
text. A null anchor (the out-of-band shape) reads as the floor; a non-message
or unloadable frontier contributes the floor itself, so the absorbed span can
never read above it; a decline records the reading in its error text, as
decided above.

Refined 2026-08-22, at the adoption unit's adversarial close, after the
sender-authority span was shown to be an AMBIENT veto: in a live group any
bystander line landing during the turn's seconds-wide span lowered the
reading, so an admin action declined whenever anyone chatted — the modal
case, not the rare one the over-decline acceptance was written for — and a
message the protection unit had refused service to still lowered it. The
folded set is therefore the turn's CO-SUMMONERS, not its bystanders: the
minimum over the summoner's folded stamp and the span's ADDRESSED, UNLIMITED
messages — the same opened-debt predicate the budgets count. An addressed
message absorbed mid-turn joins the turn's authority because the turn answers
it; unaddressed chatter contributes nothing, in the span exactly as before
the summons — one rule for context in both positions. The influence threat —
context steering the model toward a tool call — is out of this gate's scope
in every position, and is carried by the layers built for it: the fail-closed
palette, the tools' own designs, and the human review on moderation commands.
Escalation stays impossible: a member's addressed message still lowers an
admin turn, and an unaddressed message cannot summon anything.

Refined once more 2026-08-22, after verification broke both directions of the
line above: the framework's tail-derived continuation anchor let a message
absorbed after a round's result take over the turn's identity (escalation),
and a bystander's PROPAGATING message — unaddressed, yet the model-owed tail
under decision 0021 — became the summoner slot itself, its min-folded stamp
vetoing the admin whose debt it merely carried. Two resolutions, one per
layer. The framework now holds the open turn's anchor as actor state until a
close that ends the turn, so a continuation can never re-anchor onto an
absorbed line (the slice-8 amendment). And the gate's summoner endpoint is
the debt ORIGIN SET, not the anchor's own stamp: from the anchor, the
contiguous chain of answer-due chat messages at or before it is walked in
the loaded ledger, and the fold takes the sender authorities of the
own-debt-takers in that chain — the co-summoners — while pure propagators
carry no vote, exactly as span bystanders carry none. The stamp's min-fold
stays what it is for ANSWERING (0021/0036 unchanged); the gate simply stops
reading a carrier's fold as if the carrier were a summoner.

Refined 2026-08-23, twice, both on the origin walk's read-through edge. First
the walk became marker-aware: the framework now stores a turn-closure marker
when a close ends a turn over an unanswered outcome, so a message owed behind
that marker still owes — the walk reads through the marker, through a turn's
machinery, and through a dead turn's narration (a text a later marker
disowns), instead of ending on them. Then verification broke the walk's own
default: every block kind the classifier did not name ended the chain, and
truncating a minimum-fold RAISES the reading — a dead turn's thinking block,
an ordinary reasoning-model product anchored on the dead turn before its tool
call, cut the chain in front of a member whose unanswered summons still owed,
and the next turn's tools admitted at admin. The default is therefore
inverted: the chain ends ONLY at what answered a debt or never owed one — a
completed answer, meaning a text no turn-closure marker disowns, or a chat
message owing nothing — and every other block extends the walk: machinery, a
dead turn's narration and reasoning, and every unnamed kind, anchored on a
disowned turn or not, anchored at all or not. Extending is the judged bound
for anchor-less unnamed kinds too, stated deliberately: it is the direction
that cannot raise the fold, because one block further back can only add a
voter to a minimum, and over-declining is the cost this record accepted at
its birth. Rejected: keeping the end as the default and naming each safe
kind — that is exactly the first cut, and under it every kind added later
arrives as a fresh escalation; a walk that feeds a minimum must default to
the direction the fold forgives.

Corrected 2026-08-23, on the final verification, without changing the
behavior: the "cannot raise the fold" argument above is unsound as written.
A chain with no debt-taker folds to the FLOOR, so walking one block further
CAN find a taker and raise the reading — the verification proved the shape
with an admin taker behind a dead turn's thinking. The inverted default is
still right, for the reason the record already holds: the fold ranges over
the debt ORIGIN SET, and a reading raised by finding the debt's true owner
is a correct reading, not an escalation. What actually bounds the walk is
the stronger obligation the argument left unstated: every kind able to
REPRESENT AN ANSWER must sit in an ending arm — today the undisowned text
and the debt-free chat message — because a missed answer extends the walk
into a previous, settled debt. A future kind that answers must join that
set; a future kind that cannot answer is safe by the default. One dead-turn
shape sits outside the marker set deliberately: an interrupt writes no
closure marker (the framework's teardown edge stores `interrupted`, which
stays opaque so a resumed approval can continue), so a turn dead by
interrupt still reads its narration as a completed answer. Unreachable
today — the palette holds member-level tools only, so the gate has nothing
to escalate — and recorded as a named blocker: before any registration
above member (the moderation unit), the walk must classify a narration
under an `interrupted` tail, or the framework must store the closure on
that edge too.

## Rejected alternatives

- **The decision-time frontier read.** Escalates under absorption, as above.
- **A consumer admission-record block per admitted call.** Ledger noise
  recording a derivable behavioral fact; the ledger is for facts behavior
  cannot re-derive.
