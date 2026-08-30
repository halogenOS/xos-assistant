# halogenOS Group Assistant

A community assistant for [halogenOS](https://halogenos.org), the performance-focused,
open-source Android distribution. It answers community questions and helps with recurring
group tasks across the project's chat platforms, Telegram first, through a shared
platform-neutral core and one thin adapter per platform.

**Status: pre-alpha.** The core spine stands: the platform-neutral core consumes the
ledger framework, records inbound messages as ledger blocks, takes turns against a
registered provider, and yields replies on a subscription edge. The Telegram adapter
speaks the Bot API directly — updates in, plain sends out — with its update offset
persisted in a state file the embedder names (decisions 0013–0019). Updates arrive by
one of two answering modes, chosen by one predicate: a deployment with a public HTTPS
address configures `[webhook]` and Telegram pushes each update to a loopback listener
whose response code is the acknowledgement; a deployment without one long-polls exactly
as before, first clearing any registered webhook. The live model is
in: the assistant answers through the chat-completions provider with its key held in memory and
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

The core depends on the agent-ledger framework, published as ronna-core at
`https://github.com/xdevs23/ronna-core`. The manifest names a relative path and not that
address (decision 0004, revisited by decision 0134): the framework repository is expected
as a checkout named `agent-ledger` beside this repository's own directory. Clone the two
side by side before building.

## Running

The `assistant` binary takes exactly one argument, the path of a TOML configuration
file:

    nix develop -c cargo run -p assistant -- assistant.toml

The file names the store path, the Telegram state-file path, the prompt directory
(the repository's `prompts` directory holds the assistant's system prompt), the log
destination (the bare word `stderr` for the console, or a file path — as a bare
string, or as `log = { file = "..." }` for a file whose name collides with the
console word — the console word matches exactly and lowercase), the model id, and
optional endpoint overrides for tests. Secrets are
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

    # Optional: whether direct chats are served — "on" (the default) or
    # "off". Off, a direct message is refused before anything is written:
    # nothing is recorded, nothing is answered. Groups are unaffected.
    #direct_chats = "off"

    # Optional: the reasoning effort every conversation is created under —
    # one of "off", "auto", "minimal", "low" (the default), "medium",
    # "high", "xhigh", "max". Any other value refuses the start.
    #reasoning = "medium"

    # Optional: how group messages summon a turn — "helpful" (the default:
    # every group message reaches the model, which decides whether to
    # speak and abstains silently otherwise) or "addressed" (only a
    # mention, a reply to the assistant, or its name summons a turn).
    #answering = "addressed"

    # Optional: the assistant's name. Absent, the display name read from
    # the platform at startup is used. The name feeds the prompt identity,
    # the default disclosure line, and the group wake word.
    #name = "Xenia"

    # Optional: the first-interaction disclosure line, sent ahead of the
    # first answer to each person. Absent, a line naming the assistant an
    # AI system is composed from the name — the line is never absent.
    #disclosure = "Hi, I'm Xenia, an AI system."

    [secrets.bot_token]
    env = "ASSISTANT_BOT_TOKEN"

    [secrets.chat_completions_api_key]
    file = "chat-completions.key"

    # Optional: a mirror API token for the release lookup. Absent, the
    # lookup runs unauthenticated at the mirror's lower rate limit.
    #[secrets.mirror_token]
    #env = "ASSISTANT_MIRROR_TOKEN"

    # Optional: the web search vendor's API key. It is the search tool's
    # whole switch: absent, the tool is not registered and the assistant is
    # never taught to search. A configured key that cannot be read refuses
    # the start.
    #[secrets.search_api_key]
    #env = "ASSISTANT_SEARCH_API_KEY"

    # Optional: which locale the web search asks the vendor for. The
    # language defaults to "en"; the country is sent only when set.
    #[search]
    #country = "de"
    #language = "de"

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

    # Optional: how updates arrive. With this section the assistant registers
    # the public address with Telegram and serves the deliveries on the
    # loopback port behind the deployment's own HTTPS reverse proxy; without
    # it the assistant long-polls. Both fields are required together — a
    # section naming only one refuses the start — and no secret belongs here:
    # the adapter generates its own, keeps it beside the state file readable
    # by its owner alone, and no human ever handles it.
    #[webhook]
    #public_url = "https://assistant.example.org/telegram/webhook"
    #listen_port = 8085

The `[endpoints]` table carries one key per host the process talks to, and can override
each of them: `telegram`, `chat_completions` (the OpenAI-compatible model endpoint),
`forge` (the commit lookup's canonical forge, default
`https://git.halogenos.org`), `mirror` (the release lookup's API host, default
`https://api.github.com`), `wiki` (the wiki lookup's raw host, default
`https://raw.githubusercontent.com`), `wiki_index` (the host the wiki lookup reads its
rendered page index from, default `https://github.com`) and `search` (the web search
vendor, default `https://google.serper.dev`); omitted entries keep the real hosts, and the
overrides exist for the test suites' loopback servers.

The three lookup tools answer community questions from the project's own sources: a
commit by repository and reference from the canonical forge, a release — the
latest, or one by tag — from the builds repository, and a wiki page by its name
from the project wiki's raw pages, with a five-minute response cache mirroring the
raw host's own cache header. Every conversation records a tool palette at creation
naming exactly the registered tools, and appends a superseding palette when the
registered set changes; a conversation without a palette admits none, and a call
outside the palette is declined with a recorded error the model reads.

The web search answers questions the project's own sources cannot: it returns a page
of ranked results — each one's title, link, snippet where the result has one, and a
hint about the kind of host it sits on — and opens nothing. It registers only when
`secrets.search_api_key` is configured; without a key the tool is absent from every
palette and the prompt teaches no search at all. The query is sent exactly as
written, never corrected and never truncated: a query over 400 characters is refused
whole with the limit named, pages run from 1 to 5, and a query carrying a person
reference — an at sign followed by a name — is refused before anything is sent.
Each person draws at most five searches per ten minutes, since every call is a paid
request, and the same query within the window is answered from memory. Facts about
halogenOS itself still come only from the project lookups.

The report tool files a spam report with the group's moderation bot on the
assistant's own moderation read: when the group's pinned rules are clearly and
unmistakably violated — never on a borderline call — the fixed
`/report@<handle>` line goes out as a threaded reply to the violating message,
at most once per group per report window. It registers only when `moderation_handle`
is configured, works in groups only, and takes no arguments — the member's reply
is what names the reported message. The platform-side setup it needs is recorded
in the group operator's reference document.

The react tool puts one emoji reaction on a message where a reply would add nothing
— the off-topic chatter the silence default already keeps the assistant quiet about.
The model names the message by its shown id and picks the emoji; the core records
that choice verbatim, bounded at 32 bytes, and holds no emoji list of its own, while
the adapter maps the pick onto its platform's own reaction set and drops a pick
outside it. The tool registers everywhere — a reaction needs nothing but a chat. A
message takes at most one reaction for as long as that reaction is recorded, and an
erasure that empties the record leaves no shadow, so a later turn may react to that
message afresh; the rule that words and a reaction never land together is taught to
the model rather than enforced, since the answer is written after the tool returns.
The assistant reads nobody else's reactions: the platform delivers those only to a
chat administrator, which the assistant deliberately is not.

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

Privacy self-service is built in (decisions 0071–0076): `/privacyout` stops the
sender's messages from being collected or answered on that platform — from then on
their inbound messages are dropped at ingestion before any write — and
`/unblockprivacy` turns collection back on; `/privacydelete` answers a confirm
instruction and `/confirmdelete` within five minutes runs the erasure, asked and
confirmed in any chat because the pending state is keyed by the person. An
opted-out person's identity row survives as an emptied stub carrying the flag, so
the objection is never forgotten, and the deterministic replies are bounded per
person — never silenced by the answering budgets. A plain-language ask reaches the
same mechanisms through the privacy tool, which acts only on the one person who
asked and otherwise points at the commands.

Direct chats are served by default and can be switched off with `direct_chats =
"off"`: a direct message is then refused before anything is written — no identity
row, no conversation, no answer, the `/privacy` command included — while groups are
served as ever. The two spelled values are the whole vocabulary; anything else
refuses the start.

The `reasoning` key sets the reasoning effort every conversation is created under,
one of the framework's eight levels (`off`, `auto`, `minimal`, `low`, `medium`,
`high`, `xhigh`, `max`); an unknown value refuses the start, and the absent key
means `low` — moderation assessments ride on some thinking, while no set level at
all lets the model think unboundedly. The level is stored on each conversation at
its creation, so a changed key reaches new conversations only; conversations from
before the key existed keep deferring to the provider's default.

