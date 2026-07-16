# Spotify integration gate

Spotify is not compiled into v1.

Both gates must pass before implementation:

1. Written policy/quota confirmation for a client that also plays local files
   and internet radio, including access beyond development mode.
2. An isolated Tauri spike proving OAuth PKCE, refresh, Web Playback SDK/EME,
   play/pause/seek, and reconnect on Windows/macOS/Linux x64 and ARM64.

If accepted, implement `SpotifySource` as a separate provider. It reports
`dspAllowed=false` and `analysisAllowed=false`; protected audio never enters
FFmpeg, EQ, visualization, recording, or caching. If either gate fails, remove
the spike and do not ship a partial integration.
