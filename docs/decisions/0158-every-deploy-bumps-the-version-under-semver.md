# 0158 — every deploy bumps the version, under semver, major pinned at zero

Date: 2026-08-30.

The assistant reports its own version and build revision through the
runtime-facts tool (decision 0131; the revision wiring per unit 32). For that
report to mean anything, the version must move when the software does: a
deployment that changes behavior while the number stands still makes the
honest answer a lie of omission.

## Decision

Every deployment is preceded by a version bump in this repository's workspace
manifest, following semantic versioning with the major pinned at zero while
the project is pre-1.0:

- a deployment carrying any new behavior or feature moves the MINOR;
- a fix-only deployment moves the PATCH;
- the major stays 0 until a deliberate decision moves it.

The workspace manifest's one `[workspace.package] version` is the single
recorded value; every crate inherits it, the compiled binary reports it, and
the deployment derivation reads it from the pinned source manifest rather
than carrying a second literal.

## Rejected

- *A version bumped on a schedule or per commit* — the number would move
  without meaning; it moves per DEPLOYMENT because that is the unit a group
  member can observe.
- *A build number or date in place of semver* — the operator chose semver,
  and semver's fix/feature distinction is exactly the answer to "did
  anything change for me".
- *Leaving 0.1.0 until 1.0* — the incident that motivated this record: the
  assistant answered "no new updates that I can see" while running freshly
  deployed code, because nothing it could read had changed.
