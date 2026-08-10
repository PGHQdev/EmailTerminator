# Release automation

Type: grilling
Status: open
Blocked by: 16
ADR: docs/adr/NNNN-release-automation.md (number assigned at resolution, in acceptance order)

## Question

What does a tagged release do, end to end?

Split from ticket 17 on 2026-08-10, because the release job needs the artifact
list that ticket 16 produces, while the pull-request pipeline needs nothing from
it.

Axes:

- Cross-compilation versus native runners per target, for release builds
  specifically. `libwebkit2gtk` makes Linux Tauri builds awkward to
  cross-compile.
- Every artifact from 16 built, signed with minisign for the updater, and
  attached where users are told to get it.
- Whether the GitHub Releases page is a supported download path at all, given
  `docs/reference/tauri-distribution-precedent.md` shows a browser download
  quarantines the bundle.
- Which secrets exist. An unsigned release avoids Apple credentials; the
  minisign updater key still has to live somewhere.
- How the installer script and the site are updated by the same tag.
- Whether releases are cut by hand or by tag push, and who can do it.
- Building, signing and publishing the Chrome extension (ADR 0003) alongside the
  app: Web Store submission, the sideloadable zip, and keeping both extension IDs
  in the shipped `allowed_origins`.
