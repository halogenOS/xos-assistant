# Unit 32 — the assistant can state what it is running

Date: 2026-08-28. Asked what model she runs on, the assistant answers from training data
and from stale chat history — the live group watched her claim to be the model she was
swapped away from an hour earlier. The operator's instruction of this date: a
self-introspection tool returning the runtime facts the process actually knows.

One tool, no parameters, member authority, rendering a short fact list from values the
process holds: the configured model id, the binary's version and build revision, and the
process uptime. Truth from the process, never from the model's memory.

## Grounding

**The model id is configuration.** `crates/assistant/src/config.rs:33-36` — the provider's
identifier every conversation is created under, already resolved at startup. The tool
states this value; it does not ask the provider anything.

**No build information exists in the binary today.** No `env!`/`option_env!`/build-script
capture anywhere in the workspace (checked). The deployment builds from a Nix flake store
path that carries **no `.git`**, so a build script reading git at compile time answers
nothing there — the revision must arrive as a compile-time environment value the build
passes in. `option_env!` with an honest "unknown" fallback is the shape; the deployment's
half (passing its pinned revision into the build) is one line in the deploy repository and
is NOT part of this unit's tree.

**No uptime anchor exists.** `main` (`crates/assistant/src/main.rs:176`) captures no start
instant. One is captured once at startup and carried to the tool like every other
constructed-at-assembly value.

**The tool seam and its precedents.** Fixed-wording, parameterless-or-refusing tools with
member authority: `rights.rs` (fixed results), the registration-at-assembly shape
(`assembly.rs:442-456`). The tool renders prose, like every shipped tool.

**Latency is deliberately absent from v1.** No consumer seam measures a model round trip
today; the provider streams are driven inside the framework. A guessed or misattributed
latency is worse than none. The fact list is a shape that accepts new rows without
restructuring, which is the hollow-lattice requirement for adding latency later when the
framework exposes turn timing.

## Decisions taken with this unit

- **Facts, verbatim, 2026-08-28.** The tool returns exactly these lines, each from the
  named source:
  - `model: {configured model id}` — from configuration.
  - `version: {crate version}` — `env!("CARGO_PKG_VERSION")`.
  - `revision: {build revision}` — `option_env!("ASSISTANT_BUILD_REVISION")`, or the
    literal `unknown` when the build passed none. Never a guess, never a git call at run
    time.
  - `uptime: {duration}` — since the start instant, rendered coarsely (days, hours,
    minutes); seconds precision suggests a freshness the fact list does not otherwise
    have.
  *Rejected:* latency rows in v1 (no honest source — see grounding); *rejected:* asking
  the provider for its model name (a network call to restate configuration, and a
  different answer than the one the wire actually uses).
- **No parameters, and extra arguments change nothing, 2026-08-28.** There is nothing to
  select. *Rejected:* a per-fact query parameter (a vocabulary to maintain for four
  lines of output).
- **Member authority, 2026-08-28.** Which model and how long up are facts the operator
  states publicly in the group anyway; the group's trust in her answers is the point of
  the tool. *Rejected:* admin-only (it would make her unable to answer the exact question
  members ask).
- **The teaching routes identity questions to the tool, 2026-08-28.** One sentence: asked
  what she runs on or how long she has been up, she calls the tool and answers from it,
  never from memory or from the chat. Gated on nothing — the tool has no configuration to
  be absent. *Rejected:* leaving the routing to chance, which is the failure the live
  group already produced.
- **No privacy-document change, 2026-08-28.** The facts describe the process, not any
  person; nothing new is stored or sent to any recipient. Deliberate claim, checked in
  review against the published documents rather than assumed.

## The unit's contract

The model can call one parameterless tool at member authority and receives the four fact
lines — model id from configuration, crate version, build revision or the literal
`unknown`, and coarse uptime — rendered exactly as specified, from process-held values,
with no network call and no git invocation. The teaching tells her to answer
what-are-you-running-on questions from the tool. Extra arguments are ignored. Nothing new
is stored, no recipient receives anything new, and no privacy document changes.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The rendering is byte-exact for a known tuple of inputs — pinned character for
  character, including the `revision: unknown` form.
- **AC3** The model id in the result is the configured one — pinned by constructing the
  assembly with a distinct id and reading it back through the tool.
- **AC4** Uptime is coarse and monotonic-sourced — pinned that the rendering carries no
  seconds field and that the anchor is captured once, not per call.
- **AC5** The tool is reachable by an ordinary member through the palette — pinned via
  admission, not by calling the handler directly.
- **AC6** The teaching sentence is in the composed prompt in both answering modes —
  pinned.
- **AC7** No network and no subprocess: the tool's execute path performs no I/O beyond
  reading process-held values — checked as a property of the code, and the no-git claim
  pinned by building without any git available.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-introspect`, branch
  `unit/runtime-facts`). Sites: a new tool module beside `core/src/tools/rights.rs`,
  registration at the assembly, the start instant in `crates/assistant/src/main.rs`
  threaded like the other assembly inputs, the teaching line, and the spine tests.
- The deployment's one-liner (passing its pinned revision as
  `ASSISTANT_BUILD_REVISION`) belongs to the deploy repository and ships with the next
  deploy; until then production answers `revision: unknown`, which is the designed
  honest form, not a defect.
- Small and targeted is the point: if the diff grows past a tool module, a start
  instant, one teaching sentence and tests, something is over-built.
