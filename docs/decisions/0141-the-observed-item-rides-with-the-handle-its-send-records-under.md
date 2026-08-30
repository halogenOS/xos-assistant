# 0141 — The observed item rides with the handle its send is recorded under

Date: 2026-08-30, with unit 38.

## Context

Every message the assistant sends records its delivery, and the record belongs to a
conversation. Ingestion's deterministic items already ride a call's return beside a
receipt that names their conversation, so that half needed nothing new — the driver was
simply discarding the receipt.

The rules acknowledgment does not. It rides the observation call's return, which carried
the item and nothing else: no conversation, no handle, nothing the adapter could report
the send against. The recorded design pointed at the ingestion receipt for this, which
is stale against the tree — the acknowledgment leaves through the observation path.

## Decision

The observation's delivered item rides together with the handle its send is recorded
under, as one value. The two are one fact: an item with nowhere to record its send
cannot exist, and pairing them keeps the adapter from ever holding one half without the
other. The ingestion side needs no pairing, because its receipt always names the
conversation, and it answers the same handle from there.

The handle is opaque to adapters. An adapter receives it beside the text it is asked to
send and hands the same value back once the platform has taken the message; it reads
nothing out of it and decides nothing from it.

## Rejected alternatives

- **Skipping the acknowledgment's record.** It breaks the every-message contract, and a
  class of her messages silently unrecorded is exactly the omission the recorded design
  was corrected once to remove.
- **A conversation id beside the item on the outcome.** Several of the observation's
  returns are reached before any conversation is resolved — a direct channel, a refused
  group — so the field would have to be filled with an invented value at each of them.
- **Threading the handle through the delivery item itself.** The item is the payload;
  where its send is recorded is not part of what is said.
