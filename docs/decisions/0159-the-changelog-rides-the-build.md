# 0159 — The changelog rides the build

Date: 2026-08-30, with unit 47.

## Context

A group member asks what changed in the assistant, and the assistant has
nothing to read: the deployment's source tree carries no version-control
metadata, and a model asked about its own changes answers from training
data or from the chat — both invention about the very software doing the
answering. Decision 0158 already made the version move per deployment;
this decision is where the answer about what moved comes from.

The operator settled the shape in the asking: one tool, one result, the
entire changelog since the previously deployed version, and a name that
cannot be confused with the halogenOS changelog. A tool that can browse
the whole git history comes later and is deliberately not this one.

## Decision

The changelog is embedded in the build. The deployment generates one text
— per commit in the deployed range: the date and time, the commit title
and the full commit message — and passes it as the compile-time
environment value the harness-changelog tool reads. The tool takes no
parameters and answers with exactly that text, verbatim, one result: no
pagination, no per-entry structure, no filtering. Nothing at compile time
and nothing at run time reads git, the network or the filesystem for it.

The generation is the deployment repository's concern; this repository
names the variable and the per-commit fields, and performs no generation.

A build that passes no changelog gets a tool that states its absence —
the same honesty register the runtime facts use — and instructs the model
to say so instead of recalling or inventing one. The tool registers
unconditionally with the assembly, exactly as the runtime facts do: an
absent value is a tool that answers its absence, not a tool that
vanishes.

## Rejected alternatives

- *Reading the git history in the build or at run time* — the deployment
  builds from a source tree with no version-control metadata, so there is
  no history to read where the binary is built; and history browsing is a
  capability of its own, settled for a later, deliberate tool.
- *A per-entry or paginated tool* — the operator asked for one result
  containing the entire since-last-version text; structure and paging
  would answer a question nobody asked yet.
- *Registering the tool only when a value is present* — an absent
  changelog is a fact about the build, and the honest answer to it is the
  stated absence, not a missing tool.
