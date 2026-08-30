# Unit 45 — /wipe and /compact, the session reset commands

Date: 2026-08-30. The operator ordered two more slash commands — /wipe creates a brand
new session, /compact compacts the current one — with direct chats fenced out of both,
and the runaway design decided earlier: five consecutive rate-limit refusals force a
turn end and trigger a compaction, because a session that far gone has likely corrupted
beyond recovery and the model will only continue the garbage. The motivation: a
conversation can come to carry a thousand-call tool flood from one incident, and a
model reading it keeps continuing the pattern; the framework's forced turn-end
(slice 16) ships its `tool_calls_exhausted` status key for exactly this consumer hook.

## Grounding

**What a "session" is here.** A group maps to a conversation id through the mapping
table; conversation state derives from append-only blocks, and the model reads the
WHOLE ledger every turn (`store.list_blocks` at the framework's `actor.rs:1044`,
`blocks_to_messages` over the full vector at `:1145`; `LedgerSource::list` is "Every
block in this ledger, in ledger order", `ratchet.rs:39-42`). No read-window, floor, or
summary mechanism over the model's ledger read exists anywhere in the framework
(the words compact/window elsewhere name unrelated bounds), and slice 16's own launch notes assign
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
ended; the bus is a global broadcast any subscriber taps (the observer at
`streams.rs:114` is the pattern; its match ignores `BlocksChanged`), so the
auto-compact watcher is NEW WORK in that exact shape — a spawned task beside
the observer — and no app code reads the key today.

**Privacy.** Neither command erases personal data, stops collection, or adds a
recipient; retention is "kept until erasure is requested" and P2 already covers
reading older discussion. Hiding history from the MODEL changes projection, not
retention — no D-row moves, and the update triggers (a retention change, a new
off-machine path) do not fire. Deleting blocks WOULD be a retention change; this
unit deletes nothing. (The one conversation deletion outside erasure is
`map_new_channel`'s race-loser — a just-created empty conversation the mapping
claim deletes before anything references it, `assembly.rs:1812-1823`; no
established history is ever deleted by anything but erasure.)

## Decisions taken with this unit

- **The command catalogue arrives here, minimally, adopted from the commands-menu
  design, 2026-08-30.** `crates/core/src/commands.rs` lands with `enum Command`,
  `ALL`, `invocation()`, `offered(ChannelKind, Authority) -> bool` and
  `recognized()` folding ASCII case, carrying variants for the five privacy
  commands, `/wipe` and `/compact`; `privacy::family_command` becomes a projection
  of `recognized()` with every privacy pin standing except the exact-spelling
  negative, which becomes the folded-recognition assertion (mixed case recognized,
  foreign words refused — the design's own intent). The stamp condition at
  `assembly.rs:795-801` widens to "a recognised command, or the mirror". `summary()`
  and the menu publication stay with the commands-menu unit; a dated note at the top
  of its spec records what this unit built. `/del` keeps its exact comparison
  in the mirror, outside the catalogue. *Rejected:* a fourth hand-matched list —
  the recorded smell the catalogue design exists to end; *rejected:* waiting for any other
  catalogue adopter — this unit is approved and lands first.
- **Both commands are offered in groups at Moderator and above, and nowhere else,
  2026-08-30.** `offered(Group, Moderator)` true, `offered(Direct, _)` false — the
  operator fenced direct chats out. The five privacy variants' rows are
  stated, not derived: offered to Member and above in BOTH channel kinds,
  today's behavior exactly — the direct-chat fence is these two commands' own,
  never catalogue-wide. An invocation below the floor, or in a direct
  chat, is recognized, stamped `LimitedBy::Command`, and answers silence — the
  commands-menu rule: no debt, no model turn, no refusal line advertising a
  moderator surface. The floor is checked against the delivered authority the
  ingest path holds, the mirror's precedent. *Rejected:* an Admin-only floor — the
  mirror already trusts Moderator with message deletion, a heavier effect than a
  context reset.
- **/wipe retires the mapping and claims a brand-new conversation, 2026-08-30.**
  The group's mapping row is deleted and the first-contact path runs for the same
  channel: a fresh conversation, the composed prompt, the palette, the mapping
  claim — exactly what a newly admitted group gets, with no history inherited. The
  old conversation stays whole and erasure-reachable (the retire promise). The next
  member message speaks into an empty session. Standing observations (title,
  pinned announcement) re-arrive because the command's outcome carries a
  CHANNEL-RESET directive beside its answer — the withdraw directive's exact
  precedent: the core decides, the adapter mechanically translates by forgetting
  its once-per-process lookup memory for the channel — a named slot on
  `IngestOutcome::Recorded` beside `deliver`, the adapter's new arm voiding
  the memory exactly as the admission path already voids it (`driver.rs:671`;
  the `:685-690` site is the skip CHECK, not the forget), so
  the channel's next contact re-runs the first-contact lookup and re-enriches
  the fresh conversation. No adapter decision is added, only a translation.
  Every debt the old conversation still owed is consciously cut with it — the
  operator ordered the reset, and the old conversation still shows any
  unanswered message to a human reader; stated here so nobody calls it a
  burial. An answer or outbound item mid-flight at the swap resolves its
  channel from the mapping at delivery time and is dropped — TODAY silently
  (`outbound.rs:336-341` and `:438-443` return Ok with no trace; the
  driver's logged case is a different branch), so this unit ADDS the
  warn-level log at exactly those two unmapped branches, and the drop is
  accepted openly: a session
  the operator is resetting owes its in-flight products to the record, not to
  the chat. The runtime may still spend one turn on a retired debt (nothing
  marks the source retired to the scheduler); its answer lands in the dead
  conversation and drops at the same logged branch — the retire machinery's
  recorded leftover class, accepted. *Rejected:* the framework's `fork_continuation::NewThread` — it
  deep-copies the trailing user group into the fresh thread, and a wipe that
  carries the triggering text over is not "a brand new session"; *rejected:*
  deleting the old conversation — a retention change the privacy record forbids
  without revision; the only deletion of an ESTABLISHED conversation is
  erasure's (the mapping claim's just-created race-loser is the recorded
  exception).
- **/compact forks the conversation and detaches everything but the kept tail,
  2026-08-30.** The retire machinery runs for the group's conversation with one
  addition: after the fork inherits the full history and takes the fresh prompt,
  every block of the PRE-FORK snapshot (the retire precedent's enumeration
  basis — the `list_blocks` read at `assembly.rs:973` iterated by the detach loop at `:1038-1043`, never the post-insert list, so the fresh
  prompt is structurally exempt) is detached from the FORK except the kept set:
  the trailing chat-message blocks (member and assistant text rows, their stamps
  and debts riding) up to `COMPACT_KEPT_MESSAGES = 20` of them — the constant
  lives in `commands.rs` beside the catalogue and its copy, one home for both
  the sweep and the nothing-to-cut check — every date marker among them PLUS the
  marker immediately preceding the oldest kept row (the kept rows keep their own
  day), the inherited `ToolPalette` block — KEPT (configuration, not
  traffic; without it a fork whose first wake is a framework drive would run
  tool-less; when several palette blocks exist, the newest alone) — and the
  NEWEST context-note block per observed fact (title, pinned announcement:
  the group knowledge the model keeps with no re-enrichment round trip).
  Report blocks are NOT kept: their deliveredness lives only in the outbound
  edge's process memory, so the core cannot tell a delivered report from a
  pending one — keeping all would re-deliver, which the edge's contract
  refuses above all — and the pending-report loss is accepted under the
  tree's own recorded process-death precedent, with eyes open: the exhausted
  turn the auto-compact follows was looping, and its report is the least
  trustworthy product it made. Quote blocks COUNT as chat rows, for the kept
  tail and the nothing-to-cut bound alike, and the invoking command row
  counts INSIDE the kept bound (it is the newest chat row; only the
  nothing-to-cut check reads past it). Join-notice and Delivered blocks
  ride the cut side, ruled here: a reset conversation owes no memory of old
  joins — the lawful record keeps them — and delivery records project
  nothing. TOOL TRAFFIC is defined
  exactly: `ToolCall`, `ToolResult` and `ToolError` blocks. None survives a
  compact — no call, no result, no error:
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
  unanswered message to any human reader. In-flight answers at the
  swap drop exactly as /wipe's do, accepted the same way. /compact needs NO channel-reset directive on either
  trigger: the kept context notes carry the group knowledge across, so the
  signal path — which has no command outcome to ride a directive on — needs
  no core-to-adapter transport at all; the reset directive is /wipe's alone.
  A conversation predating the first-contact notes carries none to keep —
  its compacted fork stays note-less until the platform fact next changes
  or the process restarts, accepted as self-limiting. The OUTBOUND EDGE's seam is decided here, not left to the builder:
  the edge seeds a conversation it has never seen at zero on the premise
  that all its blocks postdate the edge (`outbound.rs`'s `seed_cursors` and
  the vacant-cursor insert in `deliver_stored_items`) — false for a fork,
  whose kept assistant
  answers would re-send into the group and whose first-delivery disclosure
  resolution would write into junction-SHARED blocks (the disclosure
  resolution inside the reply arm; the edit-through-a-fork `detach_block`'s
  own doc forbids, `store/conversations.rs:347-351`). The repair makes the
  premise true: an unseen conversation seeds at its INHERITED BOUNDARY —
  the newest of its blocks that another conversation also holds, or zero
  when it holds none. A junction row is what makes a block part of a
  conversation, so a block two conversations hold is one this conversation
  was forked with, and since ids ascend along junction order the partition
  is exact: inherited history is born delivered, and /wipe's fresh
  conversation, which shares nothing with anybody, is untouched. The
  framework's durable ratchet cursor is deliberately NOT the seed, though
  at the instant of the fork it holds exactly this value
  (`confirm_inherited_history`, the min of the inherited boundary and the
  source's confirmed position): it is the frontier of what the model has
  been driven through, it advances with every turn, and by the time a
  completed stream wakes this edge it stands past the very answer the wake
  is about — reading it there would swallow that answer. The sweep's cost is one store round trip: the
  framework's bulk door (`Store::detach_blocks`, slice 19) detaches the whole
  set in one transaction, so the motivating thousand-row flood costs one
  commit under the stamp lock, not a row-by-row pause.
  *Rejected:* a summary block — the
  framework has no summarization capability and inventing model-written summaries
  of member messages is new personal-data processing this unit refuses to smuggle
  in; *rejected:* per-position projection (a marker hiding what sits below it) —
  projection is per-kind with no position context (`render.rs:39-58`), and the
  verified burial defect is the standing warning against new read-through shapes;
  *rejected:* keeping tool blocks inside the tail — the kept tail must be
  poison-free by construction.
- **The auto-compact rides the same operation, keyed on the framework's own
  signal, level-triggered and self-consuming, 2026-08-30.** On any
  `BlocksChanged` for a conversation, the app folds that conversation's status
  blocks; when the fold finds a `tool_calls_exhausted` status AND the
  conversation is currently MAPPED to a channel, the compact operation runs on
  it — the operator's design for the corrupted-session case, and slice 16's
  launch note names this unit as its home. The trigger is level-read from the
  durable fold, so a lagged or dropped bus event self-heals on the next wake
  (the bus is deliberately lossy, `bus.rs:396-402`); it is self-consuming
  because the exhausted marker is never in the kept set — the FORK carries no
  marker, so the fresh conversation cannot re-fire — and the mapped-only guard
  makes the swept SOURCE (now unmapped) ineligible however many late appends
  wake its fold. An unmapped conversation is never auto-compacted. The
  re-claim takes the WINNER CHECK `map_new_channel` already owns
  (`assembly.rs:1820-1823`): a claim lost to a concurrent racer deletes the
  just-created fork — junction rows alone, every block lives on in the
  source — logs at warn, and the winner's state governs; no mapped-nowhere
  phantom fork ever owes turns nothing can deliver. A claim-lost COMMAND
  still answers its done line — true of the surviving state, whichever
  racer produced it. The
  whole operation — check and act, both triggers — runs under
  ONE hold: the global stamp lock the ingest path already holds plus the
  erasure fence SHARED (`assembly.rs:376-386`; `:393-401` names the
  interleaving class the fence exists for), and the marker-present AND
  mapped checks are RE-READ inside that hold, so a wake that lost the race
  finds the source unmapped and stands down; an in-process in-flight set
  (one entry per conversation) keeps concurrent wakes from stacking behind
  the lock only to re-fork in sequence. The straddler is stated: an
  ingestion that resolved the old conversation before the lock and appends
  after the swap lands its rows in the RETIRED conversation — its answer
  drops at the unmapped branch with this unit's log, the retire machinery's
  own leftover class, accepted. One
  operation, two triggers: the command and the signal. The auto-compact
  answers nothing in chat (no command was invoked); a warn-level log records
  it — implemented but unpinned, the commands-menu precedent for log
  assertions. *Rejected:* a second, different auto-compact shape — one
  decision, recorded once; *rejected:* edge-triggering on the bus event alone —
  the lossy channel would drop exactly the incident it exists for.
- **Both commands answer like rights commands, on their own window, 2026-08-30.**
  ONE per-principal `ReplyWindow` instance shared by the family — /wipe and
  /compact together, the privacy family's one-window-per-family precedent
  (`assembly.rs:362`) — with `grant_with`, budget-exempt, its own constants
  equal to the privacy window's values (each bound carries its own constant — the
  tree's rule; the principal key never moves at the swap, `window.rs:117-190`). The reset is applied exactly with the granted reply; a failed
  apply answers silence with the warn log, the rights precedent — and the
  silence never claims atomicity: the swap is separate store calls, so a
  failure or crash midway can leave a half-swept, unmapped orphan fork
  (harmless, never cleaned) or an unmapped channel that the adapter's
  unacknowledged-update redelivery converges on the next attempt. The fixed lines,
  exact copy, stored as consts beside the catalogue and pinned byte for byte:
  - Wipe, applied: `Done. This group starts a fresh session; the old one stays on record.`
  - Compact, applied: `Done. This session was compacted: recent messages stay, old context is set aside.`
  - Compact, nothing to cut (the conversation already holds no tool-traffic
    blocks — the exact three kinds — and no more chat rows than the kept
    bound, counting only rows OLDER than the invoking command row; the
    signal path, having no command row, counts the whole readable set):
    `This session is already compact. Nothing changed.`
  *Rejected:* sharing the privacy window instance — a flood of one family must not
  silence the other's rights commands.

## The unit's contract

A moderator or admin in a group resets that group's session with `/wipe` or trims it
with `/compact`; the model's next turn reads the fresh or trimmed conversation;
everyone else, and every direct chat, meets silence on both commands. The framework's
forced turn-end triggers the same compaction unattended. Nothing established is deleted anywhere:
old conversations remain whole, readable, and erasure-reachable (the mapping
claim's just-created race-loser is the recorded exception, as today), no privacy
row moves, and an answer in flight at the moment of a reset is dropped with
the warn log this unit adds at the edge's unmapped branches — the reset is
the point, stated openly. A moderation report pending delivery at a compact
is lost under the tree's own recorded process-death precedent, accepted.

## Acceptance criteria

- **AC1 — one catalogue, one recognition.** `commands.rs` holds the seven variants;
  `recognized()` folds ASCII case (pin); `privacy::family_command` is a projection
  of it; the privacy pins stand except the exact-spelling negative, which becomes
  the folded-recognition assertion; `/del` is not in the catalogue (pin); the stamp
  condition covers any recognised command (pin: an unoffered `/wipe` from a member
  takes the stamp and opens no turn).
- **AC2 — /wipe resets.** A moderator's `/wipe` maps the channel to a new empty
  conversation (fresh prompt, palette, no inherited blocks), answers its exact
  line, the old conversation remains whole and erasure-reachable, and the
  channel's next contact re-runs the first-contact lookup (pins on the new
  mapping, the empty history, the old rows, the line, and the re-enrichment
  through the reset directive).
- **AC3 — /compact trims.** With a conversation holding tool traffic and more chat
  than the kept tail, a moderator's `/compact` maps the channel to a fork whose
  readable history is exactly the kept set (no tool-traffic block, at most
  `COMPACT_KEPT_MESSAGES` chat rows — quotes counted — plus their date
  markers including the oldest kept row's own, the palette block, the newest
  context notes, stamps and debts intact), answers its exact line; the source conversation keeps every block; a
  second `/compact` immediately after answers the nothing-to-cut line (pins).
- **AC3b — the fork is born delivered.** The outbound edge seeds an unseen
  conversation at its inherited boundary: after a /compact, no kept
  assistant answer re-sends and no disclosure line is written into any
  junction-shared block (pins); a conversation that inherited nothing still
  seeds at zero (pin: /wipe's conversation delivers its first answer
  normally).
- **AC4 — the floor and the fence.** Below-floor and direct-chat invocations of
  both commands are stamped silent; the Moderator floor reads the delivered
  authority; a direct-chat `/privacy` still answers (the fence is not
  catalogue-wide) (pins; the monotonicity pin is NEW WORK this unit creates over
  `Command::offered` — a higher standing is offered a superset, over all
  variants and both channel kinds).
- **AC5 — the auto-compact.** When a `tool_calls_exhausted` status lands in a
  MAPPED conversation, the app runs the same compact operation on it and
  answers nothing in chat; the fork carries no marker and the unmapped source
  never re-fires (pins); the operation observably equals the command's (one
  test drives both paths to the same shape); the warn log is implemented but
  unpinned, the commands-menu precedent.
- **AC6 — nothing personal moves.** No block is deleted (pin over the source
  conversation after both commands); the privacy documents are untouched; the
  windows are budget-exempt and flood-bounded (pins).
- **AC7 — the checks.** fmt, clippy with warnings denied, the full suite, the doc
  build, exit codes read bare.

## Notes for launch

- Decision records number from the highest shipped at merge; expected: the
  catalogue's minimal landing, the wipe shape, the compact shape and its kept-tail
  bound, the auto-compact trigger.
- The commands-menu unit's spec gains the dated note recording what this unit
  built of its design.
- The operator-facing documents move with the unit, the repo's practice for a
  command surface: the group-operator contract gains the two commands'
  moderator sentences and the README's command mention stays true — reviewers
  confirm no published sentence contradicts the unit.
- The framework is consumed as it is: no framework change rides this unit.
