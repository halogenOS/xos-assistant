# 0068 — Titles derive on a configured title model, main model by default

Date: 2026-08-23.

## Context

The framework derives a short conversation title in the background, over at
most the first eight blocks, on whatever its provider module names as the
cheap background model. An audit found the gateway provider naming that model
as a hardcoded slug — a model of another vendor, deployed outside the EU — so
a deployment whose main model is EU-pinned still shipped its title requests,
user prose included, to a region nobody chose. The assistant's deployment
posture treats the configured model as the one model member text may reach.

## Decision

The background model becomes configuration, end to end, with the main model
as the only default:

- The framework's gateway provider reads an optional background-model slug
  from its instance configuration. Configured, title requests go out on
  exactly that slug; not configured, they go out on the request's own main
  model — the selector carries the conversation's configured model as the
  fallback, so no hardcoded id exists to cross vendors or regions silently.
- The assistant exposes this as the optional `title_model` configuration key,
  refused when present but empty (trimmed like its sibling keys, unknown keys
  refused as everywhere in the file), flowing into the provider's in-memory
  configuration. Omitting the key is how the main-model default is chosen.

Cost, for the record: one title derivation per conversation, over at most
eight blocks — the main-model default is a negligible price for a correct
region.

## Rejected alternatives

- **Keep a hardcoded background slug.** The defect itself: a model id chosen
  by the framework, not the deployment, silently crossing vendors and
  regions.
- **Hardcode an EU slug instead.** Trades one deployment's correctness for
  every other deployment's; any fixed id repeats the class of defect.
- **Skip title derivation when no title model is configured.** Loses the
  feature to avoid a negligible cost, and makes the unconfigured default a
  silent behavior change instead of a safe one.

## Closed 2026-08-23

Decision 0077 switches title derivation off entirely: the operator decided
the feature is not wanted — no surface reads a derived title. The
region-correctness reasoning above stands for any consumer that derives
titles; this deployment no longer does, the `title_model` key is removed
with the feature, and the rejected alternative "skip title derivation" is
superseded by that explicit decision — not adopted silently.
