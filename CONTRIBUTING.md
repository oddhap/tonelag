# Contributing

Run `pnpm check` and `cargo test --manifest-path src-tauri/Cargo.toml` before a
pull request. Run `pnpm bindings` whenever a Rust command/snapshot type changes.

Do not copy from the official Winamp source release. Behavior can be studied,
but distributed code must be original or come from a compatible open-source
project with its license/header preserved. Do not add skins, samples, logos, or
screenshots without an explicit redistribution license.

New audio formats need a synthetic fixture, corrupt-input case, seek/end test,
and a license review. Changes to unsafe custom AVIO code require a regression
test and `cargo clippy --all-targets -- -D warnings`.
