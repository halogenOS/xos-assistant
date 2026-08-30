# Unit 45 — /wipe and /compact, the session reset commands

Date: 2026-08-31. The operator's order, verbatim (2026-08-30, working copy — de-quoted
at landing): "I also need two more slash commands /wipe – creates a brand new session
/compact – compacts the current session", with "Direct chats arent allowed for these
bots at the moment" fencing the audience, and the earlier design for the runaway case:
"Also once the model hits 5 rate limit errors in consecutively, force a turn end, and
trigger a compaction. The session has likely corrupted beyond the point of recovery at
this point, and the model will only do garbage otherwise." The live motivation: a
production conversation still carries a thousand-call tool flood from an old incident,
and the model keeps continuing that pattern; the framework's forced turn-end (slice 16)
ships its `tool_calls_exhausted` status key for exactly this consumer hook.

## Grounding

**What a "session" is here.** A group maps to a conversation id through the mapping
table; conversation state derives from append-only blocks, and the model reads the
WHOLE ledger every turn (`store.list_blocks` at the framework's `actor.rs:1044`,
`blocks_to_messages` over the full vector at `:1145`; `LedgerSource::list` is "Every
block in this ledger, in ledger order", `ratchet.rs:39-42`). No window, floor, or
summary mechanism exists anywhere in the framework — greps for compact/summarize/
prune/window surface only slice 16's tool-call window, whose own launch notes assign
"the consumer's auto-compaction on the status key" to "the consumer's own unit (with
/compact)" (`docs/slices/16-the-tool-call-window.md:263-264`).

**The two id-changing precedents.** New conversation ids arrive two ways today:
`map_new_channel` (fresh conversation + composed prompt + palette + mapping claim,
`assembly.rs:1794-1828`) and `retire_stale_channels` (`assembly.rs:968-1064`), which
forks with FULL junction-inherited history, detaches the inherited `SystemPrompt`
blocks (`assembly.rs:1038-1045`), inserts the current prompt, and re-points the
mapping (`delete_by_conversation` + `claim`). The retire precedent's recorded promise
holds for both commands: nothing is rewritten and nothing is deleted — the old
conversation stays whole, readable, exportable, and reachable by erasure
(`assembly.rs:946-951`). `detach_block` removes a junction membership, never a block
(`store/conversations.rs:338-372`); a block the source conversation still references
is no orphan, so `gc_orphan_blocks` (consumer-invoked, only inside erasure today,
`erasure.rs:337`) cannot touch it.

