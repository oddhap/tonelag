# Security policy

Please report archive traversal, decoder, credential-redaction, or remote stream
issues privately to the repository maintainers before opening a public issue.

The skin importer rejects traversal, symlinks, more than 1,000 files, archives
over 25 MiB compressed, and more than 100 MiB expanded. Remote playlists are
limited to 2 MiB and two nesting levels. The app has no telemetry or automatic
crash upload.

The optional skin browser contacts `skins.webamp.org` only while its window is
open and downloads from `r2.webampskins.org` only after the user selects
**Install**. Catalog responses are capped at 2 MiB, redirects are disabled,
remote URLs and identifiers are allowlisted, adult-marked entries are excluded,
and each downloaded archive must match its catalog MD5 before the regular skin
archive validation runs.
