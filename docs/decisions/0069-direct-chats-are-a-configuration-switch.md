# 0069 — Direct chats are a configuration switch

Date: 2026-08-23

## Context

The assistant serves groups and direct chats through one entry point. A
deployment may want to open the group surface first and hold direct chats
back until the direct-chat feature set — privacy self-service above all —
ships. Holding back must mean holding back entirely: a direct message that
is merely unanswered still creates a principal row, a channel mapping and a
ledger block, which is exactly the personal data such a deployment wants
absent.

## Decision

The assembly takes a direct-chat switch, `on` or `off`, defaulting to on so
the repository's generic behavior is unchanged; a deployment spells `off`
in its configuration file. The value vocabulary is closed — anything but
the two words refuses the start at the decode, and an unknown key is
refused like every other unknown key in the file.

Off, the entry point refuses a direct-channel inbound before anything is
written — no mapping, no principal row, no ledger block, no answer, no
deterministic reply — mirroring the unauthorized-group refusal's
fail-closed shape, with its own outcome variant because there is nothing
for the adapter to perform: the adapter acknowledges the update and the
offset advances, so the loop never wedges behind a refused message. Group
channels are untouched either way, and direct-channel observations already
observe nothing.

While the switch is off, the privacy policy's direct-chat sentences
over-describe: they name processing that does not happen. That is the
harmless direction — describing more than is done — and the switch is
expected to open when the direct-chat feature set ships, at which point the
policy is exact again.

## Rejected alternatives

- **Dropping direct updates in the adapter.** Behavior in an adapter, and
  every further platform would need the same drop re-written.
- **Reusing the withdraw directive.** Withdraw instructs the adapter to
  leave a group; a person's chat has nothing to leave, and a directive that
  cannot be performed would push a platform call that can only fail.
- **Recording without answering, like the protection budgets.** Protection
  limits answering and deliberately keeps recording; this switch exists to
  keep the rows out entirely, so recording would defeat it.
- **Defaulting to off.** The repository is generic and its default behavior
  must stay what it always was; opting out is the deployment's explicit
  line.
