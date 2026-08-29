# 0045 — Erasure does not reach tool blocks yet — OPEN

Date: 2026-08-22. In the 0012 lineage; revisited when the framework offers the
path.

## Context

A tool call's input and result may quote a person's prose, and they live in
framework-owned tables the assistant's erasure cannot reach today: the erasure
write path nulls the assistant's own content table and deletes identity rows,
while tool blocks are framework kinds with framework storage.

## Decision

Accepted for this unit, with its reasoning recorded: the group tools are
project lookups whose inputs are overwhelmingly technical, and the erasure
write path for tool blocks is framework work — filed on the framework
improvements list beside the other seams. OPEN.

## Rejected alternatives

- **Blocking the unit on a framework erasure seam.** The tools would wait on a
  release the assistant does not control, for a surface whose personal-data
  exposure is marginal today.
- **A consumer-side scrub of framework tables.** Reaching into another crate's
  storage names its internals; decision 0032 already records what that coupling
  costs where it was unavoidable.

---

Narrowed 2026-08-23, by decision 0063: the report block — a consumer kind in a
consumer table — is reached by erasure directly, keyed by the reported principal
it stores for exactly that purpose. What stays OPEN here is exactly the
framework-owned surface: tool call and result blocks, whose input and result can
quote a person's prose, still waiting on the framework seam.

---

Amended 2026-08-29, with unit 27 (the web search). The acceptance above rests on a
stated ground: "the group tools are project lookups whose inputs are overwhelmingly
technical". A web search query does not meet that description. It is written by the
model out of the conversation, in the members' own words, and it is recorded on the
same framework-owned tables erasure does not reach — so the unreached surface is
WIDER than the ground that was accepted for it, and this record says so instead of
letting the widening pass unnamed.

The gap stays OPEN and still waits on the same framework seam. Two mitigations
answer the widening, both shipped with the unit and both pinned:

1. **No deliberate member identifier leaves, or is recorded.** The query guard
   (decision 0115) refuses a query carrying a handle-form token whole, before
   anything is sent, so the identifier reaches neither the vendor nor the tool
   record.
2. **The refusal echoes nothing.** A refused query answers with the rule and the
   fix and never names what was matched — a guard that wrote the identifier it
   refused into a record erasure cannot reach would protect nothing.

What remains, stated plainly: a member's own words can ride into a stored query
without naming anyone, and erasure does not reach them. The public policy already
names this gap; the impact assessment's addendum of 2026-08-29 carries the
assessment.
