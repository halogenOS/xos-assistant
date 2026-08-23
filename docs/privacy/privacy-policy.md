# Privacy Policy — the halogenOS Group Assistant

This policy explains how personal data is processed when you take part in a halogenOS chat
group that the halogenOS Group Assistant is a member of, or when you write to the assistant
directly. It is provided pursuant to Articles 13 and 14 GDPR.

**Controller**

Simão Gomes Viana\
c/o IP-Management #10911\
Ludwig-Erhard-Str. 18\
20459 Hamburg\
Germany

Data protection enquiries: [privacy@halogenos.org](mailto:privacy@halogenos.org)

**What we process, why, and on what basis**

**Messages**

Everything written in a group the assistant belongs to is stored: the text of each message,
including the caption under a picture or a file. The pictures, files, voice messages,
stickers and videos themselves are not stored. Later edits are not collected, so a message
is kept as it was first received. Messages posted anonymously on behalf of the group are
not stored, because the platform does not disclose who wrote them. The one thing kept from
such a post is the text of the group's rules when it is pinned.

Messages you send to the assistant in a direct chat are stored the same way.

The group's title and the rules text pinned in it are stored as the assistant's notes about
the group. A rules text can name a person.

When the assistant looks something up in the project's public sources to answer, the query
it sent and the result it received are stored with the conversation.

Free text carries things Article 9 GDPR treats as special: a health remark, a belief,
something about a person's private life, dropped into a technical conversation. The
assistant does not look for such content and cannot recognise it, so it is stored and sent
on like any other text. Legitimate interest is a basis under Article 6. Article 9 asks for a
condition of its own, and none of the conditions in Article 9(2) fits a remark of that kind:
posting in an open group does not make it manifestly public in that sense. We say so plainly
instead of claiming a condition we do not have. What stands in its place is the rest of the
structure: nothing of the kind is sought, detected or singled out, no profile is built from
it, and deletion on request reaches the text.

_Purpose:_ answering questions in the group and in a direct chat, reading enough of the
conversation to answer in context, keeping the assistant available by limiting how much it
answers per person and per chat, and passing on a report when a member asks for one.
_Legal basis:_ Article 6(1)(f) GDPR (legitimate interest in running an assistant in our own
community groups). We do not ask for consent and the group rules are not a consent form:
consent you would have to give in order to enter a group would not be a free choice, and
one member withdrawing it could not stop a group conversation from being stored. You can
object to processing based on legitimate interest at any time; the block on that right,
below, says how.

**Who wrote them**

Stored with each message: your username, your display name, and the numeric account
identifier your chat platform assigns you. These are held apart from the message text, in
tables of their own, so that a deletion request can remove them.

Stored alongside: the time the platform gives for the message and the time it reached us,
which message it replied to, whether it was addressed to the assistant, whether you were an
administrator of that group at the time, and whether an answer was due or a limit refused
one.

Where it comes from: your username, your display name, your account identifier and your
administrator standing come from the chat platform, not from anything you wrote. The group's
title and its pinned rules come from the group. A report about a message comes from the
member who asked for it.

_Purpose:_ attributing messages inside a conversation, addressing you in an answer, counting
how much the assistant has answered you and the chat so that it stays available to
everybody, letting the operator act on abuse, and making deletion on request possible at
all.
_Legal basis:_ Article 6(1)(f) GDPR.

**What is sent to the language model**

To write an answer, the assistant sends the conversation to a language model through a
processor. What goes out with the request:

- the text of the messages in that conversation
- the public username of each person who wrote one, so that the assistant can address you
  the way the group does, by your handle
- the instructions the maintainers wrote for the assistant, the group's title and its pinned
  rules text
- what the assistant looked up for that answer, and the answers it wrote earlier in the same
  conversation

The whole stored conversation goes with every request, not only the last few messages,
because an answer that cannot see the thread misreads it. Your display name and your numeric
account identifier stay with us and are not sent. Nothing else about you is attached. A
direct chat is sent the same way, and nobody but you and the assistant ever saw it.

Once per new conversation, a short piece of it also goes to a smaller model, which returns a
few words naming that conversation.

The special content described under Messages travels with the rest, under the same basis and
with the same open point: Article 6(1)(f) covers the sending, and Article 9 has no condition
we can name for it.

_Purpose:_ producing an answer that fits the conversation and can address people by name,
and naming the conversation.
_Legal basis:_ Article 6(1)(f) GDPR.

**What we do not do**

No advertising is displayed. No web analytics, tracking or profiling takes place. Nothing
you write is sold, rented or passed on for marketing. Nothing you write is used to train a
language model, by us or by our processor; what the model provider behind the processor does
is set out in the table below. The assistant builds no profile of anybody and makes no
automated decision within the meaning of Article 22 GDPR. It does not moderate: it cannot
warn, remove or ban anybody, and it watches nobody. It can pass a report to the group's
moderation bot when a member replies to a message and asks for one, and the group's
administrators decide what happens. That step is written by the same language model that
writes the answers, so it can go wrong and report a message nobody meant; the report is a
public reply in the group, so you see it when it happens. The reported message's identifier
is stored with the report, so the reported person's deletion request empties that reference
too. Its answers are written by a language model and can be wrong.
Treat them as a helpful member's answer and not as an official statement of the project.

**Recipients and processors**

