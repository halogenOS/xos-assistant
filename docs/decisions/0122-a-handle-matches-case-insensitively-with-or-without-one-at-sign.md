# 0122 — A handle matches case-insensitively, with or without one at sign

Date: 2026-08-25, with unit 29; sharpened 2026-08-29.

## Context

Platform usernames are case-insensitive identifiers, and the model reads handles in
two shapes: a projected join line writes the at sign, a message's speaker prefix does
not. Whatever the model types has to reach the same stored row.

## Decision

The parameter is normalised by removing EXACTLY ONE leading at sign and folding case;
the stored side is folded the same way and keeps any at sign it carries. So `@ada`,
`ada` and `ADA` are one question, while `@@ada` folds to `@ada`, matches no stored
handle and answers the unshown refusal instead of quietly naming somebody.

The affirmative answer prints exactly one at sign, ahead of the handle in its STORED
form: the case the platform delivered, not the case the model typed.

That the stored form carries no at sign is a fact of today's platform, not of the
storable bound, which admits one. An adapter that ever stores at-signed handles needs
its own look at this normalisation; it is not decided here by accident.

## Rejected alternatives

- **Exact matching.** It would refuse a person visibly present in the conversation
  over a capital letter.
- **Accepting only the bare form.** The model reads handles written with the sign
  everywhere else and would be refused for copying what it saw.
- **Stripping every leading at sign.** It turns a malformed ask into a confident
  answer about a real person.
