# 0042 — Admission is a consumer check at the top of execute

Date: 2026-08-22

## Context

The framework's admission chain offers no consumer seam with ledger access: its
gate hook receives the input string alone, and tool-call inserts are
framework-internal with fixed columns, so provenance cannot be stamped at
insert.

## Decision

The mechanism that exists is used: one admission wrapper shared by every tool
handler, whose execute first reads the palette block and resolves the
provenance through the tool context's ledger access, and declines — returning
the recorded tool error, no network touched — before the tool body runs.
"Declined, never executed" means the tool's body; the wrapper is technically
entered. Both missing seams join the framework improvements list: a
context-bearing gate, and a consumer fact on the tool-call insert.

## Rejected alternatives

- **Waiting for the framework seams.** The gate would ship ungated until the
  framework moved; fail-closed cannot wait on someone else's release.
- **Per-tool hand-written checks.** One rule, one place — the wrapper. A copy
  per tool is the copy that drifts.
