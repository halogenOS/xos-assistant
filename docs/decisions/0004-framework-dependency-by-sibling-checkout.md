# 0004 — Framework dependency by sibling checkout

Date: 2026-08-21

## Context

The core is built on the ledger framework this project's storage decision adopted. The
framework library is complete and stands alone, but it has no public home yet: publishing
it is its maintainer's call, and that call has not been made. The core's manifest still
has to name the dependency somehow.

## Decision

The manifest names a relative path dependency on a sibling checkout of the framework
repository, and the README states the expected layout. When the framework gains a public
home, the path becomes a normal registry or git dependency and nothing else changes.

## Rejected alternatives

- **A git URL.** No remote exists to name; inventing one now would pin the manifest to a
  location nobody has committed to.
- **Vendoring the framework into this repository.** A fork of our own fresh fork: two
  trees to keep aligned for zero benefit, and every framework fix would need a second
  copy step.
