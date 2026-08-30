# 0077 — The display name is not stored, and titles are not derived

Date: 2026-08-23

## Context

Two data-minimization findings from the operator's review of what the
assistant actually holds and sends, decided together because both remove
personal-data processing nothing consumes.

First, the display name. The identity table stored each sender's display
name beside the username, refreshed on every message. Verification showed
it dead weight: written on refresh, read by nothing in production — the
speaker column carries the username (decision 0065), the operator match
reads the external id, the privacy documents promise it never crosses to
the processor. A stored personal attribute with no consumer is exactly what
minimization removes.

Second, the conversation title. The framework's metadata worker derives a
short title per conversation by sending an excerpt of the conversation's
prose to a model. The assistant is a chat bot: no surface shows a
conversation list, so nobody reads a derived title, and every derivation
still ships member prose to a model for a product feature that does not
exist here. Decision 0068 made the derivation's model configurable to fix
its region; the operator has now decided the derivation itself is not
wanted at all.

## Decision

The display name leaves every layer:

- `SenderIdentity` drops the field, so the adapter boundary carries the
  external id and the username and nothing more; the adapter stops decoding
  the platform's name fields entirely, so a display name never enters the
  process as a typed value.
- Identity resolution reads and writes the username alone, and the
  principals table's `display_name` column is dropped — values included —
  by an appended migration step, per decision 0026's discipline. Erasure's
  identity pass is unchanged in shape: it deletes the principal's row,
  which now simply holds one field less.

Title derivation is switched off:

- The framework grows a construction option on its runtime context,
  default on, off meaning no metadata worker is spawned and no title
  request is ever dispatched — zero title traffic by construction.
- The assembly builds its context with the option off, unconditionally: a
  configuration knob would imply the feature might be wanted, and the
  decision is that it is not.
- The `title_model` configuration key, its resolver and its start error are
  removed with the feature; a stale file still naming the key is refused at
  the load by the unknown-key rule, so the removal fails loudly instead of
  silently ignoring a line.

The privacy documents shrink accordingly: the policy's identity category
and language-model section lose their display-name and smaller-model
sentences, and the impact assessment's title-derivation defect note closes
— the non-EEA naming model is no longer an open dependency, because the
naming step no longer exists.

Residual, stated: titles derived before this change persist in the
framework's metadata ledger of upgraded stores, and decision 0012's OPEN
item on erased prose shaping a derived title still applies to them. No new
derivation will ever join them.

## Amended 2026-08-29 — the join notice, and what this decision still holds

Unit 36 stores one display name, and this decision is not reopened by it.
What 0077 removed was a display name held as IDENTITY data — a column on the
identity row, refreshed on every message, read by nothing. That stays
removed: `SenderIdentity` still carries the external id and the username
alone, the adapter still decodes no name field on a message, and the
`principals` table still has no column for one.

What unit 36 adds is different in kind. A group's join announcement SHOWS a
name, and that name is the announcement's content the way a message's text
is its content — it is the thing being assessed, since a joining account
whose displayed name is itself an advertisement is the offense before it
posts anything. So it is stored once, on the event that carried it, in the
join-notice table, under the same erasure discipline as message text: the
person-keyed pass empties it, and the projected join line carries it into a
request as event content, never as an attribute of a person's message.

The privacy documents move with it — the processing record's new category
and its corrected boundary sentences, the policy's identity and deletion
wording, and the two assessments' identity claims — so no published sentence
claims a display name is never stored or never sent.

## Amended 2026-08-30 — the crossing identity carries three facts

Unit 42 adds one field to the crossing identity, and this decision is not reopened by it.
The sentence above that says the boundary carries "the external id and the username alone"
was true of the two-field shape; the shape is now three, and decision 0151 records why.

What this decision removed stays removed. The display name is still not decoded from a
message, still not stored on the identity row, and still not sent anywhere; the amendment
of 2026-08-29 above is the only place a shown name is held, on the join event that carried
it.

What unit 42 adds is not personal data held about a person. It is the platform's own
statement that an account is automated, read fresh off every update and stored NOWHERE: no
column, no migration, no erasure pass, no privacy document sentence. It exists for the
length of one message's handling, where the adapter narrows an automated sender's
addressing and the core declines to summon one by mode, and then it is gone.

## Rejected alternatives

- **Keeping the dead column.** A stored personal attribute nobody reads is
  liability without function, and every privacy document would keep
  carrying sentences about data held for nothing.
- **Reopening the identity column for the join notice (2026-08-29).** A name
  shown once at a join is a fact about that event; hanging it on the identity
  row would make it a standing attribute of the person, refreshed and kept —
  exactly the processing this decision removed.
- **Keeping the crossing identity at two fields and inferring the third (2026-08-30).**
  The candidates were a handle-shape heuristic and a per-message flag: the first invents a
  fact the platform already states, the second hangs an account's property on one of its
  messages.
- **Nulling the column instead of dropping it.** The null-and-retire path
  exists for machinery that cannot drop a column; this store's SQLite drops
  columns, and a retired-but-present column invites the next writer to
  fill it again.
- **Keeping title derivation because it is cheap.** Cost was never the
  objection — decision 0068 already priced it as negligible. The objection
  is member prose sent to a model for a feature with no reader; a
  processing activity justified by nothing fails Article 5(1)(c) regardless
  of price.
- **A configuration switch for titles.** A knob says a deployment might
  turn it on; no surface reads a title, so the knob would configure dead
  behavior. If a conversation-listing surface ever ships, turning the
  construction option back on is that feature's own decision to make.
- **Dropping the framework feature instead of switching it off.** Other
  consumers of the framework read their titles; the framework keeps the
  feature and gains the off switch, defaulting on so existing consumers are
  unchanged.
