# 0117 — The query and the pages are bounded, and a refusal is whole

Date: 2026-08-27, with unit 27.

## Context

Two inputs reach the vendor from the model: a query and a page number. Both need a
bound, and the interesting question is what a bound DOES when it is hit. A truncated
query is a different question with the same tool call around it — the model would
read the answer as an answer to what it asked.

## Decision

A query is at most 400 characters and a page runs from 1 to 5. Over either bound the
call is refused WHOLE, with the number named in the refusal so the model can correct
it, and nothing is sent. A query is never truncated to fit. Past page 5 the model is
fishing, and the budget is better spent on a reworded query. The request timeout is
the shared lookup layer's own default, like every other outbound request here.

Every refusal and every failure is a taught result the model reads and the chat never
sees (decision 0044): the refused key, the refusal that is not about the key, the
rate limit, the unreachable host, the timeout and the unreadable answer are each
distinguishable from one another and from an honest empty page — and none of them
carries a bare status number.

## Rejected alternatives

- **Truncating an over-long query.** A different question, asked in the model's name,
  answered as though it were the one asked.
- **Unbounded paging.** Every page is a billed request, and a model paging for an
  answer that is not there spends a person's whole budget on one bad query.
- **One generic failure result.** The model cannot tell "reword this" from "this
  deployment has no working key", and it retries the one it cannot fix.
