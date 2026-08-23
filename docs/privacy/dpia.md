# Data protection impact assessment: the halogenOS Group Assistant

**Draft, not yet published.**

Date: 2026-08-23

Assessment under Article 35 GDPR for the assistant described in this repository, in the
state it is in on the date above. It records the processing, the reasons it is necessary,
what it can do to the people in the groups, which shipped mechanism answers each of those
risks, what remains, and when this document has to be looked at again.

Controller: Simão Gomes Viana, c/o IP-Management #10911, Ludwig-Erhard-Str. 18, 20459
Hamburg, Germany. Enquiries: privacy@halogenos.org. No data protection officer is
appointed; the thresholds in § 38 BDSG are not met.

## 1. Why this assessment exists

The German data protection authorities' list of processing operations that always require
an assessment (the DSK list under Article 35(4) GDPR) names, at entry 11, the use of
artificial intelligence to process personal data in order to steer the interaction with
the people concerned, with the express example of a system that interacts with people
through conversation. The assistant is exactly that: a conversational system, in groups of
natural persons, processing what they write.

Two of the WP248 criteria apply independently: the processing is systematic (every message
in the group, continuously), and it uses a technology whose behavior is not fully
predictable (a large language model). The assessment is therefore treated as mandatory, not
as a precaution.

## 2. Scope and the people concerned

**In scope.** The assistant in the halogenOS community groups on the platforms it supports
(Telegram today), plus direct chats between a person and the assistant.

**The people concerned.** Members of those groups whose messages the assistant stores,
whether they address it or not, and people who write to it directly. Membership is open,
so the set is not enumerable in advance and includes people who never interact with the
assistant at all. Children are not excluded from the community groups by any mechanism the
project controls, which raises the standard the notice and the deletion path have to meet.

**Out of scope.** The chat platform's own processing, which the platform runs as its own
controller, and anything the project does outside the assistant.

## 3. The processing, described

### 3.1 Purposes

1. Answering community questions in the project's groups.
2. Reading a conversation in context, including older discussion, so an answer follows the
   thread instead of the last line.
3. Keeping the assistant available: two counters bound how much it answers per person and
   per chat.

### 3.2 Categories of data

- **Message content.** Text, including the caption of a media message. No media, no files,
  no voice, no stickers (decision 0017). Edited versions are not collected; the message
  stands as first seen.
- **Identity.** The platform's opaque account identifier, display name, username. Held in
  tables of their own and never inline in the ledger (decisions 0003, 0006). The username
  is transmitted to the processor with the conversation, by the operator's decision of
  2026-08-23, so the assistant can address people by the handle the group uses. The
  account identifier and the display name stay on the machine.

  > Amended 2026-08-23: the username projection shipped. Until this note the sentence
  > above described the operator's decision ahead of the running system, the harmless
  > direction; it now describes what every request carries. The handle transmitted is
  > the one stored with the message at receipt — a person who later changes their
  > handle is projected under the handle they spoke with, and a person the platform
  > gives no handle is projected with no identifier at all.
- **Circumstance.** Arrival time, the platform send time, the reply reference, whether the
  message was addressed to the assistant, and the authority the person held in that chat
  at that moment (member or administrator).
- **Group facts.** The channel title and the pinned rules text, stored as context notes
  (decision 0047).
- **Derived state.** Conversation membership, the answering counters, the tool palette,
  the group authorization row.
- **Special categories, incidentally.** Free text in a technical community can still
  reveal health, belief, political opinion or sexual orientation in passing. The
  assistant does not seek such data and cannot detect it. Posting in an open group does
  not by itself make that data manifestly public under Article 9(2)(e), so this is
  treated as a real exposure and answered in section 6.

### 3.3 Collection

The platform delivers group updates to the assistant. Recording every message requires the
bot's platform privacy mode to be off or the assistant to be an administrator, which is
deployment configuration, stated as such in the operator reference. Anonymous
administrator posts and automatic channel forwards are skipped, because the platform hides
the real author and a stand-in sender would mint a shared identity that erasure could
never scope correctly (decision 0016).

A group only reaches the ledger if the project operator added the assistant to it. The
authorization is a stored fact, checked before anything is written, and every other path
fails closed and withdraws (decision 0052).

### 3.4 The flow

1. The adapter translates a platform update into the core's message model and hands over
   the sender's external identity, never a resolved internal identifier (decision 0006).
