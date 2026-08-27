# 0109 — A refused threaded send retries plainly, and the answer is never lost

Date: 2026-08-24

## Context

Decision 0059 has the adapter state send-without-reply tolerance on every threaded
send, which covers exactly one cause: a target the platform can no longer find, which
degrades to a plain send. Every other refusal of that request fails the send, and the
outbound consumer logs it and drops the reply.

While only the report threaded, the cost of that was one moderation nudge. With
decision 0106 the answers thread too, and threading would introduce a way to lose an
answer that did not exist while answers were plain: a target in another chat, a topic
the platform will not reply into, any refusal a future platform version invents.

## Decision

Where the platform refuses a send that carried a reply target, the adapter sends the
same text once more without the target. The recovery is bounded to that one cause and
that one attempt: it is the thread that failed, not the text, and an answer must never
be lost to a courtesy. A second refusal is the send's own failure, reported as before.

"Refused" means the platform answered and declined — a client-error status, or a
success status carrying a false ok flag. A server-error status is deliberately
outside it, alongside a transport failure and an undecodable answer: each leaves the
message's fate exactly as unknown as the others, and repeating any of them could
double a send. A spent rate-limit bound is outside it too, because it asks for time,
not for a different request. (Corrected 2026-08-27: this paragraph originally said
"a failure status", which read as any failure and licensed the double-send the code
refuses — the pinned behaviour was always the narrower one.)

## Rejected alternatives

- **Relying on the tolerance stated on the request.** It covers only a deleted target.
- **Retrying every failed send.** A transport failure may have delivered the message,
  so the retry is how the same answer reaches the chat twice.
- **Retrying the whole reply rather than the refused chunk.** Only the first chunk
  carries a target, so only the first chunk can be refused for carrying one; resending
  the delivered chunks would repeat what the chat already holds.
