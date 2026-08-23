# Record of processing activities: the halogenOS Group Assistant

**Draft, not yet published.**

Date: 2026-08-23

The record required by Article 30(1) GDPR. One controller, one processing activity. The
detail behind the entries lives in the impact assessment and the legitimate-interest
assessment beside this document; this record states the facts in the form the supervisory
authority asks for them.

## 1. Controller and contact

| Entry | Content |
|---|---|
| Controller | Simão Gomes Viana, c/o IP-Management #10911, Ludwig-Erhard-Str. 18, 20459 Hamburg, Germany |
| Contact for data protection | privacy@halogenos.org |
| Data protection officer | None appointed. The thresholds in § 38 BDSG are not met. |
| Joint controllers | None. The assistant serves only groups the controller operates and administers. |
| Representative under Article 27 | Not applicable. The controller is established in the EU. |
| Record maintained since | 2026-08-23 |

## 2. Processing activity

| Entry | Content |
|---|---|
| Reference | A-01 |
| Name | Community assistant answering questions in the project's chat groups |
| Description | A bot in the halogenOS community groups stores the groups' messages, and answers questions addressed to it by sending the conversation's text to a language model through a processor. |
| Platforms | Telegram today. A further platform reaches this record before it ships. |
| Started | Pre-alpha. The activity has not begun in the main community group. |

## 3. Purposes

| # | Purpose | Legal basis |
|---|---|---|
| P1 | Answering community questions about the project in its own chat groups | Article 6(1)(f), legitimate interest |
| P2 | Reading a conversation in context, including older discussion, so an answer follows the thread | Article 6(1)(f), legitimate interest |
| P3 | Keeping the assistant available: counters bound how much it answers per person and per chat | Article 6(1)(f), legitimate interest |

The interests, the necessity of each purpose and the balancing are assessed in the
legitimate-interest assessment. Consent is not used and is not collected anywhere in this
activity.

## 4. Categories of data subjects

| # | Category | Note |
|---|---|---|
| S1 | Members of the project's community groups whose messages the assistant stores | Includes members who never address the assistant. The set is open and not enumerable in advance. |
| S2 | People who write to the assistant directly | Their conversations are stored the same way and removed whole on erasure. |
| S3 | Group administrators, in that capacity | The authority a person held in a chat at the time of a message is stored beside it. |

Minors are not excluded by any mechanism the project controls, and the notice and the
erasure path are written to that standard.

## 5. Categories of personal data

| # | Category | Content | Where it is stored |
|---|---|---|---|
| D1 | Message content | The text of a message, including the caption of a media message. No media, no files, no voice, no stickers. Edits are not collected. | Content table of the message block kind |
| D2 | Identity | The platform's opaque account identifier, display name, username. The username is transmitted to the processor with each request, by the operator's decision of 2026-08-23, so the assistant can address people by their handle. The account identifier and the display name are not transmitted. | Identity tables of their own, never inline in the ledger |
| D3 | Circumstance | Arrival time, platform send time, reply reference, whether the message was addressed to the assistant, the authority held in that chat at that moment | Content table of the message block kind |
| D4 | Group facts | Channel title, pinned rules text, stored as context notes | Note table |
| D5 | Derived state | Conversation membership and order, answering counters, tool palette, group authorization | Ledger and its side tables |
| D6 | Special categories, incidentally | Free text can reveal health, belief, political opinion or sexual orientation in passing. Not sought, not detected, not used. | Inside D1 |
| D7 | Report record (added 2026-08-23) | The reported message's platform identifier, the reported sender's internal identifier, and the fixed report command line. Written only when a member replies to a message and asks for a report. | Content table of the report block kind |
| D8 | Reply reference (added 2026-08-23) | The platform identifier of the message a message replies to, kept for reply threading and the report's target. | Content table of the message block kind, beside D3 |

Personal data is kept apart from the ledger by design: a block carries position, kind and
links, and the personal columns live in tables referenced by key, so append-only storage
and erasure coexist (decisions 0003, 0006, 0012).

Anonymous administrator posts and automatic channel forwards are not stored at all, because
the platform hides the real author (decision 0016).

