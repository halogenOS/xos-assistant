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
| Data protection officer | None appointed, decided 2026-08-23 with the reasoning recorded in the impact assessment. § 38(1) sentence 2 BDSG attaches its duty to processing that is objectively subject to an assessment under Article 35, and that document's pre-check concludes Article 35 does not compel one here. The assessment is carried out as a precaution, which does not create the duty. The residual risk of the opposite reading is named there. This row previously gave the headcount thresholds as the reason, and then the existence of the assessment; both were wrong. |
| Competent supervisory authority | Bayerisches Landesamt für Datenschutzaufsicht (BayLDA), Promenade 18, 91522 Ansbach. Settled by the operator 2026-08-23: the establishment is in Bavaria, and the Hamburg address above is a mail-forwarding contact address only. |
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
| P1 | Answering community questions in the project's own chat groups. Widened 2026-08-29 with the web search: the earlier wording read "about the project", which described what the assistant could answer from and not what members ask. Questions that are not about the project are answered too, and answering one can send a search query to the search provider in R6. | Article 6(1)(f), legitimate interest |
| P2 | Reading a conversation in context, including older discussion, so an answer follows the thread | Article 6(1)(f), legitimate interest |
| P3 | Keeping the assistant available: counters bound how much it answers per person and per chat | Article 6(1)(f), legitimate interest |

The interests, the necessity of each purpose and the balancing are assessed in the
legitimate-interest assessment. Consent is not used and is not collected anywhere in this
activity.

## 4. Categories of data subjects

| # | Category | Note |
|---|---|---|
| S1 | Members of the project's community groups whose messages the assistant stores | Includes members who never address the assistant. The set is open and not enumerable in advance. |
| S2 | People who write to the assistant directly | Direct chats are switched off in this deployment, so a direct message is refused before any write and nobody is a data subject by that route today. The impact assessment's scope section states what the assistant stores in a direct chat when the switch is on. |
| S3 | Group administrators, in that capacity | The authority a person held in a chat at the time of a message is stored beside it. |

Minors are not excluded by any mechanism the project controls, and the notice and the
erasure path are written to that standard.

## 5. Categories of personal data

