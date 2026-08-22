# 0029 — Unit four is protection plus the recorded authority fact

Date: 2026-08-22

## Context

The stage plan's fourth entry bundled flood protection, the one-authority turn, the
feature tools and spam reporting into one unit.

## Decision

The stage is re-sliced: unit four ships protection and the recorded debt-authority
fact; unit five ships the tools with admission and the spam reporting. The reason is
observability — one-authority enforcement has no observable effect until tool
admission exists, so shipping enforcement without tools would ship code nothing can
exercise, while the authority fact itself is a write-time stamp the protection unit
already touches. The tool unit then reads a stamped fact that has been recorded and
tested one unit earlier.

## Rejected alternatives

- **Keeping the original bundle.** One oversized unit whose enforcement half could
  only be proven through tools written in the same change — twice the surface, none
  of it verifiable in isolation.
- **Deferring the authority stamp to the tool unit too.** The stamp shares the
  write path this unit reworks; deferring it would reopen the same serialization
  one unit later.
