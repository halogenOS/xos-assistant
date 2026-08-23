# Privacy Policy — the halogenOS Group Assistant

This policy explains how personal data is processed when you write in a halogenOS chat
group the assistant belongs to, or write to it directly. Provided pursuant to Articles 13
and 14 GDPR.

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
including the caption under a picture or a file. The media itself is not, edits are not
collected, and a post made anonymously for the group is skipped unless it is the pinned
rules. Direct chats are stored the same way, as are the group's title and pinned rules,
which can name a person, and every lookup the assistant makes in the project's public
sources with the result it received.

The assistant does not look for sensitive content and cannot recognise it, so whatever a
message carries is stored and sent on like any other text. If a stored message concerns you
and you want it gone, whether you wrote it or not, it goes on request: your own by the route
below, somebody else's after review by a person.

_Purpose:_ answering questions in groups and direct chats, reading the conversation for
context, limiting how much the assistant answers per person and per chat, and passing on a
report when a member asks for one.
_Legal basis:_ Article 6(1)(f) GDPR, our legitimate interest in running an assistant in our
own community groups. We ask for no consent, and the group rules are not a consent form:
consent required to enter a group would not be free, and one member's withdrawal could not
stop the group's conversation from being stored. You can object at any time, as set out
below.

**Who wrote them**

Stored with each message: your username, your display name and the numeric account
identifier your chat platform assigns you, held in tables of their own so that a deletion
request can remove them. Beside it: the platform's send time and the time it reached us,
which message it replied to, whether it was addressed to the assistant, whether you were an
administrator then, and whether an answer was due or a limit refused one. The identity data
and your administrator standing come from the chat platform, the title and rules from the
group, a report from the member who asked for it. Nothing about you is taken from anywhere
else.

_Purpose:_ attributing messages, addressing you in an answer, letting the operator act on
abuse, and making deletion possible at all.
_Legal basis:_ Article 6(1)(f) GDPR.

**What is sent to the language model**

To write an answer, the assistant sends the conversation to a language model through a
processor. What goes out with the request:

- the text of the messages in that conversation
- the public username of each writer, so the assistant can address you by your handle
- the maintainers' instructions, the group's title and its pinned rules
- what the assistant looked up, and its own earlier answers in that conversation

The whole stored conversation goes every time: an answer that cannot see the thread misreads
it. Your display name and account identifier stay with us. Once per new conversation, a
short piece of it also goes to a smaller model, which returns a few words naming it.

_Purpose and legal basis:_ as under Messages. The sending is how an answer gets written.

**What we do not do**

No advertising, no analytics, no tracking, no profiling. Nothing you write is sold, rented
or passed on for marketing, and nothing trains a language model, by us or by our processor.
No automated decision within the meaning of Article 22 GDPR is made about you. The assistant
does not moderate: it cannot warn, remove or ban anybody, and it watches nobody. It can pass
a report to the group's moderation bot when a member replies to a message and asks for one,
and the group's administrators decide what happens. That relay is written by the same
language model that writes the answers, so it can misfire and report a message nobody meant;
it goes out as a public reply, so you see it. The reported message's identifier is stored
with the report and emptied when the reported person is deleted. Answers are model-written
and can be wrong.

**Recipients and processors**

| Recipient | Role | Basis |
|-----------|------|-------|
| Requesty Ltd, London, United Kingdom | Passes the conversation to a language model and returns the answer. Requests enter through its European endpoint, and what it stores it stores in Frankfurt. Zero data retention is configured: no message and no answer reaches storage or training. It keeps billing telemetry without content: token counts, the model identifier, a timestamp. | Processor, Article 28 GDPR |
| Sub-processors engaged by Requesty | Two layers. Amazon Web Services in Frankfurt runs the infrastructure, with Requesty answerable to us for it. The model providers it routes to, Google today, answer to Requesty, which is responsible for the choice and the written terms but not for a provider's own breach. Zero data retention binds Requesty and not a model provider: whether a provider keeps a request or trains on it follows the model's terms. | Sub-processors, Article 28(2) and (4) GDPR |
| Your chat platform | Delivers and stores the same messages as part of its own service, under its own privacy policy. It does not act for us. | Independent controller |
| The other people in the group | See your messages as they always have | Your own act of posting |

