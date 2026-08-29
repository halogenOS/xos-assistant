# 0132 — The distribution is read at execute time, through one reader

Date: 2026-08-30, with unit 37.

## Context

A host states its distribution in its os-release file. Nothing in this repository read
that file before, so the whole surface — where the read happens, what shape the parse
takes, and what an absent answer looks like — is decided here.

## Decision

One public reader owns the read and the parse together. It answers a finished
distribution string or nothing at all, and there is no way for a caller to hold half of
it — no raw text handed out, no parse a caller could run against a file it opened
itself. Under it sits a public parse-from-text step the reader wraps, so the parse can
be exercised over injected shapes without a host file while the execute path keeps
calling the one surface. The byte pin over the assembled core recomputes its expectation
through that same production reader, so no test shape reaches the assembly and the tool
gains no field.

The read happens at execute time, per call, like the clock reading beside it. A host
rebuilt onto a new release then answers as it is now, without waiting for a restart. It
is one small local file, read plainly inside the execute path: a sub-millisecond read
needs no offload machinery, and the promise the tool's header makes — no network call,
no subprocess, nothing that can hang or leak — survives intact with this one named file
added to it.

Every failure collapses to nothing: a missing file, an unreadable one, a text naming
neither key. Nothing renders the literal `unknown`, matching the build revision's
precedent — a named fact that should exist says plainly when it is absent, while
silence is reserved for parts whose absence is normal, as the clock reading's zone
parts are.

The parse, exactly: take the line whose key is `PRETTY_NAME`, else the line whose key is
`NAME`, splitting each line at its first `=`; trim ASCII whitespace around the value;
strip one matching pair of surrounding double or single quotes; pass escape sequences
through as stored. A value empty after that answers nothing and yields, so an empty
`PRETTY_NAME` falls through to a usable `NAME` and both empty is nothing. A key stated
twice answers from its first line.

## Rejected alternatives

- **A crate for os-release parsing.** A dependency, its supply chain and its upgrade
  path, for two keys and a quote pair.
- **A subprocess.** Running `uname` or a release helper breaks the header's promise
  outright, and buys nothing the file does not already state.
- **Compile-time capture.** It would state the build host's distribution, not the one
  the process runs on — the exact class of confidently wrong answer this tool exists to
  end.
- **Full os-release escape handling.** Machinery for bytes no distribution puts in
  these two keys; a decoder nobody can exercise is a defect waiting for its first real
  input.
- **A path parameter on the reader so tests could point it elsewhere.** A seam existing
  only for the tests, on the one surface that must be identical in the test and in
  production. The parse step is public instead, which exercises the same code by
  handing it the same text.
