# Privacy policy: the halogenOS Group Assistant

**Draft, not yet published.**

Date: 2026-08-23

The halogenOS Group Assistant is a bot in the halogenOS community chat groups. It answers
questions about the project, and to do that it reads and stores the messages in the groups
it was added to. This page tells you what it keeps, who else sees it, how long it stays,
and what you can ask us to do about it. It is written for the person typing in the group.

## Who is responsible

Simão Gomes Viana
c/o IP-Management #10911
Ludwig-Erhard-Str. 18
20459 Hamburg
Germany

Data protection enquiries: privacy@halogenos.org

## What the assistant stores

In a group the assistant belongs to, the platform delivers every message to it, and it
stores:

- the words you write, including the caption under a photo or a file
- an identifier for your account on the platform, your display name and your username,
  held in tables of their own, apart from the messages
- when the message reached the assistant, and whether you were an administrator of that
  group at the time
- whether the message was addressed to the assistant
- the group's title, and the text of the pinned rules message when the group has one

Pictures, files, voice messages, stickers and video are not stored. Neither are edits: the
assistant keeps a message as it first saw it, and a later correction never reaches it.
Messages posted anonymously on behalf of the group, and posts forwarded automatically from
a channel, are skipped, because the platform hides who wrote them.

A direct chat with the assistant is stored the same way.

The assistant only serves groups the project operator added it to. Added by anyone else,
it stores nothing and leaves.

## Why, and on what legal basis

Three purposes, and nothing else:

1. **Answering questions.** Someone asks about a build, a commit, a release or a setting,
   and the assistant answers in the group.
2. **Understanding the conversation.** An answer to "does that also affect the camera?"
   only works if the assistant can read what came before it, and community questions
   often point back to a discussion from weeks ago.
3. **Keeping the assistant usable.** Two counters limit how often it answers one person
   and one chat, so a flood cannot exhaust it for everybody else.

The legal basis is our legitimate interest, Article 6(1)(f) GDPR: running a working
assistant in our own community groups. The balance behind that basis, in short: the
assistant is announced in the group's pinned rules before you write anything, it stores
what people chose to post to a group of strangers and not anything private about them, it
sends no names to anyone, it takes no action against any member, and you can object or ask
for deletion at any time. What it stores is what you already said to everyone in the room.

**We do not ask for your consent, and the group rules are not a consent form.** Consent
that you must give to enter a group would not be a free choice, and one person taking it
back could not stop a group conversation from being recorded. So we tell you plainly what
happens instead, and we give you a real objection.

## Where your messages go

To write an answer, the assistant sends the text of the conversation to a language model
through Requesty Inc., which routes it. Concretely:

- **Your identity stays with us.** Your display name, your username and your account's
  numeric identifier are not part of what we send. The model receives the words of the
  conversation without any name attached to them.
- **The request stays in the EU.** It goes to Requesty's European entry point in
  Frankfurt, Germany, and the model that answers is Google's Gemini on Google Vertex AI,
  pinned to European serving by the model name we configure.
- **Nothing is kept on the other side.** The account is configured for zero data
  retention: neither Requesty nor the model provider stores the text after the answer
  comes back, and neither trains on it.
- **There is a contract.** Requesty processes only on our instruction, under a data
  processing agreement under Article 28 GDPR that carries the European Commission's
  standard contractual clauses.

Two more recipients, both narrow:

- **Public project sources.** A question about a commit, a release or the wiki makes the
  assistant query the halogenOS forge, the builds repository's public interface or the
  project wiki's public pages. Such a query carries a repository name and a commit
  reference, a release tag, or a wiki page name. It carries nothing about you.

  > Amended 2026-08-23: the wiki joined the public project sources — a wiki query
  > carries a page name and nothing else.
- **The chat platform.** Telegram is not ours and is not acting for us. It handles your
  messages under its own privacy policy, as its own responsible party, exactly as it did
  before the assistant arrived. The other people in the group see your messages the same
  way they always have.

