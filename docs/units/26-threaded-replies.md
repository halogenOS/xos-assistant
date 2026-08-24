# Unit 26 — an answer to someone who asked is delivered as a reply to them

Date: 2026-08-24. Revision 2, narrowed after a cold probe ran the real turns and killed
revision 1. When a member addresses the assistant and it answers, the answer arrives as a
loose message with nothing tying it to the question. Threading it is the fix — but only
where the assistant can name the message being answered without guessing, which is a much
smaller set than revision 1 assumed.

## Grounding (what the probe computed, against real runs)

**The turn does not record who asked.** Revision 1 rested on `dispatch_anchor` being "the
message that summoned the turn". It is the FRONTIER, and this repository says so where it
matters: "the id of the summoning frontier … the anchor names the frontier alone"
(`core/src/kind.rs:911-914`), and "a bystander's line that happened to become the dispatch
frontier never speaks its carried min-fold as if the bystander had summoned the turn"
(`core/src/tools/provenance.rs:22-25`). Probed on real turns: three members talking and one
turn answering Alice anchors on the oldest warm-up line; a batch that dies followed by an
unrelated message anchors on that unrelated speaker; a failed turn followed by someone
re-engaging anchors on the re-engager. Threading onto the anchor would publicly quote-reply
the wrong member routinely. Decision 0018's objection is not spent — it is inverted.

**What IS recorded is which messages the turn absorbed, and which of them addressed the
assistant.** `provenance::co_summoners` (`core/src/tools/provenance.rs:107`) walks a turn's
absorbed messages, and a chat message stores its `addressed` fact and its platform origin.

**Threading every answer would arm a live hazard.** Decision 0046 accepted "a disobedient
model typing the command into an answer anyway" as harmless, in terms: "the moderation bot
acts only on command REPLIES, and the assistant's answers are unthreaded (decision 0059
keeps them so), so stray prose is noise, not a filed report", and it REJECTED outbound prose
sanitation as the alternative. The moderation bot acts on a reply carrying the command shape
(`core/src/tools/report.rs:205-207`). A threaded answer whose prose contains that shape —
a member asking what the command does, a model slip, an injected line — becomes a real
filed report against whatever message it threaded onto, bypassing every check the report
tool performs.

**A refused target loses the answer today.** The adapter's `allow_sending_without_reply`
covers exactly one case, "message to be replied not found" (`adapters/telegram/src/
client.rs:412-427`). Any other refusal becomes a failed send and the reply is dropped
(`driver.rs:747-751`). Threading introduces a way to lose an answer that does not exist
while answers are unthreaded.

**The projection half is dropped, and why.** Revision 1 also rendered the reply link into
what the model reads. The probe proved that leaks: the deletion request keeps its own reply
reference by decision 0085, so an erased message's origin prints on the very next line
(breaking existing pins at `adapters/telegram/tests/adapter/deletion.rs:77` and
`core/tests/spine/mirror.rs:200`), and the accepted erasure residual at
`core/src/kind.rs:143-150` would be exported to the vendor on every request. With ids
visible, a model naming the reply-target id files a report against the innocent message and
`resolve_reportable` accepts it — proven, not theorised. None of that is worth a
convenience, so this unit renders nothing new to the model.

## Decisions taken with this unit

- **An answer threads only onto a message that addressed the assistant, and only when
  exactly one did, 2026-08-24.** The target is resolved by walking the turn's absorbed
  messages and taking the one whose stored `addressed` fact is true. Exactly one: none means
  nobody asked, several means the turn answered a crowd and picking one tells the others
  they were ignored. This supersedes decision 0018's answer clause and decision 0059's
  restatement of it, both amended in place with this date and reason. *Rejected:* threading
  onto the dispatch anchor (revision 1 — it is the frontier, and the probe showed it lands
  on bystanders); *rejected:* threading onto the newest addressed message when several
  addressed (the same guess decision 0018 refused, wearing a different hat).
- **Helpful-mode answers are never threaded, 2026-08-24.** In helpful mode nobody addressed
  the assistant, so the rule above yields no target by construction rather than by a special
  case. Recorded as a decision because it is a product judgment as much as a mechanical
  outcome: answering an unaddressed message is a courtesy, and quote-replying someone who
  never asked, in front of the group, is not.
