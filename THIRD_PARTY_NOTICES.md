# Third-party notices

Tonelag is original software and does not contain code from the
official Winamp source release.

The application depends on open-source packages listed in the lockfiles. In
particular:

- Tauri is available under the Apache-2.0 and MIT licenses.
- React is available under the MIT license.
- JSZip, i18next, react-i18next, rtrb, rustfft, and the other Rust/JavaScript
  dependencies recorded in the lockfiles are distributed under their declared
  permissive licenses. Release SBOM files provide the complete versioned list.
- FFmpeg is dynamically linked and must be built in LGPL-only mode, without
  `--enable-gpl` or `--enable-nonfree`. Release artifacts must include the exact
  corresponding source and build configuration.
- libopenmpt is available under the 3-clause BSD license.
- Souvlaki is available under the MIT license and provides the Windows SMTC,
  macOS Now Playing/Remote Command Center, and Linux MPRIS adapter.
- Webamp is MIT-licensed and was used only as a behavioral and file-format
  reference. No Webamp source file is currently copied into this repository.

Tonelag can access the independently hosted Webamp Skin Museum at the user's
request. No catalog previews or downloadable skins are bundled with Tonelag.
Those skins are third-party works whose copyright and redistribution terms may
vary; the Webamp MIT license does not automatically apply to them.

The Winamp name and trademarks belong to their respective owners. Compatibility
with Winamp Classic skin files does not imply endorsement or affiliation.