| # | Category | Content | Where it is stored |
|---|---|---|---|
| D1 | Message content | The text of a message, including the caption of a media message. No media, no files, no voice, no stickers. Changed 2026-08-31 (unit T3): an edited message is stored as a further version beside the first, in a row of its own naming the message it revises — nothing is rewritten, and the earlier version stays. A repeat of a version already stored, which the platform delivers on its own for changes nobody made, is not recorded, and neither is an edit that leaves a message without text or an edit naming a message the store holds no version of. | Content table of the message block kind |
| D2 | Identity | The platform's opaque account identifier and username. The username is transmitted to the processor with each request, by the operator's decision of 2026-08-23, so the assistant can address people by their handle. The account identifier is not transmitted. Narrowed 2026-08-23 (decision 0077): the display name is no longer collected or stored as identity data — its column was dropped with its values, and the adapter decodes no name field on a message. Qualified 2026-08-29 (unit 36): a display name is still not identity data and is not attached to messages; where a join notice announces someone, the name that notice showed is stored once as the event's own content under D10, and is erased with the person. | Identity tables of their own, never inline in the ledger |
| D3 | Circumstance | Arrival time, platform send time, reply reference, whether the message was addressed to the assistant, the authority held in that chat at that moment. Extended 2026-08-31 (unit T3): for an edited message the platform's edit time is what is recorded as its send time — the moment that version came into being — and the arrival time beside it is unchanged. | Content table of the message block kind |
| D4 | Group facts | Channel title, pinned rules text, stored as context notes | Note table |
| D5 | Derived state | Conversation membership and order, answering counters, tool palette, group authorization | Ledger and its side tables |
| D6 | Special categories, incidentally | Free text can reveal health, belief, political opinion or sexual orientation in passing. Not sought, not detected, not used. Aligned 2026-08-23: the groups are readable by anyone on the platform without joining or approval, so content a person posts about themselves is covered by Article 9(2)(e), and what one member reveals about another is the residual the impact assessment carries under R2. | Inside D1 |
| D7 | Report record (added 2026-08-23) | The reported message's platform identifier, the reported sender's internal identifier, and the fixed report command line. Written only when a member replies to a message and asks for a report. Changed 2026-08-24: written when the assistant's own assessment finds a message in clear violation of the group's pinned rules — member-initiated reporting is removed — and at most once per reported message. Qualified 2026-08-29 (unit 36): a report may name a join announcement (D10) instead of a message, and where that announcement named several people the record holds no reported person's identifier at all — one is stored only where the reported record names exactly one person, so no report attributes an announcement to a person who may not be its subject. | Content table of the report block kind |
| D8 | Reply reference (added 2026-08-23) | The platform identifier of the message a message replies to, kept for reply threading and the report's target. Narrowed 2026-08-24: kept for reply threading alone — the report now names its target by the message identifier the assistant's own assessment reads, never from a stored reply. | Content table of the message block kind, beside D3 |
| D9 | Suppression flag (added 2026-08-23) | One boolean on the identity row recording that the person opted out of collection on that platform. Purpose: honoring the objection going forward — from the moment it stands, the person's new messages are dropped at ingestion, and the flag survives the person's own deletion because forgetting who objected would silently resume collection. Set and cleared only by the person's own commands or their own plain-language ask through the privacy tool. | Identity table, on the person's row |
| D10 | Join notice (added 2026-08-29) | One record per person a group's join announcement named: the name the platform displayed for them at that moment, their public handle, their internal identifier, the announcement's own platform identifier and its send time. Purpose: the assistant reads what the group read, so a joining account whose displayed name is itself an advertisement can be reported to the group's moderation bot before it posts anything — the report is the whole effect, and the group's human administrators decide (decision 0070). Not written for a person whose suppression flag stands, and not written for the assistant's own entry. | Content table of the join-notice block kind |
| D11 | Reaction record (added 2026-08-30) | The emoji the assistant chose, the marked message's platform identifier, the marked person's internal identifier — the same datum D7 names for a report — and the time the record was written. Written when the assistant puts an emoji reaction on a message instead of replying to it, and at most once per marked message. The emoji is the assistant's own expression and no data about the person; the message reference and the internal identifier are, which is why the record is here. The assistant collects nobody else's reactions: the platform sends reaction updates only to a chat administrator, and this assistant is deliberately not one. | Content table of the message-mark block kind |
| D12 | Revision reference (added 2026-08-31, unit T3; recorded here 2026-09-03, unit 58, where the category was stated only in the time limits) | The platform identifier of the message an edited message is a further version of, stored on the later version's row. It is the author's own data, the identifier of a message they sent. | Content table of the message block kind, beside D1 |
| D13 | Compaction summary (added 2026-09-03, unit 58) | The prose the model writes about the older half of a long conversation, out of the members' messages in it, so the conversation can continue without carrying that half whole. It is written from personal data and can quote it, it opens the conversation that continues, and it travels to the processor in every later request. | Content table of the block the successor conversation opens with |
| D14 | Platform message number (added 2026-09-03, unit 58) | The message's own number on the platform, stored on its row and opening the message's projected line, where sections 6 and 9 already state that it rides in every request and that erasure nulls it. It names a message and not a person, and it is the author's own data, the identifier of a message they sent. | Content table of the message block kind, beside D1 |

Personal data is kept apart from the ledger by design: a block carries position, kind and
links, and the personal columns live in tables referenced by key, so append-only storage
and erasure coexist (decisions 0003, 0006, 0012). Stated 2026-09-03 (unit 58): one
category is prose and not a column. The compaction summary (D13) is written from personal
data and can quote it, and erasure reaches it by the lineage rebuild D13's own row in
section 8 describes, never by nulling a column.

Anonymous administrator posts and automatic channel forwards are not stored at all, because
the platform hides the real author (decision 0016).

## 6. Categories of recipients

