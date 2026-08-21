# halogenOS Group Assistant

A community assistant for [halogenOS](https://halogenos.org), the performance-focused,
open-source Android distribution. It answers community questions and helps with recurring
group tasks across the project's chat platforms, Telegram first, through a shared
platform-neutral core and one thin adapter per platform.

**Status: pre-alpha.** The core spine stands: the platform-neutral core consumes the
ledger framework, records inbound messages as ledger blocks, takes turns against a
registered provider, and yields replies on a subscription edge. The adapters are still
skeletons, and the registered provider is a scripted one — the live model arrives with a
later unit.

## The framework checkout

The core depends on the agent-ledger framework, which has no public home yet (decision
0004): the manifest names a relative path, and the framework repository is expected as a
checkout named `agent-ledger` beside this repository's own directory. Clone the two side
by side before building.

## Development

Cargo runs inside the Nix development shell, which provides the toolchain:

    nix develop -c cargo test --workspace
    nix develop -c cargo clippy --workspace --all-targets -- -D warnings
    nix develop -c cargo fmt --check

## Layout

- `crates/core` — the platform-neutral core: conversation handling, knowledge lookup,
  command semantics, rate and abuse protection.
- `crates/adapters/telegram` — the Telegram adapter: translation between the Telegram Bot
  API and the core's message model.
- `docs/decisions` — decision records, numbered in order.
- `docs/dependency-review.md` — every dependency's version and advisory check, recorded
  before the manifest names it.
- `docs/platform-vocabulary.txt` — the word list the no-platform-vocabulary test greps
  the core against; each adapter adds its platform's terms here.

The architecture rules that bind every change, including the two invariants that separate
the core from the adapters, are written down in `CLAUDE.md`.

## License

GNU General Public License v3.0 — see [LICENSE](LICENSE). The storage and orchestration
subsystems are adopted from [ronna-lightspeed](https://github.com/xdevs23/ronna-lightspeed),
which carries the same license.
