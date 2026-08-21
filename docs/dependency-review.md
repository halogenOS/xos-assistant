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
