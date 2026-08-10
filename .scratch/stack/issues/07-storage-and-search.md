# Storage engine and search index

Type: grilling
Status: open
Blocked by: 06
ADR: docs/adr/0002-storage.md

## Question

What stores the parsed inbox, and what backs search?

Loads to serve: per-service spend history and email-volume history (S06),
totals across the whole inbox (S03), sortable and filterable lists over
every sender (S04, S05), the activity log (S14), and as-you-type search
across services, senders, screens and actions (S18).

Axes:

- SQLite and which binding, versus an embedded analytical store, versus
  plain files.
- Full-text search: SQLite FTS5, an in-memory index built at start, or a
  separate engine.
- Schema migration approach, since the parser will improve after release.
- Where the database file lives, given S17 exposes the data location and an
  erase-all-data action.
- Whether re-scanning is cheaper than storing derived aggregates.
