# 0116 — Project facts come only from the project lookups

Date: 2026-08-27, with unit 27.

## Context

The sourcing teaching routes every substantive claim through the tools: "your lookup
tools are the only source of substantive claims: any claim about the project … must
come from a lookup you made in this turn". That sentence was written when every
lookup tool read a project source. Registering a web search inside "your lookup
tools" would silently authorise a random web page to back a claim about halogenOS —
and a wrong page about this project is exactly the failure the sourcing rule exists
to prevent.

## Decision

The search teaching carries a carve-out, and it composes on the same predicate the
tool's admission takes: project facts still come ONLY from the project lookups. The
web search answers questions about the world and is never the source for a claim
about the project, its features, its procedures or its builds.

Beside it, two sentences about what a search result is: a snippet is a hint and an
answer built on one says where it came from, naming the page; and a snippet that does
not contain the claim is a miss, exactly as the existing lookup rule already says of
an unanswering lookup — the model says it does not know instead of filling the gap
from memory.

## Rejected alternatives

- **Leaving the sourcing rule unamended.** The moment the search registers, "your
  lookup tools" includes a web proxy, and the project-claim rule authorises it.
- **Rewriting the base sourcing rule for every deployment.** A deployment with no
  search key has no web proxy to carve out, and a rule that mentions a tool nobody
  has is a rule nobody can check.
