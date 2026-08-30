# 0144 — The quoteless-her decision is superseded, and its pin moves

Date: 2026-08-30, with unit 38.

## Context

The unit that landed replies as quotes decided that a reply to one of the assistant's
own messages quotes nothing, and it was right on the tree it was written against: no
stored fact said which of her blocks a reply answered, and guessing one would have
reproduced the misattribution that unit existed to end. The operator overturned it once
the reason had a fix — their words, on being told her side was quoteless: "Why not?
Please fix it".

## Decision

Her sent messages record their ids, so a reply to her resolves like every other reply,
and the quoteless-her decision is superseded. The pin that asserted a reply to her lands
quoteless becomes the pin that it quotes her words, keeping its still-wakes assertion
unchanged; the translation pin on the bare variant moves with the widening.

The earlier unit's own document stays unedited. This record is the amendment, which is
how a spec keeps one voice: a document is rewritten to read as one present-tense
description, and the history of what it used to say lives in the decisions and in git.

## Rejected alternatives

- **Editing the earlier unit's document.** Its criteria describe the tree it shipped
  against, and rewriting them would erase the reason its decision was correct then.
- **Keeping the quoteless case and adding hers beside it.** Two behaviours for one
  reply, chosen by whose message it points at, is the branch the whole path exists to
  avoid — everything past the resolution already works on either target.
- **Leaving the earlier pin standing beside the new one.** A suite asserting both that a
  reply to her quotes nothing and that it quotes her words is a suite that cannot fail.
