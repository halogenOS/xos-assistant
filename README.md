# halogenOS Group Assistant

A community assistant for [halogenOS](https://halogenos.org), the performance-focused,
open-source Android distribution. It answers community questions and helps with recurring
group tasks across the project's chat platforms, Telegram first, through a shared
platform-neutral core and one thin adapter per platform.

**Status: pre-alpha.** The core spine stands: the platform-neutral core consumes the
ledger framework, records inbound messages as ledger blocks, takes turns against a
registered provider, and yields replies on a subscription edge. The Telegram adapter
speaks the Bot API directly — long polling in, plain sends out — with its update offset
persisted in a state file the embedder names (decisions 0013–0019). The live model is
in: the assistant answers through the OpenRouter provider with its key held in memory and
never stored, addressing decides which messages are answered (a direct message, a
mention, a reply to the assistant), a failed turn tells the chat once, and the
`assistant` binary embeds the pieces into a runnable process (decisions 0020–0028).
Flood protection is in: two configurable answering budgets — per sender and per chat —
limit what the assistant answers, never what it records, and every message carries the
authority of the debt it opens or propagates, the fact the coming tool unit's admission
reads (decisions 0029–0036). The feature tools with admission and spam reporting arrive
with the next unit.

## The framework checkout

The core depends on the agent-ledger framework, which has no public home yet (decision
0004): the manifest names a relative path, and the framework repository is expected as a
checkout named `agent-ledger` beside this repository's own directory. Clone the two side
by side before building.

## Running

The `assistant` binary takes exactly one argument, the path of a TOML configuration
file:

    nix develop -c cargo run -p assistant -- assistant.toml

The file names the store path, the Telegram state-file path, the prompt directory
(the repository's `prompts` directory holds the assistant's system prompt), the log
destination (the bare word `stderr` for the console, or a file path — as a bare
string, or as `log = { file = "..." }` for a file whose name collides with the
console word — the console word matches exactly and lowercase), the model id, and
optional endpoint overrides for tests. Secrets are referenced indirectly — an
environment variable name or a
file path per secret — and never appear in the file itself:

    store_path = "assistant.db"
    telegram_state_path = "telegram.offset"
    prompt_dir = "prompts"
    log = "stderr"
    model = "<the provider's id for the model>"

    [secrets.bot_token]
    env = "ASSISTANT_BOT_TOKEN"

    [secrets.openrouter_key]
    file = "openrouter.key"

    # Optional; the values shown are the defaults.
    [protection]
    principal_answers = 6
    principal_window_seconds = 600
    channel_answers = 20
    channel_window_seconds = 600

The protection table sets the two answering budgets: how many messages one sender is
answered per window (counted across every chat, direct and group alike) and how many
messages one chat is answered per window. The table and each of its fields may be
omitted — absent fields keep the defaults shown. A window of zero disables that
budget; an answer count of zero is refused at start, since disabling is the window's
job. Budgets limit answering only, never recording: an over-limit message is still
recorded with the refusing budget named on it, draws no reply and no notice — a
refusal notice would hand a flooder the assistant's voice — and cannot cancel an
answer someone else is still owed.

The process logs its startup facts — never a secret — and stops cleanly on SIGTERM.
Deployment wiring, including the group-privacy platform setting the record-all
policy depends on, stays outside the repository.

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
- `crates/assistant` — the runnable process: configuration, secret indirection, logging,
  and the assembled core with the OpenRouter provider and the Telegram adapter.
- `prompts` — the assistant's system prompt, loaded at start and recorded per
  conversation at its creation.
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
