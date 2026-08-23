# 0090 — The disclosure line is a configured value

Date: 2026-08-23

## Context

Decision 0079 shipped the first-interaction line as the operator's copy
verbatim, a constant in the core. With the name configurable (decision
0089) the line must follow, and the transparency duty it discharges is not
optional.

## Decision

A `disclosure` key overrides the first-interaction line whole. Unset, the
line composes from the resolved name — "Hi, I'm <name>, an AI
system, made to assist members of the community.", the original copy's
shape with the name as its one slot — so the line is never empty: unset
means the composed default, never no text. An empty configured value is
refused at the load; a surviving line resolves trimmed.

The mechanism of decisions 0078 and 0079 is unchanged: the line is stored
into the first answer, per person, mechanically. The introduction receipt
reads the CURRENT line's prefix, so a deployment that edits the line
re-introduces people the old line already reached — one repeated line, the
same harmless direction the unreadable-provenance fold already takes, and
the opposite direction (a skipped first line) stays impossible.

## Rejected alternatives

- **Dropping the line when unset.** The transparency duty is not optional;
  unset means the default text.
- **Recognizing every historical line as a receipt.** Would need a stored
  registry of past lines for a rare edit whose worst outcome is one
  repeated introduction.
- **Composing the line only and refusing an override.** The operator's
  copy is wording, and wording that ships to people is the operator's to
  set.
