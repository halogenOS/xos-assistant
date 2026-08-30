# 0168 — A leaked reasoning trace is cut at the send, never in the framework

Date: 2026-08-30, with unit 43.

## Context

A live answer arrived in a group with the model's whole reasoning in front of it: prose
about how to handle the question, a closing think-tag, then the real answer. The tag pair
is a model's own convention for marking a reasoning block, and a decoder that loses the
opening tag leaves the closing one standing inside the answer text.

Two places could remove it. The framework turns a provider's stream into a stored block
and serves every consumer built on it. The application's outbound edge turns a stored
answer into a message on a chat platform. The second one was decided: not the
framework, because a model might write a closing tag on purpose, but the layer that sends
to the platform.

## Decision

The cut lives at the application's outbound edge, the single place an answer's text
becomes a platform message, and it applies to the model's own answers alone.

The rule, exactly, is a count of closing tags: an answer whose text carries EXACTLY ONE
closing think-tag loses that tag and everything before it, and what follows is the answer.
Any other count — none, or two and more — is delivered byte for byte. One closer is the
shape the live leak had, and it is cut whether or not an opening tag precedes it: no
opener is consulted, because the count alone decides. Two or more is a shape nobody has
seen leak, and a send that guessed which of them ended a trace could amputate an answer
that merely writes about the tags. The rule's accepted cost is stated plainly: an answer
that legitimately mentions the tag exactly once is indistinguishable from the leak by
count, and the cut takes it — everything before the one mention is dropped. That trade
was decided with eyes open; a shape that surfaces in practice gets a new decision then.
The tags are matched as exact bytes, no case folding and no attribute spelling, because
the leak is the one literal token a model emits.

The cut runs ahead of the answer arm's two other steps, and it reads the model's prose
under any disclosure line an earlier delivery already stored. So an answer that is nothing
but reasoning cuts to nothing and takes the empty-answer path of unit 22 — accounted
delivered, nothing sent — and the first-interaction line is composed back in front of the
cut text, including on a re-delivery of an answer whose stored block already carries it.
The introduction is resolved as a choice between two openings and never as prose, so the
text that reaches the wire is composed in exactly one place and cannot be assembled from a
text the cut never saw. The deterministic replies never meet the cut at all: the failure
notice and a filed report's line are fixed texts a person wrote, and they take the other
branch by construction.

The stored block keeps the model's full text. The ledger is the record of what the model
wrote and the model's own history reads back its own words, so the stored answer and the
delivered one differ by exactly what the send cut. This narrows decision 0079: what the
ledger and the channel share is the introduction line over the same answer, not one text
byte for byte. It narrows decision 0142 the same way — a member replying to a leaked
answer quotes the stored block, so the model is shown its own words, trace and all, which
is precisely what keeping them is for.

That read-back is a weighed cost, not an oversight: every kept trace can re-enter the
model's context as an assistant turn carrying the tag, mildly normalizing the very
malformation this cut repairs. It is accepted because the loop is bounded — the wire
never carries a trace again, so members see nothing and only a deliberate reply to the
leaked answer resurfaces it — and because the alternative, editing the stored block, is
the content rewriting every rejected alternative below refuses. A leak frequent enough
to bend the model's habits would be a provider defect to escalate, not a storage rule to
change.

## Rejected alternatives

- **The cut in the framework, where a stream becomes a stored block.** A model may emit
  the tag deliberately, and a framework serving several consumers cannot decide that for
  them. It would also mean the stored block is no longer what the model wrote, and keeping
  that record is the ledger's whole purpose.
- **Teaching the model to stop leaking.** The leak is what a decoder produced, not what
  the model chose to say; prose in the prompt cannot reach it, and the one place the
  artifact is visible with certainty is the text about to be sent.
- **Refusing or marking a leaked answer.** The prose behind the trace is a usable answer.
  Withholding it, or decorating it with a warning, costs a member their reply over a
  formatting fault they did not cause.
- **The last closing tag no opening tag precedes.** The rule this decision first carried,
  replaced on 2026-08-30. It reads an opener to decide whether a closer is
  stray, so an answer that legitimately quotes a closing tag in its prose — a member asking
  what the tag means, a pasted model output — loses everything in front of that quote,
  amputating a real answer. The count rule cuts the observed shape and leaves every other
  shape to the next decision.
- **Cutting on the opening tag as well — dropping any bracketed block.** That is content
  rewriting: a message about the tags, a code sample, a quoted model output all carry the
  bytes legitimately, and the send has no way to tell them apart. Only the unopened closer
  is unambiguous evidence of a leak.
