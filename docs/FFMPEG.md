# Native audio library policy

Release builds use FFmpeg 8.1.2 and libopenmpt 0.8.7 from the checksummed
archives in `scripts/ffmpeg/build.sh`. The configuration starts with
`--disable-everything`, enables only audio demux/decode/resampling, disables the
FFmpeg network stack, and never enables `--enable-gpl` or `--enable-nonfree`.

HTTP(S), redirects, playlists, cancellation, and ICY metadata are implemented
by Rust/rustls and exposed to FFmpeg through custom read-only AVIO. FFmpeg is
dynamically linked so users retain the LGPL relinking/replacement rights. Every
binary release must include:

- both exact source archives;
- `ffmpeg-config.txt` and `ffmpeg-config.log` from the target build;
- `THIRD_PARTY_NOTICES.md` and the versioned SBOM files;
- the shared FFmpeg and libopenmpt libraries used by the executable.

The public prerelease workflow is intentionally not a stable-release signal.
Each produced package must additionally be inspected on a clean machine to
confirm that no build-host library path remains and that only the intended
LGPL/BSD libraries are present.

On macOS, run `scripts/release/bundle-macos-libs.sh` after creating the `.app`
and before signing or building the DMG. The script recursively copies dylibs to
`Contents/Frameworks`, rewrites their load paths, and fails if a non-system
dependency comes from outside the pinned build prefix. This prevents an
accidental Homebrew development build from becoming a public artifact.
