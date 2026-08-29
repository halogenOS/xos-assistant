# 0134 — The homes are pinned constants, and the denial of the second one goes

Date: 2026-08-30, with unit 37.

## Context

The source row states where the software lives: this assistant's repository and the
repository of the framework it is built on. No manifest in the workspace carries a
repository key, and the repository's own written record still said the framework had no
public home. The framework was published on 2026-08-29 and announces that public name
in the user agent it sends.

## Decision

Two constants in the tool's own module, pinned by a test that states their text
character for character — the pattern the lookups' default hosts already use, so an
accidental edit fails loudly instead of quietly sending a member to a repository that
is not this software's.

The README's sentence saying the framework has no public home is corrected, and so is
the same denial in the core manifest's comment beside the dependency: a shipped tool
must not answer what a tracked file denies. Decision 0004 — this repository's own
record of the sibling-checkout path — stands as written and is revisited here only for
the public home it noted in passing. The dependency mechanics are untouched; moving the
manifest to a repository dependency is separate work that this unit does not start.

## Rejected alternatives

- **`CARGO_PKG_REPOSITORY`.** Empty in every manifest here today. Adding manifest keys
  so that a chat answer has something to read inverts the dependency: the packaging
  metadata would exist to serve one tool's row.
- **Configuration.** Where the software's source lives is a fact of the software, the
  same on every deployment. A configurable answer invites a deployment to state a
  repository that is not the one it was built from.
- **Leaving the README as it was.** A published sentence contradicting a shipped tool's
  answer, with the reader left to decide which of the two is lying.
