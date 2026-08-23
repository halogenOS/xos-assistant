# Privacy Policy — the halogenOS Group Assistant

We run the halogenOS Group Assistant in halogenOS chat groups. This notice is given under
Articles 13 and 14 GDPR.

**Controller**

Simão Gomes Viana\
c/o IP-Management #10911\
Ludwig-Erhard-Str. 18\
20459 Hamburg\
Germany

Data protection enquiries: [privacy@halogenos.org](mailto:privacy@halogenos.org)

**Processing**

**Messages**

We store the text of each message in a group the assistant belongs to, including the
caption under a picture or a file, the group's pinned rules, which can name a person, and
the assistant's lookups with their results. We do not store the media itself, edits, or
posts made anonymously for the group, except a pinned rules text. We do not serve direct
chats: a direct message is rejected and not stored.

We do not look for sensitive content and cannot recognise it: whatever a message carries,
we store and send on like any other text. We delete a stored message that concerns you on
request, whether you wrote it or not, your own through the route below and somebody else's
after a person reviews it.

_Purpose:_ answering questions in the groups, reading the conversation for
context, limiting how much the assistant answers per person and per chat, and passing on a
report when a member asks for one.
_Legal basis:_ Article 6(1)(f) GDPR, our legitimate interest in running an assistant in our
own community groups. You can object at any time, see below.

**Message author**

We store your username and the numeric account identifier your chat platform assigns you,
in tables of their own so that a deletion request can remove them. We do not store your
display name. We
store beside each message the platform's send time and the time it reached us, which
message it replied to, whether it was addressed to the assistant, whether you were an
administrator then, and whether an answer was due or a limit refused one. We take the
identity data and your administrator standing from the chat platform, the rules from the
group, a report from the member who asked for it. We take nothing about you from anywhere
else.

_Purpose:_ attributing messages, addressing you in an answer, letting the operator act on
abuse, and making deletion possible at all.
_Legal basis:_ Article 6(1)(f) GDPR.

**Language model**

We send the conversation to a language model through a processor, and each request carries:

- the text of the messages in that conversation
- the public username of each writer, so the assistant can address you by your handle
- the maintainers' instructions and the group's pinned rules
- what the assistant looked up, and its own earlier answers in that conversation

We send the whole stored conversation every time, because an answer that cannot see the
thread misreads it. We keep your account identifier here.

_Purpose and legal basis:_ as under Messages. The sending is how an answer gets written.

**Excluded uses**

We show no advertising and run no analytics, tracking or profiling. We do not sell, rent or
pass on what you write for marketing, and neither we nor our processor train a language
model on it. We make no automated decision about you within the meaning of Article 22 GDPR.
The assistant does not moderate: it cannot warn, remove or ban anybody, and it watches
nobody. It can pass a report to the group's moderation bot when a member replies to a
message and asks for one, and the group's administrators decide what happens. The same
language model that writes the answers writes that relay, so it can misfire and report a
message nobody meant; it goes out as a public reply, so you see it. We store the reported
message's identifier with the report and empty it when the reported person is deleted.
Answers are model-written and can be wrong.

**Recipients and processors**

| Recipient | Role | Basis |
|-----------|------|-------|
| Requesty Ltd, London, United Kingdom | Passes the conversation to a language model and returns the answer. Requests enter through its European endpoint, and what it stores it stores in Frankfurt. Zero data retention is configured: no message and no answer reaches storage or training. It keeps billing telemetry without content: token counts, the model identifier, a timestamp. | Processor, Article 28 GDPR |
| Sub-processors engaged by Requesty | Two layers. Amazon Web Services in Frankfurt runs the infrastructure, with Requesty answerable to us for it. The model providers it routes to, Google today, answer to Requesty, which is responsible for the choice and the written terms but not for a provider's own breach. Zero data retention binds Requesty and not a model provider: whether a provider keeps a request or trains on it follows the model's terms. | Sub-processors, Article 28(2) and (4) GDPR |
| Your chat platform | Delivers and stores the same messages as part of its own service, under its own privacy policy. It does not act for us. | Independent controller |
| The other people in the group | See your messages as they always have | Your own act of posting |

