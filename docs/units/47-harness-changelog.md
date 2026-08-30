# Unit 47 — the harness changelog tool

Date: 2026-08-30. The operator's direction, which settled every open corner in
the asking: the changelog — date and time, commit title and full commit message
per commit — rides the build so the model can obtain it through a changelog
tool; the tool is named `harness_changelog` so it cannot be confused with the
halogenOS changelog; it answers with ONE result containing the entire changelog
since the last version, not per-commit entries; and a more sophisticated tool
that browses the whole git history comes later — for now the text is embedded
in the build.

## What this unit builds

One model-facing tool, `harness_changelog`, that answers with the assistant
software's own changelog — everything since the previously deployed version — as
ONE text result, verbatim from a value embedded in the build. No git access, no
history browsing, no network: the sophisticated history tool is explicitly LATER
and not this unit.

The tool exists so the model can answer "what changed / what's new in you" from
the record instead of memory, exactly as `runtime_facts` (unit 32) ended invention
about model, version and revision. It is about the ASSISTANT SOFTWARE (the
"harness"), never the halogenOS operating system's changelog — the name and the
tool description both carry that distinction.

## The mechanism — the revision idiom, extended

Follow `crates/core/src/tools/runtime.rs` exactly; it is the settled precedent:

- The changelog arrives as a compile-time environment value,
  `option_env!("ASSISTANT_BUILD_CHANGELOG")`, beside `ASSISTANT_BUILD_REVISION`.
  The deployment generates the text (per commit in the deployed range: date+time,
  commit title, full commit message) and passes it in at build time; nothing at
  compile time or run time could read git history from the deployment's source
  tree, which carries no version-control metadata. The generation itself is the
  deployment repository's concern and no code in THIS repository performs it.
- A `const fn` resolves the absent case in ONE place (the `resolve_revision`
  shape). A build that passes no changelog answers a stated absence: the tool's
  result says plainly that this build carries no changelog, and instructs the
  model to say so rather than recall or invent one (the `UNREADABLE_RESULT`
  register). `unknown`-style honesty, never fabrication.
- A present changelog is returned VERBATIM as the tool result, the whole text,
  one call, one result. No pagination, no per-entry structure, no filtering: the
  operator ruled one result containing the entire since-last-version text.

## The tool's shape

- New module `crates/core/src/tools/changelog.rs`, patterned on `runtime.rs`:
  `NAME = "harness_changelog"`, a `ToolDefinition` with an empty-object parameter
  schema (the tool takes nothing), an `execute` returning `ToolOutcome::Done`
  with the embedded text or the stated absence.
- `REQUIRED_AUTHORITY: Authority::Member` — what changed in the assistant is a
  fact any group member may ask about, the same reasoning recorded on
  `runtime_facts`.
- The tool description states, in plain words, that this is the changelog of the
  assistant software itself (the harness it runs as), NOT the halogenOS
  operating system's changelog, and that ROM/OS release questions are a
  different tool's business (the release lookup exists for those).
- Registration: the tool joins UNCONDITIONALLY at the assembly, beside
  `RuntimeFacts` (`crates/core/src/assembly.rs`, the unit-32 admit site) — it
  has no configuration to be absent; an absent value is a tool that answers its
  absence, not a tool that vanishes. Palette membership follows from the
  existing derivation (`into_registry`); nothing else is touched.

## Teaching

The identity section (`crates/core/src/teaching.rs`, `identity_section`) already
routes model/version/uptime questions to `runtime_facts`. This unit extends the
same sentence family: when someone asks what changed, what is new, or what was
updated in the assistant itself, the model calls `harness_changelog` and answers
from what it returns — never from memory, never from the conversation. The
sentence must keep the existing distinction sharp: questions about halogenOS
releases stay with the release lookup; only questions about the ASSISTANT go to
the changelog tool.

## Acceptance criteria

- AC1: `harness_changelog` is registered unconditionally, appears in every new
  conversation's palette, and requires Member authority — pinned by test at the
  assembly or set level, the way unit 32's registration is pinned.
- AC2: with the compile-time value present, the tool returns the embedded text
  verbatim as one result — pinned over the resolve function with injected
  shapes (the compile-time env itself cannot vary under test; the resolve
  function is the tested surface, exactly as `resolve_revision` is tested).
- AC3: with the value absent, the result is the stated-absence text, byte-pinned,
  and it instructs honesty rather than recall.
- AC4: the teaching names the tool for assistant-change questions and the
  sentence keeps the assistant/OS distinction — pinned the way existing teaching
  sentences are pinned.
- AC5: the tool description text contains the distinction from the OS changelog
  — pinned.

## Bounds

- No git, no network, no filesystem read in this tool. One embedded value, one
  result.
- No new dependency.
- The deployment-side generation (the range, the format's exact rendering, the
  wiring that passes the env value) is the deployment repository's commit, not
  this one. This repository only names the variable and the per-commit fields
  the operator asked for: date+time, commit title, full commit message.
- A decision record documents the embed-now-browse-later ruling with its date
  and the rejected alternative (in-build git history):
  `docs/decisions/0159-the-changelog-rides-the-build.md`.
