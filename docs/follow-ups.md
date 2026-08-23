# Follow-ups

Work recorded, not built — each item names the unit that recorded it and
what resolving it takes. Resolved items move into a decision record.

- **The group-to-supergroup migration strands the stored channel mapping**
  (group context, 2026-08-23). The platform renumbers a chat when a group
  becomes a supergroup; the stored mapping, the authorization row and the
  conversation stay keyed to the old id, and the renumbered chat arrives as
  a stranger. The migration signal exists on the wire but is discarded (see
  the next item). Resolving this means translating the migration into a
  re-keying of the mapping and the authorization.
- **The wire client discards error response bodies on non-success status**
  (group context, 2026-08-23). A non-success status is reduced to its code,
  which hides the migration signal above and every refusal detail the
  platform states. Resolving this means decoding the refusal envelope on
  failure statuses too, without letting any token-bearing detail into the
  error text.
- **The framework superseding-block compaction** (group context,
  2026-08-23). Context notes accumulate in stream order and the newest
  wording is authoritative; a superseding-block mechanism in the framework
  — already on its improvements list — would compact the superseded ones
  out of the projection.