We store messages on a server in Germany. Data leaves the EU/EEA in three places:

- Our processor is a UK company, though it stores and serves in Frankfurt. The European
  Commission has decided the United Kingdom offers adequate protection, so no further
  safeguard is needed.
- A model deployment outside the EEA would receive the request there, under the standard
  contractual clauses in the processor agreement (Article 46(2)(c) GDPR); the configured
  model is served in the EU. Write to the address above for a copy.
- Your chat platform is outside the EU/EEA and receives every message and answer as it
  delivers them, as its own controller.

**Retention and deletion**

We keep messages until somebody asks us to delete them. We set no automatic expiry,
deliberately: the assistant answers questions about discussions from months ago, and a
schedule would delete everyone's history to reach the part one person wanted gone. We keep
identity data and circumstances as long as the message; the pinned rules while the
assistant serves the group, each replaced by the next; answer counters until they fall
out of their window; a report with its message reference emptied when the reported person is
deleted; lookup records with no time limit. Deleting a message in your chat app does not
reach us, and neither your leaving a group nor the assistant leaving it deletes anything:
ask, and it goes. If the service stops for good, we delete the store with it.

We delete on request the text of every message you wrote, its send time and reply reference,
and your identity data. What remains in a
group is an empty placeholder holding a position in the conversation.

Some things stay. The assistant's own answers stay, and one can carry your handle and repeat
what you asked. On your message rows the store's internal number, the arrival time and your
administrator standing stay; once your identity data is gone that number names nobody,
though it still ties those messages together. Our processor has nothing to delete, since it
keeps no message, and a model provider behind it may keep a request under its own terms,
beyond our reach. We do not reach lookup records, the pinned rules text, or the
copy of your message's identifier on somebody else's reply where it matches nothing of
yours, and a query or a rules text can quote your words or name you. If you opted out, we
keep your account identifier with the opt-out mark on purpose, because forgetting it would
mean collecting your messages again.

**Your rights**

You can ask for confirmation and access (Article 15), rectification (Article 16), erasure
(Article 17) and restriction (Article 18). Portability under Article 20 does not apply: it
covers data processed on consent or for a contract, and everything here rests on legitimate
interest.

You can also delete and object in the group. `/privacyout` stops us collecting and answering
your messages from that moment; `/privacydelete`, confirmed with `/confirmdelete`, removes
your stored data; `/unblockprivacy` turns collection back on. They act on the sending
account immediately, by machine, because you asked and confirmed. Plain words work too: tell
the assistant to stop collecting or to delete, and it honors that the same way.

Write to [privacy@halogenos.org](mailto:privacy@halogenos.org) for anything else, or if you
prefer a person, with your username and the group. A handle is visible to everyone, so we
may ask you to show the request comes from that account, and nothing more. We answer within
one month and charge nothing. A person answers email; the commands are the one place a
machine acts, and only on your own confirmed instruction.

**Objection**

You can object at any time to any processing described here, for reasons that come from your
own situation, by email or with the commands above. We then stop unless we can show
compelling legitimate reasons that override your interests, rights and freedoms, or we need
the data for legal claims. In practice we weigh nothing: `/privacyout` stops collection from
that moment, deletion removes what came before, and neither asks for a justification.

**Complaint**

You can complain to a supervisory authority, in particular where you live, where you work,
or where the alleged infringement took place. The authority competent for us is:

Bayerisches Landesamt für Datenschutzaufsicht (BayLDA)\
Promenade 18, 91522 Ansbach\
[https://www.lda.bayern.de](https://www.lda.bayern.de)

**Changes**

We update this policy as the assistant changes. The current version is always at this
address, and the assistant answers `/privacy` with it.

Last updated: 23 August 2026