2. The core resolves the person to an internal identifier, refreshing the display fields,
   and appends the message as a block whose content row holds the text.
3. Addressing is decided at the write: a direct message, a mention, or a reply to the
   assistant opens an answer debt (decision 0021). Everything else is stored and left
   unanswered.
4. If a debt is open and the counters allow it, the conversation is projected into a
   request: the system prompt, the group's context notes in the system voice, and the
   conversation's messages in order. The request goes to the provider.
5. The model may call one of the project lookups. Admission is decided by the
   conversation's recorded tool palette and fails closed (decision 0041).
6. The answer is stored as a block and sent back to the chat.

### 3.5 Recipients and transfers

- **Requesty Inc.**, the processor, under an Article 28 agreement carrying the EU standard
  contractual clauses. Requests enter through the European endpoint in Frankfurt, Germany
  (AWS eu-central-1) and are served on infrastructure in the EU. Zero data retention is
  enabled for the account: prompts and completions are not stored once the response is
  returned, and nothing is used for training. What the processor receives is the
  conversation's text and the public username of each speaker.
- **Sub-processors of the processor.** Requesty engages the model providers it routes to
  as its own sub-processors, under its own agreements, and stays answerable for them.
  Google is the sub-processor serving the model in use today. The model is not pinned in
  this assessment, because the processor's obligations, not the model's identity, are what
  the assessment rests on; a model change inside the EU region changes no conclusion here.
- **Public project sources.** The halogenOS forge for a commit lookup and the builds
  repository's public interface for a release lookup. Queries carry a repository name and
  a reference or tag.
- **The platform.** An independent controller for its own delivery and storage, not a
  processor of ours.

No transfer to a third country is intended. Requests, storage and inference stay in
Germany and the EU.

### 3.6 Storage and deletion

Message history has no expiry timer (decision 0003), for the reason set out in section 4.
Deletion on request is one operation with three steps (decisions 0011, 0012): the person's
message text, send time and reply reference are emptied in every conversation; their
direct conversations are removed whole, mappings included; their identity rows are
deleted. Block structure is untouched, so nothing is orphaned. An emptied message projects
a fixed marker in its own voice and never a word of the person's prose (decision 0027).
Deletion waits for an open model stream on the affected conversation, confirms the
settlement by re-reading stored state, and fails loudly without deleting anything past a
bounded wait (decision 0028).

Three record types are not reached by that operation today: lookup call records, which
live in framework-owned tables (decision 0045), context notes, which carry no person
reference at all (decision 0055), and the assistant's own answer blocks, which can quote
an erased person's words and, since the speaker projection shipped, repeat their handle
(noted 2026-08-23; the gap predates the projection, which widened its content by one
field). All are recorded as open, all wait on the same storage framework seam, and the
first is named in the public policy; the answer-block gap joins R6.

### 3.7 Measures in place

- Personal data in tables of its own, referenced by key, so append-only storage and
  deletion coexist (decisions 0003, 0012).
- Identity minimised at the provider boundary: one identifier is transmitted, the public
  username, decided by the operator on 2026-08-23 so the assistant can address people by
  the handle the group already uses. The display name and the numeric account identifier
  stay on the machine, and no other attribute of a person is attached to a request.
- The provider key is held in memory and never written to storage (decision 0020).
- Answering counters per person and per chat, limiting answers and never storage; an
  over-limit message draws no reply and no notice, because a refusal notice would hand a
  flooder the assistant's voice (decisions 0030, 0034).
- Group admission authorized persistently and fail-closed, with withdrawal on any
  unauthorized contact (decision 0052).
- The lookup palette recorded per conversation, admission fail-closed, tool authority
  floored at member level, lookup failures reported to the model and never to the chat
  (decisions 0041, 0044).
- Byte bounds on both system-voice surfaces the group controls, the rules text and the
  title, with an over-bound text refused whole and never truncated (decisions 0048, 0049).
- The privacy pointer answered deterministically, without a model turn, at most once per
  chat per window (decision 0053).
- No moderation capability ships: the warn, report and ban lines are held out of the
  system prompt until their mechanisms exist (decision 0046).

  > Amended 2026-08-23: the report mechanism now exists and its line returned to the
  > prompt; the warn and ban lines stay held. The addendum in section 12 assesses the
  > report and the wiki fetch this unit added — the review trigger "any moderation
  > capability shipping" fired and is answered there.

