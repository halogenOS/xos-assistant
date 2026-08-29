# Unit 37 — the runtime facts state the OS and the software's homes

Date: 2026-08-30. The operator's instruction, 2026-08-29, verbatim: "the self
introspection should gain OS information (just the distro, the architecture, and also
links to the software it runs on)". The runtime-facts tool answers what the assistant
is and where it runs; the distribution, the CPU architecture and the public homes of
the software join the same answer.

## Grounding

**The tool was shaped for this.** `fact_lines` (`core/src/tools/runtime.rs:97-125`)
renders the result as rows and its own doc says a new fact joins as another row without
restructuring — unit 34 added the two clock rows exactly this way, and unit 34 also
recorded that a new operator-named fact joins THIS tool, never a second one. Readings
arrive as execute-time arguments, never fields, which keeps the tool's no-cache size
pin green (`runtime.rs:454-460`). Registration is unconditional
(`core/src/assembly.rs`, the assembled-tools admission); nothing about it changes.

**What exists to source the three facts.** Nothing in either tree reads
`/etc/os-release` today, and no crate for it exists in the workspace: the read is new,
consumer-side. The architecture is `std::env::consts::ARCH`, compile-time, always
present. No manifest carries a `repository` key and the tree's precedent for stating a
URL is a pinned module constant with a pin test (the commit lookup's default hosts,
`core/src/tools/mod.rs:172-180`).

**The contract sentence this widens.** The tool header promises the execute path
"reads process-held values only — no network call, no subprocess"
(`runtime.rs:33-38`), and unit 32's AC7 pinned that promise. A distribution read is a
host FILE read. The promise that matters — no network, no subprocess, nothing that can
hang or leak — survives intact; this unit records the widened wording (process-held
values and one named host file) here, in its own doc, never by editing unit 32's.

**The decline stays whole.** An unreadable conversation record declines the entire
list (`UNREADABLE_RESULT`, `runtime.rs:74-80`). The OS facts do not depend on that
read, and answering them alone would grow a partial-answer branch the tool was built
without; the single decline stays, deliberately.

## Decisions taken with this unit

- **Three rows join, nothing else, 2026-08-30.** `os:` (the distribution), `arch:`
  (the CPU architecture), and `source:` (the public homes). The operator named exactly
  these; no kernel version, no hostname, no uptime duplication. *Rejected:* a separate
  `os_info` tool — unit 34 recorded that operator-named facts join the runtime tool.
- **The distribution is read from `/etc/os-release` at execute time, `PRETTY_NAME`
  first, `NAME` as the fallback, `unknown` when the file answers nothing,
  2026-08-30.** Execute-time, so a rebuilt host answers truthfully without a restart;
  a plain line-parse of the two keys, no dependency. The absent case follows the
  revision precedent (`UNKNOWN_REVISION` renders the literal `unknown`): the fact is
  named and should exist, so its absence says so — silence is the zone precedent, for
  parts whose absence is normal. *Rejected:* an os-info crate for two keys;
  *rejected:* a subprocess (`uname`) — the header's promise;
  *rejected:* compile-time capture — it would state the build host's OS, not the one
  the process runs on.
- **The architecture is `std::env::consts::ARCH`, 2026-08-30.** The binary's own
  architecture, compile-time by nature, always present — the one honest answer a
  process can give about itself. *Rejected:* reading the host's architecture from the
  OS — a 64-bit host running a 32-bit binary would then misstate what the software
  actually is.
- **The homes are two pinned constants in the tool module, 2026-08-30.** The
  assistant's repository `https://github.com/halogenOS/xos-assistant` and the
  framework's `https://github.com/xdevs23/ronna-core`, rendered on one `source:` row,
  pinned like the commit lookup's default hosts. Both are public facts: the README
  already links the framework author's public repositories, and the framework
  announces its public name in its user agent. The framework's own manifest stays
  untouched — its decision 0004 (no repository key until a home is committed to) is
  the framework's to revisit, and a consumer stating where its software lives
  contradicts nothing. *Rejected:* `CARGO_PKG_REPOSITORY` — empty in every manifest
  today, and adding manifest keys to answer a chat question inverts the dependency;
  *rejected:* configuration — the homes are facts of the software, not of a
  deployment.
- **The teaching and the description name the new questions, 2026-08-30.** "What OS
  do you run on", "what are you built on" route to the tool the same way the clock
  questions do; the description's enumeration grows, and the verbatim teaching pin
  moves with the sentence it pins.
- **The privacy claim is re-taken, not assumed, 2026-08-30.** The three facts describe
  the software and its host distribution — no person, no message content, no new
  recipient; the result rides the existing conversation to the existing model
  processor. The reviewer checks the published documents rather than trusting this
  paragraph, unit 32's own convention.

## The unit's contract

The runtime-facts tool's answer gains three rows — `os:` with the host distribution
read from `/etc/os-release` at execute time (`unknown` when unreadable), `arch:` with
the binary's architecture, and `source:` with the two pinned public repositories — in
the same single prose result, declined whole exactly as today when the conversation
record is unreadable. The execute path still makes no network call and spawns no
subprocess; its one new read is the named host file. The teaching and the tool
description route the new questions. No parameter joins, no new dependency, no
configuration, no privacy-document change, and no change to any other fact's wording.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The three rows render byte-exact at both granularities the tool already
  pins (module `fact_lines` tests and the assembled-core spine pin), driven with an
  injected os-release shape so the pins are deterministic.
- **AC3** The absent case is honest: an unreadable or missing os-release renders
  `os: unknown` — pinned; and the parse takes `PRETTY_NAME` over `NAME` — pinned.
- **AC4** The unreadable conversation record still declines the WHOLE list, OS rows
  included — pinned.
- **AC5** The teaching sentence routes the OS questions and its verbatim pin moves
  with it; the description enumerates the new questions — both pinned.
- **AC6** No subprocess and no network call ride the execute path — held by the
  restated contract in the tool header and checked in review, with the os-release
  read the one named file access.
- **AC7** The decision records land as numbered documents continuing from the highest
  shipped (0130), each dated, each naming this unit, each with rejected alternatives.

## Notes for launch

- Worktree `~/projects/halogenos-assistant-osinfo`, branch `unit/os-information`.
  Sites: `core/src/tools/runtime.rs` (rows, constants, header contract), the
  os-release read (a small seam in the tool module taking the file's text so tests
  inject shapes; production hands it the real file), `prompts` teaching plus
  `core/src/teaching.rs` composition if the sentence lives there, the description,
  the module and spine pins, `docs/decisions/0131+`.
- The build's first step is `git rebase main`; unit 31 may merge first and this
  branch predates it.
- The quality bar from the operator, verbatim scope for the reviewers: "The code must
  always be better and cleaner afterwards than it was before. If you had to add a
  snowflake if somewhere, it's a smell."
