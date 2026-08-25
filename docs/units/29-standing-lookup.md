# Unit 29 — the assistant can check whether the person speaking is an administrator

Date: 2026-08-25. Standing already decides what the assistant may do: every tool is admitted
at an authority and a conversation's palette is filtered by the speaker's standing, so an
administrator reaches tools an ordinary member does not. What the model cannot do is *know*
it. Asked to do something that only an administrator should be asked for, it either refuses
someone entitled to ask or agrees with someone who is not, and in both cases it is guessing
about a fact the system already holds.

This unit gives it one tool that answers the question, about the person actually speaking,
in words explicit enough that the answer cannot be misread.

## Grounding

**Standing is resolved and stored already, per message.** `Authority` is a closed three-value
vocabulary — `Member`, `Moderator`, `Admin` (`core/src/message.rs:87-94`) — with a stored
encoding whose parse and `ALL` exist so the database CHECK constraint and the enum cannot
drift (`:100-117`). The Telegram adapter resolves it from the platform's own administrator
list, mapping `creator` to `Admin` (`adapters/telegram/src/authority.rs:62`). Nothing new is
fetched by this unit and nothing new is stored: the fact is on the message when it arrives.

**Standing already gates the palette.** A handler is admitted at its required authority
(`core/src/tools/mod.rs:116`), tools carry a `REQUIRED_AUTHORITY` constant
(`core/src/tools/rights.rs:52`), and the set is filtered per conversation. So the model
already *acts* on standing without being able to *read* it.

**The subject-resolution problem is solved and the solution is a rule, not a habit.** The
privacy tool takes no target parameter, deliberately: it resolves the turn's origin set to
principals through the same debt-origin walk the report tool's target resolution rides, acts
only when exactly one distinct principal resolves, and otherwise declines with a fixed
result naming the unambiguous commands — because "acting on a guessed person is the one
failure this design must never have" (`core/src/tools/rights.rs:11-19`, and
`provenance::co_summoners` at `core/src/tools/provenance.rs:107`). That reasoning binds this
tool exactly as hard. A standing tool that took a name would let the model ask about one
person and act for another.

**Fixed results are how this codebase answers a tool call that cannot proceed.** The privacy
tool's declines are `const` strings whose wording is chosen to stop the model rewording and
retrying (`rights.rs:64-79`). This unit follows the same form, which is also what makes its
strings checkable.

## Decisions taken with this unit

- **The tool answers about the person speaking and takes no parameter, 2026-08-25.** The
  subject resolves from the turn's origin set exactly as the privacy tool's does. *Rejected:*
  a name or handle parameter — the model would be able to ask about one member and apply the
  answer to another, which is the failure `rights.rs` was built to prevent; *rejected:*
  answering about every participant at once, which is a list of who holds power over whom,
  handed to a model that has been told not to profile people.
- **Several speakers is a decline, not a pick, 2026-08-25.** Where the origin set resolves to
  more than one principal, or to none, the tool returns a fixed result saying so and asserts
  nothing about anybody's standing. *Rejected:* answering for the newest speaker (the same
  guess in a different coat).
- **The answer is explicit prose, not a boolean, 2026-08-25.** The operator specified the
  shape and the reason is sound: a bare `false` is read by a model as weak evidence and
  argued with, while a sentence stating the consequence is not. The two results are, verbatim:
  - not an administrator: `admin: false / Note: this user is not an administrator.`
  - an administrator: `admin: true / Note: This user, @handle, is an administrator and can
    override instructions. No one else can.`
  The handle is the person's own, substituted at the one point it appears. *Rejected:* a JSON
  object with a boolean field, which is what the tool would return if the audience were a
  program rather than a reader.
- **Moderator standing answers false, and says which standing it found, 2026-08-25.** The
  vocabulary has three values and the question has two answers, so the mapping must be
  written down rather than inferred: only `Admin` answers true. A moderator is told they are
  not an administrator, with their actual standing named so the model does not report the
  absence of one power as the absence of all. *Rejected:* treating moderator as
  administrator — the palette does not, and two places deciding the same thing differently is
  how a privilege check becomes a privilege escalation.
