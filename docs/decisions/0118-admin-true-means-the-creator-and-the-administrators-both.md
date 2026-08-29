# 0118 — Admin true means the creator and the administrators both

Date: 2026-08-25, with unit 29. Settled by the operator the same day.

## Context

The standing lookup answers one question in two words: is this person an
administrator. The stored vocabulary it answers from has three values, and they do
not line up with the word. Under decision 0015 the adapter maps the platform's
creator to Admin and everyone the platform labels an administrator to Moderator. So
the people a group calls administrators — the ones whose names sit in the member
list under that heading, the ones who can pin, remove and promote — are stored as
Moderator, and Admin names exactly one person.

## Decision

The answer means what the group's own member list means. Admin and Moderator both
answer true; Member answers false.

The result is read by a model that knows nothing about this codebase's enum, in a
group where the word has a plain meaning. A tool answering only for Admin would tell
a real administrator they are not one, pin that false statement into a test, and
dissolve the reason the tool exists: it would contradict an honest claimant, which
is the one case it was built to handle correctly.

## Rejected alternatives

- **The creator alone.** Wrong about the people it is asked about most, and it would
  have needed different wording throughout, because "administrator" is false for the
  people who are ones.
- **Renaming the stored vocabulary to match the platform's words.** The enum is a
  stored encoding with a schema constraint behind it, shared by the admission check
  and the debt fold. Changing what a stored word means to suit one tool's sentence
  is the reverse of the fix.