## 4. Necessity and proportionality

**Is storing every message necessary?** Purposes 1 and 2 cannot be served by storing only
the messages that mention the assistant. A question like "is that the same problem as last
week?" is answerable only against the surrounding conversation, and the community's own
questions routinely point backwards. The alternative designs were considered: answering
from the last few messages held in memory drops exactly the older context that makes the
assistant more useful than a search box, and storing only addressed messages produces
answers that misread their own thread. Storage is therefore tied to purpose 2, not
incidental to it. The platform's developer terms allow storing what the service needs to
function and forbid collection beyond that, in particular the building of datasets and
models from group content: nothing here is used to train anything, and the project keeps
the material for its own conversations only.

**Is keeping it without a timer proportionate?** The counter-design is a retention window.
It was rejected in decision 0003 and the reasoning holds under this assessment: a window
deletes the entire community's history on a schedule to reach the part one person wanted
removed, it destroys the long memory that is purpose 2, and it leaves the deletion
mechanism necessary anyway. What makes the absence of a timer proportionate is the pairing
with the rest: separation of personal data, deletion that reaches the prose, a single
identifier crossing to the processor and nothing more, no profiling, no action taken
against anybody. The proportionality rests on
that pairing, not on the storage alone. If any part of the pairing were removed, this
conclusion would have to be taken again.

**Is the amount minimal?** Text only, no media, no edits, no anonymous stand-in senders.
Identity is one opaque account identifier plus the display fields the platform already
shows to everyone in the group. The provider receives the conversation's text and one of
those fields, the public username, which the group sees on every message anyway; the
identifier and the display name do not cross. Lookups carry repository references. Nothing is collected for a purpose beyond
the three named.

**Is the legal basis right?** Legitimate interest under Article 6(1)(f), assessed
separately in the legitimate-interest assessment beside this document. Consent was
rejected: consent required to enter a group is not freely given, it cannot be withdrawn
in a way the group conversation could honor, and it would not cover the members who joined
before the assistant did.

## 5. Views of the people concerned

Article 35(9) asks for the views of the people concerned where appropriate. The
consultation surface is the community itself: the group's pinned rules announce the
assistant and carry the pointer to the policy, the privacy command answers with the same
pointer on demand, and members can object in the group in front of everyone, or privately
by mail. Objections and their handling are recorded and feed the review triggers in
section 10. No formal survey is planned; a community group that dislikes the assistant
says so loudly and immediately, and that signal is treated as the consultation.

## 6. Risks to the people concerned

| # | Risk | Severity before mitigation | Likelihood |
|---|------|---------------------------|------------|
| R1 | Everything said in the group is kept without an end date, so a remark made in passing stays available years later | Medium | Certain by design |
| R2 | Special-category content appears in ordinary conversation and is stored and transmitted with the rest | High | Occasional |
| R3 | Conversation text, together with each speaker's public username, is exposed to an external processor and its sub-processor | Medium | Every answer |
| R4 | What crosses to the processor is attributable to a named account, so it is pseudonymous at best and not anonymous | Medium | Every answer |
| R5 | Whoever holds the group's pin right steers the assistant's system voice, possibly against a member | Medium | Rare |
| R6 | Deletion is promised but three record types are not reached | Medium | Rare: a query, a rules text or an assistant answer quoting a person |
| R7 | The model writes something wrong or harmful about a member, in the group | Medium | Occasional |
| R8 | The store is compromised and a full community history is taken at once | High | Rare |
| R9 | The assistant appears in a group whose members never expected it | Medium | Rare |
| R10 | A direct chat, more personal than a group post, is stored the same way | Medium | Occasional |
| R11 | The assistant's answering capacity is exhausted by one flooder, or used to amplify one | Low | Occasional |

## 7. Mitigations, mapped to what ships

**R1, unbounded history.** Separation of personal data from the ledger (0003, 0006) plus
deletion that empties the prose, the send time and the reply reference and removes direct
conversations whole (0011, 0012), so the person, not the calendar, decides what
disappears. The notice states the absence of a timer plainly instead of hiding it behind a
vague "as long as necessary". Storage never leaves the project's own server; there is no
search interface, no export, no analysis over the history.