The stored messages sit on a server run for the project in Germany. Nobody else receives
them. They are not sold, not used for advertising, and not fed into any analysis of
people.

## How long it stays

There is no timer. Messages stay until somebody asks for them to go.

That is a deliberate decision, and here is the reason. The assistant is useful in exact
proportion to how far back it can look, and a question about a discussion from three
months ago is a normal question in a community group. A retention window would delete
everybody's history on a schedule to reach the small part one person actually wanted gone,
and it would not remove the need for deletion on request, which is the mechanism that
genuinely helps you. So we keep the history, and we delete on request, quickly and
completely.

One consequence you should know: deleting a message in your chat app does not reach the
assistant. The platform does not tell it. Our copy stays until you ask us.

## Your rights

**Ask what we have.** You can get a copy of everything stored about you, together with the
purposes and the recipients named above.

**Ask us to delete it.** Deletion is one operation, and it does this: the text of every
message of yours is emptied, along with the time it was sent and the reference it replied
to. Your direct conversation with the assistant is removed whole, and your identity rows
are deleted. What remains is structure with nothing in it, so the surrounding group
conversation still reads in order. None of your words are left, and nothing points back to
you.

**Object.** You can object to this processing at any time, for reasons that come from your
own situation, because the basis is legitimate interest. We answer an objection within a
month, in plain words, and we erase what we hold of you unless we can show you compelling
legitimate reasons that outweigh your interest. If part of what you ask cannot be done
without the assistant leaving the group entirely, we tell you that instead of pretending
otherwise.

**Correct, restrict, take with you.** You can also have wrong data corrected, have
processing restricted while a dispute runs, and receive your data in a portable form.

**How to ask.** Write to privacy@halogenos.org. Tell us the group and your username, so we
can find you. If we cannot match your request to an account we may ask you one question to
confirm it is yours, and no more than that. Requests are free and answered within one
month. If a request is complicated we tell you inside that month how much longer we need.

## Two places deletion does not reach yet

We would rather name these than let you find them:

1. **Lookup records.** When the assistant looks up a commit or a release, the query and
   the result are stored as their own records, and today our deletion routine does not
   reach them. The content is technical, a repository name and a reference, but a query
   can quote the words you used to ask.
2. **Group notes.** The group's title and its pinned rules text are stored as notes. If a
   rules text names a person, deletion does not reach that note. A note is replaced when
   the group's rules are pinned anew.

Both wait on the same missing piece in the storage framework the assistant is built on.
When it exists, deletion covers them, and this section disappears.

## Decisions about you

The assistant makes none. It writes answers. It does not decide anything about you with
legal effect or similar weight. Its answer counters limit how much it replies, never what
it stores, and they change nobody's standing in the group. The one moderation step it
performs is started and decided by a person, never by the assistant: when a member
replies to a message and asks for a report, the assistant forwards that ask to the
group's moderation bot as a report command, and the group's human administrators decide
what happens. The assistant judges nothing about the reported message, and it stores the
reported message's identifier with the report so that a deletion request from the
reported person also empties the report's reference to them.

> Amended 2026-08-23: this section previously said the assistant takes no moderation
> action. With the report feature it relays a member's report to the group's moderation
> bot — visible to the group's administrators — and the paragraph above states exactly
> what that does and does not decide.

Its answers are written by a language model and can be wrong. Treat them as a helpful
member's answer, not as an official statement of the project.

## Data protection officer

None is appointed: the thresholds in § 38 BDSG are not met. Data protection questions go
to privacy@halogenos.org and are answered there.

## If you are unhappy with us

You can complain to a data protection supervisory authority. Ours is the Hamburg
Commissioner for Data Protection and Freedom of Information (Der Hamburgische Beauftragte
für Datenschutz und Informationsfreiheit), reachable at datenschutz-hamburg.de. You may
also go to the authority where you live or where you work.

## Changes

This page carries its date at the top. The current version is always reachable with the
`/privacy` command in any chat with the assistant, and through the group's pinned rules
message.
