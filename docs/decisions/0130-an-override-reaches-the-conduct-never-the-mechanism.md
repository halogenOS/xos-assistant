# 0130 — An override reaches the conduct, never the mechanism

Date: 2026-08-25, with unit 29.

## Context

The lookup tells the model that an administrator can override its instructions. Left
there, the sentence is too wide: a model reading it may believe an administrator can
also make a tool do something the tool does not do.

## Decision

An administrator can tell the assistant how to conduct itself — tone, subject, what to
leave alone. Nobody reaches the mechanism by instruction: decision 0070's human
decision point, the privacy tool's subject resolution, the admission rule and the
erasure fence hold whoever is speaking. The conduct prose says so.

The mechanisms hold either way; they are code and read no instruction. What the
teaching prevents is a model that believes an instruction COULD work, keeps trying,
and tells the member it is about to happen.

## Rejected alternatives

- **Leaving it unsaid.** The protections hold and the model still promises things it
  cannot do, which is worse for the member than a plain refusal.
- **Teaching the model to refuse administrators out of caution.** It makes the lookup
  useless in the direction it is most often needed: an administrator asking for
  something ordinary.
