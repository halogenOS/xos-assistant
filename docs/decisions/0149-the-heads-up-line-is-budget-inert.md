# 0149 — The heads-up line changes nothing about the budgets

Date: 2026-08-30, with unit 40.

## Context

An early reading of this unit claimed a spending difference: that a turn which announced
and then failed would spend its debt where a silent failed turn would not, so teaching the
line would quietly make failures more expensive for a member.

The claim was checked against the counted-debt predicate the two budgets share and against
its pins, instead of being carried forward.

## Decision

No such difference exists, and the unit changes nothing about the budgets.

One definition decides what consumes budget: an opened debt is counted unless a committed
assistant answer anchored to the summoning message trims to nothing. That is a COMPLETED
silent turn — the framework's record of a turn that chose to say nothing. A failed turn
commits no such empty answer, so it counts the same with a heads-up line ahead of the call
and without one. The exclusion the line could interact with is the one case where the
assistant deliberately said nothing, and a turn that announces a search is not that case.

This is written down so nobody derives the false contrast a second time.

## Rejected alternatives

- **Recording the claimed difference as a known cost.** It is not true, and a wrong cost
  in the record is worse than no record.
- **Excluding announced-then-failed turns from the counts.** It would carve a hole in the
  one definition of what consumes budget for a case that does not exist, and give an
  abuser a shape that spends nothing.
- **Saying nothing, since nothing changed.** The next reader would re-derive the same
  wrong reading from the same two mechanisms.
