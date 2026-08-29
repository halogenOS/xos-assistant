# 0115 — The query guard refuses the handle form, and echoes nothing

Date: 2026-08-27; the grammar defined operationally 2026-08-29, with unit 27.

## Context

A model-written query is member-derived text leaving for a new third party, and a
tool call and its result live on framework tables erasure does not reach. Two things
follow: a deliberate person reference must never be sent, and the refusal must never
write the identifier into the record either.

What the guard can match on is narrow by design. Display names are not stored at all
(decision 0077) and the adapter never even translates them, so there is no
display-name half to match. The principals table is adapter-scoped and holds only
people who spoke, so a membership check would miss exactly the mentioned bystanders
a guard exists for.

## Decision

The guard refuses a query containing a token in the HANDLE FORM: an at sign starting
a token, followed by a name. It refuses the form regardless of whose it is, and it
needs no member list. The definition is platform-neutral by construction — it covers
a bare handle and a federated one alike.

The grammar, operationally: a name character is a letter, a digit or an underscore; a
candidate token is an at sign followed by name characters of which the FIRST is a
letter — so a version pin like `package@1.2.3` sits outside the grammar itself rather
than being a special case. An at sign PRECEDED by a name character is an email
address and never a candidate, which is what lets a dotted local part through whole.
A candidate ended by a slash is a scoped package name.

Matching runs on a normalised view — formatting characters dropped, a single
separator between a candidate's name characters collapsed — so `@ h a n d l e`,
`@h.a.n.d.l.e` and `@handle` are one token. Normalisation is applied to FIND a token
and never to the whole query as one string, so word boundaries survive and a single
ordinary word is never matched, however exactly it equals somebody's handle: a search
for a word somebody happens to be called is chance, not a submission of personal
data. What is sent to the vendor is always the query as written, or nothing.

A refused query answers with the rule and the fix and does NOT echo the matched
token: a guard that writes the identifier it refused into permanent storage protects
nothing.

## Amended 2026-08-29 — one display name is stored now, and the match set is unchanged

The context above says display names are not stored at all and the adapter never
translates them. As of unit 36 that sentence needs its qualification: a group's
join announcement is recorded with the name it showed, as that event's own
content in the join-notice table, and the adapter decodes name fields on the
joiner type it added. What decision 0077 removed stays removed — no name on the
identity row, none decoded on a message — and 0077's own dated amendment states
the distinction between an identity attribute and an event's content.

What the guard matches on does not move. A display-name half needs a standing
name PER PERSON to compare a query against; what exists is one name per join
EVENT, erasable with the person and recorded so the assistant can assess the
announcement. Matching queries against those names means reading members'
recorded names in order to protect them, which is the second reason the
alternative below is refused, and it holds unchanged. The match set stays
handle-shaped.

## Rejected alternatives

- **A display-name half.** There is no data for it. Collecting display names in
  order to protect them would falsify four pinned document statements and breach the
  impact assessment's own safeguard — a guard that collects more personal data to
  guard personal data.
- **A member-list check on the handle.** Adapter-scoped, speaker-only, so it misses
  the bystanders it exists for.
- **Confusable folding.** A UTS-39 table is a new dependency against an adversary
  this guard does not face: the query's author is our own model, the failure mode is
  carelessness, and no lexical rule stops a model that paraphrases a person instead.
  The guard is a discipline device and is recorded as one.
- **Naming the matched token in the refusal.** The first revision did. See decision
  0045's grounds: the record is exactly what erasure cannot reach.