## 6. Categories of recipients

| # | Recipient | Role | What it receives |
|---|---|---|---|
| R1 | Requesty Inc. | Processor under Article 28, on the controller's instruction only | The conversation's text and the public username of each speaker, plus the system prompt and the group's context notes. The account identifier and the display name are not sent. Requests enter the EU region in Frankfurt, Germany and are served on infrastructure in the EU. Zero data retention is enabled: nothing is stored once the response is returned, and nothing is used for training. |
| R2 | The model providers Requesty routes to, Google among them today | Sub-processors engaged by R1 under its own agreements, with R1 answerable for them | The same request, served on EU infrastructure. The model is not named in this record, because the processor's obligations and the region, not the model's identity, are what the record states. |
| R3 | Public project sources | Not a recipient of personal data | A commit lookup queries the halogenOS forge and a release lookup queries the builds repository's public interface. A query carries a repository name and a reference or tag. |
| R4 | The chat platform | Independent controller of its own delivery and storage, not a processor of the controller | Its own handling of the same messages, under its own policy, unchanged by the assistant. |
| R5 | The group's administrators, via the group's moderation bot (added 2026-08-23) | Recipients of the report event inside the group they already administer | When a member replies to a message and asks for a report, the assistant sends the fixed report command as a reply to that message; the moderation bot forwards the event to the group's administrators. The event carries the reported message's identifier — a message the administrators already see in their own group — and no data from the assistant's store. |

Nobody else receives the data. It is not sold, not shared with advertisers, not analysed
for any secondary purpose, and not used to train any model.

> Amended 2026-08-23: R3 gains the project wiki's public pages beside the forge and
> the builds repository — a wiki query carries a page name and nothing about any
> person — and R5 records the report event. The impact assessment's addendum of the
> same date assesses both.

## 7. Third-country transfers

| Entry | Content |
|---|---|
| Transfers intended | None. Requests enter through the processor's European endpoint in Frankfurt, Germany (AWS eu-central-1) and are served on infrastructure in the EU, and the store is held on a server run for the project in Germany. |
| Safeguard held in reserve | The processor agreement carries the European Commission's standard contractual clauses, so a transfer that occurred outside the intended routing would rest on Article 46(2)(c). |
| Documentation | The processor agreement and its clauses are on file with the controller. The countersigned copy is outstanding, listed as an open dependency in section 10. |

## 8. Erasure concept and time limits

| # | Data | Time limit |
|---|---|---|
| D1, D3 | Message content and circumstance | No scheduled expiry. Kept until erasure is requested, and erased on request. The reasoning is recorded in decision 0003 and assessed in the impact assessment. |
| D2 | Identity | Deleted on erasure of the person. Display fields are refreshed from the platform on each later message and never accumulate a history. |
| D4 | Group facts | Kept while the group is served. A note is superseded when the group's rules are pinned anew. |
| D5 | Derived state | Answering counters age out of their window by use. Conversation state follows the messages it derives from. A direct conversation is removed whole on erasure. |
| Direct chats | Whole conversations with the assistant | Removed entirely on erasure, mappings included, because a two-party chat that lost its human still identifies the person (decisions 0011, 0012). |
| D7, D8 | Report record and reply reference (added 2026-08-23) | The reported person's erasure empties the report record's message reference, and an emptied report is never sent. The reply reference is emptied from both ends: a person's own messages lose it with the rest of their row, and every other person's message replying to one of the erased person's messages loses its stored copy of that message's identifier too (decision 0063 and its refinement of 2026-08-23). Qualified 2026-08-23: a reply recorded between a failed erasure attempt and its retry keeps its stored copy — the retry can no longer match it once the person's own message references are emptied. The copy links to no recorded person; a reach key independent of those references is the decided follow-up, recorded in decision 0063's second refinement. Widened 2026-08-23: the same stored copy also stays on a reply recorded after the person's erasure completed, and on a reply to a message the assistant never recorded — the retry window is one way the match is lost, not the boundary. Every such copy links to no recorded person, and the reach key above is the decided follow-up for all of them, recorded in decision 0063's later refinement. |

