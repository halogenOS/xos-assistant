# Data protection impact assessment: the halogenOS Group Assistant

**Draft, not yet published.**

Date: 2026-08-23

An assessment in the form Article 35(7) GDPR prescribes, for the assistant described in
this repository, in the state it is in on the date above. It is carried out as a
precaution; section 1 records the pre-check of whether Article 35 compels one at all. It
records the processing, the reasons it is necessary,
what it can do to the people in the groups, which shipped mechanism answers each of those
risks, what remains, and when this document has to be looked at again.

Controller: Simão Gomes Viana, c/o IP-Management #10911, Ludwig-Erhard-Str. 18, 20459
Hamburg, Germany. Enquiries: privacy@halogenos.org.

Data protection officer: none appointed, decided 2026-08-23 with the reasoning in
section 9. § 38(1) sentence 2 BDSG attaches the duty to processing that is objectively
subject to a data protection impact assessment, in the statute's word "unterliegen". It
does not attach to a controller's decision to write one. Two earlier versions of this note
were wrong in opposite directions: the first made the § 38 headcount thresholds decide a
question they do not reach, the second derived the duty from this document's own
description of itself, which reasons in a circle. Enquiries are answered at the address
above.

Competent supervisory authority: Bayerisches Landesamt für Datenschutzaufsicht (BayLDA),
Promenade 18, 91522 Ansbach. Settled by the operator 2026-08-23: the establishment is in
Bavaria, and the Hamburg address in the controller block is a mail-forwarding contact
address only, which does not determine competence.

## 1. Whether Article 35 compels this assessment: the pre-check

German guidance expects a controller to check whether an assessment is required and to
document that check whichever way it comes out. This section is that check, taken
2026-08-23, and it replaces an earlier version that declared the duty settled.

**What speaks for a duty.** Entry 11 of the German data protection authorities' list under
Article 35(4) GDPR names the use of artificial intelligence to process personal data in
order to steer the interaction with the people concerned. The entry carries no qualifying
rider of the kind entries 1 to 4 attach to their subjects, so on its face it applies
unconditionally, and its second example is a system that advises people through
conversation. A plain reading of that headline reaches an assistant that converses with
natural persons and processes what they write.

**What speaks against.** The example's own wording, processing a person's data "für deren
Beratung", contemplates a system that works on the data of the person it advises in order
to advise them. A general-knowledge assistant does that only in part: it answers questions
about a software project from public sources and from the group's own conversation, and
the personal data it holds is that conversation and not a file about the person asking.
Alongside that, the authority competent for this controller has published its own
checklist for chatbots built on large language models (the Bavarian authority's AI
checklist, 2024, still a consultation draft), and the DSK's 2025 guidance on artificial
intelligence routes a controller through a conditional check-and-document step for such
systems. Neither instrument applies entry 11 to a chatbot. A supervisory authority that
considered this exact class of system compulsorily listed would have said so in the
document it wrote for that class.

**How the list relates to the general test.** The EDPB's Opinion 5/2018 on the German list
construes a national list as further specifying Article 35(1), "which will prevail in any
case". The list is therefore read through the general high-risk test and not as a
mechanical trigger standing apart from it.

**The WP248 criteria, honestly counted.** The Article 29 Working Party's guidelines set out
nine criteria and treat two or more as an indication of high risk. An earlier version of
this section counted "the processing is systematic", which is not one of the nine. The
count as it actually stands:

- *Criterion 8, innovative use or applying new technological solutions.* Met. A large
  language model writing the answers is the case the guidelines describe.
- *Criterion 4, data of a highly personal nature.* Met for direct chats between one person
  and the assistant, which the guidelines reach by way of communications whose
  confidentiality a person may expect. Recorded 2026-08-23: direct chats are switched off
  entirely by configuration in the deployment, so the criterion is met by a capability
  that is not in use. Arguable and not claimed for public group posts, because a message
  typed into an open community group is not a communication anyone can expect to be
  confidential.
- *Criterion 7, data concerning vulnerable data subjects.* Arguable. The groups are open
  and the platform applies no age check, so children are among the people concerned and
  the guidelines name children expressly. Not claimed as settled, because the imbalance
  the criterion aims at, the employee or the patient who cannot walk away, is absent: no
  one depends on a community chat group.
- *Criterion 3, systematic monitoring.* Not claimed. The guidelines mean observing,
  monitoring or controlling data subjects. The assistant observes and controls nothing: it
  takes no action against anybody, attaches no consequence to what it reads, and watches
  for nothing in particular. Storing what a group says is not monitoring the people who
  said it.