**R2, special categories.** Nothing detects such content, so the mitigation is structural:
the recipient stores nothing (zero data retention), so an accidental disclosure that
travels to the processor leaves no record there once the answer is returned, the assistant
builds no profile of anybody, and deletion on request
reaches the prose completely and fast. A person who realizes they revealed something can
have it gone within the month, in practice within days. Residual exposure is accepted and
named.

**R3, provider exposure.** The mitigation is contractual and territorial, not technical: an
Article 28 agreement with standard contractual clauses, the European entry point in
Frankfurt, serving inside the EU, zero data retention at the processor and at its
sub-processor, and no training on the content. The processor stays answerable for the
sub-processors it engages. What crosses is the conversation's text and the public username
of each speaker, which is exactly what any member of the group sees.

**R4, attribution at the processor.** The operator decided on 2026-08-23 that the assistant
addresses people by their handle, which means the username travels with the conversation.
This document does not describe the transmitted data as anonymous or pseudonymous in any
protective sense: a public username identifies an account, and the text beside it is
attributable to that account for as long as the request exists. What limits the risk is
that the request does not persist. Zero data retention means the attributable record is
gone once the answer is returned, no profile is accumulated on the other side, the numeric
account identifier and the display name never cross, and the data itself is what the person
already published to a group. The capability bought with it, an assistant that can answer
"@handle, that setting moved", is the reason the operator accepted the exchange, and the
public policy states the transmission plainly instead of implying anonymity.

**R5, system-voice steering.** Accepted with reasoning in decision 0049: the group
governing its own assistant is the point of the feature, and pinning is an administrator
right in the target groups. The surface is bounded by byte limits on both the rules text
and the title, with over-bound text refused whole instead of truncated (0048, 0049), and
the trust boundary is written down in the operator reference. The assistant can take no
action against a member, so a steered system voice can produce bad prose, not
consequences.

**R6, deletion gaps.** Both are recorded as open decisions (0045, 0055), both are named in
the public policy in plain words, and both wait on one storage framework seam that is on
the framework improvements list. Content in the unreached records is technical in the
lookup case and governance prose in the note case. Meanwhile a note is superseded whenever
the group's rules are pinned anew.

**R7, wrong output.** The system prompt is the maintainer's, with the moderation lines
held out until their mechanisms exist (0046), so the assistant cannot act on a wrong
judgment. Answering counters bound the volume. The policy tells readers the answers are
model-written and can be wrong. A failed turn tells the chat once (0025) instead of
retrying into noise.

**R8, compromise.** The provider key lives in memory only and is never stored (0020).
Secrets are referenced indirectly in configuration, never written into it. Storage
protection at rest is deployment configuration and is tracked as an open item in
section 11, because the platform's developer terms require it and this assessment cannot
claim it while it is unproven.

**R9, unexpected groups.** Group admission is a stored authorization written only by an
invitation from the configured operator; every other contact is refused without touching
the ledger and the assistant withdraws (0052). The rules pin announces the assistant in
the groups it does serve.

**R10, direct chats.** Deletion removes a direct conversation whole, mappings included,
because a two-party chat that lost its human is metadata that still identifies the person
(0011, 0012). Direct chats are never used to answer in a group: conversations do not cross.

**R11, capacity abuse.** Two counters, per person across all chats and per chat, limit
answering and never storage; an over-limit message draws silence, so the assistant's voice
cannot be borrowed by a flooder (0030, 0034).

## 8. Residual risk

| # | Residual | Reasoning |
|---|----------|-----------|
| R1 | Low to medium | The absence of a timer is real and stays real; it is answered by deletion on request, announced transparency, and the absence of any secondary use. |
| R2 | Medium | Accepted. No mechanism can detect what a person reveals in passing, and since 2026-08-23 such a disclosure travels with the speaker's handle attached. Zero data retention and fast erasure are what is available. |
| R3 | Low to medium | Contract, EU territory, zero retention, no training. Raised from low on 2026-08-23: the transmitted set now includes a public identifier. |
| R4 | Medium | Accepted by the operator on 2026-08-23 for the mention capability. Not solvable while the capability exists; bounded by the processor retaining nothing and by the identifier being the one the group already sees. |
| R5 | Low | Bounded surface, no action capability. |
| R6 | Low to medium | Narrow content, named openly, one framework seam away from closed. |
| R7 | Low | No action capability; bounded volume; stated plainly to readers. |
| R8 | Medium until storage protection at rest is confirmed, then low | The one control this assessment cannot yet call shipped. |
| R9 | Low | Fail-closed authorization, healed on every later contact. |
| R10 | Low | Whole-conversation removal on deletion. |
| R11 | Low | Counters bound answering; silence over refusal notices. |