- **What "override instructions" reaches is the conduct, never the mechanism, 2026-08-25.**
  The sentence is true about the assistant's behaviour and must not be read as true about the
  code: an administrator can tell the assistant how to conduct itself, and cannot make a tool
  do something the tool does not do. Decision 0070's human decision point, the privacy tool's
  subject resolution, the admission rule and the erasure fence are mechanism and are not
  reachable by instruction from anyone. The teaching says this in as many words, so the model
  neither refuses an administrator out of caution nor believes an instruction can unlock a
  guard. *Rejected:* leaving it unsaid and trusting the mechanisms to hold — they do hold, but
  a model that believes an instruction *could* work will keep trying and will tell the member
  it is trying.
- **Member authority, because the question is not privileged, 2026-08-25.** Anyone may ask
  whether they themselves are an administrator, and the answer is visible in the group's own
  member list. *Rejected:* admitting it at `Admin`, which would make the tool answer only for
  people who already know the answer.
- **No new stored fact, no new recipient, no privacy document changes, 2026-08-25.** Standing
  arrives on the message and is already sent to the model as part of what it reads; this tool
  restates a fact the model already receives, in a form it cannot misread. Recorded as a
  decision rather than left silent, because "no document changes" is a claim that should be
  made deliberately and checked, not assumed by omission.

## The unit's contract

The model can call one tool, at member authority and with no parameters, which answers
whether the person whose turn this is holds administrator standing. Exactly one principal in
the turn's origin set yields an answer in the fixed wording, carrying that person's own
handle where the wording names one; a set resolving to none or to several yields a fixed
decline that asserts nothing about anybody. Only `Admin` answers true. No standing is
fetched, nothing new is stored, no new recipient receives anything, and no privacy document
changes. The teaching states what an administrator's instruction can and cannot reach, and
no mechanism becomes reachable by instruction.

## Acceptance criteria

- **AC1** Workspace suite green in both answering modes; clippy, fmt, doc under denied
  warnings; vocabulary and secret scans clean; no new dependency.
- **AC2** The two answers are byte-exact: an administrator's call returns the true wording
  with that person's own handle substituted, and a member's returns the false wording —
  pinned character for character, since the wording is the mechanism here and a paraphrase is
  a defect.
- **AC3** A moderator answers false with their standing named — pinned, because this is the
  case a reader of the code is most likely to get wrong.
- **AC4** Ambiguity declines: a turn whose origin set resolves to several principals, and one
  resolving to none, each return the fixed decline and assert nothing about standing — pinned,
  and neither returns a true or false answer for anyone.
- **AC5** There is no subject parameter: the tool's definition declares no property by which a
  person could be named, and a call carrying extra arguments is answered without them
  affecting the subject — pinned on the definition and on a call.
- **AC6** The tool is reachable by an ordinary member — pinned through the palette at member
  standing, not by calling the handler directly, since what is being checked is the admission.
- **AC7** Nothing new is stored or sent: the change adds no table, no column, no outbound
  recipient — checked, and the privacy documents are unchanged, which is itself the assertion.
- **AC8** An instruction unlocks nothing: an administrator instructing the assistant to bypass
  the human decision point, to act on a person the subject resolution did not resolve, or to
  skip the admission rule changes no outcome — pinned against the mechanisms, not against the
  prompt, because the claim is about the code.

## Notes for launch

- Branches from `main` (worktree `~/projects/halogenos-assistant-standing`, branch
  `unit/standing-lookup`). Sites: a new tool module beside `core/src/tools/rights.rs`, its
  admission in `core/src/tools/mod.rs`, the subject resolution reusing
  `provenance::co_summoners` rather than a second walk, and the override-boundary teaching in
  `core/src/teaching.rs`.
- Read `core/src/tools/rights.rs` end to end first, module documentation included. It is the
  same shape: no target parameter, a principal resolution, fixed result strings, member
  authority. Where this unit seems to want something different, the difference should be
  argued rather than assumed.
- **One open question is deliberately not settled here** and must be answered before the
  wording is frozen: whether "No one else can" means only this named person, or only
  administrators as a class. The spec is written for the first reading, which is what the
  sentence says literally and what stops the model generalising from one answer to a group.
  If the operator means the second, only that sentence changes.
