# 0182 — the zero message id is named, not defended against

Date: 2026-08-31, with the editing unit.

## Context

The platform documents its message identifier as `0` for an ephemeral
message and for a message the server scheduled instead of sending: "In such
cases, this field will be 0 and the relevant message will be unusable until
it is actually sent."

## Decision

The adapter stores the id opaquely and the core treats every origin as an
opaque key, so a zero would let two distinct messages share one key — for
the revision reference exactly as for the reply reference and the deletion
mirror that already key on it. This unit adds no validity check for one
platform's sentinel value in the core, because a check on the SHAPE of an id
is the platform vocabulary the core must not carry, and the adapter decides
nothing. It is recorded here as the known edge, with its receipt, so the
unit that handles ephemeral messages inherits a stated fact rather than
rediscovering it.

## Rejected alternatives

- **Refusing an origin of `"0"` in the core.** A platform's sentinel spelled
  into platform-neutral code.
- **Letting the exception go unrecorded because it predates this unit.** The
  two new readers this unit adds make it worse, and an unwritten edge is the
  defect the design document exists to prevent.
