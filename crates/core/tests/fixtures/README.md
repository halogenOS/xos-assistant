# Test fixtures

Recorded inputs the core's tests read instead of reaching a network or an
older build. `wiki-index.html` is one captured wiki index page; the store
below is the one file here that has a generating command.

## `previous-build.sqlite`

A store written by the build that PRECEDES unit 55, kept so the projection
can be proven not to have changed what an existing database renders.

The rows in it are the shapes the projection has to keep rendering: a member
message with a handle, one without, one whose own prose carries a `---`
fence line, a revision recorded under its own id, a row an erasure emptied,
and a join notice. Each carries the platform's own send time.

It records domain migration version 21 — the count before this unit's two
appended steps — which is what the equivalence test asserts, so a fixture
regenerated at the wrong commit fails loudly instead of quietly becoming a
new-build one.

## Regenerating it

The writer is `crates/core/examples/previous_build_fixture.rs`, committed so
the file has a recipe. Run it at the previous build's commit, in a worktree
of this repository, with the example source placed there:

```sh
git worktree add --detach ../previous-build 4d56841
mkdir -p ../previous-build/crates/core/examples
cp crates/core/examples/previous_build_fixture.rs ../previous-build/crates/core/examples/
cd ../previous-build
cargo run -p assistant-core --example previous_build_fixture -- /tmp/previous-build.sqlite
```

The store opens write-ahead, so consolidate it into the single file this
directory holds before copying it in:

```sh
sqlite3 /tmp/previous-build.sqlite 'PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; VACUUM;'
cp /tmp/previous-build.sqlite crates/core/tests/fixtures/previous-build.sqlite
```

Then remove the worktree — its own command, after the copy is verified:

```sh
git worktree remove --force ../previous-build
```