**Erasure on request** is one operation with three steps: the person's message text, send
time and reply reference are emptied in every conversation, their direct conversations are
removed whole, and their identity rows are deleted. Block structure is untouched, so
nothing is orphaned; an emptied message contributes a fixed marker and never a word of the
person's prose (decision 0027). The operation waits for an open model stream on the
affected conversation, confirms settlement by re-reading stored state, and fails loudly
without deleting anything past a bounded wait (decision 0028). Requests are answered within
one month.

**Two recorded gaps**, stated here as they are stated in the public policy and the impact
assessment:

1. Lookup call records live in framework-owned tables the erasure path does not reach
   today (decision 0045). Their content is technical, and a query can quote the words a
   person used to ask.
2. Context notes carry no person reference, so erasure cannot reach them even in principle
   (decision 0055). A rules text can in principle name a person.

Both wait on the same storage framework seam and are named openly to the people concerned.

## 9. Technical and organisational measures

A general description under Article 30(1)(g), mapped to the mechanisms that ship.

| Area | Measure |
|---|---|
| Separation | Personal data in tables of its own, referenced by key, never inline in the ledger (decisions 0003, 0006, 0012). |
| Data minimisation | Text only, no media, no files, no voice, no stickers, no edits (decision 0017). Anonymous stand-in senders skipped (decision 0016). No profiling, no scoring, no secondary use. |
| Minimisation at the boundary | One identifier is transmitted to the processor, the public username, decided by the operator on 2026-08-23 so the assistant can address people by their handle. The display name and the numeric account identifier stay on the machine, and no other attribute of a person is attached to a request. |
| Processor control | Article 28 agreement with standard contractual clauses, European entry point, serving inside the EU, zero data retention, no training on the content, and sub-processors engaged by the processor under its own agreements. |
| Secret handling | The provider key is held in memory and never written to storage (decision 0020). Secrets are referenced indirectly in configuration, by environment variable name or file path, and never appear in it. |
| Access control | Group admission is a stored authorization written only by the operator's own invitation. Every other contact is refused without touching the ledger, and the assistant withdraws (decision 0052). |
| Availability and abuse | Two answering counters, per person across all chats and per chat, limit answering and never storage. An over-limit message draws silence, so a flooder cannot borrow the assistant's voice (decisions 0030, 0034). |
| Boundary discipline in the tools | The lookup palette is recorded per conversation and admission fails closed; tool authority is floored at member level; lookup failures are reported to the model and never to the chat (decisions 0041, 0044). |
| Bounded input into the system voice | Byte bounds on both surfaces the group controls, the rules text and the title, with over-bound text refused whole instead of truncated (decisions 0048, 0049). |
| No decisions about people | No decision capability ships. A member can ask the assistant to relay a report to the group's moderation bot; the assistant detects nothing, files nothing on its own, and the group's human administrators judge the report (added 2026-08-23; the warn and ban lines stay held out of the system prompt, decision 0046). No automated decision with legal or similarly significant effect is taken. |
| Transparency | The group's pinned rules announce the assistant and point at the policy, and the privacy command answers deterministically, without a model turn, at most once per chat per window (decision 0053). |
| Erasure | The operation described in section 8, with its two recorded gaps. |
| Storage protection at rest | Deployment configuration. Open, see section 10. |

## 10. Open dependencies

Stated here exactly as the impact assessment states them, because this record must not
claim a measure that is not yet in place:

1. **Storage protection at rest** for the message store. Required by the platform's
   developer terms and relied on by the breach mitigation in the impact assessment. Must
   be in place before the assistant enters the main community group.
2. **The countersigned processor agreement** with Requesty Inc., returned and on file. The
   terms are accepted and the clauses apply; the signature round-trip is outstanding and
   must be complete before the main community group.

## 11. Keeping this record current

This record is updated whenever the activity changes, and in any case on the review
triggers listed in the impact assessment: a new platform or a group beyond the project's
own, a change of processor, model provider, region or retention setting, a new path that
sends message content off the machine, a change to what is collected, any moderation
capability shipping, a change to retention or any secondary use, closure or discovery of an
erasure gap, a personal data breach, and twelve months since the date above with no other
trigger.
