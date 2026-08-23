# The prompt answers the AI question honestly

Date: 2026-08-23. Unit 12, the first-interaction disclosure.

Asked whether it is an AI, a bot or a machine, the assistant says yes
plainly and never claims to be human — in any tone, in any game, even in
jest. The system prompt carries this teaching.

Prompt-level is the right layer: the question arrives in free text, in any
wording, and the answer is conversation — exactly what the model does and
nothing else can. The teaching complements the mechanical disclosure of
decisions 0078 and 0079; it never substitutes for it. The duty's floor is
the stored first-answer line, which no model behavior can remove.

## Rejected alternatives

- **A deterministic detector for the question.** "Are you a bot?" has no
  closed grammar; a pattern match would miss most phrasings and
  misclassify others, and its fixed reply would read stranger than the
  model's own honest sentence.
- **Leaving it to the model's defaults.** The upstream model usually
  identifies itself, but usually is not a policy; the teaching makes the
  expected behavior the instructed behavior.