- **An answer whose prose carries the report command shape is never threaded, 2026-08-24.**
  The hazard decision 0046 accepted exists only when the answer is a reply, so the answer
  stays a plain message instead. This preserves 0046's reasoning rather than overturning it,
  and it is not the prose sanitation 0046 rejected: nothing is rewritten, stripped or
  refused — the text goes out exactly as written, unthreaded.
- **A refused target retries plainly and the answer is never lost, 2026-08-24.** Where the
  platform refuses a send that carried a reply target, the adapter sends the same text once
  more without it. The retry is bounded to that one cause and that one attempt: it is the
  thread that failed, and an answer must not be lost to a courtesy. *Rejected:* relying on
  `allow_sending_without_reply` (it covers only a deleted target).
- **Nothing new is rendered to the model, 2026-08-24.** See the grounding: the projection
  half leaks an erased origin, exports the erasure residual, and lets a model file a report
  against the wrong message. Dropped whole.
- **No new stored fact, 2026-08-24.** The addressed flag, the origin and the absorbed-message
  walk all exist.

## The unit's contract

When exactly one of the messages a turn absorbed addressed the assistant, and the answer's
prose does not carry the report command shape, the answer is delivered as a reply to that
message, first chunk only; where the platform refuses that send, the same text is sent once
more without the target. In every other case — nobody addressed it, several did, the prose
carries the command shape, the origin is absent or erased — the answer is delivered plainly,
and in no case is an answer withheld or lost. Reports keep their existing threading and
their existing undeliverable handling. Nothing new reaches the model, no new stored fact, no
new configuration, no new dependency, and no change to when the assistant answers or stays
silent.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The addressed case threads onto the asker: a turn where one member addressed the
  assistant among several speakers delivers with THAT member's origin as the target —
  pinned on a history where the addressed message is neither the newest nor the dispatch
  anchor, since both are what revision 1 got wrong.
- **AC3** Ambiguity and absence both send plainly: no addressed message, and two addressed
  messages in one turn, each deliver the answer with no target — pinned, and neither yields
  silence.
- **AC4** Helpful mode never threads: an unaddressed message answered in helpful mode
  delivers plainly — pinned in that mode specifically.
- **AC5** The report shape is never threaded: an answer whose prose contains the moderation
  command lead delivers plainly, with its text unchanged — pinned, and the text pinned
  byte-for-byte so the guard cannot become sanitation.
- **AC6** A refused threaded send retries plainly and delivers: pinned at the adapter with a
  refusal that is not "message not found", proving the answer arrives rather than being
  dropped, and proving exactly one retry.
- **AC7** Reports are untouched: existing report threading and undeliverable pins pass
  unchanged.
- **AC8** The records match the code: decisions 0018 and 0059 carry their amendments with
  this date and reason, decision 0046's Gate-2 reasoning is annotated with the guard that
  now preserves it, and the `OutboundReply::reply_target` doc and the `outbound.rs` comment
  no longer assert that answers stay unthreaded. Three existing pins asserting "the answer
  stays unthreaded" (`core/tests/spine/report.rs:407`, `adapters/telegram/tests/adapter/
  report.rs:74,178`) are rewritten to the new rule rather than deleted.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-replies`, branch
  `unit/threaded-replies`). Sites: `core/src/outbound.rs` (the delivery loop and
  `deliverable_of` — note the latter is pure over ONE block and has no ledger, so the
  lookup belongs in the caller that already holds `list_blocks`),
  `core/src/tools/provenance.rs:107` (`co_summoners`, the live anchor-to-rows walk),
  `core/src/tools/report.rs:205` (the command lead the guard tests against),
  `adapters/telegram/src/client.rs:412` and `driver.rs:747` (the refusal retry).
- The probe's findings are the grounding above; do not re-derive them. Its full report
  named more: `reply_to_assistant` carries no id and would render no link (moot now that
  the projection half is dropped), direct chats are unscoped (a 1:1 has one other
  participant, so the addressed rule threads there too — acceptable, and say so if it
  reads oddly), and the synchronous ingest replies (`/privacy`, the acknowledgment) stay
  plain because they never touch `OutboundReply`.
