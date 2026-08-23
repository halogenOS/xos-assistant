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
  tables of their own and never inline in the ledger (decisions 0003, 0006).
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

- **Requesty Inc.**, processor under an Article 28 agreement carrying the EU standard
  contractual clauses. Requests enter through the European endpoint in Frankfurt, Germany
  (AWS eu-central-1). Zero data retention is enabled for the account: prompts and
  completions are not stored after the response, by Requesty or by the model provider.
- **The model provider.** Google Gemini served on Google Vertex AI, pinned to European
  serving through the configured model name, so inference does not leave the EU.
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

Two record types are not reached by that operation today: lookup call records, which live
in framework-owned tables (decision 0045), and context notes, which carry no person
reference at all (decision 0055). Both are recorded as open, both wait on the same storage
framework seam, and both are named in the public policy.

### 3.7 Measures in place

- Personal data in tables of its own, referenced by key, so append-only storage and
  deletion coexist (decisions 0003, 0012).
- Identity kept out of provider requests: no display name, no username, no account
  identifier is sent. The request carries the conversation's words without names; a
  neutral per-conversation speaker label is a settled design improvement, not yet
  built, and its arrival changes nothing the provider learns.
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
with the rest: separation of personal data, deletion that reaches the prose, no identity
sent outside, no profiling, no action taken against anybody. The proportionality rests on
that pairing, not on the storage alone. If any part of the pairing were removed, this
conclusion would have to be taken again.

**Is the amount minimal?** Text only, no media, no edits, no anonymous stand-in senders.
Identity is one opaque account identifier plus the display fields the platform already
shows to everyone in the group. The provider receives conversation text with the identity
removed. Lookups carry repository references. Nothing is collected for a purpose beyond
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
| R3 | Conversation text is exposed to an external processor and a model provider | Medium | Every answer |
| R4 | A person is re-identified from the content of a message even though identifiers were removed before sending | Medium | Occasional |
| R5 | Whoever holds the group's pin right steers the assistant's system voice, possibly against a member | Medium | Rare |
| R6 | Deletion is promised but two record types are not reached | Medium | Rare, and only where a query or a rules text quotes a person |
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
the content is not sent with any identity attached, the recipient stores nothing (zero
data retention), the assistant builds no profile of anybody, and deletion on request
reaches the prose completely and fast. A person who realizes they revealed something can
have it gone within the month, in practice within days. Residual exposure is accepted and
named.

**R3, provider exposure.** The processor chain is contractual and territorial: an
Article 28 agreement with standard contractual clauses, the European entry point in
Frankfurt, EU-pinned inference, zero data retention on both hops, and no training on the
content. What crosses is conversation text without names, usernames or account
identifiers.

**R4, re-identification.** Removing identifiers is not anonymization, and this document
does not claim it is. The assessment claims something narrower and true: the recipient is
contractually bound, retains nothing, and receives no key that links the text to an
account. The residual case is a person who identifies themselves inside their own message,
which no mechanism can undo.

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
| R2 | Medium | Accepted. No mechanism can detect what a person reveals in passing. Deletion and non-transmission of identity are what is available. |
| R3 | Low | Contract, EU territory, zero retention, no training, no identity. |
| R4 | Low to medium | Structural, not solvable; bounded by the recipient retaining nothing. |
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
- Any new path that sends message content off the machine, including a new tool.
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
