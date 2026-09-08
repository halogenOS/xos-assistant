# Reply recency

Decided 2026-09-08.

The requested behavior, verbatim:

> when the conversation is unlatched, the model is not supposed to answer to messages that are more than 10 member messages back. The only exception is /report. This also applies to when the last model answer was from more than 12h ago.

The clarification, verbatim:

> 12h means if the last message the model sent to the group is older than 12h, the same 10-member-messages rule applies as when freshly unlatched

The answering instructions apply the same member-message count after either condition.
The clock concerns the last successful message sent to the group, not private model text,
a failed sending attempt, or the age of the member's message.
The rule covers both plain sends and threaded replies. Reporting through the report tool
is the only exception, under its existing assessment rules.
Older history remains available as context; it is not a backlog of questions to answer.
The reply tool's description defers to this rule instead of granting unconditional
permission to answer any member message the conversation holds.

A separate 12-hour message-age cutoff is rejected by the clarification.
Removing old history is rejected because reporting and context still need it.

## Checks

- Every answering mode and capability combination includes the complete instruction.
- The instruction states both conditions, the count, the successful-send clock, both
  sending tools, and the reporting exception.
- The reply tool's description does not contradict the instruction.
- These checks verify the instructions supplied to the model, not model compliance.
