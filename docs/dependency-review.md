# Dependency review

Every dependency is checked before a manifest names it, and the check is recorded here.
Two questions, both answered against a source instead of from memory:

1. **What is the current version?** Looked up on the registry at the time of adding,
   never recalled.
2. **Does that exact version carry a known compromise?** Checked against the OSV
   advisory aggregate, which carries the Rust advisory database. A version number alone
   is not evidence of safety — malicious releases reach package registries regularly.

A dependency that fails either question does not go in. One that passes is recorded with
the date it was checked.

## 2026-08-21 — the core spine

Current versions from the crates.io API, advisories from the OSV API, both queried on the
day of this entry.

| Crate | Version | Latest at check | Advisories | Why it is here |
|---|---|---|---|---|
| agent-ledger | path | — | — | The ledger runtime the core is built on. Reviewed in its own repository: its `docs/dependency-review.md` covers every crate it pulls in, so the transitive tree is not re-reviewed here. |
| serde_json | 1.0.151 | 1.0.151 | none | The consumer write path takes block fields as a JSON map; the framework's API names the type. |
| tokio | 1.53.1 | 1.53.1 | none | The outbound edge is a spawned task and a channel; the framework already runs on this runtime. |
| chrono | 0.4.45 | 0.4.45 | none | The inbound message's timestamp type; the framework already depends on it. |
| thiserror | 2.0.20 | 2.0.20 | none | The core's error type derives its `Error` impl; the framework already depends on it. |
| tracing | 0.1.44 | 0.1.44 | none | The outbound edge runs in a background task; a failure there has no caller to return to, so it is logged instead of swallowed. The framework already depends on it. |
| rusqlite | 0.40.2 | 0.40.2 | none | The framework's consumer seam (`domain_run`) hands closures a `rusqlite::Connection` without re-exporting the crate, so the core's own tables are queried through a direct dependency on the same major version. No features named: the framework's `bundled` engine is what actually links. |

All six registry crates resolved to the current latest at the time of the check.

**The test-only split.** tokio appears a second time in the development section with the
macro attribute and the multi-threaded runtime flavour the tests need. Cargo unifies
features across a dependency graph, so a feature enabled in the library section would be
compiled into every dependent tree whether it wanted it or not.

## 2026-08-21 — the first platform adapter

The adapter speaks the platform's HTTP API directly, so its manifest names two crates
this repository's manifests had not named before. Current versions from the crates.io
API, advisories from the OSV API, both queried on the day of this entry. The other
crates the adapter's manifest names — chrono, serde_json, thiserror, tokio, tracing and
the framework itself — are already reviewed in the core-spine entry above and are not
re-recorded.

| Crate | Version | Latest at check | Advisories | Why it is here |
|---|---|---|---|---|
| reqwest | 0.13.4 | 0.13.4 | none | The HTTP client the adapter's wire runs over; the framework already depends on this version, and its review names the transitive stack (hyper, rustls, aws-lc-sys). Only the `json` feature is named; the crate's own default supplies TLS with a vendored provider, the same reasoning the framework's review records. |
| serde | 1.0.229 | 1.0.229 | none | The adapter's minimal update model derives its decoding; the framework already depends on this version. |

Both resolved to the current latest at the time of the check.

**The test-only additions.** The adapter's development section adds tokio's `macros`,
`rt-multi-thread`, `net` and `io-util` features — the scripted platform server in the
test suite is a plain listener on the loopback interface, written against tokio's own
networking, so the suite adds no new crate. Cargo unifies features across a dependency
graph, so these stay in the development section for the same reason the core's do.

## 2026-08-22 — the live model

The runnable process reads a TOML configuration file and writes structured logs, so the
binary crate's manifest names two crates this repository's manifests had not named before.
Current versions from the crates.io API, advisories from the OSV API, both queried on the
day of this entry. The other crates the binary names — the workspace's own two, the
framework, serde, thiserror, tokio, tracing — are already reviewed above and are not
re-recorded.

| Crate | Version | Latest at check | Advisories | Why it is here |
|---|---|---|---|---|
| toml | 1.1.4 | 1.1.4 | none | The configuration file's format; serde-decoded into the binary's own typed configuration. |
| tracing-subscriber | 0.3.23 | 0.3.23 | none | The process's log writer: the whole tree already speaks tracing, and this is that project's own subscriber crate. Only the `fmt` and `env-filter` features are named. |

Both resolved to the current latest at the time of the check.

**The chat-completions feature's transitive additions.** The core's `chat_completions`
feature enables the framework's own `openrouter` feature (that project's name for its shared
chat-completions wire), which pulls `eventsource-stream` (with `nom` and `minimal-lexical`
under it) into the lock file.
These arrive through the framework, whose own dependency review records them; per the
core-spine entry's rule, the transitive tree behind the framework is not re-reviewed here.

**The feature-only additions to reviewed crates.** The core's development section adds
tokio's `net` and `io-util` for the scripted completions server; the binary names
tokio's `rt-multi-thread`, `signal` and `time` — its runtime, its stop, and the timer
driver behind the adapter's waits, which the binary enables because it is what builds
the runtime — and its development section adds `net` and `io-util` for the
process-level tests' scripted servers —
features of a crate reviewed above, staying in the sections that need them because
Cargo unifies features across a dependency graph.

## 2026-08-22 — the tools

The two lookup tools perform bounded HTTP GETs from the core, which gives the core its
first network dependency. Current version from the crates.io API, advisory check from the
OSV API, both queried on the day of this entry — before the manifest named the crate.

| Crate | Version | Latest at check | Advisories | Why it is here |
|---|---|---|---|---|
| reqwest | 0.13.4 | 0.13.4 | none | The lookup tools' HTTP client — the same crate, at the same major version, that the framework and the adapter already run their wires over, so the workspace keeps one HTTP stack and one TLS story. Only the `json` feature is named; the crate's own default supplies TLS with a vendored provider, per the adapter entry's reasoning. |

Resolved to the current latest at the time of the check; the OSV query for the exact
version returned no advisory.

**The test-only additions.** The suites' scripted forge and mirror servers are plain
listeners on the loopback interface written against tokio's own networking, which the
core's and the binary's development sections already enable — no new crate and no new
feature arrives for them.

## 2026-08-29 — the web search

The web search posts JSON to a search vendor and reads a JSON answer back. **No
dependency was added for it**, and this entry records the check that established
that rather than an addition:

- The request goes through the core's shared lookup layer, which already runs on
  `reqwest` with its `json` feature — reviewed in the tools entry above, at the same
  major version the framework and the adapter run on. The layer grew a bounded POST
  seam beside its bounded GET; a POST is the same client, the same timeout, the same
  redirect refusal and the same body cap.
- The query guard needs no unicode table. A confusables dependency (a UTS-39
  implementation) was considered and REJECTED on its merits, recorded in decision
  0115: the guard exists against a careless model, not an adversary, and the
  normalisation it does need — dropping formatting characters, collapsing a single
  separator inside a candidate token — is a dozen lines against `char` classes.
- The suite's scripted vendor is the loopback listener the other lookups' fixtures
  already use, written against tokio's own networking, which the core's development
  section already enables.

Nothing in this unit's manifests changed, so there was no version to look up and no
registry history to check — which is itself the answer this rule asks for.
