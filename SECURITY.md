# Security policy

Please report archive traversal, decoder, credential-redaction, or remote stream
issues privately to the repository maintainers before opening a public issue.

The skin importer rejects traversal, symlinks, more than 1,000 files, archives
over 25 MiB compressed, and more than 100 MiB expanded. Remote playlists are
limited to 2 MiB and two nesting levels. The app has no telemetry or automatic
crash upload.
