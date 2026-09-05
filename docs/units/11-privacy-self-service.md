# Unit 11 — privacy self-service

Date: 2026-08-23. Revision 2, rewritten after the cold probe returned eighteen
findings including two critical ones: revision 1 ran the erasure inside the
ingestion path that holds the erasure fence — a self-deadlock, not a slow path —
and borrowed the report tool's target resolution, which resolves someone else's
message and, under absorption, someone else's person. Revision 2 states the
execution model, its own resolution rule, the full fixed copy, and every
ordering the probe found unstated. Status: settled for implementation.

The operator's design: a person can opt out of collection, delete their data,
and opt back in — through explicit commands handled programmatically, and
through plain language the model reads and honors by calling a tool the system
enforces. The last prerequisite before the assistant joins its first group.
While the direct-chat switch is off, these commands are reachable in the
groups (and the enquiries address remains); stated plainly here and in the
policy sentence this unit adds.

## Decisions taken with this unit

- **Opt-out is a suppression stub, the one lawful remnant, 2026-08-23.** The
  identity row survives as a stub carrying the opt-out flag — the
  suppression-list shape, lawful because storing the identifier is what
  honoring the objection takes. The flag is a boolean column on the identity
  table by appended migration step (INTEGER NOT NULL DEFAULT 0; a boolean, no
  frozen vocabulary — the schema's own precedent), adapter-scoped like the
  identity it hangs on: opting out on one platform is opting out there, and
  the fixed copy says "on this platform". Erasure leaves the flag standing.
  From the moment it stands, the person's inbound messages are DROPPED at
  ingestion — the full no-write claim: no message row, no identity refresh, no
  principal write, no conversation creation, no palette append, no mapping —
  and the outcome reuses the disregarded variant, whose doc widens to "refused
  without effect at the person's own ask or the operator's switch"; the
  adapter acknowledges and the offset advances, exactly as for the direct-chat
  gate. Opt-out is not deletion and does not reach backward: what was stored
  before stands until deletion, KEEPS BEING PROJECTED to the model with later
  turns, and a pre-flag unanswered question may still draw its one answer —
  all three stated in the copy's plain reach line, none hidden. Rejected:
  dropping the identity row (collection would silently resume); suppressing
  the stored history's projection (a second projection gate for content
  deletion already removes on request); cancelling pre-flag debts (a turn
  mid-flight cannot be recalled anyway; the flag stops collection, deletion
  stops history).
- **Ordering, stated whole, 2026-08-23.** Ingestion decides in this order:
  channel-kind mismatch; group authorization; the direct-chat gate; then a
  READ-ONLY identity lookup by adapter and external id — a new lookup that
  writes nothing, added beside the resolving one — which yields the standing
  flag if any; then the command family (exempt from suppression, see below);
  then, flag standing, the drop. Only past all of that does the writing
  resolution run, the stamp lock get taken, the palette reconcile, the
  channel map. The suppression check therefore precedes every write the
  ingestion path can make, and the AC pins the no-write claim table by table.
- **The privacy command family is exempt from suppression, and identity stays
  frozen, 2026-08-23.** An opted-out person's `/unblockprivacy` must work, or
  the door never reopens from inside; their `/privacy` keeps answering. The
  exemption covers exactly the deterministic command family. An exempted
  command message is recorded (the request itself is the lawful processing of
  honoring it) with the command stamp, but through the READ-ONLY identity
  path: the display fields are not refreshed — the freeze the stub promises
  holds even across the person's own commands, and after a deletion no
  command re-materializes the emptied fields. Rejected: full suppression (a
  one-way door); refreshing on exempt commands (the freeze would leak).
- **Deletion confirms programmatically and runs outside the fence,
  2026-08-23.** `/privacydelete` answers the confirm instruction and files
  the pending confirmation, keyed BY PRINCIPAL — a deletion asked in one chat
  confirms in any, since the person is the subject, not the room. The memory
  is process-held with a named window constant (`CONFIRM_WINDOW`, five
  minutes), a named cap swept like every peer structure, forgotten on
  restart — deletion is the flow where forgetting errs safe. `/confirmdelete`
  inside the window consumes the pending state, answers the fixed started
  line, and SPAWNS the erasure as its own task after ingestion returns — the
  ingestion path holds the erasure fence for reading, the erasure takes it
  for writing, and running it inline is the deadlock revision 1 shipped. The
  started line promises what the mechanism delivers: the deletion is
  underway, not instantaneously done; a failure is logged and leaves the data
  standing, and re-asking works — the copy never claims completion the spawn
  cannot see. With nothing pending, or past the window, the fixed
  nothing-pending line (one line covers both — a lapsed pending IS nothing
  pending). A second confirm after a completed run answers the same line.
  The receipt a confirm returns names no erased rows. Rejected: erasure
  inline (self-deadlock); a completion callback into the chat (a new
  outbound path for one line, and the deterministic return already answered);
  a durable pending store (a restart re-arming a half-asked deletion).