| # | Recipient | Role | What it receives |
|---|---|---|---|
| R1 | Requesty Ltd, London, United Kingdom (entity corrected 2026-08-23 from Requesty Inc.) | Processor under Article 28, on the controller's instruction only | The conversation's text and the public username of each speaker, plus the system prompt, the group's context notes and its stored join notices. The account identifier is not sent. Corrected 2026-08-29 (unit 36): no display name is attached to a message (decision 0077), and the one display name that does travel is the name a join announcement showed, which rides in the projected join line as that event's content. Extended 2026-08-29 (unit 29): a member's administrator standing reaches it too, on demand and never with the conversation — when the model looks a handle up with the standing tool, the tool's fixed answer states whether that person was an administrator when they last spoke, names the handle in the affirmative case, and travels in the next request as that tool result. Requests enter through its European endpoint, and what it stores it stores in Frankfurt, Germany (AWS eu-central-1). Zero data retention is configured: it writes no message and no answer to storage and uses none of it for training. It keeps billing telemetry carrying no content, meaning token counts, the model identifier and a timestamp. Recorded 2026-09-03 (unit 58): two further things ride in the same request, and rode in it before this entry stated them. Every projected member message and every projected join line opens with the platform's own number for that message, which names a message and not a person, and is what lets the model name the message it assesses. And where a long conversation was compacted, the summary the model wrote of its older half (D13) opens the conversation that continues, so it travels with every later request. |
| R2 | Sub-processors engaged by R1, in two layers: the infrastructure it runs on, Amazon Web Services in Frankfurt, and the model providers it routes to, stated as a category (generalised 2026-08-27: a provider follows the model chosen and is not named here, exactly as individual models are not — the record states the chain) | Sub-processors under R1's own agreements. R1 stays answerable for the infrastructure layer, and for the provider layer it answers for the choice, for the written terms and for reporting the provider's published position accurately, not for that provider's own breach of it | The same request. Corrected 2026-08-23: zero data retention binds R1 alone, so whether a model provider keeps a request or trains on it follows the terms of the model chosen. Individual models are not named in this record, because what the record states is the chain, the region a deployment sits in and where the retention promise ends. Recorded 2026-09-03 (unit 58): the region this row owes is stated here. The controller verified on 2026-08-31, by reading the provider's published serving region, that the provider of the configured model serves inside the EU, and the public policy's sentence about the model running in the EU rests on that check. The deployment's configuration names the model, and the provider stays unnamed for the reason above. A change of model, provider or region fires the review trigger in section 11, and that trigger is where the region is checked again. |
| R3 | Public project sources | Not a recipient of personal data | A commit lookup queries the halogenOS forge and a release lookup queries the builds repository's public interface. A query carries a repository name and a reference or tag. |
| R4 | The chat platform | Independent controller of its own delivery and storage, not a processor of the controller | Its own handling of the same messages, under its own policy, unchanged by the assistant. |
| R5 | The group's administrators, via the group's moderation bot (added 2026-08-23) | Recipients of the report event inside the group they already administer | When a member replies to a message and asks for a report, the assistant sends the fixed report command as a reply to that message; the moderation bot forwards the event to the group's administrators. The event carries the reported message's identifier — a message the administrators already see in their own group — and no data from the assistant's store. Changed 2026-08-24: the report is sent when the assistant's own assessment finds a message in clear violation of the pinned rules; the administrators decide, and the assistant takes no action itself. |
| R6 | Serper, United Kingdom (added 2026-08-29, with the web search) | Processor under Article 28, on the controller's instruction only. Its own terms name the customer as controller and itself as processor where personal data is processed in the service | The search query the assistant writes for a question that is not about the project — words drawn from the conversation, and nothing else: no account identifier, no username and no other part of the conversation. A query written with a handle in it — an at sign followed by a name — is refused before anything is sent, so no query in that form reaches this recipient. The transfer basis is in section 7. |

Nobody else receives the data. It is not sold, not shared with advertisers, not analysed
for any secondary purpose, and not used to train any model.

Added 2026-08-30, with the reactions unit, so it is stated instead of left to inference:
the emoji the assistant chooses travels to the chat platform with the placement and to
nobody new. It reaches no processor — it is not part of what is sent to the model, and the
stored reaction record travels nowhere at all. No recipient is added by this capability,
and R1's list of what a request carries is unchanged.

> Amended 2026-08-23: R3 gains the project wiki's public pages beside the forge and
> the builds repository — a wiki query carries a page name and nothing about any
> person — and R5 records the report event. The impact assessment's addendum of the
> same date assesses both.

## 7. Third-country transfers

| Entry | Content |
|---|---|
| Transfers, rewritten 2026-08-23 | The earlier entry stating that no transfer is intended was wrong. The store is held on a server run for the project in Germany, and data leaves the EEA in three places, each listed below. |
| The processor itself | Requesty Ltd is a company in the United Kingdom, although it stores and serves in Frankfurt. Covered by the European Commission's adequacy decision for the United Kingdom under Article 45 GDPR, so no further safeguard is required. |
| Model deployments outside the EEA | Where a deployment sits outside the EEA, the request reaches it there. Covered by the European Commission's standard contractual clauses in the processor agreement, under Article 46(2)(c) GDPR. Recorded 2026-08-23 as live for the conversation-naming step, which reached a lightweight model hardcoded in the framework outside the EEA; closed the same day by decision 0077 — title derivation is switched off entirely, so no naming request exists and the entry covers the answering model's routing alone. |
| The search provider (added 2026-08-29) | Serper is a company in the United Kingdom and receives the search query there. Covered by the same adequacy decision for the United Kingdom under Article 45 GDPR, so no further safeguard is required. With this entry the count in the row above becomes FOUR places, not three. |
| The chat platform | Sits outside the EU/EEA and receives every message and every answer as part of delivering them, as its own controller under its own policy, not on the controller's instruction. |
| Documentation | The processor agreement and its clauses are on file with the controller. The countersigned copy is outstanding, listed as an open dependency in section 10. |

