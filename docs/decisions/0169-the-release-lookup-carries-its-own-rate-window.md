# 0169 — the release lookup carries its own rate window

Date: 2026-08-30, with the limiter binding.

## Context

A session once ground the release lookup in a loop for hours: each call was
in-rate for the conversation-wide tool window, so nothing refused it, and
the model kept reading failure after failure as something to retry. The
framework's per-tool windows (its slice 17) exist for exactly this shape —
a bound on one tool's pace, beside the conversation's whole-turn bound.

## Decision

The assembly binds `lookup_release` to six calls per sixty trailing
seconds, the operator's numbers, at the one place the runtime context is
still unshared. The binding is conditional on the tool being registered:
the framework's builder refuses a name nothing registered, and an assembly
whose embedder admits no lookups is a test rig, not a misconfiguration.
The numbers live once, as constants beside the tool's name, and the
refusal the model reads is the framework's per-tool template — teaching it
to answer from what it has, use a different tool, or wait.

## Rejected alternatives

- **Bounding every lookup the same way.** The other lookups have shown no
  runaway; a bound nobody measured is a number nobody can defend. Tools
  gain windows when their behavior asks for one.
- **A retry-suppressor inside the tool.** The tool answering "slow down"
  from inside would be a second rate decision beside the framework's one;
  the window is the recorded home for pacing.
