# 0111 — The vendor sits behind a trait and posts through the lookup layer

Date: 2026-08-25; the POST seam decided 2026-08-27, with unit 27.

## Context

The search vendor is one HTTP endpoint with a key, a request shape and a response
shape, and every one of those is the vendor's and not the assistant's. Meanwhile the
shared lookup layer already owns the discipline every outbound request in this
repository runs under: one request, a named timeout, no redirect following, a body
cap. That layer was GET-only, and its failure wording is the bare status sentence
("answered HTTP 403") this unit forbids on the search's surface.

## Decision

The vendor sits behind a `SearchProvider` trait with one implementation. The tool
owns the envelope, the guard, the budget, the cache and the teaching surface; the
implementation owns the endpoint, the key, the request body and the response shape,
and the tool names none of them.

The implementation posts through the shared lookup layer, which grows a bounded POST
seam instead of bypassing it. The seam hands back the STATUS as a number and the
decoded body, unworded, so a caller with taught results of its own maps them itself:
the transport's verdict is typed (`WireFailure`) and each caller words it. The GET
paths keep the sentences they always answered, by wording that same verdict.

## Rejected alternatives

- **One concrete client inside the tool.** The tool would then know the vendor's
  header names and JSON field names, and the endpoint decision would be recorded in
  the same place as the envelope decision.
- **A private HTTP client beside the lookup layer.** It loses the shared timeout,
  redirect and body-cap discipline, and records the same decision a second time —
  which is how two request paths end up disagreeing about what a redirect means.
- **Reusing the shared status wording.** "answered HTTP 403" is exactly what this
  unit forbids: the model must be told the key was refused, not shown a number.
