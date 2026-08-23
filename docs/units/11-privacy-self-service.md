# Unit 11 — privacy self-service

Date: 2026-08-23. Revision 1. The operator's design, settled in conversation: a person
can opt out of collection, delete their data, and opt back in — through explicit
commands handled programmatically, and through plain language the model reads and
honors by calling a tool the system then enforces. This unit is the last prerequisite
before the assistant joins its first group.

## Decisions taken with this unit

- **Opt-out is a suppression stub, the one lawful remnant, 2026-08-23.** Honoring
  "do not collect me" requires remembering who asked: the identity row survives as
  a minimal stub carrying the opt-out flag — the suppression-list shape, lawful
  because storing the identifier is what honoring the objection takes. From the
  moment the flag stands, the person's inbound messages are dropped at ingestion
  before anything is written: no message row, no answer, no identity refresh — the
  display fields freeze as they were and erasure can empty them while the flag
  stays. Opt-out is not deletion: what was stored before the flag stands until
  deletion is asked for; both commands exist and the replies say which does what.
  Rejected: dropping the identity row too (the next message would be a stranger's
  and collection would silently resume — the exact failure the stub prevents);
  adapter-side suppression (an allowlist in the adapter is behavior there, and
  the identity tables are the core's).
- **The privacy command family is exempt from suppression, 2026-08-23.** An
  opted-out person's `/unblockprivacy` must work, or opting back in is
  impossible; their `/privacy` keeps answering, since the notice must stay
  reachable. The exemption is exactly the deterministic command family — an
  exempted command is recorded like any command message (command-limited stamp)
  and answered deterministically; everything else from the person is dropped.
  Rejected: full suppression (a one-way door nobody can reopen from inside).
- **Deletion confirms programmatically, always, 2026-08-23.** `/privacydelete`
  answers with the fixed confirm instruction; the deletion runs only when
  `/confirmdelete` arrives from the same person within the confirm window (a
  named constant, minutes). The pending confirmation is process memory: a
  restart forgets it and the person asks again — deletion is the one flow where
  forgetting errs safe. The confirm consumes the pending state, runs the
  existing erasure operation, and answers with the fixed completion line. A
  confirm with nothing pending gets the fixed nothing-pending line. The model
  is never in the destructive path: even the tool-initiated deletion only files
  the same pending state and relays the same confirm instruction. Rejected: a
  model-judged confirmation (the destructive step rests on exact string match
  or it rests on judgment, and judgment misfires); a durable pending store (a
  restart re-arming a half-asked deletion is worse than asking twice).
- **Plain language reaches the same mechanisms through one tool, 2026-08-23**
  (the operator's dual-path design). A member who writes, explicitly, that they
  want no collection or want their data gone — the operator's examples: "I dont
  want this bot to collect my data, please delete it", a reply to the assistant
  saying "Stop reading my messages" — is honored without knowing the commands:
  the model calls the privacy tool, whose actions are opt-out and
  request-deletion. The tool takes NO target: it acts on the turn's debt-origin
  summoner, resolved exactly as the report tool resolves its target, so the
  model cannot aim it at anyone but the person who asked. Opt-out applies
  immediately — the tool writes the flag through a guarded consumer surface
  under the erasure fence; request-deletion files the pending confirmation and
  the tool result carries the confirm instruction for the model to relay. The
  explicitness bar lives in the tool description and the prompt: clear requests
  trigger it, vague grumbling gets an answer pointing at the commands. The tool
  is member authority, registered beside the others, palette-governed.
  Rejected: LLM-triggered deletion without the programmatic confirm (the
  safeguard exists precisely because the model can misread); a target
  parameter (unforgeable summoner resolution is the whole point).
- **The command tokens and replies are fixed, 2026-08-23.** `/privacyout` opts
  out, `/privacydelete` starts deletion, `/confirmdelete` confirms it,
  `/unblockprivacy` opts back in (the operator's name for it). All four join
  the deterministic command family beside `/privacy`: recognized by the
  reported invoked-command translation, answered with fixed lines shipped as
  named constants (exact copy pinned), recorded with the command stamp, bounded
  by the shared reply window, working unaddressed in groups. Opt-out and
  opt-in are idempotent: repeating the standing state answers with the fixed
  already-so line and changes nothing. Rejected: burying the actions in
  `/privacy` arguments (four explicit tokens beat a flag grammar in a chat).
- **What each action reaches is stated, not implied, 2026-08-23.** Opt-out:
  stops collection and answering, keeps history until deletion. Deletion: the
  existing erasure operation, whose reach and named gaps the privacy documents
  record; the completion line does not overclaim. Opt-in: lifts the flag,
  collection resumes at the next message, nothing is restored. The stub after
  deletion holds the flag and the frozen-empty identity, nothing else. The
  policy's rights section gains the in-chat sentence once this unit ships; the
  impact assessment's objection-handling section records the honored-by-
  machine path as the safeguard it is.

## The unit's contract

The opt-out flag on the identity tables by appended migration step (frozen-list
discipline where enums are quoted); the suppression check at ingestion before
any write, after the invoked-command exemption; the pending-confirmation memory
with its window constant; the four command semantics beside the privacy
command's, sharing its stamp, window and unaddressed rule; the privacy tool
(no parameters beyond the action, summoner-resolved, fence-guarded flag write,
pending-filing for deletion) registered at member authority; the fixed lines
as named constants with exact copy; the prompt's explicitness teaching; the
policy sentence and the impact-assessment note. The adapter is untouched
except that nothing new crosses its boundary — the command translation
already carries the tokens.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency; a previous-unit store
  upgrades through the appended step alone — pinned.
- **AC2** Suppression end to end: after `/privacyout`, the person's group
  message is dropped (no row, no identity refresh, no answer, offset advances);
  their `/privacy` and `/unblockprivacy` still answer; after `/unblockprivacy`
  the next message is recorded and answered; repeats answer the already-so
  lines — pinned over the wire.
- **AC3** Deletion: `/privacydelete` answers the confirm instruction;
  `/confirmdelete` inside the window runs the erasure (the person's text
  emptied, identity emptied, stub and flag surviving) and answers the
  completion line; outside the window or with nothing pending, the fixed
  lines; a restart forgets the pending state — pinned with paused time.
- **AC4** The tool: an explicit plain-language ask makes the scripted model
  call the tool; opt-out acts on the resolved summoner (the absorbed-bystander
  shape pinned — the bystander is not opted out); request-deletion files the
  pending state and the relayed result carries the confirm instruction; the
  tool declines in a turn whose origin set is empty of people — pinned end to
  end.
- **AC5** Interactions: an opted-out person's pending deletion still confirms
  (the command exemption covers it); erasure with the flag standing keeps the
  stub; the answer-window and command-window bounds hold for the new commands;
  the palette governs the tool — pinned.
- **AC6** The decisions recorded with dates and rejected alternatives; the
  policy sentence and the impact-assessment note ship; the exact copy of
  every fixed line matches the spec constants — pinned in the docs test.
