# Unit 8 — the wiki lookup and the report

Date: 2026-08-23. Revision 2, rewritten after the cold gap probe returned fourteen
findings including two critical ones: revision 1 gave the report no delivery
contract (the edge's wake timing, its restart seeding and its error path each
dropped a filed report silently) and resolved the reply target through the
dispatch anchor, the mechanism decision 0043 itself records as reachable by a
bystander. Revision 2 states the delivery contract with its accepted losses,
moves target resolution onto the debt-origin walk, and pulls the palette
supersession forward — the stale-palette gap this unit would otherwise ship is
the mid-session palette mechanism the next unit needs anyway. Status: settled
for implementation.

Two capabilities close the assistant's v1 feature set: answering from the
project's wiki, and filing a spam report with the group's moderation bot when a
member asks. Both were researched against the live services; every mechanism
names its verified backend. The report half settles the debt decision 0018
recorded: the outbound edge learns to carry an origin, and replies gain
threading.

## Decisions taken with this unit

- **The wiki tool reads the project wiki's raw pages, 2026-08-23.** Decision 0038
  ruled the wiki tool out while the project SITE had no wiki; the project's
  manifest repository wiki on the mirror forge is a different backend and it is
  real — sixteen pages, served raw as plain text at a stable unauthenticated
  address with no redirect. The lookup takes a page name (the title with spaces
  as dashes; parentheses literal), performs one bounded GET against a configured
  base address defaulting to the real raw host, and returns the page text
  decoded lossily as UTF-8; the model-facing result is bounded by its own named
  constant with a truncation marker — a truncated wiki page is a degraded
  answer, not a changed meaning, unlike rules. A missing page is a tool error
  naming the page-name shape. Page-name validation gets its own predicate — the
  existing repository predicate rejects parentheses. The raw host publishes no
  rate-limit contract, so the tool keeps a per-process response cache: keyed by
  the full request address, a named TTL matching the host's own cache header
  (five minutes), misses and 404s cached alike (negative caching bounds a model
  guessing page names), a named entry cap cleared whole when hit — the
  established memory-cap shape. Rejected: enumerating pages via the wiki's git
  transport (a clone for a page list, the shape unit 5 already rejected); the
  page-index scrape (an HTML page with no stable contract); content-type
  enforcement (the host says plain text today; an HTML body on a 200 passes
  through as text and reads as what it is); waiting again (the backend exists).
- **The model learns the page names from the wiki itself, 2026-08-23.** The
  tool's description teaches the name shape and names the entry page; the model
  starts at the entry page or the sidebar when it does not know a page, both
  ordinary fetches. No page list lives in code or configuration. Rejected: a
  configured page list (drifts); embedding today's names in the prompt (drifts
  identically, and the prompt is the maintainer's document).