## 8. Erasure concept and time limits

| # | Data | Time limit |
|---|---|---|
| D1, D3 | Message content and circumstance | Changed 2026-09-02 (unit 53): kept while the conversation holding them is in use, and deleted whole 90 days after that conversation's newest entry. Any entry refreshes the whole conversation, so the span measures disuse and never the age of a single message. Erasure on request is unchanged and never waits for the span. The reasoning is recorded in decision 0003 as refined by decision 0198, and assessed in the impact assessment. |
| D2 | Identity | Deleted on erasure of the person. Widened 2026-09-02 (unit 53): also deleted when the retention sweep leaves no stored row naming that person at all — no message, no join announcement, no report, no reaction — because identity kept for records that are gone is identity kept for nothing. A row whose suppression flag stands is not reached by the sweep, for the reason under D9: remembering the objection is what honoring it takes. The username is refreshed from the platform on each later message and never accumulates a history; no display name is stored as identity data (decision 0077), and the shown name a join announcement carried is erased with the person under D10 (unit 36, 2026-08-29). Qualified 2026-08-23: for a person whose suppression flag stands, the refresh stops with the flag, and erasure empties the row to the flag-bearing remnant described under D9 instead of deleting it — remembering the objection is what honoring it takes. |
| D4 | Group facts | Kept while the group is served. A note is superseded when the group's rules are pinned anew. |
| D5 | Derived state | Answering counters age out of their window by use. Conversation state follows the messages it derives from. A direct conversation is removed whole on erasure. |
| Direct chats | Whole conversations with the assistant | Removed entirely on erasure, mappings included, because a two-party chat that lost its human still identifies the person (decisions 0011, 0012). |
| D7, D8 | Report record and reply reference (added 2026-08-23) | The reported person's erasure empties the report record's message reference, and an emptied report is never sent. The reply reference is emptied from both ends: a person's own messages lose it with the rest of their row, and every other person's message replying to one of the erased person's messages loses its stored copy of that message's identifier too (decision 0063 and its refinement of 2026-08-23). Qualified 2026-08-23: a reply recorded between a failed erasure attempt and its retry keeps its stored copy — the retry can no longer match it once the person's own message references are emptied. The copy links to no recorded person; a reach key independent of those references is the decided follow-up, recorded in decision 0063's second refinement. Widened 2026-08-23: the same stored copy also stays on a reply recorded after the person's erasure completed, and on a reply to a message the assistant never recorded — the retry window is one way the match is lost, not the boundary. Every such copy links to no recorded person, and the reach key above is the decided follow-up for all of them, recorded in decision 0063's later refinement. |
| D10 | Join notice (added 2026-08-29) | The person's erasure empties their join records in every conversation — the shown name, the handle, the announcement reference and its send time — one record per person, so another person named by the same announcement keeps theirs. An emptied join record states nothing to the model. An administrator's deletion of the announcement itself empties every record of that announcement, and a person whose suppression flag stands has no join record to erase, because none was ever written. |
| D11 | Reaction record (added 2026-08-30) | The marked person's erasure empties the record's stored message reference. The row survives with the erasure-keyed internal identifier and the chosen emoji: the identifier is what erasure reaches the row by, and the emoji states what the assistant expressed and names nobody. An emptied reaction record states nothing to the model, which never sees the record at all. An administrator's deletion of the marked message empties the reference too, through the same deletion mirror that reaches a message's reply references and a report's target. The record lives exactly as long as the message record beside it, under D1's own rule: kept while its conversation is in use, deleted with that conversation 90 days after its newest entry (changed 2026-09-02, unit 53), erased on request. **The reaction already visible in the chat is not withdrawn** — that is the stated residual: withdrawing it would mean a network call from inside an operation that is store-only by design, and the visible reaction is a fact about the assistant, attached to a message the group and the platform already hold as their own, naming nobody. |
| D12 | Revision reference (added 2026-08-31) | The identifier of the message an edited message is a further version of. It is the author's own data — the identifier of a message they sent — so their erasure empties it beside the text, the origin, the send time, the reply reference and the handle on every row they wrote. An administrator's deletion of the message through the moderation bot empties every recorded version of it, this reference included, so one deletion reaches the whole chain however many versions it holds. |
| D13 | Compaction summary (added 2026-09-03, unit 58) | Kept inside the conversation that opens with it and deleted with that conversation, under D1's own span: 90 days after that conversation's newest entry. The compacted ancestor the summary was written from carries the same span from its own last entry. An erasure does not empty the prose the way a column is nulled — the lineage is rebuilt from the root upward without the erased person's blocks, each summary regenerated from the rebuilt half beneath it, and every superseded summary goes with the conversation that held it. Nothing established is deleted until every regenerated summary is in hand, so a failed rebuild leaves the lineage standing and the operation runs again. |
| D14 | Platform message number (added 2026-09-03, unit 58) | The span it shares with the message: kept while the conversation holding it is in use, and deleted with that conversation 90 days after its newest entry. Erasure on request nulls it beside the text, through the author-keyed pass, and an administrator's deletion of the message nulls it on every recorded version through the deletion mirror. |