| Recipient | Role | Basis |
|-----------|------|-------|
| Requesty Ltd, London, United Kingdom | Passes the conversation to a language model and returns the answer. Requests enter through its European endpoint, and what it stores it stores in Frankfurt, Germany. Zero data retention is configured, so it writes no message and no answer to storage and uses none of it for training. It keeps billing telemetry that carries no content: token counts, the model identifier, a timestamp. | Processor, Article 28 GDPR |
| Sub-processors engaged by Requesty | Two layers. The infrastructure it runs the service on, Amazon Web Services in Frankfurt, for which Requesty stays answerable to us. And the model providers it routes to, Google today, for which Requesty answers for the choice, for the written terms and for reporting that provider's published position accurately, but not for that provider's own breach of it. Zero data retention binds Requesty and not a model provider: whether a provider keeps a request or trains on it follows the terms of the model chosen. | Sub-processors, Article 28(2) and (4) GDPR |
| Your chat platform | Delivers and stores the same messages as part of its own service, under its own privacy policy. It does not act for us. | Independent controller |
| The other people in the group | See your messages as they always have | Your own act of posting |

Messages are stored on a server in Germany. Data still leaves the EU/EEA in three places,
and this is what covers each:

- Our processor is a company in the United Kingdom, although it stores and serves in
  Frankfurt. The European Commission has decided that the United Kingdom offers adequate
  protection, so that transfer needs no further safeguard.
- Where a model deployment sits outside the EEA, the request reaches it there. The smaller
  model that names a conversation is one such case today. Those transfers rest on the
  European Commission's standard contractual clauses, which the processor agreement carries
  as the safeguard under Article 46(2)(c) GDPR. Write to the address above for a copy.
- Your chat platform is outside the EU/EEA and receives every message and every answer as
  part of delivering them, under its own policy and as its own controller.

**Retention**

Messages are kept until somebody asks for them to be deleted. There is no automatic expiry,
and that is deliberate: the assistant answers questions about discussions from weeks or
months ago, and deleting everyone's history on a schedule would take that away while doing
nothing for the person who actually wants their own words gone. What protects you is
deletion on request.

The rest, in the same plain terms:

- who wrote a message, and the circumstances stored beside it: kept as long as the message
- the group's title and its pinned rules text: kept while the assistant serves the group,
  and a rules text is replaced when new rules are pinned
- the counters that limit answering: they fall out of their window as time passes
- a report: kept, with the message reference emptied when the reported person is deleted
- records of the assistant's lookups: kept with no time limit, and not reached by deletion

Deleting a message in your chat app does not reach us. The platform does not tell the
assistant, so our copy stays until you ask. Leaving a group deletes nothing either, and
neither does the assistant leaving it: ask, and it goes. If the service stops for good, the
store is deleted with it.

**Deletion, and what it does**

Ask, and this goes: the text of every message you wrote, the platform's send time and the
reply reference stored with it, your whole direct conversation with the assistant, and the
identity data described above. What remains in a group is an empty placeholder holding a
position in the conversation, with none of your words in it.

Some things stay, and it is fairer to name them. The answers the assistant wrote stay, and
an answer can carry your handle and repeat what you asked. On a message row of yours, the
internal number the store uses, the time the message reached us and whether you were an
administrator at the time stay too; once your identity data is gone, that number names
nobody, but it still ties those messages to one another.

Our processor has nothing to delete, because zero data retention is configured and it keeps
no message. A model provider behind it can keep a request under its own terms, and we cannot
delete anything there.

Three things deletion does not reach: records of the assistant's lookups, which hold the
query and its result; the group's title and pinned rules text, which are stored with no link
to any person; and, on somebody else's reply to you, a stored copy of your message's
identifier in the cases where it no longer matches anything of yours. A query or a rules
text can quote your words or name you.

**Your rights**

Under the GDPR you have the right to obtain confirmation as to whether your data is
processed and to access it (Article 15), to rectification (Article 16), to erasure
(Article 17), and to restriction of processing (Article 18). Data portability under
Article 20 does not apply here: it covers data processed on consent or for a contract, and
everything described here rests on legitimate interest.

To exercise any of these, write to
[privacy@halogenos.org](mailto:privacy@halogenos.org). Tell us your username and the group,
so that your data can be found. A handle is visible to everyone in the group, so we may ask
you to show that the request comes from that account before we act on it, and we ask for
nothing beyond that. Requests are free and are answered within one month. Every request is
answered by a person, and no decision here is automated.

**Your right to object**

You can object at any time to any of the processing described here, for reasons that come
from your own situation. Everything here rests on legitimate interest, so an objection can
be raised against all of it. Write to
[privacy@halogenos.org](mailto:privacy@halogenos.org).

We then stop processing your data unless we can show compelling legitimate reasons that
override your interests, rights and freedoms, or unless we need it for legal claims. In
practice an objection is answered by deleting your data on the path described above. One
thing an objection cannot do is keep the group's conversation from being stored while you
keep writing in it: each new message is stored as it arrives. Where that is the case we say
so, and we say what we can do instead.

**Right to lodge a complaint**

You may lodge a complaint with a supervisory authority, in particular in the Member State
of your habitual residence, place of work, or the place of the alleged infringement. The
authority competent for this controller is:

Bayerisches Landesamt für Datenschutzaufsicht (BayLDA)\
Promenade 18, 91522 Ansbach\
[https://www.lda.bayern.de](https://www.lda.bayern.de)

**Changes**

This policy may be updated as the assistant changes. The current version is always
available at this address, and the assistant answers the `/privacy` command with it.

Last updated: 23 August 2026