- **Erasure keeps the stub when the flag stands, and the record says so,
  2026-08-23.** The erasure operation gains one conditional: an identity row
  whose opt-out flag stands is EMPTIED — display fields to the empty string,
  the schema's non-null contract kept — instead of deleted, so the flag
  survives its own person's deletion. The operation's documented idempotency
  changes with it, recorded as a dated refinement on the erasure decision:
  for a flagged person, a repeat erasure re-runs over emptiness and reports
  completion rather than not-found — honest, harmless, stated. For an
  unflagged person nothing changes. Rejected: the flag in its own table
  keyed by external id (a second identity surface to keep consistent, when
  the row already exists precisely because the stub must).
- **Plain language reaches the same mechanisms through one tool, with an
  unambiguous subject, 2026-08-23.** The privacy tool — member authority,
  palette-governed, actions `opt_out` and `request_deletion`, any other or
  absent action answered with the fixed invalid-action result — acts on the
  turn's origin set resolved to PRINCIPALS: the own-debt-takers of the debt
  origin walk, mapped to their stored principal ids. Exactly one distinct
  principal in the set: the tool acts on it. Several — the absorbed
  co-summoner shape — or none, or an erased row without a principal: the
  tool DECLINES with the fixed ambiguity result naming the commands, because
  acting on a guessed person is the one failure this design must never have;
  the commands are always unambiguous. `opt_out` writes the flag through a
  guarded consumer surface holding the erasure fence for reading; the
  no-write rule's amendment gains its dated second clause — a tool may write
  the consumer's own identity-table fact when the write IS the honored
  right. `request_deletion` files the same principal-keyed pending state and
  returns the fixed result carrying the literal confirm token for the model
  to relay; the prompt orders the relay verbatim, the pinned fact is the
  token in the tool result, and a model garbling it costs one retry via the
  command path — stated residual. The tool's writes failing return the
  transient result in the report tool's established wording style.
  Rejected: acting on "the newest" of several co-summoners (the wrong
  person, structurally); a target parameter (forgeability).
- **The rights replies are bounded per person, and never budget-silenced,
  2026-08-23.** The shared channel-keyed reply window is a courtesy bound;
  a rights mechanism cannot be starved by a neighbor's `/privacy`. The four
  new commands and the tool's deterministic replies are bounded by their own
  window keyed by PRINCIPAL (`PRIVACY_REPLY_WINDOW`, the same length), so
  one person's flood bounds that person alone. The answer-budget check does
  not gate the family at all: a rights request is answered even from a
  sender the flood budgets have silenced — the per-person window is the
  bound, and the state change (flag, pending, confirm) applies exactly when
  its reply is granted, never silently. `/privacy` keeps its existing
  channel-keyed bound; it is a notice pointer, not a state change.
  Rejected: state-change-on-recorded-silence (a destructive action with no
  receipt); the shared channel window (cross-person starvation of a right).