Answering is a mode (decisions 0087 and 0088): under the default `helpful` every
group message summons a model turn and the model decides whether to speak — it
answers when it can genuinely help and otherwise emits a fixed abstention
sentinel, which the process swallows: nothing reaches the chat, the turn is
closed, and no answer-window slot is spent, since the window bounds what the
assistant says. A rate-limited member's message opens no turn at all, so a flood
still costs nothing. Under `answering = "addressed"` only a mention, a reply to
the assistant, its name, or a direct chat summons a turn. A message from a
BOT account is narrower still, in every mode: only an explicit mention
summons the assistant — a bot's reply or name-drop never does, an
unmentioned bot message owes no answer, and nothing ever waits behind one
(decisions 0151-0154). The name (decision
0089) defaults to the platform display name read once at startup, feeds the
prompt's identity and the disclosure line's default, and — as one whole word,
case-insensitively — wakes the assistant in groups; a name that cannot form a
clean trigger word falls back to mention-and-reply, logged. The disclosure line
(decision 0090) can be overridden whole with `disclosure`; unset composes it
from the name and it is never absent.

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
  and the assembled core with the chat-completions provider and the Telegram adapter.
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

Copyright (C) 2026 Simão Gomes Viana

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, version 3. See [LICENSE](LICENSE).

The storage and orchestration subsystems are adopted from
[ronna-lightspeed](https://github.com/xdevs23/ronna-lightspeed) by the same
author, which carries the GNU General Public License v3.0.