Added 2026-09-03 (unit 58): if the service stops for good, the controller deletes the
server. Deleting it deletes everything held for the service and ends every span in this
table. That commitment has stood in the public policy since 2026-08-23, and it is the
controller's word and not a mechanism in the code.

Added 2026-09-03 (unit 58): a request about a message the requester did not write is
answered by a person, who weighs it against the writer's rights, decides, and states the
outcome. That route is the controller's word too, and no code path deletes another
author's message on request.

**Erasure on request** is one operation with three steps: the person's message text,
platform message number, send time, reply reference, handle and revision reference are
emptied in every conversation, their direct conversations are removed whole, and their
identity rows are deleted. Widened 2026-09-03 (unit 58): the first step empties the six
columns the erasure pass nulls. Block structure is untouched, so nothing is orphaned; an
emptied message contributes a fixed marker and never a word of the person's prose
(decision 0027). The operation waits for an open model stream on the
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
| Data minimisation | Text only, no media, no files, no voice, no stickers (decision 0017). Extended 2026-08-31 (unit T3): each distinct version of an edited message is stored, and the platform's own repeated deliveries of unchanged text are not. Anonymous stand-in senders skipped (decision 0016). No profiling, no scoring, no secondary use. Extended 2026-08-29 with the web search: a query is bounded to 400 characters and refused whole instead of truncated past it, pages are bounded to five, and each person's searches are bounded to five per ten minutes — so the volume of member-derived text that can reach the search provider is bounded per person and per call (decisions 0112, 0117). |
| Minimisation at the boundary | One identifier of a PERSON is transmitted to the processor, the public username, decided by the operator on 2026-08-23 so the assistant can address people by their handle. The numeric account identifier stays on the machine, no display name is stored beside a message or attached to a request as identity data (decision 0077), and no other attribute of a person is attached to a request. Corrected 2026-08-29 (unit 29): one attribute now reaches a request without being attached to a message — a person's administrator standing, sent only as the answer to a standing lookup the model made about a handle the conversation showed, and never as a field beside their messages. Corrected 2026-08-29 (unit 36): a join announcement's shown name is stored as that event's content and rides the projected join line into a request, which is the one display name that crosses the boundary. Extended 2026-08-29 for the search provider (R6), whose boundary is narrower still: it receives the query alone — no identifier of any kind, no username, no conversation — and a query carrying a person reference in the handle form is refused whole before anything is sent, by a guard that does not echo what it matched (decision 0115). Corrected 2026-09-03 (unit 58): one further identifier crosses the boundary, and it names no person. Every projected member message and every projected join line opens with the platform's own number for that message, so the model can name the message it assesses, and that number is erased with the message it marks: the author-keyed erasure pass nulls the platform's number beside the text, and the deletion mirror nulls it on every recorded version. The internal number that stays on an erased message record, which the public policy names, is a different number and reaches no request. |
| Processor control | Article 28 agreement with standard contractual clauses, European endpoint, storage in Frankfurt, the United Kingdom's adequacy decision for the processor relationship, zero data retention at the processor and no training there, and sub-processors engaged by the processor under its own agreements. Corrected 2026-08-23: the retention promise reaches the processor and stops, so the provider layer is governed by the terms of the chosen model, and the Approved-Models restriction that would bind it contractually is not configured. |
| Secret handling | The provider key is held in memory and never written to storage (decision 0020). Secrets are referenced indirectly in configuration, by environment variable name or file path, and never appear in it. |
| Access control | Group admission is a stored authorization written only by the operator's own invitation. Every other contact is refused without touching the ledger, and the assistant withdraws (decision 0052). |
| Availability and abuse | Two answering counters, per person across all chats and per chat, limit answering and never storage. An over-limit message draws silence, so a flooder cannot borrow the assistant's voice (decisions 0030, 0034). |
| Boundary discipline in the tools | The lookup palette is recorded per conversation and admission fails closed; tool authority is floored at member level; lookup failures are reported to the model and never to the chat (decisions 0041, 0044). |
| Bounded input into the system voice | Byte bounds on both surfaces the group controls, the rules text and the title, with over-bound text refused whole instead of truncated (decisions 0048, 0049). |
| No decisions about people | No decision capability ships. A member can ask the assistant to relay a report to the group's moderation bot; the assistant detects nothing, files nothing on its own, and the group's human administrators judge the report (added 2026-08-23; the warn and ban lines stay held out of the system prompt, decision 0046). Changed 2026-08-24: the assistant now files the report on its own assessment of the pinned rules — an assessment the human administrators judge; it still decides nothing about anyone (decision 0070), and each message is reported at most once. No automated decision with legal or similarly significant effect is taken. |
| Transparency | The group's pinned rules announce the assistant and point at the policy, and the privacy command answers deterministically, without a model turn, at most once per chat per window (decision 0053). |
| Erasure | The operation described in section 8, with its two recorded gaps. |
| Storage protection at rest | The store and the operator-provided credentials sit on an encrypted volume, unlocked by a passphrase entered at each boot and held nowhere on the machine. Deployment configuration; recorded as in place 2026-08-24. |