- The remaining criteria are not met: no evaluation or scoring, no automated
  decision-making with legal or similar effect, no large-scale processing in the sense the
  guidelines describe, no matching or combining of datasets from separate operations, and
  nothing that prevents anybody from exercising a right or using a service.

One criterion is clearly met and two are arguable.

**Conclusion, taken by the controller 2026-08-23.** The pre-check does not produce a clear
duty. This assessment is carried out as a precaution, following the guidelines'
recommendation to carry one out where there is doubt, and not because Article 35 compels
it. The purpose behind the decision is the same one that shaped the system: risk as low as
it can be brought at a cost the project can carry. Two consequences are recorded where
they belong. Section 9 records that § 38(1) sentence 2 BDSG is therefore not triggered,
with the residual risk of the opposite reading named. Section 8's judgment rests on the
mitigations and not on that question.

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

  > Amended 2026-08-23, later the same day: the identity category shrinks. The display
  > name is no longer collected or stored at all (decision 0077) — the adapter stops
  > decoding it, the identity table's column is dropped with its values by an appended
  > migration step, and the category is the account identifier and the username. Every
  > sentence in this document saying the display name "stays on the machine" or "does
  > not cross" now holds trivially: there is no stored display name to stay anywhere.

  > Amended 2026-08-29: the join notice narrows that "trivially". A display name is
  > still not identity data and is still attached to no message; the identity table
  > still has no column for one. What changed is that a group's join announcement is
  > now recorded, and the name that announcement showed is stored as the announcement's
  > own content and rides the projected join line into a request — so exactly one
  > display name per recorded join does cross, as event content, erasable with the
  > person. Every sentence below saying the display name "does not cross" is to be
  > read as: no display name crosses attached to a person's message or identity.
- **Circumstance.** Arrival time, the platform send time, the reply reference, whether the
  message was addressed to the assistant, and the authority the person held in that chat
  at that moment (member or administrator).
- **Join notices (added 2026-08-29).** One record per person a group's join announcement
  named: the name the platform displayed for them, their handle, their internal
  identifier, and the announcement's own identifier and send time. It is the one place a
  display name is stored, as the announcement's content rather than as identity data, and
  the person-keyed erasure empties it like message text. Not written for a person whose
  suppression flag stands.
- **Group facts.** The channel title and the pinned rules text, stored as context notes
  (decision 0047).
- **Derived state.** Conversation membership, the answering counters, the tool palette,
  the group authorization row.
- **Special categories, incidentally.** Free text in a technical community can still
  reveal health, belief, political opinion or sexual orientation in passing. The assistant
  does not seek such data and cannot detect it. Restated 2026-08-23 on a fact the operator
  settled, that the groups are readable by anyone on the platform without joining or
  approval: content a person posts about themselves in such a group is data they
  manifestly made public by their own deliberate act, and Article 9(2)(e) is claimed for
  it. Content one member reveals about another is not covered by any condition in
  Article 9(2), because the person concerned published nothing. That narrower residual is
  what R2 now carries, and the legitimate-interest assessment sets out both parts with the
  narrow-reading risk attached to the claim.

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

- **Requesty Ltd, London, United Kingdom**, the processor, under an Article 28 agreement
  carrying the EU standard contractual clauses (entity corrected 2026-08-23 from Requesty
  Inc.; the company is British, which makes the processor relationship itself a transfer,
  see below). Requests enter through its European endpoint, and what it stores it stores
  in Frankfurt, Germany (AWS eu-central-1). Zero data retention is configured for the
  account: Requesty writes no message and no answer to storage and uses none of it for
  training. It keeps billing telemetry that carries no content, meaning token counts, the
  model identifier and a timestamp. What the processor receives is the conversation's text
  and the public username of each speaker.
- **Sub-processors of the processor, in two layers.** The infrastructure Requesty runs the
  service on, Amazon Web Services in Frankfurt, for which Requesty stays answerable to us.
  And the model providers it routes to, a category rather than a name (generalised
  2026-08-27: the provider follows the model chosen, exactly as this assessment already
  declines to pin the model), for which Requesty answers for the
  choice, for the written terms and for reporting that provider's published position
  accurately, but not for that provider's own breach of it. Corrected 2026-08-23: zero
  data retention binds Requesty alone. Whether a model provider keeps a request or trains
  on it follows the terms of the model chosen, which makes the choice of model a data
  protection decision and not only a quality one. The individual model is still not pinned
  in this assessment, because what the assessment rests on is the processor's obligations,
  the region a deployment sits in and the terms of the chosen model.
