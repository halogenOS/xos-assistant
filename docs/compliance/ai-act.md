# EU AI Act compliance record

Status: adopted 2026-08-23, with the first-interaction-disclosure unit.
Verified against the official texts: Regulation (EU) 2024/1689 (the AI Act)
and the Commission's Guidelines on the transparency obligations under
Article 50 (the numbered paragraphs cited below are the guidelines').
This page is the assistant's AI Act position in one place; the decision
record on the operator's provider role carries the rejected alternatives.

## 1. The role: provider

The operator is the **provider** of this AI system, not merely a deployer.
The assistant is assembled from a general-purpose model, given its own
purpose (a community assistant answering under its own name) and put into
service under the operator's name — the exact shape the Article 50
guidelines' §2.3 names in its third example of who counts as a provider of
an interactive AI system. The two exits from the Act's scope do not apply:
Article 2(10) (purely personal, non-professional activity) fails because
the assistant serves a public community around an open-source project, and
Article 2(12) (free and open-source release) covers releasing a model or
system, not operating a service with it — the running assistant is a
service put into service, whatever its source license says.

Corrected 2026-08-23: earlier drafts of the privacy documents called the
operator a deployer. The impact assessment carries the dated correction in
its review-trigger list.

The upstream general-purpose model's provider keeps its own Chapter V
obligations; nothing here transfers them. This page covers the assistant
as an AI system.

## 2. The risk class: minimal, with Article 50 duties

- **Article 5 (prohibited practices): clear.** The assistant runs no
  subliminal or manipulative techniques, no exploitation of vulnerable
  groups, no social scoring, no biometric categorisation, no emotion
  recognition, no scraping for face databases.
- **Chapter III (high risk): not applicable.** The assistant matches no
  Annex III category — it grants no access to services, scores no people,
  decides no employment, education, credit, migration or justice matter.
  Its one standing-adjacent capability, the report relay, is an assessment
  a human judges (the assistant-assesses-a-human-decides rule); the
  impact assessment's review trigger reopens this classification if a
  real moderation capability ever ships.
- **Article 50 (transparency): applies**, and is what the rest of this
  page discharges.

## 3. Article 50(1) and 50(5): the disclosure

Natural persons interacting with the assistant must be informed that they
are interacting with an AI system, in a clear manner, at the latest at the
first interaction (Article 50(1) with 50(5)). The guidelines make the
placement concrete: the information belongs **in the system itself**, per
person, at their first interaction (paras 37, 38 and 143) — a profile
field or a policy page is not the system informing anyone. Para 34 asks
for wording accessible to vulnerable readers, children included.

Discharged by the first-interaction disclosure: the assistant's first
answer to each natural person opens with the disclosure line

> Hi, I'm <name>, an AI system, made to assist members of the
> community.

stored into the answer itself, resolved per person from the ledger's own
memory, shown again to a person the store no longer knows after their
deletion. Plain words a young reader understands, no legalese. Beside the
mechanical line, the system prompt teaches the model to say plainly that
it is an AI whenever asked, and never to claim to be human.

Amended 2026-08-23, with the helpful-mode unit: the line is a
configuration value — the `disclosure` key overrides the text whole, and
an unset key composes the shown default from the assistant's resolved
name, so the line is never absent; the original adopted copy ("Hi, I'm
Xenia, the halogenOS Assistant Bot, an AI system, made to assist members
of the community.") remains a valid configured value. The duty holds
under every answering mode: whether the assistant answers only when
addressed or offers help on unaddressed messages, the first SPOKEN answer
to a person still carries the line, and an abstained turn speaks nothing
and therefore introduces no one — silence discharges nothing and owes
nothing.

## 4. Article 50(2): marking of generated text

Providers of systems generating synthetic text must ensure the output is
marked in a machine-readable format as artificially generated. The
position taken, per the permitted path in the guidelines:

- **Upstream marking relied on.** The guidelines allow a provider who
  builds on a general-purpose model to rely on the marking tools the
  model's provider supplies (paras 74 and 78). The assistant adds the
  disclosure line, splits an over-cap answer into chunks at the
  platform's message limit, and can ship a truncated answer when a
  later chunk fails to send — all three touch the emitted text, and
  text-marking schemes are sensitive to exactly such edits. The
  due-diligence check below therefore covers the marking's survival
  through the real pipeline, chunked and truncated output included,
  not just the model's raw text.
- **Due-diligence check: pending the first live turn.** Reliance requires
  checking that the upstream marking is actually present in the deployed
  configuration. The check is recorded here as OPEN and runs on the first
  live turn's output; its result is written back into this section when
  it exists.
- **The practice bound noted.** The Code of Practice on marking of
  AI-generated content, Sub-measure 1.1.2, bounds the marking practice to
  outputs above roughly 200 tokens; a short chat answer sits below the
  practice's own floor. Recorded as the current industry practice this
  position leans on, not as an exemption in the Act's text.
- **Detection route.** Verification and detection run through the public
  industry-standard detector for the upstream model's marking; no
  detector of our own is built.

## 5. In place of the Code of Practice: the gap analysis

The guidelines let a provider demonstrate compliance without signing the
Code of Practice by other adequate means (para 148). The position taken
is a recorded gap analysis instead of accession:

| Duty | Where it is met |
| --- | --- |
| Art 50(1)+(5) disclosure, in-system, first interaction | The first-answer line, this unit |
| Art 50(1) accessibility (para 34) | The line's child-appropriate copy |
| Art 50(2) machine-readable marking | Upstream marking relied on (section 4), check pending |
| Art 50(3) emotion recognition / biometric categorisation | No such capability exists |
| Art 50(4) deep fakes and public-interest text | No image, audio or video is generated; the assistant publishes no text to inform the public on matters of public interest — it answers people in a chat |
| Art 4 AI literacy | Section 6 |

## 6. Article 4: AI literacy

The operator built and operates the system and carries the provider's
literacy duty in person. The documents in this repository — the unit
specifications, the decision records, the privacy assessments and this
page — are the ongoing literacy record: they are how the people operating
the assistant learn and keep current what the system does, what it may
not do, and why.

## 7. When this record is taken again

- Any moderation capability shipping (shared trigger with the impact
  assessment) — it reopens the risk classification.
- The due-diligence check's first result (section 4) — it closes the OPEN
  item or forces a new marking position.
- A change of upstream model or provider.
- New guidance from the Commission or the AI Office on Article 50.
