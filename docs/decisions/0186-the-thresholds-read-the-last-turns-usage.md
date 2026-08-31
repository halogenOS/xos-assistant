# 0186 — The compaction thresholds read the last turn's reported usage

Date: 2026-08-31, with unit 48.

## Context

The design names two conditions for compacting on the assistant's own initiative: only 50k
of context left, or an expired prompt cache over a half-full window. Both are readings of
how much of the model's context window a conversation currently occupies, and nothing in
the ledger records that. The framework reports token usage on every completed stream and
persists none of it.

## Decision

The stream observer — the one subscriber already folding those events — keeps what each
conversation's last reporting turn MEASURED: the tokens it occupied, and when it
dispatched. Both are stream facts, so they live where the stream facts live rather than in
a second subscriber seeing the same bus twice.

The occupied amount is the request's input plus the response's output. Reasoning tokens are
deliberately not added on top: where a provider reports them separately they are already
inside the output count.

The window's size comes from the model configuration, as a stated key beside the model's
own name. No provider reports it.

With either number unknown, BOTH arms stay silent — the trigger never fires blind. A turn
whose provider reported no usage leaves the last known number standing rather than
overwriting it with a fabricated zero, and a deployment that has stated no window size gets
no automatic door at all, and no periodic sweep for one either. `/compact` and the forced
turn end are unaffected.

## Rejected alternatives

- **Persisting usage in the ledger.** It is a fact about a request, not about the
  conversation, and the framework's own rule is that a fact which can be measured where it
  arrives is not a column.
- **Estimating the occupancy from the stored blocks.** Every estimate is a second answer to
  a question the provider already answers exactly, and the two would disagree precisely on
  the conversations near the wall.
- **A window size guessed from the model's name.** A table of model names in this consumer
  would be wrong the week after it was written.