**The command machinery.** Recognition is hand-matched three times today (the privacy
five, the mirror's `/del`, the reply-acted list) — a fourth list is the smell the
commands-menu design (`docs/units/telegram/15-commands-menu.md`, unbuilt: no
`crates/core/src/commands.rs` exists) was written to end: one `enum Command`,
`recognized(Option<&InvokedCommand>)` folding ASCII case, `offered(ChannelKind,
Authority) -> bool` as the floor, the stamp condition widening to "a recognised
command, or the mirror" (`assembly.rs:795-801` still says "family or mirror"),
`offered()` deciding the answer as well as the menu. The mirror is the floor
precedent: `ADMINISTRATOR_FLOOR = Authority::Moderator`, group-only, checked against
the delivered authority (`mirror.rs:41,58-75`); "the administrator IS the human
decision of decision 0070" (`mirror.rs:21-24`). Fixed answers are core copy —
"wording is behavior" (`message.rs:356-360`) — delivered as
`DeliveryItem::CommandAnswer` (`message.rs:362-368`, adapter send at
`driver.rs:571-581`); rights commands grant through a per-principal `ReplyWindow`
with the state change applied exactly with the granted reply (`window.rs:117-197`).

**The consumer hook slice 16 shipped.** `Status::TOOL_CALLS_EXHAUSTED`
(`records.rs:49`) lands anchored on a turn five consecutive rate-limit refusals
ended; the app's bus subscription already receives every `CoreEvent` including
`BlocksChanged` (`streams.rs:114`), and no app code reads the key today.

**Privacy.** Neither command erases personal data, stops collection, or adds a
recipient; retention is "kept until erasure is requested" and P2 already covers
reading older discussion. Hiding history from the MODEL changes projection, not
retention — no D-row moves, and the update triggers (a retention change, a new
off-machine path) do not fire. Deleting blocks WOULD be a retention change; this
unit deletes nothing.

## Decisions taken with this unit

- **The command catalogue arrives here, minimally, adopted from the commands-menu
  design, 2026-08-31.** `crates/core/src/commands.rs` lands with `enum Command`,
  `ALL`, `invocation()`, `offered(ChannelKind, Authority) -> bool` and
  `recognized()` folding ASCII case, carrying variants for the five privacy
  commands, `/wipe` and `/compact`; `privacy::family_command` becomes a projection
  of `recognized()` with every privacy pin standing except the exact-spelling
  negative, which becomes the folded-recognition assertion (mixed case recognized,
  foreign words refused — the design's own intent). The stamp condition at
  `assembly.rs:795-801` widens to "a recognised command, or the mirror". `summary()`
  and the menu publication stay with the commands-menu unit; a dated note at the top
  of its spec records what this unit built (the same note the parked limits-commands
  unit will extend — its spec adopted the identical design first but lands later,
  and re-anchors on the catalogue as built here). `/del` keeps its exact comparison
  in the mirror, outside the catalogue. *Rejected:* a fourth hand-matched list —
  the recorded smell the catalogue design exists to end; *rejected:* waiting for
  the limits unit to build the catalogue — it is parked and this unit is approved
  first.
- **Both commands are offered in groups at Moderator and above, and nowhere else,
  2026-08-31.** `offered(Group, Moderator)` true, `offered(Direct, _)` false — the
  operator fenced direct chats out. An invocation below the floor, or in a direct
  chat, is recognized, stamped `LimitedBy::Command`, and answers silence — the
  commands-menu rule: no debt, no model turn, no refusal line advertising a
  moderator surface. The floor is checked against the delivered authority the
  ingest path holds, the mirror's precedent. *Rejected:* an Admin-only floor — the
  mirror already trusts Moderator with message deletion, a heavier effect than a
  context reset.
- **/wipe retires the mapping and claims a brand-new conversation, 2026-08-31.**
  The group's mapping row is deleted and the first-contact path runs for the same
  channel: a fresh conversation, the composed prompt, the palette, the mapping
  claim — exactly what a newly admitted group gets, with no history inherited. The
  old conversation stays whole and erasure-reachable (the retire promise). The next
  member message speaks into an empty session; standing observations (title, pinned
  announcement) re-arrive through the adapter's lazy first-contact lookup, whose
  once-per-process memory is cleared for the channel so the fresh conversation is
  re-enriched. *Rejected:* the framework's `fork_continuation::NewThread` — it
  deep-copies the trailing user group into the fresh thread, and a wipe that
  carries the triggering text over is not "a brand new session"; *rejected:*
  deleting the old conversation — a retention change the privacy record forbids
  without revision, and the tree's only conversation deletions are erasure's.
- **/compact forks the conversation and detaches everything but the kept tail,
  2026-08-31.** The retire machinery runs for the group's conversation with one
  addition: after the fork inherits the full history and takes the fresh prompt,
  every inherited block is detached from the FORK except the kept tail — the
  trailing chat-message blocks (member and assistant text rows, their stamps and
  debts riding) up to `COMPACT_KEPT_MESSAGES = 20` of them, and every date marker
  among them. NO tool block survives a compact — no call, no result, no error:
  tool traffic is exactly the poison the command exists to cut, and the lawful
  record keeps it in the source conversation. Because no tool call crosses, the
  fork can never park on a dangling call; because detaching only removes junction
  rows from the fork, the source conversation keeps every block referenced and GC
  is unreachable. The cursor confirms on the fork's tail as the fork machinery
  already does; detached lower ids leave a later-id anchor valid
  (`ratchet.rs:342-351`). An owed debt inside the kept tail survives verbatim —
  the walk reads the same stamps; a debt older than the kept tail is consciously
  cut with the context that poisoned it, stated here so nobody calls it a burial:
  the operator ordered the session reset, and the old conversation still shows the
  unanswered message to any human reader. *Rejected:* a summary block — the
  framework has no summarization capability and inventing model-written summaries
  of member messages is new personal-data processing this unit refuses to smuggle
  in; *rejected:* per-position projection (a marker hiding what sits below it) —
  projection is per-kind with no position context (`render.rs:39-58`), and the
  verified burial defect is the standing warning against new read-through shapes;
  *rejected:* keeping tool blocks inside the tail — the kept tail must be
  poison-free by construction.
- **The auto-compact rides the same operation, keyed on the framework's own
  signal, 2026-08-31.** The app observes `BlocksChanged`, folds the conversation's
  status blocks, and when a `tool_calls_exhausted` status lands, runs the compact
  operation on that conversation — the operator's design for the corrupted-session
  case, and slice 16's launch note names this unit as its home. One operation, two
  triggers: the command and the signal. The auto-compact answers nothing in chat
  (no command was invoked); a warn-level log records it. *Rejected:* a second,
  different auto-compact shape — one decision, recorded once.
- **Both commands answer like rights commands, on their own window, 2026-08-31.**
  Per-principal `ReplyWindow` with `grant_with`, budget-exempt, its own constants
  equal to the privacy window's values (each bound carries its own constant — the
  tree's rule). The reset is applied exactly with the granted reply; a failed
  apply answers silence with the warn log, the rights precedent. The fixed lines,
  exact copy, stored as consts beside the catalogue and pinned byte for byte:
  - Wipe, applied: `Done. This group starts a fresh session; the old one stays on record.`
  - Compact, applied: `Done. This session was compacted: recent messages stay, old context is set aside.`
  - Compact, nothing to cut (the conversation already holds no more than the kept
    tail and no tool traffic): `This session is already compact. Nothing changed.`
  *Rejected:* sharing the privacy window instance — a flood of one family must not
  silence the other's rights commands.

## The unit's contract

A moderator or admin in a group resets that group's session with `/wipe` or trims it
with `/compact`; the model's next turn reads the fresh or trimmed conversation;
everyone else, and every direct chat, meets silence on both commands. The framework's
forced turn-end triggers the same compaction unattended. Nothing is deleted anywhere:
old conversations remain whole, readable, and erasure-reachable, and no privacy row
moves.

## Acceptance criteria

- **AC1 — one catalogue, one recognition.** `commands.rs` holds the seven variants;
  `recognized()` folds ASCII case (pin); `privacy::family_command` is a projection
  of it; the privacy pins stand except the exact-spelling negative, which becomes
  the folded-recognition assertion; `/del` is not in the catalogue (pin); the stamp
  condition covers any recognised command (pin: an unoffered `/wipe` from a member
  takes the stamp and opens no turn).
- **AC2 — /wipe resets.** A moderator's `/wipe` maps the channel to a new empty
  conversation (fresh prompt, palette, no inherited blocks), answers its exact
  line, and the old conversation remains whole and erasure-reachable (pins on the
  new mapping, the empty history, the old rows, and the line).
- **AC3 — /compact trims.** With a conversation holding tool traffic and more chat
  than the kept tail, a moderator's `/compact` maps the channel to a fork whose
  readable history is exactly the kept tail (no tool block, at most
  `COMPACT_KEPT_MESSAGES` chat rows plus their date markers, stamps and debts
  intact), answers its exact line; the source conversation keeps every block; a
  second `/compact` immediately after answers the nothing-to-cut line (pins).
- **AC4 — the floor and the fence.** Below-floor and direct-chat invocations of
  both commands are stamped silent; the Moderator floor reads the delivered
  authority (pins; the monotonicity check covers the new variants).
- **AC5 — the auto-compact.** When a `tool_calls_exhausted` status lands in a
  conversation, the app runs the same compact operation on it, logs at warn, and
  answers nothing in chat; the operation observably equals the command's (one
  test drives both paths to the same shape).
- **AC6 — nothing personal moves.** No block is deleted (pin over the source
  conversation after both commands); the privacy documents are untouched; the
  windows are budget-exempt and flood-bounded (pins).
- **AC7 — the checks.** fmt, clippy with warnings denied, the full suite, the doc
  build, exit codes read bare.

## Notes for launch

- Decision records number from the highest shipped at merge; expected: the
  catalogue's minimal landing, the wipe shape, the compact shape and its kept-tail
  bound, the auto-compact trigger.
- The commands-menu unit's spec gains the dated note; the parked limits-commands
  spec re-anchors when that unit is next touched.
- The framework is consumed as it is: no framework change rides this unit.
