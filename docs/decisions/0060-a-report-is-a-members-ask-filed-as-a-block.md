# 0060 — A report is a member's ask, filed as a block, delivered with the turn's answer

Date: 2026-08-23

## Context

The report half of the feature list: a member asks the assistant to file a
spam report with the group's moderation bot, which acts only on command
replies to the offending message. Decision 0043's anchor mechanism records
the exact shape in which a turn's bare anchor can be a bystander's line, so
the target must come from the debt origin walk, never the anchor.

## Decision

**The flow.** A member replies to an offending message and addresses the
assistant asking for a report. The report tool — member authority, group
conversations only, under the same admission gate as every tool (the gate
supplies no extra protection at member authority; stated, not implied) —
takes NO target parameter: it resolves the reply target through the debt
origin walk decision 0043 settled. The target is the newest co-summoner's
stored reply target; several co-summoners with targets resolve to the
newest, stated; a bystander's reply loses to a co-summoner's even when
newer. A turn whose origin set carries no reply target gets a tool error
saying a report needs a reply; a reply to the assistant's own message is
refused with its own error; a reply pointing at a message the ledger never
recorded — an erased one included — is refused too, because a report block
must name a principal erasure can reach (decision 0003's rule). Executing
the tool appends a report block — a consumer kind carrying the target
origin, the reported message's principal id and the fixed report line —
under the erasure fence, which the tool receives at construction. The block
is agency-inert, frontier-transparent like the context note, read through
by the consumer's debt walk (its exclusion set widened from notes exactly
to the consumer's delivery and supersession kinds), and classified as an
explicit extend in the provenance chain, not left to the default. The block
projects nothing: the filed report is machinery, and the model's knowledge
of it is the tool result. This crosses unit 5's "no tool writes anywhere"
rule, which gains its dated amendment: a tool may append blocks of kinds
that exist for tool-driven delivery; lookups still write nothing.

**The delivery contract, with its accepted losses.** The consumer's
outbound edge delivers report blocks threaded — the fixed `/report@` line
plus the configured moderation handle, sent as a platform reply to the
reported message — on BOTH stream events: with the answer on the turn's
completion, and on the turn's failure beside the notice, so a turn that
dies after filing still files. Within the completion the report sends
before the answer text, so the member's confirmation reads after the deed.
A report block undelivered when the process dies is LOST — the edge's
restart seeding stands, and re-delivering reports from history would ping
every group admin at-least-once; for a moderation nudge the accepted loss
is the safer direction, recorded plainly. A failed platform send is logged
and not retried, same acceptance. The tool result's wording claims filing,
not arrival: the exact copy ships in named constants and says the report
goes out with this turn. A targetless report — one whose origin an erasure
nulled — is skipped as undeliverable.

**The budget and the window.** A report ask consumes an answer slot like
any addressed turn. Filings are additionally bounded per channel by the
atomic line-window primitive under its own named constant
(`REPORT_WINDOW`); the grant is atomic — a second tool call in the same
round loses it and gets the declined result — and the slot is spent only
once the append stands, mirroring the unit-7 ordering fix: a transiently
failed append revokes the grant. The window is process memory: for this
bound a restart forgives at most one extra report, and the re-argument is
recorded here instead of inherited from the courtesy-line rationale — one
extra ping is the cost, and the conservative direction is fewer reports,
never more. The window instance is constructed where the tool set's
assembly finishes and injected into the tool at registration — the tool
never reaches into the assembly. Declined and error results teach no-retry
in the admission wrapper's established wording style.

## Rejected alternatives

- **A side channel to the adapter.** A third outbound path for one line.
- **A model-chosen target parameter.** Projection carries no message
  handles, and adding them ships identifiers to the provider against the
  recorded posture; the member's reply is ground truth.
- **Autonomous spam detection.** A model turn per unaddressed message is a
  cost decision the operator has not made — deferred, recorded.
- **A durable delivery cursor for reports.** At-least-once against a line
  that pings every admin.
- **Delivery through the deterministic return path.** The ingest call
  returned before the turn ran.
- **Waiting for the next successful turn on failure.** A report delivered
  an hour late, threaded onto old context, is a different product event.
- **An unbounded filing path.** A hidden-mention flood a hostile member
  controls.
- **Spending the window at delivery.** The edge would need the window, and
  a dropped delivery would re-arm a spent ping.

Refined 2026-08-23, at the unit's close. The failure wake delivers every
finalized undelivered block, not reports alone: the delivery cursor is one
high-water mark, and a report-only failure read would strand a finalized
answer behind an advanced cursor. A turn that dies after finalizing
narration therefore delivers that narration beside the threaded report and
the notice — more truthful than losing it. And the report window's grant is
taken atomically before the append and revoked on a failed append; the
spend-after wording the unit spec first carried is ratified as the shipped
take-then-revoke order, whose concurrency safety the spec's close section
records.
