# Legitimate interest assessment: the halogenOS Group Assistant

**Draft, not yet published.**

Date: 2026-08-23

The assessment behind the legal basis named in the privacy policy: Article 6(1)(f) GDPR,
legitimate interest, for storing and answering messages in the halogenOS community groups.
It runs the three tests the EDPB sets out, states the safeguards it counts on, and reaches
an outcome. It is a companion to the data protection impact assessment beside it; where
that document describes the processing in full, this one only weighs it.

Controller: Simão Gomes Viana, c/o IP-Management #10911, Ludwig-Erhard-Str. 18, 20459
Hamburg, Germany. Enquiries: privacy@halogenos.org.

Noted 2026-08-23: no data protection officer is appointed, and the reasoning is recorded
with the pre-check in section 1 of the impact assessment beside this document. § 38(1)
sentence 2 BDSG attaches its duty to processing that is objectively subject to an
assessment under Article 35, and that pre-check concludes Article 35 does not compel one
here. The assessment is written as a precaution, which does not create the duty. The
competent supervisory
authority is the Bayerisches Landesamt für Datenschutzaufsicht (BayLDA) in Ansbach, settled
by the operator 2026-08-23: the establishment is in Bavaria, and the Hamburg address in the
controller block is a mail-forwarding contact address only.

## 1. The processing being assessed

Storing every message in the project's own community groups, together with the sender's
platform identity and the circumstances of the message, and sending conversation text
without identity to a language model in order to answer questions addressed to the
assistant. Full description in the impact assessment; data categories and recipients are
not repeated here.

## 2. Purpose test: is there a legitimate interest?

Three interests, all our own and all lawful:

1. **Answering community questions.** halogenOS is an open-source project whose support
   happens in its chat groups. Maintainer time is the scarcest resource the project has,
   and the same questions arrive weekly. An assistant that answers them is a direct
   operational interest of the project and a direct benefit to the people asking.
2. **Answering them in context.** An answer that cannot see the conversation it is part
   of is worse than no answer, because it is confidently beside the point. Reading the
   discussion, including older discussion, is what separates the assistant from a search
   box.
3. **Keeping the assistant available and affordable.** Bounding how much the assistant
   answers per person and per chat protects both the community's access to it and the
   project's costs.

These are real, present and specific, not speculative. The Court of Justice confirmed in
its 2024 decision on a sports association's data sharing that ordinary operational and even
purely commercial interests can qualify, so a community project's interest in running its
own support channel clears this test comfortably. The interests are pursued lawfully: no
scraping of foreign groups, no dataset building, no training on the content, no secondary
use.

**Result: passed.**

## 3. Necessity test: is the processing needed to reach it?

The question is not whether the processing is convenient, but whether a less intrusive
route reaches the same purpose.

**Storing message content.** Purposes 1 and 2 require it. The routes considered:

- *Only messages that address the assistant.* Rejected: the community's questions point
  backwards constantly, and an answer built without the surrounding discussion misreads
  its own thread. This route defeats purpose 2 outright.
- *A short window held in memory, nothing persisted.* Rejected: it loses exactly the older
  context that makes the assistant useful, and a restart erases the group's memory with
  it.
- *Storing summaries instead of messages.* Rejected: a summary is a derived personal
  record too, it is less accurate, it cannot be corrected against the original, and
  deletion of it is harder to prove than deletion of a text column.

**Storing identity.** The platform account identifier is needed to attribute messages
inside a conversation, to apply the per-person answering counter, and, most importantly,
to make deletion possible at all: a request to delete one person's messages can only be
served if the messages can be linked to that person. The username is kept so a request can
be matched to an account and so the operator can act on abuse; the display name is not
stored as identity data (narrowed 2026-08-23, decision 0077). Both live in tables of their
own. Amended 2026-08-29: where a group announces that someone joined, the name that
announcement showed is stored once as the announcement's own content, in the join-notice
table — necessary because a joining account whose displayed name is itself an
advertisement is the offense, and the assistant cannot report what it did not record. It
is erased with the person like message text.

