# 0154 — An unsummoned bot message carries no owing tail, and the walk reads through it

Date: 2026-08-30, with unit 42.

## Context

Summoning is not the only way a turn opens. The write-time stamp composes answer-due as the
message's own debt OR the conversation's owing tail, and the watcher fires a turn for any
newest block stamped answer-due. So a message that summons nothing still opens a turn by
CARRYING an earlier message's unanswered debt. Without a rule for that case, a bot's plain
message appended while the tail owes would open exactly the turn decision 0153 forbids,
wearing somebody else's debt.

## Decision

Two halves, standing together.

First: a bot sender's unsummoned message stamps answer-due false outright. The one
production call site that composes a stamp passes no owing tail for such a message — it
does not even read one — so the row takes no debt of its own and carries none. The stamp
composition itself stays pure and gains no sender input, so no other caller moves.

Second: the owing-tail walk reads through a live chat row whose stored stamp is false.
Without this, the false-stamped bot row would BURY the older debt, since a false stamp
means "settled" to the walk. The walk is one decision recorded at two places by its own
contract — the tail condition and the query that skips a whole transparent run — and both
widen together, because widening one alone is a silent no-op that still buries the debt.
The query gains the stamp condition on the typed answer-due column as a third transparency
dimension beside the kind set and the erased shape, DISJUNCTIVELY: a row is transparent
when it is erased OR false-stamped. A conjunctive rewrite would shrink the erased dimension
for true-stamped erased rows and regress decision 0086.

The safety argument splits by row class, because the first half above creates a second
class. A row whose stamp was composed against a READ owing tail — every production chat-row
append but the bot row decided here — takes that stamp under the entry point's lock with the
tail read in the same critical section, so its stored false stamp CERTIFIES that nothing
older owed at that write: anything owed would have made the tail half true, limited rows
included, since a command above an owed tail carries it. The ledger is append-only and
stamps never newly owe after the fact, so reading through such a row reaches the same
settled frontier that stopping at it named. That equality is pinned on the false-row shape
production actually writes — a command's limited false row above a settled tail — in both
answering modes.

The unsummoned bot's row certifies nothing. It is stamped false by rule, without reading a
tail at all, and it is written false deliberately ABOVE a live debt, so no claim about the
frontier behind it can be read off it. The widened walk IS its safety: this is the row the
debt has to survive, and reading through it is what lets the debt survive.

The owed debt therefore stays owed across any run of unsummoned bot messages, and the next
message entitled to carry it — anyone's, or a bot's carrying the mention — opens the turn
with it intact.

## Rejected alternatives

- **Letting the tail ride on bot messages.** It is a bot triggering this assistant while
  wearing another message's debt, which is the thing being stopped.
- **Storing the bot fact on the row so the walk can name bot rows.** A stored copy that
  drifts, for a distinction the stamp already encodes, plus a column, a migration and an
  erasure question.
- **Widening only the tail condition, or only the query.** Either alone leaves the debt
  buried by the other: the condition decides the tail row, the query decides everything
  behind it.
- **Answering the owed debt on a timer instead.** A mechanism this unit has no order for.
