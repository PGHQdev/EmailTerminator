# Community data format

Type: grilling
Status: open
Blocked by: —
ADR: docs/adr/NNNN-community-data-format.md (number assigned at resolution, in acceptance order)

## Question

What file format and schema hold the unsubscribe recipes, cancellation
playbooks, and the critical-services list, given they live in the repository
as pull-requestable data?

Axes:

- Format: TOML, YAML, JSON, or Markdown with front matter. The reader is a
  contributor writing a playbook by hand, so the format is a contribution
  cost.
- One file per service or one file per category.
- Schema and validation. How a malformed pull request fails in CI before a
  maintainer reads it.
- How a service in a mailbox is matched to its entry — domain, sender
  address, or a declared matcher.
- Playbook steps are shown to the user in S08 and may also be executed by
  an agent in S09. Does one format serve both readers?
- Versioning: entries ship inside the app bundle, so a corrected playbook
  must reach users. Bundle-only, or fetched with an offline default?