Messages are stored on a server in Germany. Data leaves the EU/EEA in three places:

- Our processor is a UK company, though it stores and serves in Frankfurt. The European
  Commission has decided the United Kingdom offers adequate protection, so no further
  safeguard is needed.
- A model deployment outside the EEA receives the request there, as the smaller naming model
  does today, under the standard contractual clauses in the processor agreement
  (Article 46(2)(c) GDPR). Write to the address above for a copy.
- Your chat platform is outside the EU/EEA and receives every message and answer as it
  delivers them, as its own controller.

**Retention and deletion**

Messages are kept until somebody asks for them to be deleted. There is no automatic expiry,
deliberately: the assistant answers questions about discussions from months ago, and a
schedule would delete everyone's history to reach the part one person wanted gone. The rest
follows the message: identity data and circumstances live as long as it does; the title and
pinned rules stay while the assistant serves the group, each replaced by the next; answer
counters fall out of their window; a report is kept with its message reference emptied when
the reported person is deleted; lookup records have no time limit. Deleting a message in
your chat app does not reach us, and neither leaving a group nor the assistant leaving it
deletes anything: ask, and it goes. If the service stops for good, the store goes with it.

Deletion takes the text of every message you wrote, its send time and reply reference, your
whole direct conversation with the assistant, and your identity data. What remains in a
group is an empty placeholder holding a position in the conversation.

Some things stay, and it is fairer to name them. The assistant's own answers stay, and one
can carry your handle and repeat what you asked. On your message rows, the store's internal
number, the arrival time and your administrator standing stay; once your identity data is
gone that number names nobody, though it still ties those messages together. Our processor
has nothing to delete, since it keeps no message, and a model provider behind it may keep a
request under its own terms, beyond our reach. Deletion also leaves lookup records, the
title and pinned rules text, and the copy of your message's identifier on somebody else's
reply where it matches nothing of yours; a query or a rules text can quote your words or
name you. If you opted out, your account identifier with the opt-out mark survives on
purpose, because forgetting it would mean collecting your messages again.

**Your rights**

You have the right to confirmation and access (Article 15), rectification (Article 16),
erasure (Article 17) and restriction (Article 18). Portability under Article 20 does not
apply: it covers data processed on consent or for a contract, and everything here rests on
legitimate interest.

Deletion and objection also work in the group. `/privacyout` stops the assistant collecting
and answering your messages from that moment; `/privacydelete`, confirmed with
`/confirmdelete`, removes your stored data; `/unblockprivacy` turns collection back on. They
act on the sending account immediately, by machine, because you asked and confirmed. Plain
words work too: tell the assistant to stop collecting or to delete, and it honors that the
same way.

For anything else, or if you prefer a person, write to
[privacy@halogenos.org](mailto:privacy@halogenos.org) with your username and the group. A
handle is visible to everyone, so we may ask you to show the request comes from that
account, and nothing more. Requests are free and answered within one month. Mail is answered
by a person; the commands are the one place a machine acts, and only on your own confirmed
instruction.

**Your right to object**

You can object at any time to any processing described here, for reasons that come from your
own situation, by mail or with the commands above. We then stop unless we can show
compelling legitimate reasons that override your interests, rights and freedoms, or we need
the data for legal claims. In practice no weighing takes place: `/privacyout` stops
collection from that moment, deletion removes what came before, and neither asks for a
justification.

**Right to lodge a complaint**

You may complain to a supervisory authority, in particular where you live, where you work,
or where the alleged infringement took place. The authority competent for this controller
is:

Bayerisches Landesamt für Datenschutzaufsicht (BayLDA)\
Promenade 18, 91522 Ansbach\
[https://www.lda.bayern.de](https://www.lda.bayern.de)

**Changes**

This policy may be updated as the assistant changes. The current version is always at this
address, and the assistant answers `/privacy` with it.

Last updated: 23 August 2026