- **Public project sources.** The halogenOS forge for a commit lookup and the builds
  repository's public interface for a release lookup. Queries carry a repository name and
  a reference or tag.
- **Serper, United Kingdom**, the search provider (added 2026-08-29, with the web
  search). Unlike the project sources above it IS a recipient of member-derived text:
  it receives the search query the assistant writes for a question that is not about
  the project, in words drawn from the conversation. It receives nothing else — no
  account identifier, no username, no other part of the conversation — and a query
  carrying a person reference in the handle form is refused whole before anything is
  sent. Its terms are governed by the law of the United Kingdom, and its privacy
  policy names the customer controller and itself processor where personal data is
  processed in the service. Section 14 assesses this addition.
- **The platform.** An independent controller for its own delivery and storage, not a
  processor of ours.

**Transfers outside the EU/EEA.** Rewritten 2026-08-23; the previous flat statement that
no transfer is intended was wrong. Data leaves the EEA in FOUR places (three until
2026-08-29, when the web search was added), each with its own basis:

1. The processor is a company in the United Kingdom, although it stores and serves in
   Frankfurt. The transfer rests on the European Commission's adequacy decision for the
   United Kingdom under Article 45 GDPR, so no further safeguard is required for it.
2. Where a model deployment sits outside the EEA, the request reaches it there. Those
   transfers rest on the standard contractual clauses the processor agreement carries,
   under Article 46(2)(c) GDPR.
3. The chat platform sits outside the EU/EEA and receives every message and every answer
   as part of delivering them, as its own controller under its own policy.
4. Added 2026-08-29: the search provider is a company in the United Kingdom and
   receives the search query there. The transfer rests on the same adequacy decision
   for the United Kingdom under Article 45 GDPR, so no further safeguard is required
   for it either.

**Defect found 2026-08-23, being closed.** The conversation-naming step, which sends a
short piece of a new conversation to a smaller model to get a few words naming it, ships
with a lightweight model hardcoded in the framework, and that deployment is outside the
EEA. It is a framework defect, not a configuration choice, and the fix is in flight: the
naming step will follow the same configured model as the answers. Until it merges, case 2
above is live on every new conversation, and no claim in this document that requests stay
in the EU may be read without it. The public policy carries the same qualification.

> Closed 2026-08-23: the conversation-naming feature is switched off entirely (decision
> 0077) — no surface reads a derived title, so the assembly disables the derivation at
> construction and no naming request goes out to any model, in any region, ever. The
> configured-model fix of decision 0068 shipped first and is now moot for this
> deployment. Case 2 above remains stated because the answering model's own deployment
> region follows the processor's routing; the naming step no longer contributes to it.
> Titles derived before the switch-off persist in upgraded stores as stored metadata;
> nothing new joins them.

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
  the handle the group already uses. The numeric account identifier stays on the machine,
  no display name is stored beside a message or attached to a request as identity data, and
  no other attribute of a person is attached to a request. Amended 2026-08-29: the one
  display name that reaches a request is the name a group's join announcement showed,
  carried as that announcement's own content in the projected join line, never as an
  attribute of a message's sender.
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

  > Amended 2026-08-24: the report became the assistant's own assessment — it judges
  > each group message against the pinned rules and reports a clear violation itself;
  > member-initiated reporting is removed. The trigger fired again and the addendum in
  > section 13 answers it. The warn and ban lines stay held; the
  > assistant-assesses-a-human-decides rule (decision 0070) binds the new path
  > structurally.

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
identifier does not cross, and a display name crosses only as the content of a recorded
join announcement (amended 2026-08-29). Lookups carry repository references. Nothing is collected for a purpose beyond
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

