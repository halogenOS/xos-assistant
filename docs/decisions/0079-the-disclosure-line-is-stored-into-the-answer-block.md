# The disclosure line is stored into the answer block, in the operator's copy

Date: 2026-08-23. Unit 12, settled with the operator in the unit's design
review; the two amendments recorded in the unit specification bind here.

The copy is the operator's, verbatim and fixed:

> Hi, I'm Xenia, the halogenOS Assistant Bot, an AI system, made to assist
> members of the community.

followed by a blank line, then the answer. One line, plain words a young
reader understands (the guidelines' accessibility note, para 34), no
legalese. The fallibility note ("answers can be wrong") lives in the bot's
bio and the policy; the line's one job is the disclosure.

The line is **stored, not added at delivery**: it is prepended into the
final answer block itself, so the ledger carries exactly what the chat saw
and the model reads in its own history that this person was already
introduced. The framework owns the finalize transaction and stayed
untouched in this unit, so the consumer's prepend rides the outbound
edge's first read of the finalized block — the earliest consumer-owned
moment, ahead of every delivery — written back through one idempotent
statement that cannot stack a second line. The guarantee stays mechanical:
the model neither writes the line nor can omit it. The write names the
framework's text table directly, the same deliberate coupling decision
0032 records for the header and junction tables.

The deterministic replies — the privacy command answers, the rules
acknowledgment, the report line, the failure notice — carry no disclosure:
they are fixed lines a human operator wrote, not model output, and
burdening a rights reply with it would blur what the line marks.

## Rejected alternatives

- **The earlier draft copy with a fallibility clause.** The operator's
  review moved the fallibility note into the bio and the policy; a longer
  line dilutes the disclosure it exists for.
- **Prepending at delivery only, ledger untouched.** The ledger would not
  carry what the chat saw, and the model would never see in history that
  the person was introduced.
- **A system nudge making the model weave the introduction.** A
  disclosure that depends on model obedience is advice, not a mechanism —
  the same reasoning as the assistant-assesses-a-human-decides rule. The
  prompt still teaches honest self-identification for free-text questions;
  the duty itself never rests on it.
- **A disclosure on deterministic lines.** Nothing machine-generated to
  mark.
