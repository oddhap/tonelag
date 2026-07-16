# Release checklist

Public artifacts are unsigned prereleases until this checklist is fully recorded
for a tag. Signing and notarization can be added without changing the package
layout.

## Artifacts

- Windows x64 and ARM64: NSIS plus portable ZIP.
- macOS 13+: one universal `.app`/DMG.
- Linux x64 and ARM64: AppImage and DEB based on Ubuntu 22.04 / Debian 12.
- `SHA256SUMS`, CycloneDX SBOM, `THIRD_PARTY_NOTICES.md`.
- Exact FFmpeg and libopenmpt source archives plus `ffmpeg-config.txt`.

## Hardware matrix

For every OS/architecture combination:

- launch/open-with and clean shutdown;
- 60 minutes local playback with seek, track end, and sample-rate change;
- 60 minutes direct MP3/AAC radio with metadata and one forced reconnect;
- no crash and no reported audio underrun;
- main/EQ/playlist at 1×, 2×, winshade, DPI change, and two monitors where
  available;
- default fixture plus licensed flat/nested `.wsz` fixtures.

Windows ARM64 requires a recorded physical-device run; a successful cross-build
alone is insufficient. Only tags with the complete matrix may be called stable.

## Unsigned install notes

- Windows: SmartScreen may require **More info → Run anyway**.
- macOS: use **Open** from Finder's context menu; never recommend disabling
  Gatekeeper globally.
- Linux: mark AppImage executable or install the DEB with the system package
  manager.

The frameless skin regions use transparent macOS webviews, so the Tauri
`macOSPrivateApi` option is enabled. This rules out Mac App Store distribution;
the supported macOS channel is the signed/notarized DMG when signing is added.
