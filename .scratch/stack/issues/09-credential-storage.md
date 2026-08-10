# Credential storage

Type: grilling
Status: open
Blocked by: 06
ADR: docs/adr/NNNN-credential-storage.md (number assigned at resolution, in acceptance order)

## Question

Where do IMAP app passwords, Gmail OAuth client secrets and refresh tokens,
and Tier 2 provider API keys live?

Options: the OS keychain through a binding, an encrypted file with a
user-supplied passphrase, an encrypted file with a machine-derived key, or
plain configuration.

Axes:

- The headless CLI may run where no keychain session exists. What happens
  then?
- S17 offers erase-all-data. What must that reach?
- A passphrase prompt on every launch conflicts with the app's intended
  ease. A machine-derived key is convenience with weaker guarantees. Name
  the position taken.
- Linux keychain availability is inconsistent. State the fallback.