## 10. Open dependencies

Stated here exactly as the impact assessment states them, because this record must not
claim a measure that is not yet in place:

1. ~~**Storage protection at rest** for the message store.~~ **Closed 2026-08-24.**
   Required by the platform's developer terms and relied on by the breach mitigation in
   the impact assessment. The deployment holds the store on an encrypted volume: `/var`,
   which carries the message store and the operator-provided credentials, is a LUKS
   volume whose passphrase is entered at each boot and is never stored on the machine.
2. **The countersigned processor agreement** with Requesty Ltd, returned and on file. The
   terms are accepted and the clauses apply; what is outstanding is the signature
   round-trip, which the controller completes when able. Corrected 2026-08-24: an earlier
   revision of this entry stated that it must be complete before the main community
   group. That condition was never set by the controller and did not belong in this
   record.
3. **The Approved-Models restriction** under clause 5.5 of the processor agreement, which
   would bind the processor to a named set of models and therefore to their retention and
   training terms. Not configured today. Added 2026-08-23 with the correction that zero
   data retention binds the processor alone.
4. **No signed Article 28 instrument with the search provider**, Serper, is on file with
   the controller. Added 2026-08-29 with the web search: its published terms name the
   customer as controller and itself as processor where personal data is processed, and
   the adequacy decision for the United Kingdom covers the transfer, but no signed
   instrument is on file with the controller yet. Until one is, this record states the
   reliance as what it is.
5. **The non-EEA conversation-naming model**, the framework defect described in section 7,
   with its fix in flight. Closed 2026-08-23: title derivation is switched off entirely
   (decision 0077), so no naming request exists to cross anywhere.

## 11. Keeping this record current

Trigger fired and answered 2026-08-29: "any new path that sends message content off
the machine, including a new tool" — the web search is such a path. This record was
revised for it (sections 3, 6, 7, 9 and 10), and the impact assessment's addendum of
the same date carries the assessment.

Trigger fired and answered 2026-09-02: "a change to retention" — message history gained a
90-day expiry per conversation, recorded in decision 0198. The time limits in section 8
were revised for it (D1, D2, D3 and D11), and the impact assessment's addendum of the same
date carries the assessment.

This record is updated whenever the activity changes, and in any case on the review
triggers listed in the impact assessment: a new platform or a group beyond the project's
own, a change of processor, model provider, region or retention setting, a new path that
sends message content off the machine, a change to what is collected, any moderation
capability shipping, a change to retention or any secondary use, closure or discovery of an
erasure gap, a personal data breach, and twelve months since the date above with no other
trigger.
