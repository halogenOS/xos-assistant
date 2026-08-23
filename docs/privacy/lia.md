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
served if the messages can be linked to that person. Display name and username are kept so
a request can be matched to an account and so the operator can act on abuse. All three live
in tables of their own.

**Sending text and one handle to a model.** Answering is the purpose and a language model
is the mechanism. Since the operator's decision of 2026-08-23 the request carries the
public username of each speaker beside the text, because an assistant that answers in a
group has to be able to say which person it is answering, and the handle is how a group
addresses its members. The necessity is judged on that capability: without the handle the
model can write an answer, but not one that points at the person who asked, and in a busy
group that produces answers nobody can attach to their question. Everything else stays
back. The numeric account identifier and the display name do not cross, no history beyond
the conversation being answered is uploaded, and the recipient keeps nothing. The
transmitted identifier is the one the group already sees on every message, which is why
the exchange is proportionate and not a widening of who knows what about the person.

**Keeping history without a timer.** The one point where necessity deserves real
scepticism, and it is assessed on its own in section 4 of the impact assessment. In short:
a retention window would delete the whole community's memory on a schedule to reach the
small part one person wanted removed, would not remove the need for deletion on request,
and would take away purpose 2. The absence of a timer is necessary for purpose 2 only in
combination with an effective deletion path, which is why the two decisions were taken
together.

**What is not necessary, and therefore not done.** Media, files and voice are not stored.
Edits are not collected. Anonymous administrator posts and channel forwards are skipped.
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
separately in section 5, because legitimate interest does not answer it.

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
  > report to the group's moderation bot. The assistant still decides nothing — a
  > member starts it, the group's human administrators judge it — so the balancing
  > above holds; the impact assessment's addendum carries the detail.
- **No profiling, no scoring, no targeting.** The counters count messages in a window and
  nothing else.
- **No new audience.** The people in the group already saw the message and the handle
  above it. The only new reader is a model behind a processor that retains nothing, and it
  receives no more than the group did.
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
authority, in Hamburg or where the member lives.

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

## 5. The point this basis does not cover

Legitimate interest is a basis under Article 6. Special-category data under Article 9
needs an exception of its own, and none applies cleanly to a health remark someone drops
into a technical conversation: posting in an open group does not by itself make data
manifestly public in the sense of Article 9(2)(e). This assessment does not pretend
otherwise. What answers it, imperfectly and knowingly, is the structure around it: no
detection and therefore no targeting of such content, a recipient that retains nothing and
builds no profile, and deletion that reaches the text quickly on request. Since 2026-08-23
such a remark travels to the processor with the speaker's handle beside it, which makes it
attributable for the life of the request and no longer. That is a real increase in the
exposure and it is recorded as such: accepted residual risk in the impact assessment, not
a solved problem.

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
   display name and the numeric account identifier stay on the machine, and nothing is
   added to a request without weighing this assessment again.
3. Processing stays inside the EU, at a processor under an Article 28 agreement with
   standard contractual clauses, with zero data retention, no training on the content, and
   sub-processors engaged by that processor under its own agreements and its own answer
   for them.
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
