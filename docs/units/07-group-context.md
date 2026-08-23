# Unit 7 — group context

Date: 2026-08-23. Revision 2, rewritten after the two-seat spec probe: the unbriefed
gap audit returned sixteen findings including two critical ones (a context note
appended over an owed message buries the turn; the specified outbound transport does
not exist), and the platform fact check refuted five of seven claims revision 1
leaned on (the sticky update-selection setting, the pin payload's inaccessible form,
the by-sending-date pin lookup, the membership shape, and the command's mandate).
Revision 2 replaces the membership gate with a persisted fail-closed authorization
model and moves the new outbound items off the event edge entirely. Status: settled
for implementation.

The assistant meets its first real group knowing nothing about it: no name, no
rules, no say over who put it there. This unit gives the session the group's own
facts — its title and its rules, read from the group itself instead of hardcoded —
teaches the assistant to acknowledge a rules change where everyone can see it,
restricts group membership to invitations from the operator, and answers the
privacy command. It is the unit the first deployment waits on.

## Decisions taken with this unit

- **Group facts come from the group, as context notes on the ledger, 2026-08-23.**
  The group's title and rules are observed facts, not configuration. They land as a
  new consumer block kind — the context note: a topic and a text, agency-inert,
  frontier-transparent, projected to the model in the system voice, appended only
  when the observed text differs from the newest stored note of the same topic.
  The system-voice projection follows the framework's date marker; providers join
  system lines rather than overwrite, so a note never erases the system prompt.
  Unlike the date marker, a note is appended by an independent path at an
  arbitrary moment, so it must never bury a debt: the kind answers the framework's
  frontier-transparency hook (the walk reads through any transparent kind — probed
  against the merged walk), and the consumer's own owing-tail read walks past
  notes the same way, so a note landing on top of an unanswered message leaves the
  turn owed and the debt propagation intact. Notes accumulate in stream order and
  the projection wording makes the newest authoritative — the event-sourced answer
  to supersession; a framework superseding-block, already on the improvements
  list, is the future compaction, not a blocker. Rejected: rendering rules into
  the system prompt block (written once at creation, an edit never reaches a live
  conversation); a mutable rules row (blocks are the only content unit and storage
  is append-only); deferring the append until no debt is open (a queue and a
  second delivery problem, when transparency answers the ordering outright).
- **The rules contract reads the pinned announcement, 2026-08-23** (the operator's
  contract, recorded in the tracked reference document this unit adds). Both
  target platforms share the concept of pinning a message; the platform delivers
  a pin as an event, and its lookup exposes exactly one pinned message chosen by
  the pinned messages' SENDING dates — not by pin recency. The adapter therefore
  reports two platform-neutral observations: the channel's title, and the text of
  a pinned announcement (the one the lookup exposes, or the one a pin event
  names). The CORE owns the contract: a pinned text whose first line is exactly
  the rules prefix `Rules:` followed by a newline — case-sensitive, a carriage
  return before the newline tolerated, nothing before the prefix — is the group's
  rules; the prefix line is stripped and the remainder becomes the rules note. A
  remainder that is empty after trimming is not rules and is refused with a log
  line. A pinned text without the prefix is not rules and supersedes nothing. A
  pin whose content the platform withholds (the inaccessible form) yields no
  observation. The rules text is bounded by a named byte constant; an over-bound
  rules text is refused whole with a log line, never truncated — a cut rule is a
  different rule. Cold start is operational: because the lookup selects by
  sending date, an old rules pin can sit invisibly behind a newer announcement —
  the reference document tells the operator to post a fresh rules message and pin
  it, and the acknowledgment confirms the pickup. Rules removal has no event on
  the platform and is out of mechanical reach: a stale note stands until the next
  rules pin, and the reference document says to replace rules, not merely unpin
  them — recorded plainly. Rejected: fetching pin history (the platform exposes
  no enumeration); treating every pin as rules (groups pin announcements too);
  adapter-side prefix parsing (the contract is product behavior; the adapter only
  translates); truncating over-bound rules (meaning-changing).
- **Whoever can pin can steer the assistant — recorded as accepted trust,
  2026-08-23.** A rules note is a system-voiced line written by whoever holds the
  group's pin right. That is the point of the feature — the group governs its
  assistant — and pinning is an administrator right in the target groups. The
  byte bound above caps the surface; the trust boundary is the group's own admin
  set and is stated in the reference document. Rejected: an operator-only rules
  source (the operator IS a group admin; a second gate adds a knob, not safety).
- **Deterministic outbound rides the call's return, not the event edge,
  2026-08-23.** Message ingestion is already a direct call whose result the
  adapter driver classifies; the observation surface follows the same shape, and
  everything deterministic this unit sends — the acknowledgment, the privacy
  answer, the withdraw directive — is returned from that call as a value the
  driver translates (a text to send, a leave to perform). The event edge stays
  exactly what it is: the model's answers and the failure notice. No consumer
  event type, no bus rework, no mapping-row dependency for the withdraw, and
  redelivery semantics come out right by construction: a replayed update
  re-returns an idempotent directive, and the on-delta rule keeps a replayed
  acknowledgment from repeating. Rejected: a consumer event composed over the
  framework bus (reshapes the event type across the assembly, the streams, the
  tool bounds and the edge, to deliver three deterministic lines); emitting the
  acknowledgment from the stored note via the edge cursor (at-least-once against
  a courtesy line's at-most-once intent).
- **A rules change is acknowledged in the chat, with fixed wording, 2026-08-23.**
  When an observation appends a rules note — new or changed — the returned value
  carries the acknowledgment: the fixed line `Rules noted. The assistant follows
  the pinned rules of this group.`, a named constant beside the failure notice.
  Deterministic product behavior, not a model answer: no turn, no budget slot, no
  wording drift, and the note itself stays inert. At most one acknowledgment per
  channel per acknowledgment window (a named-constant cooldown): within the
  window a further delta still appends its note, silently — the flood-amplifier
  discipline the protection unit recorded for notices applies to any bot line a
  non-operator can trigger. Title changes are not acknowledged. Rejected: a
  model-generated acknowledgment (a turn for a confirmation line); unbounded
  acknowledgments (a pin-toggling admin makes the bot spam the chat); silence
  (the operator asked for visible confirmation).
- **Group membership is authorized, persistently, fail-closed, 2026-08-23.** The
  operator's invitation is a durable fact, not a fleeting event. A new
  authorization table (its migration step appended per the schema discipline)
  records which group channels the operator admitted. It is written exactly one
  way: a membership observation — the assistant added to a group, with the acting
  principal — whose adder matches the configured operator. Everything else fails
  closed: a group message, or any observation, for a group channel with no
  authorization row is refused without touching the ledger, and the refusal
  carries the withdraw directive; a membership observation with a foreign adder,
  or with no operator configured, or with no adder named, returns the directive
  too and records nothing. The gate therefore needs no delivery guarantee: a
  failed or lost leave call is healed by the next contact from that group, which
  is refused and re-directed all over again, and a restart changes nothing
  because the authorization is a table row, not process memory. Existing group
  mappings at migration time are backfilled as authorized — they were admitted
  under the old regime by the operator's own hand. Direct channels are untouched
  by all of this. The membership transition the adapter reports is judged by
  membership, not by a status pair: from outside the group to inside it, in any
  member shape the platform grants (member, administrator, restricted-but-in),
  and only for group-kind chats — the platform fires the same update for private
  blocks and unblocks, which are nobody's invitation. Rejected: a fleeting gate
  on the add event alone (fails open on a lost event, a failed leave, or a
  restart); an adapter-side allowlist (behavior in the adapter); startup
  membership reconciliation (no platform surface enumerates the bot's groups;
  the fail-closed refusal covers the gap without one).
- **The privacy command answers deterministically, 2026-08-23.** The developer
  terms demand a privacy policy that is easy for users to reach; the command is
  this project's chosen surface for it (the platform mandates the policy, not
  the command — recorded precisely; the platform-side policy field named in the
  reference document is deployment wiring). A chat message whose first token is
  exactly `/privacy` — or `/privacy@` plus the assistant's own handle,
  case-insensitive on the handle, which the adapter normalizes away as
  translation; a foreign-handle suffix is NOT normalized and NOT answered, the
  command was aimed at someone else — is recorded on the ledger like any
  message, stamped as taking no debt through the stamp's existing limited
  classification extended with a command kind: no turn, no answer-window count,
  no unlatch, and a pending debt on the tail propagates past it exactly as past
  any non-owing message. The returned value carries the fixed answer: `Privacy
  policy: ` plus the configured address, or `The privacy policy is not published
  yet.` when none is configured. The command answers whether or not the message
  was addressed — invoking a command is addressing by form; the stored addressed
  column keeps the adapter's resolution untouched. When the channel's answer
  window is exhausted, the command is recorded and answered with silence, the
  same discipline the protection unit set for notices — a deterministic reply is
  not a protection bypass. Rejected: routing the command through the model (a
  legal pointer must be exact and free); counting command replies against the
  answer window (the stored counting shape is the owing shape — counting without
  owing would need a bolted-on stamp branch, and the window's job is bounding
  model cost); a separate cooldown mechanism (the exhausted-window silence rule
  already bounds the reply rate).
- **Observations may open a conversation; a lookup feeds them; authorization
  precedes both, 2026-08-23.** An observation for an authorized, unmapped group
  channel runs the same winner-only creation path a message does — system prompt
  and palette included — so a group's title and rules exist on the ledger before
  anyone speaks. The observation path holds the same two locks ingestion holds:
  the erasure fence (an observation must not create a mapping mid-erasure) and
  the stamp lock (the on-delta read-then-append must be serialized, or two equal
  observations both append). The adapter observes lazily: once per channel per
  process, on first contact with an authorized group — the assistant being
  added, or the first message seen — it looks the channel up (title and the
  exposed pinned announcement) and reports what it finds; it reports every pin
  event it sees thereafter. A membership observation from an add is reported
  before the add's lookup observations, so authorization is judged first. A
  failed lookup is best-effort: logged, retried on the next first-contact (the
  once-per-process memory is not set on failure), never halting the update
  batch — group facts are enrichment, not authority, unlike the admin fetch.
  Rejected: a core-to-adapter query surface (a new boundary for what a push
  solves); observing on every message (a platform call per message); suppressing
  the lookup in the adapter for unauthorized channels (an adapter decision; the
  core's refusal answers it, and a wasted lookup against a stranger group costs
  one call).
- **Erasure does not reach context notes — recorded OPEN, 2026-08-23, in the
  0045 lineage.** A rules text is governance prose that can in principle name a
  person, and the note table carries topic and text with no principal id, so
  erasure cannot reach it even in principle. Accepted with its reasoning: the
  note quotes the group's own published governance, not a person's conversation,
  and the erasure write path for non-message blocks is the same framework seam
  the tool-block OPEN waits on. Recorded as its own decision beside 0045,
  revisited when the framework offers the path.

## The unit's contract

### Core vocabulary

One new inbound surface beside message ingestion, accepting platform-neutral
observations that carry the channel key and channel kind: channel title, pinned
announcement text, and the assistant's own membership change with its acting
principal (optional — absence fails closed). The surface performs the same
channel-kind-mismatch check ingestion performs and returns the same
classification vocabulary, extended with the returned deterministic items: a
fixed text to deliver, and the withdraw directive. Message ingestion's return
gains the same item carriage for the privacy command's answer. One new block
kind, the context note (topic and text), content table by appended migration
step, inert agency, frontier-transparent, system-voice projection, on-delta
append per topic under the stamp lock. One new authorization table by appended
migration step, with backfill of existing group mappings. The core gains the
rules-prefix contract, the rules byte bound, the acknowledgment window, and the
privacy command semantics with the command-limited stamp kind. All fixed
wording lives in named constants beside the failure notice. No platform
vocabulary in the core; the invariant scan stays green (the scan checks
platform names — the reviewer seat judges the new vocabulary's neutrality).

### Adapter translation

The update selection becomes explicit: the poll names the update types it
consumes — messages, edited messages, and the assistant's own membership
updates — because an absent selection inherits whatever an earlier setting left
on the token. The update decoding gains the pin service message (the pinned
payload's inaccessible form — its date-zero discriminator — yields no
observation) and the membership update; pin handling precedes the
on-behalf-of-chat skip, because an anonymous-admin pin arrives exactly there.
Membership updates translate to observations only for group-kind chats, judged
by the membership transition, never by a literal status pair. The first-contact
lookup fetches title and the exposed pinned announcement. The withdraw
directive maps to the platform's leave call; a failure is logged and left to
the gate's self-healing. The command suffix normalization strips exactly the
assistant's own handle from a leading command token and nothing else. No
behavior: every decision above sits in the core.

### Configuration

Two additions under the refused-unknown-keys rule, both absent from the
repository: an operators table keyed by adapter name whose value is the
adapter-scoped external id of the operator (an empty value is refused at
load), and the optional privacy policy address.

### Documentation

A tracked reference document for group operators: the rules-pin contract
verbatim, the fresh-message-and-pin cold-start step, the replace-don't-unpin
rule, the trust statement, and the platform-side policy field note. The
deployment notes keep only deployment wiring. Follow-ups recorded, not built:
the group-to-supergroup migration strands the stored channel mapping (the
platform renumbers the chat); the wire client discards error response bodies
on non-success status, hiding the migration signal and every refusal detail;
the framework superseding-block compaction.

## Acceptance criteria

- **AC1** Workspace suite green, parallel and single-threaded identical; clippy,
  fmt, doc under denied warnings; vocabulary and secret scans clean; no new
  dependency; a store from the previous unit upgrades through the appended
  migration steps alone, pinned.
- **AC2** End to end over the adapter: a pin event carrying a rules-prefixed
  text appends one rules note, projects it in the system voice on the next
  turn, and delivers exactly one acknowledgment; the same text re-observed
  appends and acknowledges nothing; a changed text appends again; a second
  change inside the acknowledgment window appends silently — pinned block by
  block on the ledger.
- **AC3** The prefix contract pinned: strip, case sensitivity, carriage-return
  tolerance, refusal of the empty remainder, refusal of the over-bound text,
  the non-prefixed pin ignored, the inaccessible pin yielding nothing.
- **AC4** A note appended over an unanswered message buries nothing: the turn
  still fires anchored on the owed message and debt propagation reads through
  the note — pinned against the framework walk. A note landing between two
  chat messages renders a wire shape the live provider accepts — pinned at the
  provider render.
- **AC5** Authorization end to end over the adapter: an add by the operator
  writes the row and stands; a foreign add, an add with no operator configured,
  and a group message without authorization each return the withdraw directive,
  touch no ledger and map no conversation; the directive reaches the scripted
  leave call; a replayed membership update re-returns it idempotently; a
  restart preserves the authorization; migration backfills an existing group
  mapping — each pinned. A private-chat membership shape produces no
  observation — pinned in the adapter.
- **AC6** The privacy command: configured address answered with the fixed line,
  unconfigured with the not-yet-published line; no turn on the ledger, the
  latch untouched, a pending tail debt preserved past the command; the
  suffix-normalized form answers, the foreign-suffix form does not; the
  unaddressed group form answers; an exhausted answer window yields recorded
  silence — pinned.
- **AC7** Two racing equal observations append one note (the stamp lock holds
  the on-delta read-write); an observation racing an erasure respects the
  fence; an observation-created conversation carries the system prompt and the
  palette — pinned.
- **AC8** Decisions recorded with dates and rejected alternatives, the erasure
  OPEN among them; the reference document exists with the operator contract;
  the follow-ups are recorded; the fixed strings ship as named constants with
  the exact copy above.

## Refined at the unit's close, 2026-08-23

The closing verification and the two adversarial seats refuted parts of the
mechanisms above; the build closes them under these refinements:

1. **Authority resolution is deferred to the core's need.** The adapter no
   longer halts the batch when the administrator fetch fails for a group
   message; it delivers the message with authority unresolved. The core
   refuses an unauthorized group before ever reading authority — the withdraw
   needs none — and answers an authorized message whose authority is missing
   with a typed transient refusal the driver halts on, exactly as before.
   Nothing is recorded with a defaulted authority; the never-default rule
   holds. This closes the verified wedge: a stranger's group message arrived
   in front of an administrator fetch its own leave call had doomed, and the
   halt starved every chat behind it.
2. **The privacy answer shares the acknowledgment window.** The claim that
   the exhausted-window silence bounds the reply rate was refuted: the
   command stamp keeps the command out of both budget counts, so a quiet
   channel never exhausts and every repeat answered. Deterministic command
   replies are bounded the way the acknowledgment already is: at most one
   per channel per window, recorded silence within it. The rejected
   alternative "a separate cooldown mechanism" stands rejected — this is the
   same mechanism, shared.
3. **The title is bounded in the core.** The platform's title cap was
   load-bearing for the system-voice surface; the core owns its own bound —
   a named byte constant, an over-bound title refused whole with a log line.
4. **The ledger records what the person typed.** The adapter no longer
   rewrites a self-directed command suffix out of the stored text; the text
   lands verbatim, and the adapter reports the invoked command as its own
   typed translation beside the addressed flag. The core matches the
   reported command, never the text. A foreign-handle suffix reports no
   command.
5. **The consumer's transparent walk is scoped to context notes.** Walking
   past every frontier-transparent kind silently widened debt propagation
   onto the framework's turn-closure markers — a protection-relevant change
   nobody specified. The consumer walk reads through notes exactly; the
   framework's own walk keeps governing turn liveness. The failed-turn tail
   shape is pinned at its pre-unit behavior.
6. **A pin event outranks the lookup's pin.** When a pin event arrives for a
   channel whose first-contact lookup has not run, the lookup reports the
   title only — the event carries the authoritative text, and the lookup's
   by-sending-date pin would otherwise append stale rules and spend the
   acknowledgment on them.
7. **Note appends are bounded per topic per window.** The acknowledgment
   window bounded the chat line and left the ledger unbounded: a pin toggler
   appended a system-voiced note per toggle. Appends of one topic are capped
   within the window by a small named constant; a capped delta lands on the
   next observation after the window opens. The full-history hot-path reads
   are gone with it: the newest-note lookup and the owing-tail read are
   bounded queries, never a full conversation hydration.

## Refined 2026-08-23, by the operator's decision

The acknowledgment window on the rules line and refinement 7's note append
cap are removed. Both defended against a pin-toggling spammer who cannot
exist: pinning is an administrator-only right on the platform, so the only
people the bounds ever touched were admins making legitimate rules edits —
a second real correction within the window was picked up without its
confirmation, and a burst of real edits past the cap did not reach the
ledger until the window passed. The on-delta comparison stays as the whole
admission check: every real rules change appends its note and carries the fixed
acknowledgment, whatever the interval; an identical re-pin appends nothing
and says nothing. Refinement 7's bounded hot-path reads (the newest-note
lookup, the owing-tail read) are unrelated to the cap and stand. The
notice-answer window of refinement 2 is untouched: a flood of
notice-drawing triggers is a vector anyone in the chat can cause, so that
bound protects against something real. AC2's "a second change inside the
acknowledgment window appends silently" is superseded by this decision; the
pins now assert that a second real change acknowledges and records.