- **The command tokens and every fixed line, 2026-08-23.** Tokens:
  `/privacyout`, `/privacydelete`, `/confirmdelete`, `/unblockprivacy` (the
  operator's name). The lines, exact copy, shipped as named constants:
  - opt-out done: `Understood. From now on your messages here are not
    collected, and anything you send after this gets no answer. Your
    privacy commands still work. What was stored before stays until you ask
    for deletion with /privacydelete. Undo with /unblockprivacy.`
  - opt-out already so: `You are already opted out. Undo with
    /unblockprivacy, or delete stored data with /privacydelete.`
  - opt-in done: `Collection is on again for you. Nothing that was deleted
    comes back.`
  - opt-in already so: `You were not opted out. Nothing changed.`
  - confirm instruction: `To delete your stored data, reply /confirmdelete
    within five minutes. This removes your messages and identity data and
    cannot be undone.`
  - started: `Deletion is underway. Your messages and identity data are
    being removed.`
  - nothing pending: `There is no deletion waiting for confirmation. Start
    one with /privacydelete.`
  - tool ambiguity result: `Several people spoke in this turn, so the
    request is not acted on. The person concerned should send /privacyout
    or /privacydelete themselves.`
  - tool invalid action result: `The privacy tool accepts opt_out or
    request_deletion. Nothing was changed. Do not retry with other words.`
  - tool transient result: `The change did not take effect. Nothing was
    recorded. The person can use /privacyout or /privacydelete directly.`
  All idempotent, all working unaddressed in groups, all recognized via the
  reported invoked-command translation. Amended 2026-09-04 (unit 58): the
  opt-out line promises what the flag does and names the commands that still
  work.
- **The legal documents move with the unit, enumerated, 2026-08-23.** The
  policy: the rights section gains the in-chat sentence (naming the group
  reach while direct chats are off); the automated-decision sentence gains
  its honest carve-out — deletion runs by machine exactly when a person
  commands and confirms it, which is the person's decision, not the
  machine's; the objection sentence updates — suppression now exists, so
  objecting to collection going forward IS honored in place; the deletion
  section's reach list gains the surviving stub, named as what remembering
  the objection costs. The record of processing gains the suppression flag
  as a data item with its purpose; the impact assessment's
  objection-handling section records the honored-by-machine path as the
  safeguard it is. All pinned in the docs test.

## The unit's contract

The flag column by appended migration step; the read-only identity lookup;
the ordering above; the suppression drop reusing the disregarded outcome
with its widened doc; the four command semantics with the principal-keyed
reply window, budget exemption, and reply-granted state changes; the
pending-confirmation memory (principal-keyed, window, cap, sweep, restart
forgetting); the spawned erasure with the fence taken outside ingestion and
the conditional stub-keeping empty; the privacy tool (actions, principal
resolution with the single-principal rule, fence-guarded flag write, the
amendment's second clause, transient results); the fixed lines above as
named constants with exact copy; the prompt's explicitness and
relay-verbatim teaching; the enumerated document updates. The adapter is
untouched — the command translation already carries the tokens, and the
disregarded outcome is already acknowledged.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied
  warnings; vocabulary and secret scans clean; no new dependency; a
  previous-unit store upgrades through the appended step alone — pinned.
- **AC2** Suppression, the full no-write claim: after `/privacyout`, the
  person's group message leaves no message row, no identity refresh, no
  principal write, no conversation, no palette append, no mapping (each
  table read raw), no answer, and the offset advances; a fresh-channel
  first message from a suppressed person creates nothing; their `/privacy`
  and `/unblockprivacy` still answer WITHOUT refreshing the frozen display
  fields; after `/unblockprivacy` the next message records and answers;
  repeats answer the already-so lines — pinned over the wire.
- **AC3** Deletion: the confirm instruction; a cross-chat confirm (asked in
  one group, confirmed in another) runs; the erasure executes after the
  ingestion returned (no deadlock — pinned by the flow completing), empties
  the person, keeps the stub iff the flag stands, deletes the row
  otherwise; the started, nothing-pending and post-window lines exact; a
  second confirm answers nothing-pending; a restart forgets the pending
  state; an erasure failure leaves the pending consumed, the data standing
  and a log line — pinned with paused time.
- **AC4** The tool: an explicit ask makes the scripted model call it;
  single-principal origin set acts on that person; the absorbed
  co-summoner shape DECLINES with the ambiguity result; none and
  erased-row shapes decline; `request_deletion` files the pending and the
  result carries the literal confirm token; invalid and absent actions
  answer the invalid-action result; a failed flag write answers the
  transient result and changes nothing — pinned end to end.
- **AC5** Bounds: the per-person window bounds one person without touching
  another (member A's flood, member B's confirm instruction still
  answers); a budget-silenced sender's rights command still answers; the
  state change lands exactly with the granted reply; the palette governs
  the tool; erasure with the flag standing keeps the stub and the repeat
  reports completion — pinned.
- **AC6** The decisions recorded with dates and rejected alternatives; the
  0044-amendment's second clause recorded; the erasure decision's
  idempotency refinement recorded; every fixed line matches the spec copy
  verbatim; the four policy edits and the two assessment notes ship —
  pinned in the docs test.
