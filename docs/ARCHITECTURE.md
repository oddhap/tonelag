# Architecture

## Invariants

1. `AppController` is authoritative. React owns only transient selection and
   dialog state.
2. Every state mutation increments `AppSnapshot.revision`; frontends discard
   older snapshots.
3. Playback never resumes automatically after a process restart.
4. The audio callback performs no allocation or blocking I/O.
5. HTTP credentials are redacted before an error can be logged or displayed.
6. Spotify or another protected provider is not routed into FFmpeg/DSP.

## Modules

- `audio.rs`: decoder thread, FFmpeg codec/resampler, EQ, FFT, ring buffer, CPAL.
- `http_io.rs`: reqwest/rustls downloader, cancellable custom AVIO, ICY parsing.
- `controller.rs`: commands, queue transitions, state revisions, windows.
- `playlist.rs`: bounded M3U/M3U8/PLS parsing and export.
- `skin.rs`: archive validation and persistent skin store.
- `stream.rs`: redirects, remote playlist resolution, MIME and HLS policy.
- `system_media.rs`: isolated OS media-control adapter.
- `persistence.rs`: same-directory temporary file, flush, and atomic rename.

Rust types are exported by `ts-rs` into `src/bindings/generated`; frontend code
imports only through `src/bindings/contracts.ts`.

## Source capability boundary

`SourceCapabilities` controls UI and DSP behavior. Local files are seekable.
HTTP streams are live, non-seekable, metadata-updating sources. A future Spotify
provider must set `dspAllowed=false` and `analysisAllowed=false` and own its audio
path outside this module.

## Window model

Windows, macOS, and X11 use three decorationless webviews. Positions and playlist
height are persisted. Panels snap at 10 logical pixels; moving an attached main
panel moves its attached descendants. Wayland is detected before display and uses
one 275×464 logical window because clients cannot authoritatively position three
top-level surfaces.