**Sending text and one handle to a model.** Answering is the purpose and a language model
is the mechanism. Since the operator's decision of 2026-08-23 the request carries the
public username of each speaker beside the text, because an assistant that answers in a
group has to be able to say which person it is answering, and the handle is how a group
addresses its members. The necessity is judged on that capability: without the handle the
model can write an answer, but not one that points at the person who asked, and in a busy
group that produces answers nobody can attach to their question. Everything else stays
back. The numeric account identifier does not cross, no display name is stored beside a
message or sent as an attribute of one (narrowed 2026-08-23, decision 0077; amended
2026-08-29: the name a recorded join announcement showed does travel, as that
announcement's content in the projected join line), and no history
beyond the conversation being answered is uploaded. The processor keeps nothing, and a
model provider behind it keeps what the terms of the chosen model allow, which is why the
choice of model belongs in this assessment and not only in the impact assessment. The
transmitted identifier is the one the group already sees on every message, which is why
the exchange is proportionate and not a widening of who knows what about the person.

**Keeping history without a timer.** The one point where necessity deserves real
scepticism, and it is assessed on its own in section 4 of the impact assessment. In short:
a retention window would delete the whole community's memory on a schedule to reach the
small part one person wanted removed, would not remove the need for deletion on request,
and would take away purpose 2. The absence of a timer is necessary for purpose 2 only in
combination with an effective deletion path, which is why the two decisions were taken
together.

**Recording a member's edit is necessary, added 2026-08-31.** Storing only the first
version means holding a record of what a person said that the person has already
corrected, and answering a question its author has already withdrawn — which purposes 1
and 2 are both worse for, and which the accuracy principle in Article 5(1)(d) argues
against. So each distinct version is stored beside the first, and the assistant reads the
later one as what the person now means. The bound is stated honestly rather than
overstated: every distinct version a person writes is kept, decision 0003 sets no
retention timer, and the added volume is not bounded. What the drops remove is only the
platform's own repeated deliveries of unchanged text — a link preview attaching to a
message nobody touched — never a version a person wrote.

**What is not necessary, and therefore not done.** Media, files and voice are not stored.
Anonymous administrator posts and channel forwards are skipped.
No profile of any member is built. No message is used to train anything. No third party
receives the history.

**Result: passed, on the condition that the deletion path stays effective.**

## 4. Balancing test: do the members' rights override it?

### 4.1 What is actually being processed

Messages people chose to post to an open community group, in front of every other member.
This is not private correspondence, not observed behavior, not data collected behind
someone's back, and not data enriched from other sources. Direct chats with the assistant
are the more sensitive part of the set, and they are the part deletion removes whole.

Free text can still carry special-category content by accident. That case is addressed
separately in section 5, because Article 6 alone does not answer it.

### 4.2 What members can reasonably expect

The strongest factor in favor. The assistant is announced in the group's pinned rules
before anyone writes a word, the platform itself labels a bot with message access in the
member list, the privacy pointer is answerable on demand with the privacy command, and the
policy states in plain words that everything in the group is stored and for how long. A
person joining a project's support group where an announced assistant answers questions
expects to be read by that assistant. Nobody is surprised.

Two expectation edges are recognised instead of argued away. Members who were in a group
before the assistant arrived did not join under that announcement, and the rules pin plus
the announcement message is what reaches them. And an open group has passers-by who read
the room before reading the pin. Both are reasons the notice has to be visible in the
group and not only on a website, which is why the pinned rules carry it.

### 4.3 The impact on a member

- **No decisions.** The assistant grants nothing, refuses nothing and takes no moderation
  action of its own. Nobody's standing in the community depends on it.

  > Amended 2026-08-23: the report feature lets a member ask the assistant to relay a
  > report to the group's moderation bot. A member starts it and the group's human
  > administrators judge it, and the assistant can neither warn, remove nor ban. Refined
  > later the same day, on the public policy's audit: the relay itself is produced by the
  > same language model that writes the answers, so it can misfire and report a message
  > nobody meant. That is a real effect on a member, and it is public in the group the
  > moment it happens, carries no sanction from the assistant, and reaches human judgment
  > either way. The balancing above holds on those three facts, not on a claim that
  > nothing is decided. The impact assessment's addendum carries the detail.

  > Amended 2026-08-24: the assessment is now the assistant's own — it judges each
  > group message against the pinned rules and reports a clear violation itself;
  > member-initiated reporting is removed. The three facts the balancing holds on are
  > unchanged: a report is public where it happens, carries no sanction from the
  > assistant, and reaches human judgment — the group's administrators decide, and each
  > message is reported at most once. The impact assessment's second addendum carries
  > the detail, the false-positive residual included.
- **No profiling, no scoring, no targeting.** The counters count messages in a window and
  nothing else.
- **No new audience.** The people in the group already saw the message and the handle
  above it. The new reader is a model reached through a processor that retains nothing,
  and it receives no more than the group did. Corrected 2026-08-23: the processor's
  retention promise does not bind the model provider behind it, whose own retention
  follows the terms of the chosen model.
- **No commercial use.** Nothing is sold, shared, advertised against or analysed.
- **The real impact** is that a remark keeps existing on a project server after the person
  stopped thinking about it, and that it can resurface in an answer months later. That is
  a genuine effect on how freely people speak, and it is the reason the deletion path has
  to be as complete and as fast as it is.

### 4.4 The imbalance question

There is no dependency relationship: nobody needs the groups to work, study or receive a
service, membership is voluntary and reversible, and the project has no power over any
member. There is one asymmetry: the operator is also the group administrator, so a member
who dislikes the arrangement is asking the same person who runs the assistant. The answer
to that is external, and it is stated in the policy: a complaint goes to a supervisory
authority, to the Bayerisches Landesamt für Datenschutzaufsicht for this controller, or to
the authority where the member lives or works. The competence follows the establishment in
Bavaria, settled by the operator 2026-08-23.

### 4.5 Children

The community groups are open and no age check exists on the platform. Minors are
therefore among the people concerned, which raises the standard, and the response is the
one Recital 38 points at: no profiling, no marketing, no decisions, notice in plain
language, and a deletion path a young person can use with one message and no formalities.

### 4.6 Weighing

The interests are real and modest; the data is what people already published to a group;
expectations are set before collection and reinforced in the group itself; there are no
decisions, no profiling, no secondary use and no new audience; the objection and deletion
paths are real and cheap to use. Against that stands one genuine effect, the persistence
of what was said, answered by deletion on request and by the honesty of stating the
absence of a timer instead of dressing it as "as long as necessary".

**Result: the interests are not overridden. The basis holds.**

## 5. Article 9: the condition claimed, and the part it does not reach

Legitimate interest is a basis under Article 6. Special-category data under Article 9 needs
a condition of its own. Rewritten 2026-08-23 on a fact the operator settled: the community
groups are public in the strong sense. Anyone on the platform can open them and read every
message without joining, without approval and without anybody's permission. Posting can
require passing the group's entry check; reading never does. On that fact the question
splits in two, and only one part is claimed.

**What a person posts about themselves: Article 9(2)(e).** Someone who writes about their
own health, belief, politics or private life into a venue the general public can read has
manifestly made that data public by their own deliberate act, which is what Article 9(2)(e)
asks for. The condition is claimed for that content, and the processing described in this
assessment rests on it together with Article 6(1)(f).

*The narrow-reading risk, named.* The Court of Justice reads the exception restrictively in
C-252/21 and looks for a deliberate act of making public, not merely for data that ended up
visible. The claim here stands on one verified fact and nothing else: these groups are
readable by the general public. If a group is ever closed, made approval-only or
invite-only, the act of posting into it stops being publication to the public and the claim
falls with it. That change is on the review-trigger list of the impact assessment, and this
section is re-taken with it.

**What one person writes about another: the residual.** Member A posts something sensitive
about member B. B published nothing, so Article 9(2)(e) cannot reach it, and no other
condition in Article 9(2) fits either. The position is stated carefully, because it is easy
to state badly. Publishing another person's special-category data into a public group is
A's own act and A's own processing, and where it is unlawful it is A's wrong. The assistant
does not invite it, does not ask for it, cannot recognise it and gains nothing from it: the
exposure is incidental, unsought and undetectable, and it is introduced by a third party's
act into a conversation the assistant only reads.

What the controller owes for it is reactive, and it is honored. An erasure request reaches
everything of the requesting person's own, immediately and by mechanism. A request about
somebody else's message is answered by a person inside the month, on the same rights. An
objection under Article 21 is honored the same way today, and by machine once the
self-service unit ships. Since 2026-08-23 such content also travels to the processor with
the speaker's handle beside it, attributable for the life of the request and no longer.

The result is a residual that is minimised, not a condition that is claimed. It is
carried as accepted residual risk in the impact assessment, at the narrower scope this
section now gives it.

## 6. Why not consent

Consent was considered and rejected, and the reasons are kept written down here:

- Consent required to enter a group is not freely given. Tying access to a permission for
  processing that access does not require makes the permission presumptively invalid.
- Consent must be withdrawable as easily as it is given. One member withdrawing cannot
  stop a group's conversation from being stored, so the promise would be false the day it
  was made.
- Consent does not reach the members who were already in the group.
- A basis that collapses for the first person who withdraws is not a basis to build a
  community service on.

The rules pin therefore delivers the notice. It never collects consent.

## 7. Safeguards this outcome depends on

The outcome above is conditional. Remove any of these and the balance has to be weighed
again:

1. Personal data stored apart from the ledger, and deletion that empties the message text,
   the send time and the reply reference, removes direct conversations whole and deletes
   the identity rows (decisions 0003, 0006, 0011, 0012, 0027, 0028).
2. Exactly one identifier in provider requests, the public username, and no more: the
   numeric account identifier stays on the machine, no display name is stored beside a
   message or attached to a request as identity data (decision 0077; amended 2026-08-29:
   a recorded join announcement's shown name travels as that event's content), and
   nothing is added to a request without weighing this assessment again.

   > Re-weighed 2026-08-29, with the web search. The obligation in the last clause
   > fired: the search sends member-derived text to a place it did not go before.
   > The weighing was performed and its outcome is recorded here instead of
   > assumed. What crosses to the search provider is the QUERY and nothing else —
   > no account identifier, no username, no other part of the conversation — so
   > this safeguard's own subject, the identifiers in a request, is not weakened;
   > the search boundary is narrower than the model provider's, not wider. A query
   > carrying a person reference in the handle form is refused whole before
   > anything is sent, by a guard that does not echo what it matched (decision
   > 0115). The safeguard therefore stands as written, with the search boundary
   > named beside it.

   > Re-weighed 2026-08-29, with the standing lookup. The obligation in the last
   > clause fired a second time: an attribute of a person that had never left the
   > machine now reaches a request. The weighing was performed and its outcome is
   > recorded here instead of assumed. What is added is not an identifier — the
   > identifiers in a request are still the public username and nothing else — but
   > one attribute, a person's administrator standing, sent as the fixed answer to a
   > lookup the model made and never as a field beside a message. Three things keep
   > the balance: the fact is one the group's own member list shows anybody who
   > opens it, the lookup answers only for a handle the conversation already showed
   > and only in a group, and it is what lets the assistant tell a member who claims
   > authority from a member who holds it, which is a protection for the members
   > whose group would otherwise be steered by a claim. A person whose data was
   > erased is not found, because the match is on the handle erasure removes. The
   > safeguard therefore stands as written, with the standing answer named beside
   > it, and the obligation continues to bind whatever is added next.

3. The processing chain stays as assessed: a processor in the United Kingdom under an
   Article 28 agreement with standard contractual clauses, storing and serving in
   Frankfurt, with the United Kingdom's adequacy decision covering the processor
   relationship itself, zero data retention at the processor, no training there, and
   sub-processors engaged by that processor under its own agreements. Corrected
   2026-08-23: zero data retention binds the processor alone, so the terms of the chosen
   model decide the provider layer, and a model deployment outside the EEA rests on the
   standard contractual clauses. The conversation-naming step reached such a deployment
   through a framework defect whose fix was in flight; closed 2026-08-23 by decision
   0077 — title derivation is off entirely, and no naming request exists.

   > Re-weighed 2026-08-29, with the web search. The chain changed: a SECOND
   > processor joined it, the search provider, receiving the query alone. It is a
   > company in the United Kingdom, so the transfer rests on the same adequacy
   > decision under Article 45 GDPR that already covers the model processor, with
   > no further safeguard required; its terms name the customer controller and
   > itself processor where personal data is processed. The weighing performed
   > with this unit finds the balance unchanged: the addition is one bounded,
   > adequacy-covered recipient of a query, weighed against members getting an
   > answer to a question the project's own sources cannot answer. The signed
   > Article 28 instrument with that provider is not yet on file and is carried as
   > an open dependency in the record of processing, section 10 — stated, not
   > assumed. The impact assessment's addendum of the same date carries the full
   > assessment, and this safeguard continues to bind: a further change to the
   > chain reopens this weighing again.
4. Notice in the group before collection: the rules pin and the deterministic privacy
   command (decision 0053), plus the platform's own policy field.
5. Objection and deletion answered within a month, free, with no more identity checking
   than needed, and a request from the account itself accepted as proof of who is asking.
6. No profiling, no secondary use, no sharing of the history, no training.
7. No moderation capability without a fresh assessment (decision 0046 holds those lines
   out today).

   > Amended 2026-08-23: the report capability shipped WITH its assessment — the
   > impact assessment's addendum of the same date — so this safeguard is met, not
   > removed. The condition continues to bind the held-out warn and ban lines.

   > Amended 2026-08-24: the autonomous assessment shipped WITH its assessment the
   > same way — the impact assessment's addendum of that date. The condition
   > continues to bind the held-out warn and ban lines.
8. Group admission restricted to the operator's own invitation, fail-closed (decision
   0052).

## 8. Handling an objection

An objection under Article 21(1) needs no reason beyond the person's own situation, and it
is handled like this: the objection is acknowledged, the person's data is erased through
the standard path unless we can show compelling legitimate reasons that outweigh their
interest, and the answer goes out within a month in plain words. Where something cannot be
done at all, for example continuing to store a group conversation the person keeps writing
in, the answer says so instead of promising a mechanism that does not exist, and it says
what the alternatives are. Objections and their outcomes are recorded, and a pattern of
them is a trigger to reassess this document.

## 9. Outcome

The processing may proceed on Article 6(1)(f). This assessment is reviewed on the same
triggers as the impact assessment, and in particular whenever the safeguards in section 7
change, when a moderation capability ships, when the processor or the region changes, or
twelve months from the date above.
