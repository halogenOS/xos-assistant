# 0100 — The index is a second configured base

Date: 2026-08-24

## Context

Page content is served as plain text from the raw host; the rendered index that
lists the pages is served from the forge host. These are two different hosts.

## Decision

The wiki lookup carries a second base address for the index, defaulting to the
real forge host and overridable the way the raw base is, so a test points it at
a loopback server serving a captured index and a deployment never hard-codes a
host. Page content keeps reading the raw base unchanged. No page list is ever
baked into code or configuration; the names come from the wiki itself.

## Rejected alternatives

- **Deriving the index host from the raw host by string surgery.** Brittle and
  undiscoverable; the two hosts are unrelated names.
- **A single base for both.** The raw host does not serve the rendered index.
