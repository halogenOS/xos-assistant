# 0192 — A failed turn is silent in the chat

Date: 2026-08-31

## Context

Decision 0025 gave every failed turn one line in the chat, and decision 0057 took that
line away for one failure class. In practice the line arrived unasked for, twice in a
row, in a group that had asked the assistant nothing at that moment: a deployment
restart cut the turns that were in flight, and each cut turn spoke about itself. The
operator's instruction is that the assistant does not tell the group about its own
problems, and that the line goes away for every failure, not for one class.

The two things the line was supposed to carry hold without it. The latch already closes
the conversation on a failure, so nothing keeps spending, and the next message that
addresses the assistant re-engages it and is answered. What the line added was an
announcement, and an announcement is what nobody asked for.

## Decision

A failed turn says nothing in the chat, for every failure cause and in every situation.
The outbound edge's stream-error arm delivers what the dead turn already put on the
ledger — a filed report above all: the wake delivers what was filed before the death —
and sends nothing of its own after that. The uniform failure line, the payment-class
classification that suppressed it, and the reply kind that marked it are all gone; with
silence universal, a rule that chose silence for one class has no remaining job.

The record moves to the log. The arm writes one information line per stream error,
naming the conversation and carrying the error text the framework rendered off the
event. That line is unconditional: one of the framework's own emit sites writes nothing
on its success path, so a failure of that shape would otherwise be recorded nowhere at
all.

Nothing else changes. The latch still closes the conversation, spending still stops, and
the next addressed message still re-engages exactly as before. No retry, no message
after a restart, no new chat behavior of any kind.

## Rejected alternatives

- **A deduplicated one-time line.** Keeps the announcement and adds bookkeeping to
  decide when it repeats. The instruction was that the line goes away entirely, and a
  line that is sometimes sent is still a line the group did not ask for.
- **Suppressing only the failures a shutdown caused.** Narrows the fix to the case that
  was observed and leaves every other cut turn talking about itself. It also asks the
  core to tell one cause from another off text nobody owns, which is the coupling
  decision 0025 already rejected.
- **Retrying the cut-off answer after a restart.** Answers questions whose askers have
  moved on, minutes or hours late, and does it in a burst for every conversation the
  restart interrupted. That is the same flood the line was, with model spend behind it.

## Its place among the earlier records

This supersedes decision 0025's one-line rule, and it subsumes decision 0057: the
payment class needed a rule of its own only while every other class spoke. Both records
carry a dated note pointing here; nothing else in either changes, since each remains the
honest account of what was decided on its own date.