- **The outbound edge carries an origin, and replies thread, 2026-08-23** (the
  deferral of decision 0018 falls due). The outbound reply gains an optional
  reply-target carrying the platform origin of the message it answers; the
  adapter translates it into the platform's reply parameters (the current
  primitive — the old reply field was replaced two platform versions ago) with
  send-without-reply tolerance, so a deleted target degrades to a plain send.
  The chunking rule threads only the first chunk. The model's ANSWERS stay
  unthreaded in this unit — decision 0018's judgment stands; the field exists
  for the report and whatever answer-threading decision comes later. The
  adapter's reply decode grows the replied-to message id (today it decodes the
  author alone); a reply without a usable id stores no target. Rejected:
  threading answers now (a product-texture change smuggled into a plumbing
  unit; the operator's call).
- **A report is a member's ask, filed as a block, delivered with the turn's
  answer, 2026-08-23.** The flow: a member replies to an offending message and
  addresses the assistant asking for a report. The inbound message gains the
  reply target's origin as a translated field beside the addressed flag; it is
  stored on the chat-message row (its own appended migration step) under the
  same erasure null as the existing origin. The report tool — member authority,
  group conversations only, under the same admission gate as every tool (the
  gate supplies no extra protection at member authority; stated, not implied) —
  takes NO target parameter: it resolves the reply target through the DEBT
  ORIGIN WALK decision 0043 settled, never the bare anchor (the anchor can be a
  bystander's line — 0043 records the exact shape). The target is the newest
  co-summoner's stored reply target; several co-summoners with targets resolve
  to the newest, stated; a turn whose origin set carries no reply target gets a
  tool error saying a report needs a reply. A reply to the assistant's own
  message is refused with its own error. Executing the tool appends a report
  block — a new consumer kind carrying the target origin, the reported
  message's principal id, and the fixed report line — under the erasure fence,
  which the tool receives at construction; the block is agency-inert,
  frontier-transparent like the context note, read through by the consumer's
  debt walk (its exclusion set widens from notes exactly to the consumer's
  delivery kinds), and classified as an explicit extend in the provenance
  chain, not left to the default. The block projects nothing: the filed report
  is machinery, and the model's knowledge of it is the tool result. This
  crosses unit 5's "no tool writes anywhere" rule, which gains its dated
  amendment: a tool may append blocks of kinds that exist for tool-driven
  delivery; lookups still write nothing. Rejected: a side channel to the
  adapter (a third outbound path for one line); a model-chosen target
  parameter (projection carries no message handles, and adding them ships
  identifiers to the provider against the recorded posture; the member's reply
  is ground truth); autonomous spam detection (a model turn per unaddressed
  message is a cost decision the operator has not made — deferred, recorded).
- **The report's delivery contract, with its accepted losses, 2026-08-23.** The
  consumer's outbound edge delivers report blocks threaded — the fixed line
  `/report@` plus the configured moderation handle, sent as a platform reply to
  the reported message — on BOTH stream events: with the answer on the turn's
  completion, and on the turn's failure beside the notice, so a turn that dies
  after filing still files. Ordering within the completion: the report sends
  before the answer text, so the member's confirmation reads after the deed. A
  report block undelivered when the process dies is LOST — the edge's restart
  seeding stands, and re-delivering reports from history would ping every group
  admin at-least-once; for a moderation nudge the accepted loss is the safer
  direction, recorded plainly. A failed platform send is logged and not
  retried, same acceptance. The tool result's wording claims filing, not
  arrival: the exact copy ships in the spec's constants and says the report
  goes out with this turn. Rejected: a durable delivery cursor for reports
  (at-least-once against a line that pings every admin); delivery through the
  deterministic return path (the ingest call returned before the turn ran);
  waiting for the next successful turn on failure (a report delivered an hour
  late, threaded onto old context, is a different product event).
- **The report rides the turn's budget and its own atomic window, 2026-08-23.**
  A report ask consumes an answer slot like any addressed turn. Filings are
  additionally bounded per channel by the atomic line-window primitive under
  its own named constant (`REPORT_WINDOW`); the slot is taken only after the
  block append stands, mirroring the unit-7 ordering fix; a second tool call in
  the same round loses the atomic grant and gets the declined result. The
  window is process memory: for this bound a restart forgives at most one
  extra report, and the re-argument is recorded here rather than inherited
  from the courtesy-line rationale. The window instance is constructed where
  the tool set is assembled and injected into the tool at registration — the
  tool never reaches into the assembly. Declined and error results teach
  no-retry in the admission wrapper's established wording style. Rejected: an
  unbounded filing path (a hidden-mention flood a hostile member controls);
  spending the window at delivery (the edge would need the window and a
  dropped delivery would re-arm a spent ping — the conservative direction is
  fewer reports, never more).
- **The palette supersedes on delta, and existing conversations gain the new
  tools, 2026-08-23** (pulled forward from the mid-session palette plan; the
  probe showed unit 5's no-pre-existing-store assumption expired when unit 7
  deployed). Admission already reads the newest palette block. Ingestion (and
  observation), under the stamp lock, compare the newest stored palette against
  the registered tool set on each conversation's first activity per process and
  append a fresh palette block on delta — the same on-delta shape as the
  context note, one write per real change. Conversations created before this
  unit therefore admit the wiki and report tools on their next activity; a
  future palette change reaches live conversations the same way. The palette
  block stays inert and invisible; nothing summons the model about it (the
  model-visible palette-delta note is the next unit's concern, recorded there).
  Rejected: a migration backfill (a one-shot fix that leaves the next change
  stranded again); per-conversation registration state (the ledger's newest
  palette IS that state).
- **The prompt regains the report bullet, tied to the tool, 2026-08-23.** The
  gated line returns per decision 0046, and the prompt's tool teaching names
  the report tool as the ONLY way to report — the model is told never to write
  the moderation command in answer prose. The residual — a disobedient model
  typing the command into an answer — is accepted with its reasoning: the
  moderation bot acts only on command REPLIES, and the assistant's answers are
  unthreaded, so stray prose is noise, not a filed report. Recorded in 0046's
  closure. Rejected: outbound prose sanitation (censoring the assistant's own
  speech on a pattern is the bolted-on conditional the structure rule forbids).
- **The moderation handle and report line, 2026-08-23.** The handle is an
  optional configuration key — trimmed, a leading `@` stripped, refused empty
  after trimming; absent means the report tool does not register and the
  palette-delta mechanism removes it from conversations that had it. One
  global handle: one deployment serves one community; per-group handles are
  rejected until a second community exists. The line's wording is a named core
  constant. The wiki base address is trimmed and refused empty the same way.
  The platform-side setup is operational and recorded in the operator
  reference document: bot-to-bot communication enabled for the assistant, the
  moderation bot's bot-to-bot setting opened to all bots, and the assistant
  NOT a group administrator — the moderation bot ignores administrators'
  reports, so an administrator assistant files into silence. Whether the
  moderation bot honors a bot's report at all is undocumented; the first live
  filing settles it, and the reference document says so plainly.
- **Erasure reaches the report block, 2026-08-23.** The report block stores the
  reported message's principal id precisely so erasure can reach it: the
  reported person's erasure nulls the block's target origin (and the line goes
  undeliverable — the edge skips a targetless report); the reporter's erasure
  nulls the reply-target column on their own message row through the existing
  author-keyed pass. The tool's append holds the erasure fence, so a report
  cannot re-materialize an origin an erasure just nulled. The block's line
  text stays (it names nobody). This narrows the 0045 lineage instead of
  joining it, recorded. Rejected: the OPEN-set shrug (the block exists to
  carry an identifier; shipping it unreachable would be the exact gap 0003
  exists to prevent).
- **The privacy documents move with the capability, 2026-08-23.** Shipping a
  report capability and a new egress falsifies four tracked drafts that state
  no moderation capability ships. This unit updates them: the policy's
  moderation sentence, the impact assessment (its own review trigger names
  this unit's shape — a dated addendum covers the report disclosure to group
  admins and the wiki fetch egress), the legitimate-interest assessment's
  capability line, and the record of processing's data and recipients tables
  (the reported message's identifier as a data item; the group's admins, via
  the moderation bot, as recipients of the report event). Drafts amended in
  place with dated notes — they are unpublished; the no-edit rule binds
  decision records, not drafts.

## The unit's contract

### Core

The wiki lookup module beside the two existing lookups: page-name predicate,
text-body bounded GET (a sibling of the JSON-only fetch), lossy UTF-8 decode,
model-facing result bound with truncation marker, the keyed cache with TTL,
negative caching and clear-at-cap, member authority. The report tool module:
no parameters, group-only, origin-walk target resolution, self-report refusal,
the atomic window taken after the append, the erasure fence held, three result
constants with no-retry teaching. The report block kind: target origin,
reported principal id, line text; inert, frontier-transparent, debt-walk
read-through, explicit chain-extend; content table by appended migration step.
The chat-message reply-target column by its own appended step under the
author-keyed erasure null. The outbound reply's optional reply target; the
edge delivers report blocks threaded, before the answer, on completion and on
failure, skipping targetless ones. The palette-delta append under the stamp
lock on first activity per process. The prompt regains the report bullet;
unit 5's no-write rule gains its dated amendment; erasure's pass extends to
the report table keyed by the reported principal. All fixed wording in named
constants with the exact copy pinned.

### Adapter

Reply decode grows the replied-to message id; reply-target translation stores
nothing without a usable id. The send path speaks the platform's current
reply parameters with send-without-reply tolerance; only the first chunk
threads. No behavior.

### Configuration

Two optional keys under the refused-unknown-keys rule: the wiki raw base
address (real-host default, trimmed, refused empty) and the moderation bot
handle (trimmed, leading `@` stripped, refused empty; absent = report tool
unregistered).

### Documentation

The operator reference document gains the report setup (the three
platform-side switches, the not-an-administrator requirement with its reason,
the live-test caveat). The four privacy drafts updated as decided. The
decisions above recorded with dates and rejected alternatives; the 0044,
0045 and 0046 lineages amended as named.

## Acceptance criteria

- **AC1** Workspace suite green both modes; clippy, fmt, doc denied warnings;
  vocabulary and secret scans clean; no new dependency; the store upgrades
  through the appended steps alone (both new steps) — pinned.
- **AC2** Wiki end to end over the tool-scripted provider and a scripted raw
  host: a page fetch reaches the model and the answer reaches the chat; a
  missing page and a timeout become tool errors; the cache serves a repeat
  inside the TTL without a second request, refetches past it, caches a 404,
  and clears at the cap; the result bound truncates with the marker; the
  default base address is the real host — pinned with paused time.
- **AC3** The page-name predicate pinned: dashes, parentheses, rejection of
  path separators and empties; the existing predicates untouched.
- **AC4** Report end to end over the adapter: member replies to an offending
  message and asks; the tool resolves the target through the origin walk
  (pinned including the absorbed-bystander shape — the bystander's reply
  target loses to the co-summoner's); the block lands with target origin and
  reported principal; the edge sends the fixed line as a platform reply to
  the offending message's id, before the answer, against the scripted wire;
  the tool result names the filing — pinned block by block.
- **AC5** Report bounds and failure shapes pinned: no-reply error;
  self-report refusal; direct-conversation refusal; second ask inside the
  window declined with the no-retry result; the window reopens under paused
  time; the slot is taken only after the append (transient append failure
  spends nothing); a turn that errors after filing still delivers the report
  beside the notice; the ask consumed an answer slot.
- **AC6** Erasure and absence pinned: the reported person's erasure nulls the
  block's target and the edge skips it; the reporter's erasure nulls their
  row's reply target; the platform-deleted target sends without reply; a
  non-reply stores no target; the tool's append respects the erasure fence.
- **AC7** No handle configured: the report tool absent from registration and
  from a fresh palette, and REMOVED from a pre-existing conversation's
  palette by the delta append; the wiki tool stands alone. A pre-unit
  conversation gains both tools on first activity — pinned.
- **AC8** The prompt regains the bullet tied to the tool; 0046 records the
  gate closed with the prose residual; the 0044 amendment, the erasure
  narrowing and the privacy-draft updates are present; the answers stay
  unthreaded; the exact copy of every new constant matches the spec — pinned.
