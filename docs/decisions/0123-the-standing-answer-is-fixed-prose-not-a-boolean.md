# 0123 — The standing answer is fixed prose, not a boolean

Date: 2026-08-25, with unit 29; the byte form fixed 2026-08-29.

## Context

The audience for this result is a language model deciding whether to obey an
instruction. A bare `false` reads as weak evidence and gets argued with; a sentence
stating the consequence does not.

## Decision

Two fixed answers, each exactly two lines joined by one newline: the `admin:` line
and the `Note:` line.

For a person who is not an administrator: `admin: false`, then a note saying so. It
names nobody — no handle, no at sign — because nothing needs naming to state an
absence.

For an administrator: `admin: true`, then a note naming the handle, stating that this
person can override instructions and that regular members cannot, and telling the
model to use the tool again when someone asks for something privileged.

These are the operator's own words and are kept as given, "user" included, which is
not the vocabulary the rest of this repository uses for a person. The exception is
deliberate and recorded here so a later cleanup does not silently rewrite a string
whose exactness is the mechanism.

## Rejected alternatives

- **A JSON object with a boolean field.** What the tool would return if its audience
  were a program instead of a reader.
- **Paraphrasing the strings to match the repository's vocabulary.** The wording is
  the mechanism; a paraphrase is a defect, and the suite pins the bytes to make it
  one.