Added 2026-08-23, with the privacy-self-service unit: an objection to collection going
forward is now honored by machine, in place, the moment it is made — `/privacyout` (or
the person's plain-language ask, read by the model and enforced by the system) raises a
suppression flag that drops the person's new messages at ingestion before anything is
written, and deletion is commanded and confirmed by the person in the chat and then runs
by machine. This is a safeguard, not an Article 22 decision: the machine executes the
person's own decision and takes none of its own, the flag survives the person's deletion
so the objection cannot be forgotten, and the mailed path with its human answer remains
open beside it.

## 6. Risks to the people concerned

| # | Risk | Severity before mitigation | Likelihood |
|---|------|---------------------------|------------|
| R1 | Everything said in the group is kept without an end date, so a remark made in passing stays available years later | Medium | Certain by design |
| R2 | Special-category content about a person other than the poster appears in ordinary conversation and is stored and transmitted with the rest | Medium | Occasional |
| R3 | Conversation text, together with each speaker's public username, is exposed to an external processor and its sub-processor | Medium | Every answer |
| R4 | What crosses to the processor is attributable to a named account, so it is pseudonymous at best and not anonymous | Medium | Every answer |
| R5 | Whoever holds the group's pin right steers the assistant's system voice, possibly against a member | Medium | Rare |
| R6 | Deletion is promised but three record types are not reached | Medium | Rare: a query, a rules text or an assistant answer quoting a person |
| R7 | The model writes something wrong or harmful about a member, in the group | Medium | Occasional |
| R8 | The store is compromised and a full community history is taken at once | High | Rare |
| R9 | The assistant appears in a group whose members never expected it | Medium | Rare |
| R10 | A direct chat, more personal than a group post, is stored the same way | Medium | Occasional |
| R11 | The assistant's answering capacity is exhausted by one flooder, or used to amplify one | Low | Occasional |
| R12 | A member's own words, rewritten by the model into a search query, reach a further third party and are stored in a record erasure does not reach (added 2026-08-29, with the web search) | Medium | Whenever a question is not about the project |

## 7. Mitigations, mapped to what ships

**R1, unbounded history.** Separation of personal data from the ledger (0003, 0006) plus
deletion that empties the prose, the send time and the reply reference and removes direct
conversations whole (0011, 0012), so the person, not the calendar, decides what
disappears. The notice states the absence of a timer plainly instead of hiding it behind a
vague "as long as necessary". Storage never leaves the project's own server; there is no
search interface, no export, no analysis over the history.

**R2, special categories.** Re-scoped and re-rated 2026-08-23, when the operator settled
that the groups are readable by anyone on the platform without joining or approval. What a
person posts about themselves in such a group is data they manifestly made public, and
Article 9(2)(e) covers it, so the risk that remains is the narrower one in the table: what
one member reveals about another, which no condition in Article 9(2) reaches. Severity
therefore drops from high to medium, and the drop is scope and not comfort, because the
covered part left the risk instead of being mitigated inside it.

For the part that remains, the mitigation is structural, since nothing detects such
content. The exposure is incidental, unsought and undetectable, and it enters through a
third party's own act: publishing another person's special-category data into a public
group is that poster's processing and, where it is unlawful, that poster's wrong. The
assistant neither invites it nor benefits from it. What the controller owes is reactive and
is honored: the processor stores nothing, so a disclosure leaves no record at that layer
once the answer is returned, though a model provider behind it can keep the request under
the terms of the chosen model; no profile is built from any of it; erasure on request
reaches everything of the requesting person's own by mechanism, within the month and in
practice within days; a request about somebody else's message is answered by a person on
the same rights; and an objection is honored the same way today and by machine once the
self-service unit ships (which it did on 2026-08-23; section 5 records the shipped
path). The residual is minimised, not resolved, and it is named as such.

**R3, provider exposure.** The mitigation is contractual and territorial, not technical: an
Article 28 agreement with standard contractual clauses, the European entry point, storage
in Frankfurt, the United Kingdom's adequacy decision for the processor itself, and zero
data retention at the processor with no training on the content there. Corrected
2026-08-23: that retention promise reaches the processor and stops. A model provider's own
retention and training follow the terms of the model chosen, so the mitigation for the
provider layer is the choice of model and the region its deployment sits in, not a blanket
assurance. The processor agreement offers an Approved-Models restriction that would bind
that choice contractually; it is not configured today and is carried in section 9. What
crosses is the conversation's text and the public username of each speaker, which is
exactly what any member of the group sees.

**R4, attribution at the processor.** The operator decided on 2026-08-23 that the assistant
addresses people by their handle, which means the username travels with the conversation.
This document does not describe the transmitted data as anonymous or pseudonymous in any
protective sense: a public username identifies an account, and the text beside it is
attributable to that account for as long as the request exists. What limits the risk is
that the request does not persist at the processor. Zero data retention means the
attributable record is gone there once the answer is returned, and no profile is
accumulated on that layer, while a model provider's own retention follows the terms of the
chosen model. The numeric account identifier never crosses, and a display name crosses
only as the content of a recorded join announcement (amended 2026-08-29). The data itself
is what the person already published to a group. The capability bought with it, an assistant that can answer
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
protection at rest is deployment configuration; it was tracked as an open item while
unproven, and is closed as of 2026-08-24 — the store and the credentials sit on a LUKS
volume whose passphrase is entered at each boot and held nowhere on the machine.

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

**R12, member words in a search query.** Added 2026-08-29 with the web search. Four
mitigations ship with the capability and each is pinned by the suite. The query
guard refuses any query carrying a person reference in the handle form, whole,
before anything is sent, and its refusal does not echo what it matched — so a
deliberate identifier reaches neither the provider nor the tool record (decision
0115). The query is bounded to 400 characters and refused instead of truncated past
it, the pages to five, and each person's searches to five per ten minutes, which
bounds the volume of member-derived text that can cross at all (decisions 0112,
0117). What crosses is the query and nothing else: no account identifier, no
username, no other part of the conversation, which makes this boundary narrower than
the model provider's. And the recipient is a processor in an adequacy-covered
country. What is NOT mitigated is the record: the query lives on framework-owned
tables erasure does not reach, which is decision 0045's standing gap, amended the
same day to say that its accepted ground — inputs that are "overwhelmingly
technical" — no longer describes this input. That is stated, not closed.

## 8. Residual risk

| # | Residual | Reasoning |
|---|----------|-----------|
| R1 | Low to medium | The absence of a timer is real and stays real; it is answered by deletion on request, announced transparency, and the absence of any secondary use. |
| R2 | Low to medium | Re-rated 2026-08-23 with the narrower scope: self-posted content is covered by Article 9(2)(e), leaving what one member reveals about another. No mechanism can detect that, and since 2026-08-23 such a disclosure travels with the speaker's handle attached. Zero data retention at the processor, reactive remedies and fast erasure are what is available, and the model provider's own terms decide the rest. Rises again if the groups ever stop being publicly readable. |
| R3 | Low to medium | Contract, EU territory, zero retention, no training. Raised from low on 2026-08-23: the transmitted set now includes a public identifier. |
| R4 | Medium | Accepted by the operator on 2026-08-23 for the mention capability. Not solvable while the capability exists. Bounded by the processor retaining nothing, by the terms of the chosen model at the provider layer, and by the identifier being the one the group already sees. |
| R5 | Low | Bounded surface, no action capability. |
| R6 | Low to medium | Narrow content, named openly, one framework seam away from closed. |
| R7 | Low | No sanction capability, bounded volume, stated plainly to readers. Extended 2026-08-23 to the model-produced report relay: a misfire is public, administrator-judged and carries no consequence from the assistant. Extended 2026-08-24 to the autonomous assessment: a false positive is now the model's own misjudgment rather than a misread request, bounded the same way — public, administrator-judged, no consequence from the assistant, each message reported at most once — and by the configured reasoning level the judgment runs at. Section 13 carries the assessment. |
| R8 | Low | Storage protection at rest was the one control this assessment could not call shipped. Confirmed 2026-08-24: the deployment holds the store and the operator-provided credentials on an encrypted volume, so the rating stands at low rather than conditionally. |
| R9 | Low | Fail-closed authorization, healed on every later contact. |
| R10 | Low | Whole-conversation removal on deletion. |
| R11 | Low | Counters bound answering; silence over refusal notices. |
| R12 | Low to medium | Added 2026-08-29. What crosses is a query and nothing else, to a processor in an adequacy-covered country, under a per-person bound and a length bound, with deliberate person references refused before they are sent. The residual is what stays behind: the query is stored where erasure does not reach, and a query can carry a member's own words without naming anyone. It falls to low when decision 0045's framework seam closes. |

**Overall judgment.** With the mitigations above in place, the residual risk to the people
concerned is not high within the meaning of Article 36(1), and prior consultation with the
supervisory authority is not required. Restated 2026-08-23: the judgment depends on the
open items in section 9 being closed, meaning the countersigned agreement, storage
protection at rest, the Approved-Models restriction and the non-EEA naming model. If any of
them stays open, the judgment is re-taken before the assistant enters the main community
group. It does not depend on the officer question, which section 9 records as decided.
Amended 2026-08-23: the naming-model item is closed by decision 0077 — the derivation is
off, so the judgment now depends on the three remaining items.

## 9. Open items this assessment depends on

**Decided 2026-08-23, and therefore not an open item: no data protection officer is
appointed.** § 38(1) sentence 2 BDSG requires an appointment where a controller carries out
processing that is subject to a data protection impact assessment under Article 35, in the
statute's own word "unterliegen". The duty follows the processing being objectively subject
to an assessment, not the controller's decision to write one, so a precautionary assessment
does not create the duty by existing. Section 1's pre-check concludes that Article 35 does
not compel an assessment here, which leaves § 38(1) sentence 2 untriggered. The Baden-
Württemberg authority states the same connection in its negative form, that where no
assessment has to be carried out, no data protection officer has to be appointed either
("keine DSFA durchzuführen und daher auch kein Datenschutzbeauftragter zu bestellen ist").

The residual risk of the opposite reading is named openly: entry 11 of the
German list carries no qualifying rider, and a supervisory authority could read it as
covering this assistant. On that reading the assessment would be compelled and the
appointment would follow with it. Two things bound the exposure. The assessment the
authority would ask for already exists, because it was written as a precaution, and the
decision here is recorded with its reasoning and its date, which is what accountability
under Article 5(2) asks for. If the authority takes the opposite view, the appointment is
made and this entry is rewritten.

Recorded as context and not as reasoning: the federal government has committed to
introducing a bill repealing § 38(1) BDSG by the end of 2026. That is where the law may be
going. It is not why this decision was taken, and the decision would read the same without
it.

1. ~~**Storage protection at rest** for the message store~~ — **closed 2026-08-24**,
   required by the platform's developer terms and relied on by the R8 mitigation.
   Deployment configuration, not repository work, and now configured: the store and the
   operator-provided credentials sit on `/var`, a LUKS volume unlocked by a passphrase
   entered at each boot and held nowhere on the machine.
2. **The countersigned processor agreement** with Requesty Ltd returned and on file. The
   terms are accepted and the clauses apply; what is outstanding is the signature
   round-trip, which the controller completes when able. Corrected 2026-08-24: an earlier
   revision stated that it must be complete before the main group. That condition was
   never set by the controller and did not belong in this assessment.
3. **The Approved-Models restriction** under clause 5.5 of the processor agreement, which
   would bind the processor contractually to a named set of models and therefore to their
   retention and training terms. Not configured today. Added 2026-08-23 together with the
   correction that zero data retention binds the processor alone.
4. **The non-EEA conversation-naming model**, a framework defect with the fix in flight,
   described in section 3.5. Until it merges, a piece of every new conversation reaches a
   deployment outside the EEA under the standard contractual clauses. Closed 2026-08-23:
   title derivation is switched off entirely (decision 0077), so no naming request exists
   to cross anywhere — this item no longer conditions the overall judgment.
5. **Records of processing** under Article 30, drafted 2026-08-23 and kept beside this
   document.
6. **The deletion gaps** recorded in decisions 0045 and 0055, waiting on the storage
   framework.

## 10. When this assessment is taken again

Any one of these triggers a review, and none of them is optional:

- A new platform adapter, or the assistant entering a group beyond the project's own.
- A change of processor, model provider, endpoint region or retention setting, including
  anything that would move inference outside the EU.
- Any new path that sends message content off the machine, including a new tool, and any
  change to which identifiers travel with a request.
- A change to what is collected: media, edits, reactions, membership events.
- Any change to the groups' readability. A group that becomes closed, approval-only or
  invite-only voids the Article 9(2)(e) claim recorded for self-posted content, because
  posting into it stops being publication to the public. Added 2026-08-23 with that claim.
- Any moderation capability shipping, in particular the held-out warn, report and ban
  lines.
- A change to retention, or the introduction of any secondary use of the history.
- Closure of either deletion gap, or discovery of a third.
- A personal data breach, or a near miss.
- A complaint or an objection that this assessment did not anticipate.
- New guidance from the German authorities or the EDPB on conversational AI, or a
  decision that changes the legitimate-interest analysis.
- Any capability that touches a person's standing in the group — a real moderation
  decision above the report relay — which also reopens the EU AI Act risk
  classification (a minimal-risk conversational system today; noted 2026-08-23 as
  deployer, corrected 2026-08-23 to provider: the assistant is assembled from a
  general-purpose model, given its own purpose and put into service under the
  operator's name, which is the provider role under the Article 50 guidelines. The
  AI Act compliance record beside the privacy documents carries the analysis).
- Twelve months since this date, with no other trigger.

## 12. Addendum, 2026-08-23: the report and the wiki fetch

The review trigger "any moderation capability shipping" fired with the report
feature, and this addendum is the assessment it demands; the wiki fetch shipped in
the same unit and is covered with it. This document is a draft amended in place, per
its own status line.

**The report, described.** A member replies to an offending message and asks the
assistant for a report. The assistant resolves the reported message from that reply,
never from a judgment of its own about which message deserves reporting, and appends a
report record
carrying the reported message's platform identifier, the reported sender's internal
identifier, and a fixed command line naming the group's moderation bot. The line is
then sent into the group as a reply to the reported message, where the moderation
bot forwards it to the group's administrators.

Corrected 2026-08-23, on the public policy's audit: the sentence that stood here said the
assistant decides nothing. That claims too much. The assistant watches nobody, detects
nothing and files nothing on its own, and the reported message is taken from the asking
member's reply and never from a judgment of its own. Between those two facts sits a model:
the same language model that writes the answers decides that a request is a report request
and produces the relay, so the step can go wrong and file a report about a message nobody
meant. What bounds that is visibility and powerlessness. The relay is a public reply in the
group, so a misfire is seen by everyone the moment it happens, the group's human
administrators judge every report, and the assistant itself cannot warn, remove or ban
anybody.

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
  moderation bot are the judgment. The assistant amplifies a member's ask to the
  administrators, nothing more.
- *A misfired report.* Added 2026-08-23. Because the relay is model-produced, a
  report can name a message its sender never expected to be reported. The effect on
  that person is reputational and momentary, not procedural: no sanction follows
  from the assistant, the administrators see the reported message itself and judge
  it, and the relay is public, so a wrong report is visible and correctable in the
  same channel it appeared in. Residual risk low, and it joins R7 instead of
  standing alone.

**The deletion mirror.** Added 2026-08-23, with the deletion-mirror unit. When a
group administrator deletes a message by replying to it with the moderation bot's
own deletion command, the assistant — which reads the same group — sees that reply
and erases its stored copy of the named message: the same per-row nulls a person's
own deletion applies, scoped to the one message, silently. This is reactive
bookkeeping of an administrator's own act, not a moderation capability: no model
sits anywhere in the path, the assistant judges nothing and answers nothing, and
the step removes stored personal data instead of creating or disclosing any.
Deletions that do not arrive as that reply — the moderation bot's bulk purges and
its direct removals — produce nothing the assistant can read and leave the stored
copy in place; the person-wide deletion routes remain for those, and the operator
reference states the bound plainly. One timing window is named for honesty: an
answer the assistant is already producing when the mirror runs was assembled from
the conversation as it stood before the command arrived, so it can fold the
pre-erasure prose of the deleted message into a public answer. The store is nulled
mid-turn all the same, and every later reading sees only the erased marker — the
same deletion-timing window every erasure route carries, not a new exposure.

**The wiki fetch.** A wiki question makes the assistant fetch one page of the
project's public wiki. The request carries a page name and nothing about any person;
responses are cached in memory for five minutes. This is the same shape as the
existing forge and mirror lookups — a public project source, not a recipient of
personal data — and section 3.5 is amended by naming it there in spirit; the
records-of-processing recipient table carries the row.

**Judgment.** The report is human-initiated, human-judged, bounded, erasable and public
even when the model misfires, and the wiki fetch carries no personal data. The overall
judgment of section 8 stands:
the residual risk is not high in the meaning of Article 36(1). The warn and ban
lines remain held out, and their shipping remains a review trigger.

> Superseded in part 2026-08-24: the report is no longer human-initiated — section 13
> assesses the autonomous form. Everything else in this addendum stands as the record
> of the shipped mechanism it described.

## 13. Addendum, 2026-08-24: the autonomous moderation assessment

The review trigger "any moderation capability shipping" fired a second time: the
assistant now assesses each group message against the group's pinned rules on its own
initiative and reports a clear violation to the group's moderation bot. Member-initiated
reporting is removed as redundant. This document is a draft amended in place, per its
own status line.

**The assessment, described.** Under the helpful answering mode every group message
already reaches the model. The prompt now also teaches it to judge each message against
the newest pinned rules — guaranteed present in its context, the way the system prompt
is — and, on a clear violation, to file a report naming the violating message by the
identifier shown beside it in the conversation. The tool validates that identifier
against the set of messages the turn is actually assessing, so the model cannot aim a
report at an old message, an invented identifier or another chat; each message is
reported at most once, however many turns re-assess it; and the report record stores
what it always stored — the reported message's identifier, its sender's internal
identifier, and the fixed command line. The judgment is reasoned: the configured
reasoning level sizes the model's thinking, and the teaching instructs it to think
before it reports and to leave borderline calls, rule-absent messages and rule-less
groups unreported.

**The processing purpose.** Autonomous assessment joins the stated purposes under the
same legitimate interest as the rest — running an assistant in our own community groups,
which includes keeping them within their own pinned rules. It reads no message the
helpful mode did not already read and stores nothing beyond the existing report record.
It does add one element to what a request carries to the provider: so the model can name
the message it assesses, each user message in the projection now opens with that
message's platform identifier — an opaque per-message reference, travelling beside the
public handle already transmitted (the transmitted-identifier line above), carried only
inside the request, retained by the processor under the zero-retention terms, and nulled
by erasure together with the message it marks. It is not an identity identifier — the
numeric account identifier still stays on the machine, and no display name is attached to
a message — so the
identity-minimisation posture at the provider boundary is unchanged; beyond that one
projection element and the unchanged report record, the assessment discloses nothing new.
The public policy states the assessment in plain words.

**The human-decides bound.** The assistant assesses, the moderation bot's human
administrators decide (decision 0070, structural): the report is the capability's whole
output, no warn, mute, removal or ban exists, and the administrators can ignore any
report. This is not an automated decision within the meaning of Article 22 GDPR: the
output is a report to humans who decide, not an effect on the member.

**The false-positive residual.** A wrong report is now the model's own misjudgment —
nobody asked for it — and it pings the administrators and names a member's message in
front of them. That exposure is the accepted residual of an assessment-only capability,
recorded plainly: it is bounded by the human decision (no consequence follows from the
assistant), by the per-origin bound (one report per message, ever), by publicity (the
report is a visible reply in the same channel, correctable where it happened), and by
the reasoning the judgment runs at. No reasoning trace is stored — what is stored is
the report itself — and this assessment claims no audit trail beyond that. The residual
joins R7.

**Judgment.** The assessment is machine-made, human-judged, bounded per message,
erasable and public. The overall judgment of section 8 stands: the residual risk is
not high in the meaning of Article 36(1). The warn and ban lines remain held out, and
their shipping remains a review trigger.

## 14. Addendum, 2026-08-29: the web search

The review trigger "any new path that sends message content off the machine,
including a new tool" fired with the web search, and this addendum is the
assessment it demands. This document is a draft amended in place, per its own
status line.

**The search, described.** The assistant answers project questions from the
project's own sources. Asked something else — a question about the world — it can
now call one tool that sends a search query to a search provider and reads back a
page of ranked results: each result's title, its link, its snippet where the result
has one, and a hint about the kind of host it sits on. It opens no page, follows no
link and reads no document; fetching is not part of this capability (decision 0110).
The query is written by the model, in words drawn from the conversation, and it is
sent exactly as written or not at all.

**What is new for the people concerned.**

- *A further recipient of member-derived text.* The provider receives the query.
  This is the first recipient of members' words other than the model processor and
  the platform itself, and it is stated as such. It receives nothing beside the
  query: no account identifier, no username, no other part of the conversation. The
  boundary is therefore NARROWER than the model provider's, which carries the whole
  conversation and each speaker's public handle.
- *A further transfer.* The provider is a company in the United Kingdom, covered by
  the Commission's adequacy decision under Article 45 GDPR. Section 3.5's list is
  four places now, not three, and the public policy says so in the same words.
- *A wider stored surface behind an unclosed gap.* A tool call and its result live
  on framework-owned tables the erasure path does not reach (decision 0045). That
  gap was accepted on the ground that lookup inputs are "overwhelmingly technical".
  A model-written query out of a conversation is not that, so decision 0045 carries
  a dated amendment saying so, and the two mitigations that answer the widening —
  the guard, and the refusal that echoes nothing — are recorded there.

**What bounds it.** The query guard refuses any query carrying a person reference in
the handle form — an at sign followed by a name, matched on a normalised view so
spaced, dotted and zero-width spellings are one token — whole, before anything is
sent, and its refusal names the rule without echoing what it matched (decision
0115). The guard is a discipline device and is recorded as one: it stops the
deliberate submission of an identifier, and no lexical rule can stop a model that
describes a person instead. Beside it: a query is at most 400 characters and is
refused instead of truncated past it, pages run to five, and each person draws at
most five searches per ten minutes (decisions 0112, 0117). The assistant's answer
still names where a claim came from, and a claim about the project may not come from
the web at all — the project lookups remain its only source (decision 0116).

**The false-answer question.** A snippet can be wrong, and an answer built on one
can be wrong with it. This is R7's existing residual and not a new one: the answers
were already model-written and already stated as fallible in the public policy. What
changes is where a wrong claim can come from, and the teaching answers that
narrowly — a snippet is a hint, an answer built on one says where it came from, and
a snippet that does not contain the claim is a miss.

**Judgment.** One further processor, in an adequacy-covered country, receiving a
bounded query and nothing else, under a per-person spend bound, with deliberate
person references refused before they are sent. The new risk R12 is registered with
its mitigations and its residual re-rating in sections 6 to 8. The overall judgment
of section 8 stands: the residual risk is not high in the meaning of Article 36(1).
Fetching a page is not part of this capability and remains a review trigger of its
own.
