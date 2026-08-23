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
mention, a reply to the assistant), a failed turn tells the chat once — except a
provider refusal for lack of balance, which stays out of the chat entirely — and the
`assistant` binary embeds the pieces into a runnable process (decisions 0020–0028).
Flood protection is in: two configurable answering budgets — per sender and per chat —
limit what the assistant answers, never what it records, and every message carries the
authority of the debt it opens or propagates (decisions 0029–0036). The tools are in:
three project lookups — a commit lookup against the project's canonical forge, a
release lookup against the builds repository on the mirror, and a wiki lookup reading
the project wiki's raw pages — behind a per-conversation tool palette that fails
closed and supersedes itself when the registered set changes, with tool authority
enforced by the anchor gate over the turn's provenance (decisions 0037–0046). Group
context is in: the group's title and pinned rules reach the model as context notes,
group membership is authorized fail-closed by the operator's own invitation, and the
`/privacy` command answers deterministically (decisions 0047–0055). The report is in:
a member who replies to an offending message and asks makes the assistant file a spam
report with the group's moderation bot, delivered as a threaded reply to the reported
message (decisions 0058–0063).

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
console word — the console word matches exactly and lowercase), the model id (with an
optional `title_model` conversation titles are derived with — absent, titles derive
on the main model), and optional endpoint overrides for tests. Secrets are
referenced indirectly — an
environment variable name or a
file path per secret — and never appear in the file itself:

    store_path = "assistant.db"
    telegram_state_path = "telegram.offset"
    prompt_dir = "prompts"
    log = "stderr"
    model = "<the provider's id for the model>"

    # Optional: the address the /privacy command answers with. Absent, the
    # command answers a fixed not-yet-published line instead.
    #privacy_policy = "https://example.org/privacy"

    # Optional: the moderation bot's handle the report tool files toward,
    # with or without the leading @. Absent, the report tool is not
    # registered and the assistant cannot file reports.
    #moderation_handle = "moderation_bot"

    # Optional: the model conversation titles are derived with. Absent,
    # titles derive on the main model above — never on a model the file
    # does not name.
    #title_model = "<the provider's id for the title model>"
    # Optional: whether direct chats are served — "on" (the default) or
    # "off". Off, a direct message is refused before anything is written:
    # nothing is recorded, nothing is answered. Groups are unaffected.
    #direct_chats = "off"

    [secrets.bot_token]
    env = "ASSISTANT_BOT_TOKEN"

    [secrets.openrouter_key]
    file = "openrouter.key"

    # Optional: a mirror API token for the release lookup. Absent, the
    # lookup runs unauthenticated at the mirror's lower rate limit.
    #[secrets.mirror_token]
    #env = "ASSISTANT_MIRROR_TOKEN"

    # Optional; the values shown are the defaults.
    [protection]
    principal_answers = 6
    principal_window_seconds = 600
    channel_answers = 20
    channel_window_seconds = 600

    # The operator per adapter: the external id of the one account whose
    # group invitations the assistant accepts. With no entry for an adapter,
    # every group add on it is refused and the assistant leaves the group.
    [operators]
    telegram = "<the operator's numeric Telegram user id>"

The `[endpoints]` table can override any of the five hosts the process talks to —
`telegram`, `openrouter`, `forge` (the commit lookup's canonical forge, default
`https://git.halogenos.org`), `mirror` (the release lookup's API host, default
`https://api.github.com`) and `wiki` (the wiki lookup's raw host, default
`https://raw.githubusercontent.com`); omitted entries keep the real hosts, and the
overrides exist for the test suites' loopback servers.

The three lookup tools answer community questions from the project's own sources: a
commit by repository and reference from the canonical forge, a release — the
latest, or one by tag — from the builds repository, and a wiki page by its name
from the project wiki's raw pages, with a five-minute response cache mirroring the
raw host's own cache header. Every conversation records a tool palette at creation
naming exactly the registered tools, and appends a superseding palette when the
registered set changes; a conversation without a palette admits none, and a call
outside the palette is declined with a recorded error the model reads.

The report tool files a spam report with the group's moderation bot when a member
replies to an offending message and asks: the fixed `/report@<handle>` line goes
out as a threaded reply to the reported message, before the assistant's answer, at
most once per group per report window. It registers only when `moderation_handle`
is configured, works in groups only, and takes no arguments — the member's reply
is what names the reported message. The platform-side setup it needs is recorded
in the group operator's reference document.

The protection table sets the two answering budgets: how many messages one sender is
answered per window (counted across every chat, direct and group alike) and how many
messages one chat is answered per window. The table and each of its fields may be
omitted — absent fields keep the defaults shown. A window of zero disables that
budget; an answer count of zero is refused at start, since disabling is the window's
job. Budgets limit answering only, never recording: an over-limit message is still
recorded with the refusing budget named on it, draws no reply and no notice — a
refusal notice would hand a flooder the assistant's voice — and cannot cancel an
answer someone else is still owed.

Group membership is the operator's call: the assistant stays only in groups the
configured operator added it to, the admission persists across restarts, and a group
add by anyone else — or any group contact with no operator entry for that adapter —
is refused, with the assistant leaving the group. The `/privacy` command is answered
deterministically, without a model turn: with `Privacy policy: ` plus the configured
`privacy_policy` address, or with the fixed not-yet-published line when the key is
absent — an empty value is refused at start. The answer goes out at most once per
chat per window; repeats within it are recorded in silence.

Direct chats are served by default and can be switched off with `direct_chats =
"off"`: a direct message is then refused before anything is written — no identity
row, no conversation, no answer, the `/privacy` command included — while groups are
served as ever. The two spelled values are the whole vocabulary; anything else
refuses the start.

The process logs its startup facts — never a secret — and stops cleanly on SIGTERM.
Deployment wiring, including the group-privacy platform setting the record-all
policy depends on, stays outside the repository.

## Development

Cargo runs inside the Nix development shell, which provides the toolchain:

    nix develop -c cargo test --workspace
    nix develop -c cargo clippy --workspace --all-targets -- -D warnings
    nix develop -c cargo fmt --check

## Layout

- `crates/core` — the platform-neutral core: conversation handling, the lookup tools
  with their palette and admission, rate and abuse protection.
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
