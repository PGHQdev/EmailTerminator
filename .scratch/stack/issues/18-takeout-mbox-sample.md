# Obtain a real Google Takeout mbox sample

Type: task
Status: open
Blocked by: —

## Question

Nothing to decide. A real Google Takeout mail export must exist locally before
the mbox parser choice can be judged, because ticket 01 could not verify which
mbox variant Takeout emits.

The work:

- Request a Gmail-only export from Google Takeout and wait for it to land.
- Record, without copying any message content into the repository: the mbox
  variant and its `From ` separator form, whether the file is gzipped, which
  Gmail-specific headers appear (`X-Gmail-Labels`, `X-GM-THRID`), the total
  size and message count, and any line-ending or escaping quirk.
- Note where the file lives on disk so later tickets and tests can point at it.

The sample is a local fixture. It never enters the repository — it is one real
person's mail.

Arms tickets 06, 07 and 15. Blocks none of them: the root decision must not
wait on an export queue.
