# 0133 — The architecture stated is the binary's own

Date: 2026-08-30, with unit 37.

## Context

"Which architecture do you run on" has two different answers on the same machine: the
architecture the host CPU offers, and the architecture the binary was compiled for.
They differ whenever a 32-bit build runs on a 64-bit host, or a translated build runs
under emulation.

## Decision

The architecture the compiler resolved for this binary. It is the one honest answer a
process can give about itself, it is always present, and it needs no read of anything.
A 32-bit build served by a 64-bit host is a 32-bit assistant, and that is what the row
states.

It is a named constant of the tool, beside the compiled-in version and the build
revision, so the byte pin over the assembled core states one name instead of restating
a value.

## Rejected alternatives

- **Reading the host's architecture from the operating system.** It answers a question
  nobody asked: a member asking what the assistant runs on is asking about the
  assistant, and a 64-bit answer from a 32-bit process is a confident misstatement of
  the software.
- **Stating both.** Two architectures on one row invites the reader to work out which
  one matters, which is the tool answering with a puzzle.