**Overall judgment.** With the mitigations above in place, the residual risk to the people
concerned is not high within the meaning of Article 36(1), and prior consultation with the
supervisory authority is not required. This judgment depends on two conditions that are
not yet fully in place, listed in section 11; if either fails, the judgment is re-taken
before the assistant enters the main community group.

## 9. Open items this assessment depends on

1. **Storage protection at rest** for the message store, required by the platform's
   developer terms and relied on by the R8 mitigation. Deployment configuration, not
   repository work. Must be in place before the main group.
2. **The countersigned processor agreement** with Requesty Inc. returned and on file. The
   terms are accepted and the clauses apply; the signature round-trip is outstanding and
   must be complete before the main group.
3. **Records of processing** under Article 30, a short document for one system, not yet
   written.
4. **The two deletion gaps** (decisions 0045, 0055), waiting on the storage framework.

## 10. When this assessment is taken again

Any one of these triggers a review, and none of them is optional:

- A new platform adapter, or the assistant entering a group beyond the project's own.
- A change of processor, model provider, endpoint region or retention setting, including
  anything that would move inference outside the EU.
- Any new path that sends message content off the machine, including a new tool, and any
  change to which identifiers travel with a request.
- A change to what is collected: media, edits, reactions, membership events.
- Any moderation capability shipping, in particular the held-out warn, report and ban
  lines.
- A change to retention, or the introduction of any secondary use of the history.
- Closure of either deletion gap, or discovery of a third.
- A personal data breach, or a near miss.
- A complaint or an objection that this assessment did not anticipate.
- New guidance from the German authorities or the EDPB on conversational AI, or a
  decision that changes the legitimate-interest analysis.
- Twelve months since this date, with no other trigger.

## 12. Addendum, 2026-08-23: the report and the wiki fetch

The review trigger "any moderation capability shipping" fired with the report
feature, and this addendum is the assessment it demands; the wiki fetch shipped in
the same unit and is covered with it. This document is a draft amended in place, per
its own status line.

**The report, described.** A member replies to an offending message and asks the
assistant for a report. The assistant resolves the reported message from that reply —
never from its own judgment or the model's choice — and appends a report record
carrying the reported message's platform identifier, the reported sender's internal
identifier, and a fixed command line naming the group's moderation bot. The line is
then sent into the group as a reply to the reported message, where the moderation
bot forwards it to the group's administrators. The assistant decides nothing: no
detection, no scoring, no automatic filing — a person asks, a person judges.

**What it changes for the people concerned.**

- *A new disclosure path.* The report event — "this message was reported" — becomes
  visible to the group's administrators through the moderation bot. The message
  itself was already visible to them; what is new is the reported-ness. The event
  carries the message's identifier, no prose and no names from the assistant's side.
- *A new stored identifier.* The reported message's identifier and its sender's
  internal identifier are stored on the report record, precisely so erasure can
  reach it: the reported person's deletion empties the record's message reference,
  and an emptied report is never sent. The reporter's own reply reference sits on
  their message row and is emptied by their deletion like the rest of their row.
- *Abuse of the path.* A member could ask for reports in bad faith. The bound is
  one filed report per group per report window, each one costs the asker an answer
  from their own answering budget, and the human administrators behind the
  moderation bot are the judgment — the assistant amplifies a member's ask to the
  admins, nothing more.

**The wiki fetch.** A wiki question makes the assistant fetch one page of the
project's public wiki. The request carries a page name and nothing about any person;
responses are cached in memory for five minutes. This is the same shape as the
existing forge and mirror lookups — a public project source, not a recipient of
personal data — and section 3.5 is amended by naming it there in spirit; the
records-of-processing recipient table carries the row.

**Judgment.** The report is human-initiated, human-judged, bounded, and erasable;
the wiki fetch carries no personal data. The overall judgment of section 8 stands:
the residual risk is not high in the meaning of Article 36(1). The warn and ban
lines remain held out, and their shipping remains a review trigger.
